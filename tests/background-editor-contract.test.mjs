import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  backgroundFromProperties,
  parseCssGradient,
  serializeCssGradient,
  splitTopLevelCssList,
} from "$lib/inspector/background-model";

function source(path) {
  return readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
}

test("gradient editor separates live drag from its final commit and supports the keyboard", () => {
  const editor = source("src/lib/components/inspector/controls/GradientEditor.svelte");

  assert.match(editor, /window\.addEventListener\("pointermove", move\)/);
  assert.match(editor, /if \(commit\)[\s\S]*oncommit\(withRaw\(finalGradient\)\)/);
  assert.match(editor, /event\.key !== "ArrowLeft"/);
  assert.match(editor, /event\.key === "ArrowRight"/);
  assert.match(editor, /event\.key === "Delete"/);
  assert.match(editor, /ColorInput[\s\S]*property="gradient-stop"/);
  assert.match(editor, /role="group"[\s\S]*aria-label=\{t\("inspector-background-gradient-ramp"\)\}/);
  assert.match(editor, /aria-pressed=\{gradient\.repeating\}/);
  assert.match(editor, /inspector-background-gradient-add-hint/);
  assert.match(editor, /inspector-background-gradient-css-source/);
  assert.match(editor, /if \(commit\) onsourcecommit\(source\)/);
  assert.match(editor, /\.gradient-stop\.active \{ z-index: 4; \}/);
});

test("background structural actions use one batched CSS mutation", () => {
  const section = source("src/lib/components/inspector/sections/BackgroundSection.svelte");
  const inspector = source("src/lib/components/InspectorPane.svelte");

  assert.match(section, /serializeBackgroundLonghands\(next\)/);
  assert.match(section, /edit\.draftMany\(properties\)/);
  assert.match(section, /edit\.commitMany\(properties\)/);
  assert.match(inspector, /const nextPendingValues = \{ \.\.\.pendingValues, \.\.\.properties \}/);
  assert.match(inspector, /onLivePropertiesChange\?\.\([\s\S]*nextPendingValues/);
  assert.match(inspector, /for \(const \[property, value\] of entries\)[\s\S]*stageCssRuleMutation/);
});

test("background source synchronization never tracks the local state it replaces", () => {
  const section = source("src/lib/components/inspector/sections/BackgroundSection.svelte");
  const synchronization = section.match(/\$effect\(\(\) => \{[\s\S]*?\n  \}\);/)?.[0] ?? "";

  assert.match(section, /import \{ untrack \} from "svelte"/);
  assert.match(synchronization, /const previousActiveLayerId = untrack\(\(\) => activeLayerId\)/);
  assert.match(synchronization, /background = nextBackground/);
  assert.doesNotMatch(synchronization, /background\.layers/);
});

test("CSS authority releases the inspector queue before canonical preview settlement", () => {
  const app = source("src/lib/state/app.svelte.ts");
  const settlement = source("src/lib/session/workspace-mutation-coordinator.ts");

  assert.match(
    app,
    /this\.projectWorkspaceSnapshot = currentWorkspace;[\s\S]*void this\.settleCommittedInspectorCssProjection/,
  );
  assert.match(
    app,
    /settleCommittedInspectorCssProjection[\s\S]*const topologyChanged[\s\S]*refreshSourceGraph: topologyChanged,[\s\S]*refreshScss: topologyChanged/,
  );
  assert.match(
    settlement,
    /alreadyPublishedRevision > workspaceRevision[\s\S]*preview: "superseded"/,
  );
  assert.match(
    app,
    /projectWorkspaceSnapshot\?\.revision !== minimumWorkspaceRevision[\s\S]*return false/,
  );
  assert.match(
    app,
    /expectedWorkspaceRevision: minimumWorkspaceRevision/,
  );
  assert.match(
    app,
    /projectWorkspaceSnapshot\?\.revision !== identity\.workspaceRevision[\s\S]*return/,
  );
});

test("large gradients and rapid parse/serialize cycles remain bounded", () => {
  const stops = Array.from({ length: 160 }, (_, index) => (
    `oklch(70% 0.2 ${index * 2}) ${(index / 159) * 100}%`
  ));
  const value = `linear-gradient(90deg, ${stops.join(", ")})`;
  const started = performance.now();
  let serialized = value;
  for (let iteration = 0; iteration < 250; iteration += 1) {
    const parsed = parseCssGradient(serialized);
    assert.ok(parsed);
    assert.equal(parsed.items.length, 160);
    serialized = serializeCssGradient(parsed);
  }
  assert.ok(performance.now() - started < 2_000);
});

test("opaque and commented values are preserved without false layer splits", () => {
  assert.equal(parseCssGradient("linear-gradient(red, blue"), null);
  assert.deepEqual(
    splitTopLevelCssList("url('/a,b.png') /* x, y */, linear-gradient(red, blue)"),
    ["url('/a,b.png') /* x, y */", "linear-gradient(red, blue)"],
  );
  const model = backgroundFromProperties({
    "background-image": "url('/a.png'), url('/b.png')",
    "background-size": "var(--dimensiuni-fundal)",
  });
  assert.equal(model.opaqueProperties["background-size"], "var(--dimensiuni-fundal)");
  assert.equal(model.structurallyEditable, false);
});
