import type { CssAuthoringState } from "$lib/css/authoring-state.svelte";
import type { ApplicationShellState } from "$lib/application/shell-state.svelte";
import type { SelectionWorkspaceState } from "$lib/editor/selection-workspace.svelte";
import type { SourceWorkspaceState } from "$lib/editor/source-workspace.svelte";
import { pageJsRelativePath } from "$lib/js/page-path";
import {
  primarySelectionEntry,
  selectionResolution,
} from "$lib/kernel/selection-read-model";
import type {
  ProjectWorkspacePreviewProjectionOptions,
  ProjectWorkspacePreviewProjectionOutcome,
} from "$lib/kernel/project-workspace-preview-coordinator";
import type { PreviewWorkspaceState } from "$lib/preview/workspace-state.svelte";
import type { SelectionMutationIdentity } from "$lib/preview/contracts";
import type { ProjectDocumentWorkspaceState } from "$lib/project/document-workspace.svelte";
import { zolaRelativePath } from "$lib/project/files";
import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import type { InspectorTab } from "$lib/application/contracts";
import type { ProjectFile } from "$lib/project/lifecycle-contract";
import type { HistoryOperationState } from "$lib/versioning/history-operation-state.svelte";
import type { WorkbenchNavigationService } from "$lib/workbench/navigation-service";
import type { WorkbenchWorkspaceState } from "$lib/workbench/workspace-state.svelte";
import { errorMessage } from "$lib/util";
import { t } from "$lib/i18n/runtime.svelte";
import { sameCssSemanticSelection } from "$lib/inspector/css-selection-stability";

function selectionMutationIdentity(
  snapshot: import("$lib/editor/contracts").SelectionSnapshot | null,
): SelectionMutationIdentity | null {
  if (!snapshot?.primaryMemberId || snapshot.members.length === 0) return null;
  return {
    selectionRevision: snapshot.selectionRevision,
    workspaceRevision: snapshot.canvasIdentity.workspaceRevision,
    primaryMemberId: snapshot.primaryMemberId,
    members: snapshot.members.map((member) => ({
      memberId: member.memberId,
      editorNodeId: member.anchor.editorNodeId ?? null,
      sourceNodeId: member.anchor.sourceNodeId ?? null,
      renderInstanceId: member.anchor.renderInstanceId ?? null,
    })),
  };
}

export type SourceNavigationServiceDependencies = Readonly<{
  project: ProjectSessionState;
  documents: ProjectDocumentWorkspaceState;
  source: SourceWorkspaceState;
  css: CssAuthoringState;
  selection: SelectionWorkspaceState;
  preview: PreviewWorkspaceState;
  workbench: WorkbenchNavigationService;
  workbenchState: WorkbenchWorkspaceState;
  shell: ApplicationShellState;
  history: HistoryOperationState;
  status: GlobalStatusState;
  projectLatestPreview: (
    options: ProjectWorkspacePreviewProjectionOptions<"manual">,
  ) => Promise<ProjectWorkspacePreviewProjectionOutcome>;
  loadFile: (file: ProjectFile) => Promise<void>;
}>;

/** Owns navigation between Inspector focus, semantic selection and source code. */
export class SourceNavigationService {
  private readonly dependencies: SourceNavigationServiceDependencies;
  private cssFocusIntentSequence = 0;

  constructor(dependencies: SourceNavigationServiceDependencies) {
    this.dependencies = dependencies;
  }

  setCssRevealTarget(target: { selector: string; file: string }) {
    if (!target.selector || !target.file) return;
    this.dependencies.css.targetFile = target.file;
    this.dependencies.source.setCssRevealTarget(target);
  }

  async selectCssFocus(target: {
    selector: string;
    file: string;
    property?: string | null;
    expectedSelectionRevision?: number | null;
    expectedSelection?: SelectionMutationIdentity | null;
  }): Promise<boolean> {
    if (!target.selector || !target.file || this.historyLocked()) return false;
    const intentSequence = ++this.cssFocusIntentSequence;
    let expectedSelectionRevision = target.expectedSelectionRevision ?? null;
    let expectedSelection = target.expectedSelection ?? null;
    const selectionSession = this.dependencies.selection.session;
    if (expectedSelection) {
      if (!sameCssSemanticSelection(
        expectedSelection,
        selectionMutationIdentity(selectionSession.selectionSnapshot),
      )) return false;
    } else if (
      expectedSelectionRevision
      && selectionSession.selectionSnapshot?.selectionRevision !== expectedSelectionRevision
    ) return false;
    const property = target.property?.trim() || null;
    try {
      const expectedWorkspaceRevision = this.dependencies.project.workspace?.revision ?? null;
      if (
        expectedWorkspaceRevision !== null
        && this.dependencies.preview.activeIdentity?.workspaceRevision !== expectedWorkspaceRevision
      ) {
        const previousAnchor = primarySelectionEntry(selectionSession.selectionSnapshot)?.anchor ?? null;
        const outcome = await this.dependencies.projectLatestPreview({
          reason: "manual",
          minimumWorkspaceRevision: expectedWorkspaceRevision,
          force: true,
        });
        if (
          (outcome.status !== "published" && outcome.status !== "already_current")
          || this.dependencies.project.workspace?.revision !== expectedWorkspaceRevision
          || this.dependencies.preview.activeIdentity?.workspaceRevision !== expectedWorkspaceRevision
        ) return false;
        await selectionSession.refreshNavigationSnapshot(
          this.dependencies.preview.activeIdentity,
          this.dependencies.preview.activeUrl || this.dependencies.preview.src,
          { strict: true },
        );
        const currentSelection = selectionSession.selectionSnapshot;
        const currentAnchor = primarySelectionEntry(currentSelection)?.anchor ?? null;
        if (
          !currentSelection
          || selectionResolution(currentSelection) !== "resolved"
          || !previousAnchor
          || !currentAnchor
        ) return false;
        const stableAnchorMatches = Boolean(
          (previousAnchor.editorNodeId && currentAnchor.editorNodeId === previousAnchor.editorNodeId)
          || (previousAnchor.sourceNodeId && currentAnchor.sourceNodeId === previousAnchor.sourceNodeId),
        );
        if (!stableAnchorMatches) return false;
        expectedSelectionRevision = currentSelection.selectionRevision;
        expectedSelection = selectionMutationIdentity(currentSelection);
      }

      const selection = await selectionSession.applySelectionIntent({
        kind: "setFocus",
        focus: property
          ? {
              kind: "cssProperty",
              selector: target.selector,
              file: target.file,
              property,
              viewport: this.dependencies.workbenchState.previewDevice,
            }
          : {
              kind: "cssRule",
              selector: target.selector,
              file: target.file,
              viewport: this.dependencies.workbenchState.previewDevice,
            },
        expectedSelectionRevision,
        expectedSelection,
        intentSequence,
      });
      if (
        !selection
        || (selection.focus.kind !== "cssRule" && selection.focus.kind !== "cssProperty")
        || selection.focus.file !== target.file
        || selection.focus.selector !== target.selector
      ) return false;
      return property
        ? selection.focus.kind === "cssProperty" && selection.focus.property === property
        : selection.focus.kind === "cssRule";
    } catch (error) {
      if (
        this.historyLocked()
        || intentSequence !== this.cssFocusIntentSequence
        || (expectedSelectionRevision
          && selectionSession.selectionSnapshot?.selectionRevision !== expectedSelectionRevision)
      ) return false;
      this.dependencies.status.set(
        `${t("inspector-css-focus-blocked")} ${errorMessage(error)}`,
        "error",
      );
      return false;
    }
  }

  selectInspectorTab(tab: InspectorTab) {
    this.dependencies.shell.inspectorTab = tab;
    const selection = this.dependencies.selection.session;
    if (tab === "html") {
      if (selection.selectionSnapshot?.focus.kind !== "element") {
        void selection.applySelectionIntent({ kind: "setFocus", focus: { kind: "element" } });
      }
      return;
    }
    if (tab !== "js") return;
    const provenance = primarySelectionEntry(selection.selectionSnapshot)?.provenance;
    const templatePath = provenance?.definition?.file
      ?? provenance?.composition?.file
      ?? this.dependencies.documents.activeRenderedTemplatePath;
    if (templatePath) this.selectJsBehavior(pageJsRelativePath(templatePath));
  }

  async openCssSource(target: { selector: string; file: string }) {
    const project = this.dependencies.project.project;
    if (!project || !target.selector || !target.file) return;
    this.setCssRevealTarget(target);
    const targetPath = zolaRelativePath(target.file);
    const file = project.files.find((item) => (
      item.relativePath === target.file || zolaRelativePath(item.relativePath) === targetPath
    ));
    if (file && this.dependencies.documents.activeScannedPath !== file.relativePath) {
      await this.dependencies.loadFile(file);
    }
    await this.dependencies.workbench.setCenterView("code");
    this.dependencies.source.requestSelectionReveal();
  }

  private selectJsBehavior(file: string) {
    const selection = this.dependencies.selection.session;
    const focus = selection.selectionSnapshot?.focus;
    if (focus?.kind === "jsBehavior" && focus.file === file && focus.behaviorId === null) return;
    void selection.applySelectionIntent({
      kind: "setFocus",
      focus: { kind: "jsBehavior", file, behaviorId: null },
    });
  }

  private historyLocked() {
    return this.dependencies.history.quiesceActive || this.dependencies.history.leaseActive;
  }

}
