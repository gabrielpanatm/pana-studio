import type { CanvasInteractionControllerHost } from "$lib/state/canvas-interaction-host";
import {
  projectSelectionSnapshotOnCanvas,
  selectCanvasPreviewElement,
} from "$lib/state/canvas-interaction-controller";
import {
  applySelectionState as applySelectionStateFromController,
  type SelectionControllerHost,
} from "$lib/state/selection-controller";
import {
  clearPreviewSelection,
  openSelectedMarkdownContent,
  openSelectedTeraSource,
  setPreviewTeraSelection,
  type AppSelectionControllerHost,
} from "$lib/state/app-selection-controller";
import type { EditorReadModelState } from "$lib/editor/read-model.svelte";
import type { SelectionWorkspaceState } from "$lib/editor/selection-workspace.svelte";
import type { SourceWorkspaceState } from "$lib/editor/source-workspace.svelte";
import type { HtmlAuthoringState } from "$lib/editor/html-authoring-state.svelte";
import type { CssAuthoringState } from "$lib/css/authoring-state.svelte";
import type { EditorInteractionRuntime } from "$lib/editor/interaction-runtime.svelte";
import type { HtmlEditingService } from "$lib/editor/html-editing-service";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import type { PreviewTeraSelectionTarget } from "$lib/state/app-helpers";
import type { EditableStyles } from "$lib/css/contracts";
import type { CanvasElementObservation } from "$lib/canvas/contracts";
import type { SelectionSnapshot } from "$lib/editor/contracts";
import type { EditorHtmlTarget } from "$lib/editor-runtime/commands";

export type EditorSelectionServiceDependencies = Readonly<{
  selection: SelectionWorkspaceState;
  canvas: CanvasInteractionControllerHost;
  readModel: EditorReadModelState;
  source: SourceWorkspaceState;
  html: HtmlAuthoringState;
  css: CssAuthoringState;
  editor: () => EditorInteractionRuntime;
  htmlEditing: () => HtmlEditingService;
  status: GlobalStatusState;
  viewport: () => "desktop" | "tablet" | "mobile";
  setCenterView: (view: "code") => Promise<unknown>;
  openContentPage: (relativePath: string) => Promise<unknown>;
}>;

/** Owns selection projection between the Rust snapshot, Canvas, inspector and code. */
export class EditorSelectionService {
  private readonly dependencies: EditorSelectionServiceDependencies;

  constructor(dependencies: EditorSelectionServiceDependencies) {
    this.dependencies = dependencies;
  }

  private projectionHost(): SelectionControllerHost {
    const { selection, source, readModel, html, css } = this.dependencies;
    return {
      context: {
        coordinatedSelection: selection.coordinatedElement,
        activePreviewHtmlSource: source.isActivePreviewHtmlSource,
        canEditHtml: readModel.canEditHtml,
        mutationBlockedReason: readModel.htmlSourceMutationBlockedReason,
      },
      html,
      css,
      draft: this.dependencies.editor().htmlDraft,
    };
  }

  private commands(): AppSelectionControllerHost {
    const { canvas, selection, source } = this.dependencies;
    return {
      canvasInteraction: canvas,
      selectedEditorNavigationNode: selection.selectedEditorNavigationNode,
      selectionProjection: this.projectionHost(),
      openSourceLocation: (location) => this.dependencies.htmlEditing().openSource(location),
      setCenterView: this.dependencies.setCenterView,
      requestCodeSelectionReveal: () => source.requestSelectionReveal(),
      openContentPageEditor: this.dependencies.openContentPage,
    };
  }

  clear(options: { clearCanvasOverlay?: boolean } = {}) {
    clearPreviewSelection(this.commands(), options);
  }

  setTera(
    target: PreviewTeraSelectionTarget,
    options: { status?: string } = {},
  ) {
    setPreviewTeraSelection(this.commands(), target, options);
  }

  apply(observation: CanvasElementObservation, resolvedStyles?: EditableStyles) {
    applySelectionStateFromController(this.projectionHost(), observation, resolvedStyles);
  }

  projectOnCanvas(
    selection: SelectionSnapshot,
    options: { revealCode?: boolean } = {},
  ) {
    projectSelectionSnapshotOnCanvas(this.dependencies.canvas, selection, options);
  }

  async openSelectedTeraSource() {
    await openSelectedTeraSource(this.commands());
  }

  async openSelectedMarkdownContent() {
    await openSelectedMarkdownContent(this.commands());
  }

  selectPreviewElement(element: Element, options: { revealCode?: boolean } = {}) {
    this.dependencies.source.setHtmlRevealTarget();
    if (!selectCanvasPreviewElement(this.dependencies.canvas, element, options)) {
      this.dependencies.status.set(
        "Ținta nu există în EditorNavigationSnapshot-ul Rust curent.",
        "error",
      );
    }
  }

  selectHtmlTarget(target: EditorHtmlTarget, options: { revealCode?: boolean } = {}) {
    this.dependencies.source.setHtmlRevealTarget();
    const renderInstanceId = target.renderInstanceId ?? null;
    const sourceNodeId = target.sourceId ?? null;
    const candidates = this.dependencies.selection.session.navigationSnapshot?.nodes.filter((node) => (
      node.kind === "htmlElement"
      && (renderInstanceId
        ? node.renderInstanceId === renderInstanceId
        : sourceNodeId
          ? node.sourceNodeId === sourceNodeId
          : false)
    )) ?? [];
    if (candidates.length !== 1) {
      this.dependencies.status.set(
        candidates.length > 1
          ? "Ținta HTML este ambiguă în EditorNavigationSnapshot-ul Rust curent."
          : "Ținta HTML nu există în EditorNavigationSnapshot-ul Rust curent.",
        "error",
      );
      return;
    }
    void this.dependencies.selection.session.applySelectionIntent({
      kind: "selectEditorNode",
      editorNodeId: candidates[0].id,
    }).then((selection) => {
      if (!selection) return;
      projectSelectionSnapshotOnCanvas(this.dependencies.canvas, selection, options);
      if (options.revealCode) this.dependencies.source.requestSelectionReveal();
    });
  }

  async selectSourcePosition(file: string, offset: number) {
    const selection = await this.dependencies.selection.session.applySelectionIntent({
      kind: "selectSourcePosition",
      file,
      offset,
      viewport: this.dependencies.viewport(),
    });
    if (selection) {
      projectSelectionSnapshotOnCanvas(this.dependencies.canvas, selection, { revealCode: false });
    }
  }
}
