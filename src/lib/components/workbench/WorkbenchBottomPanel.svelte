<script lang="ts">
  import type { Component } from "svelte";
  import type { TerminalPaneProps } from "$lib/components/TerminalPane.svelte";
  import type { AppState } from "$lib/state/app.svelte";
  import type { TerminalQuickTask } from "$lib/terminal/runtime";
  import { t } from "$lib/i18n/runtime.svelte";

  let {
    app,
    TerminalPaneComponent = null,
  }: {
    app: AppState;
    TerminalPaneComponent?: Component<TerminalPaneProps> | null;
  } = $props();

  async function closePanel() {
    await app.setWorkbenchBottomPanel(false, "terminal");
  }
</script>

{#if TerminalPaneComponent}
  <TerminalPaneComponent
    bind:terminalHost={app.terminalHost}
    terminalTabs={app.terminalTabs}
    activeTerminalTabId={app.activeTerminalTabId}
    quickTasks={app.terminalQuickTasks}
    openTab={() => app.openTerminalTab()}
    selectTab={(id: string) => app.selectTerminalTab(id)}
    closeTab={(id: string) => app.closeTerminalTab(id)}
    runQuickTask={(task: TerminalQuickTask) => app.runTerminalQuickTask(task)}
    clearActiveTerminal={() => app.clearActiveTerminal()}
    closePane={closePanel}
  />
{:else}
  <section class="terminal-loading" aria-live="polite">
    <strong>{t("workbench-terminal")}</strong>
    <span>{t("workbench-terminal-loading")}</span>
  </section>
{/if}

<style>
  .terminal-loading {
    display: grid;
    min-width: 0;
    min-height: 0;
    place-content: center;
    gap: 4px;
    border: 1px solid var(--wb-border-subtle, var(--border));
    border-radius: var(--radius-panel);
    color: var(--wb-text-muted, var(--text-muted));
    background: var(--wb-surface-document, var(--surface));
    text-align: center;
    font-size: 12px;
  }

  .terminal-loading strong {
    color: var(--wb-text-primary, var(--text));
  }
</style>
