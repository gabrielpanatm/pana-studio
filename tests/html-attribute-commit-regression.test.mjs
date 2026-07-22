import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const appState = readFileSync(
  new URL("../src/lib/state/app.svelte.ts", import.meta.url),
  "utf8",
);
const htmlActions = readFileSync(
  new URL("../src/lib/state/html-actions-controller.ts", import.meta.url),
  "utf8",
);
const rustExecutor = readFileSync(
  new URL("../src-tauri/src/kernel/preview_projection/executor/html.rs", import.meta.url),
  "utf8",
);

test("HTML attribute completion is single-flight across inspector and Save flush", () => {
  assert.match(
    appState,
    /finishPromise:\s*Promise<EditorActionOutcome \| null> \| null/,
  );
  assert.match(
    appState,
    /if \(session\.finishPromise\) return await session\.finishPromise;[\s\S]*session\.finishPromise = operation/,
  );
  assert.match(
    appState,
    /while \(this\.activeHtmlAttributeEditSession\?\.id === session\.id\)/,
  );
  assert.doesNotMatch(
    appState,
    /return await this\.finishActiveHtmlAttributeEditSession\(\)/,
  );
});

test("an identical attribute intent settles as noop before canonical projection", () => {
  const noopGuard = htmlActions.indexOf("!receipt.workspaceMutation.changed");
  const canonicalProjection = htmlActions.indexOf(
    "await projectCommittedPreviewStructuralMutation",
    noopGuard,
  );
  assert.ok(noopGuard >= 0, "lipsește gardul workspace no-op");
  assert.ok(
    canonicalProjection > noopGuard,
    "no-op-ul trebuie interceptat înaintea proiecției canonice",
  );
  assert.match(htmlActions, /return noopAction\("Atributele coincid deja cu sesiunea proiectului\."\)/);
});

test("Rust does not issue CanvasPatch for no-op or source-only attributes", () => {
  assert.match(
    rustExecutor,
    /html_attribute_canvas_patch_allowed\(\s*commit\.workspace_mutation\.changed,\s*&patch\.attributes/,
  );
  assert.match(
    rustExecutor,
    /workspace_changed[\s\S]*\.all\(\|name\| is_live_projectable_attribute\(name\)\)/,
  );
  assert.match(rustExecutor, /let canvas_patch = if[\s\S]*\} else \{\s*None\s*\};/);
});
