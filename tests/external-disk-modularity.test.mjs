import assert from "node:assert/strict";
import { test } from "node:test";

import { analyzeExternalDiskSources } from "../scripts/analyze-external-disk-modularity.mjs";

function analyze(sources, options = {}) {
  return analyzeExternalDiskSources({ sources: new Map(sources), ...options });
}

test("guard-ul acceptă fațada unică, runtime-ul local și ownerii canonici", () => {
  const report = analyze([
    ["src/lib/session/external-disk/monitor.ts", `
      import { startProjectDiskWatch, stopProjectDiskWatch } from "$lib/project/io/external-disk";
      import { subscribeProjectDiskChanges } from "$lib/kernel/project-disk-events";
    `],
    ["src/lib/session/external-disk/reconcile.ts", `
      import { readCurrentProjectDiskManifest, reconcileCleanExternalProjectFiles }
        from "$lib/project/io/external-disk";
    `],
    ["src/lib/session/external-disk-state.svelte.ts", `
      export class ExternalDiskState { reconcileGeneration = 0; }
    `],
  ]);
  assert.deepEqual(report.violations, []);
});

test("guard-ul respinge legacy, bypass-ul fațadei, globalul și mutarea epoch-ului", () => {
  const report = analyze([
    ["src/lib/state/external-disk-controller.ts", "export const legacy = true;"],
    ["src/lib/application/composition.svelte.ts", `
      import type { ExternalDiskRuntime } from "$lib/session/external-disk/contracts";
      import { runExternalDiskCheck } from "$lib/session/external-disk/reconcile";
    `],
    ["src/lib/session/external-disk/state.ts", `
      let externalReconcileGeneration = 0;
      host.session.epoch += 1;
    `],
  ]);
  assert.deepEqual(
    new Set(report.violations.map((violation) => violation.code)),
    new Set([
      "legacy-external-disk-path",
      "external-disk-facade-bypass",
      "external-disk-runtime-owner-bypass",
      "global-external-reconcile-generation",
      "external-disk-project-epoch-mutation",
    ]),
  );
});

test("guard-ul respinge ownerul greșit, invoke-ul direct, polling-ul și modulele mari", () => {
  const report = analyze([
    ["src/lib/session/external-disk/state.ts", `
      import { invoke } from "@tauri-apps/api/core";
      ${"\n".repeat(600)}
      setInterval(() => invoke("readCurrentProjectDiskManifest"), 5_000);
    `],
  ]);
  assert.deepEqual(
    new Set(report.violations.map((violation) => violation.code)),
    new Set([
      "oversized-external-disk-module",
      "external-disk-boundary-owner-bypass",
      "direct-external-disk-invoke",
      "external-disk-polling-loop",
    ]),
  );
});
