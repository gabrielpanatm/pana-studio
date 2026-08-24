<script lang="ts">
  import { tick } from "svelte";
  import { t } from "$lib/i18n/runtime.svelte";
  import {
    calculateAnchoredPopoverPlacement,
    observeAnchoredPopoverPosition,
  } from "$lib/ui/anchored-popover";

  let {
    anchor,
    options = [],
    onselect,
  }: {
    anchor: HTMLElement | null;
    options?: string[];
    onselect: (option: string) => void;
  } = $props();

  const OPTION_HEIGHT = 28;

  let placement = $state({ left: 0, top: 0, width: 0, maxHeight: 180 });

  const popoverStyle = $derived(
    `left: ${placement.left}px; top: ${placement.top}px; width: ${placement.width}px; max-height: ${placement.maxHeight}px;`,
  );

  function updatePlacement() {
    if (!anchor) return;

    const anchorRect = anchor.getBoundingClientRect();
    const sectionBody   = anchor.closest(".section-body") as HTMLElement | null;
    const inspectorPane = anchor.closest(".inspector-pane") as HTMLElement | null;
    const horizontalTarget = sectionBody ?? inspectorPane ?? anchor;
    const targetRect = horizontalTarget.getBoundingClientRect();

    placement = calculateAnchoredPopoverPlacement({
      anchorRect,
      scopeRect: targetRect,
      itemCount: options.length,
      itemHeight: OPTION_HEIGHT,
    });
  }

  $effect(() => {
    if (!anchor) return;
    tick().then(updatePlacement);
  });

  $effect(() => {
    if (!anchor) return;
    return observeAnchoredPopoverPosition(anchor, updatePlacement);
  });
</script>

<div class="options-popover ui-popover" role="listbox" aria-label={t("inspector-options")} style={popoverStyle}>
  {#each options as opt}
    <button
      type="button"
      class="options-item ui-option"
      onmousedown={(e) => e.preventDefault()}
      onclick={() => onselect(opt)}
    >{opt}</button>
  {/each}
</div>

<style>
  .options-item {
    display: block;
    font-family: var(--font-mono);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
