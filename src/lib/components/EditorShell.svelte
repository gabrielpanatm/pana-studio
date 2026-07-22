<script lang="ts">
  import { onDestroy } from "svelte";
  import MarkdownEditor from "$lib/components/markdown/MarkdownEditor.svelte";
  import InteractivePreviewSurface from "$lib/components/preview/InteractivePreviewSurface.svelte";
  import DocumentBar from "$lib/components/workbench/DocumentBar.svelte";
  import ResponsiveCanvasToolbar from "$lib/components/workbench/ResponsiveCanvasToolbar.svelte";
  import WorkbenchSplitHandle from "$lib/components/workbench/WorkbenchSplitHandle.svelte";
  import { resetPreviewFrameDocumentAccess } from "$lib/preview/frame-origin";
  import type {
    SourceLanguage,
    TemplateWorkbenchPlan,
    WorkbenchCanvasMode,
    WorkbenchCanvasPreset,
    WorkbenchCanvasViewportSnapshot,
    WorkbenchDocumentSnapshot,
    WorkbenchGroupId,
    WorkbenchSnapshot,
    WorkbenchSplit,
    WorkbenchSurface,
  } from "$lib/types";
  import type { InteractivePreviewDomNode } from "$lib/preview/interactive";

  type CenterView = "preview" | "code" | "markdown" | "canvas" | "site" | "kernel";

  export let centerView: CenterView = "preview";
  export let previewZoom = 100;
  export let previewCanvasMode: WorkbenchCanvasMode = "fit";
  export let previewCanvasPreset: WorkbenchCanvasPreset = "desktop";
  export let previewWidthPx = 1_440;
  export let previewRulers = true;
  export let responsiveBreakpoints: Array<{ id: string; label: string; widthPx: number }> = [];
  export let previewDocumentMarkup: string | null = null;
  export let previewSrc = "";
  export let templateWorkbenchActive = false;
  export let templateWorkbenchTarget: string | null = null;
  export let templateWorkbenchPlan: TemplateWorkbenchPlan | null = null;
  export let interactivePreviewEnabled = false;
  export let interactivePreviewUrl = "";
  export let interactiveDomNodeCount = 0;
  export let refreshToken = 0;
  export let currentSourcePath = "";
  export let source = "";
  export let sourceLanguage: SourceLanguage = "plain";
  export let sourceLength = 0;
  export let editorReadOnly = false;
  export let workbenchSnapshot: WorkbenchSnapshot | null = null;
  export let dirtyWorkbenchPaths: string[] = [];
  export let activateWorkbenchDocument: (
    groupId: WorkbenchGroupId,
    document: WorkbenchDocumentSnapshot,
  ) => void | Promise<void> = () => {};
  export let closeWorkbenchDocument: (
    groupId: WorkbenchGroupId,
    document: WorkbenchDocumentSnapshot,
  ) => void | Promise<void> = () => {};
  export let setWorkbenchSurface: (surface: WorkbenchSurface) => void | Promise<void> = () => {};
  export let setWorkbenchSplit: (split: WorkbenchSplit) => void | Promise<void> = () => {};
  export let setWorkbenchSplitRatio: (ratioBasisPoints: number) => void | Promise<void> = () => {};
  export let setCanvasViewport: (
    viewport: Partial<WorkbenchCanvasViewportSnapshot>,
  ) => void | Promise<void> = () => {};
  export let onMarkdownChange: (nextSource: string, path: string) => void = () => {};
  export let attachPreviewInspector: () => void;
  export let exitTemplateWorkbench: () => void | Promise<void> = () => {};
  export let setInteractivePreviewEnabled: (enabled: boolean) => void = () => {};
  export let onInteractiveLifecycleError: (message: string) => void = () => {};
  export let onInteractiveDomSnapshot: (nodes: InteractivePreviewDomNode[]) => void = () => {};
  export let onInteractiveRealmRestarted: (previewRevision: string, durationMs: number) => void = () => {};
  export let onInteractiveRealmFailed: (previewRevision: string, durationMs: number, diagnostic: string) => void = () => {};
  export let previewFrame: HTMLIFrameElement | undefined = undefined;
  export let codeEditorHost: HTMLDivElement | undefined = undefined;

  $: sourceIsLoading = source === "Se incarca fisierul...";
  $: workbenchSplit = workbenchSnapshot?.split ?? "none";
  $: splitActive = workbenchSplit !== "none";
  $: splitRatioBasisPoints = workbenchSnapshot?.splitRatioBasisPoints ?? 5_000;
  $: secondaryGroup = workbenchSnapshot?.groups.find((group) => group.groupId === "secondary");
  $: secondaryDocument = secondaryGroup?.documents.find(
    (document) => document.documentId === secondaryGroup?.activeDocumentId,
  );
  $: sourceSurface = splitActive
    ? secondaryDocument?.surface ?? (sourceLanguage === "markdown" ? "markdown" : "code")
    : centerView === "markdown"
      ? "markdown"
      : "code";
  $: showPreview = splitActive || centerView === "preview";
  $: showSource = splitActive || centerView === "code" || centerView === "markdown";
  $: canvasViewport = {
    mode: previewCanvasMode,
    preset: previewCanvasPreset,
    widthPx: previewWidthPx,
    zoomPercent: previewZoom,
    showRulers: previewRulers,
  } satisfies WorkbenchCanvasViewportSnapshot;

  let viewportResizing = false;
  let draftPreviewWidthPx = previewWidthPx;
  let viewportResizeCleanup: (() => void) | null = null;
  $: if (!viewportResizing) draftPreviewWidthPx = previewWidthPx;

  function handlePreviewLoad() {
    if (previewFrame) resetPreviewFrameDocumentAccess(previewFrame);
    attachPreviewInspector();
  }

  function stopViewportResize(commit: boolean) {
    if (!viewportResizing) return;
    viewportResizing = false;
    document.body.classList.remove("workbench-viewport-resizing");
    viewportResizeCleanup?.();
    viewportResizeCleanup = null;
    if (commit) {
      void setCanvasViewport({
        mode: "fixed",
        preset: "custom",
        widthPx: draftPreviewWidthPx,
      });
    } else {
      draftPreviewWidthPx = previewWidthPx;
    }
  }

  function startViewportResize(event: PointerEvent, edge: "left" | "right") {
    if (event.button !== 0 || previewCanvasMode !== "fixed") return;
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = previewWidthPx;
    const scale = Math.max(0.25, previewZoom / 100);
    viewportResizing = true;
    draftPreviewWidthPx = startWidth;
    document.body.classList.add("workbench-viewport-resizing");
    const move = (moveEvent: PointerEvent) => {
      const signedDelta = (moveEvent.clientX - startX) * (edge === "right" ? 1 : -1);
      draftPreviewWidthPx = Math.round(
        Math.min(3_840, Math.max(320, startWidth + (signedDelta * 2) / scale)),
      );
    };
    const up = () => stopViewportResize(true);
    const cancel = () => stopViewportResize(false);
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up, { once: true });
    window.addEventListener("pointercancel", cancel, { once: true });
    viewportResizeCleanup = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", cancel);
    };
  }

  function resizeViewportFromKeyboard(event: KeyboardEvent) {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    const direction = event.key === "ArrowRight" ? 1 : -1;
    const step = event.shiftKey ? 50 : 10;
    void setCanvasViewport({
      mode: "fixed",
      preset: "custom",
      widthPx: Math.min(3_840, Math.max(320, previewWidthPx + direction * step)),
    });
  }

  onDestroy(() => stopViewportResize(false));
</script>

<section class="editor-shell" aria-label="Previzualizare și cod sursă">
  <DocumentBar
    snapshot={workbenchSnapshot}
    dirtyPaths={dirtyWorkbenchPaths}
    activateDocument={activateWorkbenchDocument}
    closeDocument={closeWorkbenchDocument}
    setSurface={setWorkbenchSurface}
    setSplit={setWorkbenchSplit}
    splitDisabled={!currentSourcePath || sourceIsLoading}
  />
  <div
    class:split-vertical={workbenchSplit === "vertical"}
    class:split-horizontal={workbenchSplit === "horizontal"}
    class="editor-stage-grid"
    style={`--wb-split-ratio: ${splitRatioBasisPoints / 100}%;`}
  >
  <div class:hidden-stage={!showPreview} class="preview-shell" aria-label="Suprafață vizuală">
    <ResponsiveCanvasToolbar
      viewport={canvasViewport}
      documentPath={currentSourcePath}
      breakpoints={responsiveBreakpoints}
      setViewport={setCanvasViewport}
    />
    <div class="editor-context-bar" role="status" aria-live="polite">
      <span class="editor-context-copy">
        <strong>
          {templateWorkbenchActive
            ? "Context de template"
            : interactivePreviewEnabled
              ? "Previzualizare interactivă"
              : "Editare sigură"}
        </strong>
        {#if templateWorkbenchActive}
          <span>{templateWorkbenchTarget}</span>
          {#if templateWorkbenchPlan}
            <small class:fixture={!templateWorkbenchPlan.renderContext.canonicalTruth}
              title={templateWorkbenchPlan.renderContext.explanation}>
              {templateWorkbenchPlan.renderContext.label}
            </small>
          {/if}
        {:else}
          <span>
            {interactivePreviewEnabled
              ? `JavaScript izolat · DOM numai pentru citire: ${interactiveDomNodeCount} noduri · fără acces la Tauri sau fișiere.`
              : "JavaScript-ul proiectului este oprit în editor. Deschiderea externă rulează site-ul complet."}
          </span>
        {/if}
      </span>
      <span class="editor-context-actions">
        {#if templateWorkbenchActive}
          <button type="button" onclick={() => { void exitTemplateWorkbench(); }}>Înapoi la site</button>
        {/if}
        <button
          type="button"
          disabled={!interactivePreviewEnabled && !interactivePreviewUrl}
          onclick={() => setInteractivePreviewEnabled(!interactivePreviewEnabled)}
        >
          {interactivePreviewEnabled ? "Revino la editare sigură" : "Pornește modul interactiv"}
        </button>
      </span>
    </div>
    <div
      class:canvas-fit={previewCanvasMode === "fit"}
      class:canvas-fixed={previewCanvasMode === "fixed"}
      class:with-rulers={previewRulers}
      class="preview-stage"
      style={`--preview-zoom-scale: ${previewZoom / 100}; --canvas-width-px: ${draftPreviewWidthPx}px;`}
    >
      <div class="preview-viewport-frame">
        {#if previewCanvasMode === "fixed" && previewRulers}
          <div class="canvas-ruler" aria-hidden="true">
            <span>0</span>
            <strong>{draftPreviewWidthPx}px</strong>
            <span>{draftPreviewWidthPx}</span>
          </div>
        {/if}
        {#if previewCanvasMode === "fixed"}
          <button
            type="button"
            class="viewport-resize-handle left"
            aria-label="Redimensionează canvas-ul din stânga"
            title="Trage pentru lățime liberă · săgeți ±10px · Shift ±50px"
            onpointerdown={(event) => { startViewportResize(event, "left"); }}
            onkeydown={resizeViewportFromKeyboard}
          ></button>
          <button
            type="button"
            class="viewport-resize-handle right"
            aria-label="Redimensionează canvas-ul din dreapta"
            title="Trage pentru lățime liberă · săgeți ±10px · Shift ±50px"
            onpointerdown={(event) => { startViewportResize(event, "right"); }}
            onkeydown={resizeViewportFromKeyboard}
          ></button>
        {/if}
        <iframe
          bind:this={previewFrame}
          class:interactive-background={interactivePreviewEnabled}
          class="preview-page"
          title="Previzualizare HTML și CSS pentru site-ul de test"
          src={previewDocumentMarkup ? undefined : previewSrc}
          srcdoc={previewDocumentMarkup ?? undefined}
          sandbox={previewDocumentMarkup ? "" : "allow-scripts"}
          onload={handlePreviewLoad}
        ></iframe>
        {#if interactivePreviewEnabled}
          <InteractivePreviewSurface
            desiredUrl={interactivePreviewUrl}
            canvasMode={previewCanvasMode}
            canvasWidthPx={draftPreviewWidthPx}
            {previewZoom}
            onDomSnapshot={onInteractiveDomSnapshot}
            onLifecycleError={onInteractiveLifecycleError}
            onRealmRestarted={onInteractiveRealmRestarted}
            onRealmFailed={onInteractiveRealmFailed}
          />
        {/if}
      </div>
    </div>
  </div>

  {#if splitActive}
    <WorkbenchSplitHandle
      orientation={workbenchSplit as Exclude<WorkbenchSplit, "none">}
      ratioBasisPoints={splitRatioBasisPoints}
      onCommit={setWorkbenchSplitRatio}
    />
  {/if}

  <section
    class:hidden-stage={!showSource}
    class="source-panel source-stage"
    id="source"
    aria-label="Cod sursa"
  >
    {#if sourceSurface === "markdown" && sourceLanguage === "markdown" && !sourceIsLoading}
      {#key currentSourcePath}
        <MarkdownEditor
          {source}
          path={currentSourcePath}
          {refreshToken}
          readOnly={editorReadOnly}
          onChange={onMarkdownChange}
        />
      {/key}
    {:else if sourceSurface === "markdown" && sourceLanguage === "markdown"}
      <div class="markdown-loading-stage" aria-label="Se încarcă fișierul Markdown"></div>
    {:else}
      <div class="code-source-layout">
        <div class="source-header">
          <h2 title={currentSourcePath}>
            <strong>Cod</strong>
            <span>{currentSourcePath || "Niciun fișier deschis"}</span>
          </h2>
          <span class="source-meta">{sourceLanguage.toUpperCase()} • {sourceLength} chars</span>
        </div>
        <div bind:this={codeEditorHost} class="code-editor-host" data-language={sourceLanguage}></div>
      </div>
    {/if}
  </section>
  </div>
</section>

<style>
  .editor-shell {
    position: relative;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    min-width: 0;
    min-height: 0;
    height: 100%;
    border: 1px solid var(--border);
    border-radius: 10px;
    overflow: hidden;
    box-shadow: var(--shadow);
    background: var(--surface);
  }

  .preview-shell {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    min-height: 0;
    min-width: 0;
    margin: 0;
    background: transparent;
    overflow: hidden;
  }

  .editor-stage-grid {
    grid-row: 2;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .editor-stage-grid > .preview-shell,
  .editor-stage-grid > .source-stage {
    grid-area: 1 / 1;
  }

  .editor-stage-grid.split-vertical {
    grid-template-columns: minmax(0, calc(var(--wb-split-ratio) - 2.5px)) 5px minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
  }

  .editor-stage-grid.split-vertical > .preview-shell {
    grid-area: 1 / 1;
  }

  .editor-stage-grid.split-vertical > :global(.split-handle) {
    grid-area: 1 / 2;
  }

  .editor-stage-grid.split-vertical > .source-stage {
    grid-area: 1 / 3;
  }

  .editor-stage-grid.split-horizontal {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: minmax(0, calc(var(--wb-split-ratio) - 2.5px)) 5px minmax(0, 1fr);
  }

  .editor-stage-grid.split-horizontal > .preview-shell {
    grid-area: 1 / 1;
  }

  .editor-stage-grid.split-horizontal > :global(.split-handle) {
    grid-area: 2 / 1;
  }

  .editor-stage-grid.split-horizontal > .source-stage {
    grid-area: 3 / 1;
  }

  :global(body.workbench-split-resizing) .preview-stage {
    pointer-events: none;
  }

  .editor-context-bar {
    position: relative;
    z-index: 2;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 40px;
    padding: 4px 10px;
    border-bottom: 1px solid var(--border-2);
    color: var(--text-muted);
    font-size: 12px;
    background: var(--surface-2);
  }

  .editor-context-copy,
  .editor-context-actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .editor-context-copy {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .editor-context-copy > span:not(.editor-context-actions) {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .editor-context-copy small {
    flex: 0 0 auto;
    padding: 2px 6px;
    border: 1px solid color-mix(in srgb, var(--source-origin-local) 35%, var(--border-3));
    border-radius: 999px;
    color: var(--source-origin-local);
    font-size: 12px;
    font-weight: 800;
    background: var(--surface);
  }

  .editor-context-copy small.fixture {
    border-color: color-mix(in srgb, #d97706 38%, var(--border-3));
    color: #b45309;
  }

  .editor-context-bar strong {
    flex: 0 0 auto;
    color: var(--brand-strong);
    font-weight: 850;
  }

  .editor-context-actions {
    flex: 0 0 auto;
  }

  .editor-context-actions button {
    min-height: 32px;
    padding: 0 9px;
    border: 1px solid var(--border-3);
    border-radius: 6px;
    color: var(--text-strong);
    font-size: 12px;
    font-weight: 800;
    background: var(--surface);
    cursor: pointer;
  }

  .editor-context-actions button:hover:not(:disabled) {
    border-color: var(--brand);
  }

  .editor-context-actions button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .editor-context-actions button:focus-visible {
    outline: 2px solid var(--focus-ring, var(--brand));
    outline-offset: 2px;
  }

  .preview-stage {
    position: relative;
    display: block;
    box-sizing: border-box;
    width: 100%;
    height: 100%;
    min-height: 0;
    min-width: 0;
    padding: 0;
    overflow: auto;
    overscroll-behavior: contain;
    background: var(--wb-canvas-pasteboard, color-mix(in srgb, var(--surface-3) 88%, #8da39a));
  }

  .preview-stage.canvas-fixed {
    padding: 28px 24px 20px;
  }

  .preview-viewport-frame {
    position: relative;
    height: 100%;
    min-height: 240px;
    overflow: visible;
    background: #fff;
  }

  .canvas-fit .preview-viewport-frame {
    width: 100%;
  }

  .canvas-fixed .preview-viewport-frame {
    width: calc(var(--canvas-width-px) * var(--preview-zoom-scale));
    margin: 0 auto;
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--wb-border-strong, var(--border-4)) 78%, transparent),
      0 10px 28px rgba(16, 24, 21, 0.16);
  }

  .preview-page.interactive-background {
    visibility: hidden;
    pointer-events: none;
  }

  .preview-page {
    position: absolute;
    inset: 0 auto auto 0;
    display: block;
    width: 100%;
    max-width: none;
    height: 100%;
    min-height: 100%;
    margin: 0;
    border: 0;
    border-radius: 0;
    background: #ffffff;
    box-shadow: none;
    overflow: hidden;
    transform-origin: top left;
  }

  .canvas-fixed .preview-page {
    width: var(--canvas-width-px);
    height: calc(100% / var(--preview-zoom-scale));
    min-height: calc(100% / var(--preview-zoom-scale));
    transform: scale(var(--preview-zoom-scale));
  }

  .canvas-ruler {
    position: absolute;
    z-index: 5;
    left: 0;
    right: 0;
    top: -22px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 18px;
    padding: 0 3px;
    border-bottom: 1px solid color-mix(in srgb, var(--wb-text-muted, var(--text-muted)) 32%, transparent);
    color: var(--wb-text-muted, var(--text-muted));
    font-family: "JetBrains Mono", "SFMono-Regular", Consolas, monospace;
    font-size: 12px;
    pointer-events: none;
  }

  .canvas-ruler::after {
    position: absolute;
    inset: auto 0 -4px;
    height: 4px;
    background: repeating-linear-gradient(
      to right,
      color-mix(in srgb, var(--wb-text-muted, var(--text-muted)) 45%, transparent) 0 1px,
      transparent 1px 10px
    );
    content: "";
  }

  .canvas-ruler strong {
    padding: 1px 5px;
    border-radius: 999px;
    color: var(--wb-text-primary, var(--text));
    background: var(--wb-surface-chrome, var(--surface-2));
    font-weight: 750;
  }

  .viewport-resize-handle {
    position: absolute;
    z-index: 7;
    top: 50%;
    width: 14px;
    height: 42px;
    padding: 0;
    border: 1px solid var(--wb-border-strong, var(--border-4));
    border-radius: 5px;
    background: var(--wb-surface-document, var(--surface));
    box-shadow: 0 2px 8px rgba(16, 24, 21, 0.12);
    cursor: col-resize;
    transform: translateY(-50%);
    touch-action: none;
  }

  .viewport-resize-handle::after {
    position: absolute;
    inset: 8px 5px;
    border-inline: 1px solid var(--wb-text-muted, var(--text-muted));
    content: "";
  }

  .viewport-resize-handle.left {
    left: -8px;
  }

  .viewport-resize-handle.right {
    right: -8px;
  }

  .viewport-resize-handle:hover,
  .viewport-resize-handle:focus-visible {
    border-color: var(--wb-accent, var(--brand));
    outline: 2px solid var(--wb-focus-ring, var(--brand-strong));
    outline-offset: 1px;
  }

  :global(body.workbench-viewport-resizing) .preview-viewport-frame iframe {
    pointer-events: none;
  }

  .source-panel {
    height: 100%;
    min-height: 0;
    border-top: 1px solid var(--border);
    background: var(--surface-6);
  }

  .markdown-loading-stage {
    height: 100%;
    min-height: 0;
    background: var(--surface);
  }

  .source-stage {
    display: block;
    border-top: 0;
    overflow: hidden;
  }

  .code-source-layout {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    height: 100%;
    min-height: 0;
  }

  .hidden-stage {
    display: none;
  }

  .source-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    height: 34px;
    padding: 0 10px;
    border-bottom: 1px solid var(--border-3);
    color: var(--text-strong);
  }

  .source-header h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    margin: 0;
    overflow: hidden;
    color: var(--text-strong);
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: 13px;
    font-weight: 800;
  }

  .source-header h2 strong {
    color: var(--wb-accent-strong, var(--brand-strong));
    font-size: 12px;
    font-weight: 800;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .source-header h2 span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .source-meta {
    display: inline-flex;
    align-items: center;
    flex: 0 0 auto;
    min-height: 22px;
    padding: 0 7px;
    border: 1px solid var(--border-4);
    border-radius: 999px;
    color: var(--text-muted);
    font-family: "JetBrains Mono", "SFMono-Regular", Consolas, monospace;
    font-size: 12px;
    font-weight: 700;
    background: var(--surface-2);
  }

  .code-editor-host {
    min-height: 0;
    height: 100%;
    overflow: hidden;
  }
</style>
