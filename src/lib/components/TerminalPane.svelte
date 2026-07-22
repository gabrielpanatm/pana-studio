<script lang="ts">
  import "@xterm/xterm/css/xterm.css";
  import { IconBolt, IconChevronDown, IconEraser, IconPlus, IconX } from "@tabler/icons-svelte";
  import type { TerminalQuickTask, TerminalTab } from "$lib/terminal/runtime";

  export type TerminalPaneProps = {
    terminalTabs: TerminalTab[];
    activeTerminalTabId: string;
    quickTasks: TerminalQuickTask[];
    openTab: () => void;
    selectTab: (tabId: string) => void;
    closeTab: (tabId: string) => void;
    runQuickTask: (task: TerminalQuickTask) => void | Promise<void>;
    clearActiveTerminal: () => void | Promise<void>;
    closePane: () => void | Promise<void>;
    terminalHost?: HTMLDivElement;
  };

  let {
    terminalTabs,
    activeTerminalTabId,
    quickTasks,
    openTab,
    selectTab,
    closeTab,
    runQuickTask,
    clearActiveTerminal,
    closePane,
    terminalHost = $bindable(),
  }: TerminalPaneProps = $props();
</script>

<section class="terminal-pane" aria-label="Terminal integrat">
  <div class="terminal-toolbar">
    <div class="terminal-tab-strip" role="tablist" aria-label="Terminal tabs">
      {#each terminalTabs as tab}
        <div class:active={activeTerminalTabId === tab.id} class="terminal-tab">
          <button
            type="button"
            role="tab"
            aria-selected={activeTerminalTabId === tab.id ? "true" : "false"}
            tabindex={activeTerminalTabId === tab.id ? 0 : -1}
            title={tab.description}
            onclick={() => { void selectTab(tab.id); }}
          >
            <span>{tab.title}</span>
          </button>
          <button class="terminal-tab-close" type="button" title={`Inchide ${tab.title}`} onclick={() => closeTab(tab.id)}>
            <IconX size={13} stroke={2.2} />
          </button>
        </div>
      {/each}
    </div>

    <div class="terminal-actions" aria-label="Terminal actions">
      {#each quickTasks as task}
        <button class="terminal-task-button" type="button" title={task.title} onclick={() => runQuickTask(task)}>
          <IconBolt size={13} stroke={2} />
          <span>{task.label}</span>
        </button>
      {/each}
      <button class="terminal-icon-button" type="button" title="Curăță terminalul activ" onclick={clearActiveTerminal}>
        <IconEraser size={14} stroke={2} />
      </button>
      <button class="terminal-add-button" type="button" title="Tab nou terminal" onclick={openTab}>
        <IconPlus size={14} stroke={2} />
        <span>Filă nouă</span>
      </button>
      <button
        class="terminal-icon-button"
        type="button"
        title="Ascunde terminalul (Ctrl+`)"
        aria-label="Ascunde terminalul"
        onclick={() => { void closePane(); }}
      >
        <IconChevronDown size={15} stroke={2} />
      </button>
    </div>
  </div>

  <div class="terminal-body">
    <div bind:this={terminalHost} class="terminal-host" aria-label="Terminal shell activ"></div>
  </div>
</section>

<style>
  .terminal-pane {
    position: relative;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    min-height: 0;
    border: 1px solid var(--border);
    border-radius: 10px;
    overflow: hidden;
    box-shadow: var(--shadow);
    background: var(--surface-7);
  }

  .terminal-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    min-width: 0;
    padding: 8px 9px 0;
  }

  .terminal-actions {
    display: inline-flex;
    flex: 0 0 auto;
    gap: 6px;
    justify-content: flex-end;
  }

  .terminal-add-button,
  .terminal-task-button,
  .terminal-icon-button {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    min-height: 26px;
    padding: 0 9px;
    border: 1px solid var(--border-3);
    border-radius: 999px;
    font-size: 12px;
    font-weight: 700;
    background: var(--surface-5);
  }

  .terminal-task-button {
    color: var(--brand);
  }

  .terminal-icon-button {
    justify-content: center;
    width: 28px;
    min-width: 28px;
    padding: 0;
    color: var(--text-muted);
  }

  .terminal-add-button:hover,
  .terminal-task-button:hover,
  .terminal-icon-button:hover {
    border-color: var(--brand);
    color: var(--text-strong);
  }

  .terminal-tab-strip {
    display: flex;
    align-items: center;
    flex: 1 1 auto;
    gap: 6px;
    min-height: 0;
    min-width: 0;
    padding: 0;
    overflow: auto hidden;
    overscroll-behavior-x: contain;
  }

  .terminal-tab {
    display: inline-flex;
    align-items: center;
    flex: 0 0 auto;
    height: 32px;
    min-width: 0;
    max-width: 220px;
    border: 1px solid var(--border-3);
    border-radius: 8px;
    background: var(--surface-5);
  }

  .terminal-tab.active {
    border-color: var(--brand);
    background: color-mix(in srgb, var(--brand-soft) 82%, var(--surface-4));
  }

  .terminal-tab > button {
    display: inline-flex;
    align-items: center;
    height: 30px;
    border: 0;
    background: transparent;
  }

  .terminal-tab > button:first-child {
    flex: 1 1 auto;
    min-width: 0;
    padding: 0 8px 0 10px;
    overflow: hidden;
    color: var(--text);
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: 12px;
    font-weight: 700;
  }

  .terminal-tab.active > button:first-child {
    color: var(--text-strong);
  }

  .terminal-tab-close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    min-width: 26px;
    padding: 0;
    border-left: 1px solid color-mix(in srgb, var(--border-3) 78%, transparent);
    color: var(--text-muted);
  }

  .terminal-tab-close:hover {
    color: var(--text-strong);
  }

  .terminal-body {
    display: flex;
    flex-direction: column;
    min-height: 0;
    padding: 8px 9px 9px;
    overflow: hidden;
    font-size: 13px;
    line-height: 1.55;
  }

  .terminal-host {
    flex: 1 1 auto;
    min-height: 0;
    height: auto;
    border: 1px solid var(--terminal-shell-border);
    border-radius: 9px;
    overflow: hidden;
  }
</style>
