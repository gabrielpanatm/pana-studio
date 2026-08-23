import assert from "node:assert/strict";
import test from "node:test";
import { analyzeDesignSystemUiSources } from "../scripts/analyze-design-system-ui-modularity.mjs";

function analyze(sources) {
  return analyzeDesignSystemUiSources({ sources: new Map(sources) });
}

test("guard-ul acceptă shell-ul subțire și ownerii canonici", () => {
  const report = analyze([
    ["src/lib/components/creation/DesignSystemWorkspace.svelte", `
      const tokens = new DesignTokenCatalogState(authority);
      const themes = new ThemeStyleCatalogState(authority);
      const fonts = new FontManagerState(identity);
    `],
    ["src/lib/components/creation/design-system/catalog-state.svelte.ts", `
      import { readDesignTokenCatalog, readThemeStyleCatalog } from "$lib/css/io";
    `],
    ["src/lib/fonts/manager-state.svelte.ts", `import { getFontManager } from "$lib/fonts/io";`],
    ["src/lib/components/creation/design-system/font-manager/controller.svelte.ts", `
      import { searchGoogleFonts, downloadGoogleFontFamily, planLocalFontImport,
        applyLocalFontImport, installBundledFontFamily, planFontFamilyRemoval,
        removeFontFamily } from "$lib/fonts/io";
    `],
  ]);
  assert.deepEqual(report.violations, []);
});

test("guard-ul respinge logica de domeniu și ownerii dubli în shell", () => {
  const report = analyze([
    ["src/lib/components/creation/DesignSystemWorkspace.svelte", `
      import { searchGoogleFonts } from "$lib/fonts/io";
      let detailMode = "info";
      const first = new FontManagerState(identity);
      const second = new FontManagerState(identity);
    `],
  ]);
  assert.deepEqual(new Set(report.violations.map((item) => item.code)), new Set([
    "design-system-shell-domain-logic",
    "design-system-owner-instantiation",
    "design-system-boundary-owner-bypass",
  ]));
});

test("guard-ul respinge scanarea relațiilor per rând, invoke-ul și modulele supradimensionate", () => {
  const report = analyze([
    ["src/lib/components/creation/design-system/StylesheetsWorkspace.svelte", `
      import { invoke } from "@tauri-apps/api/core";
      function styleUsageCount() { return graph.relations.filter(Boolean).length; }
      ${"\n".repeat(650)}
    `],
  ]);
  assert.deepEqual(new Set(report.violations.map((item) => item.code)), new Set([
    "direct-design-system-invoke",
    "per-stylesheet-relation-scan",
    "oversized-design-system-module",
  ]));
});
