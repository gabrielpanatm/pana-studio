import { SOURCE_LOADING_SENTINEL } from "$lib/editor-runtime/source-state";
import { ReactiveEffectsLifecycle } from "$lib/lifecycle/reactive-effects.svelte";
import type { ApplicationPreferencesState } from "$lib/application/preferences.svelte";
import type { ApplicationShellState } from "$lib/application/shell-state.svelte";
import type { CssAuthoringState } from "$lib/css/authoring-state.svelte";
import type { EditorReadModelState } from "$lib/editor/read-model.svelte";
import type { SelectionWorkspaceState } from "$lib/editor/selection-workspace.svelte";
import type { SourceWorkspaceState } from "$lib/editor/source-workspace.svelte";
import type { ProjectDocumentWorkspaceState } from "$lib/project/document-workspace.svelte";
import type { WorkbenchWorkspaceState } from "$lib/workbench/workspace-state.svelte";

export type CodeEditorLifecycleDependencies = {
  documents: Pick<ProjectDocumentWorkspaceState, "activeScannedPath">;
  readModel: Pick<EditorReadModelState, "canPreviewCurrentSource">;
  shell: Pick<ApplicationShellState, "centerView">;
  workbench: Pick<WorkbenchWorkspaceState, "snapshot">;
  source: SourceWorkspaceState;
  selection: SelectionWorkspaceState;
  css: Pick<CssAuthoringState, "targetFile">;
  mutationLocked: () => boolean;
};

/** Owns CodeMirror creation, synchronization, selection and mutation locks. */
export class CodeEditorLifecycle {
  private readonly effects: ReactiveEffectsLifecycle;

  constructor(
    dependencies: CodeEditorLifecycleDependencies,
    appearance: Pick<ApplicationPreferencesState, "accent" | "theme">,
  ) {
    const { documents, readModel, shell, workbench, source, selection, css } = dependencies;
    this.effects = new ReactiveEffectsLifecycle([
      () => {
        if (
          documents.activeScannedPath
          && !readModel.canPreviewCurrentSource
          && shell.centerView === "preview"
        ) {
          shell.centerView = "code";
        }
      },
      () => {
        const activeActivity = workbench.snapshot?.activeActivity ?? "editor";
        const secondaryGroup = workbench.snapshot?.groups.find(
          (group) => group.groupId === "secondary",
        );
        const secondaryDocument = secondaryGroup?.documents.find(
          (document) => document.documentId === secondaryGroup.activeDocumentId,
        );
        const splitSourceSurface = workbench.snapshot?.split !== "none"
          ? secondaryDocument?.surface ?? null
          : null;

        const codeEditorHost = source.hostElement;
        if (
          source.controller
          && (
            !codeEditorHost
            || !source.controller.ownsHost(codeEditorHost)
          )
        ) {
          source.controller.destroy();
          source.controller = null;
        }

        if (activeActivity !== "editor" || shell.centerView === "kernel") return;
        const codeSurfaceVisible = shell.centerView === "code" || splitSourceSurface === "code";
        if (!codeEditorHost || !codeSurfaceVisible) return;
        if (source.controller) {
          source.controller.requestMeasure();
          return;
        }
        void source.createEditor();
      },
      () => {
        if (!source.controller) return;
        source.controller.setLanguage(source.sourceLanguage);
      },
      () => {
        if (!source.controller) return;
        appearance.accent;
        source.controller.setTheme(appearance.theme);
      },
      () => {
        if (!source.controller) return;
        source.controller.setReadOnly(dependencies.mutationLocked());
      },
      () => {
        if (
          !source.controller
          || source.source === SOURCE_LOADING_SENTINEL
          || source.controller.getDoc() === source.source
        ) return;
        source.syncingSourceFromEditor = true;
        source.controller.setDoc(source.source);
        source.syncingSourceFromEditor = false;
      },
      () => {
        source.pendingBootstrapDiagnosticReveal;
        documents.activeScannedPath;
        source.source;
        source.controller;
        source.applyPendingBootstrapDiagnosticReveal();
      },
      () => {
        source.pendingSourceRangeReveal;
        documents.activeScannedPath;
        source.source;
        source.controller;
        source.applyPendingSourceRangeReveal();
      },
      () => {
        if (!source.controller) return;
        shell.centerView;
        source.source;
        source.sourceLanguage;
        source.currentSourceRelativePath;
        selection.session.selectionSnapshot;
        selection.coordinatedElement;
        selection.selectedTemplateSourceNode;
        selection.activeCssSelector;
        css.targetFile;
        source.codeSelectionRevealRequestId;
        source.syncSelectionHighlight(source.consumeSelectionRevealRequest());
      },
    ]);
  }

  start() {
    return this.effects.start();
  }

  stop() {
    return this.effects.stop();
  }
}
