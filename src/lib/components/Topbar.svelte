<script lang="ts">
  import ApplicationMenuBar from "$lib/components/topbar/ApplicationMenuBar.svelte";
  import HistoryActionButtons from "$lib/components/topbar/HistoryActionButtons.svelte";
  import PanelLayoutButtons from "$lib/components/topbar/PanelLayoutButtons.svelte";
  import ThemeButton from "$lib/components/topbar/ThemeButton.svelte";
  import ToolbarButton from "$lib/components/topbar/ToolbarButton.svelte";
  import {
    legacyTranslator,
    localeRevision,
  } from "$lib/i18n/runtime.svelte";

  $: t = legacyTranslator($localeRevision);
  import { IconExternalLink, IconFolderOpen, IconSearch } from "@tabler/icons-svelte";
  import type { CommandCenterAction } from "$lib/workbench/contracts";
  type UiTheme = "dark" | "light";

  export let canUndo = false;
  export let canRedo = false;
  export let inspectorHasPending = false;
  export let uiTheme: UiTheme = "dark";
  export let leftPaneCollapsed = false;
  export let rightPaneCollapsed = false;
  export let terminalPaneOpen = false;
  export let sidebarsAvailable = true;
  export let noProject = false;
  export let canOpenInBrowser = false;

  export let openProjectFolder: () => void;
  export let openCurrentProjectInBrowser: () => void | Promise<void>;
  export let saveActiveFile: () => void | Promise<boolean>;
  export let undoAction: () => void | Promise<void>;
  export let redoAction: () => void | Promise<void>;
  export let toggleUiTheme: () => void;
  export let toggleLeftPane: () => void;
  export let toggleTerminalPane: () => void;
  export let toggleRightPane: () => void | Promise<void>;
  export let openCommandCenter: () => void = () => {};
  export let executeCommandCenterAction: (action: CommandCenterAction) => void | Promise<void> = () => {};
</script>

<header class="topbar">
  <div class="topbar-left">
    <ApplicationMenuBar
      {noProject}
      {canUndo}
      {canRedo}
      {sidebarsAvailable}
      executeAction={executeCommandCenterAction}
      {openCommandCenter}
    />
  </div>

  <button
    type="button"
    class="command-center-trigger"
    aria-keyshortcuts="Control+K Meta+K"
    title={`${t("workbench-command-center-open")} (Ctrl+K)`}
    onclick={openCommandCenter}
  >
    <IconSearch size={16} stroke={1.8} />
    <span>{t("workbench-command-center-search")}</span>
    <kbd>Ctrl K</kbd>
  </button>

  <div class="workspace-toolbar" aria-label={t("workbench-workspace-actions")}>
    {#if noProject}
      <div class="toolbar-group project-actions" aria-label={t("workbench-project")}>
        <ToolbarButton
          title={t("workbench-open-project-folder")}
          cta
          onclick={() => openProjectFolder()}
        >
          <IconFolderOpen size={17} stroke={1.8} />
        </ToolbarButton>
      </div>
    {:else if canOpenInBrowser}
      <div class="toolbar-group project-actions" aria-label={t("workbench-run")}>
        <ToolbarButton
          title={t("workbench-open-site-browser")}
          onclick={() => { void openCurrentProjectInBrowser(); }}
        >
          <IconExternalLink size={17} stroke={1.8} />
        </ToolbarButton>
      </div>
    {/if}

    <div class="toolbar-group history-actions" aria-label={t("workbench-history-actions")}>
      <HistoryActionButtons
        {canUndo}
        {canRedo}
        {inspectorHasPending}
        {saveActiveFile}
        {undoAction}
        {redoAction}
      />
    </div>

    <div class="toolbar-group theme-actions" aria-label={t("workbench-theme")}>
      <ThemeButton {uiTheme} {toggleUiTheme} />
    </div>

    {#if !noProject}
      <div class="toolbar-group segmented-group panel-layout-controls" aria-label={t("workbench-workspace-panels")}>
        <PanelLayoutButtons
          {leftPaneCollapsed}
          {rightPaneCollapsed}
          {terminalPaneOpen}
          showSidebars={sidebarsAvailable}
          {toggleLeftPane}
          {toggleTerminalPane}
          {toggleRightPane}
        />
      </div>
    {/if}
  </div>
</header>

<style>
  .topbar {
    flex: 0 0 auto;
    display: grid;
    grid-template-columns: minmax(300px, 1fr) minmax(260px, 520px) minmax(220px, 1fr);
    align-items: center;
    gap: 12px;
    min-height: 50px;
    padding: 0 10px;
    border-top: 1px solid var(--skeuo-edge-highlight);
    border-bottom: 1px solid var(--border-strong);
    background: var(--material-panel);
    box-shadow: var(--shadow-panel);
  }

  .topbar-left {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .workspace-toolbar,
  .toolbar-group {
    display: flex;
    align-items: center;
  }

  .workspace-toolbar {
    gap: 8px;
    min-width: 0;
    justify-content: flex-end;
  }

  .toolbar-group {
    position: relative;
    gap: 6px;
  }

  .toolbar-group + .toolbar-group {
    margin-left: 7px;
  }

  .toolbar-group + .toolbar-group::before {
    content: "";
    position: absolute;
    left: -9px;
    top: 50%;
    width: 1px;
    height: 22px;
    background: linear-gradient(
      180deg,
      transparent,
      var(--border-strong) 22% 78%,
      transparent
    );
    transform: translateY(-50%);
  }

  .segmented-group {
    gap: 0;
    padding: 2px;
    overflow: hidden;
    border: 1px solid var(--border-subtle);
    border-radius: calc(var(--radius-control) + 1px);
    background: var(--material-inset);
    box-shadow:
      inset 0 1px 2px var(--skeuo-shade-soft),
      inset 0 -1px 0 var(--skeuo-edge-highlight);
  }

  .segmented-group > :global(.toolbar-icon-button.segmented:first-child) {
    border-radius: calc(var(--radius-control) - 3px) 0 0 calc(var(--radius-control) - 3px);
  }

  .segmented-group > :global(.toolbar-icon-button.segmented:last-child) {
    border-radius: 0 calc(var(--radius-control) - 3px) calc(var(--radius-control) - 3px) 0;
  }

  .segmented-group > :global(.toolbar-icon-button.segmented:only-child) {
    border-radius: calc(var(--radius-control) - 3px);
  }

  .segmented-group > :global(.toolbar-icon-button.segmented + .toolbar-icon-button.segmented)::before {
    position: absolute;
    inset: 7px auto 7px 0;
    width: 1px;
    background: color-mix(in srgb, var(--border-strong) 72%, transparent);
    content: "";
    pointer-events: none;
  }

  .command-center-trigger {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    height: 32px;
    padding: 0 7px 0 10px;
    border: 1px solid var(--wb-border-subtle, var(--border-3));
    border-radius: var(--radius-control);
    color: var(--wb-text-muted, var(--text-muted));
    text-align: left;
    background: var(--material-inset);
    box-shadow: var(--shadow-inset);
  }

  .command-center-trigger:hover {
    border-color: var(--border-strong);
    color: var(--wb-text-primary, var(--text));
    background: color-mix(in srgb, var(--surface-inset) 92%, var(--brand));
  }

  .command-center-trigger:focus-visible {
    outline: 2px solid var(--wb-focus-ring, var(--brand-strong));
    outline-offset: 1px;
  }

  .command-center-trigger > span {
    min-width: 0;
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--font-body);
  }

  .command-center-trigger kbd {
    flex: 0 0 auto;
    padding: 3px 6px;
    border: 1px solid var(--wb-border-subtle, var(--border));
    border-radius: 3px;
    color: var(--wb-text-muted, var(--text-muted));
    font-family: inherit;
    font-size: var(--font-meta);
    background: var(--material-control);
    box-shadow: var(--shadow-control);
  }

  @media (max-width: 1080px) {
    .topbar {
      grid-template-columns: minmax(260px, 0.9fr) minmax(220px, 1fr) auto;
    }

    .command-center-trigger kbd,
    .theme-actions {
      display: none;
    }
  }

</style>
