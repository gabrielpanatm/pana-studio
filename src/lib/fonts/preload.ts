import { primaryFontFamily, type FontRoleRow } from "$lib/fonts/model";
import type { FontInventory, LocalFontFile, SourceGraph, SourceGraphTemplate } from "$lib/types";

const PRELOAD_START = "<!-- pana-studio-font-preload:start -->";
const PRELOAD_END = "<!-- pana-studio-font-preload:end -->";

export function activeFontPreloadFiles(roles: FontRoleRow[], inventory: FontInventory | null): LocalFontFile[] {
  if (!inventory) return [];
  const activeFamilies = new Set(
    roles
      .map((role) => normalizeFontName(primaryFontFamily(role.variable?.value)))
      .filter(Boolean),
  );
  if (!activeFamilies.size) return [];

  const files = inventory.families
    .filter((family) => activeFamilies.has(normalizeFontName(family.family)))
    .flatMap((family) => family.files)
    .filter(isPreloadableFontFile);

  return uniqueFontFiles(files);
}

export function fontPreloadTargetCandidates(graph: SourceGraph | null): SourceGraphTemplate[] {
  if (!graph) return [];
  return graph.templates
    .filter((template) => !template.isPartial)
    .slice()
    .sort((left, right) => fontPreloadTargetScore(left) - fontPreloadTargetScore(right));
}

export function upsertFontPreloadBlock(source: string, files: LocalFontFile[]) {
  const block = buildFontPreloadBlock(files, headIndent(source));
  const existing = fontPreloadBlockRegex();

  if (existing.test(source)) {
    return block ? source.replace(existing, `${block}\n\n`) : source.replace(existing, "");
  }
  if (!block) return source;

  const insertion = `${block}\n\n`;
  const stylesheetIndex = firstStylesheetIndex(source);
  if (stylesheetIndex > -1) {
    return `${source.slice(0, stylesheetIndex)}${insertion}${source.slice(stylesheetIndex)}`;
  }

  const headEndIndex = source.search(/^\s*<\/head>/im);
  if (headEndIndex > -1) {
    return `${source.slice(0, headEndIndex)}${insertion}${source.slice(headEndIndex)}`;
  }

  return `${insertion}${source}`;
}

export function buildFontPreloadBlock(files: LocalFontFile[], indent = "") {
  const links = uniqueFontFiles(files)
    .filter(isPreloadableFontFile)
    .map((file) => `${indent}<link rel="preload" href="${publicFontUrl(file.file)}" as="font" type="${fontMimeType(file)}" crossorigin>`);
  if (!links.length) return "";
  return [`${indent}${PRELOAD_START}`, ...links, `${indent}${PRELOAD_END}`].join("\n");
}

function fontPreloadTargetScore(template: SourceGraphTemplate) {
  const name = template.name.toLowerCase();
  const file = template.file.toLowerCase();
  if (name === "base.html") return 0;
  if (file.endsWith("/templates/base.html")) return 1;
  if (name === "layout.html") return 2;
  if (file.endsWith("/templates/layout.html")) return 3;
  if (name.includes("base")) return 4;
  if (name.includes("layout")) return 5;
  return 10;
}

function firstStylesheetIndex(source: string) {
  const match = source.match(/^\s*<link\s+[^>]*rel=["']stylesheet["'][^>]*>/im);
  return match?.index ?? -1;
}

function fontPreloadBlockRegex() {
  return new RegExp(`^[ \\t]*${escapeRegExp(PRELOAD_START)}[\\s\\S]*?^[ \\t]*${escapeRegExp(PRELOAD_END)}[ \\t]*`, "m");
}

function headIndent(source: string) {
  const stylesheet = source.match(/^([ \t]*)<link\s+[^>]*rel=["']stylesheet["'][^>]*>/im);
  if (stylesheet?.[1]) return stylesheet[1];
  const headEnd = source.match(/^([ \t]*)<\/head>/im);
  return headEnd?.[1] || "  ";
}

function uniqueFontFiles(files: LocalFontFile[]) {
  const seen = new Set<string>();
  return files.filter((file) => {
    if (seen.has(file.file)) return false;
    seen.add(file.file);
    return true;
  });
}

function isPreloadableFontFile(file: LocalFontFile) {
  return ["woff2", "woff", "ttf", "otf"].includes(file.extension.toLowerCase());
}

function fontMimeType(file: LocalFontFile) {
  switch (file.extension.toLowerCase()) {
    case "woff2":
      return "font/woff2";
    case "woff":
      return "font/woff";
    case "ttf":
      return "font/ttf";
    case "otf":
      return "font/otf";
    default:
      return "font/woff2";
  }
}

function publicFontUrl(projectRelativeFile: string) {
  const staticRelative = projectRelativeFile
    .replace(/^themes\/[^/]+\/static\//, "")
    .replace(/^static\//, "");
  return `/${staticRelative.split("/").map(encodeURIComponent).join("/")}`;
}

function normalizeFontName(value: string | null | undefined) {
  return (value ?? "")
    .toLowerCase()
    .replace(/^['"]|['"]$/g, "")
    .replace(/[^a-z0-9]+/g, "")
    .trim();
}

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
