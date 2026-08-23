<script lang="ts">
  import type { Component } from "svelte";
  import type { TerminalPaneProps } from "$lib/components/TerminalPane.svelte";
  import type { TerminalWorkspaceState } from "$lib/terminal/workspace.svelte";
  import type { TerminalQuickTask } from "$lib/terminal/runtime";
  import { t } from "$lib/i18n/runtime.svelte";

  let {
    workspace,
    TerminalPaneComponent = null,
  }: {
    workspace: TerminalWorkspaceState;
    TerminalPaneComponent?: Component<TerminalPaneProps> | null;
  } = $props();

  async function closePanel() {
    await workspace.closePane();
  }
</script>

{#if TerminalPaneComponent}
  <TerminalPaneComponent
    bind:terminalHost={workspace.terminalHost}
    terminalTabs={workspace.terminalTabs}
    activeTerminalTabId={workspace.activeTerminalTabId}
    quickTasks={workspace.terminalQuickTasks}
    openTab={() => workspace.openTab()}
    selectTab={(id: string) => workspace.selectTab(id)}
    closeTab={(id: string) => workspace.closeTab(id)}
    runQuickTask={(task: TerminalQuickTask) => workspace.runQuickTask(task)}
    clearActiveTerminal={() => workspace.clearActive()}
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
