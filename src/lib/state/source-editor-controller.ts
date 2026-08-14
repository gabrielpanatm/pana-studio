import type { CodeEditorContextMenuRequest, CodeEditorController } from "$lib/editor/controller";
import {
  codeSelectionRangeForSourceRange,
  type CodeSelectionRange,
} from "$lib/editor/source-ranges";
import type { CodeSelectionPresentation } from "$lib/editor/codemirror";
import { projectRelativeZolaPath, scannedCacheKey } from "$lib/project/files";
import {
  queueFileBufferDraftChangeSetForPath,
  queueFileBufferDraftTextTransitionForPath,
} from "$lib/session/file-buffer-draft-sync";
import {
  findPreviewElementForMarkdownTarget,
  markdownTargetAtPosition,
} from "$lib/preview/selection";
import type { SelectionSnapshot, SourceLanguage } from "$lib/types";
import type { GlobalStatusKind } from "$lib/status/global-status";
import { t } from "$lib/i18n/runtime.svelte";
import { primarySelectionEntry, selectionCodeTarget } from "$lib/kernel/selection-read-model";

export type SourceEditorControllerHost = {
  codeEditorHost: HTMLDivElement | undefined;
  codeEditorController: CodeEditorController | null;
  source: string;
  sourceLanguage: SourceLanguage;
  uiTheme: "dark" | "light";
  projectTransitionFrontendLeaseActive: boolean;
  kernelUndoRedoFrontendQuiesceActive: boolean;
  kernelUndoRedoFrontendLeaseActive: boolean;
  aiEditLeaseFrontendLockActive: boolean;
  syncingSourceFromEditor: boolean;
  syncingSelectionFromCode: boolean;
  sourceCache: Record<string, string>;
  currentSourceCacheKey: string;
  currentSourceRelativePath: string;
  currentSourcePath: string;
  selectionSnapshot: SelectionSnapshot | null;
  activeScannedPath: string | null;
  isActivePreviewHtmlSource: boolean;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
  syncHtmlCodeToPreview: (sourceText: string, cursorPosition: number) => void;
  selectSourcePositionFromCode: (file: string, offset: number) => void | Promise<void>;
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
      || host.kernelUndoRedoFrontendQuiesceActive
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
      host.setGlobalStatus(t("source-editor-unsaved", {
        path: host.currentSourcePath,
      }), "unsaved");
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
    if (!host.currentSourceRelativePath) return;
    const byteOffset = new TextEncoder().encode(sourceText.slice(0, position)).byteLength;
    void host.selectSourcePositionFromCode(host.currentSourceRelativePath, byteOffset);
    return;
  }

  if (host.sourceLanguage === "html" && host.currentSourceRelativePath) {
    const byteOffset = new TextEncoder().encode(sourceText.slice(0, position)).byteLength;
    void host.selectSourcePositionFromCode(host.currentSourceRelativePath, byteOffset);
    return;
  }

  if (host.sourceLanguage === "js" && host.currentSourceRelativePath) {
    const byteOffset = new TextEncoder().encode(sourceText.slice(0, position)).byteLength;
    void host.selectSourcePositionFromCode(host.currentSourceRelativePath, byteOffset);
    return;
  }

  if (!host.activeScannedPath?.endsWith(".md")) return;
  const target = markdownTargetAtPosition(sourceText, position);
  if (!target) return;
  const previewDoc = host.getPreviewDocument();
  const element = previewDoc ? findPreviewElementForMarkdownTarget(previewDoc, target) : null;
  if (!element) {
    withSyncingCode(host, () => host.postPreviewMessage({ type: "select-markdown-target", target }));
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
  host.setGlobalStatus(t("source-editor-unsaved", { path: relativePath }), "unsaved");
}

export function syncCodeSelectionHighlight(host: SourceEditorControllerHost, reveal = false) {
  if (!host.codeEditorController) return;
  const projection = codeSelectionProjectionForCoordinator(host);
  host.codeEditorController.setSelectedRange(
    projection?.range ?? null,
    reveal,
    projection?.presentation ?? "range",
  );
}

function codeSelectionProjectionForCoordinator(host: SourceEditorControllerHost): {
  range: CodeSelectionRange;
  presentation: CodeSelectionPresentation;
} | null {
  const projection = selectionCodeTarget(host.selectionSnapshot);
  if (
    !projection?.range
    || !["html", "css", "scss"].includes(host.sourceLanguage)
  ) return null;
  if (projectRelativeZolaPath(projection.file ?? "") !== host.currentSourceRelativePath) {
    return null;
  }
  const focus = host.selectionSnapshot?.focus;
  const primary = primarySelectionEntry(host.selectionSnapshot);
  const presentation: CodeSelectionPresentation =
    focus?.kind === "cssRule" || focus?.kind === "cssProperty"
      ? "cssRule"
      : primary?.subject.kind === "htmlElement" || primary?.subject.kind === "runtimeElement"
        ? "htmlElement"
        : "range";
  return {
    range: codeSelectionRangeForSourceRange(host.source, projection.range),
    presentation,
  };
}

export function withSyncingCode(host: SourceEditorControllerHost, fn: () => void) {
  host.syncingSelectionFromCode = true;
  fn();
  queueMicrotask(() => {
    host.syncingSelectionFromCode = false;
  });
}
