import type { CodeEditorContextMenuRequest, CodeEditorController } from "$lib/editor/controller";
import { SOURCE_LOADING_SENTINEL } from "$lib/editor-runtime/source-state";
import { contextMenu } from "$lib/context-menu/store.svelte";
import type { ApplicationPreferencesState } from "$lib/application/preferences.svelte";
import { codeSelectionRangeForSourceRange } from "$lib/editor/source-ranges";
import type { CodeSelectionPresentation } from "$lib/editor/codemirror";
import { cssRuleContextFromSource } from "$lib/css/source-sync";
import { t } from "$lib/i18n/runtime.svelte";
import { primarySelectionEntry, selectionCodeTarget } from "$lib/kernel/selection-read-model";
import {
  currentHtmlRelativePath,
  currentSourceRelativePath,
  detectSourceLanguage,
  projectRelativeZolaPath,
  scannedCacheKey,
  zolaRelativePath,
} from "$lib/project/files";
import {
  findPreviewElementForMarkdownTarget,
  markdownTargetAtPosition,
} from "$lib/preview/selection";
import {
  queueFileBufferDraftChangeSetForPath,
  queueFileBufferDraftTextTransitionForPath,
} from "$lib/session/file-buffer-draft-sync";
import type { EditorSelectionSessionController } from "$lib/state/editor-selection-session.svelte";
import type { GlobalStatusKind } from "$lib/status/global-status";
import type { ProjectBootstrapSourceLocation } from "$lib/project/lifecycle-contract";
import type { SourceRange } from "$lib/source-graph/contracts";

export type SourceWorkspaceContext = Readonly<{
  activeScannedPath: string | null;
  activePreviewPath: string;
  projectTransitionLocked: boolean;
  historyLocked: boolean;
  aiLocked: boolean;
  selection: Pick<EditorSelectionSessionController, "selectionSnapshot">;
  saveHasPending: boolean;
}>;

export type SourceWorkspaceCommands = {
  context: () => SourceWorkspaceContext;
  setStatus: (text: string, kind: GlobalStatusKind) => void;
  syncHtmlCodeToPreview: (sourceText: string, cursorPosition: number) => void;
  selectSourcePosition: (file: string, offset: number) => void | Promise<void>;
  getPreviewDocument: () => Document | undefined;
  postPreviewMessage: (payload: Record<string, unknown>) => void;
  selectPreviewElement: (element: Element, options?: { revealCode?: boolean }) => void;
  save: () => Promise<unknown>;
};

export function sourceWorkspaceReadOnly(
  context: Pick<SourceWorkspaceContext, "projectTransitionLocked" | "historyLocked" | "aiLocked">,
) {
  return context.projectTransitionLocked || context.historyLocked || context.aiLocked;
}

type CodeRevealTarget =
  | { kind: "html" }
  | { kind: "css"; selector: string; file: string };

/** Owns source text, CodeMirror lifetime, source caches and exact reveal requests. */
export class SourceWorkspaceState {
  hostElement = $state<HTMLDivElement | undefined>(undefined);
  controller = $state<CodeEditorController | null>(null);
  source = $state("");
  sourceCache = $state<Record<string, string>>({});
  cssSourceRevision = $state(0);
  codeRevealTarget = $state<CodeRevealTarget>({ kind: "html" });
  codeSelectionRevealRequestId = $state(0);
  pendingBootstrapDiagnosticReveal = $state<({ relativePath: string } & ProjectBootstrapSourceLocation) | null>(null);
  pendingSourceRangeReveal = $state<{ relativePath: string; range: SourceRange } | null>(null);
  syncingSourceFromEditor = false;
  syncingSelectionFromCode = false;

  private codeSelectionRevealConsumedId = 0;
  private readonly commands: SourceWorkspaceCommands;
  private readonly preferences: Pick<ApplicationPreferencesState, "theme">;

  constructor(
    commands: SourceWorkspaceCommands,
    preferences: Pick<ApplicationPreferencesState, "theme">,
  ) {
    this.commands = commands;
    this.preferences = preferences;
  }

  get currentSourcePath() {
    return this.commands.context().activeScannedPath ?? "";
  }

  get sourceLanguage() {
    return detectSourceLanguage(this.currentSourcePath);
  }

  get currentSourceCacheKey() {
    const activePath = this.commands.context().activeScannedPath;
    return activePath ? `scanned:${activePath}` : "no-project";
  }

  get currentSourceRelativePath() {
    return currentSourceRelativePath(this.commands.context().activeScannedPath);
  }

  get currentHtmlRelativePath() {
    return currentHtmlRelativePath(this.commands.context().activePreviewPath);
  }

  get isActivePreviewHtmlSource() {
    return this.sourceLanguage === "html"
      && this.currentSourceRelativePath === this.currentHtmlRelativePath;
  }

  readOnly() {
    const context = this.commands.context();
    return sourceWorkspaceReadOnly(context);
  }

  async createEditor() {
    if (!this.hostElement || this.controller) return;
    const { createCodeEditorController } = await import("$lib/editor/controller");
    if (!this.hostElement || this.controller) return;
    this.controller = createCodeEditorController({
      host: this.hostElement,
      doc: this.source,
      language: this.sourceLanguage,
      theme: this.preferences.theme,
      readOnly: this.readOnly(),
      onDocumentChange: (nextSource, cursorPosition, changeSet) => {
        if (this.syncingSourceFromEditor) return;
        const previousSource = this.source;
        this.source = nextSource;
        this.sourceCache = { ...this.sourceCache, [this.currentSourceCacheKey]: nextSource };
        if (this.currentSourceRelativePath) {
          queueFileBufferDraftChangeSetForPath(
            this.currentSourceRelativePath,
            previousSource,
            nextSource,
            changeSet.changes,
          );
        }
        this.commands.setStatus(t("source-editor-unsaved", {
          path: this.currentSourcePath,
        }), "unsaved");
        if (this.isActivePreviewHtmlSource) this.commands.syncHtmlCodeToPreview(nextSource, cursorPosition);
        if (this.sourceLanguage === "html") this.handleCursorSelection(cursorPosition, nextSource);
        if (this.sourceLanguage === "css" || this.sourceLanguage === "scss") {
          this.notifyCssSourceChanged();
        }
      },
      onSelectionChange: (cursorPosition, docText) => {
        if (!this.syncingSelectionFromCode) this.handleCursorSelection(cursorPosition, docText);
      },
      onContextMenu: (request) => this.openContextMenu(request),
    });
    this.applyPendingBootstrapDiagnosticReveal();
  }

  handleCursorSelection(position: number, sourceText: string) {
    if (["css", "scss", "html", "js"].includes(this.sourceLanguage)) {
      if (!this.currentSourceRelativePath) return;
      const byteOffset = new TextEncoder().encode(sourceText.slice(0, position)).byteLength;
      void this.commands.selectSourcePosition(this.currentSourceRelativePath, byteOffset);
      return;
    }
    const activePath = this.commands.context().activeScannedPath;
    if (!activePath?.endsWith(".md")) return;
    const target = markdownTargetAtPosition(sourceText, position);
    if (!target) return;
    const previewDoc = this.commands.getPreviewDocument();
    const element = previewDoc ? findPreviewElementForMarkdownTarget(previewDoc, target) : null;
    if (!element) {
      this.withSyncingCode(() => this.commands.postPreviewMessage({ type: "select-markdown-target", target }));
      return;
    }
    this.withSyncingCode(() => this.commands.selectPreviewElement(element));
  }

  updateMarkdown(nextSource: string, relativePath = this.currentSourceRelativePath) {
    if (!relativePath) return;
    const cacheKey = scannedCacheKey({ relativePath });
    const previousSource = this.commands.context().activeScannedPath === relativePath
      ? this.source
      : (this.sourceCache[cacheKey] ?? "");
    if (nextSource === previousSource) return;
    if (this.commands.context().activeScannedPath === relativePath) this.source = nextSource;
    this.sourceCache = { ...this.sourceCache, [cacheKey]: nextSource };
    queueFileBufferDraftTextTransitionForPath(relativePath, previousSource, nextSource, "markdown.editor");
    this.commands.setStatus(t("source-editor-unsaved", { path: relativePath }), "unsaved");
  }

  syncSelectionHighlight(reveal = false) {
    if (!this.controller) return;
    const projection = this.codeSelectionProjection();
    this.controller.setSelectedRange(
      projection?.range ?? null,
      reveal,
      projection?.presentation ?? "range",
    );
  }

  private codeSelectionProjection(): {
    range: ReturnType<typeof codeSelectionRangeForSourceRange>;
    presentation: CodeSelectionPresentation;
  } | null {
    const selection = this.commands.context().selection.selectionSnapshot;
    const projection = selectionCodeTarget(selection);
    if (!projection?.range || !["html", "css", "scss"].includes(this.sourceLanguage)) return null;
    if (projectRelativeZolaPath(projection.file ?? "") !== this.currentSourceRelativePath) return null;
    const primary = primarySelectionEntry(selection);
    const presentation: CodeSelectionPresentation =
      selection?.focus.kind === "cssRule" || selection?.focus.kind === "cssProperty"
        ? "cssRule"
        : primary?.subject.kind === "htmlElement" || primary?.subject.kind === "runtimeElement"
          ? "htmlElement"
          : "range";
    return {
      range: codeSelectionRangeForSourceRange(this.source, projection.range),
      presentation,
    };
  }

  setCssRevealTarget(target: { selector: string; file: string }) {
    if (!target.selector || !target.file) return;
    if (
      this.codeRevealTarget.kind === "css"
      && this.codeRevealTarget.selector === target.selector
      && this.codeRevealTarget.file === target.file
    ) return;
    this.codeRevealTarget = { kind: "css", selector: target.selector, file: target.file };
  }

  setHtmlRevealTarget() {
    if (this.codeRevealTarget.kind !== "html") this.codeRevealTarget = { kind: "html" };
  }

  requestSelectionReveal() {
    this.codeSelectionRevealRequestId += 1;
  }

  consumeSelectionRevealRequest() {
    if (this.codeSelectionRevealConsumedId === this.codeSelectionRevealRequestId) return false;
    this.codeSelectionRevealConsumedId = this.codeSelectionRevealRequestId;
    return true;
  }

  revealBootstrapDiagnostic(relativePath: string, location: ProjectBootstrapSourceLocation) {
    if (
      !relativePath
      || !Number.isSafeInteger(location.line)
      || location.line < 1
      || !Number.isSafeInteger(location.column)
      || location.column < 1
    ) return;
    this.pendingBootstrapDiagnosticReveal = { relativePath, ...location };
    this.applyPendingBootstrapDiagnosticReveal();
  }

  applyPendingBootstrapDiagnosticReveal() {
    const target = this.pendingBootstrapDiagnosticReveal;
    if (!target) return false;
    if (this.commands.context().activeScannedPath !== target.relativePath) {
      this.pendingBootstrapDiagnosticReveal = null;
      return false;
    }
    if (
      !this.controller
      || this.source === SOURCE_LOADING_SENTINEL
      || this.controller.getDoc() !== this.source
    ) return false;
    this.controller.revealLineColumn(target.line, target.column);
    this.pendingBootstrapDiagnosticReveal = null;
    return true;
  }

  revealSourceRange(relativePath: string, range: SourceRange) {
    if (!relativePath || !Number.isSafeInteger(range.start) || !Number.isSafeInteger(range.end)) return;
    this.pendingSourceRangeReveal = { relativePath, range };
    this.applyPendingSourceRangeReveal();
  }

  applyPendingSourceRangeReveal() {
    const target = this.pendingSourceRangeReveal;
    if (!target) return false;
    if (this.commands.context().activeScannedPath !== target.relativePath) {
      this.pendingSourceRangeReveal = null;
      return false;
    }
    if (
      !this.controller
      || this.source === SOURCE_LOADING_SENTINEL
      || this.controller.getDoc() !== this.source
    ) return false;
    this.controller.setSelectedRange(codeSelectionRangeForSourceRange(this.source, target.range), true);
    this.pendingSourceRangeReveal = null;
    return true;
  }

  notifyCssSourceChanged() {
    this.cssSourceRevision += 1;
  }

  cssRuleContext(file: string, selector: string, viewport: import("$lib/css/contracts").CssViewport) {
    if (!this.isOpenCssSource(file) || !selector) return null;
    return cssRuleContextFromSource(this.source, file, selector, viewport);
  }

  isOpenCssSource(file: string) {
    if (this.sourceLanguage !== "css" && this.sourceLanguage !== "scss") return false;
    if (!file || !this.currentSourceRelativePath) return false;
    return zolaRelativePath(file) === zolaRelativePath(this.currentSourceRelativePath);
  }

  withSyncingCode(fn: () => void) {
    this.syncingSelectionFromCode = true;
    fn();
    queueMicrotask(() => {
      this.syncingSelectionFromCode = false;
    });
  }

  private openContextMenu(request: CodeEditorContextMenuRequest) {
    const selection = this.commands.context().selection.selectionSnapshot;
    contextMenu.open({
      source: "code",
      x: request.event.clientX,
      y: request.event.clientY,
      title: this.currentSourcePath || t("workbench-source-code"),
      subtitle: t("workbench-code-position", { line: request.line, column: request.column }),
      items: [
        {
          id: "save-source",
          label: t("workbench-save"),
          shortcut: "Ctrl+S",
          disabled: !this.commands.context().saveHasPending,
          action: async () => { await this.commands.save(); },
        },
        {
          id: "select-html-at-cursor",
          label: t("workbench-select-html-at-cursor"),
          disabled: this.sourceLanguage !== "html",
          separatorBefore: true,
          action: () => this.handleCursorSelection(request.position, request.docText),
        },
        {
          id: "reveal-current-selection",
          label: t("workbench-reveal-selection-code"),
          disabled: !primarySelectionEntry(selection),
          action: () => this.syncSelectionHighlight(true),
        },
        {
          id: "copy-code-selection",
          label: t("workbench-copy-code-selection"),
          disabled: !request.hasSelection,
          separatorBefore: true,
          action: async () => {
            if (!request.selectedText) return;
            await navigator.clipboard?.writeText(request.selectedText);
            this.commands.setStatus(t("workbench-code-selection-copied"), "idle");
          },
        },
      ],
    });
  }
}
