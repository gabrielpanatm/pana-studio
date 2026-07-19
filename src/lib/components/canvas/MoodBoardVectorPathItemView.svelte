<script lang="ts">
  import type {
    MoodBoardVectorHandleMode,
    MoodBoardVectorNode,
    MoodBoardVectorPathItem,
  } from "$lib/mood-board/model";
  import {
    moodBoardVectorHandleLineStyleFromNode,
    moodBoardVectorHandleStyle,
    moodBoardVectorNodeHandleMode,
    moodBoardVectorNodeStyle,
  } from "$lib/mood-board/item-view";
  import { buildVectorSvgPath } from "$lib/mood-board/vector";

  type VectorNodeHandle = "node" | "in" | "out";
  type VectorControlHandle = "in" | "out";

  export let item: MoodBoardVectorPathItem;
  export let selected = false;
  export let showVectorNodes = false;
  export let vectorNodeSelectionToken = "";
  export let editActive = false;
  export let isNodeSelected: (index: number) => boolean = () => false;
  export let isHandleVisible: (index: number, handle: VectorControlHandle) => boolean = () => false;
  export let onPathDoubleClick: (event: MouseEvent) => void = () => undefined;
  export let onNodePointerDown: (event: PointerEvent, index: number, handle: VectorNodeHandle) => void = () => undefined;
  export let onNodeClick: (event: MouseEvent, index: number) => void = () => undefined;
  export let onNodeKeydown: (event: KeyboardEvent, index: number) => void = () => undefined;

  function nodeHandleMode(node: MoodBoardVectorNode): MoodBoardVectorHandleMode {
    return moodBoardVectorNodeHandleMode(node);
  }
</script>

<svg
  class="vector-path-preview"
  viewBox={`0 0 ${item.viewBoxWidth} ${item.viewBoxHeight}`}
  preserveAspectRatio="none"
  aria-label="Path vectorial"
>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <path
    d={buildVectorSvgPath(item)}
    fill={item.fill}
    stroke={item.stroke}
    stroke-width={item.strokeWidth}
    vector-effect="non-scaling-stroke"
    ondblclick={onPathDoubleClick}
  />
  {#if selected}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <path
      class="vector-path-hit"
      d={buildVectorSvgPath(item)}
      fill="transparent"
      stroke="transparent"
      stroke-width="16"
      vector-effect="non-scaling-stroke"
      ondblclick={onPathDoubleClick}
    />
  {/if}
</svg>

{#if showVectorNodes}
  <div class="vector-html-node-layer" data-vector-selection={vectorNodeSelectionToken} aria-label="Noduri path">
    {#each item.nodes as node, index}
      {#if vectorNodeSelectionToken && node.in && isHandleVisible(index, "in")}
        <div class="vector-html-handle-line" style={moodBoardVectorHandleLineStyleFromNode(item, node, "in")}></div>
        <div
          role="button"
          tabindex="0"
          class="vector-html-handle"
          style={moodBoardVectorHandleStyle(item, node, "in")}
          aria-label={`Handle intrare ${index + 1}`}
          onpointerdown={(event) => onNodePointerDown(event, index, "in")}
          onclick={(event) => onNodeClick(event, index)}
          onkeydown={(event) => onNodeKeydown(event, index)}
        ></div>
      {/if}
      {#if vectorNodeSelectionToken && node.out && isHandleVisible(index, "out")}
        <div class="vector-html-handle-line" style={moodBoardVectorHandleLineStyleFromNode(item, node, "out")}></div>
        <div
          role="button"
          tabindex="0"
          class="vector-html-handle"
          style={moodBoardVectorHandleStyle(item, node, "out")}
          aria-label={`Handle ieșire ${index + 1}`}
          onpointerdown={(event) => onNodePointerDown(event, index, "out")}
          onclick={(event) => onNodeClick(event, index)}
          onkeydown={(event) => onNodeKeydown(event, index)}
        ></div>
      {/if}
      <div
        role="button"
        tabindex="0"
        class="vector-html-node"
        class:edit-mode={editActive}
        class:selected-node={Boolean(vectorNodeSelectionToken) && isNodeSelected(index)}
        class:mirrored-node={nodeHandleMode(node) === "mirrored"}
        class:locked-node={nodeHandleMode(node) === "locked"}
        class:independent-node={nodeHandleMode(node) === "independent"}
        style={moodBoardVectorNodeStyle(item, node)}
        aria-label={`Nod ${index + 1}`}
        onpointerdown={(event) => onNodePointerDown(event, index, "node")}
        onclick={(event) => onNodeClick(event, index)}
        onkeydown={(event) => onNodeKeydown(event, index)}
      ></div>
    {/each}
  </div>
{/if}

<style>
  .vector-path-preview {
    display: block;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    overflow: visible;
  }

  .vector-path-hit {
    cursor: cell;
    pointer-events: all;
  }

  .vector-html-node-layer {
    position: absolute;
    inset: 0;
    z-index: 24;
    pointer-events: none;
  }

  .vector-html-node,
  .vector-html-handle {
    position: absolute;
    z-index: 2;
    width: 13px;
    height: 13px;
    padding: 0;
    border: 2px solid var(--brand);
    border-radius: 999px;
    background: #ffffff;
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.92), 0 2px 8px rgba(0, 0, 0, 0.18);
    cursor: move;
    transform: translate(-50%, -50%);
    pointer-events: auto;
  }

  .vector-html-handle {
    z-index: 1;
    width: 11px;
    height: 11px;
    background: var(--brand);
    border-color: var(--surface);
  }

  .vector-html-handle-line {
    position: absolute;
    z-index: 0;
    height: 1.5px;
    background: color-mix(in srgb, var(--brand) 70%, #ffffff);
    transform-origin: 0 50%;
    pointer-events: none;
  }

  .vector-html-node:not(.edit-mode) {
    opacity: 0.92;
  }

  .vector-html-node:hover,
  .vector-html-handle:hover {
    background: var(--brand-strong);
  }

  .vector-html-node.selected-node {
    background: var(--brand);
    border-color: var(--surface);
  }

  .vector-html-node.locked-node {
    border-style: dashed;
  }

  .vector-html-node.independent-node {
    border-color: color-mix(in srgb, var(--brand) 62%, var(--text-muted));
  }
</style>
