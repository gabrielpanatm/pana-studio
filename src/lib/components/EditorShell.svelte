<script lang="ts">
  import MarkdownEditor from "$lib/components/markdown/MarkdownEditor.svelte";
  import InteractivePreviewSurface from "$lib/components/preview/InteractivePreviewSurface.svelte";
  import { resetPreviewFrameDocumentAccess } from "$lib/preview/frame-origin";
  import type { SourceLanguage, TemplateWorkbenchPlan } from "$lib/types";
  import type { InteractivePreviewDomNode } from "$lib/preview/interactive";

  type CenterView = "preview" | "code" | "markdown" | "canvas" | "site" | "kernel";
  type PreviewDevice = "desktop" | "tablet" | "mobile";

  export let centerView: CenterView = "preview";
  export let previewDevice: PreviewDevice = "desktop";
  export let previewZoom = 100;
  export let tabletPreviewWidth = "1024px";
  export let mobilePreviewWidth = "768px";
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

  function handlePreviewLoad() {
    if (previewFrame) resetPreviewFrameDocumentAccess(previewFrame);
    attachPreviewInspector();
  }
</script>

<section class="editor-shell" aria-label="Preview si cod sursa">
  <div class:hidden-stage={centerView !== "preview"} class="preview-shell">
    {#if templateWorkbenchActive}
      <div class="workbench-banner">
        <span class="workbench-banner-copy">
          <strong>Template Workbench</strong>
          <span>{templateWorkbenchTarget}</span>
          {#if templateWorkbenchPlan}
            <small class:fixture={!templateWorkbenchPlan.renderContext.canonicalTruth}
              title={templateWorkbenchPlan.renderContext.explanation}>
              {templateWorkbenchPlan.renderContext.label}
            </small>
          {/if}
        </span>
        <button type="button" onclick={() => { void exitTemplateWorkbench(); }}>Înapoi la site</button>
      </div>
    {/if}
    <div class="design-safe-banner" role="status">
      <strong>{interactivePreviewEnabled ? "Interactive Preview" : "Design Safe"}</strong>
      <span>
        {interactivePreviewEnabled
          ? `JavaScript izolat · DOM read-only ${interactiveDomNodeCount} noduri · fără acces la Tauri sau fișiere.`
          : "JavaScript-ul proiectului este oprit în editor. Run extern execută site-ul complet."}
      </span>
      <button
        type="button"
        disabled={!interactivePreviewEnabled && !interactivePreviewUrl}
        onclick={() => setInteractivePreviewEnabled(!interactivePreviewEnabled)}
      >
        {interactivePreviewEnabled ? "Revino la Design Safe" : "Pornește interactiv"}
      </button>
    </div>
    <div class="preview-stage">
      <iframe
        bind:this={previewFrame}
        class:desktop={previewDevice === "desktop"}
        class:tablet={previewDevice === "tablet"}
        class:mobile={previewDevice === "mobile"}
        class:interactive-background={interactivePreviewEnabled}
        class="preview-page"
        style={`--preview-zoom-scale: ${previewZoom / 100}; --tablet-preview-width: ${tabletPreviewWidth}; --mobile-preview-width: ${mobilePreviewWidth};`}
        title="Preview basic HTML CSS test site"
        src={previewDocumentMarkup ? undefined : previewSrc}
        srcdoc={previewDocumentMarkup ?? undefined}
        sandbox={previewDocumentMarkup ? "" : "allow-scripts"}
        onload={handlePreviewLoad}
      ></iframe>
      {#if interactivePreviewEnabled}
        <InteractivePreviewSurface
          desiredUrl={interactivePreviewUrl}
          {previewDevice}
          {previewZoom}
          {tabletPreviewWidth}
          {mobilePreviewWidth}
          onDomSnapshot={onInteractiveDomSnapshot}
          onLifecycleError={onInteractiveLifecycleError}
          onRealmRestarted={onInteractiveRealmRestarted}
          onRealmFailed={onInteractiveRealmFailed}
        />
      {/if}
    </div>
  </div>

  <section
    class:hidden-stage={centerView !== "code" && centerView !== "markdown"}
    class="source-panel source-stage"
    id="source"
    aria-label="Cod sursa"
  >
    {#if centerView === "markdown" && sourceLanguage === "markdown" && !sourceIsLoading}
      {#key currentSourcePath}
        <MarkdownEditor
          {source}
          path={currentSourcePath}
          {refreshToken}
          readOnly={editorReadOnly}
          onChange={onMarkdownChange}
        />
      {/key}
    {:else if centerView === "markdown" && sourceLanguage === "markdown"}
      <div class="markdown-loading-stage" aria-label="Se încarcă fișierul Markdown"></div>
    {:else}
      <div class="code-source-layout">
        <div class="source-header">
          <h2 title={currentSourcePath}>{currentSourcePath || "Niciun fișier deschis"}</h2>
          <span class="source-meta">{sourceLanguage.toUpperCase()} • {sourceLength} chars</span>
        </div>
        <div bind:this={codeEditorHost} class="code-editor-host" data-language={sourceLanguage}></div>
      </div>
    {/if}
  </section>
</section>

<style>
  .editor-shell {
    position: relative;
    display: grid;
    grid-template-rows: minmax(0, 1fr);
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

  .workbench-banner {
    position: relative;
    z-index: 2;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 34px;
    padding: 0 10px;
    border-bottom: 1px solid color-mix(in srgb, #7c3aed 24%, var(--border-3));
    color: var(--text);
    font-size: 12px;
    background: color-mix(in srgb, #7c3aed 8%, var(--surface));
  }

  .workbench-banner-copy {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .workbench-banner-copy > span {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .workbench-banner-copy small {
    flex: 0 0 auto;
    padding: 2px 6px;
    border: 1px solid color-mix(in srgb, var(--source-origin-local) 35%, var(--border-3));
    border-radius: 999px;
    color: var(--source-origin-local);
    font-size: 9px;
    font-weight: 800;
    background: var(--surface);
  }

  .workbench-banner-copy small.fixture {
    border-color: color-mix(in srgb, #d97706 38%, var(--border-3));
    color: #b45309;
  }

  .workbench-banner strong {
    margin-right: 6px;
    color: #6d28d9;
    font-weight: 850;
  }

  .workbench-banner button {
    flex: 0 0 auto;
    min-height: 24px;
    padding: 0 9px;
    border: 1px solid color-mix(in srgb, #7c3aed 32%, var(--border-3));
    border-radius: 6px;
    color: #5b21b6;
    font-size: 11px;
    font-weight: 800;
    background: var(--surface);
    cursor: pointer;
  }

  .design-safe-banner {
    position: relative;
    z-index: 2;
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 28px;
    padding: 0 10px;
    border-bottom: 1px solid var(--border-2);
    color: var(--text-muted);
    font-size: 11px;
    background: var(--surface-2);
  }

  .design-safe-banner strong {
    flex: 0 0 auto;
    color: var(--brand-strong);
    font-weight: 850;
  }

  .design-safe-banner span {
    flex: 1 1 auto;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .design-safe-banner button {
    flex: 0 0 auto;
    min-height: 23px;
    padding: 0 8px;
    border: 1px solid var(--border-3);
    border-radius: 6px;
    color: var(--text-strong);
    font-size: 10px;
    font-weight: 750;
    background: var(--surface);
    cursor: pointer;
  }

  .design-safe-banner button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .preview-stage {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: flex-start;
    width: 100%;
    height: 100%;
    min-height: 0;
    min-width: 0;
    padding: 0;
    overflow: hidden;
    overscroll-behavior: contain;
    background: transparent;
  }

  .preview-page.interactive-background {
    visibility: hidden;
    pointer-events: none;
  }

  .preview-page {
    flex: 0 0 auto;
    display: block;
    width: calc(100% / var(--preview-zoom-scale));
    max-width: none;
    height: calc(100% / var(--preview-zoom-scale));
    min-height: calc(100% / var(--preview-zoom-scale));
    margin: 0;
    border: 0;
    border-radius: 0;
    background: #ffffff;
    box-shadow: none;
    overflow: hidden;
    transform: scale(var(--preview-zoom-scale));
    transform-origin: top center;
  }

  .preview-page.tablet {
    width: min(calc(var(--tablet-preview-width) / var(--preview-zoom-scale)), calc(100% / var(--preview-zoom-scale)));
    max-width: none;
  }

  .preview-page.mobile {
    width: min(calc(var(--mobile-preview-width) / var(--preview-zoom-scale)), calc(100% / var(--preview-zoom-scale)));
    max-width: none;
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
    min-width: 0;
    margin: 0;
    overflow: hidden;
    color: var(--text-strong);
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: 13px;
    font-weight: 800;
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
    font-size: 11px;
    font-weight: 700;
    background: var(--surface-2);
  }

  .code-editor-host {
    min-height: 0;
    height: 100%;
    overflow: hidden;
  }
</style>
