<script lang="ts">
  import {
    IconArrowBarDown,
    IconArrowBarUp,
    IconCopy,
    IconCornerUpLeft,
    IconDownload,
    IconStack2,
    IconTrash,
    IconUnlink,
  } from "@tabler/icons-svelte";
  import MoodBoardColorControls from "$lib/components/canvas/MoodBoardColorControls.svelte";
  import MoodBoardFrameControls from "$lib/components/canvas/MoodBoardFrameControls.svelte";
  import MoodBoardImageControls from "$lib/components/canvas/MoodBoardImageControls.svelte";
  import MoodBoardShapeControls from "$lib/components/canvas/MoodBoardShapeControls.svelte";
  import MoodBoardSvgGroupControls from "$lib/components/canvas/MoodBoardSvgGroupControls.svelte";
  import MoodBoardTextControls from "$lib/components/canvas/MoodBoardTextControls.svelte";
  import MoodBoardVectorPathControls from "$lib/components/canvas/MoodBoardVectorPathControls.svelte";
  import type { MoodBoardAlignMode, MoodBoardDistributeMode } from "$lib/mood-board/layout";
  import type {
    MoodBoardItem as MoodBoardItemData,
    MoodBoardVectorHandleMode,
    MoodBoardVectorNodeEditState,
  } from "$lib/mood-board/model";
  import type { ScssVariable } from "$lib/types";

  export let selectedItems: MoodBoardItemData[] = [];
  export let selectedSvgElementId: string | null = null;
  export let parentFrameTitle = "";
  export let scssVariables: ScssVariable[] = [];
  export let previewItem: (item: MoodBoardItemData) => void;
  export let commitItemEdit: (beforeItem: MoodBoardItemData, nextItem: MoodBoardItemData) => void;
  export let duplicateSelectedItems: () => void;
  export let groupSelectedItems: () => void;
  export let ungroupSelectedGroup: () => void;
  export let alignSelectedItems: (mode: MoodBoardAlignMode) => void = () => undefined;
  export let distributeSelectedItems: (mode: MoodBoardDistributeMode) => void = () => undefined;
  export let bringSelectedItemsToFront: () => void;
  export let sendSelectedItemsToBack: () => void;
  export let deleteSelectedItems: () => void;
  export let detachItem: (itemId: string) => void;
  export let applyImageToSelectedElement: ((path: string) => void | Promise<void>) | undefined = undefined;
  export let extractPaletteFromImage: ((itemId: string, path: string) => void | Promise<void>) | undefined = undefined;
  export let canApplyVectorMask: (itemId: string) => boolean = () => false;
  export let applyVectorMask: (itemId: string) => void = () => undefined;
  export let clearVectorMask: (itemId: string) => void = () => undefined;
  export let exportVectorPath: (itemId: string) => void | Promise<void> = () => undefined;
  export let extractSvgSubPath: (itemId: string, elementId: string) => void = () => undefined;
  export let extractAllSvgSubPaths: (itemId: string) => void = () => undefined;
  export let ungroupVectorGroup: (itemId: string) => void = () => undefined;
  export let convertShapeToPath: (itemId: string) => void = () => undefined;
  export let exportImageWebp: (itemId: string) => void | Promise<void> = () => undefined;
  export let exportCompositionWebp: (itemId: string) => void | Promise<void> = () => undefined;
  export let heavyAssetBusy = false;
  export let applyColorToScssVariable: ((color: string, label: string, variableName?: string) => void | Promise<void>) | undefined = undefined;
  export let vectorEditTargetItemId: string | null = null;
  export let vectorEditTargetSvgElementId: string | null = null;
  export let vectorNodeEditState: MoodBoardVectorNodeEditState | null = null;
  export let setSelectedVectorNodeHandleMode: (mode: MoodBoardVectorHandleMode) => void = () => undefined;
  export let enterVectorEditMode: (itemId: string, svgElementId?: string | null) => void = () => undefined;
  export let exitVectorEditMode: () => void = () => undefined;

  let activeVectorNodeMode: MoodBoardVectorHandleMode | "mixed" | null = null;

  $: singleItem = selectedItems.length === 1 ? selectedItems[0] : null;
  $: imageItem = singleItem?.type === "image" ? singleItem : null;
  $: vectorItem = singleItem?.type === "vectorPath" ? singleItem : null;
  $: textItem = singleItem?.type === "text" ? singleItem : null;
  $: shapeItem = singleItem?.type === "shape" ? singleItem : null;
  $: frameItem = singleItem?.type === "frame" ? singleItem : null;
  $: groupItem = singleItem?.type === "group" ? singleItem : null;
  $: colorItem = singleItem?.type === "color" ? singleItem : null;
  $: svgGroupItem = singleItem?.type === "vectorGroup" ? singleItem : null;
  $: vectorPathEditActive = Boolean(vectorItem && vectorEditTargetItemId === vectorItem.id);
  $: activeVectorNodeMode = vectorNodeEditState && vectorNodeEditState.modes.length > 0
    ? vectorNodeEditState.modes.every((mode) => mode === vectorNodeEditState?.modes[0])
      ? vectorNodeEditState.modes[0]
      : "mixed"
    : null;
</script>

{#if selectedItems.length}
  <div class="canvas-context-bar" role="toolbar" tabindex="-1" aria-label="Controale canvas" onpointerdown={(event) => event.stopPropagation()}>
    <div class="context-left">
      {#if selectedItems.length > 1}
        <span class="selection-chip">{selectedItems.length} elemente</span>
      {:else if singleItem}
        <span class="selection-chip">{singleItem.type}</span>
      {/if}

      {#if parentFrameTitle && singleItem}
        <span class="parent-chip" title={`În frame: ${parentFrameTitle}`}>{parentFrameTitle}</span>
        <button type="button" title="Scoate din frame" onclick={() => detachItem(singleItem.id)}>
          <IconCornerUpLeft size={15} stroke={2} />
        </button>
      {/if}

      <span class="separator"></span>

      <button type="button" title="Duplică" onclick={duplicateSelectedItems}>
        <IconCopy size={15} stroke={2} />
      </button>
      {#if selectedItems.length > 1}
        <button type="button" title="Grupează selecția" onclick={groupSelectedItems}>
          <IconStack2 size={15} stroke={2} />
        </button>
      {:else if groupItem}
        <button type="button" title="Degrupează" onclick={ungroupSelectedGroup}>
          <IconUnlink size={15} stroke={2} />
        </button>
      {/if}
      <button type="button" title="Adu în față" onclick={bringSelectedItemsToFront}>
        <IconArrowBarUp size={15} stroke={2} />
      </button>
      <button type="button" title="Trimite în spate" onclick={sendSelectedItemsToBack}>
        <IconArrowBarDown size={15} stroke={2} />
      </button>
      {#if frameItem || groupItem}
        <button type="button" title="Exportă compoziția WebP lossless / 1920px" disabled={heavyAssetBusy} onclick={() => singleItem && exportCompositionWebp(singleItem.id)}>
          <IconDownload size={15} stroke={2} />
        </button>
      {/if}
      {#if selectedItems.length > 1}
        <span class="separator"></span>
        <button type="button" title="Aliniază la stânga" onclick={() => alignSelectedItems("left")}>L</button>
        <button type="button" title="Aliniază pe centru orizontal" onclick={() => alignSelectedItems("center")}>C</button>
        <button type="button" title="Aliniază la dreapta" onclick={() => alignSelectedItems("right")}>R</button>
        <button type="button" title="Aliniază sus" onclick={() => alignSelectedItems("top")}>T</button>
        <button type="button" title="Aliniază pe centru vertical" onclick={() => alignSelectedItems("middle")}>M</button>
        <button type="button" title="Aliniază jos" onclick={() => alignSelectedItems("bottom")}>B</button>
        {#if selectedItems.length > 2}
          <button type="button" title="Distribuie orizontal" onclick={() => distributeSelectedItems("horizontal")}>H</button>
          <button type="button" title="Distribuie vertical" onclick={() => distributeSelectedItems("vertical")}>V</button>
        {/if}
      {/if}

      {#if imageItem}
        <MoodBoardImageControls
          {imageItem}
          {previewItem}
          {commitItemEdit}
          {applyImageToSelectedElement}
          {extractPaletteFromImage}
          {canApplyVectorMask}
          {applyVectorMask}
          {clearVectorMask}
          {exportImageWebp}
          {heavyAssetBusy}
        />
      {:else if colorItem}
        <MoodBoardColorControls
          {colorItem}
          {scssVariables}
          {previewItem}
          {commitItemEdit}
          {applyColorToScssVariable}
        />
      {:else if vectorItem}
        <MoodBoardVectorPathControls
          {vectorItem}
          {vectorPathEditActive}
          {activeVectorNodeMode}
          {vectorNodeEditState}
          {previewItem}
          {commitItemEdit}
          {setSelectedVectorNodeHandleMode}
          {enterVectorEditMode}
          {exitVectorEditMode}
          {exportVectorPath}
        />
      {:else if svgGroupItem}
        <MoodBoardSvgGroupControls
          {svgGroupItem}
          {selectedSvgElementId}
          {vectorEditTargetItemId}
          {vectorEditTargetSvgElementId}
          {vectorNodeEditState}
          {previewItem}
          {commitItemEdit}
          {setSelectedVectorNodeHandleMode}
          {enterVectorEditMode}
          {exitVectorEditMode}
          {exportVectorPath}
          {extractSvgSubPath}
          {extractAllSvgSubPaths}
          {ungroupVectorGroup}
        />
      {:else if textItem}
        <MoodBoardTextControls
          {textItem}
          {previewItem}
          {commitItemEdit}
        />
      {:else if shapeItem}
        <MoodBoardShapeControls
          {shapeItem}
          {previewItem}
          {commitItemEdit}
          {convertShapeToPath}
        />
      {:else if frameItem}
        <MoodBoardFrameControls
          {frameItem}
          {previewItem}
          {commitItemEdit}
        />
      {/if}
    </div>

    <button class="delete-button" type="button" title="Șterge selecția" aria-label="Șterge selecția" onclick={deleteSelectedItems}>
      <IconTrash size={17} stroke={2.3} />
    </button>
  </div>
{/if}

<style>
  .canvas-context-bar {
    position: absolute;
    top: 12px;
    left: 50%;
    z-index: 45;
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: min(calc(100% - 28px), 860px);
    min-height: 44px;
    padding: 5px 6px 5px 10px;
    border: 1px solid var(--border-3);
    border-radius: 10px;
    background: color-mix(in srgb, var(--surface) 94%, transparent);
    box-shadow: 0 14px 34px rgba(0, 0, 0, 0.18);
    backdrop-filter: blur(12px);
    transform: translateX(-50%);
  }

  .context-left {
    display: flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
    overflow-x: auto;
    scrollbar-width: none;
  }

  .context-left::-webkit-scrollbar {
    display: none;
  }

  .canvas-context-bar :global(button),
  .canvas-context-bar :global(.radius-control),
  .canvas-context-bar :global(.opacity-control) {
    display: inline-grid;
    place-items: center;
    flex: 0 0 auto;
    min-width: 31px;
    height: 30px;
    padding: 0 8px;
    border: 1px solid transparent;
    border-radius: 7px;
    color: var(--text-muted);
    background: transparent;
    font-size: 12px;
    font-weight: 850;
    white-space: nowrap;
  }

  .canvas-context-bar :global(button:hover),
  .canvas-context-bar :global(button.active) {
    color: var(--brand);
    border-color: var(--brand);
    background: var(--brand-soft);
  }

  .canvas-context-bar :global(button:disabled) {
    color: color-mix(in srgb, var(--text-muted) 42%, transparent);
    border-color: transparent;
    background: transparent;
    cursor: not-allowed;
  }

  .delete-button {
    margin-left: 8px;
    color: #c93333;
  }

  .delete-button:hover {
    color: #c93333;
    border-color: color-mix(in srgb, #c93333 52%, var(--border-3));
    background: color-mix(in srgb, #c93333 12%, var(--surface));
  }

  .canvas-context-bar :global(.selection-chip),
  .canvas-context-bar :global(.parent-chip),
  .canvas-context-bar :global(.muted) {
    flex: 0 0 auto;
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 800;
  }

  .canvas-context-bar :global(.selection-chip) {
    color: var(--text);
  }

  .canvas-context-bar :global(.parent-chip) {
    max-width: 130px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .canvas-context-bar :global(.selected-path-chip) {
    color: var(--brand);
  }

  .canvas-context-bar :global(.muted-chip) {
    color: color-mix(in srgb, var(--text-muted) 72%, transparent);
  }

  .canvas-context-bar :global(.warning-chip) {
    color: #9b5a00;
  }

  .canvas-context-bar :global(.separator) {
    flex: 0 0 auto;
    width: 1px;
    height: 24px;
    margin: 0 4px;
    background: var(--border-3);
  }

  .canvas-context-bar :global(.radius-control),
  .canvas-context-bar :global(.opacity-control),
  .canvas-context-bar :global(.color-control) {
    grid-template-columns: auto 34px;
    gap: 5px;
    border-color: var(--border-3);
  }

  .canvas-context-bar :global(.text-control) {
    flex: 0 0 auto;
    width: 150px;
    height: 30px;
    min-width: 0;
    padding: 0 8px;
    border: 1px solid var(--border-3);
    border-radius: 7px;
    color: var(--text);
    background: var(--surface);
    font-size: 12px;
    font-weight: 750;
    outline: none;
  }

  .canvas-context-bar :global(.text-control:focus) {
    border-color: var(--brand);
    box-shadow: 0 0 0 2px var(--brand-soft);
  }

  .canvas-context-bar :global(.opacity-control) {
    grid-template-columns: auto 54px;
  }

  .canvas-context-bar :global(.color-control) {
    display: block;
    width: 31px;
    min-width: 31px;
    padding: 0;
    background: var(--control-color);
    overflow: hidden;
  }

  .canvas-context-bar :global(.radius-control input),
  .canvas-context-bar :global(.opacity-control input),
  .canvas-context-bar :global(.color-control input) {
    width: 34px;
    padding: 0;
    border: 0;
    color: var(--text);
    text-align: center;
    background: transparent;
    font: inherit;
  }

  .canvas-context-bar :global(.opacity-control input) {
    width: 54px;
    accent-color: var(--brand);
  }

  .canvas-context-bar :global(.color-control input) {
    width: 100%;
    height: 100%;
    opacity: 0;
    cursor: pointer;
  }

  .canvas-context-bar :global(.number-control) {
    flex: 0 0 auto;
    width: 54px;
    height: 30px;
    border: 1px solid var(--border-3);
    border-radius: 7px;
    color: var(--text);
    text-align: center;
    background: var(--surface-2);
    font-size: 12px;
    font-weight: 800;
  }

  .canvas-context-bar :global(.number-control.compact-number) {
    width: 44px;
  }

  .canvas-context-bar :global(.select-control),
  .canvas-context-bar :global(.variable-select) {
    flex: 0 0 auto;
    height: 30px;
    max-width: 116px;
    border: 1px solid var(--border-3);
    border-radius: 7px;
    color: var(--text);
    background: var(--surface-2);
    font-size: 12px;
    font-weight: 800;
  }

  .canvas-context-bar :global(.variable-wrap) {
    position: relative;
    display: inline-grid;
    place-items: center;
  }

  .canvas-context-bar :global(.variable-select) {
    position: absolute;
    left: 50%;
    top: calc(100% + 8px);
    z-index: 3;
    width: 220px;
    max-width: 220px;
    transform: translateX(-50%);
    box-shadow: 0 12px 26px rgba(0, 0, 0, 0.18);
  }

</style>
