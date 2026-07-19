<script lang="ts">
  import { tick } from "svelte";
  import type { ScssVariable } from "$lib/types";

  let {
    anchor,
    suggestions = [],
    onselect,
  }: {
    anchor: HTMLElement | null;
    suggestions?: ScssVariable[];
    onselect: (variable: ScssVariable) => void;
  } = $props();

  const VIEWPORT_MARGIN = 8;
  const GAP = 4;
  const MAX_HEIGHT = 240;
  const MIN_HEIGHT = 96;
  const OPTION_HEIGHT = 26;
  const POPOVER_PADDING = 8;

  let placement = $state({
    left: 0,
    top: 0,
    width: 0,
    maxHeight: 180,
  });

  const popoverStyle = $derived(
    `left: ${placement.left}px; top: ${placement.top}px; width: ${placement.width}px; max-height: ${placement.maxHeight}px;`,
  );

  function clamp(value: number, min: number, max: number) {
    return Math.min(Math.max(value, min), max);
  }

  function updatePlacement() {
    if (!anchor) return;

    const anchorRect = anchor.getBoundingClientRect();
    const sectionBody = anchor.closest(".section-body") as HTMLElement | null;
    const inspectorPane = anchor.closest(".inspector-pane") as HTMLElement | null;
    const horizontalTarget = sectionBody ?? inspectorPane ?? anchor;
    const targetRect = horizontalTarget.getBoundingClientRect();

    const viewportWidth = window.innerWidth;
    const viewportHeight = window.innerHeight;
    const width = Math.max(
      anchorRect.width,
      Math.min(targetRect.width, viewportWidth - VIEWPORT_MARGIN * 2),
    );
    const left = clamp(
      targetRect.left,
      VIEWPORT_MARGIN,
      Math.max(VIEWPORT_MARGIN, viewportWidth - width - VIEWPORT_MARGIN),
    );

    const spaceBelow = viewportHeight - anchorRect.bottom - VIEWPORT_MARGIN;
    const spaceAbove = anchorRect.top - VIEWPORT_MARGIN;
    const openAbove = spaceBelow < 180 && spaceAbove > spaceBelow;
    const availableSpace = Math.max(MIN_HEIGHT, openAbove ? spaceAbove : spaceBelow);
    const contentHeight = suggestions.length * OPTION_HEIGHT + POPOVER_PADDING;
    const maxHeight = Math.min(MAX_HEIGHT, contentHeight, availableSpace - GAP);
    const top = openAbove
      ? Math.max(VIEWPORT_MARGIN, anchorRect.top - GAP - maxHeight)
      : Math.min(anchorRect.bottom + GAP, viewportHeight - VIEWPORT_MARGIN - maxHeight);

    placement = { left, top, width, maxHeight };
  }

  $effect(() => {
    if (!anchor) return;
    tick().then(updatePlacement);
  });

  $effect(() => {
    if (!anchor) return;
    const inspectorPane = anchor.closest(".inspector-pane");
    inspectorPane?.addEventListener("scroll", updatePlacement, { passive: true });

    return () => {
      inspectorPane?.removeEventListener("scroll", updatePlacement);
    };
  });
</script>

<svelte:window onresize={updatePlacement} onscroll={updatePlacement} />

<div class="suggestion-popover" role="listbox" aria-label="Variabile SCSS" style={popoverStyle}>
  {#each suggestions as variable}
    <button
      type="button"
      class="suggestion-option"
      onmousedown={(event) => event.preventDefault()}
      onclick={() => onselect(variable)}
    >
      <span class="suggestion-name">${variable.name}</span>
      <span class="suggestion-value">{variable.value}</span>
    </button>
  {/each}
</div>

<style>
  .suggestion-popover {
    position: fixed;
    z-index: 1000;
    overflow: auto;
    padding: 4px;
    border: 1px solid var(--border-4);
    border-radius: 7px;
    background: var(--surface-2);
    box-shadow: 0 12px 30px rgba(0, 0, 0, 0.25);
  }

  .suggestion-option {
    display: grid;
    grid-template-columns: minmax(0, 0.9fr) minmax(0, 1.1fr);
    gap: 8px;
    width: 100%;
    min-height: 26px;
    padding: 4px 7px;
    border: 0;
    border-radius: 5px;
    color: var(--text);
    background: transparent;
    text-align: left;
    cursor: pointer;
  }

  .suggestion-option:hover,
  .suggestion-option:focus {
    background: var(--brand-soft);
    outline: none;
  }

  .suggestion-name,
  .suggestion-value {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 10px;
    font-family: "JetBrains Mono", "SFMono-Regular", Consolas, monospace;
  }

  .suggestion-name {
    color: var(--brand-strong);
    font-weight: 700;
  }

  .suggestion-value {
    color: var(--text-muted);
  }
</style>
