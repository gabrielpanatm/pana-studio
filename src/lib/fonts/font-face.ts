import type { LocalFontFamily, LocalFontFile } from "$lib/types";

export function buildManagedFontFaceBlock(family: LocalFontFamily) {
  const familyName = escapeCssString(family.family);
  const rules = family.files.map((file) => buildFontFaceRule(familyName, file)).join("\n\n");
  return buildManagedFontFaceCssBlock(family.family, rules);
}

export function upsertManagedFontFaceBlock(source: string, family: LocalFontFamily) {
  return upsertManagedFontFaceCssBlock(source, family.family, buildManagedFontFaceBlock(family));
}

export function buildManagedFontFaceCssBlock(familyName: string, cssBlock: string) {
  return [
    managedFontStartMarker(familyName),
    cssBlock.trim(),
    managedFontEndMarker(familyName),
  ].join("\n");
}

export function upsertManagedFontFaceCssBlock(source: string, familyName: string, cssBlock: string) {
  const block = cssBlock.includes(managedFontStartMarker(familyName))
    ? cssBlock.trim()
    : buildManagedFontFaceCssBlock(familyName, cssBlock);
  const existing = managedFontBlockRegex(familyName);

  if (existing.test(source)) {
    return source.replace(existing, block);
  }

  const insertIndex = indexAfterLastFontFace(source);
  if (insertIndex > -1) {
    const before = source.slice(0, insertIndex).trimEnd();
    const after = source.slice(insertIndex).trimStart();
    return `${before}\n\n${block}\n\n${after}`;
  }

  return `${block}\n\n${source}`;
}

function buildFontFaceRule(familyName: string, file: LocalFontFile) {
  const weight = file.weightRange ? `${file.weightRange.start} ${file.weightRange.end}` : (file.weight ?? 400);
  const style = file.style ?? "normal";
  const lines = [
    "@font-face {",
    `  font-family: '${familyName}';`,
    `  src: url('${publicFontUrl(file.file)}') format('${file.format}');`,
    `  font-weight: ${weight};`,
    `  font-style: ${style};`,
    "  font-display: swap;",
  ];
  if (file.unicodeRange) {
    lines.push(`  unicode-range: ${file.unicodeRange};`);
  }
  lines.push("}");
  return lines.join("\n");
}

function publicFontUrl(projectRelativeFile: string) {
  const staticRelative = projectRelativeFile
    .replace(/^themes\/[^/]+\/static\//, "")
    .replace(/^static\//, "");
  return `/${staticRelative.split("/").map(encodeURIComponent).join("/")}`;
}

function managedFontStartMarker(family: string) {
  return `/* pana-studio-font:${markerFamilyName(family)}:start */`;
}

function managedFontEndMarker(family: string) {
  return `/* pana-studio-font:${markerFamilyName(family)}:end */`;
}

function managedFontBlockRegex(family: string) {
  const marker = escapeRegExp(markerFamilyName(family));
  return new RegExp(
    String.raw`\/\* pana-studio-font:${marker}:start \*\/[\s\S]*?\/\* pana-studio-font:${marker}:end \*\/`,
    "m",
  );
}

function markerFamilyName(family: string) {
  return family.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "font";
}

function escapeCssString(value: string) {
  return value.replace(/\\/g, "\\\\").replace(/'/g, "\\'");
}

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function indexAfterLastFontFace(source: string) {
  const matches = [...source.matchAll(/@font-face\s*\{[\s\S]*?\}\s*/g)];
  const last = matches.at(-1);
  if (!last || last.index === undefined) return -1;
  return last.index + last[0].length;
}
