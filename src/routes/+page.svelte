<script lang="ts">
  import "./workspace-shell.css";
  import type { Component } from "svelte";
  import type { TerminalPaneProps } from "$lib/components/TerminalPane.svelte";
  import AppChrome from "$lib/components/workspace/AppChrome.svelte";
  import StartupView from "$lib/components/startup/StartupView.svelte";
  import ProjectOpenRecoveryDialog from "$lib/components/project/ProjectOpenRecoveryDialog.svelte";
  import ProjectTransitionDecisionDialog from "$lib/components/project/ProjectTransitionDecisionDialog.svelte";
  import WorkspaceCenterArea from "$lib/components/workspace/WorkspaceCenterArea.svelte";
  import WorkspaceInspectorArea from "$lib/components/workspace/WorkspaceInspectorArea.svelte";
  import WorkspaceProjectArea from "$lib/components/workspace/WorkspaceProjectArea.svelte";
  import ActivityRail from "$lib/components/workbench/ActivityRail.svelte";
  import { scannedCacheKey } from "$lib/project/files";
  import {
    kernelUndoRedoProjectionLeaseMatches,
    type KernelUndoRedoProjectionLease,
  } from "$lib/kernel/undo-redo-projection-lease";
  import { requireProjectWorkspaceUndoRedoCommandReceipt } from "$lib/kernel/project-workspace-undo-redo-receipt";
  import { reconcileProjectWorkspaceTopologyAfterHistory } from "$lib/kernel/project-workspace-history-topology";
  import { isMessageFromExactPreviewFrame } from "$lib/preview/frame-origin";
  import { isPreviewControlPlaneMessage } from "$lib/state/app-preview-runtime-controller";
  import {
    readProjectWorkspaceState,
    redoProjectWorkspace,
    undoProjectWorkspace,
  } from "$lib/project/io";
  import { rebaseFileBufferDraftSyncProjection } from "$lib/session/file-buffer-draft-sync";
  import { projectLatestProjectWorkspacePreview } from "$lib/kernel/project-workspace-preview-coordinator";
  import { AppState } from "$lib/state/app.svelte";
  import { reloadAuthorizedAiReconciliationFromDisk } from "$lib/state/ai-coordination-controller";
  import { appShortcutIntent, deleteShortcutIntent } from "$lib/ui/app-shortcuts";
  import {
    nativeZoomListenerOptions,
    preventNativeGestureZoom,
    preventNativeZoomWheel,
    resetNativeWebviewZoom,
    resetNativeZoomIfVisualViewportChanged,
  } from "$lib/ui/native-zoom";
  import { installSmoothWheelScrolling } from "$lib/ui/smooth-wheel";
  import {
    selectTopbarUndoRedoRoute,
    topbarUndoRedoState,
    type TopbarUndoRedoDirection,
  } from "$lib/ui/undo-redo-router";
  import type {
    CommandCenterAction,
    ProjectWorkspaceSnapshot,
    ProjectWorkspaceUndoRedoCommandReceipt,
    WorkbenchSurface,
    WorkspaceSourceOpenOptions,
  } from "$lib/types";
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { t } from "$lib/i18n/runtime.svelte";

  type ProjectWorkspaceUndoRedoOutcome =
    | {
        ok: true;
        snapshot: ProjectWorkspaceSnapshot["history"];
        receipt: ProjectWorkspaceUndoRedoCommandReceipt;
      }
    | { ok: false; message: string };

  const app = new AppState();
  let TerminalPaneComponent = $state<Component<TerminalPaneProps> | null>(null);
  let terminalPaneLoading = false;
  let topbarKernelUndoRedo = $state<ProjectWorkspaceSnapshot | null>(null);
  let topbarKernelUndoRedoKey = "";
  let topbarKernelUndoRedoLoading = $state(false);
  let kernelUndoRedoInFlight = false;
  let externalRecoveryInFlight = $state(false);
  let commandCenterOpen = $state(false);

  const topbarUndoRedo = $derived(topbarUndoRedoState({
    kernelCanUndo: Boolean(topbarKernelUndoRedo?.history.canUndo),
    kernelCanRedo: Boolean(topbarKernelUndoRedo?.history.canRedo),
  }));
  const editorSidebarsAvailable = $derived(
    app.applicationSurface === "workbench"
      && (app.workbenchSnapshot?.activeActivity ?? "editor") === "editor",
  );

  async function refreshTopbarKernelUndoRedoState() {
    if (!app.scannedProject) {
      topbarKernelUndoRedo = null;
      return null;
    }
    topbarKernelUndoRedoLoading = true;
    try {
      topbarKernelUndoRedo = await readProjectWorkspaceState();
      return topbarKernelUndoRedo;
    } catch (error) {
      topbarKernelUndoRedo = null;
      app.setGlobalStatus(t("workbench-history-read-failed", { error: errorMessage(error) }), "error");
      return null;
    } finally {
      topbarKernelUndoRedoLoading = false;
    }
  }

  async function runTopbarUndoRedo(direction: TopbarUndoRedoDirection) {
    if (app.scannedProject) {
      await refreshTopbarKernelUndoRedoState();
    }
    const route = selectTopbarUndoRedoRoute(direction, {
      kernelCanUndo: Boolean(topbarKernelUndoRedo?.history.canUndo),
      kernelCanRedo: Boolean(topbarKernelUndoRedo?.history.canRedo),
    });

    if (route === "workspace") {
      await runKernelUndoRedo(direction);
    }
  }

  async function runKernelUndoRedo(
    direction: TopbarUndoRedoDirection,
  ): Promise<ProjectWorkspaceUndoRedoOutcome> {
    if (kernelUndoRedoInFlight) {
      const message = t("workbench-history-in-flight");
      return { ok: false, message };
    }

    const lease: KernelUndoRedoProjectionLease = {
      expectedProjectRoot: app.sessionProjectRoot,
      expectedSessionId: app.kernelProjectSessionId,
      expectedSessionEpoch: app.projectSessionEpoch,
    };
    if (!lease.expectedProjectRoot || !lease.expectedSessionId) {
      const message = t("workbench-history-session-required");
      return { ok: false, message };
    }

    kernelUndoRedoInFlight = true;
    let frontendLeaseAcquired = false;
    let operationReceipt: ProjectWorkspaceUndoRedoCommandReceipt | null = null;
    try {
      await app.beginKernelUndoRedoFrontendLease();
      frontendLeaseAcquired = true;
      requireCurrentKernelUndoRedoUiLease(lease, t("workbench-history-lease-frontend"));
      const before = await refreshTopbarKernelUndoRedoState();
      requireCurrentKernelUndoRedoUiLease(lease, t("workbench-history-lease-read"));
      const target = direction === "undo" ? before?.history.nextUndo : before?.history.nextRedo;
      if (!before || !target) {
        const message = direction === "undo"
          ? t("workbench-history-no-undo")
          : t("workbench-history-no-redo");
        return { ok: false, message };
      }

      app.setGlobalStatus(
        direction === "undo" ? t("workbench-history-applying-undo") : t("workbench-history-applying-redo"),
        "saving",
      );
      const identity = {
        expectedProjectRoot: before.projectRoot,
        expectedSessionId: before.runtimeSessionId,
        expectedRevision: before.revision,
        expectedTransactionId: target.transactionId,
      };
      const receipt = direction === "undo"
        ? await undoProjectWorkspace(identity)
        : await redoProjectWorkspace(identity);
      operationReceipt = receipt;
      requireCurrentKernelUndoRedoUiLease(lease, t("workbench-history-lease-receipt"));
      requireProjectWorkspaceUndoRedoCommandReceipt(receipt, {
        projectRoot: lease.expectedProjectRoot,
        runtimeSessionId: lease.expectedSessionId,
        direction,
        revisionBefore: before.revision,
        transactionId: target.transactionId,
      });
      // History changed the canonical CSS state. Any optimistic Inspector
      // layer belongs to the pre-history revision and must never be replayed
      // over the generation that Rust is about to publish.
      app.clearInspectorLiveProperties();
      if (receipt.workbench) {
        app.workbenchSnapshot = receipt.workbench.snapshot;
      }
      topbarKernelUndoRedo = receipt.workspace;
      const previewWarning = await syncAfterKernelUndoRedo(receipt, lease);
      const label = direction === "undo" ? t("workbench-history-undo-label") : t("workbench-history-redo-label");
      app.setGlobalStatus(
        previewWarning
          ? t("workbench-history-applied-preview-warning", { operation: label, warning: previewWarning })
          : t("workbench-history-applied", { operation: label }),
        previewWarning ? "unsaved" : "restored",
      );
      return { ok: true, snapshot: receipt.workspace.history, receipt };
    } catch (error) {
      const label = direction === "undo" ? t("workbench-history-undo-label") : t("workbench-history-redo-label");
      const detail = errorMessage(error);
      const message = operationReceipt
        ? t("workbench-history-projection-failed", { operation: label, error: detail })
        : t("workbench-history-not-applied", { operation: label, error: detail });
      app.setGlobalStatus(
        message,
        "error",
      );
      await refreshTopbarKernelUndoRedoState();
      return { ok: false, message };
    } finally {
      if (frontendLeaseAcquired) app.endKernelUndoRedoFrontendLease();
      kernelUndoRedoInFlight = false;
    }
  }

  function kernelUndoRedoUiLeaseIsCurrent(lease: KernelUndoRedoProjectionLease) {
    return kernelUndoRedoProjectionLeaseMatches(app, lease);
  }

  function requireCurrentKernelUndoRedoUiLease(
    lease: KernelUndoRedoProjectionLease,
    operation: string,
  ) {
    if (!kernelUndoRedoUiLeaseIsCurrent(lease)) {
      throw new Error(t("workbench-history-session-changed", { operation }));
    }
  }

  async function syncAfterKernelUndoRedo(
    receipt: ProjectWorkspaceUndoRedoCommandReceipt,
    lease: KernelUndoRedoProjectionLease,
  ): Promise<string | null> {
    requireCurrentKernelUndoRedoUiLease(lease, t("workbench-history-lease-projection"));
    const entry = receipt.result.entry;
    for (const projection of receipt.result.documents) {
      requireCurrentKernelUndoRedoUiLease(lease, t("workbench-history-lease-documents"));
      rebaseFileBufferDraftSyncProjection(projection.relativePath, projection.snapshot);
      if (projection.snapshot) {
        applySourceTextFromKernelUndoRedo(projection.relativePath, projection.snapshot.text);
      } else {
        removeSourceTextAfterKernelUndoRedo(projection.relativePath);
      }
    }
    requireCurrentKernelUndoRedoUiLease(lease, t("workbench-history-lease-source"));
    if (entry.pageJsPaths.length > 0) app.jsRefreshToken += 1;
    if (entry.documentPaths.some((path) => /\.(?:css|scss)$/i.test(path))) {
      app.notifyCssSourceChanged();
    }
    await reconcileProjectWorkspaceTopologyAfterHistory(app, receipt, lease);
    requireCurrentKernelUndoRedoUiLease(lease, t("workbench-history-lease-topology"));
    // Inspectorul, CodeMirror și navigatorul trebuie să reflecte snapshot-ul
    // Rust chiar dacă proiecția iframe-ului este momentan indisponibilă.
    app.refreshToken += 1;
    try {
      await projectLatestProjectWorkspacePreview(app, {
        reason: "history-restore",
        minimumWorkspaceRevision: receipt.workspace.revision,
        requestedPaths: [...new Set([...entry.documentPaths, ...entry.pageJsPaths])].sort(),
      });
      requireCurrentKernelUndoRedoUiLease(lease, t("workbench-history-lease-preview"));
      return null;
    } catch (error) {
      requireCurrentKernelUndoRedoUiLease(lease, t("workbench-history-lease-preview-failure"));
      return errorMessage(error);
    }
  }

  function applySourceTextFromKernelUndoRedo(relativePath: string, text: string) {
    app.sourceCache = {
      ...app.sourceCache,
      [scannedCacheKey({ relativePath })]: text,
    };
    if (app.activeScannedPath === relativePath) {
      app.source = text;
    }
  }

  function removeSourceTextAfterKernelUndoRedo(relativePath: string) {
    const nextCache = { ...app.sourceCache };
    delete nextCache[scannedCacheKey({ relativePath })];
    app.sourceCache = nextCache;
    if (app.activeScannedPath === relativePath) {
      app.source = "";
    }
  }

  function errorMessage(error: unknown) {
    return error instanceof Error ? error.message : String(error);
  }

  function breakpointValue(name: string, fallback: string) {
    return app.scssVariables.find((variable) => variable.name === name)?.value || fallback;
  }

  async function undoFromShortcut() {
    await runTopbarUndoRedo("undo");
  }

  async function redoFromShortcut() {
    await runTopbarUndoRedo("redo");
  }

  function handleAppShortcuts(event: KeyboardEvent) {
    const intent = appShortcutIntent(event);
    if (app.aiEditLeaseFrontendLockActive || app.externalDiskState.reconciling || app.externalDiskState.workspaceProjectionRecoveryRequired) {
      if (intent !== "none") event.preventDefault();
      return;
    }
    if (intent === "none") return;
    event.preventDefault();
    if (intent === "commandCenter") openCommandCenter();
    else if (intent === "save") void app.saveActiveFile();
    else if (intent === "undo") void undoFromShortcut();
    else if (intent === "redo") void redoFromShortcut();
    else if (intent === "toggleTerminal") void app.toggleTerminalPane();
    else if (intent === "showProblems" && app.scannedProject) {
      void app.openAuditWorkspace("overview");
    }
    else if (intent === "toggleEditorSplit" && app.scannedProject) {
      void app.setSynchronizedWorkbenchSplit(
        app.workbenchSnapshot?.split === "none" ? "vertical" : "none",
      );
    }
    else if (intent === "togglePrimarySidebar" && app.scannedProject && editorSidebarsAvailable) {
      app.leftPaneCollapsed = !app.leftPaneCollapsed;
    }
  }

  function openCommandCenter() {
    if (
      app.aiEditLeaseFrontendLockActive
      || app.externalDiskState.reconciling
      || app.externalDiskState.workspaceProjectionRecoveryRequired
    ) return;
    commandCenterOpen = true;
  }

  function closeCommandCenter() {
    commandCenterOpen = false;
  }

  async function toggleInspectorFromCommandCenter() {
    if (!app.rightPaneCollapsed) {
      await app.flushInteractiveEditorDrafts("template-switch");
    }
    app.rightPaneCollapsed = !app.rightPaneCollapsed;
  }

  async function openCommandCenterDocument(
    relativePath: string,
    surface: WorkbenchSurface,
  ) {
    const candidatePaths = [relativePath];
    const file = app.scannedProject?.files.find(
      (candidate) => candidatePaths.includes(candidate.relativePath),
    );
    if (!file) {
      throw new Error(t("workbench-command-resource-missing", { path: relativePath }));
    }
    app.openProjectWorkbench();
    await app.loadScannedProjectFile(file);
    await app.setCenterView(
      surface === "code" ? "code" : surface === "markdown" ? "markdown" : "preview",
    );
    if (surface === "code") app.requestCodeSelectionReveal();
  }

  async function executeCommandCenterAction(action: CommandCenterAction) {
    closeCommandCenter();
    if (action.kind === "set_activity") {
      await selectWorkbenchActivity(action.activity);
      return;
    }
    if (action.kind === "open_document") {
      await openCommandCenterDocument(action.relativePath, action.surface);
      return;
    }

    switch (action.command) {
      case "open_project":
        await app.openProjectFolder();
        break;
      case "close_project":
        await app.closeCurrentProject();
        break;
      case "save":
        await app.saveActiveFile();
        break;
      case "undo":
        await runTopbarUndoRedo("undo");
        break;
      case "redo":
        await runTopbarUndoRedo("redo");
        break;
      case "validate":
        await app.runZolaValidation("manual");
        break;
      case "run_external":
        await app.openCurrentProjectInBrowser();
        break;
      case "refresh_session":
        await app.refreshCurrentSession();
        break;
      case "rescan_project":
        await app.rescanCurrentProject();
        break;
      case "toggle_terminal":
        await app.toggleTerminalPane();
        break;
      case "show_problems":
        await app.openAuditWorkspace("overview");
        break;
      case "show_output":
        await app.openAuditWorkspace("runtime", true);
        break;
      case "show_timeline":
        app.motionWorkspace.openTimeline();
        app.setPreviewExecutionMode("motion");
        break;
      case "split_vertical":
        await app.setSynchronizedWorkbenchSplit("vertical");
        break;
      case "split_horizontal":
        await app.setSynchronizedWorkbenchSplit("horizontal");
        break;
      case "close_split":
        await app.setSynchronizedWorkbenchSplit("none");
        break;
      case "canvas_fit":
        await app.setWorkbenchCanvasViewport({ mode: "fit", zoomPercent: 100 });
        break;
      case "canvas_desktop":
        await app.setWorkbenchCanvasViewport({ mode: "fixed", preset: "desktop", widthPx: 1_440 });
        break;
      case "canvas_tablet":
        await app.setWorkbenchCanvasViewport({ mode: "fixed", preset: "tablet", widthPx: 768 });
        break;
      case "canvas_mobile":
        await app.setWorkbenchCanvasViewport({ mode: "fixed", preset: "mobile", widthPx: 390 });
        break;
      case "toggle_left_sidebar":
        app.leftPaneCollapsed = !app.leftPaneCollapsed;
        break;
      case "toggle_inspector":
        await toggleInspectorFromCommandCenter();
        break;
      case "toggle_theme":
        app.toggleUiTheme();
        break;
      case "open_settings":
        app.openApplicationSettings();
        break;
      case "show_visual":
        await app.setCenterView("preview");
        break;
      case "show_code":
        await app.setCenterView("code");
        break;
      case "show_markdown":
        await app.setCenterView("markdown");
        break;
    }
  }

  async function selectWorkbenchActivity(activity: import("$lib/types").WorkbenchActivity) {
    try {
      await app.setWorkbenchActivity(activity);
      app.openProjectWorkbench();
      app.clearNotification("workbench.activity");
    } catch (error) {
      app.escalateGlobalStatus({
        id: "workbench.activity",
        level: "warning",
        title: t("workbench-activity-open-failed"),
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  function handleWindowMessage(event: MessageEvent) {
    const data = event.data;
    const userIntentLocked = app.aiEditLeaseFrontendLockActive
      || app.externalDiskState.reconciling
      || app.externalDiskState.workspaceProjectionRecoveryRequired;
    if (userIntentLocked && !isPreviewControlPlaneMessage(data)) return;
    if (
      data?.source === "pana-studio-preview"
      && data.type === "preview-shortcut"
      && isMessageFromExactPreviewFrame(app.previewFrame, event)
    ) {
      if (!app.previewRuntime.acceptIncomingMessage()) return;
      if (data.shortcut === "save") void app.saveActiveFile();
      else if (data.shortcut === "undo") void undoFromShortcut();
      else if (data.shortcut === "redo") void redoFromShortcut();
      return;
    }
    app.handlePreviewMessage(event);
  }

  function handleDeleteShortcut(event: KeyboardEvent) {
    if (app.aiEditLeaseFrontendLockActive || app.externalDiskState.reconciling || app.externalDiskState.workspaceProjectionRecoveryRequired) {
      event.preventDefault();
      return;
    }
    const intent = deleteShortcutIntent(event, {
      activeWorkbenchActivity: app.workbenchSnapshot?.activeActivity ?? "editor",
      applicationSurface: app.applicationSurface,
      centerView: app.centerView,
      selectionSnapshot: app.selectionSnapshot,
    });
    if (intent === "none") return;
    event.preventDefault();
    if (intent === "deleteSelectedTera") {
      void app.deleteSelectedTeraNode();
      return;
    }
    void app.deleteHtmlElement();
  }

  function handleVisualViewportChange() {
    resetNativeZoomIfVisualViewportChanged();
  }

  async function recoverExternalProjectionFromDisk() {
    if (externalRecoveryInFlight) return;
    externalRecoveryInFlight = true;
    try {
      await reloadAuthorizedAiReconciliationFromDisk(app);
    } finally {
      externalRecoveryInFlight = false;
    }
  }

  async function ensureTerminalPaneLoaded() {
    if (TerminalPaneComponent || terminalPaneLoading) return;
    terminalPaneLoading = true;
    try {
      TerminalPaneComponent = (await import("$lib/components/TerminalPane.svelte")).default;
    } finally {
      terminalPaneLoading = false;
    }
  }

  function hideBootScreen() {
    const bootScreen = document.getElementById("pana-boot-screen");
    if (!bootScreen) return;
    bootScreen.classList.add("is-hidden");
    window.setTimeout(() => bootScreen.remove(), 120);
  }

  async function revealApplication() {
    const bootScreen = document.getElementById("pana-boot-screen");
    if (bootScreen) {
      bootScreen.setAttribute("aria-label", t("application-loading-label"));
      const subtitle = bootScreen.querySelector<HTMLElement>(".boot-subtitle");
      if (subtitle) subtitle.textContent = t("application-loading-subtitle");
    }
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    hideBootScreen();
    try {
      await getCurrentWindow().show();
    } catch {
      // Browser-only development does not expose a native Tauri window.
    }
  }

  async function openWorkspaceSource(
    path: string,
    options: WorkspaceSourceOpenOptions = {},
  ) {
    const candidatePaths = [path];
    const file = app.scannedProject?.files.find((item) => candidatePaths.includes(item.relativePath));
    if (!file) {
      app.setGlobalStatus(t("workbench-file-not-scanned", { path }), "error");
      return;
    }
    await app.loadScannedProjectFile(file, {
      preferredTemplatePagePath: options.templateContextPagePath,
      preferredTemplateRoute: options.templateContextUrl,
    });
    await app.setCenterView(
      options.surface === "visual"
        ? "preview"
        : options.surface === "markdown"
          ? "markdown"
          : "code",
    );
  }

  $effect(() => {
    if (!app.scannedProject) {
      topbarKernelUndoRedoKey = "";
      topbarKernelUndoRedo = null;
      return;
    }
    const nextKey = [
      app.currentProjectPath,
      app.refreshToken,
      app.projectWorkspaceMutationEpoch,
    ].join(":");
    if (nextKey === topbarKernelUndoRedoKey || topbarKernelUndoRedoLoading) return;
    topbarKernelUndoRedoKey = nextKey;
    void refreshTopbarKernelUndoRedoState();
  });

  onMount(() => {
    const disposeSmoothWheelScrolling = installSmoothWheelScrolling(window);
    requestAnimationFrame(() => {
      window.setTimeout(() => {
        void app.initFromStorage(window.localStorage).finally(revealApplication);
      }, 0);
    });
    window.addEventListener("message", handleWindowMessage);
    window.addEventListener("keydown", handleAppShortcuts, { capture: true });
    window.addEventListener("keydown", handleDeleteShortcut, { capture: true });
    window.addEventListener("wheel", preventNativeZoomWheel, nativeZoomListenerOptions);
    window.addEventListener("gesturestart", preventNativeGestureZoom, nativeZoomListenerOptions);
    window.addEventListener("gesturechange", preventNativeGestureZoom, nativeZoomListenerOptions);
    window.addEventListener("gestureend", preventNativeGestureZoom, nativeZoomListenerOptions);
    window.visualViewport?.addEventListener("resize", handleVisualViewportChange);
    window.visualViewport?.addEventListener("scroll", handleVisualViewportChange);
    resetNativeWebviewZoom();
    return () => {
      disposeSmoothWheelScrolling();
      app.destroy();
      window.removeEventListener("message", handleWindowMessage);
      window.removeEventListener("keydown", handleAppShortcuts, { capture: true });
      window.removeEventListener("keydown", handleDeleteShortcut, { capture: true });
      window.removeEventListener("wheel", preventNativeZoomWheel, nativeZoomListenerOptions);
      window.removeEventListener("gesturestart", preventNativeGestureZoom, nativeZoomListenerOptions);
      window.removeEventListener("gesturechange", preventNativeGestureZoom, nativeZoomListenerOptions);
      window.removeEventListener("gestureend", preventNativeGestureZoom, nativeZoomListenerOptions);
      window.visualViewport?.removeEventListener("resize", handleVisualViewportChange);
      window.visualViewport?.removeEventListener("scroll", handleVisualViewportChange);
    };
  });

  $effect(() => {
    if (app.terminalPaneOpen) void ensureTerminalPaneLoaded();
  });
</script>

<svelte:head>
  <title>Pană Studio</title>
</svelte:head>

<main
  class:dark-theme={app.uiTheme === "dark"}
  class:light-theme={app.uiTheme === "light"}
  class:external-reconcile-lock={app.externalDiskState.reconciling || app.externalDiskState.workspaceProjectionRecoveryRequired}
  class:startup-active={!app.scannedProject && app.applicationSurface !== "settings"}
  class="app-shell"
  inert={app.externalDiskState.reconciling || app.externalDiskState.workspaceProjectionRecoveryRequired}
  aria-busy={app.externalDiskState.reconciling || app.externalDiskState.workspaceProjectionRecoveryRequired}
>
  {#if app.scannedProject || app.applicationSurface === "settings"}
    <AppChrome
      {app}
      topbarCanUndo={topbarUndoRedo.canUndo}
      topbarCanRedo={topbarUndoRedo.canRedo}
      undoAction={() => runTopbarUndoRedo("undo")}
      redoAction={() => runTopbarUndoRedo("redo")}
      {commandCenterOpen}
      {openCommandCenter}
      {closeCommandCenter}
      {executeCommandCenterAction}
    >

      <div class="workbench-frame">
        <ActivityRail
          activeActivity={app.workbenchSnapshot?.activeActivity ?? "editor"}
          disabled={!app.scannedProject}
          terminalOpen={app.applicationSurface === "workbench" && app.terminalPaneOpen}
          settingsActive={app.applicationSurface === "settings"}
          selectActivity={selectWorkbenchActivity}
          toggleTerminal={() => { void app.toggleTerminalPane(); }}
          selectSettings={() => {
            if (!app.scannedProject && app.applicationSurface === "settings") {
              app.openProjectWorkbench();
            } else {
              app.openApplicationSettings();
            }
          }}
        />
        <section
          class:left-pane-collapsed={app.leftPaneCollapsed}
          class:right-pane-collapsed={app.rightPaneCollapsed}
          class:center-workspace-active={!editorSidebarsAvailable}
          class="workspace"
          style={`--left-pane-width: ${app.leftPaneWidth}px; --right-pane-width: ${app.rightPaneWidth}px;`}
          aria-label={t("workbench-aria-label")}
        >
          <WorkspaceProjectArea {app} />

          <WorkspaceCenterArea
            {app}
            {TerminalPaneComponent}
            {breakpointValue}
            {openWorkspaceSource}
          />

          <WorkspaceInspectorArea {app} />
        </section>
      </div>
    </AppChrome>
  {:else}
    <StartupView {app} />
  {/if}

  <ProjectTransitionDecisionDialog
    request={app.projectTransitionDecisionRequest}
    confirm={(requestId: string, diagnostic: string) => app.confirmProjectTransitionOperatorDecision(requestId, diagnostic)}
    cancel={(requestId: string) => app.cancelProjectTransitionOperatorDecision(requestId)}
  />

  <ProjectOpenRecoveryDialog
    request={app.projectOpenRecoveryDecisionRequest}
    abandon={(requestId: string) => app.confirmProjectOpenRecoveryAbandonment(requestId)}
    cancel={(requestId: string) => app.cancelProjectOpenRecoveryDecision(requestId)}
  />
</main>

{#if app.externalDiskState.workspaceProjectionRecoveryRequired}
  <dialog open class="external-reconcile-recovery" aria-labelledby="external-reconcile-recovery-title">
    <strong id="external-reconcile-recovery-title">{t("workbench-external-recovery-title")}</strong>
    <p>{t("workbench-external-recovery-description")}</p>
    <button type="button" disabled={externalRecoveryInFlight} onclick={recoverExternalProjectionFromDisk}>
      {externalRecoveryInFlight ? t("workbench-external-recovery-loading") : t("workbench-external-recovery-action")}
    </button>
  </dialog>
{/if}
