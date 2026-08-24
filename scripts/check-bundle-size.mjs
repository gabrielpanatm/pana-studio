import { readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const clientOutput = join(
  projectRoot,
  ".svelte-kit/output/client/_app/immutable",
);
const clientRoot = join(projectRoot, ".svelte-kit/output/client");
const manifestPath = join(clientRoot, ".vite/manifest.json");

export const bundleLimits = Object.freeze({
  maximumChunkBytes: 500_000,
  // The startup screen does not need the project navigator, Editor or
  // Inspector. Keep enough release-build headroom without allowing those
  // surfaces to leak back into the static boot graph.
  maximumInitialGraphBytes: 1_350_000,
  maximumInitialGraphGzipBytes: 350_000,
});

export const lazyFeatureGraphDefinitions = Object.freeze([
  Object.freeze({
    feature: "settings",
    entryName: "SettingsWorkspace",
    maximumBytes: 70_000,
    maximumGzipBytes: 21_000,
  }),
  Object.freeze({
    feature: "design-system",
    entryName: "DesignSystemWorkspace",
    maximumBytes: 225_000,
    maximumGzipBytes: 75_000,
  }),
  Object.freeze({
    feature: "version-control",
    entryName: "VersionControlWorkspace",
    // The versioning surface owns the shared accessible SelectControl graph.
    // Keep a narrow release-build margin while still preventing that feature
    // from absorbing unrelated workspace code.
    maximumBytes: 82_000,
    maximumGzipBytes: 23_000,
  }),
  Object.freeze({
    feature: "audit",
    entryName: "AuditWorkspace",
    maximumBytes: 95_000,
    maximumGzipBytes: 30_000,
  }),
]);

// Pană Studio has one SvelteKit route. Its happy-path boot graph consists of the
// client runtime, generated app, root layout and page node. The error node and
// error template are intentionally excluded because a healthy boot does not fetch them.
export const initialGraphEntryNames = Object.freeze([
  "entry/start",
  "entry/app",
  "nodes/0",
  "nodes/2",
]);

function filesBelow(path) {
  return readdirSync(path, { withFileTypes: true }).flatMap((entry) => {
    const target = join(path, entry.name);
    return entry.isDirectory() ? filesBelow(target) : [target];
  });
}

export function manifestKeysForNames(manifest, names) {
  const keysByName = new Map(
    Object.entries(manifest).map(([key, entry]) => [entry.name, key]),
  );
  return names.map((name) => {
    const key = keysByName.get(name);
    if (!key) {
      throw new Error(`[bundle] manifest is missing initial entry ${name}`);
    }
    return key;
  });
}

export function collectStaticGraph(manifest, rootKeys) {
  const visited = new Set();

  function visit(key) {
    if (visited.has(key)) return;
    const entry = manifest[key];
    if (!entry) {
      throw new Error(`[bundle] manifest references missing entry ${key}`);
    }
    if (!entry.file?.endsWith(".js")) {
      throw new Error(`[bundle] initial entry ${key} does not emit JavaScript`);
    }
    visited.add(key);
    for (const dependency of entry.imports ?? []) visit(dependency);
  }

  for (const rootKey of rootKeys) visit(rootKey);
  return [...visited];
}

export function localeCatalogEntries(manifest) {
  const prefix = "src/lib/i18n/generated/catalog.";
  const suffix = ".ts";
  const entries = Object.keys(manifest)
    .filter((key) =>
      key.startsWith(prefix)
      && key.endsWith(suffix)
      && key.length > prefix.length + suffix.length
    )
    .map((key) => ({
      key,
      locale: key.slice(prefix.length, -suffix.length),
    }))
    .sort((left, right) => left.locale.localeCompare(right.locale));
  if (entries.length === 0) {
    throw new Error("[bundle] manifest contains no generated locale catalogs");
  }
  return entries;
}

export function measureManifestGraph(manifest, rootKeys, outputRoot) {
  return measureManifestEntries(
    manifest,
    collectStaticGraph(manifest, rootKeys),
    outputRoot,
  );
}

export function measureManifestEntries(manifest, keys, outputRoot) {
  const entries = keys.map((key) => {
    const path = join(outputRoot, manifest[key].file);
    const source = readFileSync(path);
    return {
      key,
      path,
      bytes: source.byteLength,
      gzipBytes: gzipSync(source).byteLength,
    };
  });
  return {
    entries,
    bytes: entries.reduce((total, entry) => total + entry.bytes, 0),
    gzipBytes: entries.reduce((total, entry) => total + entry.gzipBytes, 0),
  };
}

export function measureLazyFeatureGraphs(
  manifest,
  outputRoot,
  definitions = lazyFeatureGraphDefinitions,
) {
  const initialRoots = manifestKeysForNames(manifest, initialGraphEntryNames);
  const initialKeys = new Set(
    localeCatalogEntries(manifest).flatMap(({ key }) => (
      collectStaticGraph(manifest, [...initialRoots, key])
    )),
  );
  return definitions.map((definition) => {
    const [rootKey] = manifestKeysForNames(manifest, [definition.entryName]);
    const featureKeys = collectStaticGraph(manifest, [rootKey])
      .filter((key) => !initialKeys.has(key));
    if (!featureKeys.includes(rootKey)) {
      throw new Error(
        `[bundle] lazy feature ${definition.feature} is already part of the initial graph`,
      );
    }
    const measurement = measureManifestEntries(manifest, featureKeys, outputRoot);
    return {
      ...definition,
      rootKey,
      ...measurement,
      entries: measurement.entries
        .map((entry) => ({
          key: entry.key,
          file: manifest[entry.key].file,
          bytes: entry.bytes,
          gzipBytes: entry.gzipBytes,
        }))
        .sort((left, right) => right.bytes - left.bytes),
    };
  });
}

export function checkBundleSize(options = {}) {
  const chunks = filesBelow(clientOutput)
    .filter((path) => path.endsWith(".js"))
    .map((path) => ({ path, bytes: statSync(path).size }))
    .sort((left, right) => right.bytes - left.bytes);

  if (chunks.length === 0) {
    throw new Error("[bundle] client build contains no JavaScript chunks");
  }

  const oversized = chunks.filter(
    ({ bytes }) => bytes >= bundleLimits.maximumChunkBytes,
  );
  if (oversized.length > 0) {
    const details = oversized
      .map(({ path, bytes }) => `${relative(projectRoot, path)} (${bytes} bytes)`)
      .join("\n");
    throw new Error(
      `[bundle] JavaScript chunks must stay below ${bundleLimits.maximumChunkBytes} bytes:\n${details}`,
    );
  }

  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  const initialRoots = manifestKeysForNames(manifest, initialGraphEntryNames);
  // The layout awaits one locale import before mounting the page. Measure each
  // supported locale as part of boot even though Rollup correctly emits it as a
  // dynamic chunk, then enforce the budget against the worst supported case.
  const initialGraphs = localeCatalogEntries(manifest).map(({ key, locale }) => ({
    locale,
    ...measureManifestGraph(manifest, [...initialRoots, key], clientRoot),
  }));
  const largestInitialGraph = initialGraphs.reduce((largest, graph) =>
    graph.bytes > largest.bytes ? graph : largest
  );
  const largestInitialGzipGraph = initialGraphs.reduce((largest, graph) =>
    graph.gzipBytes > largest.gzipBytes ? graph : largest
  );
  const graphFailures = [];
  if (largestInitialGraph.bytes >= bundleLimits.maximumInitialGraphBytes) {
    graphFailures.push(
      `${largestInitialGraph.locale}: ${largestInitialGraph.bytes} raw bytes (limit < ${bundleLimits.maximumInitialGraphBytes})`,
    );
  }
  if (largestInitialGzipGraph.gzipBytes >= bundleLimits.maximumInitialGraphGzipBytes) {
    graphFailures.push(
      `${largestInitialGzipGraph.locale}: ${largestInitialGzipGraph.gzipBytes} gzip bytes (limit < ${bundleLimits.maximumInitialGraphGzipBytes})`,
    );
  }
  if (graphFailures.length > 0) {
    throw new Error(
      `[bundle] initial JavaScript graph exceeds its budget:\n${graphFailures.join("\n")}`,
    );
  }

  const featureGraphs = measureLazyFeatureGraphs(manifest, clientRoot);
  const featureFailures = featureGraphs.flatMap((graph) => {
    const failures = [];
    if (graph.bytes >= graph.maximumBytes) {
      failures.push(
        `${graph.feature}: ${graph.bytes} raw bytes (limit < ${graph.maximumBytes})`,
      );
    }
    if (graph.gzipBytes >= graph.maximumGzipBytes) {
      failures.push(
        `${graph.feature}: ${graph.gzipBytes} gzip bytes (limit < ${graph.maximumGzipBytes})`,
      );
    }
    return failures;
  });
  if (featureFailures.length > 0) {
    throw new Error(
      `[bundle] lazy feature graph exceeds its budget:\n${featureFailures.join("\n")}`,
    );
  }

  const largest = chunks[0];
  if (!options.quiet) console.log(
      `[bundle] ${chunks.length} client JS chunks; largest ${relative(projectRoot, largest.path)} (${largest.bytes} bytes)`,
    );
  if (!options.quiet) for (const graph of initialGraphs) {
    console.log(
      `[bundle] initial graph (${graph.locale}) ${graph.entries.length} JS files; ${graph.bytes} raw / ${graph.gzipBytes} gzip bytes`,
    );
  }
  if (!options.quiet) for (const graph of featureGraphs) {
    const largestContribution = graph.entries[0];
    console.log(
      `[bundle] lazy feature (${graph.feature}) ${graph.entries.length} exclusive JS files; ${graph.bytes} raw / ${graph.gzipBytes} gzip bytes; largest ${largestContribution.file} (${largestContribution.bytes} bytes)`,
    );
  }
  return { chunks, initialGraphs, featureGraphs };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) checkBundleSize();
