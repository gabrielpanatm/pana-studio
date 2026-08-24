import type { AiCoordinationState } from "$lib/ai/coordination-state.svelte";
import type { ExternalDiskState } from "$lib/session/external-disk-state.svelte";
import type { WorkspaceLayoutState } from "$lib/ui/workspace-layout.svelte";
import type { ApplicationShellState } from "$lib/application/shell-state.svelte";
import type { WorkbenchWorkspaceState } from "$lib/workbench/workspace-state.svelte";
import type { WorkbenchNavigationService } from "$lib/workbench/navigation-service";
import type { SourceWorkspaceState } from "$lib/editor/source-workspace.svelte";
import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { ProjectDocumentService } from "$lib/project/document-service";
import type { ProjectStartupState } from "$lib/project/startup-state.svelte";
import type { ProjectTransitionService } from "$lib/project/transition-service";
import type { ProjectSaveService } from "$lib/project/save-service";
import type { WorkspaceHistoryService } from "$lib/versioning/workspace-history-service.svelte";
import type { ControlledPreviewWorkspaceState } from "$lib/preview/controlled-state.svelte";
import type { ProjectBrowserPreviewService } from "$lib/state/project-browser-preview-controller";
import type { AppSessionService } from "$lib/state/app-session-controller";
import type { ProjectDerivedStateService } from "$lib/project/derived-state-service";
import type { TerminalWorkspaceState } from "$lib/terminal/workspace.svelte";
import type { MotionWorkspaceState } from "$lib/motion/workspace.svelte";
import type { PreviewWorkspaceState } from "$lib/preview/workspace-state.svelte";
import type { ApplicationPreferencesState } from "$lib/application/preferences.svelte";
import { flushWorkspaceMutationInputs } from "$lib/session/workspace-mutation-coordinator";
import { appShortcutIntent } from "$lib/ui/app-shortcuts";
import type { CommandCenterAction } from "$lib/workbench/contracts";
import type {
  WorkbenchActivity,
  WorkbenchSurface,
} from "$lib/workbench/contracts";
import { t } from "$lib/i18n/runtime.svelte";
import { getCurrentWindow } from "@tauri-apps/api/window";

export type CommandCenterGuards = Readonly<{
  ai: AiCoordinationState;
  externalDisk: ExternalDiskState;
}>;

export type CommandCenterWorkspace = Readonly<{
  layout: WorkspaceLayoutState;
  shell: ApplicationShellState;
  state: WorkbenchWorkspaceState;
  navigation: WorkbenchNavigationService;
  source: SourceWorkspaceState;
}>;

export type CommandCenterProject = Readonly<{
  session: ProjectSessionState;
  documents: ProjectDocumentService;
  startup: ProjectStartupState;
  transitions: ProjectTransitionService;
}>;

export type CommandCenterActions = Readonly<{
  save: ProjectSaveService;
  history: WorkspaceHistoryService;
  controlledPreview: ControlledPreviewWorkspaceState;
  browserPreview: ProjectBrowserPreviewService;
  appSession: AppSessionService;
  derived: ProjectDerivedStateService;
  terminal: TerminalWorkspaceState;
  motion: MotionWorkspaceState;
  preview: PreviewWorkspaceState;
  selectActivity: (activity: WorkbenchActivity) => Promise<void>;
  openAudit: (view: "overview" | "runtime", focusObservability?: boolean) => Promise<unknown>;
}>;

export type CommandCenterServiceDependencies = Readonly<{
  guards: CommandCenterGuards;
  workspace: CommandCenterWorkspace;
  project: CommandCenterProject;
  actions: CommandCenterActions;
  preferences: ApplicationPreferencesState;
}>;

/** Owns command-center visibility, dispatch and global keyboard intents. */
export class CommandCenterService {
  open = $state(false);
  private readonly dependencies: CommandCenterServiceDependencies;

  constructor(dependencies: CommandCenterServiceDependencies) {
    this.dependencies = dependencies;
  }

  show() {
    if (this.interactionLocked()) return;
    this.open = true;
  }

  close() {
    this.open = false;
  }

  handleShortcut(event: KeyboardEvent, editorSidebarsAvailable: boolean) {
    const intent = appShortcutIntent(event);
    if (this.interactionLocked()) {
      if (intent !== "none") event.preventDefault();
      return;
    }
    if (intent === "none") return;
    event.preventDefault();
    const d = this.dependencies;
    if (intent === "commandCenter") this.show();
    else if (intent === "openProject") void d.project.startup.openFolder();
    else if (intent === "closeApplication") void getCurrentWindow().close();
    else if (intent === "openSettings") void this.execute({ kind: "app_command", command: "open_settings" });
    else if (intent === "save") void d.actions.save.saveActiveFile();
    else if (intent === "undo") void d.actions.history.run("undo");
    else if (intent === "redo") void d.actions.history.run("redo");
    else if (intent === "toggleTerminal") void d.actions.terminal.togglePane();
    else if (intent === "showProblems" && d.project.session.project) {
      void d.actions.openAudit("overview");
    } else if (intent === "toggleEditorSplit" && d.project.session.project) {
      void d.workspace.state.setSplit(
        d.workspace.state.snapshot?.split === "none" ? "vertical" : "none",
      );
    } else if (
      intent === "togglePrimarySidebar"
      && d.project.session.project
      && editorSidebarsAvailable
    ) d.workspace.layout.toggleLeftPane();
  }

  async execute(action: CommandCenterAction) {
    const d = this.dependencies;
    if (action.kind === "set_activity") {
      await d.actions.selectActivity(action.activity);
      return;
    }
    if (action.kind === "open_document") {
      await this.openDocument(action.relativePath, action.surface);
      return;
    }
    switch (action.command) {
      case "open_project": await d.project.startup.openFolder(); break;
      case "close_application": await getCurrentWindow().close(); break;
      case "close_project": await d.project.transitions.close(); break;
      case "save": await d.actions.save.saveActiveFile(); break;
      case "undo": await d.actions.history.run("undo"); break;
      case "redo": await d.actions.history.run("redo"); break;
      case "validate": await d.actions.controlledPreview.runValidation("manual"); break;
      case "run_external": await d.actions.browserPreview.open(); break;
      case "refresh_session": await d.actions.appSession.refresh(); break;
      case "rescan_project": await d.actions.derived.rescan(); break;
      case "toggle_terminal": await d.actions.terminal.togglePane(); break;
      case "show_problems": await d.actions.openAudit("overview"); break;
      case "show_output": await d.actions.openAudit("runtime", true); break;
      case "show_timeline":
        d.actions.motion.openTimeline();
        d.actions.preview.setExecutionMode("motion");
        break;
      case "split_vertical": await d.workspace.state.setSplit("vertical"); break;
      case "split_horizontal": await d.workspace.state.setSplit("horizontal"); break;
      case "close_split": await d.workspace.state.setSplit("none"); break;
      case "canvas_fit":
        await d.workspace.state.setCanvasViewport({ mode: "fit", zoomPercent: 100 });
        break;
      case "canvas_desktop":
        await d.workspace.state.setCanvasViewport({ mode: "fixed", preset: "desktop", widthPx: 1_440 });
        break;
      case "canvas_tablet":
        await d.workspace.state.setCanvasViewport({ mode: "fixed", preset: "tablet", widthPx: 768 });
        break;
      case "canvas_mobile":
        await d.workspace.state.setCanvasViewport({ mode: "fixed", preset: "mobile", widthPx: 390 });
        break;
      case "toggle_left_sidebar": d.workspace.layout.toggleLeftPane(); break;
      case "toggle_inspector": await this.toggleInspector(); break;
      case "toggle_theme": d.preferences.toggleTheme(); break;
      case "open_settings":
        await flushWorkspaceMutationInputs("template-switch");
        d.workspace.shell.openSettings();
        break;
      case "open_about":
        await flushWorkspaceMutationInputs("template-switch");
        d.workspace.shell.openSettings("about");
        break;
      case "show_visual": await d.workspace.navigation.setCenterView("preview"); break;
      case "show_code": await d.workspace.navigation.setCenterView("code"); break;
    }
  }

  private interactionLocked() {
    const { ai, externalDisk } = this.dependencies.guards;
    return ai.frontendLockActive
      || externalDisk.snapshot.reconciling
      || externalDisk.snapshot.workspaceProjectionRecoveryRequired;
  }

  private async toggleInspector() {
    if (!this.dependencies.workspace.layout.rightPaneCollapsed) {
      await flushWorkspaceMutationInputs("template-switch");
    }
    this.dependencies.workspace.layout.toggleRightPane();
  }

  private async openDocument(relativePath: string, surface: WorkbenchSurface) {
    const d = this.dependencies;
    const file = d.project.session.project?.files.find(
      (candidate) => candidate.relativePath === relativePath,
    );
    if (!file) throw new Error(t("workbench-command-resource-missing", { path: relativePath }));
    d.workspace.shell.openWorkbench();
    await d.project.documents.load(file);
    await d.workspace.navigation.setCenterView(surface === "code" ? "code" : "preview");
    if (surface === "code") d.workspace.source.requestSelectionReveal();
  }
}
