import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = "src/lib/components/creation/design-system/";
const SHELL = "src/lib/components/creation/DesignSystemWorkspace.svelte";
const CATALOG_OWNER = `${ROOT}catalog-state.svelte.ts`;
const FONT_OWNER = "src/lib/fonts/manager-state.svelte.ts";
const FONT_CONTROLLER = `${ROOT}font-manager/controller.svelte.ts`;
const EXPECTED = new Set([
  SHELL,
  `${ROOT}DesignTokensWorkspace.svelte`,
  `${ROOT}DesignClassesWorkspace.svelte`,
  `${ROOT}StylesheetsWorkspace.svelte`,
  `${ROOT}FontManagerWorkspace.svelte`,
  `${ROOT}ResourceWorkspaceShell.svelte`,
  `${ROOT}contracts.ts`,
  CATALOG_OWNER,
  `${ROOT}font-manager/FontInstaller.svelte`,
  `${ROOT}font-manager/FontDetail.svelte`,
  FONT_CONTROLLER,
  FONT_OWNER,
]);
const BOUNDARY_OWNERS = new Map([
  ["readDesignTokenCatalog", CATALOG_OWNER],
  ["readThemeStyleCatalog", CATALOG_OWNER],
  ["getFontManager", FONT_OWNER],
  ["searchGoogleFonts", FONT_CONTROLLER],
  ["downloadGoogleFontFamily", FONT_CONTROLLER],
  ["planLocalFontImport", FONT_CONTROLLER],
  ["applyLocalFontImport", FONT_CONTROLLER],
  ["installBundledFontFamily", FONT_CONTROLLER],
  ["planFontFamilyRemoval", FONT_CONTROLLER],
  ["removeFontFamily", FONT_CONTROLLER],
]);

function filesUnder(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const target = path.join(directory, entry.name);
    return entry.isDirectory() ? filesUnder(target) : /\.(?:ts|svelte)$/.test(entry.name) ? [target] : [];
  });
}

function lineForOffset(source, offset) {
  return source.slice(0, offset).split("\n").length;
}

export function analyzeDesignSystemUiSources({ sources, requireCompleteLayout = false }) {
  const violations = [];
  if (requireCompleteLayout) {
    for (const file of EXPECTED) {
      if (!sources.has(file)) violations.push({ code: "missing-design-system-module", file, line: 1 });
    }
  }

  for (const [rawFile, source] of sources) {
    const file = rawFile.split(path.sep).join("/");
    const lines = source.split("\n").length;
    if (file === SHELL && lines > 350) violations.push({ code: "oversized-design-system-shell", file, line: 1, detail: `${lines}>350` });
    if ((file.startsWith(ROOT) || file === FONT_OWNER) && lines > 650) {
      violations.push({ code: "oversized-design-system-module", file, line: 1, detail: `${lines}>650` });
    }
    if (file === `${ROOT}index.ts` || file === `${ROOT}font-manager/index.ts`) {
      violations.push({ code: "design-system-barrel", file, line: 1 });
    }

    if (file === SHELL) {
      for (const pattern of [
        /\$lib\/(?:fonts|css)\/io/,
        /createProjectTextFile/,
        /\bdetailMode\b/,
        /<DesignTokenCatalog\b/,
        /\b(?:searchGoogleFonts|downloadGoogleFontFamily|getFontPreviewAsset)\b/,
      ]) {
        const match = pattern.exec(source);
        if (match) violations.push({ code: "design-system-shell-domain-logic", file, line: lineForOffset(source, match.index) });
      }
      for (const owner of ["DesignTokenCatalogState", "ThemeStyleCatalogState", "FontManagerState"]) {
        const count = [...source.matchAll(new RegExp(`new\\s+${owner}\\b`, "g"))].length;
        if (count !== 1) violations.push({ code: "design-system-owner-instantiation", file, line: 1, detail: `${owner}:${count}` });
      }
    }

    if (file.startsWith(ROOT)) {
      for (const match of source.matchAll(/(?:@tauri-apps\/api|\binvoke\s*\()/g)) {
        violations.push({ code: "direct-design-system-invoke", file, line: lineForOffset(source, match.index) });
      }
      if (/\bstyleUsageCount\b/.test(source) || /relations[^\n]*\.filter\s*\(/.test(source)) {
        violations.push({ code: "per-stylesheet-relation-scan", file, line: 1 });
      }
    }

    if (file === SHELL || file.startsWith(ROOT) || file === FONT_OWNER) {
      for (const [symbol, owner] of BOUNDARY_OWNERS) {
        for (const match of source.matchAll(new RegExp(`\\b${symbol}\\b`, "g"))) {
          if (file !== owner) violations.push({ code: "design-system-boundary-owner-bypass", file, line: lineForOffset(source, match.index), detail: symbol });
        }
      }
    }
  }

  if (requireCompleteLayout) {
    const tokenCatalog = sources.get("src/lib/components/creation/DesignTokenCatalog.svelte") ?? "";
    if (/\bquery\b|\bnormalizedQuery\b/.test(tokenCatalog)) {
      violations.push({ code: "duplicate-design-token-filter", file: "src/lib/components/creation/DesignTokenCatalog.svelte", line: 1 });
    }
    const stylesheets = sources.get(`${ROOT}StylesheetsWorkspace.svelte`) ?? "";
    if (!/new Map<string, number>\(\)/.test(stylesheets) || !/relation\.kind !== "usesStyle"/.test(stylesheets)) {
      violations.push({ code: "missing-stylesheet-usage-index", file: `${ROOT}StylesheetsWorkspace.svelte`, line: 1 });
    }
  }

  return { violations: violations.sort((a, b) => a.file.localeCompare(b.file) || a.line - b.line || a.code.localeCompare(b.code)) };
}

export function analyzeDesignSystemUiModularity(projectRoot = process.cwd()) {
  const files = filesUnder(path.join(projectRoot, "src"));
  return analyzeDesignSystemUiSources({
    sources: new Map(files.map((file) => [path.relative(projectRoot, file).split(path.sep).join("/"), readFileSync(file, "utf8")])),
    requireCompleteLayout: true,
  });
}

const isMain = process.argv[1] && path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url));
if (isMain) {
  const report = analyzeDesignSystemUiModularity();
  console.log(JSON.stringify(report, null, 2));
  if (report.violations.length > 0) process.exitCode = 1;
}
