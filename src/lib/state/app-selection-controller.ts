import { applySelectionState as applySelectionStateFromController } from "$lib/state/selection-controller";
import {
  projectSelectionSnapshotOnCanvas,
  selectCanvasNavigationNode,
} from "$lib/state/canvas-interaction-controller";
import type { PreviewTeraSelectionTarget } from "$lib/state/app-helpers";
import type { AppState } from "$lib/state/app.svelte";
import { editorSourceReferenceLocation } from "$lib/source-provenance";
import type { EditableStyles, PageSection, CanvasElementObservation } from "$lib/types";
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
  const renderInstanceId = target.selector.match(
    /data-pana-render-instance-id=["']([^"']+)["']/,
  )?.[1] ?? null;
  const node = app.editorNavigationSnapshot?.nodes.find((candidate) =>
    candidate.kind === "teraBoundary"
    && (
      candidate.id === target.sourceId
      || candidate.sourceNodeId === target.sourceId
      || candidate.boundary?.sourceNodeId === target.sourceId
    )
    && (
      !renderInstanceId
      || candidate.boundary?.rootRenderInstanceIds.includes(renderInstanceId)
    )
  ) ?? null;
  if (!node) {
    app.setGlobalStatus(
      "Boundary-ul Tera nu există în EditorNavigationSnapshot-ul Rust curent.",
      "error",
    );
    return;
  }
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

export function selectTeraLayerSource(
  app: AppState,
  _section: PageSection,
  sourceId: string,
) {
  const node = app.editorNavigationSnapshot?.nodes.find(
    (candidate) => candidate.kind === "teraBoundary"
      && candidate.boundary?.sourceNodeId === sourceId,
  ) ?? null;
  if (!node) {
    app.setGlobalStatus(
      "Boundary-ul nu există în EditorNavigationSnapshot-ul Rust curent.",
      "error",
    );
    return;
  }
  selectCanvasNavigationNode(app, node);
}
