import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const checkOnly = process.argv.includes("--check");
const lock = JSON.parse(fs.readFileSync(path.join(root, "package-lock.json"), "utf8"));
const packageVersion = JSON.parse(fs.readFileSync(
  path.join(root, "node_modules/@tabler/icons/package.json"),
  "utf8",
)).version;
const lockedVersion = lock.packages?.["node_modules/@tabler/icons"]?.version;

if (!lockedVersion || packageVersion !== lockedVersion) {
  throw new Error(`Tabler Icons instalat (${packageVersion}) nu corespunde lockfile (${lockedVersion ?? "lipsă"}).`);
}

const metadata = JSON.parse(fs.readFileSync(
  path.join(root, "node_modules/@tabler/icons/icons.json"),
  "utf8",
));
const nodes = JSON.parse(fs.readFileSync(
  path.join(root, "node_modules/@tabler/icons/tabler-nodes-outline.json"),
  "utf8",
));
const ids = Object.keys(nodes).sort((left, right) => left.localeCompare(right, "en"));
const metadataIds = Object.keys(metadata).sort((left, right) => left.localeCompare(right, "en"));

if (JSON.stringify(ids) !== JSON.stringify(metadataIds)) {
  throw new Error("Metadata și geometria Tabler Outline nu au aceleași iconuri.");
}

const safeId = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
const safePathData = /^[ MmAaCcHhLlQqSsTtVvZz0-9+.,-]+$/;
const allowedAttributes = new Set(["d", "fill", "opacity", "stroke"]);
const icons = {};

for (const id of ids) {
  if (!safeId.test(id)) throw new Error(`ID Tabler invalid: ${id}`);
  const definition = metadata[id];
  const iconNodes = nodes[id];
  if (!definition || !Array.isArray(iconNodes) || iconNodes.length === 0 || iconNodes.length > 32) {
    throw new Error(`Definiție Tabler incompletă: ${id}`);
  }
  const category = String(definition.category || "Other").trim() || "Other";
  const tags = [...new Set((definition.tags || []).map((tag) => String(tag).trim()).filter(Boolean))];
  if (category.length > 80 || /[\u0000-\u001f\u007f]/.test(category)) {
    throw new Error(`Categorie Tabler invalidă în ${id}.`);
  }
  if (tags.length > 64 || tags.some((tag) => tag.length > 80 || /[\u0000-\u001f\u007f]/.test(tag))) {
    throw new Error(`Taguri Tabler invalide în ${id}.`);
  }
  for (const node of iconNodes) {
    if (!Array.isArray(node) || node.length !== 2 || node[0] !== "path") {
      throw new Error(`Nod SVG nepermis în ${id}.`);
    }
    const attributes = node[1];
    for (const [name, rawValue] of Object.entries(attributes)) {
      const value = String(rawValue);
      if (!allowedAttributes.has(name)) throw new Error(`Atribut SVG nepermis ${name} în ${id}.`);
      if (name === "d" && (!value || value.length > 8192 || !safePathData.test(value))) {
        throw new Error(`Geometrie SVG invalidă în ${id}.`);
      }
      if (name === "fill" && value !== "currentColor") throw new Error(`fill invalid în ${id}.`);
      if (name === "stroke" && value !== "none") throw new Error(`stroke invalid în ${id}.`);
      if (name === "opacity" && value !== ".5") throw new Error(`opacity invalid în ${id}.`);
    }
  }
  icons[id] = {
    category,
    tags,
    nodes: iconNodes,
  };
}

for (const required of ["home", "search", "settings", "star"]) {
  if (!icons[required]) throw new Error(`Iconul implicit obligatoriu lipsește: ${required}`);
}

const output = `${JSON.stringify({
  schemaVersion: 1,
  packId: "tabler-outline",
  packVersion: lockedVersion,
  license: "MIT",
  icons,
})}\n`;
const outputPath = path.join(
  root,
  "src-tauri/resources/icon-packs/tabler-outline-3.41.1.json",
);

if (checkOnly) {
  const current = fs.existsSync(outputPath) ? fs.readFileSync(outputPath, "utf8") : "";
  if (current !== output) {
    throw new Error("Registrul Tabler Outline este absent sau nu corespunde generatorului. Rulează npm run icons:generate.");
  }
  console.log(`Registrul Tabler Outline este valid: ${ids.length} iconuri, ${Buffer.byteLength(output)} bytes.`);
} else {
  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  fs.writeFileSync(outputPath, output);
  console.log(`Registru Tabler Outline generat: ${ids.length} iconuri, ${Buffer.byteLength(output)} bytes.`);
}
