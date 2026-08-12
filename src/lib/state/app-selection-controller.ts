import { applySelectionState as applySelectionStateFromController } from "$lib/state/selection-controller";
import {
  projectSelectionSnapshotOnCanvas,
} from "$lib/state/canvas-interaction-controller";
import type { PreviewTeraSelectionTarget } from "$lib/state/app-helpers";
import type { AppState } from "$lib/state/app.svelte";
import { editorSourceReferenceLocation } from "$lib/source-provenance";
import type { EditableStyles, CanvasElementObservation } from "$lib/types";
import { t } from "$lib/i18n/runtime.svelte";

export function clearPreviewSelection(
  app: AppState,
  options: { clearCanvasOverlay?: boolean } = {},
) {
  app.acceptedSelectionObservation = null;
  app.inspectorSelectionSummary = null;
  if (app.activeCanvasIdentity) {
    void app.applySelectionIntent({ kind: "clearSelection" });
  }
  if (options.clearCanvasOverlay) {
    app.postPreviewMessage({ type: "clear-canvas-interaction-overlays" });
  }
}

export function setPreviewTeraSelection(
  app: AppState,
  target: PreviewTeraSelectionTarget,
  options: { status?: string } = {},
) {
  const candidates = app.editorNavigationSnapshot?.nodes.filter((candidate) =>
    candidate.kind === "teraBoundary"
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
    app.setGlobalStatus(
      candidates.length > 1
        ? "Boundary-ul Tera este ambiguu în EditorNavigationSnapshot-ul Rust curent."
        : "Boundary-ul Tera nu există în EditorNavigationSnapshot-ul Rust curent.",
      "error",
    );
    return;
  }
  const node = candidates[0];
  void app.applySelectionIntent({
    kind: "selectEditorNode",
    editorNodeId: node.id,
  }).then((selection) => {
    if (selection) projectSelectionSnapshotOnCanvas(app, selection);
  });
  if (options.status) {
    app.setGlobalStatus(options.status, "idle");
  }
}

export function applySelectionState(
  app: AppState,
  selection: CanvasElementObservation,
  resolvedStyles?: EditableStyles,
) {
  applySelectionStateFromController(app.selectionControllerHost(), selection, resolvedStyles);
}

export async function openSelectedTeraSource(app: AppState) {
  const provenance = app.selectedEditorNavigationNode?.sourceProvenance;
  const source = provenance?.definition ?? provenance?.composition;
  if (!source?.canOpenInCode) {
    app.setGlobalStatus(t("inspector-tera-select-node"), "error");
    return;
  }
  await app.openSourceLocation(editorSourceReferenceLocation(source));
  await app.setCenterView("code");
  app.requestCodeSelectionReveal();
}

export async function openSelectedMarkdownContent(app: AppState) {
  const navigationNode = app.selectedEditorNavigationNode;
  const source = navigationNode?.sourceProvenance.definition;
  const relativePath = source?.file.replaceAll("\\", "/").replace(/^\.\//, "") ?? "";
  if (
    navigationNode?.kind !== "markdownBoundary"
    || navigationNode.sourceProvenance.resolution !== "resolved"
    || !relativePath.startsWith("content/")
    || !relativePath.toLocaleLowerCase().endsWith(".md")
  ) {
    app.setGlobalStatus(t("markdown-boundary-unresolved"), "error");
    return;
  }
  await app.openContentPageEditor(relativePath);
}
