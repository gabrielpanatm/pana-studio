import {
  applySelectionState as applySelectionStateFromController,
  type SelectionControllerHost,
} from "$lib/state/selection-controller";
import {
  projectSelectionSnapshotOnCanvas,
  type CanvasInteractionControllerHost,
} from "$lib/state/canvas-interaction-controller";
import type { PreviewTeraSelectionTarget } from "$lib/state/app-helpers";
import { editorSourceReferenceLocation } from "$lib/source-provenance";
import type { EditableStyles } from "$lib/css/contracts";
import type { CanvasElementObservation } from "$lib/canvas/contracts";
import type { EditorNavigationNode } from "$lib/editor/contracts";
import { t } from "$lib/i18n/runtime.svelte";

export type AppSelectionControllerHost = {
  canvasInteraction: CanvasInteractionControllerHost;
  selectedEditorNavigationNode: EditorNavigationNode | null;
  selectionProjection: SelectionControllerHost;
  openSourceLocation: (source: string) => Promise<unknown>;
  setCenterView: (view: "code") => Promise<unknown>;
  requestCodeSelectionReveal: () => void;
  openContentPageEditor: (relativePath: string) => Promise<unknown>;
};

export function clearPreviewSelection(
  app: AppSelectionControllerHost,
  options: { clearCanvasOverlay?: boolean } = {},
) {
  app.canvasInteraction.selection.editorSelection.clearSelectionProjection();
  if (app.canvasInteraction.session.activeCanvasIdentity) {
    void app.canvasInteraction.selection.editorSelection.applySelectionIntent({ kind: "clearSelection" });
  }
  if (options.clearCanvasOverlay) {
    app.canvasInteraction.commands.postPreviewMessage({ type: "clear-canvas-interaction-overlays" });
  }
}

export function setPreviewTeraSelection(
  app: AppSelectionControllerHost,
  target: PreviewTeraSelectionTarget,
  options: { status?: string } = {},
) {
  const candidates = app.canvasInteraction.selection.editorSelection.navigationSnapshot?.nodes.filter((candidate) =>
    candidate.kind === "boundary"
    && candidate.boundary?.kind !== "markdown"
    && (
      candidate.id === target.sourceId
      || candidate.sourceNodeId === target.sourceId
      || candidate.boundary?.sourceNodeId === target.sourceId
    )
    && (
      !target.renderInstanceId
      || candidate.boundary?.rootRenderInstanceIds.includes(target.renderInstanceId)
    )
  ) ?? [];
  if (candidates.length !== 1) {
    app.canvasInteraction.commands.setGlobalStatus(
      candidates.length > 1
        ? "Boundary-ul semantic este ambiguu în EditorNavigationSnapshot-ul Rust curent."
        : "Boundary-ul semantic nu există în EditorNavigationSnapshot-ul Rust curent.",
      "error",
    );
    return;
  }
  const node = candidates[0];
  void app.canvasInteraction.selection.editorSelection.applySelectionIntent({
    kind: "selectEditorNode",
    editorNodeId: node.id,
  }).then((selection) => {
    if (selection) projectSelectionSnapshotOnCanvas(app.canvasInteraction, selection);
  });
  if (options.status) {
    app.canvasInteraction.commands.setGlobalStatus(options.status, "idle");
  }
}

export function applySelectionState(
  app: AppSelectionControllerHost,
  selection: CanvasElementObservation,
  resolvedStyles?: EditableStyles,
) {
  applySelectionStateFromController(app.selectionProjection, selection, resolvedStyles);
}

export async function openSelectedTeraSource(app: AppSelectionControllerHost) {
  const provenance = app.selectedEditorNavigationNode?.sourceProvenance;
  const source = provenance?.definition ?? provenance?.composition;
  if (!source?.canOpenInCode) {
    app.canvasInteraction.commands.setGlobalStatus(t("inspector-tera-select-node"), "error");
    return;
  }
  await app.openSourceLocation(editorSourceReferenceLocation(source));
  await app.setCenterView("code");
  app.requestCodeSelectionReveal();
}

export async function openSelectedMarkdownContent(app: AppSelectionControllerHost) {
  const navigationNode = app.selectedEditorNavigationNode;
  const source = navigationNode?.sourceProvenance.definition;
  const relativePath = source?.file.replaceAll("\\", "/").replace(/^\.\//, "") ?? "";
  if (
    navigationNode?.kind !== "boundary"
    || navigationNode.boundary?.kind !== "markdown"
    || navigationNode.sourceProvenance.resolution !== "resolved"
    || !relativePath.startsWith("content/")
    || !relativePath.toLocaleLowerCase().endsWith(".md")
  ) {
    app.canvasInteraction.commands.setGlobalStatus(t("markdown-boundary-unresolved"), "error");
    return;
  }
  await app.openContentPageEditor(relativePath);
}
