<script lang="ts">
  import { IconPencil } from "@tabler/icons-svelte";
  import {
    moodBoardShapeWithFill,
    moodBoardShapeWithKind,
    moodBoardShapeWithStroke,
    moodBoardShapeWithStrokeWidth,
  } from "$lib/mood-board/context-actions";
  import { cloneMoodBoardItem } from "$lib/mood-board/item-view";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import type { MoodBoardItem, MoodBoardShapeItem } from "$lib/mood-board/model";

  export let shapeItem: MoodBoardShapeItem;
  export let previewItem: (item: MoodBoardItem) => void;
  export let commitItemEdit: (beforeItem: MoodBoardItem, nextItem: MoodBoardItem) => void;
  export let convertShapeToPath: (itemId: string) => void = () => undefined;

  let editBeforeItem: MoodBoardShapeItem | null = null;
  const shapeOptions = [
    { value: "rectangle", label: "Pătrat" },
    { value: "ellipse", label: "Cerc" },
    { value: "diamond", label: "Diamond" },
  ];

  function cloneItem(value: MoodBoardShapeItem): MoodBoardShapeItem {
    return cloneMoodBoardItem(value);
  }

  function beginEdit() {
    editBeforeItem ??= cloneItem(shapeItem);
  }

  function commitEdit(nextItem: MoodBoardShapeItem) {
    if (!editBeforeItem) return;
    const before = editBeforeItem;
    editBeforeItem = null;
    commitItemEdit(before, nextItem);
  }

  function updateShapeKind(value: string) {
    const nextItem = moodBoardShapeWithKind(shapeItem, value);
    if (nextItem) commitItemEdit(cloneItem(shapeItem), nextItem);
  }

  function updateShapeFill(value: string) {
    beginEdit();
    previewItem(moodBoardShapeWithFill(shapeItem, value));
  }

  function commitShapeFill(value: string) {
    commitEdit(moodBoardShapeWithFill(shapeItem, value));
  }

  function updateShapeStroke(value: string) {
    beginEdit();
    previewItem(moodBoardShapeWithStroke(shapeItem, value));
  }

  function commitShapeStroke(value: string) {
    commitEdit(moodBoardShapeWithStroke(shapeItem, value));
  }

  function updateShapeStrokeWidth(value: string) {
    beginEdit();
    previewItem(moodBoardShapeWithStrokeWidth(shapeItem, value));
  }

  function commitShapeStrokeWidth(value: string) {
    commitEdit(moodBoardShapeWithStrokeWidth(shapeItem, value));
  }
</script>

<span class="separator"></span>
<div class="shape-select">
  <SelectControl value={shapeItem.shape} options={shapeOptions} ariaLabel="Tip formă" onchange={updateShapeKind} />
</div>
<label class="color-control" style={`--control-color:${shapeItem.fill};`} title="Fill formă">
  <input
    type="color"
    value={shapeItem.fill}
    aria-label="Fill formă"
    onfocus={beginEdit}
    oninput={(event) => updateShapeFill(event.currentTarget.value)}
    onchange={(event) => commitShapeFill(event.currentTarget.value)}
    onblur={(event) => commitShapeFill(event.currentTarget.value)}
  />
</label>
<label class="color-control" style={`--control-color:${shapeItem.stroke};`} title="Stroke formă">
  <input
    type="color"
    value={shapeItem.stroke}
    aria-label="Stroke formă"
    onfocus={beginEdit}
    oninput={(event) => updateShapeStroke(event.currentTarget.value)}
    onchange={(event) => commitShapeStroke(event.currentTarget.value)}
    onblur={(event) => commitShapeStroke(event.currentTarget.value)}
  />
</label>
<input
  class="number-control compact-number"
  type="number"
  min="0"
  max="24"
  step="1"
  value={shapeItem.strokeWidth}
  aria-label="Grosime contur formă"
  onfocus={beginEdit}
  oninput={(event) => updateShapeStrokeWidth(event.currentTarget.value)}
  onblur={(event) => commitShapeStrokeWidth(event.currentTarget.value)}
/>
<button type="button" title="Convertește forma în path editabil" onclick={() => convertShapeToPath(shapeItem.id)}>
  <IconPencil size={15} stroke={2} />
</button>

<style>
  .shape-select {
    width: 98px;
    min-width: 0;
  }
</style>
