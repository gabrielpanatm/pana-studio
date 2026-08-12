import { getEditableStylesFromObservation } from "$lib/css/matcher";
import { defaultSelectorForObservation } from "$lib/css/selectors";
import {
  deriveSelectionEditorState,
} from "$lib/preview/selection";
import type {
  CanvasElementObservation,
  CenterView,
  CoordinatedElementSelection,
  EditableAttributes,
  EditableStyles,
  SourceEditLocation,
} from "$lib/types";
import { htmlTextSelectionKey } from "$lib/state/html-draft-controller";

export type SelectionControllerHost = {
  coordinatedElementSelection: CoordinatedElementSelection | null;
  previewFrame: HTMLIFrameElement | undefined;
  pendingTag: string | null;
  pendingTagOriginal: string | null;
  pendingTagSourceLocation: SourceEditLocation | null;
  tagStatus: string;
  overrideRules: Record<string, EditableStyles>;
  variableOverrides: Record<string, string>;
  isActivePreviewHtmlSource: boolean;
  canEditHtml: boolean;
  htmlSourceMutationBlockedReason: string;
  classEditorValue: string;
  imageSourceValue: string;
  attributeValues: EditableAttributes;
  textContentValue: string;
  activeHtmlTextEditKey: string | null;
  activeHtmlTextEditValue: string | null;
  variableValues: Record<string, string>;
  classStatus: string;
  imageStatus: string;
  attributeStatus: string;
  textStatus: string;
  editableStyles: EditableStyles;
  centerView: CenterView;
  getPreviewDocument: () => Document | undefined;
  postPreviewMessage: (payload: Record<string, unknown>) => void;
  syncCodeSelectionHighlight: (reveal?: boolean) => void;
};

export function applySelectionState(
  host: SelectionControllerHost,
  selection: CanvasElementObservation,
  resolvedStyles?: EditableStyles,
) {
  host.pendingTag = null;
  host.pendingTagOriginal = null;
  host.pendingTagSourceLocation = null;
  host.tagStatus = "";
  const nextCssSelector = defaultSelectorForObservation(selection);
  const editorState = deriveSelectionEditorState(selection, {
    variableOverrides: host.variableOverrides,
    canEditHtmlSource: host.isActivePreviewHtmlSource,
    canEditSemanticSource: host.canEditHtml,
    blockedReason: host.htmlSourceMutationBlockedReason,
  });
  host.classEditorValue = editorState.classEditorValue;
  host.imageSourceValue = editorState.imageSourceValue;
  host.attributeValues = editorState.attributeValues;
  const activeSelectionKey = host.coordinatedElementSelection
    ? htmlTextSelectionKey(host.coordinatedElementSelection)
    : null;
  host.textContentValue = activeSelectionKey !== null
    && host.activeHtmlTextEditKey === activeSelectionKey
    && host.activeHtmlTextEditValue !== null
    ? host.activeHtmlTextEditValue
    : editorState.textContentValue;
  host.variableValues = editorState.variableValues;
  host.classStatus = editorState.classStatus;
  host.imageStatus = editorState.imageStatus;
  host.attributeStatus = editorState.attributeStatus;
  host.textStatus = editorState.textStatus;
  const existingOverride = host.overrideRules[nextCssSelector];
  host.editableStyles = existingOverride ?? resolvedStyles ?? getEditableStylesFromObservation(selection);
}
