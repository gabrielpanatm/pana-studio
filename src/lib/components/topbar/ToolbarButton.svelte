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
  aria-pressed={segmented ? active : undefined}
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
    background: var(--material-control);
    box-shadow: var(--shadow-control);
    transition:
      background 120ms ease,
      border-color 120ms ease,
      box-shadow 120ms ease,
      color 120ms ease,
      transform 80ms ease,
      opacity 120ms ease;
  }

  .toolbar-icon-button:hover:not(:disabled) {
    border-color: var(--border-strong);
    color: var(--text-strong);
    background: var(--material-control-hover);
    box-shadow: var(--shadow-control-hover);
  }

  .toolbar-icon-button:active:not(:disabled) {
    background: var(--material-control-selected);
    box-shadow: var(--shadow-pressed);
    transform: translateY(1px);
  }

  .toolbar-icon-button.segmented {
    width: 30px;
    min-width: 30px;
    height: 28px;
    min-height: 28px;
    border: 0;
    border-radius: 0;
    color: var(--text-muted);
    background: transparent;
    box-shadow: none;
  }

  .toolbar-icon-button.segmented + :global(.toolbar-icon-button.segmented) {
    margin-left: 0;
  }

  .toolbar-icon-button.active {
    border-color: color-mix(in srgb, var(--brand) 42%, var(--border-subtle));
    color: var(--brand-strong);
    background: var(--material-control-selected);
    box-shadow: var(--shadow-pressed);
  }

  .toolbar-icon-button.segmented:hover:not(:disabled):not(.active) {
    border-color: transparent;
    color: var(--text-strong);
    background: var(--material-control-hover);
    box-shadow: inset 0 1px 0 var(--skeuo-edge-highlight);
  }

  .toolbar-icon-button.segmented.active {
    border-color: transparent;
    color: var(--brand-strong);
    background: var(--material-control-selected);
    box-shadow:
      inset 0 1px 2px var(--skeuo-shade-soft),
      inset 0 -1px 0 var(--skeuo-edge-highlight);
  }

  .toolbar-icon-button.segmented.active:hover:not(:disabled) {
    color: var(--brand-strong);
    background: var(--material-control-selected);
    box-shadow:
      inset 0 1px 2px var(--skeuo-shade-soft),
      inset 0 -1px 0 var(--skeuo-edge-highlight);
  }

  .toolbar-icon-button.segmented:active:not(:disabled) {
    background: color-mix(in srgb, var(--brand) 16%, var(--surface-inset));
    box-shadow: var(--shadow-pressed);
    transform: none;
  }

  .toolbar-icon-button.segmented:focus-visible {
    outline-offset: -2px;
  }

  .toolbar-icon-button:disabled {
    opacity: 0.38;
    cursor: not-allowed;
    background: color-mix(in srgb, var(--surface-4) 55%, transparent);
    box-shadow: none;
  }

  .toolbar-icon-button.segmented:disabled {
    background: transparent;
  }

  .toolbar-icon-button.open-folder-cta {
    border-color: var(--brand);
    color: var(--text-on-accent);
    background: var(--material-accent);
    box-shadow: var(--shadow-control);
  }

  .toolbar-icon-button.open-folder-cta:hover {
    border-color: var(--brand-strong);
    background: linear-gradient(
      180deg,
      color-mix(in srgb, var(--brand-strong) 86%, white),
      var(--brand-strong)
    );
    box-shadow: var(--shadow-control-hover);
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
