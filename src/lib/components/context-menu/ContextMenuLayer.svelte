<script lang="ts">
  import { tick } from "svelte";
  import type { ContextMenuItem, ContextMenuState } from "$lib/context-menu/store.svelte";

  let { state: menuState }: { state: ContextMenuState } = $props();

  let menuEl = $state<HTMLDivElement | null>(null);
  let left = $state(0);
  let top = $state(0);

  function clampToViewport() {
    if (!menuEl || !menuState.current) return;
    const rect = menuEl.getBoundingClientRect();
    const padding = 8;
    left = Math.max(padding, Math.min(menuState.current.x, window.innerWidth - rect.width - padding));
    top = Math.max(padding, Math.min(menuState.current.y, window.innerHeight - rect.height - padding));
  }

  $effect(() => {
    const current = menuState.current;
    if (!current) return;
    left = current.x;
    top = current.y;
    void tick().then(clampToViewport);
  });

  function handleWindowPointerDown(event: PointerEvent) {
    if (!menuState.current) return;
    if (menuEl && event.target instanceof Node && menuEl.contains(event.target)) return;
    menuState.close();
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    if (!menuState.current) return;
    if (event.key === "Escape") {
      event.preventDefault();
      menuState.close();
    }
  }

  function runItem(item: ContextMenuItem) {
    void menuState.run(item);
  }
</script>

<svelte:window
  onpointerdown={handleWindowPointerDown}
  onkeydown={handleWindowKeydown}
  onresize={clampToViewport}
/>

{#if menuState.current}
  <div
    bind:this={menuEl}
    class="context-menu"
    style={`left: ${left}px; top: ${top}px;`}
    role="menu"
    aria-label={menuState.current.title ?? "Meniu contextual"}
  >
    {#if menuState.current.title || menuState.current.subtitle}
      <div class="context-menu-heading">
        {#if menuState.current.title}<strong>{menuState.current.title}</strong>{/if}
        {#if menuState.current.subtitle}<span>{menuState.current.subtitle}</span>{/if}
      </div>
    {/if}

    <div class="context-menu-items">
      {#each menuState.current.items as item}
        {#if item.separatorBefore}
          <span class="context-menu-separator" aria-hidden="true"></span>
        {/if}
        <button
          class:danger={item.tone === "danger"}
          type="button"
          role="menuitem"
          disabled={item.disabled}
          onclick={() => runItem(item)}
        >
          <span>{item.label}</span>
          {#if item.shortcut}<kbd>{item.shortcut}</kbd>{/if}
        </button>
      {/each}
    </div>
  </div>
{/if}

<style>
  .context-menu {
    position: fixed;
    z-index: 10000;
    min-width: 190px;
    max-width: min(320px, calc(100vw - 16px));
    padding: 5px;
    border: 1px solid var(--border-4);
    border-radius: 8px;
    background: var(--surface);
    color: var(--text);
    box-shadow: 0 18px 42px rgba(15, 23, 42, 0.22);
  }

  .context-menu-heading {
    display: grid;
    gap: 2px;
    padding: 6px 8px 7px;
    border-bottom: 1px solid var(--border-2);
    margin-bottom: 4px;
  }

  .context-menu-heading strong {
    min-width: 0;
    overflow: hidden;
    color: var(--text-strong);
    font-size: 12px;
    font-weight: 850;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .context-menu-heading span {
    min-width: 0;
    overflow: hidden;
    color: var(--text-muted);
    font-size: 12px;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .context-menu-items {
    display: grid;
    gap: 2px;
  }

  .context-menu button {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    width: 100%;
    min-height: 28px;
    padding: 0 8px;
    border: 0;
    border-radius: 6px;
    color: var(--text);
    font: inherit;
    font-size: 12px;
    text-align: left;
    background: transparent;
    cursor: pointer;
  }

  .context-menu button:hover:not(:disabled),
  .context-menu button:focus-visible:not(:disabled) {
    color: var(--text-strong);
    background: var(--surface-4);
    outline: none;
  }

  .context-menu button.danger {
    color: #b91c1c;
  }

  .context-menu button.danger:hover:not(:disabled),
  .context-menu button.danger:focus-visible:not(:disabled) {
    background: color-mix(in srgb, #ef4444 11%, var(--surface-4));
  }

  .context-menu button:disabled {
    color: var(--text-muted);
    opacity: 0.55;
    cursor: not-allowed;
  }

  .context-menu kbd {
    color: var(--text-muted);
    font-family: "JetBrains Mono", "SFMono-Regular", Consolas, monospace;
    font-size: 12px;
    font-weight: 700;
  }

  .context-menu-separator {
    display: block;
    height: 1px;
    margin: 4px 3px;
    background: var(--border-2);
  }
</style>
