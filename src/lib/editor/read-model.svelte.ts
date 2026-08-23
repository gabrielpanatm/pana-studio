import {
  deriveAppDirtyState,
  deriveCanAddChildToSelectedElement,
  deriveCanEditHtml,
  deriveCanPreviewCurrentSource,
  deriveHtmlSourceMutationBlockedReason,
} from "$lib/state/app-derived";
import type { AiCoordinationState } from "$lib/ai/coordination-state.svelte";
import type { ExternalDiskState } from "$lib/session/external-disk-state.svelte";
import type { HtmlAuthoringState } from "$lib/editor/html-authoring-state.svelte";
import type { ProjectDocumentWorkspaceState } from "$lib/project/document-workspace.svelte";
import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { SelectionWorkspaceState } from "$lib/editor/selection-workspace.svelte";
import type { SourceWorkspaceState } from "$lib/editor/source-workspace.svelte";
import { t } from "$lib/i18n/runtime.svelte";

export type EditorReadModelDependencies = {
  project: ProjectSessionState;
  documents: ProjectDocumentWorkspaceState;
  source: SourceWorkspaceState;
  html: HtmlAuthoringState;
  selection: SelectionWorkspaceState;
  ai: AiCoordinationState;
  externalDisk: ExternalDiskState;
};

/** Read-only editor capabilities and dirty state derived from domain owners. */
export class EditorReadModelState {
  private readonly dependencies: EditorReadModelDependencies;

  constructor(dependencies: EditorReadModelDependencies) {
    this.dependencies = dependencies;
  }

  get globalDirtyState() {
    return deriveAppDirtyState({
      projectWorkspaceSnapshot: this.dependencies.project.workspace,
      htmlPending: this.dependencies.html.htmlPending,
      inspectorPending: this.dependencies.html.inspectorPending,
    });
  }

  get sessionHasPending() { return this.globalDirtyState.dirty; }
  get inspectorHasPending() { return this.globalDirtyState.dirty; }
  get saveHasPending() { return this.globalDirtyState.canSave; }

  get canEditHtml() {
    return deriveCanEditHtml({
      isActivePreviewHtmlSource: this.dependencies.source.isActivePreviewHtmlSource,
      selectedSourceEditTarget: this.dependencies.selection.selectedSourceEditTarget,
      selectedSemanticSourceLocation: this.dependencies.selection.selectedSemanticSourceLocation,
    });
  }

  get canAddChildToSelectedElement() {
    return deriveCanAddChildToSelectedElement({
      editorSelection: this.dependencies.selection.session,
    });
  }

  get canPreviewCurrentSource() {
    return deriveCanPreviewCurrentSource({
      activeScannedPath: this.dependencies.documents.activeScannedPath,
      sourceLanguage: this.dependencies.source.sourceLanguage,
      activeTemplateFile: this.dependencies.documents.activeTemplateFile,
    });
  }

  get htmlSourceMutationBlockedReason() {
    return deriveHtmlSourceMutationBlockedReason({
      activeScannedPath: this.dependencies.documents.activeScannedPath,
    });
  }

  get immediateDiskOperationBlockedReason() {
    if (this.dependencies.ai.frontendLockActive) {
      return t("workbench-source-operations-ai-blocked");
    }
    if (this.dependencies.externalDisk.snapshot.workspaceProjectionRecoveryRequired) {
      return t("workbench-disk-operations-projection-blocked");
    }
    return this.globalDirtyState.immediateDiskOperationBlockedReason;
  }
}
