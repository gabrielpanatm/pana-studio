import { resolve, sep } from "node:path";

export const viteDevIgnoredRoots = Object.freeze([
  "src-tauri",
  "benchmark-results",
  "tools/performance-benchmark/target",
]);

/**
 * @param {string} projectRoot
 * @returns {(candidatePath: string) => boolean}
 */
export function createViteDevWatchIgnored(projectRoot) {
  const ignoredRoots = viteDevIgnoredRoots.map((path) => resolve(projectRoot, path));

  return (candidatePath) => {
    const candidate = resolve(projectRoot, candidatePath);
    return ignoredRoots.some((root) => (
      candidate === root || candidate.startsWith(`${root}${sep}`)
    ));
  };
}
