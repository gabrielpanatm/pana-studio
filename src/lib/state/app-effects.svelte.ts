import type { AppState } from "$lib/state/app.svelte";
import { scheduleAiContextSnapshot as scheduleAiContextSnapshotFromController } from "$lib/state/ai-context-controller";
import { registerNativeWindowCloseGuard } from "$lib/state/native-window-close-controller";
import { savePaneDimensions } from "$lib/ui/preferences";
import { readProjectWorkspaceState } from "$lib/project/io";
import { subscribeProjectWorkspaceMutations } from "$lib/kernel/project-workspace-events";
import { applyApplicationAppearanceToPreviewDocument } from "$lib/preview/bridge";
import { contrastingTextColor } from "$lib/state/app-helpers";
import { synchronizeCanvasInteractionBinding } from "$lib/state/canvas-interaction-controller";
import {
  projectWorkspacePreviewRevisionIsPublished,
  scheduleProjectWorkspaceDerivedPreviewProjection,
} from "$lib/kernel/project-workspace-preview-coordinator";
import { t } from "$lib/i18n/runtime.svelte";

const TERMINAL_SESSION_VERSION = 6;

export function registerAppEffects(app: AppState) {
  // Rebind the same physical document whenever Rust publishes a new Canvas
  // identity (for example after an in-place structural commit).
  $effect(() => {
    app.previewFrame;
    app.activeCanvasUrl;
    app.previewSrc;
    app.browserPreviewRoute;
    app.applicationSurface;
    app.workbenchSnapshot?.activeActivity;
    app.centerView;
    app.activeScannedPath;
    app.editorNavigationSnapshot?.identity.transactionId;
    app.editorNavigationSnapshot?.identity.previewRevision;
    app.editorNavigationSnapshot?.route;
    app.editorNavigationSnapshot?.focusedView?.activeDocumentPath;
    app.activeCanvasIdentity?.projectRoot;
    app.activeCanvasIdentity?.runtimeSessionId;
    app.activeCanvasIdentity?.workspaceRevision;
    app.activeCanvasIdentity?.transactionId;
    app.activeCanvasIdentity?.previewRevision;
    synchronizeCanvasInteractionBinding(app);
  });

  // Project the Rust-owned File Explorer only after both authoritative
  // Workspace and Workbench mirrors identify the same live session.
  $effect(() => {
    const projectRoot = app.sessionProjectRoot;
    const sessionId = app.kernelProjectSessionId;
    const workspaceRevision = app.projectWorkspaceSnapshot?.revision ?? null;
    const workbenchRevision = app.workbenchSnapshot?.revision ?? null;
    app.aiCoordinationSnapshot?.coordinationRevision;
    if (
      !projectRoot
      || !sessionId
      || workspaceRevision === null
      || workbenchRevision === null
    ) {
      app.fileExplorerSnapshot = null;
      app.fileExplorerLoading = false;
      app.fileExplorerError = "";
      return;
    }
    const timer = window.setTimeout(() => {
      void app.refreshFileExplorerSnapshot();
    }, 24);
    return () => window.clearTimeout(timer);
  });

  $effect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    void subscribeProjectWorkspaceMutations((notice) => {
      if (
        notice.projectRoot === app.sessionProjectRoot
        && notice.runtimeSessionId === app.kernelProjectSessionId
      ) {
        const workspaceAlreadyVisible = (
          app.projectWorkspaceSnapshot?.projectRoot === notice.projectRoot
          && app.projectWorkspaceSnapshot.runtimeSessionId === notice.runtimeSessionId
          && app.projectWorkspaceSnapshot.revision >= notice.workspaceRevision
        );
        const previewAlreadyVisible = !notice.previewProjectionRequired
          || projectWorkspacePreviewRevisionIsPublished(
            notice.projectRoot,
            notice.runtimeSessionId,
            notice.workspaceRevision,
          );
        if (!workspaceAlreadyVisible) app.markProjectWorkspaceMutation();
        if (notice.previewProjectionRequired && !previewAlreadyVisible) {
          scheduleProjectWorkspaceDerivedPreviewProjection(
            app,
            "workspace-mutation",
            notice.workspaceRevision,
          );
        }
      }
    }).then((cleanup) => {
      if (disposed) cleanup();
      else unlisten = cleanup;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  });

  // Restore the Rust-owned navigation projection for the active ProjectSession.
  $effect(() => {
    const projectRoot = app.sessionProjectRoot;
    const sessionId = app.kernelProjectSessionId;
    if (!projectRoot || !sessionId) {
      app.workbenchSnapshot = null;
      return;
    }
    let cancelled = false;
    const timer = window.setTimeout(() => {
      void app.refreshWorkbenchState().catch((error) => {
        if (cancelled) return;
        app.workbenchSnapshot = null;
        app.escalateGlobalStatus({
          id: "workbench.restore",
          level: "warning",
          title: t("workbench-restore-failed-title"),
          message: error instanceof Error ? error.message : String(error),
        });
      });
    }, 40);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  });

  // Keep a read-only UI mirror of the single Rust authority. The serial makes
  // slower reads unable to overwrite a newer workspace revision/session.
  $effect(() => {
    const projectRoot = app.sessionProjectRoot;
    const sessionId = app.kernelProjectSessionId;
    app.projectWorkspaceMutationEpoch;
    app.saveRequest;
    if (!projectRoot || !sessionId) {
      app.projectWorkspaceSnapshot = null;
      return;
    }
    let cancelled = false;
    const timer = window.setTimeout(() => {
      void readProjectWorkspaceState()
        .then((snapshot) => {
          if (
            cancelled
            || app.sessionProjectRoot !== projectRoot
            || app.kernelProjectSessionId !== sessionId
          ) return;
          if (
            snapshot?.projectRoot === projectRoot
            && snapshot.runtimeSessionId === sessionId
          ) {
            app.projectWorkspaceSnapshot = snapshot;
          }
        })
        .catch(() => {
          // Păstrăm ultimul snapshot Rust confirmat. Un read derivat temporar
          // indisponibil nu poate șterge autoritatea deja publicată în UI.
        });
    }, 40);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  });

  // Route native window close through ProjectTransitionPolicy while a project session is open.
  $effect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    void registerNativeWindowCloseGuard(app).then((cleanup) => {
      if (disposed) {
        cleanup();
        return;
      }
      unlisten = cleanup;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  });

  // Auto-switch to code view if preview is not available.
  $effect(() => {
    if (!app.canPreviewCurrentSource && app.centerView === "preview") {
      app.centerView = "code";
    }
  });

  // Markdown view is only valid for Markdown sources; Code remains available for raw editing.
  $effect(() => {
    if (app.centerView === "markdown" && app.sourceLanguage !== "markdown") {
      app.centerView = "code";
    }
  });

  // Create code editor when host is ready.
  $effect(() => {
    const activeActivity = app.workbenchSnapshot?.activeActivity ?? "editor";
    const secondaryGroup = app.workbenchSnapshot?.groups.find(
      (group) => group.groupId === "secondary",
    );
    const secondaryDocument = secondaryGroup?.documents.find(
      (document) => document.documentId === secondaryGroup.activeDocumentId,
    );
    const splitSourceSurface = app.workbenchSnapshot?.split !== "none"
      ? secondaryDocument?.surface ?? null
      : null;

    const codeEditorHost = app.codeEditorHost;
    if (
      app.codeEditorController
      && (
        !codeEditorHost
        || !app.codeEditorController.ownsHost(codeEditorHost)
      )
    ) {
      app.codeEditorController?.destroy();
      app.codeEditorController = null;
    }

    // Workspace activity changes only suspend the stable Editor owner. The
    // CodeMirror instance remains bound to the same ProjectSession host and is
    // destroyed above only when Svelte replaces/removes that host.
    if (activeActivity !== "editor" || app.centerView === "kernel") {
      return;
    }
    if (app.centerView === "markdown" || splitSourceSurface === "markdown") {
      app.codeEditorController?.destroy();
      app.codeEditorController = null;
      return;
    }
    const codeSurfaceVisible = app.centerView === "code" || splitSourceSurface === "code";
    if (!codeEditorHost || !codeSurfaceVisible) return;
    if (app.codeEditorController) {
      app.codeEditorController.requestMeasure();
      return;
    }
    void app.createCodeEditor();
  });

  // Sync code editor language.
  $effect(() => {
    if (!app.codeEditorController) return;
    app.codeEditorController.setLanguage(app.sourceLanguage);
  });

  // Sync code editor theme.
  $effect(() => {
    if (!app.codeEditorController) return;
    // Theme CSS references the root accent variables. Tracking the accent here
    // also refreshes WebKit versions which cache CodeMirror theme declarations.
    app.uiAccent;
    app.codeEditorController.setTheme(app.uiTheme);
  });

  // Keep same-origin and bridged preview surfaces on the same application
  // appearance authority. Frame/navigation dependencies re-publish after load.
  $effect(() => {
    const accent = app.uiAccent;
    app.previewFrame;
    app.previewSrc;
    app.previewReloadSerial;
    app.previewDocumentMarkup;
    const textOnAccent = contrastingTextColor(accent);
    const previewDocument = app.getPreviewDocument();
    if (previewDocument) {
      applyApplicationAppearanceToPreviewDocument(previewDocument, accent, textOnAccent);
    }
    app.postPreviewMessage({
      type: "set-application-appearance",
      accent,
      textOnAccent,
    });
  });

  // Freeze source ingress while a project transition or kernel history
  // transaction owns the frontend mutation boundary.
  $effect(() => {
    if (!app.codeEditorController) return;
    app.codeEditorController.setReadOnly(
      app.projectTransitionFrontendLeaseActive
        || app.kernelUndoRedoFrontendQuiesceActive
        || app.kernelUndoRedoFrontendLeaseActive
        || app.aiEditLeaseFrontendLockActive,
    );
  });

  // Sync source text to code editor.
  $effect(() => {
    if (!app.codeEditorController || app.codeEditorController.getDoc() === app.source) return;
    app.syncingSourceFromEditor = true;
    app.codeEditorController.setDoc(app.source);
    app.syncingSourceFromEditor = false;
  });

  // Sync code selection highlight.
  $effect(() => {
    if (!app.codeEditorController) return;
    app.centerView;
    app.source;
    app.sourceLanguage;
    app.currentSourceRelativePath;
    app.selectionSnapshot;
    app.coordinatedElementSelection;
    app.selectedTemplateSourceNode;
    app.activeCssSelector;
    app.targetCssFile;
    app.codeSelectionRevealRequestId;
    app.syncCodeSelectionHighlight(app.consumeCodeSelectionRevealRequest());
  });

  // Render terminal.
  $effect(() => {
    void app.terminalController.render({
      paneOpen: app.terminalPaneOpen,
      tab: app.activeTerminalTab,
      host: app.terminalHost,
      theme: app.uiTheme,
      accent: app.uiAccent,
      cwd: app.currentProjectPath,
    });
  });

  // Terminal session version reset.
  $effect(() => {
    if (app.appliedTerminalSessionRuntimeVersion === TERMINAL_SESSION_VERSION) return;
    app.terminalController.destroyAll();
    app.appliedTerminalSessionRuntimeVersion = TERMINAL_SESSION_VERSION;
    if (app.terminalPaneOpen && app.terminalHost && app.activeTerminalTab) {
      void app.terminalController.render({
        paneOpen: app.terminalPaneOpen,
        tab: app.activeTerminalTab,
        host: app.terminalHost,
        theme: app.uiTheme,
        accent: app.uiAccent,
        cwd: app.currentProjectPath,
      });
    }
  });

  // Save pane dimensions.
  $effect(() => {
    app.leftPaneWidth;
    app.rightPaneWidth;
    app.terminalPaneHeight;
    if (typeof window === "undefined") return;
    savePaneDimensions(window.localStorage, {
      leftPaneWidth: app.leftPaneWidth,
      rightPaneWidth: app.rightPaneWidth,
      terminalPaneHeight: app.terminalPaneHeight,
    });
  });

  // Publish lightweight read-only context for AI CLI sessions.
  $effect(() => {
    app.currentProjectPath;
    app.activeScannedPath;
    app.activePreviewPath;
    app.centerView;
    app.previewDevice;
    app.sourceLanguage;
    app.selectionSnapshot;
    app.coordinatedElementSelection;
    app.activeCssSelector;
    app.targetCssFile;
    app.scssVariables.length;
    app.globalDirtyState.dirty;
    app.globalDirtyState.canSave;
    app.globalDirtyState.areas.join(",");
    app.externalDiskState.changed;
    app.externalDiskState.changedFiles.join(",");
    app.externalDiskState.blockedByDirtySession;
    app.externalDiskState.lastDetectedAt;
    app.externalDiskState.lastAppliedAt;
    app.externalDiskState.lastAppliedFiles.join(",");
    app.externalDiskState.reconciling;
    app.externalDiskState.workspaceProjectionRecoveryRequired;
    app.externalDiskState.truncated;
    scheduleAiContextSnapshotFromController(app.aiContextControllerHost());
  });
}
