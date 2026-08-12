import { readdirSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const clientOutput = join(
  projectRoot,
  ".svelte-kit/output/client/_app/immutable",
);
const maximumChunkBytes = 500_000;

function filesBelow(path) {
  return readdirSync(path, { withFileTypes: true }).flatMap((entry) => {
    const target = join(path, entry.name);
    return entry.isDirectory() ? filesBelow(target) : [target];
  });
}

const chunks = filesBelow(clientOutput)
  .filter((path) => path.endsWith(".js"))
  .map((path) => ({ path, bytes: statSync(path).size }))
  .sort((left, right) => right.bytes - left.bytes);

if (chunks.length === 0) {
  throw new Error("[bundle] client build contains no JavaScript chunks");
}

const oversized = chunks.filter(({ bytes }) => bytes >= maximumChunkBytes);
if (oversized.length > 0) {
  const details = oversized
    .map(({ path, bytes }) => `${relative(projectRoot, path)} (${bytes} bytes)`)
    .join("\n");
  throw new Error(
    `[bundle] JavaScript chunks must stay below ${maximumChunkBytes} bytes:\n${details}`,
  );
}

const largest = chunks[0];
console.log(
  `[bundle] ${chunks.length} client JS chunks; largest ${relative(projectRoot, largest.path)} (${largest.bytes} bytes)`,
);
