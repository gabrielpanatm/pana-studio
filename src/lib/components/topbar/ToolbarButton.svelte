<script lang="ts">
  export let title = "";
  export let active = false;
  export let disabled = false;
  export let pending = false;
  export let cta = false;
  export let segmented = false;
  export let onclick: (event: MouseEvent) => void | Promise<unknown> = () => {};
</script>

<button
  class="toolbar-icon-button"
  class:active
  class:pending
  class:open-folder-cta={cta}
  class:segmented
  type="button"
  {title}
  {disabled}
  onclick={onclick}
>
  <slot></slot>
  {#if pending}<span class="save-pending-dot"></span>{/if}
</button>

<style>
  .toolbar-icon-button {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    min-width: 32px;
    min-height: 30px;
    padding: 0;
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-control);
    color: var(--text);
    line-height: 0;
    background: var(--surface-4);
    transition: background 120ms ease, border-color 120ms ease, color 120ms ease, opacity 120ms ease;
  }

  .toolbar-icon-button:hover:not(:disabled) {
    border-color: var(--border-strong);
    color: var(--text-strong);
    background: var(--control-hover);
  }

  .toolbar-icon-button.segmented {
    width: 30px;
    min-width: 30px;
    height: 28px;
    min-height: 28px;
    border-color: transparent;
    border-radius: 3px;
    background: transparent;
  }

  .toolbar-icon-button.segmented + :global(.toolbar-icon-button.segmented) {
    margin-left: 1px;
  }

  .toolbar-icon-button.active {
    border-color: transparent;
    color: var(--brand-strong);
    background: var(--control-selected);
  }

  .toolbar-icon-button:disabled {
    opacity: 0.38;
    cursor: not-allowed;
    background: color-mix(in srgb, var(--surface-4) 55%, transparent);
  }

  .toolbar-icon-button.segmented:disabled {
    background: transparent;
  }

  .toolbar-icon-button.open-folder-cta {
    border-color: var(--brand);
    color: #ffffff;
    background: var(--brand);
  }

  .toolbar-icon-button.open-folder-cta:hover {
    border-color: var(--brand-strong);
    background: var(--brand-strong);
  }

  .toolbar-icon-button.pending {
    border-color: var(--warning);
    color: var(--warning);
  }

  .save-pending-dot {
    position: absolute;
    top: 4px;
    right: 4px;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--warning);
  }

  .toolbar-icon-button :global(svg) {
    display: block;
    flex: 0 0 auto;
  }
</style>
