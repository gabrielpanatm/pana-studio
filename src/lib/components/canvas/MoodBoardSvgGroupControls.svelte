<script lang="ts">
  import {
    IconArrowBarDown,
    IconArrowBarUp,
    IconBold,
    IconCopy,
    IconCornerDownRight,
    IconDownload,
    IconPencil,
    IconStack2,
    IconTrash,
    IconUnlink,
    IconVectorBezier,
  } from "@tabler/icons-svelte";
  import {
    moodBoardSvgGroupWithCornerSelectedPath,
    moodBoardSvgGroupWithDuplicatedElement,
    moodBoardSvgGroupWithDuplicatedGroup,
    moodBoardSvgGroupWithFill,
    moodBoardSvgGroupWithInsertedSelectedPathNode,
    moodBoardSvgGroupWithMovedElement,
    moodBoardSvgGroupWithMovedGroup,
    moodBoardSvgGroupWithOpacity,
    moodBoardSvgGroupWithRemovedSelectedPathNode,
    moodBoardSvgGroupWithResetElementTransform,
    moodBoardSvgGroupWithRotatedElement,
    moodBoardSvgGroupWithScaledElement,
    moodBoardSvgGroupWithSmoothSelectedPath,
    moodBoardSvgGroupWithStroke,
    moodBoardSvgGroupWithStrokeWidth,
    moodBoardSvgGroupWithText,
    moodBoardSvgGroupWithTextFontSize,
    moodBoardSvgGroupWithToggledSelectedPathClosed,
    moodBoardSvgGroupWithToggledTextWeight,
    moodBoardSvgGroupWithoutElement,
    moodBoardSvgGroupWithoutGroup,
  } from "$lib/mood-board/context-actions";
  import { moodBoardColorInputValue } from "$lib/mood-board/control-values";
  import { cloneMoodBoardItem } from "$lib/mood-board/item-view";
  import type {
    MoodBoardItem,
    MoodBoardVectorGroupItem,
    MoodBoardVectorHandleMode,
    MoodBoardVectorNodeEditState,
  } from "$lib/mood-board/model";
  import { canExtractSvgSubPath, editableSvgElementNodes } from "$lib/mood-board/svg";
  import {
    selectedSvgGroupPath,
    svgGroupElements,
  } from "$lib/mood-board/svg-groups";

  export let svgGroupItem: MoodBoardVectorGroupItem;
  export let selectedSvgElementId: string | null = null;
  export let vectorEditTargetItemId: string | null = null;
  export let vectorEditTargetSvgElementId: string | null = null;
  export let vectorNodeEditState: MoodBoardVectorNodeEditState | null = null;
  export let previewItem: (item: MoodBoardItem) => void;
  export let commitItemEdit: (beforeItem: MoodBoardItem, nextItem: MoodBoardItem) => void;
  export let setSelectedVectorNodeHandleMode: (mode: MoodBoardVectorHandleMode) => void = () => undefined;
  export let enterVectorEditMode: (itemId: string, svgElementId?: string | null) => void = () => undefined;
  export let exitVectorEditMode: () => void = () => undefined;
  export let exportVectorPath: (itemId: string) => void | Promise<void> = () => undefined;
  export let extractSvgSubPath: (itemId: string, elementId: string) => void = () => undefined;
  export let extractAllSvgSubPaths: (itemId: string) => void = () => undefined;
  export let ungroupVectorGroup: (itemId: string) => void = () => undefined;

  let editBeforeItem: MoodBoardVectorGroupItem | null = null;
  let activeVectorNodeMode: MoodBoardVectorHandleMode | "mixed" | null = null;

  $: selectedSvgElement = svgGroupItem.elements.find((element) => element.id === selectedSvgElementId) ?? null;
  $: activeSvgGroupPath = selectedSvgGroupPath(selectedSvgElement);
  $: activeSvgGroupElements = activeSvgGroupPath ? svgGroupElements(svgGroupItem.elements, activeSvgGroupPath) : [];
  $: activeSvgGroupLabel = activeSvgGroupPath?.at(-1) ?? "";
  $: selectedSvgTextElement = selectedSvgElement?.type === "text" ? selectedSvgElement : null;
  $: selectedSvgPathData = selectedSvgElement?.type === "path" ? editableSvgElementNodes(selectedSvgElement) : null;
  $: selectedSvgPathEditActive = Boolean(
    selectedSvgElement?.type === "path"
    && vectorEditTargetItemId === svgGroupItem.id
    && vectorEditTargetSvgElementId === selectedSvgElement.id,
  );
  $: activeVectorNodeMode = vectorNodeEditState && vectorNodeEditState.modes.length > 0
    ? vectorNodeEditState.modes.every((mode) => mode === vectorNodeEditState?.modes[0])
      ? vectorNodeEditState.modes[0]
      : "mixed"
    : null;
  $: firstSvgPathElement = svgGroupItem.elements.find((element) => element.type === "path") ?? null;
  $: extractableSvgCount = svgGroupItem.elements.filter(canExtractSvgSubPath).length;
  $: selectedSvgElementExtractable = selectedSvgElement ? canExtractSvgSubPath(selectedSvgElement) : false;
  $: svgExtractTitle = selectedSvgElement
    ? selectedSvgElementExtractable
      ? "Extrage sub-path ca path editabil"
      : "Sub-path-ul selectat nu poate fi extras ca Bezier editabil"
    : "Selectează un sub-path din SVG pentru extragere individuală";

  function cloneItem(value: MoodBoardVectorGroupItem): MoodBoardVectorGroupItem {
    return cloneMoodBoardItem(value);
  }

  function beginEdit() {
    editBeforeItem ??= cloneItem(svgGroupItem);
  }

  function commitEdit(nextItem: MoodBoardVectorGroupItem) {
    if (!editBeforeItem) return;
    const before = editBeforeItem;
    editBeforeItem = null;
    commitItemEdit(before, nextItem);
  }

  function toggleSelectedSvgPathEditMode() {
    if (selectedSvgElement?.type !== "path" || !selectedSvgPathData) return;
    if (selectedSvgPathEditActive) exitVectorEditMode();
    else enterVectorEditMode(svgGroupItem.id, selectedSvgElement.id);
  }

  function updateSvgFill(value: string) {
    beginEdit();
    previewItem(moodBoardSvgGroupWithFill(svgGroupItem, selectedSvgElementId, value));
  }

  function commitSvgFill(value: string) {
    commitEdit(moodBoardSvgGroupWithFill(svgGroupItem, selectedSvgElementId, value));
  }

  function updateSvgStroke(value: string) {
    beginEdit();
    previewItem(moodBoardSvgGroupWithStroke(svgGroupItem, selectedSvgElementId, value));
  }

  function commitSvgStroke(value: string) {
    commitEdit(moodBoardSvgGroupWithStroke(svgGroupItem, selectedSvgElementId, value));
  }

  function updateSvgStrokeWidth(value: string) {
    beginEdit();
    previewItem(moodBoardSvgGroupWithStrokeWidth(svgGroupItem, selectedSvgElementId, value));
  }

  function commitSvgStrokeWidth(value: string) {
    commitEdit(moodBoardSvgGroupWithStrokeWidth(svgGroupItem, selectedSvgElementId, value));
  }

  function updateSvgOpacity(value: string) {
    beginEdit();
    previewItem(moodBoardSvgGroupWithOpacity(svgGroupItem, selectedSvgElementId, value));
  }

  function commitSvgOpacity(value: string) {
    commitEdit(moodBoardSvgGroupWithOpacity(svgGroupItem, selectedSvgElementId, value));
  }

  function updateSvgText(value: string) {
    if (!selectedSvgTextElement) return;
    beginEdit();
    previewItem(moodBoardSvgGroupWithText(svgGroupItem, selectedSvgTextElement.id, value));
  }

  function commitSvgText(value: string) {
    if (!selectedSvgTextElement) return;
    commitEdit(moodBoardSvgGroupWithText(svgGroupItem, selectedSvgTextElement.id, value));
  }

  function updateSvgTextFontSize(value: string) {
    if (!selectedSvgTextElement) return;
    beginEdit();
    previewItem(moodBoardSvgGroupWithTextFontSize(svgGroupItem, selectedSvgTextElement.id, value));
  }

  function commitSvgTextFontSize(value: string) {
    if (!selectedSvgTextElement) return;
    commitEdit(moodBoardSvgGroupWithTextFontSize(svgGroupItem, selectedSvgTextElement.id, value));
  }

  function toggleSvgTextWeight() {
    if (!selectedSvgTextElement) return;
    commitItemEdit(cloneItem(svgGroupItem), moodBoardSvgGroupWithToggledTextWeight(svgGroupItem, selectedSvgTextElement.id));
  }

  function moveSelectedSvgElement(direction: "front" | "back") {
    if (!selectedSvgElementId) return;
    const nextItem = moodBoardSvgGroupWithMovedElement(svgGroupItem, selectedSvgElementId, direction);
    if (nextItem) commitItemEdit(cloneItem(svgGroupItem), nextItem);
  }

  function duplicateSelectedSvgElement() {
    if (!selectedSvgElementId) return;
    const nextItem = moodBoardSvgGroupWithDuplicatedElement(svgGroupItem, selectedSvgElementId);
    if (nextItem) commitItemEdit(cloneItem(svgGroupItem), nextItem);
  }

  function deleteSelectedSvgElement() {
    if (!selectedSvgElementId || svgGroupItem.elements.length <= 1) return;
    const nextItem = moodBoardSvgGroupWithoutElement(svgGroupItem, selectedSvgElementId);
    if (nextItem) commitItemEdit(cloneItem(svgGroupItem), nextItem);
  }

  function duplicateSelectedSvgGroup() {
    if (!activeSvgGroupPath) return;
    commitItemEdit(cloneItem(svgGroupItem), moodBoardSvgGroupWithDuplicatedGroup(svgGroupItem, activeSvgGroupPath));
  }

  function deleteSelectedSvgGroup() {
    if (!activeSvgGroupPath) return;
    const nextItem = moodBoardSvgGroupWithoutGroup(svgGroupItem, activeSvgGroupPath);
    if (nextItem) commitItemEdit(cloneItem(svgGroupItem), nextItem);
  }

  function moveSelectedSvgGroup(direction: "front" | "back") {
    if (!activeSvgGroupPath) return;
    commitItemEdit(cloneItem(svgGroupItem), moodBoardSvgGroupWithMovedGroup(svgGroupItem, activeSvgGroupPath, direction));
  }

  function toggleSelectedSvgClosed() {
    if (!selectedSvgElementId) return;
    const nextItem = moodBoardSvgGroupWithToggledSelectedPathClosed(svgGroupItem, selectedSvgElementId);
    if (nextItem) commitItemEdit(cloneItem(svgGroupItem), nextItem);
  }

  function cornerSelectedSvgPath() {
    if (!selectedSvgElementId) return;
    const nextItem = moodBoardSvgGroupWithCornerSelectedPath(svgGroupItem, selectedSvgElementId);
    if (nextItem) commitItemEdit(cloneItem(svgGroupItem), nextItem);
  }

  function smoothSelectedSvgPath() {
    if (!selectedSvgElementId) return;
    const nextItem = moodBoardSvgGroupWithSmoothSelectedPath(svgGroupItem, selectedSvgElementId);
    if (nextItem) commitItemEdit(cloneItem(svgGroupItem), nextItem);
  }

  function insertSelectedSvgNode() {
    if (!selectedSvgElementId) return;
    const nextItem = moodBoardSvgGroupWithInsertedSelectedPathNode(svgGroupItem, selectedSvgElementId);
    if (nextItem) commitItemEdit(cloneItem(svgGroupItem), nextItem);
  }

  function removeSelectedSvgNode() {
    if (!selectedSvgElementId) return;
    const nextItem = moodBoardSvgGroupWithRemovedSelectedPathNode(svgGroupItem, selectedSvgElementId);
    if (nextItem) commitItemEdit(cloneItem(svgGroupItem), nextItem);
  }

  function scaleSelectedSvgElement(scale: number) {
    if (!selectedSvgElementId) return;
    const nextItem = moodBoardSvgGroupWithScaledElement(svgGroupItem, selectedSvgElementId, scale);
    if (nextItem) commitItemEdit(cloneItem(svgGroupItem), nextItem);
  }

  function rotateSelectedSvgElement(degrees: number) {
    if (!selectedSvgElementId) return;
    const nextItem = moodBoardSvgGroupWithRotatedElement(svgGroupItem, selectedSvgElementId, degrees);
    if (nextItem) commitItemEdit(cloneItem(svgGroupItem), nextItem);
  }

  function resetSelectedSvgElementTransform() {
    if (!selectedSvgElementId) return;
    commitItemEdit(cloneItem(svgGroupItem), moodBoardSvgGroupWithResetElementTransform(svgGroupItem, selectedSvgElementId));
  }

  function exportVector() {
    void exportVectorPath(svgGroupItem.id);
  }
</script>

<span class="separator"></span>
<span class="parent-chip" title={`${svgGroupItem.elements.length} elemente SVG; ${extractableSvgCount} path-uri extractibile`}>
  {svgGroupItem.elements.length} svg
</span>
{#if svgGroupItem.unsupportedFeatures?.length}
  <span class="parent-chip warning-chip" title={`Import parțial: ${svgGroupItem.unsupportedFeatures.join(", ")}`}>
    import parțial
  </span>
{/if}
{#if selectedSvgElement}
  <span class="parent-chip selected-path-chip" title={`Element SVG selectat: ${selectedSvgElement.id}`}>{selectedSvgElement.id}</span>
  {#if selectedSvgElement.groupPath?.length}
    <span class="parent-chip muted-chip" title={`Grup SVG: ${selectedSvgElement.groupPath.join(" / ")}`}>
      {selectedSvgElement.groupPath.at(-1)}
    </span>
  {/if}
  {#if activeSvgGroupPath && activeSvgGroupElements.length > 1}
    <span class="parent-chip" title={`Grup SVG: ${activeSvgGroupPath.join(" / ")} · ${activeSvgGroupElements.length} elemente`}>
      {activeSvgGroupElements.length} în grup
    </span>
    <button type="button" title={`Duplică grupul SVG ${activeSvgGroupLabel}`} onclick={duplicateSelectedSvgGroup}>
      <IconCopy size={15} stroke={2} />
    </button>
    <button type="button" title={`Adu grupul SVG ${activeSvgGroupLabel} în față`} onclick={() => moveSelectedSvgGroup("front")}>
      <IconArrowBarUp size={15} stroke={2} />
    </button>
    <button type="button" title={`Trimite grupul SVG ${activeSvgGroupLabel} în spate`} onclick={() => moveSelectedSvgGroup("back")}>
      <IconArrowBarDown size={15} stroke={2} />
    </button>
    <button type="button" title={`Șterge grupul SVG ${activeSvgGroupLabel}`} disabled={svgGroupItem.elements.length <= activeSvgGroupElements.length} onclick={deleteSelectedSvgGroup}>
      <IconTrash size={15} stroke={2.2} />
    </button>
    <span class="separator compact"></span>
  {/if}
  {#if selectedSvgElement.type === "path"}
    <button
      type="button"
      class:active={selectedSvgPathEditActive}
      disabled={!selectedSvgPathData}
      title={selectedSvgPathEditActive ? "Ieși din editarea nodurilor SVG" : "Editează nodurile sub-path-ului SVG"}
      onclick={toggleSelectedSvgPathEditMode}
    >
      <IconVectorBezier size={15} stroke={2} />
    </button>
    <button
      type="button"
      disabled={!selectedSvgElementExtractable}
      title={svgExtractTitle}
      onclick={() => selectedSvgElementExtractable && extractSvgSubPath(svgGroupItem.id, selectedSvgElement.id)}
    >
      <IconPencil size={15} stroke={2} />
    </button>
    {#if selectedSvgPathEditActive}
      <span class="parent-chip muted-chip" title={vectorNodeEditState ? `${vectorNodeEditState.indexes.length} noduri selectate din ${vectorNodeEditState.nodeCount}` : "Selectează noduri pe sub-path"}>
        {vectorNodeEditState ? `${vectorNodeEditState.indexes.length} nod` : "noduri"}
      </span>
      <button
        type="button"
        class:active={Boolean(selectedSvgPathData?.closed)}
        disabled={!selectedSvgElementExtractable}
        title="Închide/deschide sub-path"
        onclick={toggleSelectedSvgClosed}
      >Z</button>
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
      <button
        type="button"
        disabled={!selectedSvgElementExtractable}
        title="Transformă sub-path-ul în colțuri"
        onclick={cornerSelectedSvgPath}
      >
        <IconCornerDownRight size={15} stroke={2} />
      </button>
      <button
        type="button"
        disabled={!selectedSvgElementExtractable}
        title="Netezește sub-path-ul"
        onclick={smoothSelectedSvgPath}
      >S</button>
      <button
        type="button"
        disabled={!selectedSvgElementExtractable}
        title="Adaugă nod pe cel mai lung segment"
        onclick={insertSelectedSvgNode}
      >+N</button>
      <button
        type="button"
        disabled={!selectedSvgElementExtractable}
        title="Șterge ultimul nod"
        onclick={removeSelectedSvgNode}
      >-N</button>
    {/if}
  {:else}
    <input
      class="text-control"
      type="text"
      value={selectedSvgElement.text}
      aria-label="Text SVG"
      title="Text SVG"
      onfocus={beginEdit}
      oninput={(event) => updateSvgText(event.currentTarget.value)}
      onblur={(event) => commitSvgText(event.currentTarget.value)}
    />
    <input
      class="number-control compact-number"
      type="number"
      min="1"
      max="512"
      step="1"
      value={selectedSvgElement.fontSize}
      aria-label="Mărime text SVG"
      onfocus={beginEdit}
      oninput={(event) => updateSvgTextFontSize(event.currentTarget.value)}
      onblur={(event) => commitSvgTextFontSize(event.currentTarget.value)}
    />
    <button type="button" class:active={selectedSvgElement.fontWeight >= 700} title="Bold text SVG" onclick={toggleSvgTextWeight}>
      <IconBold size={15} stroke={2} />
    </button>
  {/if}
  <button type="button" title="Micșorează elementul SVG" onclick={() => scaleSelectedSvgElement(0.9)}>0.9x</button>
  <button type="button" title="Mărește elementul SVG" onclick={() => scaleSelectedSvgElement(1.1)}>1.1x</button>
  <button type="button" title="Rotește elementul SVG la stânga" onclick={() => rotateSelectedSvgElement(-15)}>-15</button>
  <button type="button" title="Rotește elementul SVG la dreapta" onclick={() => rotateSelectedSvgElement(15)}>+15</button>
  <button type="button" title="Resetează transformarea elementului SVG" onclick={resetSelectedSvgElementTransform}>R</button>
  <button type="button" title="Duplică sub-path-ul" onclick={duplicateSelectedSvgElement}>
    <IconCopy size={15} stroke={2} />
  </button>
  <button type="button" title="Adu sub-path-ul în față" onclick={() => moveSelectedSvgElement("front")}>
    <IconArrowBarUp size={15} stroke={2} />
  </button>
  <button type="button" title="Trimite sub-path-ul în spate" onclick={() => moveSelectedSvgElement("back")}>
    <IconArrowBarDown size={15} stroke={2} />
  </button>
  <button type="button" title="Șterge elementul SVG selectat" disabled={svgGroupItem.elements.length <= 1} onclick={deleteSelectedSvgElement}>
    <IconTrash size={15} stroke={2.2} />
  </button>
{:else}
  <span class="parent-chip muted-chip" title="Click pe o formă din SVG pentru sub-selecție">fără sub-path</span>
{/if}
<button
  type="button"
  disabled={extractableSvgCount === 0}
  title={extractableSvgCount > 0 ? `Extrage toate sub-path-urile compatibile (${extractableSvgCount})` : "Nu există sub-path-uri extractibile"}
  onclick={() => extractAllSvgSubPaths(svgGroupItem.id)}
>
  <IconStack2 size={15} stroke={2} />
</button>
<button
  type="button"
  disabled={extractableSvgCount === 0}
  title={extractableSvgCount > 0 ? "Degrupează SVG-ul în path-uri editabile" : "Nu există path-uri compatibile pentru degrupare"}
  onclick={() => ungroupVectorGroup(svgGroupItem.id)}
>
  <IconUnlink size={15} stroke={2} />
</button>
<label class="color-control" style={`--control-color:${selectedSvgElement?.fill ?? firstSvgPathElement?.fill ?? "transparent"};`} title="Fill SVG">
  <input
    type="color"
    value={moodBoardColorInputValue(selectedSvgElement?.fill ?? firstSvgPathElement?.fill, "#ffffff")}
    aria-label="Fill SVG"
    onfocus={beginEdit}
    oninput={(event) => updateSvgFill(event.currentTarget.value)}
    onchange={(event) => commitSvgFill(event.currentTarget.value)}
    onblur={(event) => commitSvgFill(event.currentTarget.value)}
  />
</label>
{#if !selectedSvgElement || selectedSvgElement.type === "path"}
  <label class="color-control" style={`--control-color:${selectedSvgElement?.type === "path" ? selectedSvgElement.stroke : firstSvgPathElement?.stroke ?? "#1d7f6a"};`} title="Stroke SVG">
    <input
      type="color"
      value={moodBoardColorInputValue(selectedSvgElement?.type === "path" ? selectedSvgElement.stroke : firstSvgPathElement?.stroke, "#1d7f6a")}
      aria-label="Stroke SVG"
      onfocus={beginEdit}
      oninput={(event) => updateSvgStroke(event.currentTarget.value)}
      onchange={(event) => commitSvgStroke(event.currentTarget.value)}
      onblur={(event) => commitSvgStroke(event.currentTarget.value)}
    />
  </label>
  <input
    class="number-control compact-number"
    type="number"
    min="0"
    max="64"
    step="1"
    value={selectedSvgElement?.type === "path" ? selectedSvgElement.strokeWidth : firstSvgPathElement?.strokeWidth ?? 1}
    aria-label="Grosime contur SVG"
    onfocus={beginEdit}
    oninput={(event) => updateSvgStrokeWidth(event.currentTarget.value)}
    onblur={(event) => commitSvgStrokeWidth(event.currentTarget.value)}
  />
{/if}
<label class="opacity-control" title="Opacitate SVG">
  O
  <input
    type="range"
    min="0"
    max="1"
    step="0.05"
    value={selectedSvgElement?.opacity ?? svgGroupItem.elements[0]?.opacity ?? 1}
    aria-label="Opacitate SVG"
    onfocus={beginEdit}
    oninput={(event) => updateSvgOpacity(event.currentTarget.value)}
    onblur={(event) => commitSvgOpacity(event.currentTarget.value)}
  />
</label>
<button type="button" title="Exportă SVG în resurse/imagini" onclick={exportVector}>
  <IconDownload size={15} stroke={2} />
</button>
