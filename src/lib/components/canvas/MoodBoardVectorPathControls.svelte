<script lang="ts">
  import {
    IconCornerDownRight,
    IconDownload,
    IconVectorBezier,
  } from "@tabler/icons-svelte";
  import {
    moodBoardVectorWithCornerNodes,
    moodBoardVectorWithFill,
    moodBoardVectorWithInsertedNode,
    moodBoardVectorWithRemovedNode,
    moodBoardVectorWithSmoothNodes,
    moodBoardVectorWithStroke,
    moodBoardVectorWithStrokeWidth,
    moodBoardVectorWithToggledClosed,
  } from "$lib/mood-board/context-actions";
  import { cloneMoodBoardItem } from "$lib/mood-board/item-view";
  import type {
    MoodBoardItem,
    MoodBoardVectorHandleMode,
    MoodBoardVectorNodeEditState,
    MoodBoardVectorPathItem,
  } from "$lib/mood-board/model";

  export let vectorItem: MoodBoardVectorPathItem;
  export let vectorPathEditActive = false;
  export let activeVectorNodeMode: MoodBoardVectorHandleMode | "mixed" | null = null;
  export let vectorNodeEditState: MoodBoardVectorNodeEditState | null = null;
  export let previewItem: (item: MoodBoardItem) => void;
  export let commitItemEdit: (beforeItem: MoodBoardItem, nextItem: MoodBoardItem) => void;
  export let setSelectedVectorNodeHandleMode: (mode: MoodBoardVectorHandleMode) => void = () => undefined;
  export let enterVectorEditMode: (itemId: string, svgElementId?: string | null) => void = () => undefined;
  export let exitVectorEditMode: () => void = () => undefined;
  export let exportVectorPath: (itemId: string) => void | Promise<void> = () => undefined;

  let editBeforeItem: MoodBoardVectorPathItem | null = null;

  function cloneItem(value: MoodBoardVectorPathItem): MoodBoardVectorPathItem {
    return cloneMoodBoardItem(value);
  }

  function beginEdit() {
    editBeforeItem ??= cloneItem(vectorItem);
  }

  function commitEdit(nextItem: MoodBoardVectorPathItem) {
    if (!editBeforeItem) return;
    const before = editBeforeItem;
    editBeforeItem = null;
    commitItemEdit(before, nextItem);
  }

  function updateVectorFill(value: string) {
    beginEdit();
    previewItem(moodBoardVectorWithFill(vectorItem, value));
  }

  function commitVectorFill(value: string) {
    commitEdit(moodBoardVectorWithFill(vectorItem, value));
  }

  function updateVectorStroke(value: string) {
    beginEdit();
    previewItem(moodBoardVectorWithStroke(vectorItem, value));
  }

  function commitVectorStroke(value: string) {
    commitEdit(moodBoardVectorWithStroke(vectorItem, value));
  }

  function updateVectorStrokeWidth(value: string) {
    beginEdit();
    previewItem(moodBoardVectorWithStrokeWidth(vectorItem, value));
  }

  function commitVectorStrokeWidth(value: string) {
    commitEdit(moodBoardVectorWithStrokeWidth(vectorItem, value));
  }

  function toggleVectorClosed() {
    commitItemEdit(cloneItem(vectorItem), moodBoardVectorWithToggledClosed(vectorItem));
  }

  function cornerVectorPath() {
    commitItemEdit(cloneItem(vectorItem), moodBoardVectorWithCornerNodes(vectorItem));
  }

  function smoothVectorPath() {
    commitItemEdit(cloneItem(vectorItem), moodBoardVectorWithSmoothNodes(vectorItem));
  }

  function insertVectorNode() {
    commitItemEdit(cloneItem(vectorItem), moodBoardVectorWithInsertedNode(vectorItem));
  }

  function removeVectorNode() {
    commitItemEdit(cloneItem(vectorItem), moodBoardVectorWithRemovedNode(vectorItem));
  }

  function toggleVectorPathEditMode() {
    if (vectorPathEditActive) exitVectorEditMode();
    else enterVectorEditMode(vectorItem.id, null);
  }

  function exportVector() {
    void exportVectorPath(vectorItem.id);
  }
</script>

<span class="separator"></span>
<label class="color-control" style={`--control-color:${vectorItem.fill};`} title="Fill path">
  <input
    type="color"
    value={vectorItem.fill}
    aria-label="Fill path"
    onfocus={beginEdit}
    oninput={(event) => updateVectorFill(event.currentTarget.value)}
    onchange={(event) => commitVectorFill(event.currentTarget.value)}
    onblur={(event) => commitVectorFill(event.currentTarget.value)}
  />
</label>
<label class="color-control" style={`--control-color:${vectorItem.stroke};`} title="Stroke path">
  <input
    type="color"
    value={vectorItem.stroke}
    aria-label="Stroke path"
    onfocus={beginEdit}
    oninput={(event) => updateVectorStroke(event.currentTarget.value)}
    onchange={(event) => commitVectorStroke(event.currentTarget.value)}
    onblur={(event) => commitVectorStroke(event.currentTarget.value)}
  />
</label>
<input
  class="number-control compact-number"
  type="number"
  min="0"
  max="32"
  step="1"
  value={vectorItem.strokeWidth}
  aria-label="Grosime contur path"
  onfocus={beginEdit}
  oninput={(event) => updateVectorStrokeWidth(event.currentTarget.value)}
  onblur={(event) => commitVectorStrokeWidth(event.currentTarget.value)}
/>
<button type="button" class:active={vectorPathEditActive} title={vectorPathEditActive ? "Ieși din editarea nodurilor" : "Editează nodurile path-ului"} onclick={toggleVectorPathEditMode}>
  <IconVectorBezier size={15} stroke={2} />
</button>
{#if vectorPathEditActive}
  <span class="parent-chip muted-chip" title={vectorNodeEditState ? `${vectorNodeEditState.indexes.length} noduri selectate din ${vectorNodeEditState.nodeCount}` : "Selectează noduri pe path"}>
    {vectorNodeEditState ? `${vectorNodeEditState.indexes.length} nod` : "noduri"}
  </span>
  <button type="button" class:active={vectorItem.closed} title="Închide/deschide path" onclick={toggleVectorClosed}>Z</button>
  <button
    type="button"
    class:active={activeVectorNodeMode === "corner"}
    disabled={!vectorNodeEditState?.indexes.length}
    title="Nod corner"
    onclick={() => setSelectedVectorNodeHandleMode("corner")}
  >C</button>
  <button
    type="button"
    class:active={activeVectorNodeMode === "independent"}
    disabled={!vectorNodeEditState?.indexes.length}
    title="Handle-uri independente"
    onclick={() => setSelectedVectorNodeHandleMode("independent")}
  >I</button>
  <button
    type="button"
    class:active={activeVectorNodeMode === "mirrored"}
    disabled={!vectorNodeEditState?.indexes.length}
    title="Handle-uri mirrored"
    onclick={() => setSelectedVectorNodeHandleMode("mirrored")}
  >M</button>
  <button
    type="button"
    class:active={activeVectorNodeMode === "locked"}
    disabled={!vectorNodeEditState?.indexes.length}
    title="Handle-uri locked"
    onclick={() => setSelectedVectorNodeHandleMode("locked")}
  >L</button>
  <button type="button" title="Transformă nodurile în colțuri" onclick={cornerVectorPath}>
    <IconCornerDownRight size={15} stroke={2} />
  </button>
  <button type="button" title="Netezește path-ul" onclick={smoothVectorPath}>S</button>
  <button type="button" title="Adaugă nod pe cel mai lung segment" onclick={insertVectorNode}>+N</button>
  <button type="button" title="Șterge ultimul nod" onclick={removeVectorNode}>-N</button>
{/if}
<button type="button" title="Exportă SVG în resurse/imagini" onclick={exportVector}>
  <IconDownload size={15} stroke={2} />
</button>
