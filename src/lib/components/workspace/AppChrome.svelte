<script lang="ts">
  import HistoryPanel from "$lib/components/HistoryPanel.svelte";
  import ContextMenuLayer from "$lib/components/context-menu/ContextMenuLayer.svelte";
  import NotificationStack from "$lib/components/NotificationStack.svelte";
  import SettingsPanel from "$lib/components/SettingsPanel.svelte";
  import StatusBar from "$lib/components/StatusBar.svelte";
  import Topbar from "$lib/components/Topbar.svelte";
  import VersionsPanel from "$lib/components/VersionsPanel.svelte";
  import { contextMenu } from "$lib/context-menu/store.svelte";
  import type { AppState } from "$lib/state/app.svelte";
  import type { SaveState } from "$lib/types";
  import type { Snippet } from "svelte";

  let {
    app,
    topbarCanUndo = false,
    topbarCanRedo = false,
    undoAction,
    redoAction,
    statusSourceLabel = "",
    statusSourceValue = "",
    statusSourceOpenable = false,
    openStatusSource,
    children,
  }: {
    app: AppState;
    topbarCanUndo?: boolean;
    topbarCanRedo?: boolean;
    undoAction: () => void | Promise<void>;
    redoAction: () => void | Promise<void>;
    statusSourceLabel?: string;
    statusSourceValue?: string;
    statusSourceOpenable?: boolean;
    openStatusSource: () => void | Promise<void>;
    children?: Snippet;
  } = $props();

  async function toggleRightInspectorPane() {
    if (!app.rightPaneCollapsed) {
      try {
        await app.flushInteractiveEditorDrafts("template-switch");
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        app.setGlobalStatus(`Restrângerea Inspectorului a fost blocată: ${message}`, "error");
        return;
      }
    }
    app.rightPaneCollapsed = !app.rightPaneCollapsed;
  }
</script>

<Topbar
  currentProjectPath={app.currentProjectPath}
  canUndo={topbarCanUndo}
  inspectorHasPending={app.saveHasPending}
  canRedo={topbarCanRedo}
  previewDevice={app.previewDevice}
  centerView={app.centerView}
  sourceLanguage={app.sourceLanguage}
  uiTheme={app.uiTheme}
  noProject={!app.scannedProject}
  canPreviewCurrentSource={app.canPreviewCurrentSource}
  leftPaneCollapsed={app.leftPaneCollapsed}
  rightPaneCollapsed={app.rightPaneCollapsed}
  terminalPaneOpen={app.terminalPaneOpen}
  historyPanelOpen={app.historyPanelOpen}
  settingsPanelOpen={app.settingsPanelOpen}
  versionsPanelOpen={app.versionsPanelOpen}
  openProjectFolder={() => app.openProjectFolder()}
  closeCurrentProject={async () => { await app.closeCurrentProject(); }}
  openCurrentProjectInBrowser={() => app.openCurrentProjectInBrowser()}
  rescanCurrentProject={() => app.rescanCurrentProject()}
  refreshCurrentSession={() => app.refreshCurrentSession()}
  validateZola={async () => { await app.runZolaValidation("manual"); }}
  canOpenInBrowser={app.scannedProject?.isZola ?? false}
  saveActiveFile={() => app.saveActiveFile()}
  undoAction={undoAction}
  redoAction={redoAction}
  setPreviewDevice={(device) => (app.previewDevice = device)}
  setCenterView={(view) => { void app.setCenterView(view); }}
  toggleUiTheme={() => app.toggleUiTheme()}
  toggleLeftPane={() => { app.leftPaneCollapsed = !app.leftPaneCollapsed; }}
  toggleRightPane={toggleRightInspectorPane}
  toggleTerminalPane={() => { app.terminalPaneOpen = !app.terminalPaneOpen; }}
  toggleHistoryPanel={() => {
    const next = !app.historyPanelOpen;
    app.historyPanelOpen = next;
    if (next) {
      app.settingsPanelOpen = false;
      app.versionsPanelOpen = false;
    }
  }}
  toggleVersionsPanel={() => {
    const next = !app.versionsPanelOpen;
    app.versionsPanelOpen = next;
    if (next) {
      app.historyPanelOpen = false;
      app.settingsPanelOpen = false;
    }
  }}
  toggleSettingsPanel={() => {
    const next = !app.settingsPanelOpen;
    app.settingsPanelOpen = next;
    if (next) {
      app.historyPanelOpen = false;
      app.versionsPanelOpen = false;
    }
  }}
/>

{#if children}
  {@render children()}
{/if}

<StatusBar
  saveState={app.saveState}
  saveStatus={app.saveStatus}
  controlledPreview={app.controlledPreview}
  canvasPatchPerformance={app.canvasPatchPerformance}
  previewZoom={app.previewZoom}
  setPreviewZoom={(value) => app.setPreviewZoom(value)}
  resetPreviewZoom={() => app.resetPreviewZoom()}
  sourceLabel={statusSourceLabel}
  sourceValue={statusSourceValue}
  sourceOpenable={statusSourceOpenable}
  openSource={openStatusSource}
/>

<NotificationStack
  notifications={app.notifications}
  dismiss={(id) => app.dismissNotification(id)}
  save={() => app.saveActiveFile()}
  action={(notification, actionId) => app.handleNotificationAction(notification, actionId)}
/>

<SettingsPanel
  open={app.settingsPanelOpen}
  scannedProject={!!app.scannedProject}
  isZola={app.scannedProject?.isZola ?? false}
  isEmpty={app.scannedProject?.isEmpty ?? false}
  cachebustAssets={app.cachebustAssets}
  aiContextStatus={app.aiContextStatus}
  onStatusUpdate={(text, kind) => app.setGlobalStatus(text, kind as SaveState)}
  onCachebustAssetsChange={(value) => { app.cachebustAssets = value; }}
  close={() => { app.settingsPanelOpen = false; }}
/>

<HistoryPanel
  open={app.historyPanelOpen}
  workspace={app.projectWorkspaceSnapshot}
  {undoAction}
  {redoAction}
  discardSession={async () => { await app.discardSessionAndReloadFromDisk(); }}
  close={() => { app.historyPanelOpen = false; }}
/>

<VersionsPanel
  open={app.versionsPanelOpen}
  projectRoot={app.sessionProjectRoot}
  sessionId={app.kernelProjectSessionId}
  workspace={app.projectWorkspaceSnapshot}
  onStatusUpdate={(text, kind) => app.setGlobalStatus(text, kind)}
  activePreviewCommitOid={app.activeVersionPreview?.commitOid ?? null}
  showPreview={async (receipt) => { await app.showVersionPreview(receipt); }}
  returnToLivePreview={async () => { await app.returnToLivePreview(); }}
  afterRestore={async (receipt) => {
    if (receipt.workspace) app.projectWorkspaceSnapshot = receipt.workspace;
    await app.rescanCurrentProject(app.activeScannedPath, { strict: true });
  }}
  afterRecovery={async (receipt) => {
    if (receipt.workspace) app.projectWorkspaceSnapshot = receipt.workspace;
    await app.rescanCurrentProject(app.activeScannedPath, { strict: true });
  }}
  afterIntegration={async (receipt) => {
    if (receipt.workspace) app.projectWorkspaceSnapshot = receipt.workspace;
    await app.rescanCurrentProject(app.activeScannedPath, { strict: true });
  }}
  afterIntegrationRecovery={async (receipt) => {
    if (receipt.workspace) app.projectWorkspaceSnapshot = receipt.workspace;
    await app.rescanCurrentProject(app.activeScannedPath, { strict: true });
  }}
  close={() => { app.versionsPanelOpen = false; }}
/>

<ContextMenuLayer state={contextMenu} />
