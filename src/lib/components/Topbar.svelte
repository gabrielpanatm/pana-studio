<script lang="ts">
  import CenterViewButtons from "$lib/components/topbar/CenterViewButtons.svelte";
  import HistoryActionButtons from "$lib/components/topbar/HistoryActionButtons.svelte";
  import PanelLayoutButtons from "$lib/components/topbar/PanelLayoutButtons.svelte";
  import PreviewDeviceButtons from "$lib/components/topbar/PreviewDeviceButtons.svelte";
  import ProjectActionButtons from "$lib/components/topbar/ProjectActionButtons.svelte";
  import SettingsActionButton from "$lib/components/topbar/SettingsActionButton.svelte";
  import ThemeButton from "$lib/components/topbar/ThemeButton.svelte";
  import VersionsActionButton from "$lib/components/topbar/VersionsActionButton.svelte";
  import WorkspaceActionButtons from "$lib/components/topbar/WorkspaceActionButtons.svelte";
  type PreviewDevice = "desktop" | "tablet" | "mobile";
  type CenterView = "preview" | "code" | "markdown" | "canvas" | "site" | "kernel";
  type UiTheme = "dark" | "light";
  type SourceLanguage = "html" | "css" | "scss" | "js" | "markdown" | "plain";

  export let currentProjectPath = "";
  export let canUndo = false;
  export let canRedo = false;
  export let inspectorHasPending = false;
  export let previewDevice: PreviewDevice = "desktop";
  export let centerView: CenterView = "preview";
  export let sourceLanguage: SourceLanguage = "plain";
  export let uiTheme: UiTheme = "dark";
  export let leftPaneCollapsed = false;
  export let rightPaneCollapsed = false;
  export let terminalPaneOpen = false;
  export let historyPanelOpen = false;
  export let settingsPanelOpen = false;
  export let versionsPanelOpen = false;
  export let canPreviewCurrentSource = true;
  export let noProject = false;
  export let canOpenInBrowser = false;

  export let openProjectFolder: () => void;
  export let closeCurrentProject: () => void | Promise<void>;
  export let openCurrentProjectInBrowser: () => void | Promise<void>;
  export let rescanCurrentProject: () => void;
  export let refreshCurrentSession: () => void | Promise<void>;
  export let validateZola: () => void | Promise<void>;
  export let saveActiveFile: () => void | Promise<boolean>;
  export let undoAction: () => void | Promise<void>;
  export let redoAction: () => void | Promise<void>;
  export let setPreviewDevice: (device: PreviewDevice) => void;
  export let setCenterView: (view: CenterView) => void;
  export let toggleUiTheme: () => void;
  export let toggleLeftPane: () => void;
  export let toggleTerminalPane: () => void;
  export let toggleRightPane: () => void | Promise<void>;
  export let toggleHistoryPanel: () => void;
  export let toggleSettingsPanel: () => void;
  export let toggleVersionsPanel: () => void;

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

  <div class="workspace-toolbar" aria-label="Toolbar principal">
    <div class="toolbar-group project-actions" aria-label="Proiect">
      <ProjectActionButtons
        {noProject}
        {canOpenInBrowser}
        {openProjectFolder}
        {closeCurrentProject}
        {openCurrentProjectInBrowser}
        {rescanCurrentProject}
        {refreshCurrentSession}
        {validateZola}
      />
    </div>

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

    <div class="toolbar-group settings-actions" aria-label="Setari proiect">
      <VersionsActionButton
        {versionsPanelOpen}
        disabled={noProject}
        {toggleVersionsPanel}
      />
      <SettingsActionButton {settingsPanelOpen} {toggleSettingsPanel} />
    </div>

    <div class="toolbar-group workspace-actions" aria-label="Workspace-uri">
      <WorkspaceActionButtons
        {centerView}
        disabled={noProject}
        {setCenterView}
      />
    </div>

    <div class="toolbar-group segmented-group device-switcher" aria-label="Dimensiune preview">
      <PreviewDeviceButtons {previewDevice} {setPreviewDevice} />
    </div>

    <div class="toolbar-group segmented-group view-switcher" aria-label="Mod centru">
      <CenterViewButtons
        {centerView}
        {canPreviewCurrentSource}
        isMarkdownSource={sourceLanguage === "markdown"}
        disabled={noProject}
        {setCenterView}
      />
    </div>

    <div class="toolbar-group theme-actions" aria-label="Tema">
      <ThemeButton {uiTheme} {toggleUiTheme} />
    </div>

    <div class="toolbar-group segmented-group panel-layout-controls" aria-label="Panouri laterale">
      <PanelLayoutButtons
        {leftPaneCollapsed}
        {rightPaneCollapsed}
        {terminalPaneOpen}
        {toggleLeftPane}
        {toggleTerminalPane}
        {toggleRightPane}
      />
    </div>
  </div>
</header>

<style>
  .topbar {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    min-height: 52px;
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
    gap: 14px;
    flex-wrap: wrap;
    justify-content: flex-end;
  }

  .toolbar-group {
    position: relative;
    gap: 6px;
  }

  .toolbar-group + .toolbar-group {
    margin-left: 12px;
  }

  .toolbar-group + .toolbar-group::before {
    content: "";
    position: absolute;
    left: -14px;
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

</style>
