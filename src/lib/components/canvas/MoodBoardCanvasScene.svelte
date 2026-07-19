<script lang="ts">
  import MoodBoardItem from "$lib/components/canvas/MoodBoardItem.svelte";
  import type {
    MoodBoard,
    MoodBoardItem as MoodBoardItemData,
    MoodBoardResizeHandle,
    MoodBoardVectorNode,
    MoodBoardVectorNodeEditState,
  } from "$lib/mood-board/model";
  import type { MoodBoardMarqueeBox } from "$lib/mood-board/drag";
  import { moodBoardPenDraftPreviewBounds } from "$lib/mood-board/pen";
  import type { MoodBoardSnapGuide } from "$lib/mood-board/snap";
  import type { MoodBoardIdentityGuard, MoodBoardRequestIdentity } from "$lib/mood-board/io";

  export let board: MoodBoard;
  export let visibleItems: MoodBoardItemData[] = [];
  export let attachFrameId: string | null = null;
  export let selectedSvgElementId: string | null = null;
  export let vectorEditTargetItemId: string | null = null;
  export let vectorEditTargetSvgElementId: string | null = null;
  export let vectorNodeEditState: MoodBoardVectorNodeEditState | null = null;
  export let penDraftNodes: MoodBoardVectorNode[] = [];
  export let snapGuides: MoodBoardSnapGuide[] = [];
  export let marqueeBox: MoodBoardMarqueeBox | null = null;
  export let isItemSelected: (itemId: string) => boolean = () => false;
  export let parentFrameTitle: (itemId: string) => string = () => "";
  export let onPointerDown: (event: PointerEvent, itemId: string) => void;
  export let onResizePointerDown: (event: PointerEvent, itemId: string, resizeHandle: MoodBoardResizeHandle) => void;
  export let enterVectorEditMode: (itemId: string, svgElementId?: string | null) => void = () => undefined;
  export let onSvgElementSelect: (itemId: string, elementId: string | null) => void = () => undefined;
  export let onVectorNodeSelectionChange: (state: MoodBoardVectorNodeEditState | null) => void = () => undefined;
  export let previewItem: (item: MoodBoardItemData) => void;
  export let commitItemEdit: (beforeItem: MoodBoardItemData, nextItem: MoodBoardItemData) => void;
  export let sessionIdentity: MoodBoardRequestIdentity;
  export let isSessionCurrent: MoodBoardIdentityGuard;
</script>

<div
  class="mood-content"
  style={`transform: translate(${board.viewport.x}px, ${board.viewport.y}px) scale(${board.viewport.zoom});`}
>
  {#each visibleItems as item (item.id)}
    <MoodBoardItem
      {item}
      childItems={item.type === "frame" || item.type === "group" ? item.children : []}
      selected={isItemSelected(item.id)}
      showVectorNodes={item.type === "vectorPath" && vectorEditTargetItemId === item.id}
      attachTarget={attachFrameId === item.id}
      parentFrameTitle={parentFrameTitle(item.id)}
      {isItemSelected}
      {onPointerDown}
      {onResizePointerDown}
      selectedSvgElementId={isItemSelected(item.id) ? selectedSvgElementId : null}
      {vectorEditTargetItemId}
      {vectorEditTargetSvgElementId}
      {vectorNodeEditState}
      {enterVectorEditMode}
      {onSvgElementSelect}
      {onVectorNodeSelectionChange}
      {previewItem}
      {commitItemEdit}
      {sessionIdentity}
      {isSessionCurrent}
    />
  {/each}
  {#if penDraftNodes.length}
    {@const draft = moodBoardPenDraftPreviewBounds(penDraftNodes)}
    {#if draft}
      <svg
        class="pen-draft"
        style={`left:${draft.x}px;top:${draft.y}px;width:${draft.width}px;height:${draft.height}px;`}
        viewBox={`0 0 ${draft.width} ${draft.height}`}
        aria-label="Path în desenare"
      >
        {#if draft.path}
          <path d={draft.path} />
        {/if}
        {#each draft.nodes as node, index}
          <circle class:first-node={index === 0 && penDraftNodes.length >= 3} cx={node.x} cy={node.y} r="5" />
        {/each}
      </svg>
    {/if}
  {/if}
  {#each snapGuides as guide}
    <div
      class={`snap-guide ${guide.orientation}`}
      style={guide.orientation === "vertical"
        ? `left:${guide.position}px;top:${guide.start}px;height:${guide.end - guide.start}px;`
        : `top:${guide.position}px;left:${guide.start}px;width:${guide.end - guide.start}px;`}
    ></div>
  {/each}
</div>

{#if marqueeBox}
  <div
    class="selection-marquee"
    style={`left: ${marqueeBox.x}px; top: ${marqueeBox.y}px; width: ${marqueeBox.width}px; height: ${marqueeBox.height}px;`}
  ></div>
{/if}

{#if board.items.length === 0}
  <div class="empty-canvas">
    <h2>Mood board</h2>
    <p>Adaugă note, culori sau referințe ca să schițezi direcția vizuală a site-ului.</p>
  </div>
{/if}

<style>
  .mood-content {
    position: absolute;
    inset: 0;
    transform-origin: 0 0;
  }

  .snap-guide {
    position: absolute;
    z-index: 20;
    pointer-events: none;
    background: var(--brand);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--surface) 72%, transparent);
  }

  .pen-draft {
    position: absolute;
    z-index: 18;
    overflow: visible;
    pointer-events: none;
  }

  .pen-draft path {
    fill: none;
    stroke: var(--brand);
    stroke-width: 3;
    vector-effect: non-scaling-stroke;
  }

  .pen-draft circle {
    fill: var(--surface);
    stroke: var(--brand);
    stroke-width: 2;
    vector-effect: non-scaling-stroke;
  }

  .pen-draft circle.first-node {
    fill: var(--brand-soft);
    stroke-width: 3;
  }

  .snap-guide.vertical {
    width: 1px;
  }

  .snap-guide.horizontal {
    height: 1px;
  }

  .selection-marquee {
    position: absolute;
    z-index: 7;
    border: 1px solid var(--brand);
    border-radius: 4px;
    background: color-mix(in srgb, var(--brand) 14%, transparent);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--surface) 60%, transparent) inset;
    pointer-events: none;
  }

  .empty-canvas {
    position: absolute;
    left: 24px;
    top: 24px;
    max-width: 360px;
    padding: 14px 16px;
    border: 1px solid var(--border-3);
    border-radius: 10px;
    color: var(--text-muted);
    background: color-mix(in srgb, var(--surface) 86%, transparent);
    pointer-events: none;
  }

  .empty-canvas h2,
  .empty-canvas p {
    margin: 0;
  }

  .empty-canvas h2 {
    color: var(--text-strong);
    font-size: 15px;
    font-weight: 850;
  }

  .empty-canvas p {
    margin-top: 6px;
    font-size: 12px;
    line-height: 1.45;
  }
</style>
