<script lang="ts">
  import "@xterm/xterm/css/xterm.css";
  import {
    IconBrowser,
    IconChevronDown,
    IconCircleCheck,
    IconEraser,
    IconHammer,
    IconPlus,
    IconX,
  } from "@tabler/icons-svelte";
  import { t } from "$lib/i18n/runtime.svelte";
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

<section class="terminal-pane" aria-label={t("workbench-terminal-integrated")}>
  <header class="terminal-toolbar">
    <div class="ui-tabs compact terminal-tab-strip" role="tablist" aria-label={t("workbench-terminal-tabs")}>
      {#each terminalTabs as tab}
        <div class:active={activeTerminalTabId === tab.id} class="ui-tab terminal-tab">
          <button
            type="button"
            role="tab"
            aria-selected={activeTerminalTabId === tab.id ? "true" : "false"}
            tabindex={activeTerminalTabId === tab.id ? 0 : -1}
            title={t("workbench-terminal-shell-description")}
            onclick={() => { void selectTab(tab.id); }}
          >
            <span>{t("workbench-terminal-shell", { index: tab.index })}</span>
          </button>
          <button
            class="ui-icon-button ui-close-button terminal-tab-close"
            type="button"
            title={t("workbench-terminal-close-tab", {
              tab: t("workbench-terminal-shell", { index: tab.index }),
            })}
            onclick={() => closeTab(tab.id)}
          >
            <IconX size={13} stroke={2.2} />
          </button>
        </div>
      {/each}
    </div>

    <div class="terminal-actions" aria-label={t("workbench-terminal-actions")}>
      {#each quickTasks as task}
        <button
          class="ui-icon-button compact quiet terminal-task-button"
          type="button"
          title={`${t(task.labelId)} · ${t(task.titleId)}`}
          aria-label={t(task.labelId)}
          onclick={() => runQuickTask(task)}
        >
          {#if task.kind === "embedded-check"}
            <IconCircleCheck size={14} stroke={1.9} />
          {:else if task.kind === "embedded-build"}
            <IconHammer size={14} stroke={1.9} />
          {:else}
            <IconBrowser size={14} stroke={1.9} />
          {/if}
        </button>
      {/each}
      <button class="ui-icon-button compact quiet" type="button" title={t("workbench-terminal-clear")} onclick={clearActiveTerminal}>
        <IconEraser size={14} stroke={2} />
      </button>
      <button class="ui-icon-button compact quiet" type="button" title={t("workbench-terminal-new-tab")} onclick={openTab}>
        <IconPlus size={14} stroke={2} />
      </button>
      <button
        class="ui-icon-button compact quiet"
        type="button"
        title={`${t("workbench-terminal-hide")} (Ctrl+\`)`}
        aria-label={t("workbench-terminal-hide")}
        onclick={() => { void closePane(); }}
      >
        <IconChevronDown size={15} stroke={2} />
      </button>
    </div>
  </header>

  <div class="terminal-body">
    <div bind:this={terminalHost} class="terminal-host" aria-label={t("workbench-terminal-active-shell")}></div>
  </div>
</section>

<style>
  .terminal-pane {
    position: relative;
    display: grid;
    grid-template-rows: 38px minmax(0, 1fr);
    min-width: 0;
    min-height: 0;
    border: 1px solid var(--wb-border-subtle, var(--border));
    border-radius: var(--radius-panel);
    overflow: hidden;
    background: var(--terminal-shell-bg, var(--surface-7));
    box-shadow: var(--shadow-panel);
  }

  .terminal-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 5px;
    min-width: 0;
    padding: 2px 4px;
    border-bottom: 1px solid var(--wb-border-subtle, var(--border));
    background: var(--material-panel);
    box-shadow: inset 0 -1px 0 var(--skeuo-edge-highlight);
  }

  .terminal-actions {
    display: inline-flex;
    align-items: center;
    flex: 0 0 auto;
    gap: 2px;
    justify-content: flex-end;
  }

  .terminal-task-button {
    color: var(--brand-strong);
  }

  .terminal-tab-strip {
    display: flex;
    align-items: center;
    flex: 1 1 auto;
    gap: 2px;
    min-height: 0;
    min-width: 0;
    overflow: auto hidden;
    overscroll-behavior-x: contain;
  }

  .terminal-tab {
    display: inline-flex;
    align-items: center;
    flex: 0 0 auto;
    height: 26px;
    min-width: 0;
    max-width: 220px;
    padding: 0;
    overflow: hidden;
  }

  .terminal-tab > button {
    display: inline-flex;
    align-items: center;
    height: 25px;
    border: 0;
    background: transparent;
  }

  .terminal-tab > button:first-child {
    flex: 1 1 auto;
    min-width: 0;
    padding: 0 6px 0 8px;
    overflow: hidden;
    color: var(--text);
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: 11px;
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
    min-width: 24px;
    padding: 0;
    border-left: 0;
    color: var(--text-muted);
    opacity: 0;
  }

  .terminal-tab:hover .terminal-tab-close,
  .terminal-tab.active .terminal-tab-close,
  .terminal-tab-close:focus-visible {
    opacity: 1;
  }

  .terminal-tab-close:hover {
    color: var(--text-strong);
  }

  .terminal-body {
    display: flex;
    flex-direction: column;
    min-height: 0;
    padding: 0;
    overflow: hidden;
    font-size: 13px;
    line-height: 1.55;
    background: var(--material-inset);
    box-shadow: var(--shadow-inset);
  }

  .terminal-host {
    position: relative;
    flex: 1 1 auto;
    min-width: 0;
    min-height: 0;
    width: 100%;
    height: 100%;
    border: 0;
    border-radius: 0;
    overflow: hidden;
    background: var(--terminal-shell-bg, var(--surface-7));
  }

  :global(.terminal-host > .xterm) {
    box-sizing: border-box;
    width: 100%;
    height: 100%;
    padding: 4px 6px 6px;
    background: var(--terminal-shell-bg, var(--surface-7));
  }

  :global(.terminal-host .xterm-viewport) {
    background-color: var(--terminal-shell-bg, var(--surface-7));
  }

  :global(.terminal-host .xterm-scrollable-element > .scrollbar.vertical) {
    visibility: hidden;
    pointer-events: none;
  }

  :global(.terminal-host .terminal-scroll-proxy) {
    position: absolute;
    z-index: 12;
    top: 0;
    right: 0;
    bottom: 0;
    width: 14px;
    overflow-x: hidden;
    overflow-y: scroll;
    overscroll-behavior: contain;
    scrollbar-color: color-mix(in srgb, var(--text-muted) 42%, transparent) transparent;
    scrollbar-width: thin;
  }

  :global(.terminal-host .terminal-scroll-proxy-content) {
    width: 1px;
    min-height: 100%;
    pointer-events: none;
  }

  :global(.terminal-host .terminal-scroll-proxy::-webkit-scrollbar) {
    width: 14px;
  }

  :global(.terminal-host .terminal-scroll-proxy::-webkit-scrollbar-track) {
    background: transparent;
  }

  :global(.terminal-host .terminal-scroll-proxy::-webkit-scrollbar-thumb) {
    border: 3px solid transparent;
    border-radius: 999px;
    background: color-mix(in srgb, var(--text-muted) 42%, transparent);
    background-clip: padding-box;
  }
</style>
