import assert from "node:assert/strict";
import test from "node:test";

import { analyzePerformanceObservabilitySources } from "../scripts/analyze-performance-observability.mjs";

function completeSources() {
  return new Map([
    ["src-tauri/src/kernel/performance.rs", `
      const PERFORMANCE_SAMPLE_SCHEMA_VERSION: u32 = 3;
      "performanceTotalUs"; "projectModelCloneUs"; "project_model_build";
    `],
    ["src-tauri/src/kernel/observability/mod.rs", "PerformanceSampled kernel.performance.sampled"],
    ["src-tauri/src/kernel/project_workspace/model.rs", "#[serde(skip)]\nproject_model_performance"],
    ["src-tauri/src/commands/project/lifecycle.rs", '"project_open"'],
    ["src-tauri/src/commands/kernel_preview_pipeline.rs", '"html_edit"'],
    ["src-tauri/src/commands/css.rs", '"css_edit"'],
    ["src-tauri/src/commands/external_disk.rs", '"external_reconcile"'],
    ["src-tauri/src/project_model/incremental.rs", `
      model_clone_us template_parse_us component_graph_us block_graph_us
      content_model_us listing_items_us dynamic_widget_us markdown_us
      node_index_us duration_us
    `],
    ["src-tauri/src/source_graph/scan/incremental.rs", "upsert local derived projections"],
    ["src-tauri/src/project_model/cache.rs", "canonical cache"],
    ["scripts/performance-report.mjs", "readBoundedJsonLines fullFallbackRate"],
    ["scripts/generate-performance-fixture.mjs", "pana-studio-performance Refusing to replace unowned directory"],
    [
      "scripts/run-performance-baseline.mjs",
      "baseline releasePerformanceBudgets evaluateReleasePerformanceBudgets",
    ],
    ["scripts/check-bundle-size.mjs", "SettingsWorkspace DesignSystemWorkspace VersionControlWorkspace AuditWorkspace"],
    ["scripts/measure-feature-bundles.mjs", "features"],
  ]);
}

const packageJson = {
  scripts: {
    "performance:report": "report",
    "performance:fixture": "fixture",
    "performance:baseline": "baseline",
    "bundle:features": "features",
  },
};

test("guard-ul acceptă schema, operațiile și tooling-ul canonic", () => {
  assert.deepEqual(analyzePerformanceObservabilitySources({
    sources: completeSources(),
    packageJson,
  }).violations, []);
});

test("guard-ul respinge o operație lipsă, logging legacy și un script absent", () => {
  const sources = completeSources();
  sources.set("src-tauri/src/commands/css.rs", "no sample");
  sources.set("src-tauri/src/project_model/cache.rs", "[Pană Studio][perf]");
  const brokenPackage = structuredClone(packageJson);
  delete brokenPackage.scripts["performance:baseline"];
  assert.deepEqual(
    new Set(analyzePerformanceObservabilitySources({
      sources,
      packageJson: brokenPackage,
    }).violations.map((item) => item.code)),
    new Set([
      "missing-performance-operation",
      "legacy-project-model-perf-log",
      "missing-performance-package-script",
    ]),
  );
});

test("guard-ul respinge reapariția proiecției semantice retrase", () => {
  const sources = completeSources();
  sources.set(
    "src-tauri/src/project_model/retired.rs",
    `struct ${["Tera", "Graph"].join("")} {}`,
  );
  assert.deepEqual(
    analyzePerformanceObservabilitySources({ sources, packageJson }).violations.map(
      (item) => item.code,
    ),
    ["retired-semantic-projection"],
  );
});

test("guard-ul respinge builderii derivați integrali pe hot-path-ul unui template", () => {
  const sources = completeSources();
  sources.set(
    "src-tauri/src/source_graph/scan/incremental.rs",
    "build_component_graph build_markdown_projections",
  );
  assert.deepEqual(
    analyzePerformanceObservabilitySources({ sources, packageJson }).violations.map(
      ({ code, detail }) => [code, detail],
    ),
    [
      ["full-derived-builder-on-template-hot-path", "build_component_graph"],
      ["full-derived-builder-on-template-hot-path", "build_markdown_projections"],
    ],
  );
});
