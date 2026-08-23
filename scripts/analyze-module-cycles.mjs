import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";

const sourceRoot = path.resolve("src");
const supportedExtensions = [".ts", ".svelte", ".js"];

function sourceFiles(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) return sourceFiles(entryPath);
    return supportedExtensions.some((extension) => entry.name.endsWith(extension))
      ? [entryPath]
      : [];
  });
}

const files = sourceFiles(sourceRoot).map((file) => path.resolve(file));
const knownFiles = new Set(files);

function importCandidates(basePath) {
  return [
    basePath,
    ...supportedExtensions.map((extension) => `${basePath}${extension}`),
    ...supportedExtensions.map((extension) => path.join(basePath, `index${extension}`)),
  ];
}

function resolveLocalImport(importer, specifier) {
  const basePath = specifier.startsWith("$lib/")
    ? path.resolve("src/lib", specifier.slice("$lib/".length))
    : specifier.startsWith(".")
      ? path.resolve(path.dirname(importer), specifier)
      : null;

  if (!basePath) return null;
  return importCandidates(basePath).find((candidate) => knownFiles.has(candidate)) ?? null;
}

const staticImportPattern =
  /(?:^|\n)\s*(?:import|export)\s+(type\s+)?(?:[^;"']+?\s+from\s+)?["']([^"']+)["']/g;
const graph = new Map();

for (const file of files) {
  const source = readFileSync(file, "utf8");
  const dependencies = new Set();
  for (const match of source.matchAll(staticImportPattern)) {
    if (match[1]) continue;
    const dependency = resolveLocalImport(file, match[2]);
    if (dependency) dependencies.add(dependency);
  }
  graph.set(file, [...dependencies]);
}

let nextIndex = 0;
const indices = new Map();
const lowLinks = new Map();
const stack = [];
const onStack = new Set();
const cycles = [];

function connect(file) {
  indices.set(file, nextIndex);
  lowLinks.set(file, nextIndex);
  nextIndex += 1;
  stack.push(file);
  onStack.add(file);

  for (const dependency of graph.get(file) ?? []) {
    if (!indices.has(dependency)) {
      connect(dependency);
      lowLinks.set(file, Math.min(lowLinks.get(file), lowLinks.get(dependency)));
    } else if (onStack.has(dependency)) {
      lowLinks.set(file, Math.min(lowLinks.get(file), indices.get(dependency)));
    }
  }

  if (lowLinks.get(file) !== indices.get(file)) return;

  const component = [];
  let member;
  do {
    member = stack.pop();
    onStack.delete(member);
    component.push(member);
  } while (member !== file);

  if (component.length > 1) cycles.push(component);
}

for (const file of files) {
  if (!indices.has(file)) connect(file);
}

const relativePath = (file) => path.relative(process.cwd(), file);
const report = cycles
  .sort((left, right) => right.length - left.length)
  .map((cycle) => ({
    size: cycle.length,
    files: cycle.map(relativePath).sort(),
  }));

console.log(JSON.stringify(report, null, 2));
if (report.length > 0) process.exitCode = 1;
