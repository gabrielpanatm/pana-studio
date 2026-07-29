<script lang="ts">
  import { IconChevronDown } from "@tabler/icons-svelte";
  import type { Snippet } from "svelte";

  let {
    icon,
    title,
    hasValues = false,
    children,
  }: {
    icon?: Snippet;
    title: string;
    hasValues?: boolean;
    children: Snippet;
  } = $props();

  let collapsed = $state(false);
  let previousHasValues = $state(false);

  $effect(() => {
    if (hasValues && !previousHasValues) {
      collapsed = false;
    }
    previousHasValues = hasValues;
  });
</script>

<div class="section">
  <button
    type="button"
    class="section-header"
    aria-expanded={!collapsed}
    onclick={() => (collapsed = !collapsed)}
  >
    <span class="section-icon">
      {#if icon}
        {@render icon()}
      {/if}
    </span>
    <span class="section-title">{title}</span>
    {#if hasValues}
      <span class="section-dot"></span>
    {/if}
    <IconChevronDown class={collapsed ? "chevron collapsed" : "chevron"} size={14} stroke={1.8} aria-hidden="true" />
  </button>
  {#if !collapsed}
    <div class="section-body">
      {@render children()}
    </div>
  {/if}
</div>

<style>
  .section {
    position: relative;
    border-bottom: 1px solid var(--border-subtle);
  }

  .section:focus-within {
    z-index: 40;
  }

  .section-header {
    display: flex;
    align-items: center;
    gap: 5px;
    width: 100%;
    min-height: 32px;
    padding: 4px 10px;
    border: none;
    background: transparent;
    cursor: pointer;
    text-align: left;
    transition:
      color 120ms ease,
      background 120ms ease,
      box-shadow 120ms ease;
  }

  .section-header:hover {
    background: color-mix(in srgb, var(--control-hover) 72%, transparent);
    box-shadow: inset 0 1px 0 color-mix(in srgb, var(--skeuo-edge-highlight) 72%, transparent);
  }

  .section-header:active {
    background: color-mix(in srgb, var(--surface-inset) 36%, transparent);
    box-shadow: inset 0 1px 2px color-mix(in srgb, var(--skeuo-shade) 42%, transparent);
  }

  .section-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    width: 14px;
    flex-shrink: 0;
  }

  .section-title {
    flex: 1;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0;
    color: var(--text);
  }

  .section-dot {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: var(--brand);
    flex-shrink: 0;
  }

  :global(.chevron) {
    color: var(--text-muted);
    flex: 0 0 auto;
    transition: transform 140ms ease;
  }

  :global(.chevron.collapsed) {
    transform: rotate(-90deg);
  }

  .section-body {
    padding: 6px 10px 10px;
    display: flex;
    flex-direction: column;
    gap: 7px;
    min-width: 0;
    overflow: visible;
  }
</style>
