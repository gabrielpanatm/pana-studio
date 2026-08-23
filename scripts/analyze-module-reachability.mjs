import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";

const sourceRoot = path.resolve("src");
const routesRoot = path.resolve("src/routes");
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
const dynamicImportPattern = /import\s*\(\s*["']([^"']+)["']\s*\)/g;
const runtimeGraph = new Map();
const typeGraph = new Map();

for (const file of files) {
  const source = readFileSync(file, "utf8");
  const runtimeDependencies = new Set();
  const typeDependencies = new Set();
  for (const match of source.matchAll(staticImportPattern)) {
    const dependency = resolveLocalImport(file, match[2]);
    if (!dependency) continue;
    if (match[1]) typeDependencies.add(dependency);
    else runtimeDependencies.add(dependency);
  }
  for (const match of source.matchAll(dynamicImportPattern)) {
    const dependency = resolveLocalImport(file, match[1]);
    if (dependency) runtimeDependencies.add(dependency);
  }
  runtimeGraph.set(file, [...runtimeDependencies]);
  typeGraph.set(file, [...typeDependencies]);
}

const reachable = new Set();
const pending = files.filter((file) => file.startsWith(`${routesRoot}${path.sep}`));
while (pending.length > 0) {
  const file = pending.pop();
  if (reachable.has(file)) continue;
  reachable.add(file);
  // Runtime and type-only edges both make a source module intentional, but
  // keeping the graphs separate prevents compile-time contracts from being
  // mistaken for boot/runtime dependencies by architecture checks.
  pending.push(...(runtimeGraph.get(file) ?? []), ...(typeGraph.get(file) ?? []));
}

const relativePath = (file) => path.relative(process.cwd(), file);
const unreachable = files
  .filter((file) => file.includes(`${path.sep}src${path.sep}lib${path.sep}`))
  .filter((file) => !reachable.has(file))
  .map(relativePath)
  .sort();

console.log(JSON.stringify(unreachable, null, 2));
if (unreachable.length > 0) process.exitCode = 1;
