<script lang="ts">
  import ContextMenuLayer from "$lib/components/context-menu/ContextMenuLayer.svelte";
  import NotificationStack from "$lib/components/NotificationStack.svelte";
  import StatusBar from "$lib/components/StatusBar.svelte";
  import Topbar from "$lib/components/Topbar.svelte";
  import CommandCenter from "$lib/components/workbench/CommandCenter.svelte";
  import { contextMenu } from "$lib/context-menu/store.svelte";
  import { t } from "$lib/i18n/runtime.svelte";
  import type { AppNotification } from "$lib/notifications/center";
  import type { WorkbenchSourceStatus } from "$lib/source-provenance";
  import type { EditFlushReason } from "$lib/session/edit-flush-registry";
  import type { ApplicationPreferencesState } from "$lib/application/preferences.svelte";
  import type { NotificationCenterState } from "$lib/notifications/store.svelte";
  import type { GlobalStatusState } from "$lib/status/state.svelte";
  import type { TerminalWorkspaceState } from "$lib/terminal/workspace.svelte";
  import type { WorkspaceLayoutState } from "$lib/ui/workspace-layout.svelte";
  import type { CenterView } from "$lib/application/contracts";
  import type { CommandCenterAction } from "$lib/workbench/contracts";
  import type { Snippet } from "svelte";

  let {
    project,
    surface,
    commands,
    applicationPreferences,
    notificationCenter,
    globalStatus,
    terminalWorkspace,
    workspaceLayout,
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
    project: {
      currentPath: string;
      root: string;
      sessionId: string;
      present: boolean;
      savePending: boolean;
    };
    surface: {
      application: "workbench" | "settings";
      activeActivity: string;
      sourceStatus: WorkbenchSourceStatus | null;
    };
    commands: {
      flushDrafts: (reason: EditFlushReason) => Promise<unknown>;
      openProjectFolder: () => Promise<void>;
      openProjectInBrowser: () => Promise<void>;
      save: () => Promise<boolean>;
      openCssSource: (target: { selector: string; file: string }) => Promise<unknown>;
      openSourceLocation: (location: string) => Promise<unknown>;
      setCenterView: (view: CenterView) => Promise<unknown>;
      requestCodeSelectionReveal: () => void;
      handleNotificationAction: (notification: AppNotification, actionId: string) => Promise<unknown>;
    };
    applicationPreferences: ApplicationPreferencesState;
    notificationCenter: NotificationCenterState;
    globalStatus: GlobalStatusState;
    terminalWorkspace: TerminalWorkspaceState;
    workspaceLayout: WorkspaceLayoutState;
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
    surface.activeActivity,
  );
  async function toggleRightInspectorPane() {
    if (!workspaceLayout.rightPaneCollapsed) {
      try {
        await commands.flushDrafts("template-switch");
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        globalStatus.set(t("workbench-inspector-collapse-blocked", { message }), "error");
        return;
      }
    }
    workspaceLayout.toggleRightPane();
  }

  async function openWorkbenchSource() {
    const source = surface.sourceStatus;
    if (!source?.openable) return;
    if (source.role === "css" && source.selector) {
      await commands.openCssSource({
        selector: source.selector,
        file: source.file,
      });
      return;
    }
    await commands.openSourceLocation(source.location);
    await commands.setCenterView("code");
    commands.requestCodeSelectionReveal();
  }

</script>

<div class="chrome-inert-layer" inert={commandCenterOpen ? true : undefined}>
<Topbar
  currentProjectPath={project.currentPath}
  canUndo={topbarCanUndo}
  inspectorHasPending={project.savePending}
  canRedo={topbarCanRedo}
  uiTheme={applicationPreferences.theme}
  noProject={!project.present}
  leftPaneCollapsed={workspaceLayout.leftPaneCollapsed}
  rightPaneCollapsed={workspaceLayout.rightPaneCollapsed}
  terminalPaneOpen={terminalWorkspace.terminalPaneOpen}
  sidebarsAvailable={surface.application === "workbench" && activeWorkbenchActivity === "editor"}
  openProjectFolder={() => commands.openProjectFolder()}
  openCurrentProjectInBrowser={() => commands.openProjectInBrowser()}
  canOpenInBrowser={project.present}
  saveActiveFile={() => commands.save()}
  undoAction={undoAction}
  redoAction={redoAction}
  toggleUiTheme={() => applicationPreferences.toggleTheme()}
  toggleLeftPane={() => workspaceLayout.toggleLeftPane()}
  toggleRightPane={toggleRightInspectorPane}
  toggleTerminalPane={() => { void terminalWorkspace.togglePane(); }}
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
  globalStatus={globalStatus.current}
  sourceStatus={surface.sourceStatus}
  openSource={openWorkbenchSource}
/>
</div>

<CommandCenter
  open={commandCenterOpen}
  projectRoot={project.root}
  runtimeSessionId={project.sessionId}
  close={closeCommandCenter}
  execute={executeCommandCenterAction}
/>

<NotificationStack
  notifications={notificationCenter.notifications}
  dismiss={(id) => notificationCenter.dismiss(id)}
  save={() => commands.save()}
  action={(notification, actionId) => commands.handleNotificationAction(notification, actionId)}
/>

<style>
  .chrome-inert-layer {
    display: contents;
  }
</style>

<ContextMenuLayer state={contextMenu} />
