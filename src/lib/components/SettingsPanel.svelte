<script lang="ts">
  import { IconChevronRight, IconRocket, IconSettings, IconX } from "@tabler/icons-svelte";
  import AiIntegrationPane from "$lib/components/settings/AiIntegrationPane.svelte";
  import type { AiContextStatus } from "$lib/types";
  import { UI_TERMS } from "$lib/i18n/ui-terms";

  let {
    open = false,
    aiContextStatus = null,
    onStatusUpdate = undefined as ((text: string, kind: string) => void) | undefined,
    openPublishCenter,
    close,
  }: {
    open?: boolean;
    aiContextStatus?: AiContextStatus | null;
    onStatusUpdate?: (text: string, kind: string) => void;
    openPublishCenter: () => void | Promise<void>;
    close: () => void;
  } = $props();
</script>

{#if open}
  <div class="settings-backdrop" role="presentation" onclick={close}></div>
  <aside class="settings-panel" aria-label="Setări proiect">
    <header class="settings-header">
      <div>
        <p class="eyebrow">{UI_TERMS.settings}</p>
        <h2>Pană Studio</h2>
      </div>
      <button type="button" class="icon-button" title="Închide" onclick={close}>
        <IconX size={16} stroke={1.9} />
      </button>
    </header>

    <section class="settings-section" aria-label="Construire și publicare">
      <div class="section-heading">
        <IconRocket size={15} stroke={1.8} />
        <h3>Construire și publicare</h3>
      </div>
      <button
        type="button"
        class="settings-route"
        onclick={() => { close(); void openPublishCenter(); }}
      >
        <span><strong>Deschide centrul de publicare</strong><small>Configurație Zola, optimizare, verificare, construire și livrare.</small></span>
        <IconChevronRight size={16} stroke={1.9} />
      </button>
    </section>

    <section class="settings-section" aria-label="AI și MCP">
      <div class="section-heading">
        <IconSettings size={15} stroke={1.8} />
        <h3>AI / MCP</h3>
      </div>
      <AiIntegrationPane
        status={aiContextStatus}
        {onStatusUpdate}
      />
    </section>
  </aside>
{/if}

<style>
  .settings-backdrop {
    position: fixed;
    inset: 0;
    z-index: 39;
    background: rgba(13, 18, 16, 0.18);
  }

  .settings-panel {
    position: fixed;
    top: 0;
    left: 0;
    z-index: 40;
    display: flex;
    flex-direction: column;
    gap: 12px;
    width: min(380px, calc(100vw - 24px));
    height: 100vh;
    padding: 12px;
    border-right: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
    box-shadow: 18px 0 42px rgba(0, 0, 0, 0.22);
    overflow-y: auto;
  }

  .settings-header,
  .section-heading {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .settings-header {
    justify-content: space-between;
  }

  .eyebrow,
  .settings-header h2,
  .section-heading h3 {
    margin: 0;
  }

  .eyebrow {
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 800;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .settings-header h2 {
    margin-top: 2px;
    font-size: 18px;
    font-weight: 850;
  }

  .icon-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border: 1px solid var(--border-3);
    border-radius: 7px;
    background: var(--surface-4);
    color: var(--text-muted);
    cursor: pointer;
  }

  .icon-button:hover {
    color: var(--text);
    border-color: var(--border-4);
  }

  .settings-section {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }

  .settings-route {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    width: 100%;
    min-height: 62px;
    padding: 10px;
    border: 1px solid var(--wb-border-subtle, var(--border-2));
    border-radius: 8px;
    color: var(--wb-text-primary, var(--text));
    background: var(--wb-surface-chrome, var(--surface-2));
    text-align: left;
    cursor: pointer;
  }

  .settings-route:hover { border-color: var(--wb-accent, var(--brand)); }
  .settings-route:focus-visible { outline: 2px solid var(--wb-focus-ring, var(--brand-strong)); outline-offset: 1px; }
  .settings-route span { display: grid; gap: 4px; }
  .settings-route strong { color: var(--text-strong); font-size: 12px; }
  .settings-route small { color: var(--text-muted); font-size: 12px; line-height: 1.4; }

  .section-heading {
    color: var(--text-muted);
  }

  .section-heading h3 {
    font-size: 12px;
    font-weight: 900;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
</style>
