import { existsSync, readFileSync, readdirSync } from "node:fs";
import { basename, dirname, extname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";
import {
  initialGraphEntryNames,
  localeCatalogEntries,
  manifestKeysForNames,
  measureManifestGraph,
} from "./check-bundle-size.mjs";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const clientRoot = join(projectRoot, ".svelte-kit/output/client");
const manifestPath = join(clientRoot, ".vite/manifest.json");
const baselineStateChunk = Object.freeze({ bytes: 419_681, gzipBytes: 101_731 });

const ownedModules = Object.freeze([
  "src/lib/application/composition.svelte.ts",
  "src/lib/components/application/ApplicationWorkspace.svelte",
  "src/lib/application/workspace-surfaces.ts",
  "src/lib/application/workspace-page-lifecycle.ts",
  "src/lib/application/command-center-service.svelte.ts",
  "src/lib/editor/navigation-service.ts",
  "src/lib/editor/selection-service.ts",
  "src/lib/editor/selection-workspace.svelte.ts",
  "src/lib/project/transition-service.ts",
  "src/lib/project/document-service.ts",
  "src/lib/project/reset-service.ts",
  "src/lib/session/workspace-authority-service.ts",
  "src/lib/versioning/workspace-history-service.svelte.ts",
]);

function filesBelow(path) {
  return readdirSync(path, { withFileTypes: true }).flatMap((entry) => {
    const target = join(path, entry.name);
    return entry.isDirectory() ? filesBelow(target) : [target];
  });
}

function physicalLines(path) {
  const source = readFileSync(path, "utf8");
  return source.length === 0 ? 0 : source.split("\n").length;
}

function sourceMetrics() {
  const sourceFiles = filesBelow(join(projectRoot, "src"))
    .filter((path) => [".ts", ".svelte"].includes(extname(path)));
  const legacyReferences = sourceFiles.flatMap((path) => {
    const source = readFileSync(path, "utf8");
    return /\bAppState\b|\$lib\/state\/app\.svelte|state\/app\.svelte\.ts/.test(source)
      ? [relative(projectRoot, path)]
      : [];
  });
  const modules = Object.fromEntries(ownedModules.map((path) => [
    path,
    physicalLines(join(projectRoot, path)),
  ]));
  return {
    legacyAppStateFileExists: existsSync(join(projectRoot, "src/lib/state/app.svelte.ts")),
    legacyReferences,
    routeLines: physicalLines(join(projectRoot, "src/routes/+page.svelte")),
    modules,
    oversizedOwnedModules: Object.entries(modules)
      .filter(([, lines]) => lines > 1_000)
      .map(([path, lines]) => ({ path, lines })),
  };
}

function bundleMetrics() {
  if (!existsSync(manifestPath)) return { available: false };
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  const initialRoots = manifestKeysForNames(manifest, initialGraphEntryNames);
  const graphs = localeCatalogEntries(manifest).map(({ key, locale }) => ({
    locale,
    ...measureManifestGraph(manifest, [...initialRoots, key], clientRoot),
  }));
  const initialKeys = new Set(graphs.flatMap((graph) => graph.entries.map((entry) => entry.key)));

  function chunk(name) {
    const match = Object.entries(manifest).find(([, entry]) => entry.name === name);
    if (!match) return null;
    const [key, entry] = match;
    const source = readFileSync(join(clientRoot, entry.file));
    return {
      file: entry.file,
      bytes: source.byteLength,
      gzipBytes: gzipSync(source).byteLength,
      inInitialGraph: initialKeys.has(key),
    };
  }

  function entryChunk(key) {
    const expectedName = basename(key, extname(key));
    const entry = manifest[key] ?? Object.values(manifest).find((candidate) => (
      candidate.isDynamicEntry && candidate.name === expectedName
    ));
    if (!entry) return null;
    const source = readFileSync(join(clientRoot, entry.file));
    return {
      file: entry.file,
      bytes: source.byteLength,
      gzipBytes: gzipSync(source).byteLength,
      inInitialGraph: initialKeys.has(key),
    };
  }

  const applicationShell = chunk("pana-application-shell");
  return {
    available: true,
    legacyStateChunk: chunk("pana-state"),
    applicationShell,
    applicationShellReduction: applicationShell
      ? {
          rawPercent: Number((100 * (1 - applicationShell.bytes / baselineStateChunk.bytes)).toFixed(2)),
          gzipPercent: Number((100 * (1 - applicationShell.gzipBytes / baselineStateChunk.gzipBytes)).toFixed(2)),
        }
      : null,
    coreLayers: Object.fromEntries([
      "pana-core-foundation",
      "pana-core-domain",
      "pana-core-runtime",
      "pana-core-orchestration",
    ].map((name) => [name, chunk(name)])),
    lazyWorkspaceSurfaces: Object.fromEntries([
      "src/lib/components/workbench/ActivityRail.svelte",
      "src/lib/components/workspace/WorkspaceCenterArea.svelte",
      "src/lib/components/workspace/WorkspaceInspectorArea.svelte",
      "src/lib/components/workspace/WorkspaceProjectArea.svelte",
    ].map((key) => [key, entryChunk(key)])),
    lazySurfaces: Object.fromEntries([
      "src/lib/components/settings/SettingsWorkspace.svelte",
      "src/lib/components/creation/DesignSystemWorkspace.svelte",
      "src/lib/components/versioning/VersionControlWorkspace.svelte",
      "src/lib/components/audit/AuditWorkspace.svelte",
    ].map((key) => [key, entryChunk(key)])),
    initialGraphs: graphs.map(({ locale, bytes, gzipBytes, entries }) => ({
      locale,
      bytes,
      gzipBytes,
      files: entries.length,
    })),
  };
}

console.log(JSON.stringify({
  schemaVersion: 1,
  measuredAt: new Date().toISOString(),
  baseline: { stateChunk: baselineStateChunk },
  source: sourceMetrics(),
  bundle: bundleMetrics(),
}, null, 2));
