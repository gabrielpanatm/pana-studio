import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { createSourceLayerChunkMap } from "../scripts/source-layer-chunks.mjs";

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const legacyIo = path.join(projectRoot, "src", "lib", "project", "io.ts");

const domainIoModules = [
  "src/lib/audit/io.ts",
  "src/lib/blocks/io.ts",
  "src/lib/canvas/interaction-io.ts",
  "src/lib/content-models/io.ts",
  "src/lib/content/io.ts",
  "src/lib/creation/components-io.ts",
  "src/lib/creation/icon-io.ts",
  "src/lib/data/io.ts",
  "src/lib/editor/dynamic-widget-io.ts",
  "src/lib/editor/navigation-io.ts",
  "src/lib/editor/selection-io.ts",
  "src/lib/fonts/io.ts",
  "src/lib/kernel/recovery-io.ts",
  "src/lib/page-assets/io.ts",
  "src/lib/preview/io.ts",
  "src/lib/preview/structural-io.ts",
  "src/lib/project/io/configuration.ts",
  "src/lib/project/io/external-disk.ts",
  "src/lib/project/io/lifecycle.ts",
  "src/lib/project/io/startup.ts",
  "src/lib/project/io/workspace.ts",
  "src/lib/project/io/zola.ts",
  "src/lib/source-graph/io.ts",
  "src/lib/status/io.ts",
  "src/lib/taxonomies/io.ts",
  "src/lib/templates/io.ts",
];

function filesBelow(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const target = path.join(directory, entry.name);
    return entry.isDirectory() ? filesBelow(target) : [target];
  });
}

test("project IO has domain owners and no legacy compatibility path", () => {
  assert.equal(existsSync(legacyIo), false);
  for (const relativePath of domainIoModules) {
    const absolutePath = path.join(projectRoot, relativePath);
    assert.equal(existsSync(absolutePath), true, `${relativePath} must exist`);
    const source = readFileSync(absolutePath, "utf8");
    assert.doesNotMatch(source, /export\s+(?:type\s+)?(?:\*|\{[\s\S]*?\})\s+from/);
    assert.ok(
      source.split("\n").length <= 300,
      `${relativePath} must stay a focused IO module`,
    );
  }

  const productionSources = filesBelow(path.join(projectRoot, "src"))
    .filter((file) => /\.(?:ts|svelte|js)$/.test(file));
  for (const file of productionSources) {
    assert.doesNotMatch(
      readFileSync(file, "utf8"),
      /from\s+["']\$lib\/project\/io["']/,
      path.relative(projectRoot, file),
    );
  }
});

test("lazy domain IO stays outside the static application entry graph", () => {
  const initialGraph = createSourceLayerChunkMap({
    projectRoot,
    entry: "src/routes/+page.svelte",
    chunkNames: ["initial"],
  });
  for (const relativePath of [
    "src/lib/content-models/io.ts",
    "src/lib/creation/components-io.ts",
    "src/lib/data/io.ts",
    "src/lib/fonts/io.ts",
    "src/lib/taxonomies/io.ts",
    "src/lib/templates/io.ts",
  ]) {
    const absolutePath = path.join(projectRoot, relativePath).replaceAll("\\", "/");
    assert.equal(initialGraph.has(absolutePath), false, `${relativePath} leaked into boot`);
  }
});

test("Tauri commands have one frontend IO owner and dead wrappers stay removed", () => {
  const productionSources = filesBelow(path.join(projectRoot, "src", "lib"))
    .filter((file) => /\.(?:ts|svelte|js)$/.test(file));
  const ownersByCommand = new Map();
  const sourceCorpus = [];
  for (const file of productionSources) {
    const source = readFileSync(file, "utf8");
    sourceCorpus.push(source);
    for (const match of source.matchAll(
      /(?:invoke|invokeWorkspaceEntryMutation|invokeBoundFileBuffer)\s*(?:<[^;]*?>)?\s*\(\s*"([a-z0-9_]+)"/g,
    )) {
      const owners = ownersByCommand.get(match[1]) ?? new Set();
      owners.add(path.relative(projectRoot, file));
      ownersByCommand.set(match[1], owners);
    }
  }
  for (const [command, owners] of ownersByCommand) {
    assert.ok(
      owners.size === 1,
      `${command} has parallel frontend owners: ${[...owners].join(", ")}`,
    );
  }

  const frontend = sourceCorpus.join("\n");
  for (const deadExport of [
    "acknowledgeCanvasProjectionPhase",
    "applyNativeBlockContract",
    "createTemplate",
    "createTemplateCollection",
    "planNativeBlockContract",
    "planPageAssetContract",
    "readBlockRuntimeSnapshot",
    "readKernelProjectTransitionDecisionRetentionHotJournals",
    "readProjectModel",
    "readZolaBaseUrl",
    "readZolaProjectSettings",
    "resolveTemplateWorkbenchPlan",
    "saveZolaBaseUrl",
    "saveZolaProjectSettings",
  ]) {
    assert.doesNotMatch(
      frontend,
      new RegExp(`export\\s+(?:async\\s+)?function\\s+${deadExport}\\b`),
    );
  }
});
