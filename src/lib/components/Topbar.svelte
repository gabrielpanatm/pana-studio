<script lang="ts">
  import HistoryActionButtons from "$lib/components/topbar/HistoryActionButtons.svelte";
  import PanelLayoutButtons from "$lib/components/topbar/PanelLayoutButtons.svelte";
  import ThemeButton from "$lib/components/topbar/ThemeButton.svelte";
  import ToolbarButton from "$lib/components/topbar/ToolbarButton.svelte";
  import { IconExternalLink, IconFolderOpen, IconSearch } from "@tabler/icons-svelte";
  type UiTheme = "dark" | "light";

  export let currentProjectPath = "";
  export let canUndo = false;
  export let canRedo = false;
  export let inspectorHasPending = false;
  export let uiTheme: UiTheme = "dark";
  export let leftPaneCollapsed = false;
  export let rightPaneCollapsed = false;
  export let terminalPaneOpen = false;
  export let sidebarsAvailable = true;
  export let historyPanelOpen = false;
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
  export let toggleHistoryPanel: () => void;
  export let openCommandCenter: () => void = () => {};

  $: currentProjectName = currentProjectPath.split(/[\\/]/).filter(Boolean).at(-1) ?? currentProjectPath;
  $: title = noProject ? "Pană Studio" : currentProjectName;
  $: subtitle = noProject ? "Studio local pentru proiecte web Zola" : currentProjectPath;
</script>

<header class="topbar">
  <div class="topbar-left">
    <div class="project-meta">
      <p class="app-name" title={title}>{title}</p>
      <p class="project-path" title={subtitle}>{subtitle}</p>
    </div>
  </div>

  <button
    type="button"
    class="command-center-trigger"
    aria-keyshortcuts="Control+K Meta+K"
    title="Deschide centrul de comenzi (Ctrl+K)"
    onclick={openCommandCenter}
  >
    <IconSearch size={16} stroke={1.8} />
    <span>Caută comenzi, fișiere și simboluri…</span>
    <kbd>Ctrl K</kbd>
  </button>

  <div class="workspace-toolbar" aria-label="Acțiuni ale spațiului de lucru">
    {#if noProject}
      <div class="toolbar-group project-actions" aria-label="Proiect">
        <ToolbarButton
          title="Deschide dosar proiect"
          cta
          onclick={() => openProjectFolder()}
        >
          <IconFolderOpen size={17} stroke={1.8} />
        </ToolbarButton>
      </div>
    {:else if canOpenInBrowser}
      <div class="toolbar-group project-actions" aria-label="Rulează">
        <ToolbarButton
          title="Deschide site-ul în browser"
          onclick={() => { void openCurrentProjectInBrowser(); }}
        >
          <IconExternalLink size={17} stroke={1.8} />
        </ToolbarButton>
      </div>
    {/if}

    <div class="toolbar-group history-actions" aria-label="Salvare si istoric">
      <HistoryActionButtons
        {canUndo}
        {canRedo}
        {inspectorHasPending}
        {historyPanelOpen}
        {saveActiveFile}
        {undoAction}
        {redoAction}
        {toggleHistoryPanel}
      />
    </div>

    <div class="toolbar-group theme-actions" aria-label="Tema">
      <ThemeButton {uiTheme} {toggleUiTheme} />
    </div>

    {#if !noProject}
      <div class="toolbar-group segmented-group panel-layout-controls" aria-label="Panourile spațiului de lucru">
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
    grid-template-columns: minmax(160px, 1fr) minmax(260px, 520px) minmax(220px, 1fr);
    align-items: center;
    gap: 12px;
    min-height: 48px;
    padding: 0 10px;
    border-bottom: 1px solid var(--border);
    background: color-mix(in srgb, var(--app-bg) 92%, transparent);
    backdrop-filter: blur(10px);
  }

  .topbar-left {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .project-meta,
  .app-name,
  .project-path,
  p {
    margin: 0;
  }

  .project-meta {
    display: grid;
    gap: 2px;
    min-width: 0;
  }

  .app-name {
    max-width: 320px;
    overflow: hidden;
    color: var(--text-strong);
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: 14px;
    font-weight: 800;
  }

  .project-path {
    max-width: 420px;
    margin-top: 2px;
    overflow: hidden;
    color: var(--text-muted);
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: 12px;
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
    border-radius: 999px;
    background: color-mix(in srgb, var(--border-4) 58%, transparent);
    transform: translateY(-50%);
  }

  .segmented-group {
    gap: 0;
    padding: 2px;
    border: 1px solid var(--border-3);
    border-radius: 8px;
    background: color-mix(in srgb, var(--surface-3) 70%, transparent);
  }

  .command-center-trigger {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    height: 32px;
    padding: 0 7px 0 10px;
    border: 1px solid var(--wb-border-subtle, var(--border-3));
    border-radius: 8px;
    color: var(--wb-text-muted, var(--text-muted));
    text-align: left;
    background: color-mix(in srgb, var(--wb-surface-document, var(--surface)) 86%, transparent);
  }

  .command-center-trigger:hover {
    border-color: color-mix(in srgb, var(--wb-accent, var(--brand)) 55%, var(--wb-border-subtle));
    color: var(--wb-text-primary, var(--text));
    background: var(--wb-control-hover, var(--brand-soft));
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
    font-size: 12px;
  }

  .command-center-trigger kbd {
    flex: 0 0 auto;
    padding: 3px 6px;
    border: 1px solid var(--wb-border-subtle, var(--border));
    border-radius: 5px;
    color: var(--wb-text-muted, var(--text-muted));
    font-family: inherit;
    font-size: 12px;
    background: var(--wb-surface-chrome, var(--surface-2));
  }

  @media (max-width: 1080px) {
    .topbar {
      grid-template-columns: minmax(120px, 0.7fr) minmax(220px, 1fr) auto;
    }

    .project-path,
    .command-center-trigger kbd,
    .theme-actions {
      display: none;
    }
  }

</style>
