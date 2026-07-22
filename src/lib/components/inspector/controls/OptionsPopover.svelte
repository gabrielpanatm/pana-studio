<script lang="ts">
  import { tick } from "svelte";

  let {
    anchor,
    options = [],
    onselect,
  }: {
    anchor: HTMLElement | null;
    options?: string[];
    onselect: (option: string) => void;
  } = $props();

  const VIEWPORT_MARGIN = 8;
  const GAP = 4;
  const MAX_HEIGHT = 240;
  const MIN_HEIGHT = 80;
  const OPTION_HEIGHT = 28;
  const POPOVER_PADDING = 8;

  let placement = $state({ left: 0, top: 0, width: 0, maxHeight: 180 });

  const popoverStyle = $derived(
    `left: ${placement.left}px; top: ${placement.top}px; width: ${placement.width}px; max-height: ${placement.maxHeight}px;`,
  );

  function clamp(value: number, min: number, max: number) {
    return Math.min(Math.max(value, min), max);
  }

  function updatePlacement() {
    if (!anchor) return;

    const anchorRect = anchor.getBoundingClientRect();
    const sectionBody   = anchor.closest(".section-body") as HTMLElement | null;
    const inspectorPane = anchor.closest(".inspector-pane") as HTMLElement | null;
    const horizontalTarget = sectionBody ?? inspectorPane ?? anchor;
    const targetRect = horizontalTarget.getBoundingClientRect();

    const viewportWidth  = window.innerWidth;
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
    const openAbove  = spaceBelow < 180 && spaceAbove > spaceBelow;
    const available  = Math.max(MIN_HEIGHT, openAbove ? spaceAbove : spaceBelow);
    const contentH   = options.length * OPTION_HEIGHT + POPOVER_PADDING;
    const maxHeight  = Math.min(MAX_HEIGHT, contentH, available - GAP);
    const top        = openAbove
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
    const pane = anchor.closest(".inspector-pane");
    pane?.addEventListener("scroll", updatePlacement, { passive: true });
    return () => pane?.removeEventListener("scroll", updatePlacement);
  });
</script>

<svelte:window onresize={updatePlacement} onscroll={updatePlacement} />

<div class="options-popover" role="listbox" aria-label="Opțiuni" style={popoverStyle}>
  {#each options as opt}
    <button
      type="button"
      class="options-item"
      onmousedown={(e) => e.preventDefault()}
      onclick={() => onselect(opt)}
    >{opt}</button>
  {/each}
</div>

<style>
  .options-popover {
    position: fixed;
    z-index: 1000;
    overflow: auto;
    padding: 4px;
    border: 1px solid var(--border-4);
    border-radius: 7px;
    background: var(--surface-2);
    box-shadow: 0 12px 30px rgba(0, 0, 0, 0.25);
  }

  .options-item {
    display: block;
    width: 100%;
    min-height: 28px;
    padding: 4px 8px;
    border: 0;
    border-radius: 5px;
    color: var(--text);
    background: transparent;
    text-align: left;
    cursor: pointer;
    font-size: 12px;
    font-family: "JetBrains Mono", "SFMono-Regular", Consolas, monospace;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .options-item:hover,
  .options-item:focus {
    background: var(--brand-soft);
    color: var(--brand-strong);
    outline: none;
  }
</style>
