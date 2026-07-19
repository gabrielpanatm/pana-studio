<script lang="ts">
  import "./workspace-shell.css";
  import type { Component } from "svelte";
  import type { TerminalPaneProps } from "$lib/components/TerminalPane.svelte";
  import AppChrome from "$lib/components/workspace/AppChrome.svelte";
  import AiEditAuthorityIndicator from "$lib/components/ai/AiEditAuthorityIndicator.svelte";
  import ProjectOpenRecoveryDialog from "$lib/components/project/ProjectOpenRecoveryDialog.svelte";
  import ProjectTransitionDecisionDialog from "$lib/components/project/ProjectTransitionDecisionDialog.svelte";
  import WorkspaceCenterArea from "$lib/components/workspace/WorkspaceCenterArea.svelte";
  import WorkspaceInspectorArea from "$lib/components/workspace/WorkspaceInspectorArea.svelte";
  import WorkspaceProjectArea from "$lib/components/workspace/WorkspaceProjectArea.svelte";
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
  import {
    selectTopbarUndoRedoRoute,
    topbarUndoRedoState,
    type TopbarUndoRedoDirection,
  } from "$lib/ui/undo-redo-router";
  import type {
    ProjectWorkspaceSnapshot,
    ProjectWorkspaceUndoRedoCommandReceipt,
  } from "$lib/types";
  import { onMount } from "svelte";

  type ProjectWorkspaceUndoRedoOutcome =
    | {
        ok: true;
        snapshot: ProjectWorkspaceSnapshot["history"];
        receipt: ProjectWorkspaceUndoRedoCommandReceipt;
      }
    | { ok: false; message: string };

  const app = new AppState();
  let statusSourceLabel = $state("");
  let statusSourceValue = $state("");
  let statusSourceOpenable = $state(false);
  let TerminalPaneComponent = $state<Component<TerminalPaneProps> | null>(null);
  let terminalPaneLoading = false;
  let topbarKernelUndoRedo = $state<ProjectWorkspaceSnapshot | null>(null);
  let topbarKernelUndoRedoKey = "";
  let topbarKernelUndoRedoLoading = $state(false);
  let kernelUndoRedoInFlight = false;
  let externalRecoveryInFlight = $state(false);

  const topbarUndoRedo = $derived(topbarUndoRedoState({
    kernelCanUndo: Boolean(topbarKernelUndoRedo?.history.canUndo),
    kernelCanRedo: Boolean(topbarKernelUndoRedo?.history.canRedo),
  }));

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
      app.setGlobalStatus(`Istoricul ProjectWorkspace nu a putut fi citit: ${errorMessage(error)}`, "error");
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
      const message = "O operație Undo/Redo ProjectWorkspace este deja în curs.";
      return { ok: false, message };
    }

    const lease: KernelUndoRedoProjectionLease = {
      expectedProjectRoot: app.sessionProjectRoot,
      expectedSessionId: app.kernelProjectSessionId,
      expectedSessionEpoch: app.projectSessionEpoch,
    };
    if (!lease.expectedProjectRoot || !lease.expectedSessionId) {
      const message = "Undo/Redo ProjectWorkspace necesită o sesiune activă și identificată.";
      return { ok: false, message };
    }

    kernelUndoRedoInFlight = true;
    let frontendLeaseAcquired = false;
    let operationReceipt: ProjectWorkspaceUndoRedoCommandReceipt | null = null;
    try {
      await app.beginKernelUndoRedoFrontendLease();
      frontendLeaseAcquired = true;
      requireCurrentKernelUndoRedoUiLease(lease, "bariera frontend Undo/Redo");
      const before = await refreshTopbarKernelUndoRedoState();
      requireCurrentKernelUndoRedoUiLease(lease, "citirea istoricului ProjectWorkspace");
      const target = direction === "undo" ? before?.history.nextUndo : before?.history.nextRedo;
      if (!before || !target) {
        const message = direction === "undo"
          ? "ProjectWorkspace nu mai are o mutație disponibilă pentru undo."
          : "ProjectWorkspace nu mai are o mutație disponibilă pentru redo.";
        return { ok: false, message };
      }

      app.setGlobalStatus(
        direction === "undo" ? "Se aplică undo în sesiune..." : "Se aplică redo în sesiune...",
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
      requireCurrentKernelUndoRedoUiLease(lease, "receipt-ul Undo/Redo");
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
      topbarKernelUndoRedo = receipt.workspace;
      const previewWarning = await syncAfterKernelUndoRedo(receipt, lease);
      const label = direction === "undo" ? "Undo" : "Redo";
      app.setGlobalStatus(
        previewWarning
          ? `${label} aplicat în sesiune. Preview-ul va fi resincronizat: ${previewWarning}`
          : `${label} aplicat în sesiune.`,
        previewWarning ? "unsaved" : "restored",
      );
      return { ok: true, snapshot: receipt.workspace.history, receipt };
    } catch (error) {
      const label = direction === "undo" ? "Undo" : "Redo";
      const detail = errorMessage(error);
      const message = operationReceipt
        ? `${label} a schimbat sesiunea, dar proiecția interfeței a eșuat: ${detail} Reîncarcă proiecția aceleiași revizii.`
        : `${label} nu a fost aplicat: ${detail}`;
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
      throw new Error(`${operation} a fost invalidată de schimbarea ProjectSession.`);
    }
  }

  async function syncAfterKernelUndoRedo(
    receipt: ProjectWorkspaceUndoRedoCommandReceipt,
    lease: KernelUndoRedoProjectionLease,
  ): Promise<string | null> {
    requireCurrentKernelUndoRedoUiLease(lease, "proiecția Undo/Redo");
    const entry = receipt.result.entry;
    for (const projection of receipt.result.documents) {
      requireCurrentKernelUndoRedoUiLease(lease, "proiecția documentelor Undo/Redo");
      rebaseFileBufferDraftSyncProjection(projection.relativePath, projection.snapshot);
      if (projection.snapshot) {
        applySourceTextFromKernelUndoRedo(projection.relativePath, projection.snapshot.text);
      } else {
        removeSourceTextAfterKernelUndoRedo(projection.relativePath);
      }
    }
    requireCurrentKernelUndoRedoUiLease(lease, "proiecția source Undo/Redo");
    if (entry.pageJsPaths.length > 0) app.jsRefreshToken += 1;
    if (entry.documentPaths.some((path) => /\.(?:css|scss)$/i.test(path))) {
      app.notifyCssSourceChanged();
    }
    await reconcileProjectWorkspaceTopologyAfterHistory(app, receipt, lease);
    requireCurrentKernelUndoRedoUiLease(lease, "reconcilierea topologiei Undo/Redo");
    // Inspectorul, CodeMirror și navigatorul trebuie să reflecte snapshot-ul
    // Rust chiar dacă proiecția iframe-ului este momentan indisponibilă.
    app.refreshToken += 1;
    try {
      await projectLatestProjectWorkspacePreview(app, {
        reason: "history-restore",
        minimumWorkspaceRevision: receipt.workspace.revision,
        requestedPaths: [...new Set([...entry.documentPaths, ...entry.pageJsPaths])].sort(),
      });
      requireCurrentKernelUndoRedoUiLease(lease, "proiecția Preview după Undo/Redo");
      return null;
    } catch (error) {
      requireCurrentKernelUndoRedoUiLease(lease, "eșecul proiecției Preview după Undo/Redo");
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

  function setStatusSourceContext(context: { label: string; value: string; openable?: boolean } | null) {
    statusSourceLabel = context?.label ?? "";
    statusSourceValue = context?.value ?? "";
    statusSourceOpenable = Boolean(context?.openable && context.value);
  }

  async function openStatusSource() {
    if (!statusSourceOpenable || !statusSourceValue || !app.scannedProject) return;
    if ((statusSourceLabel.startsWith("SCSS") || statusSourceLabel.startsWith("CSS")) && app.activeCssSelector) {
      await app.openCssCodeRevealTarget({
        selector: app.activeCssSelector,
        file: statusSourceValue,
      });
      return;
    }
    if (statusSourceValue.includes(":")) {
      await app.openSourceLocation(statusSourceValue);
      await app.setCenterView("code");
      app.requestCodeSelectionReveal();
      return;
    }
    const file = app.scannedProject.files.find((item) => item.relativePath === statusSourceValue);
    if (file) {
      await app.loadScannedProjectFile(file);
      await app.setCenterView("code");
      app.requestCodeSelectionReveal();
    }
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
    if (intent === "save") void app.saveActiveFile();
    else if (intent === "undo") void undoFromShortcut();
    else if (intent === "redo") void redoFromShortcut();
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
      centerView: app.centerView,
      selectedElement: app.selectedElement,
      settingsPanelOpen: app.settingsPanelOpen,
    });
    if (intent !== "deleteSelectedHtml") return;
    event.preventDefault();
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

  async function openWorkspaceSource(path: string) {
    const file = app.scannedProject?.files.find((item) => item.relativePath === path);
    if (!file) {
      app.setGlobalStatus(`Fișierul nu este în scanarea proiectului: ${path}`, "error");
      return;
    }
    await app.loadScannedProjectFile(file);
    await app.setCenterView("code");
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
    requestAnimationFrame(() => {
      hideBootScreen();
      window.setTimeout(() => app.initFromStorage(window.localStorage), 0);
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
  class="app-shell"
  inert={app.externalDiskState.reconciling || app.externalDiskState.workspaceProjectionRecoveryRequired}
  aria-busy={app.externalDiskState.reconciling || app.externalDiskState.workspaceProjectionRecoveryRequired}
>
  <AppChrome
    {app}
    topbarCanUndo={topbarUndoRedo.canUndo}
    topbarCanRedo={topbarUndoRedo.canRedo}
    undoAction={() => runTopbarUndoRedo("undo")}
    redoAction={() => runTopbarUndoRedo("redo")}
    {statusSourceLabel}
    {statusSourceValue}
    {statusSourceOpenable}
    {openStatusSource}
  >

  <section
    class:left-pane-collapsed={app.leftPaneCollapsed}
    class:right-pane-collapsed={app.rightPaneCollapsed}
    class:center-workspace-active={app.centerView === "site" || app.centerView === "kernel"}
    class="workspace"
    style={`--left-pane-width: ${app.leftPaneWidth}px; --right-pane-width: ${app.rightPaneWidth}px;`}
    aria-label="Pană Studio workspace"
  >
    <WorkspaceProjectArea {app} />

    <WorkspaceCenterArea
      {app}
      {TerminalPaneComponent}
      {breakpointValue}
      {openWorkspaceSource}
    />

    <WorkspaceInspectorArea {app} {setStatusSourceContext} />
  </section>
  </AppChrome>

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

<AiEditAuthorityIndicator
  snapshot={app.aiCoordinationSnapshot}
  externalReconciling={app.externalDiskState.reconciling}
  projectionRecoveryRequired={app.externalDiskState.workspaceProjectionRecoveryRequired}
/>

{#if app.externalDiskState.workspaceProjectionRecoveryRequired}
  <dialog open class="external-reconcile-recovery" aria-labelledby="external-reconcile-recovery-title">
    <strong id="external-reconcile-recovery-title">Reproiectare necesară</strong>
    <p>Starea confirmată pe disk și interfața nu mai sunt sincronizate. Editarea și scrierea sunt blocate până la reîncărcare, pentru a preveni pierderea datelor.</p>
    <button type="button" disabled={externalRecoveryInFlight} onclick={recoverExternalProjectionFromDisk}>
      {externalRecoveryInFlight ? "Se reîncarcă..." : "Reîncarcă sigur de pe disk"}
    </button>
  </dialog>
{/if}
