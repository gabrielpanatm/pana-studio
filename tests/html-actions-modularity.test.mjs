import assert from "node:assert/strict";
import { test } from "node:test";

import { analyzeHtmlActionsSources } from "../scripts/analyze-html-actions-modularity.mjs";

function analyze(sources, options = {}) {
  return analyzeHtmlActionsSources({ sources: new Map(sources), ...options });
}

test("guard-ul acceptă fațada unică și ownerul comun al execution lane-ului", () => {
  const report = analyze([
    ["src/lib/editor/html-actions/execution.ts", `
      export const run = (host, operation) => host.structural.run(operation);
    `],
    ["src/lib/editor/html-actions/attributes.ts", `
      export const apply = (host) => run(host, async () => ({ status: "committed" }));
    `],
    ["src/lib/editor/html-editing-service.ts", `
      import { apply } from "$lib/editor/html-actions/attributes";
      export class HtmlEditingService { apply() { return apply(this.actions); } }
    `],
  ]);
  assert.deepEqual(report.violations, []);
});

test("guard-ul respinge căile legacy, host leak-ul, bypass-ul fațadei și modulele mari", () => {
  const report = analyze([
    ["src/lib/state/html-actions-controller.ts", "export const legacy = true;"],
    ["src/lib/editor/html-actions/attributes.ts", `${"\n".repeat(600)}export const value = 1;`],
    ["src/lib/editor/navigation-service.ts", `
      import type { HtmlActionsHost } from "$lib/editor/html-actions/host";
      import { apply } from "$lib/editor/html-actions/attributes";
      html.controllerHost();
    `],
  ]);
  assert.deepEqual(
    new Set(report.violations.map((violation) => violation.code)),
    new Set([
      "legacy-html-actions-path",
      "oversized-html-actions-module",
      "html-actions-facade-bypass",
      "html-actions-host-owner-bypass",
      "html-actions-host-leak",
    ]),
  );
});

test("guard-ul respinge lane-ul secundar, invoke-ul direct, boundary-ul străin și mesajele hardcodate", () => {
  const report = analyze([
    ["src/lib/editor/html-actions/media.ts", `
      import { invoke } from "@tauri-apps/api/core";
      export async function apply(host) {
        await host.structural.projectCommitted();
        await executePreviewForeignIntent();
        return invoke("scrie", { message: "Mutația a eșuat." });
      }
    `],
  ]);
  assert.deepEqual(
    new Set(report.violations.map((violation) => violation.code)),
    new Set([
      "html-actions-execution-owner-bypass",
      "noncanonical-html-actions-boundary",
      "direct-html-actions-invoke",
      "hardcoded-html-actions-message",
    ]),
  );
});
