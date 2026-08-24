<script lang="ts">
  import { tick } from "svelte";
  import type { CssPropertySuggestion } from "$lib/css/contracts";
  import { t } from "$lib/i18n/runtime.svelte";
  import {
    calculateAnchoredPopoverPlacement,
    observeAnchoredPopoverPosition,
  } from "$lib/ui/anchored-popover";

  let {
    anchor,
    suggestions = [],
    onselect,
  }: {
    anchor: HTMLElement | null;
    suggestions?: CssPropertySuggestion[];
    onselect: (variable: CssPropertySuggestion) => void;
  } = $props();

  const OPTION_HEIGHT = 26;

  let placement = $state({
    left: 0,
    top: 0,
    width: 0,
    maxHeight: 180,
  });

  const popoverStyle = $derived(
    `left: ${placement.left}px; top: ${placement.top}px; width: ${placement.width}px; max-height: ${placement.maxHeight}px;`,
  );

  function updatePlacement() {
    if (!anchor) return;

    const anchorRect = anchor.getBoundingClientRect();
    const sectionBody = anchor.closest(".section-body") as HTMLElement | null;
    const inspectorPane = anchor.closest(".inspector-pane") as HTMLElement | null;
    const horizontalTarget = sectionBody ?? inspectorPane ?? anchor;
    const targetRect = horizontalTarget.getBoundingClientRect();

    placement = calculateAnchoredPopoverPlacement({
      anchorRect,
      scopeRect: targetRect,
      itemCount: suggestions.length,
      itemHeight: OPTION_HEIGHT,
      minHeight: 96,
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

<div class="suggestion-popover ui-popover" role="listbox" aria-label={t("inspector-scss-variables")} style={popoverStyle}>
  {#each suggestions as variable}
    <button
      type="button"
      class="suggestion-option ui-option"
      onmousedown={(event) => event.preventDefault()}
      onclick={() => onselect(variable)}
    >
      <span class="suggestion-name">{variable.directValue ? variable.name : `$${variable.name}`}</span>
      <span class="suggestion-value">{variable.value}</span>
    </button>
  {/each}
</div>

<style>
  .suggestion-option {
    display: grid;
    grid-template-columns: minmax(0, 0.9fr) minmax(0, 1.1fr);
    gap: 8px;
    min-height: 26px;
    padding: 4px 7px;
  }

  .suggestion-name,
  .suggestion-value {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    font-family: var(--font-mono);
  }

  .suggestion-name {
    color: var(--brand-strong);
    font-weight: 700;
  }

  .suggestion-value {
    color: var(--text-muted);
  }
</style>
