import type { CssAuthoringState } from "$lib/css/authoring-state.svelte";
import type { EditorInteractionRuntime } from "$lib/editor/interaction-runtime.svelte";
import type { HtmlAuthoringState } from "$lib/editor/html-authoring-state.svelte";
import type { SelectionWorkspaceState } from "$lib/editor/selection-workspace.svelte";
import type { PreviewWorkspaceState } from "$lib/preview/workspace-state.svelte";

export type ExternalReconcileEditorResetDependencies = Readonly<{
  editor: EditorInteractionRuntime;
  html: HtmlAuthoringState;
  css: CssAuthoringState;
  selection: SelectionWorkspaceState;
  preview: PreviewWorkspaceState;
}>;

/** Clears frontend-only edit projections after Rust accepts an external reconcile. */
export async function resetEditorAfterExternalReconcile(
  dependencies: ExternalReconcileEditorResetDependencies,
) {
  dependencies.editor.htmlDraft.cancel();
  dependencies.html.mutationRevision += 1;
  dependencies.css.overrideRules = {};
  dependencies.css.variableOverrides = {};
  dependencies.css.liveLayers = {};
  dependencies.css.liveEpoch = dependencies.css.liveEpoch >= Number.MAX_SAFE_INTEGER
    ? 1
    : dependencies.css.liveEpoch + 1;
  dependencies.css.liveIdentity = null;
  dependencies.css.variableValues = {};
  dependencies.html.resetPending();
  dependencies.selection.session.clearSelectionProjection();
  dependencies.preview.postMessage({ type: "clear-canvas-interaction-overlays" });
}
