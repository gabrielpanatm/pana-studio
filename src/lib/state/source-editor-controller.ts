import type { CodeEditorContextMenuRequest, CodeEditorController } from "$lib/editor/controller";
import {
  codeSelectionRangeForCssSelector,
  codeSelectionRangeForSourceRange,
  cssSelectorAtPosition,
} from "$lib/editor/source-ranges";
import { htmlVoidTags } from "$lib/html/mutations";
import { projectRelativeZolaPath, scannedCacheKey, zolaRelativePath } from "$lib/project/files";
import {
  queueFileBufferDraftChangeSetForPath,
  queueFileBufferDraftTextTransitionForPath,
} from "$lib/session/file-buffer-draft-sync";
import {
  codeSelectionRangeForSelection,
  findPreviewElementForMarkdownTarget,
  resolveCodeCursorSelectionAction,
  resolveHtmlSourceSelectionContext,
} from "$lib/preview/selection";
import type { SaveState, SelectionInfo, SourceGraphNode, SourceLanguage, SourceNodeRange } from "$lib/types";

export type SourceEditorControllerHost = {
  codeEditorHost: HTMLDivElement | undefined;
  codeEditorController: CodeEditorController | null;
  source: string;
  sourceLanguage: SourceLanguage;
  uiTheme: "dark" | "light";
  projectTransitionFrontendLeaseActive: boolean;
  kernelUndoRedoFrontendLeaseActive: boolean;
  aiEditLeaseFrontendLockActive: boolean;
  syncingSourceFromEditor: boolean;
  syncingSelectionFromCode: boolean;
  sourceCache: Record<string, string>;
  currentSourceCacheKey: string;
  currentSourceRelativePath: string;
  currentSourcePath: string;
  selectedElement: SelectionInfo | null;
  selectedTemplateSourceNode: SourceGraphNode | null;
  activeCssSelector: string;
  targetCssFile: string;
  activeScannedPath: string | null;
  htmlSourceNodes: SourceNodeRange[];
  isActivePreviewHtmlSource: boolean;
  setGlobalStatus: (text: string, kind: SaveState) => void;
  reconcileSelectionWithSourceDocument: (document: Document, preferredSelector?: string | null) => void;
  syncHtmlCodeToPreview: (sourceText: string, cursorPosition: number) => void;
  selectDomNode: (selector: string, options?: { revealCode?: boolean }) => void;
  selectCssSelectorFromCode: (target: { selector: string; file: string }) => void;
  notifyCssSourceChanged: () => void;
  getPreviewDocument: () => Document | undefined;
  postPreviewMessage: (payload: Record<string, unknown>) => void;
  selectPreviewElement: (element: Element, options?: { revealCode?: boolean }) => void;
  openCodeEditorContextMenu?: (request: CodeEditorContextMenuRequest) => void;
};

export async function createSourceEditor(host: SourceEditorControllerHost) {
  if (!host.codeEditorHost || host.codeEditorController) return;
  const { createCodeEditorController } = await import("$lib/editor/controller");
  if (!host.codeEditorHost || host.codeEditorController) return;
  host.codeEditorController = createCodeEditorController({
    host: host.codeEditorHost,
    doc: host.source,
    language: host.sourceLanguage,
    theme: host.uiTheme,
    readOnly: host.projectTransitionFrontendLeaseActive
      || host.kernelUndoRedoFrontendLeaseActive
      || host.aiEditLeaseFrontendLockActive,
    onDocumentChange: (nextSource, cursorPosition, changeSet) => {
      if (host.syncingSourceFromEditor) return;
      const previousSource = host.source;
      host.source = nextSource;
      host.sourceCache = { ...host.sourceCache, [host.currentSourceCacheKey]: nextSource };
      if (host.currentSourceRelativePath) {
        queueFileBufferDraftChangeSetForPath(
          host.currentSourceRelativePath,
          previousSource,
          nextSource,
          changeSet.changes,
        );
      }
      host.setGlobalStatus(`Modificări nesalvate în ${host.currentSourcePath}`, "unsaved");
      if (host.sourceLanguage === "html") {
        const context = resolveHtmlSourceSelectionContext({
          sourceText: nextSource,
          cursorPosition,
          selectedElement: host.selectedElement,
          htmlVoidTags,
        });
        host.reconcileSelectionWithSourceDocument(context.parsedDocument, context.activeSelector);
      }
      if (host.isActivePreviewHtmlSource) host.syncHtmlCodeToPreview(nextSource, cursorPosition);
      if (host.sourceLanguage === "html") handleCodeCursorSelection(host, cursorPosition, nextSource);
      if (host.sourceLanguage === "css" || host.sourceLanguage === "scss") {
        host.notifyCssSourceChanged();
      }
    },
    onSelectionChange: (cursorPosition, docText) => {
      if (!host.syncingSelectionFromCode) handleCodeCursorSelection(host, cursorPosition, docText);
    },
    onContextMenu: (request) => host.openCodeEditorContextMenu?.(request),
  });
}

export function handleCodeCursorSelection(
  host: SourceEditorControllerHost,
  position: number,
  sourceText: string,
) {
  if (host.sourceLanguage === "css" || host.sourceLanguage === "scss") {
    const cssTarget = cssSelectorAtPosition(sourceText, position);
    if (cssTarget && host.currentSourceRelativePath) {
      host.selectCssSelectorFromCode({
        selector: cssTarget.selector,
        file: host.currentSourceRelativePath,
      });
    }
    return;
  }

  const action = resolveCodeCursorSelectionAction({
    sourceLanguage: host.sourceLanguage,
    sourceText,
    position,
    selectedElement: host.selectedElement,
    activeScannedPath: host.activeScannedPath,
    htmlVoidTags,
  });
  if (action.type === "select-html-node") {
    withSyncingCode(host, () => host.selectDomNode(action.selector, { revealCode: false }));
    return;
  }
  if (action.type !== "select-markdown-target") return;
  const previewDoc = host.getPreviewDocument();
  const element = previewDoc ? findPreviewElementForMarkdownTarget(previewDoc, action.target) : null;
  if (!element) {
    withSyncingCode(host, () => host.postPreviewMessage({ type: "select-markdown-target", target: action.target }));
    return;
  }
  withSyncingCode(host, () => host.selectPreviewElement(element));
}

export function updateMarkdownSource(
  host: SourceEditorControllerHost,
  nextSource: string,
  relativePath = host.currentSourceRelativePath,
) {
  if (!relativePath) return;
  const cacheKey = scannedCacheKey({ relativePath });
  const previousSource = host.activeScannedPath === relativePath
    ? host.source
    : (host.sourceCache[cacheKey] ?? "");
  if (nextSource === previousSource) return;
  if (host.activeScannedPath === relativePath) {
    host.source = nextSource;
  }
  host.sourceCache = { ...host.sourceCache, [cacheKey]: nextSource };
  queueFileBufferDraftTextTransitionForPath(relativePath, previousSource, nextSource, "markdown.editor");
  host.setGlobalStatus(`Modificări nesalvate în ${relativePath}`, "unsaved");
}

export function syncCodeSelectionHighlight(host: SourceEditorControllerHost, reveal = false) {
  if (!host.codeEditorController) return;
  host.codeEditorController.setSelectedRange(
    codeSelectionRangeForTemplateSource(host)
      ?? codeSelectionRangeForActiveCssSelector(host)
      ?? codeSelectionRangeForSelection(host.sourceLanguage, host.htmlSourceNodes, host.selectedElement),
    reveal,
  );
}

function codeSelectionRangeForTemplateSource(host: SourceEditorControllerHost) {
  const node = host.selectedTemplateSourceNode;
  if (!node?.range || host.sourceLanguage !== "html") return null;
  if (projectRelativeZolaPath(node.file) !== host.currentSourceRelativePath) return null;
  return codeSelectionRangeForSourceRange(host.source, node.range);
}

function codeSelectionRangeForActiveCssSelector(host: SourceEditorControllerHost) {
  if (host.sourceLanguage !== "css" && host.sourceLanguage !== "scss") return null;
  if (!host.activeCssSelector || !host.targetCssFile) return null;
  if (zolaRelativePath(host.targetCssFile) !== zolaRelativePath(host.currentSourceRelativePath)) return null;
  return codeSelectionRangeForCssSelector(host.source, host.activeCssSelector);
}

export function withSyncingCode(host: SourceEditorControllerHost, fn: () => void) {
  host.syncingSelectionFromCode = true;
  fn();
  queueMicrotask(() => {
    host.syncingSelectionFromCode = false;
  });
}
