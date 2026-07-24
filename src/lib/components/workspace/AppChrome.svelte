<script lang="ts">
  import ContextMenuLayer from "$lib/components/context-menu/ContextMenuLayer.svelte";
  import NotificationStack from "$lib/components/NotificationStack.svelte";
  import StatusBar from "$lib/components/StatusBar.svelte";
  import Topbar from "$lib/components/Topbar.svelte";
  import CommandCenter from "$lib/components/workbench/CommandCenter.svelte";
  import { contextMenu } from "$lib/context-menu/store.svelte";
  import type { AppState } from "$lib/state/app.svelte";
  import type { CommandCenterAction } from "$lib/types";
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
    commandCenterOpen = false,
    openCommandCenter = () => {},
    closeCommandCenter = () => {},
    executeCommandCenterAction = () => {},
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
    commandCenterOpen?: boolean;
    openCommandCenter?: () => void;
    closeCommandCenter?: () => void;
    executeCommandCenterAction?: (action: CommandCenterAction) => void | Promise<void>;
    children?: Snippet;
  } = $props();

  const activeWorkbenchActivity = $derived(
    app.workbenchSnapshot?.activeActivity ?? "editor",
  );
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

<div class="chrome-inert-layer" inert={commandCenterOpen ? true : undefined}>
<Topbar
  currentProjectPath={app.currentProjectPath}
  canUndo={topbarCanUndo}
  inspectorHasPending={app.saveHasPending}
  canRedo={topbarCanRedo}
  uiTheme={app.uiTheme}
  noProject={!app.scannedProject}
  leftPaneCollapsed={app.leftPaneCollapsed}
  rightPaneCollapsed={app.rightPaneCollapsed}
  terminalPaneOpen={app.terminalPaneOpen}
  sidebarsAvailable={app.applicationSurface === "workbench" && activeWorkbenchActivity === "editor"}
  openProjectFolder={() => app.openProjectFolder()}
  openCurrentProjectInBrowser={() => app.openCurrentProjectInBrowser()}
  canOpenInBrowser={app.scannedProject?.isZola ?? false}
  saveActiveFile={() => app.saveActiveFile()}
  undoAction={undoAction}
  redoAction={redoAction}
  toggleUiTheme={() => app.toggleUiTheme()}
  toggleLeftPane={() => { app.leftPaneCollapsed = !app.leftPaneCollapsed; }}
  toggleRightPane={toggleRightInspectorPane}
  toggleTerminalPane={() => { void app.toggleTerminalPane(); }}
  {openCommandCenter}
/>
</div>

{#if children}
  <div class="chrome-inert-layer" inert={commandCenterOpen ? true : undefined}>
  {@render children()}
  </div>
{/if}

<div class="chrome-inert-layer" inert={commandCenterOpen ? true : undefined}>
<StatusBar
  saveState={app.saveState}
  saveStatus={app.saveStatus}
  controlledPreview={app.controlledPreview}
  canvasPatchPerformance={app.canvasPatchPerformance}
  sourceLabel={statusSourceLabel}
  sourceValue={statusSourceValue}
  sourceOpenable={statusSourceOpenable}
  openSource={openStatusSource}
  aiCoordinationSnapshot={app.aiCoordinationSnapshot}
  externalReconciling={app.externalDiskState.reconciling}
  projectionRecoveryRequired={app.externalDiskState.workspaceProjectionRecoveryRequired}
/>
</div>

<CommandCenter
  open={commandCenterOpen}
  projectRoot={app.sessionProjectRoot}
  runtimeSessionId={app.kernelProjectSessionId}
  close={closeCommandCenter}
  execute={executeCommandCenterAction}
/>

<NotificationStack
  notifications={app.notifications}
  dismiss={(id) => app.dismissNotification(id)}
  save={() => app.saveActiveFile()}
  action={(notification, actionId) => app.handleNotificationAction(notification, actionId)}
/>

<style>
  .chrome-inert-layer {
    display: contents;
  }
</style>

<ContextMenuLayer state={contextMenu} />
