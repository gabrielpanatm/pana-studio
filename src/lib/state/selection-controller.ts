import { getEditableStyles, getEditableStylesFromSelection } from "$lib/css/matcher";
import { defaultSelectorForSelection, selectorOptionsForSelection } from "$lib/css/selectors";
import { createDefaultEditableStyles } from "$lib/editor/defaults";
import { updatePreviewHtmlSelectionOverlay } from "$lib/preview/bridge";
import {
  createSelectionInfo,
  createSelectionInfoFromSourceElement,
  deriveSelectionEditorState,
  findSourceElementForSelection,
} from "$lib/preview/selection";
import type { CenterView, EditableAttributes, EditableStyles, SelectionInfo, SourceEditLocation } from "$lib/types";
import { htmlTextSelectionKey } from "$lib/state/html-draft-controller";

export type SelectionControllerHost = {
  selectedClass: string;
  previewFrame: HTMLIFrameElement | undefined;
  selectedPreviewElement: Element | null;
  selectedElement: SelectionInfo | null;
  pendingTag: string | null;
  pendingTagOriginal: string | null;
  pendingTagSourceLocation: SourceEditLocation | null;
  tagStatus: string;
  activeCssSelector: string;
  overrideRules: Record<string, EditableStyles>;
  variableOverrides: Record<string, string>;
  isActivePreviewHtmlSource: boolean;
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
  clearPreviewTeraSelection: () => void;
  syncCodeSelectionHighlight: (reveal?: boolean) => void;
};

export function applySelectionState(
  host: SelectionControllerHost,
  selection: SelectionInfo,
  resolvedStyles?: EditableStyles,
) {
  host.selectedElement = selection;
  host.clearPreviewTeraSelection();
  host.pendingTag = null;
  host.pendingTagOriginal = null;
  host.pendingTagSourceLocation = null;
  host.tagStatus = "";
  const nextSelectorOptions = selectorOptionsForSelection(selection);
  const nextCssSelector = nextSelectorOptions.some((o) => o.selector === host.activeCssSelector)
    ? host.activeCssSelector
    : defaultSelectorForSelection(selection);
  host.activeCssSelector = nextCssSelector;
  const editorState = deriveSelectionEditorState(selection, {
    variableOverrides: host.variableOverrides,
    canEditHtmlSource: host.isActivePreviewHtmlSource,
    blockedReason: host.htmlSourceMutationBlockedReason,
  });
  host.classEditorValue = editorState.classEditorValue;
  host.imageSourceValue = editorState.imageSourceValue;
  host.attributeValues = editorState.attributeValues;
  host.textContentValue = host.activeHtmlTextEditKey === htmlTextSelectionKey(selection)
    && host.activeHtmlTextEditValue !== null
    ? host.activeHtmlTextEditValue
    : editorState.textContentValue;
  host.variableValues = editorState.variableValues;
  host.classStatus = editorState.classStatus;
  host.imageStatus = editorState.imageStatus;
  host.attributeStatus = editorState.attributeStatus;
  host.textStatus = editorState.textStatus;
  const existingOverride = host.overrideRules[nextCssSelector];
  host.editableStyles = existingOverride ?? resolvedStyles ?? getEditableStylesFromSelection(selection);
}

export function setActiveCssSelector(host: SelectionControllerHost, selector: string) {
  host.activeCssSelector = selector;
  if (!host.selectedElement) {
    host.editableStyles = createDefaultEditableStyles();
    return;
  }
  host.editableStyles = host.overrideRules[selector] ?? getEditableStylesFromSelection(host.selectedElement);
}

export function selectPreviewElement(
  host: SelectionControllerHost,
  element: Element,
  options: { revealCode?: boolean } = {},
) {
  const previewDocument = host.getPreviewDocument();
  const previewWindow = element.ownerDocument.defaultView;
  if (!previewDocument || !previewWindow || element.ownerDocument !== previewDocument) return;
  host.selectedPreviewElement?.classList.remove(host.selectedClass);
  host.selectedPreviewElement = element;
  host.selectedPreviewElement.classList.add(host.selectedClass);
  updatePreviewHtmlSelectionOverlay(host.selectedPreviewElement);
  applySelectionState(
    host,
    createSelectionInfo(element, previewWindow, host.selectedClass),
    getEditableStyles(element, previewWindow),
  );
  host.syncCodeSelectionHighlight(options.revealCode === true);
}

export function selectDomNode(
  host: SelectionControllerHost,
  selector: string,
  options: { revealCode?: boolean } = {},
) {
  const previewDocument = host.getPreviewDocument();
  const element = previewDocument?.querySelector(selector);
  if (element) {
    selectPreviewElement(host, element, { revealCode: options.revealCode === true });
    return;
  }
  host.postPreviewMessage({ type: "select-by-selector", selector });
}

export function reconcileSelectionWithSourceDocument(
  host: SelectionControllerHost,
  document: Document,
  preferredSelector: string | null = null,
) {
  const target = findSourceElementForSelection(document, host.selectedElement, preferredSelector);
  if (!target) return;
  host.selectedPreviewElement = null;
  applySelectionState(
    host,
    createSelectionInfoFromSourceElement(target, host.selectedElement, host.selectedClass),
  );
}
