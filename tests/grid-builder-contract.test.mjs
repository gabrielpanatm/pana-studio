import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  cloneGridTrack,
  createGridTrack,
  gridFromProperties,
  gridToProperties,
  parseGridAreasText,
  parseGridTrackList,
  serializeGridAreasRows,
  serializeGridTrackList,
  validateGridAreasRows,
} from "$lib/inspector/grid-model";
import { cssRuleContextFromSource } from "$lib/css/source-sync";

function source(path) {
  return readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
}

test("advanced grid tracks round-trip without expanding repeat or line names", () => {
  const value = "[start] minmax(0, 1fr) repeat(auto-fit, minmax(14rem, 1fr)) [end]";
  const list = parseGridTrackList(value);
  assert.equal(list.mode, "tracks");
  assert.equal(list.tracks.length, 4);
  assert.equal(list.tracks[1].kind, "minmax");
  assert.equal(list.tracks[2].kind, "repeat");
  assert.equal(list.tracks[2].repeatCount, "auto-fit");
  assert.equal(serializeGridTrackList(list), value);
});

test("dynamic, subgrid and masonry values stay preserved and non-structural", () => {
  for (const [value, mode] of [
    ["$coloane-proiect", "tracks"],
    ["subgrid [inceput] [sfarsit]", "subgrid"],
    ["masonry", "masonry"],
  ]) {
    const list = parseGridTrackList(value);
    assert.equal(list.mode, mode);
    assert.equal(list.structurallyEditable, false);
    assert.equal(serializeGridTrackList(list), value);
  }
});

test("track creation and duplication produce stable independent editor identities", () => {
  const repeat = createGridTrack("repeat", "columns");
  const reactiveLikeTrack = new Proxy(repeat, {});
  assert.throws(() => structuredClone(reactiveLikeTrack), /clone/i);
  const duplicate = cloneGridTrack(reactiveLikeTrack);
  assert.equal(repeat.raw, "repeat(2, minmax(0, 1fr))");
  assert.notEqual(duplicate.id, repeat.id);
  assert.notEqual(duplicate.repeatTracks[0].id, repeat.repeatTracks[0].id);
  assert.equal(duplicate.repeatCount, "2");
});

test("grid areas use one row per editor line and serialize as CSS strings", () => {
  const rows = parseGridAreasText("hero hero side\nmain main side");
  assert.deepEqual(rows, [
    ["hero", "hero", "side"],
    ["main", "main", "side"],
  ]);
  assert.equal(serializeGridAreasRows(rows), '"hero hero side" "main main side"');
  assert.equal(validateGridAreasRows(rows), null);
  assert.equal(validateGridAreasRows([["a", "a"], ["a", "."]]), "contiguous");
  assert.equal(validateGridAreasRows([["a", "a"], ["b"]]), "rectangular");
});

test("grid model reads gap shorthand and preserves all authored longhands", () => {
  const model = gridFromProperties({
    display: "grid",
    "grid-template-columns": "repeat(3, 1fr)",
    "grid-template-rows": "auto minmax(0, 1fr)",
    "grid-template-areas": '"a b c" "d e f"',
    gap: "$space-m 2rem",
    "grid-auto-flow": "row dense",
    "grid-column": "1 / span 2",
  });
  assert.equal(model.rowGap, "$space-m");
  assert.equal(model.columnGap, "2rem");
  assert.equal(model.templateAreas.rows.length, 2);
  const serialized = gridToProperties(model);
  assert.equal(serialized["grid-template-columns"], "repeat(3, 1fr)");
  assert.equal(serialized["grid-column"], "1 / span 2");
});

test("open-source grid projection cascades a partial viewport rule", () => {
  const context = cssRuleContextFromSource(`
.atelier {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 2rem;
}
@media (max-width: $bp-mobil) {
  .atelier { grid-template-columns: 1fr; }
}
`, "sass/pagini/index.scss", ".atelier", "mobile");
  assert.equal(context.grid.display, "grid");
  assert.equal(serializeGridTrackList(context.grid.templateColumns), "1fr");
  assert.equal(context.grid.rowGap, "2rem");
  assert.equal(context.hasViewportRule, true);
});

test("builder commits complete gestures while preview overlay remains display-only", () => {
  const builder = source("src/lib/components/inspector/controls/GridBuilder.svelte");
  const bridge = source("src-tauri/src/preview/bridge/03_canvas_agent.js");
  const messages = source("src-tauri/src/preview/bridge/12_messages_events.js");

  assert.match(builder, /edit\.draftMany\(properties\)/);
  assert.match(builder, /edit\.commitMany\(properties\)/);
  assert.match(
    builder,
    /if \(fingerprint === locallyEmittedFingerprint\) \{[\s\S]*?locallyEmittedFingerprint = "";[\s\S]*?return;[\s\S]*?\}[\s\S]*?locallyEmittedFingerprint = "";[\s\S]*?const next = inputGrid\(\)/,
    "the local acknowledgement fingerprint is one-shot, so Undo/Redo can project the same value later",
  );
  assert.match(builder, /\(track\.id\)/, "track inputs keep stable keyed identities while typing");
  assert.match(builder, /event\.key === "Escape"[\s\S]*edit\.cancel\("grid-template-areas"\)/);
  assert.match(builder, /GRID_OPAQUE_PROPERTIES/);
  const layout = source("src/lib/components/inspector/sections/LayoutSection.svelte");
  assert.match(layout, /canonicalGrid\?\.display/);
  assert.match(layout, /updatePlacementSpan/);
  assert.match(layout, /inspector-grid-column-span/);
  assert.match(builder, /addTrack\("columns"\)/);
  assert.match(builder, /duplicateTrack\("columns"/);
  assert.match(builder, /moveTrack\("rows"/);
  assert.match(builder, /serializeGridAreasRows/);
  assert.match(builder, /gridOverlayEnabled|overlayEnabled/);
  assert.match(bridge, /CANVAS_AGENT_GRID_ID/);
  assert.match(bridge, /data-pana-canvas-agent-overlay", "grid"/);
  assert.match(bridge, /"pointer-events: none"/);
  assert.doesNotMatch(bridge, /CANVAS_AGENT_GRID_ID[\s\S]{0,500}addEventListener\("(?:pointer|mouse|click)/);
  assert.match(messages, /data\.type === "set-canvas-grid-overlay"/);
  assert.match(messages, /set-live-style-css[\s\S]*updateCanvasAgentGridOverlay/);
});
