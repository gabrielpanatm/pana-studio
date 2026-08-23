import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const FILES = Object.freeze([
  "src-tauri/src/kernel/performance.rs",
  "src-tauri/src/kernel/observability/mod.rs",
  "src-tauri/src/kernel/project_workspace/model.rs",
  "src-tauri/src/commands/project/lifecycle.rs",
  "src-tauri/src/commands/kernel_preview_pipeline.rs",
  "src-tauri/src/commands/css.rs",
  "src-tauri/src/commands/external_disk.rs",
  "src-tauri/src/project_model/incremental.rs",
  "src-tauri/src/source_graph/scan/incremental.rs",
  "src-tauri/src/project_model/cache.rs",
  "scripts/performance-report.mjs",
  "scripts/generate-performance-fixture.mjs",
  "scripts/run-performance-baseline.mjs",
  "scripts/check-bundle-size.mjs",
  "scripts/measure-feature-bundles.mjs",
]);

const OPERATION_OWNERS = Object.freeze([
  ["project_open", "src-tauri/src/commands/project/lifecycle.rs"],
  ["html_edit", "src-tauri/src/commands/kernel_preview_pipeline.rs"],
  ["css_edit", "src-tauri/src/commands/css.rs"],
  ["external_reconcile", "src-tauri/src/commands/external_disk.rs"],
  ["project_model_build", "src-tauri/src/kernel/performance.rs"],
]);

const REQUIRED_SCRIPTS = Object.freeze([
  "performance:report",
  "performance:fixture",
  "performance:baseline",
  "bundle:features",
]);

const RETIRED_PROJECTION_PATTERNS = Object.freeze([
  new RegExp(`\\b${["Tera", "Graph"].join("")}\\b`),
  new RegExp(`\\b${["tera", "graph"].join("_")}\\b`),
  new RegExp(["projectModel", "Tera", "Graph"].join("")),
  new RegExp(["tera", "Graph"].join("")),
]);
const RETIRED_PROJECTION_FILE = ["src-tauri/src/project_model/tera", "graph.rs"].join("_");
const SOURCE_EXTENSIONS = new Set([".rs", ".ts", ".svelte", ".js", ".mjs"]);
const FULL_DERIVED_BUILDERS = Object.freeze([
  "build_component_graph",
  "build_block_graph",
  "refresh_content_model_template_usages",
  "build_listing_item_catalog_from_workspace_projection",
  "build_dynamic_widget_graph_from_workspace_projection",
  "build_markdown_projections",
]);

function contains(source, pattern) {
  return typeof source === "string" && pattern.test(source);
}

export function analyzePerformanceObservabilitySources({
  sources,
  packageJson,
  requireCompleteLayout = false,
}) {
  const violations = [];
  const add = (code, file, detail) => violations.push({
    code,
    file,
    line: 1,
    ...(detail ? { detail } : {}),
  });
  if (requireCompleteLayout) {
    for (const file of FILES) {
      if (!sources.has(file)) add("missing-performance-file", file);
    }
  }

  const performance = sources.get("src-tauri/src/kernel/performance.rs");
  if (!contains(performance, /PERFORMANCE_SAMPLE_SCHEMA_VERSION\s*:\s*u32\s*=\s*3/)) {
    add("missing-versioned-performance-schema", "src-tauri/src/kernel/performance.rs");
  }
  if (!contains(performance, /performanceTotalUs/)
      || !contains(performance, /projectModelCloneUs/)) {
    add("incomplete-performance-schema", "src-tauri/src/kernel/performance.rs");
  }
  const observability = sources.get("src-tauri/src/kernel/observability/mod.rs");
  if (!contains(observability, /PerformanceSampled/)
      || !contains(observability, /kernel\.performance\.sampled/)) {
    add("missing-performance-event-kind", "src-tauri/src/kernel/observability/mod.rs");
  }
  for (const [operation, owner] of OPERATION_OWNERS) {
    if (!contains(sources.get(owner), new RegExp(`"${operation}"`))) {
      add("missing-performance-operation", owner, operation);
    }
  }

  const report = sources.get("src-tauri/src/project_model/incremental.rs");
  for (const field of [
    "model_clone_us",
    "template_parse_us",
    "component_graph_us",
    "block_graph_us",
    "content_model_us",
    "listing_items_us",
    "dynamic_widget_us",
    "markdown_us",
    "node_index_us",
    "duration_us",
  ]) {
    if (!contains(report, new RegExp(`\\b${field}\\b`))) {
      add("missing-project-model-phase", "src-tauri/src/project_model/incremental.rs", field);
    }
  }
  const templateHotPath = sources.get("src-tauri/src/source_graph/scan/incremental.rs");
  for (const builder of FULL_DERIVED_BUILDERS) {
    if (contains(templateHotPath, new RegExp(`\\b${builder}\\b`))) {
      add(
        "full-derived-builder-on-template-hot-path",
        "src-tauri/src/source_graph/scan/incremental.rs",
        builder,
      );
    }
  }
  const receipt = sources.get("src-tauri/src/kernel/project_workspace/model.rs");
  if (!contains(receipt, /#\[serde\(skip\)\][\s\S]{0,100}project_model_performance/)) {
    add("serialized-internal-performance-state", "src-tauri/src/kernel/project_workspace/model.rs");
  }
  if (contains(sources.get("src-tauri/src/project_model/cache.rs"), /\[Pană Studio\]\[perf\]/)) {
    add("legacy-project-model-perf-log", "src-tauri/src/project_model/cache.rs");
  }
  for (const [file, source] of sources) {
    if (
      file === RETIRED_PROJECTION_FILE
      || RETIRED_PROJECTION_PATTERNS.some((pattern) => contains(source, pattern))
    ) {
      add("retired-semantic-projection", file);
    }
  }

  const reportScript = sources.get("scripts/performance-report.mjs");
  if (!contains(reportScript, /readBoundedJsonLines/)
      || !contains(reportScript, /fullFallbackRate/)) {
    add("incomplete-performance-report", "scripts/performance-report.mjs");
  }
  const baselineRunner = sources.get("scripts/run-performance-baseline.mjs");
  if (!contains(baselineRunner, /releasePerformanceBudgets/)
      || !contains(baselineRunner, /evaluateReleasePerformanceBudgets/)) {
    add("missing-release-performance-budgets", "scripts/run-performance-baseline.mjs");
  }
  const fixture = sources.get("scripts/generate-performance-fixture.mjs");
  if (!contains(fixture, /pana-studio-performance/)
      || !contains(fixture, /Refusing to replace unowned directory/)) {
    add("unsafe-performance-fixture", "scripts/generate-performance-fixture.mjs");
  }
  const bundle = sources.get("scripts/check-bundle-size.mjs");
  for (const feature of ["SettingsWorkspace", "DesignSystemWorkspace", "VersionControlWorkspace", "AuditWorkspace"]) {
    if (!contains(bundle, new RegExp(feature))) {
      add("missing-lazy-feature-budget", "scripts/check-bundle-size.mjs", feature);
    }
  }

  for (const script of REQUIRED_SCRIPTS) {
    if (typeof packageJson?.scripts?.[script] !== "string") {
      add("missing-performance-package-script", "package.json", script);
    }
  }
  return {
    violations: violations.sort((left, right) => (
      left.file.localeCompare(right.file)
      || left.code.localeCompare(right.code)
      || String(left.detail ?? "").localeCompare(String(right.detail ?? ""))
    )),
  };
}

export function analyzePerformanceObservability(projectRoot = process.cwd()) {
  return analyzePerformanceObservabilitySources({
    sources: readArchitectureSources(projectRoot),
    packageJson: JSON.parse(readFileSync(path.join(projectRoot, "package.json"), "utf8")),
    requireCompleteLayout: true,
  });
}

function readArchitectureSources(projectRoot) {
  const sources = new Map(FILES.map((file) => [
    file,
    readFileSync(path.join(projectRoot, file), "utf8"),
  ]));
  for (const root of ["src-tauri/src", "src", "scripts", "tests"]) {
    collectSources(projectRoot, root, sources);
  }
  return sources;
}

function collectSources(projectRoot, relativeDirectory, sources) {
  const directory = path.join(projectRoot, relativeDirectory);
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const relativePath = path.posix.join(relativeDirectory, entry.name);
    if (entry.isDirectory()) {
      collectSources(projectRoot, relativePath, sources);
    } else if (entry.isFile() && SOURCE_EXTENSIONS.has(path.extname(entry.name))) {
      sources.set(relativePath, readFileSync(path.join(projectRoot, relativePath), "utf8"));
    }
  }
}

const isMain = process.argv[1]
  && path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url));
if (isMain) {
  const report = analyzePerformanceObservability();
  console.log(JSON.stringify(report, null, 2));
  if (report.violations.length > 0) process.exitCode = 1;
}
