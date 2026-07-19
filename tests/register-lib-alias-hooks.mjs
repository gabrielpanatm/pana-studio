import { existsSync, statSync } from "node:fs";
import { registerHooks } from "node:module";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const srcLib = resolve(root, "src/lib");

const moduleSuffixes = ["", ".ts", ".js", ".mjs", ".svelte"];

function existingModulePath(basePath) {
  for (const suffix of moduleSuffixes) {
    const candidate = `${basePath}${suffix}`;
    if (existsSync(candidate) && statSync(candidate).isFile()) return candidate;
  }

  const indexCandidate = resolve(basePath, "index.ts");
  if (existsSync(indexCandidate) && statSync(indexCandidate).isFile()) return indexCandidate;

  throw new Error(`Nu pot rezolva aliasul $lib pentru ${basePath}`);
}

registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier === "$lib" || specifier.startsWith("$lib/")) {
      const relativePath = specifier === "$lib" ? "" : specifier.slice("$lib/".length);
      const targetPath = existingModulePath(resolve(srcLib, relativePath));
      return { url: pathToFileURL(targetPath).href, shortCircuit: true };
    }

    // Node's type-stripping runtime does not add TypeScript extensions to
    // relative ESM imports. Production bundling does, so mirror that resolver
    // behavior for controller-level tests that cross local barrel modules.
    if (
      context.parentURL?.startsWith("file:")
      && (specifier.startsWith("./") || specifier.startsWith("../"))
      && !specifier.match(/\.(?:[cm]?[jt]s|svelte)$/)
    ) {
      const parentPath = fileURLToPath(context.parentURL);
      const targetPath = existingModulePath(resolve(dirname(parentPath), specifier));
      return { url: pathToFileURL(targetPath).href, shortCircuit: true };
    }

    return nextResolve(specifier, context);
  },
});
