import type { AppState } from "$lib/state/app.svelte";
import { scheduleAiContextSnapshot as scheduleAiContextSnapshotFromController } from "$lib/state/ai-context-controller";
import { registerNativeWindowCloseGuard } from "$lib/state/native-window-close-controller";
import { savePaneDimensions } from "$lib/ui/preferences";
import { readProjectWorkspaceState } from "$lib/project/io";
import { subscribeProjectWorkspaceMutations } from "$lib/kernel/project-workspace-events";
import { scheduleProjectWorkspaceDerivedPreviewProjection } from "$lib/kernel/project-workspace-preview-coordinator";

const TERMINAL_SESSION_VERSION = 6;

export function registerAppEffects(app: AppState) {
  $effect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    void subscribeProjectWorkspaceMutations((notice) => {
      if (
        notice.projectRoot === app.sessionProjectRoot
        && notice.runtimeSessionId === app.kernelProjectSessionId
      ) {
        app.markProjectWorkspaceMutation();
        if (notice.previewProjectionRequired) {
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
        app.notify({
          id: "workbench.restore",
          level: "warning",
          title: "Workbench nu a fost restaurat",
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
          app.projectWorkspaceSnapshot = snapshot?.projectRoot === projectRoot
            && snapshot.runtimeSessionId === sessionId
            ? snapshot
            : null;
        })
        .catch(() => {
          if (!cancelled) app.projectWorkspaceSnapshot = null;
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
    const secondaryGroup = app.workbenchSnapshot?.groups.find(
      (group) => group.groupId === "secondary",
    );
    const secondaryDocument = secondaryGroup?.documents.find(
      (document) => document.documentId === secondaryGroup.activeDocumentId,
    );
    const splitSourceSurface = app.workbenchSnapshot?.split !== "none"
      ? secondaryDocument?.surface ?? null
      : null;
    if (
      (app.workbenchSnapshot?.activeActivity ?? "editor") !== "editor"
      || app.centerView === "kernel"
    ) {
      app.codeEditorController?.destroy();
      app.codeEditorController = null;
      app.codeEditorHost = undefined;
      return;
    }
    if (app.centerView === "markdown" || splitSourceSurface === "markdown") {
      app.codeEditorController?.destroy();
      app.codeEditorController = null;
      app.codeEditorHost = undefined;
      return;
    }
    const codeSurfaceVisible = app.centerView === "code" || splitSourceSurface === "code";
    if (!app.codeEditorHost || !codeSurfaceVisible) return;
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
    app.codeEditorController.setTheme(app.uiTheme);
  });

  // Freeze source ingress while a project transition or kernel history
  // transaction owns the frontend mutation boundary.
  $effect(() => {
    if (!app.codeEditorController) return;
    app.codeEditorController.setReadOnly(
      app.projectTransitionFrontendLeaseActive
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
    app.selectedElement;
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
    app.selectedElement;
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
    app.externalDiskState.lastCheckedAt;
    app.externalDiskState.checking;
    app.externalDiskState.reconciling;
    app.externalDiskState.workspaceProjectionRecoveryRequired;
    app.externalDiskState.truncated;
    scheduleAiContextSnapshotFromController(app.aiContextControllerHost());
  });
}
