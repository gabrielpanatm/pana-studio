import { getEditableStylesFromObservation } from "$lib/css/matcher";
import { defaultSelectorForObservation } from "$lib/css/selectors";
import type { EditableStyles } from "$lib/css/contracts";
import {
  deriveSelectionEditorState,
} from "$lib/preview/selection";
import type {
  CanvasElementObservation,
  CoordinatedElementSelection,
} from "$lib/canvas/contracts";
import {
  htmlTextSelectionKey,
  type HtmlDraftState,
} from "$lib/state/html-draft-session.svelte";
import type { HtmlAuthoringState } from "$lib/editor/html-authoring-state.svelte";
import type { CssAuthoringState } from "$lib/css/authoring-state.svelte";

export type SelectionControllerHost = {
  context: Readonly<{
    coordinatedSelection: CoordinatedElementSelection | null;
    activePreviewHtmlSource: boolean;
    canEditHtml: boolean;
    mutationBlockedReason: string;
  }>;
  html: Pick<
    HtmlAuthoringState,
    | "pendingTag"
    | "pendingTagOriginal"
    | "pendingTagSourceLocation"
    | "tagStatus"
    | "classEditorValue"
    | "imageSourceValue"
    | "classStatus"
    | "imageStatus"
  >;
  css: Pick<
    CssAuthoringState,
    "overrideRules" | "variableOverrides" | "variableValues" | "editableStyles"
  >;
  draft: HtmlDraftState;
};

export function applySelectionState(
  host: SelectionControllerHost,
  selection: CanvasElementObservation,
  resolvedStyles?: EditableStyles,
) {
  host.html.pendingTag = null;
  host.html.pendingTagOriginal = null;
  host.html.pendingTagSourceLocation = null;
  host.html.tagStatus = "";
  const nextCssSelector = defaultSelectorForObservation(selection);
  const editorState = deriveSelectionEditorState(selection, {
    variableOverrides: host.css.variableOverrides,
    canEditHtmlSource: host.context.activePreviewHtmlSource,
    canEditSemanticSource: host.context.canEditHtml,
    blockedReason: host.context.mutationBlockedReason,
  });
  host.html.classEditorValue = editorState.classEditorValue;
  host.html.imageSourceValue = editorState.imageSourceValue;
  host.draft.attributeValues = editorState.attributeValues;
  const activeSelectionKey = host.context.coordinatedSelection
    ? htmlTextSelectionKey(host.context.coordinatedSelection)
    : null;
  host.draft.textContentValue = activeSelectionKey !== null
    && host.draft.activeTextEditKey === activeSelectionKey
    && host.draft.activeTextEditValue !== null
    ? host.draft.activeTextEditValue
    : editorState.textContentValue;
  host.css.variableValues = editorState.variableValues;
  host.html.classStatus = editorState.classStatus;
  host.html.imageStatus = editorState.imageStatus;
  host.draft.attributeStatus = editorState.attributeStatus;
  host.draft.textStatus = editorState.textStatus;
  const existingOverride = host.css.overrideRules[nextCssSelector];
  host.css.editableStyles = existingOverride ?? resolvedStyles ?? getEditableStylesFromObservation(selection);
}
