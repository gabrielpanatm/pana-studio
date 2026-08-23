import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";

const supportedExtensions = [".ts", ".svelte", ".js"];
const staticImportPattern =
  /(?:^|\n)\s*(?:import|export)\s+(type\s+)?(?:[^;"']+?\s+from\s+)?["']([^"']+)["']/g;

/** @param {string} directory @returns {string[]} */
function sourceFiles(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const target = path.join(directory, entry.name);
    if (entry.isDirectory()) return sourceFiles(target);
    return supportedExtensions.some((extension) => entry.name.endsWith(extension))
      ? [path.resolve(target)]
      : [];
  });
}

/**
 * @typedef {object} SourceLayerChunkOptions
 * @property {string} projectRoot
 * @property {string} entry
 * @property {string[]} chunkNames
 * @property {string[]} [excludedFragments]
 */

/**
 * Partitions the statically reachable application graph in dependency order.
 * A chunk may depend only on its own layer or an earlier layer, so splitting
 * the core cannot manufacture the circular chunks caused by domain globs.
 *
 * @param {SourceLayerChunkOptions} options
 * @returns {Map<string, string>}
 */
export function createSourceLayerChunkMap({
  projectRoot,
  entry,
  chunkNames,
  excludedFragments = [],
}) {
  const sourceRoot = path.resolve(projectRoot, "src");
  const files = sourceFiles(sourceRoot);
  const known = new Set(files);

  /** @param {string} importer @param {string} specifier @returns {string | null} */
  function resolveImport(importer, specifier) {
    const base = specifier.startsWith("$lib/")
      ? path.resolve(sourceRoot, "lib", specifier.slice("$lib/".length))
      : specifier.startsWith(".")
        ? path.resolve(path.dirname(importer), specifier)
        : null;
    if (!base) return null;
    return [
      base,
      ...supportedExtensions.map((extension) => `${base}${extension}`),
      ...supportedExtensions.map((extension) => path.join(base, `index${extension}`)),
    ].find((candidate) => known.has(candidate)) ?? null;
  }

  /** @type {Map<string, string[]>} */
  const graph = new Map();
  for (const file of files) {
    /** @type {string[]} */
    const dependencies = [];
    for (const match of readFileSync(file, "utf8").matchAll(staticImportPattern)) {
      if (match[1]) continue;
      const dependency = resolveImport(file, match[2]);
      if (dependency && !dependencies.includes(dependency)) dependencies.push(dependency);
    }
    graph.set(file, dependencies);
  }

  /** @type {string[]} */
  const ordered = [];
  /** @type {Set<string>} */
  const visiting = new Set();
  /** @type {Set<string>} */
  const visited = new Set();
  /** @param {string} file */
  function visit(file) {
    if (visited.has(file)) return;
    if (visiting.has(file)) throw new Error(`Static source cycle at ${file}`);
    visiting.add(file);
    for (const dependency of graph.get(file) ?? []) visit(dependency);
    visiting.delete(file);
    visited.add(file);
    ordered.push(file);
  }
  visit(path.resolve(projectRoot, entry));

  const included = ordered.filter((file) => {
    const normalized = file.replaceAll("\\", "/");
    return !excludedFragments.some((fragment) => normalized.includes(fragment));
  });
  const weights = included.map((file) => readFileSync(file).byteLength);
  const targetWeight = weights.reduce((total, weight) => total + weight, 0)
    / chunkNames.length;
  /** @type {Map<string, string>} */
  const chunks = new Map();
  let cumulativeWeight = 0;
  for (let index = 0; index < included.length; index += 1) {
    const layer = Math.min(
      Math.floor(cumulativeWeight / Math.max(targetWeight, 1)),
      chunkNames.length - 1,
    );
    chunks.set(included[index].replaceAll("\\", "/"), chunkNames[layer]);
    cumulativeWeight += weights[index];
  }
  return chunks;
}
