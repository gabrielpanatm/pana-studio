<script lang="ts">
  import ContextMenuLayer from "$lib/components/context-menu/ContextMenuLayer.svelte";
  import NotificationStack from "$lib/components/NotificationStack.svelte";
  import StatusBar from "$lib/components/StatusBar.svelte";
  import Topbar from "$lib/components/Topbar.svelte";
  import CommandCenter from "$lib/components/workbench/CommandCenter.svelte";
  import { contextMenu } from "$lib/context-menu/store.svelte";
  import { t } from "$lib/i18n/runtime.svelte";
  import type { AppState } from "$lib/state/app.svelte";
  import type { CommandCenterAction } from "$lib/types";
  import type { Snippet } from "svelte";

  let {
    app,
    topbarCanUndo = false,
    topbarCanRedo = false,
    undoAction,
    redoAction,
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
        app.setGlobalStatus(t("workbench-inspector-collapse-blocked", { message }), "error");
        return;
      }
    }
    app.rightPaneCollapsed = !app.rightPaneCollapsed;
  }

  async function openWorkbenchSource() {
    const source = app.workbenchSourceStatus;
    if (!source?.openable) return;
    if (source.role === "css" && source.selector) {
      await app.openCssCodeRevealTarget({
        selector: source.selector,
        file: source.file,
      });
      return;
    }
    await app.openSourceLocation(source.location);
    await app.setCenterView("code");
    app.requestCodeSelectionReveal();
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
  canOpenInBrowser={Boolean(app.scannedProject)}
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
  globalStatus={app.currentGlobalStatus}
  sourceStatus={app.workbenchSourceStatus}
  openSource={openWorkbenchSource}
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
