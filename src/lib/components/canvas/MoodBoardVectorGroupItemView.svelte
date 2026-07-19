<script lang="ts">
  import { tick } from "svelte";
  import type {
    MoodBoardVectorGroupItem,
    MoodBoardItem,
    MoodBoardVectorHandleMode,
    MoodBoardVectorNode,
    MoodBoardVectorTransform,
  } from "$lib/mood-board/model";
  import { moodBoardVectorGroupWithTextElement } from "$lib/mood-board/item-content-actions";
  import { editableSvgElementNodes } from "$lib/mood-board/svg";
  import {
    vectorBoundsCenter,
    type MoodBoardBounds,
  } from "$lib/mood-board/vector";

  export let item: MoodBoardVectorGroupItem;
  export let selected = false;
  export let selectedSvgElementId: string | null = null;
  export let vectorNodeSelectionToken = "";
  export let beginEdit: () => void = () => undefined;
  export let previewEdit: (nextItem: MoodBoardItem) => void = () => undefined;
  export let commitEdit: (nextItem: MoodBoardItem) => void = () => undefined;
  export let cancelEdit: () => void = () => undefined;
  export let selectSvgElement: (event: PointerEvent, elementId: string | null) => void = () => undefined;
  export let onSvgElementSelect: (itemId: string, elementId: string | null) => void = () => undefined;
  export let vectorGroupElementEditActive: (elementId: string) => boolean = () => false;
  export let selectedElementTransformedBounds: (
    nodes: MoodBoardVectorNode[],
    transform: MoodBoardVectorTransform,
  ) => MoodBoardBounds = () => ({ minX: 0, minY: 0, maxX: 0, maxY: 0 });
  export let activeSvgTransformBounds: (elementId: string) => MoodBoardBounds | null = () => null;
  export let beginVectorGroupElementDrag: (event: PointerEvent, elementId: string) => void = () => undefined;
  export let svgTextEditWidth: (text: string, fontSize: number) => number = () => 0;
  export let handleVectorGroupPathDoubleClick: (event: MouseEvent, elementId: string) => void = () => undefined;
  export let vectorGroupHandleOffset: () => number = () => 28;
  export let beginVectorGroupElementTransformDrag: (
    event: PointerEvent,
    elementId: string,
    mode: "scale" | "rotate",
    bounds: MoodBoardBounds,
  ) => void = () => undefined;
  export let vectorGroupNodeScope: (elementId: string) => string = () => "";
  export let vectorHandleVisible: (
    scope: string,
    nodes: MoodBoardVectorNode[],
    index: number,
    handle: "in" | "out",
    closed: boolean,
  ) => boolean = () => false;
  export let beginVectorGroupDrag: (
    event: PointerEvent,
    elementId: string,
    nodeIndex: number,
    handle: "node" | "in" | "out",
  ) => void = () => undefined;
  export let handleVectorNodeClick: (event: MouseEvent, scope: string, nodeIndex: number) => void = () => undefined;
  export let isVectorNodeSelected: (scope: string, nodeIndex: number) => boolean = () => false;
  export let nodeHandleMode: (node: MoodBoardVectorNode) => MoodBoardVectorHandleMode = () => "corner";

  let svgTextEditingElementId = "";
  let svgTextEditorEl: HTMLInputElement | null = null;

  $: if (!selected) {
    svgTextEditingElementId = "";
  }

  function beginSvgTextEdit(event: MouseEvent, elementId: string) {
    const element = item.elements.find((entry) => entry.id === elementId);
    if (!element || element.type !== "text") return;
    event.preventDefault();
    event.stopPropagation();
    onSvgElementSelect(item.id, elementId);
    svgTextEditingElementId = elementId;
    beginEdit();
    tick().then(() => svgTextEditorEl?.focus());
  }

  function updateSvgTextElement(elementId: string, value: string) {
    previewEdit(moodBoardVectorGroupWithTextElement(item, elementId, value));
  }

  function commitSvgTextElement(elementId: string, value: string) {
    svgTextEditingElementId = "";
    commitEdit(moodBoardVectorGroupWithTextElement(item, elementId, value));
  }

  function handleSvgTextKeydown(event: KeyboardEvent, elementId: string) {
    if (event.key === "Enter") {
      event.preventDefault();
      commitSvgTextElement(elementId, (event.currentTarget as HTMLInputElement).value);
      (event.currentTarget as HTMLInputElement).blur();
    } else if (event.key === "Escape") {
      event.preventDefault();
      svgTextEditingElementId = "";
      cancelEdit();
      (event.currentTarget as HTMLInputElement).blur();
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<svg
  class="svg-group-preview"
  data-vector-selection={vectorNodeSelectionToken}
  viewBox={`${item.viewBoxX} ${item.viewBoxY} ${item.viewBoxWidth} ${item.viewBoxHeight}`}
  preserveAspectRatio="none"
  aria-label={item.title}
  onpointerdown={(event) => selectSvgElement(event, null)}
>
  {#each item.elements as element}
    {@const selectedElementNodes = vectorGroupElementEditActive(element.id) && element.type === "path" ? editableSvgElementNodes(element) : null}
    {@const selectedElementBounds = selectedElementNodes ? selectedElementTransformedBounds(selectedElementNodes.nodes, element.transform) : null}
    {@const selectedTransformBounds = selected && selectedSvgElementId === element.id && !vectorGroupElementEditActive(element.id) ? activeSvgTransformBounds(element.id) : null}
    {#if element.type === "text"}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <text
        class:selected-subpath={selected && selectedSvgElementId === element.id}
        x={element.x}
        y={element.y}
        fill={element.fill}
        opacity={element.opacity}
        font-family={element.fontFamily}
        font-size={element.fontSize}
        font-weight={element.fontWeight}
        transform={`matrix(${element.transform.join(" ")})`}
        onpointerdown={(event) => beginVectorGroupElementDrag(event, element.id)}
        ondblclick={(event) => beginSvgTextEdit(event, element.id)}
      >{element.text}</text>
      {#if selected && selectedSvgElementId === element.id && svgTextEditingElementId === element.id}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <foreignObject
          x={element.x}
          y={element.y - element.fontSize}
          width={svgTextEditWidth(element.text, element.fontSize)}
          height={element.fontSize * 1.35}
          transform={`matrix(${element.transform.join(" ")})`}
          onpointerdown={(event) => event.stopPropagation()}
        >
          <input
            bind:this={svgTextEditorEl}
            xmlns="http://www.w3.org/1999/xhtml"
            class="svg-text-editor"
            value={element.text}
            style={`font-size:${element.fontSize}px;font-family:${element.fontFamily};font-weight:${element.fontWeight};color:${element.fill};`}
            oninput={(event) => updateSvgTextElement(element.id, event.currentTarget.value)}
            onblur={(event) => commitSvgTextElement(element.id, event.currentTarget.value)}
            onkeydown={(event) => handleSvgTextKeydown(event, element.id)}
          />
        </foreignObject>
      {/if}
    {:else}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <path
        class:selected-subpath={selected && selectedSvgElementId === element.id}
        d={element.d}
        fill={element.fill}
        stroke={element.stroke}
        stroke-width={element.strokeWidth}
        opacity={element.opacity}
        transform={`matrix(${element.transform.join(" ")})`}
        vector-effect="non-scaling-stroke"
        onpointerdown={(event) => beginVectorGroupElementDrag(event, element.id)}
        ondblclick={(event) => handleVectorGroupPathDoubleClick(event, element.id)}
      />
    {/if}
    {#if selectedTransformBounds}
      {@const selectedCenter = vectorBoundsCenter(selectedTransformBounds)}
      {@const transformHandleRadius = 7}
      {@const transformHandleOffset = vectorGroupHandleOffset()}
      {@const rotateHandleY = selectedTransformBounds.minY - transformHandleOffset}
      <g class="vector-transform-overlay">
        <rect
          class="vector-transform-box"
          x={selectedTransformBounds.minX}
          y={selectedTransformBounds.minY}
          width={selectedTransformBounds.maxX - selectedTransformBounds.minX}
          height={selectedTransformBounds.maxY - selectedTransformBounds.minY}
        />
        <line
          class="vector-transform-line"
          x1={selectedCenter.x}
          y1={selectedTransformBounds.minY}
          x2={selectedCenter.x}
          y2={rotateHandleY}
        />
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <circle
          class="vector-transform-handle rotate"
          cx={selectedCenter.x}
          cy={rotateHandleY}
          r={transformHandleRadius}
          onpointerdown={(event) => beginVectorGroupElementTransformDrag(event, element.id, "rotate", selectedTransformBounds)}
        />
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <circle
          class="vector-transform-handle scale"
          cx={selectedTransformBounds.maxX}
          cy={selectedTransformBounds.maxY}
          r={transformHandleRadius}
          onpointerdown={(event) => beginVectorGroupElementTransformDrag(event, element.id, "scale", selectedTransformBounds)}
        />
      </g>
    {/if}
    {#if selectedElementNodes && selectedElementBounds}
      {@const nodeScope = vectorGroupNodeScope(element.id)}
      <g transform={`matrix(${element.transform.join(" ")})`}>
        {#each selectedElementNodes.nodes as node, index}
          {#if vectorNodeSelectionToken && node.in && vectorHandleVisible(nodeScope, selectedElementNodes.nodes, index, "in", selectedElementNodes.closed)}
            <line
              class="vector-handle-line"
              x1={node.x}
              y1={node.y}
              x2={node.x + node.in.x}
              y2={node.y + node.in.y}
            />
            <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
            <circle
              class="vector-handle"
              cx={node.x + node.in.x}
              cy={node.y + node.in.y}
              r="5"
              onpointerdown={(event) => beginVectorGroupDrag(event, element.id, index, "in")}
              onclick={(event) => handleVectorNodeClick(event, nodeScope, index)}
            />
          {/if}
          {#if vectorNodeSelectionToken && node.out && vectorHandleVisible(nodeScope, selectedElementNodes.nodes, index, "out", selectedElementNodes.closed)}
            <line
              class="vector-handle-line"
              x1={node.x}
              y1={node.y}
              x2={node.x + node.out.x}
              y2={node.y + node.out.y}
            />
            <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
            <circle
              class="vector-handle"
              cx={node.x + node.out.x}
              cy={node.y + node.out.y}
              r="5"
              onpointerdown={(event) => beginVectorGroupDrag(event, element.id, index, "out")}
              onclick={(event) => handleVectorNodeClick(event, nodeScope, index)}
            />
          {/if}
          <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
          <circle
            class="vector-node"
            class:selected-node={Boolean(vectorNodeSelectionToken) && isVectorNodeSelected(nodeScope, index)}
            class:mirrored-node={nodeHandleMode(node) === "mirrored"}
            class:locked-node={nodeHandleMode(node) === "locked"}
            class:independent-node={nodeHandleMode(node) === "independent"}
            cx={node.x}
            cy={node.y}
            r="5.5"
            onpointerdown={(event) => beginVectorGroupDrag(event, element.id, index, "node")}
            onclick={(event) => handleVectorNodeClick(event, nodeScope, index)}
          />
        {/each}
      </g>
    {/if}
  {/each}
</svg>

<style>
  .svg-group-preview {
    display: block;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    overflow: visible;
  }

  .vector-handle-line {
    stroke: color-mix(in srgb, var(--brand) 70%, #ffffff);
    stroke-width: 1.2;
    vector-effect: non-scaling-stroke;
  }

  .vector-node,
  .vector-handle {
    cursor: move;
    vector-effect: non-scaling-stroke;
  }

  .vector-node {
    fill: var(--surface);
    stroke: var(--brand);
    stroke-width: 2;
  }

  .vector-handle {
    fill: var(--brand);
    stroke: var(--surface);
    stroke-width: 1.5;
  }

  .vector-node:hover,
  .vector-handle:hover {
    fill: var(--brand-strong);
  }

  .vector-node.selected-node {
    fill: var(--brand);
    stroke: var(--surface);
    stroke-width: 2.4;
  }

  .vector-node.mirrored-node {
    stroke-dasharray: 0;
  }

  .vector-node.locked-node {
    stroke-dasharray: 3 2;
  }

  .vector-node.independent-node {
    stroke: color-mix(in srgb, var(--brand) 62%, var(--text-muted));
  }

  .vector-transform-overlay {
    pointer-events: none;
  }

  .vector-transform-box,
  .vector-transform-line {
    fill: none;
    stroke: color-mix(in srgb, var(--brand) 78%, #ffffff);
    stroke-width: 1.2;
    stroke-dasharray: 5 4;
    vector-effect: non-scaling-stroke;
    pointer-events: none;
  }

  .vector-transform-line {
    stroke-dasharray: 3 3;
  }

  .vector-transform-handle {
    fill: var(--surface);
    stroke: var(--brand);
    stroke-width: 2;
    vector-effect: non-scaling-stroke;
    pointer-events: auto;
  }

  .vector-transform-handle.scale {
    cursor: nwse-resize;
  }

  .vector-transform-handle.rotate {
    cursor: grab;
  }

  .vector-transform-handle:hover {
    fill: var(--brand-soft);
    stroke: var(--brand-strong);
  }

  .svg-group-preview path,
  .svg-group-preview text {
    cursor: pointer;
  }

  .svg-text-editor {
    width: 100%;
    height: 100%;
    padding: 0 3px;
    border: 1px solid var(--brand);
    border-radius: 3px;
    background: color-mix(in srgb, var(--surface) 88%, transparent);
    line-height: 1.15;
    outline: none;
    box-shadow: 0 0 0 2px var(--brand-soft);
  }

  .svg-group-preview path.selected-subpath,
  .svg-group-preview text.selected-subpath {
    filter: drop-shadow(0 0 2px var(--brand-strong));
  }
</style>
