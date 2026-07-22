<script lang="ts">
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
  <button type="button" class="section-header" onclick={() => (collapsed = !collapsed)}>
    <span class="section-icon">
      {#if icon}
        {@render icon()}
      {/if}
    </span>
    <span class="section-title">{title}</span>
    {#if hasValues}
      <span class="section-dot"></span>
    {/if}
    <span class="chevron" class:rotated={collapsed}>›</span>
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
    border-bottom: 1px solid var(--border);
  }

  .section:focus-within {
    z-index: 40;
  }

  .section-header {
    display: flex;
    align-items: center;
    gap: 5px;
    width: 100%;
    padding: 5px 9px;
    border: none;
    background: transparent;
    cursor: pointer;
    text-align: left;
  }

  .section-header:hover {
    background: var(--surface-4);
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
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .section-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--brand);
    flex-shrink: 0;
  }

  .chevron {
    color: var(--text-muted);
    font-size: 14px;
    transform: rotate(90deg);
    transition: transform 0.15s;
    display: inline-block;
    line-height: 1;
  }

  .chevron.rotated {
    transform: rotate(0deg);
  }

  .section-body {
    padding: 5px 9px 9px;
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
    overflow: visible;
  }
</style>
