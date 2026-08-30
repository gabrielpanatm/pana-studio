import assert from "node:assert/strict";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";

import {
  defaultRequiredOperations,
  distribution,
  readBoundedJsonLines,
  summarizePerformanceEvents,
} from "../scripts/performance-report.mjs";
import { generatePerformanceFixture } from "../scripts/generate-performance-fixture.mjs";
import { measureLazyFeatureGraphs } from "../scripts/check-bundle-size.mjs";
import {
  evaluateReleasePerformanceBudgets,
  parseBaselineArguments,
  parsePerformanceLines,
  releasePerformanceBudgets,
} from "../scripts/run-performance-baseline.mjs";

test("raportul calculează percentila nearest-rank și agregă fallback/lock-uri", () => {
  assert.deepEqual(distribution([5_000, 1_000, 2_000, 3_000, 4_000]), {
    sampleCount: 5,
    p50Us: 3_000,
    p95Us: 5_000,
    p99Us: null,
    maxUs: 5_000,
    p50Ms: 3,
    p95Ms: 5,
    p99Ms: null,
    maxMs: 5,
  });
  const events = defaultRequiredOperations.flatMap((operation) => (
    Array.from({ length: 20 }, (_, index) => ({
      attributes: {
        performanceSchemaVersion: 3,
        performanceOperation: operation,
        performanceVariant: index < 5 ? "fullFallback" : "incremental",
        performanceTotalUs: 1_000 + index,
        currentRootLockWaitUs: index,
        ...(operation === "project_model_build" ? {
          projectModelBuildMode: index < 5 ? "fullFallback" : "incremental",
          projectModelFallbackReason: index < 5 ? "created_or_deleted_source" : null,
          projectModelBuildUs: 800 + index,
          projectModelCloneUs: 100 + index,
        } : {}),
      },
    }))
  ));
  const report = summarizePerformanceEvents(events);
  assert.equal(report.complete, true);
  assert.equal(report.projectModel.sampleCount, 20);
  assert.equal(report.projectModel.fullFallbackRate, 0.25);
  assert.deepEqual(report.projectModel.fallbackReasons, {
    created_or_deleted_source: 5,
  });
  assert.equal(report.locks.currentRootLockWaitUs.p95Us, 18);
});

test("citirea JSONL rămâne bounded și ignoră prima linie parțială", (context) => {
  const root = mkdtempSync(join(tmpdir(), "pana-performance-report-"));
  context.after(() => rmSync(root, { recursive: true, force: true }));
  const log = join(root, "kernel.jsonl");
  const lines = Array.from({ length: 30 }, (_, index) => JSON.stringify({ index }));
  writeFileSync(log, `${lines.join("\n")}\n`);
  const scan = readBoundedJsonLines(log, 100);
  assert.equal(scan.truncated, true);
  assert.equal(scan.events.at(-1).index, 29);
  assert.ok(scan.events[0].index > 0);
  assert.deepEqual(scan.diagnostics, []);
});

test("fixture-ul mare este determinist și nu suprascrie directoare străine", (context) => {
  const parent = mkdtempSync(join(tmpdir(), "pana-performance-fixture-test-"));
  context.after(() => rmSync(parent, { recursive: true, force: true }));
  const root = join(parent, "owned");
  const first = generatePerformanceFixture({
    root,
    pageCount: 3,
    componentCount: 2,
    nodeCount: 5,
  });
  const indexBefore = readFileSync(join(root, "templates/index.html"), "utf8");
  const second = generatePerformanceFixture({
    root,
    pageCount: 3,
    componentCount: 2,
    nodeCount: 5,
  });
  assert.deepEqual(second, first);
  assert.equal(readFileSync(join(root, "templates/index.html"), "utf8"), indexBefore);
  assert.equal(first.expectedSourceFileCount, 18);
  assert.match(indexBefore, /performance-badge/);
  assert.doesNotMatch(indexBefore, /{%\s*import\b|\w+::\w+/);
  assert.match(indexBefore, /data-pana-block="counter"/);
  assert.match(indexBefore, /pana:widget schema=2/);
  assert.equal(
    readFileSync(join(root, ".panastudio/listing-items.toml"), "utf8")
      .includes('templateName = "listing-items/service-card.html"'),
    true,
  );

  const unowned = join(parent, "unowned");
  mkdirSync(unowned);
  writeFileSync(join(unowned, "keep.txt"), "owned by the user");
  assert.throws(
    () => generatePerformanceFixture({ root: unowned }),
    /Refusing to replace unowned directory/,
  );
});

test("graful lazy exclude dependențele de boot și păstrează contribuțiile", (context) => {
  const root = mkdtempSync(join(tmpdir(), "pana-feature-graph-"));
  context.after(() => rmSync(root, { recursive: true, force: true }));
  const manifest = {
    start: { name: "entry/start", file: "start.js", imports: ["common"] },
    app: { name: "entry/app", file: "app.js" },
    node0: { name: "nodes/0", file: "node0.js" },
    node2: { name: "nodes/2", file: "node2.js" },
    common: { file: "common.js" },
    "src/lib/i18n/generated/catalog.ro.ts": { file: "ro.js" },
    settings: {
      name: "SettingsWorkspace",
      file: "settings.js",
      imports: ["common", "settings-dependency"],
    },
    "settings-dependency": { file: "settings-dependency.js" },
  };
  for (const entry of Object.values(manifest)) {
    const target = join(root, entry.file);
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, entry.file);
  }
  const [graph] = measureLazyFeatureGraphs(manifest, root, [{
    feature: "settings",
    entryName: "SettingsWorkspace",
    maximumBytes: 100,
    maximumGzipBytes: 100,
  }]);
  assert.deepEqual(
    new Set(graph.entries.map((entry) => entry.key)),
    new Set(["settings", "settings-dependency"]),
  );
  assert.equal(graph.entries.some((entry) => entry.key === "common"), false);
});

test("runner-ul parsează doar liniile baseline standardizate", () => {
  const samples = parsePerformanceLines([
    "noise",
    '[pana-performance] {"operation":"html_edit","p95Us":12}',
    "more noise",
  ].join("\n"));
  assert.deepEqual(samples, [{ operation: "html_edit", p95Us: 12 }]);
  assert.deepEqual(
    parseBaselineArguments(["--profile", "dev", "--pages", "40", "--components", "20", "--nodes", "200"]),
    {
      profile: "dev",
      output: null,
      fixtureRoot: null,
      pageCount: 40,
      componentCount: 20,
      nodeCount: 200,
    },
  );
});

test("bugetele release acoperă fixture-ul extins și raportează depășirea exactă", () => {
  const samples = defaultRequiredOperations.map((operation) => ({
    operation,
    p95Us: releasePerformanceBudgets[operation],
    ...(operation === "html_edit" ? {
      fullP95Us: releasePerformanceBudgets.htmlFullP95Us,
      projectModelCloneUs: releasePerformanceBudgets.projectModelCloneUs,
    } : {}),
  }));
  assert.deepEqual(evaluateReleasePerformanceBudgets(samples), []);
  samples.find((sample) => sample.operation === "html_edit").p95Us += 1;
  assert.deepEqual(evaluateReleasePerformanceBudgets(samples), [{
    operation: "html_edit",
    metric: "p95Us",
    actual: releasePerformanceBudgets.html_edit + 1,
    budget: releasePerformanceBudgets.html_edit,
  }]);
});
