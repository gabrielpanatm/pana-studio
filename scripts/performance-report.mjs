import {
  closeSync,
  fstatSync,
  openSync,
  readSync,
} from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const performanceSchemaVersion = 3;
export const defaultRequiredOperations = Object.freeze([
  "project_open",
  "html_edit",
  "css_edit",
  "external_reconcile",
  "project_model_build",
]);

function rounded(value) {
  return Math.round(value * 1_000) / 1_000;
}

export function percentile(sorted, ratio) {
  if (sorted.length === 0) return null;
  const index = Math.max(0, Math.min(
    sorted.length - 1,
    Math.ceil(sorted.length * ratio) - 1,
  ));
  return sorted[index] ?? null;
}

export function distribution(values) {
  const sorted = values
    .filter((value) => Number.isFinite(value) && value >= 0)
    .sort((left, right) => left - right);
  if (sorted.length === 0) return null;
  const p50Us = percentile(sorted, 0.5);
  const p95Us = percentile(sorted, 0.95);
  const p99Us = sorted.length >= 100 ? percentile(sorted, 0.99) : null;
  const maxUs = sorted.at(-1);
  return {
    sampleCount: sorted.length,
    p50Us,
    p95Us,
    p99Us,
    maxUs,
    p50Ms: rounded(p50Us / 1_000),
    p95Ms: rounded(p95Us / 1_000),
    p99Ms: p99Us === null ? null : rounded(p99Us / 1_000),
    maxMs: rounded(maxUs / 1_000),
  };
}

function increment(counter, key) {
  counter[key] = (counter[key] ?? 0) + 1;
}

function numericAttributes(events, predicate) {
  const values = new Map();
  for (const event of events) {
    for (const [key, value] of Object.entries(event.attributes ?? {})) {
      if (!predicate(key) || !Number.isFinite(value) || value < 0) continue;
      const bucket = values.get(key) ?? [];
      bucket.push(value);
      values.set(key, bucket);
    }
  }
  return Object.fromEntries(
    [...values.entries()]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, samples]) => [key, distribution(samples)]),
  );
}

export function summarizePerformanceEvents(events, options = {}) {
  const minSamples = options.minSamples ?? 20;
  const requiredOperations = options.requiredOperations ?? defaultRequiredOperations;
  const samples = events.filter((event) => (
    event?.attributes?.performanceSchemaVersion === performanceSchemaVersion
    && typeof event.attributes.performanceOperation === "string"
    && Number.isFinite(event.attributes.performanceTotalUs)
  ));
  const operations = {};
  for (const operation of [...new Set(samples.map((event) => (
    event.attributes.performanceOperation
  )))].sort()) {
    const operationEvents = samples.filter((event) => (
      event.attributes.performanceOperation === operation
    ));
    operations[operation] = {
      duration: distribution(operationEvents.map((event) => (
        event.attributes.performanceTotalUs
      ))),
      variants: operationEvents.reduce((counts, event) => {
        increment(counts, String(event.attributes.performanceVariant ?? "unknown"));
        return counts;
      }, {}),
      phases: numericAttributes(operationEvents, (key) => (
        key.endsWith("Us") && key !== "performanceTotalUs"
      )),
    };
  }

  const dedicatedProjectModelEvents = samples.filter((event) => (
    event.attributes.performanceOperation === "project_model_build"
    && typeof event.attributes.projectModelBuildMode === "string"
  ));
  const projectModelEvents = dedicatedProjectModelEvents.length > 0
    ? dedicatedProjectModelEvents
    : samples.filter((event) => (
      typeof event.attributes.projectModelBuildMode === "string"
    ));
  const buildModes = {};
  const fallbackReasons = {};
  for (const event of projectModelEvents) {
    const mode = event.attributes.projectModelBuildMode;
    increment(buildModes, mode);
    if (mode === "fullFallback") {
      increment(
        fallbackReasons,
        String(event.attributes.projectModelFallbackReason ?? "unknown"),
      );
    }
  }
  const fullFallbackCount = buildModes.fullFallback ?? 0;
  const insufficient = requiredOperations.flatMap((operation) => {
    const count = operations[operation]?.duration?.sampleCount ?? 0;
    return count < minSamples ? [{ operation, count, required: minSamples }] : [];
  });

  return {
    schemaVersion: 1,
    performanceSampleSchemaVersion: performanceSchemaVersion,
    eventCount: events.length,
    sampleCount: samples.length,
    minSamples,
    operations,
    projectModel: {
      sampleCount: projectModelEvents.length,
      buildModes,
      fullFallbackCount,
      fullFallbackRate: projectModelEvents.length === 0
        ? null
        : rounded(fullFallbackCount / projectModelEvents.length),
      fallbackReasons,
      build: distribution(projectModelEvents.map((event) => (
        event.attributes.projectModelBuildUs
      ))),
      clone: distribution(projectModelEvents.map((event) => (
        event.attributes.projectModelCloneUs
      ))),
    },
    locks: numericAttributes(samples, (key) => (
      /Lock(?:Wait|Held)Us$/.test(key) || /LocksHeldUs$/.test(key)
    )),
    insufficient,
    complete: insufficient.length === 0,
  };
}

export function readBoundedJsonLines(path, maxBytes = 8 * 1024 * 1024) {
  if (!Number.isSafeInteger(maxBytes) || maxBytes <= 0) {
    throw new Error("maxBytes must be a positive safe integer");
  }
  const descriptor = openSync(path, "r");
  try {
    const size = fstatSync(descriptor).size;
    const start = Math.max(0, size - maxBytes);
    const length = size - start;
    const buffer = Buffer.alloc(length);
    const bytesRead = readSync(descriptor, buffer, 0, length, start);
    let source = buffer.subarray(0, bytesRead).toString("utf8");
    if (start > 0) {
      const firstNewline = source.indexOf("\n");
      source = firstNewline === -1 ? "" : source.slice(firstNewline + 1);
    }
    const events = [];
    const diagnostics = [];
    for (const [index, line] of source.split("\n").entries()) {
      if (!line.trim()) continue;
      try {
        events.push(JSON.parse(line));
      } catch (error) {
        diagnostics.push(`line ${index + 1}: ${error.message}`);
      }
    }
    return {
      path: resolve(path),
      fileBytes: size,
      scannedBytes: bytesRead,
      truncated: start > 0,
      events,
      diagnostics,
    };
  } finally {
    closeSync(descriptor);
  }
}

function parseArguments(argv) {
  const options = {
    logs: [],
    minSamples: 20,
    maxBytes: 8 * 1024 * 1024,
    allowInsufficient: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--log") options.logs.push(argv[++index]);
    else if (argument === "--min-samples") options.minSamples = Number(argv[++index]);
    else if (argument === "--max-bytes") options.maxBytes = Number(argv[++index]);
    else if (argument === "--allow-insufficient") options.allowInsufficient = true;
    else throw new Error(`Unknown argument: ${argument}`);
  }
  if (options.logs.length === 0 && process.env.PANA_PERFORMANCE_LOG) {
    options.logs.push(process.env.PANA_PERFORMANCE_LOG);
  }
  if (options.logs.length === 0) {
    throw new Error("Use --log <kernel.jsonl> or set PANA_PERFORMANCE_LOG.");
  }
  if (!Number.isSafeInteger(options.minSamples) || options.minSamples <= 0) {
    throw new Error("--min-samples must be a positive integer.");
  }
  return options;
}

function runCli() {
  const options = parseArguments(process.argv.slice(2));
  const scans = options.logs.map((path) => readBoundedJsonLines(path, options.maxBytes));
  const report = summarizePerformanceEvents(
    scans.flatMap((scan) => scan.events),
    { minSamples: options.minSamples },
  );
  console.log(JSON.stringify({
    ...report,
    sources: scans.map(({ events: _events, ...scan }) => scan),
  }, null, 2));
  if (!report.complete && !options.allowInsufficient) process.exitCode = 1;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    runCli();
  } catch (error) {
    console.error(`[performance] ${error.message}`);
    process.exitCode = 2;
  }
}
