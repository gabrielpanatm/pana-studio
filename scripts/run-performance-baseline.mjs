import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { generatePerformanceFixture } from "./generate-performance-fixture.mjs";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const prefix = "[pana-performance] ";
const requiredOperations = [
  "project_open",
  "html_edit",
  "css_edit",
  "external_reconcile",
  "project_model_build",
];

export const releasePerformanceBudgets = Object.freeze({
  external_reconcile: 10_000,
  css_edit: 1_500,
  html_edit: 20_000,
  project_model_build: 20_000,
  project_open: 40_000,
  htmlFullP95Us: 50_000,
  projectModelCloneUs: 1_500,
});

export function evaluateReleasePerformanceBudgets(samples) {
  const byOperation = new Map(samples.map((sample) => [sample.operation, sample]));
  const violations = [];
  for (const operation of requiredOperations) {
    const actual = byOperation.get(operation)?.p95Us;
    const budget = releasePerformanceBudgets[operation];
    if (!Number.isFinite(actual) || actual > budget) {
      violations.push({ operation, metric: "p95Us", actual: actual ?? null, budget });
    }
  }
  const html = byOperation.get("html_edit");
  for (const [metric, budget] of [
    ["fullP95Us", releasePerformanceBudgets.htmlFullP95Us],
    ["projectModelCloneUs", releasePerformanceBudgets.projectModelCloneUs],
  ]) {
    const actual = html?.[metric];
    if (!Number.isFinite(actual) || actual > budget) {
      violations.push({ operation: "html_edit", metric, actual: actual ?? null, budget });
    }
  }
  return violations;
}

export function parseBaselineArguments(argv) {
  const options = {
    profile: "release",
    output: null,
    fixtureRoot: null,
    pageCount: null,
    componentCount: null,
    nodeCount: null,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--profile") options.profile = argv[++index];
    else if (argument === "--output") options.output = argv[++index];
    else if (argument === "--fixture-root") options.fixtureRoot = argv[++index];
    else if (argument === "--pages") options.pageCount = Number(argv[++index]);
    else if (argument === "--components") options.componentCount = Number(argv[++index]);
    else if (argument === "--nodes") options.nodeCount = Number(argv[++index]);
    else throw new Error(`Unknown argument: ${argument}`);
  }
  if (!/^[a-zA-Z0-9_-]+$/.test(options.profile)) {
    throw new Error("Invalid Cargo profile");
  }
  for (const [name, value] of Object.entries({
    pageCount: options.pageCount,
    componentCount: options.componentCount,
    nodeCount: options.nodeCount,
  })) {
    if (value !== null && (!Number.isSafeInteger(value) || value <= 0)) {
      throw new Error(`${name} must be a positive integer`);
    }
  }
  return options;
}

export function parsePerformanceLines(source) {
  return source.split("\n").flatMap((line) => {
    const index = line.indexOf(prefix);
    if (index === -1) return [];
    return [JSON.parse(line.slice(index + prefix.length))];
  });
}

function run() {
  const options = parseBaselineArguments(process.argv.slice(2));
  const fixture = generatePerformanceFixture({
    root: options.fixtureRoot ?? undefined,
    ...(options.pageCount === null ? {} : { pageCount: options.pageCount }),
    ...(options.componentCount === null ? {} : { componentCount: options.componentCount }),
    ...(options.nodeCount === null ? {} : { nodeCount: options.nodeCount }),
  });
  const profileArguments = options.profile === "release"
    ? ["--release"]
    : ["--profile", options.profile];
  const result = spawnSync("cargo", [
    "test",
    "--manifest-path",
    "src-tauri/Cargo.toml",
    ...profileArguments,
    "performance_baseline_",
    "--",
    "--ignored",
    "--nocapture",
    "--test-threads=1",
  ], {
    cwd: projectRoot,
    encoding: "utf8",
    env: {
      ...process.env,
      PANA_PERFORMANCE_BENCH_PROJECT: fixture.root,
    },
    maxBuffer: 64 * 1024 * 1024,
  });
  const samples = parsePerformanceLines(`${result.stdout ?? ""}\n${result.stderr ?? ""}`);
  const missing = requiredOperations.filter((operation) => (
    !samples.some((sample) => sample.operation === operation)
  ));
  const budgetViolations = options.profile === "release"
    ? evaluateReleasePerformanceBudgets(samples)
    : [];
  const report = {
    schemaVersion: 1,
    status: result.status === 0 && missing.length === 0 && budgetViolations.length === 0
      ? "complete"
      : "failed",
    cargoStatus: result.status,
    profile: options.profile,
    fixture,
    budgets: options.profile === "release" ? releasePerformanceBudgets : null,
    budgetViolations,
    missingOperations: missing,
    samples,
  };
  const serialized = `${JSON.stringify(report, null, 2)}\n`;
  if (options.output) {
    const output = resolve(options.output);
    mkdirSync(dirname(output), { recursive: true });
    writeFileSync(output, serialized);
  }
  process.stdout.write(result.stdout ?? "");
  process.stderr.write(result.stderr ?? "");
  if (result.error && result.status === null) throw result.error;
  if (result.status !== 0) {
    throw new Error(`Cargo performance baseline exited with ${result.status}`);
  }
  if (missing.length > 0) {
    throw new Error(`Missing performance baselines: ${missing.join(", ")}`);
  }
  if (budgetViolations.length > 0) {
    throw new Error(`Release performance budgets exceeded: ${JSON.stringify(budgetViolations)}`);
  }
  console.log(`[performance] ${samples.length} baseline operations captured`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    run();
  } catch (error) {
    console.error(`[performance] ${error.message}`);
    process.exitCode = 1;
  }
}
