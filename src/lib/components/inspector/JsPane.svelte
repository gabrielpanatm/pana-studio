<script lang="ts">
  import type { PageJsConfig, PanaMotionConfig, SelectionInfo } from "$lib/types";
  import { getPageJsWorkspaceState } from "$lib/project/io";
  import { emptyPageJsConfig, normalizePageJsConfig } from "$lib/js/page-config";
  import MotionStudioPanel from "$lib/components/inspector/js/MotionStudioPanel.svelte";
  import { normalizePageJsTemplatePath } from "$lib/js/page-path";
  import { queuePageJsDraftSync } from "$lib/session/page-js-draft-sync";
  import {
    createPageJsRequestIdentity,
    isPageJsRequestIdentityCurrent,
    pageJsCommandPayload,
  } from "$lib/session/page-js-command-session";

  let {
    selectedElement   = null,
    projectRoot       = "",
    runtimeSessionId  = "",
    refreshToken      = 0,
    onSwitchToHtml    = undefined as (() => void) | undefined,
  }: {
    selectedElement?: SelectionInfo | null;
    projectRoot?: string;
    runtimeSessionId?: string;
    refreshToken?: number;
    onSwitchToHtml?: () => void;
  } = $props();

  const templatePath = $derived.by(() => {
    const canonicalPath = normalizePageJsTemplatePath(selectedElement?.sourceLocation?.file);
    return canonicalPath || null;
  });
  const dataAnim = $derived(selectedElement?.attributes?.["data-anim"] ?? null);

  type PageJsLoadState = "idle" | "loading" | "ready" | "error";

  let config       = $state<PageJsConfig>(emptyPageJsConfig());
  let baseConfig   = $state<PageJsConfig>(emptyPageJsConfig());
  let pageJsLoadState = $state<PageJsLoadState>("idle");
  let pageJsLoadError = $state("");
  let readyTemplatePath: string | null = null;
  let readyProjectRoot = "";
  let readyRuntimeSessionId = "";
  let readyRefreshToken: number | null = null;
  let lastTplPath  = "";
  let lastProjectRoot = "";
  let lastRuntimeSessionId = "";
  let lastHandledRefreshToken: number | null = null;
  let loadSerial = 0;

  $effect(() => {
    const tpl = templatePath;
    const targetProjectRoot = projectRoot;
    const targetRuntimeSessionId = runtimeSessionId;
    const nextPath = tpl ?? "";
    if (
      nextPath === lastTplPath
      && targetProjectRoot === lastProjectRoot
      && targetRuntimeSessionId === lastRuntimeSessionId
    ) return;
    lastTplPath = nextPath;
    lastProjectRoot = targetProjectRoot;
    lastRuntimeSessionId = targetRuntimeSessionId;
    if (tpl && targetProjectRoot && targetRuntimeSessionId) {
      void loadConfig(tpl, targetProjectRoot, targetRuntimeSessionId);
    } else {
      loadSerial += 1;
      readyTemplatePath = null;
      readyProjectRoot = "";
      readyRuntimeSessionId = "";
      readyRefreshToken = null;
      pageJsLoadState = tpl ? "error" : "idle";
      pageJsLoadError = tpl
        ? "Sesiunea proiectului nu este disponibilă pentru citirea JavaScript-ului paginii."
        : "";
      baseConfig = emptyPageJsConfig();
      config = emptyPageJsConfig();
    }
  });

  $effect(() => {
    const token = refreshToken;
    const tpl = templatePath;
    if (lastHandledRefreshToken === null) {
      lastHandledRefreshToken = token;
      return;
    }
    if (token === lastHandledRefreshToken) return;
    lastHandledRefreshToken = token;
    if (!tpl || tpl !== lastTplPath) return;
    const targetProjectRoot = projectRoot;
    const targetRuntimeSessionId = runtimeSessionId;
    if (!targetProjectRoot || !targetRuntimeSessionId) return;
    void loadConfig(tpl, targetProjectRoot, targetRuntimeSessionId);
  });

  $effect(() => {
    return () => {
      loadSerial += 1;
      readyTemplatePath = null;
      readyProjectRoot = "";
      readyRuntimeSessionId = "";
      readyRefreshToken = null;
    };
  });

  async function loadConfig(
    tpl: string,
    targetProjectRoot = projectRoot,
    targetRuntimeSessionId = runtimeSessionId,
    targetRefreshToken = refreshToken,
  ) {
    const serial = ++loadSerial;
    readyTemplatePath = null;
    readyProjectRoot = "";
    readyRuntimeSessionId = "";
    readyRefreshToken = null;
    pageJsLoadState = "loading";
    pageJsLoadError = "";

    let nextBaseConfig: PageJsConfig;
    try {
      const identity = createPageJsRequestIdentity(targetProjectRoot, targetRuntimeSessionId);
      const receipt = await getPageJsWorkspaceState(tpl, identity);
      if (
        serial !== loadSerial
        || templatePath !== tpl
        || lastTplPath !== tpl
        || refreshToken !== targetRefreshToken
        || !isPageJsRequestIdentityCurrent(identity, projectRoot, runtimeSessionId)
      ) return;
      const workspaceState = pageJsCommandPayload(
        receipt,
        identity,
        "Citirea JavaScript-ului paginii din Inspector",
      );
      nextBaseConfig = normalizePageJsConfig(workspaceState.accepted);
      config = normalizePageJsConfig(workspaceState.current);
    } catch (error) {
      if (
        serial !== loadSerial
        || templatePath !== tpl
        || lastTplPath !== tpl
        || projectRoot !== targetProjectRoot
        || runtimeSessionId !== targetRuntimeSessionId
        || refreshToken !== targetRefreshToken
      ) return;
      pageJsLoadError = error instanceof Error ? error.message : String(error);
      pageJsLoadState = "error";
      return;
    }
    if (
      serial !== loadSerial
      || templatePath !== tpl
      || lastTplPath !== tpl
      || projectRoot !== targetProjectRoot
      || runtimeSessionId !== targetRuntimeSessionId
      || refreshToken !== targetRefreshToken
    ) return;
    baseConfig = nextBaseConfig;
    readyTemplatePath = tpl;
    readyProjectRoot = targetProjectRoot;
    readyRuntimeSessionId = targetRuntimeSessionId;
    readyRefreshToken = targetRefreshToken;
    pageJsLoadState = "ready";
  }

  function isConfigReadyForTemplate(tpl: string | null): tpl is string {
    return Boolean(
      tpl
      && pageJsLoadState === "ready"
      && readyTemplatePath === tpl
      && readyProjectRoot === projectRoot
      && readyRuntimeSessionId === runtimeSessionId
      && readyRefreshToken === refreshToken
      && lastTplPath === tpl
      && lastProjectRoot === projectRoot
      && lastRuntimeSessionId === runtimeSessionId
    );
  }

  function retryLoadConfig() {
    const tpl = templatePath;
    if (!tpl || tpl !== lastTplPath) return;
    if (!projectRoot || !runtimeSessionId) return;
    void loadConfig(tpl, projectRoot, runtimeSessionId);
  }

  function stageConfig(nextConfig: PageJsConfig) {
    const targetPath = templatePath;
    if (!isConfigReadyForTemplate(targetPath)) return;
    const nextNormalized = normalizePageJsConfig(nextConfig);
    config = nextNormalized;
    queuePageJsDraftSync({
      templatePath: targetPath,
      baseConfig,
      currentConfig: nextNormalized,
      cachebustAssets: false,
      source: "inspector.js",
      coalesceKey: "page_js.motion",
    });
  }

  function updateMotionConfig(motion: PanaMotionConfig) {
    stageConfig({ ...config, version: 1, motion });
  }

</script>

<div class="js-pane">
  {#if !selectedElement}
    <p class="jp-hint">Selectează un element din canvas pentru Motion.</p>

  {:else if !dataAnim}
    <p class="jp-hint">
      Elementul selectat nu are atribut <code>data-anim</code>.<br>
      Adaugă-l în tab-ul HTML pentru a configura Motion pe acest element.
    </p>
    {#if onSwitchToHtml}
      <button type="button" class="jp-switch-btn" onclick={onSwitchToHtml}>
        Mergi la tab HTML
      </button>
    {/if}

  {:else if !templatePath}
    <div class="jp-load-state">
      <strong>JS indisponibil</strong>
      <span>Elementul selectat nu are un template sursă activ.</span>
    </div>

  {:else if pageJsLoadState === "error"}
    <div class="jp-load-state jp-load-error" role="alert">
      <strong>JS-ul paginii nu a putut fi încărcat</strong>
      <span>{pageJsLoadError || "Citirea configurației Motion a eșuat."}</span>
      <button type="button" onclick={retryLoadConfig}>Reîncearcă</button>
    </div>

  {:else if !isConfigReadyForTemplate(templatePath)}
    <div class="jp-load-state" aria-live="polite">
      <strong>Motion</strong>
      <span>Se citește JS-ul paginii curente…</span>
    </div>

  {:else}
    <div class="jp-target-bar">
      <span class="jp-target-label">data-anim</span>
      <span class="jp-target-value">{dataAnim}</span>
    </div>
    <div class="jp-context-note">
      <strong>Efecte element</strong>
      <span>Elementul selectat primește efecte proprii; cronologia paginii rămâne sub previzualizare.</span>
    </div>
    <div class="jp-design-safe-note" role="status">
      <strong>Editare sigură · JavaScript oprit</strong>
      <span>Editarea și salvarea rămân active. Folosește deschiderea externă pentru execuția efectelor.</span>
    </div>

    <MotionStudioPanel
      motion={config.motion}
      dataAnim={dataAnim}
      onChange={updateMotionConfig}
    />
  {/if}
</div>

<style>
  .js-pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .jp-hint {
    margin: 16px 12px;
    font-size: 12px;
    color: var(--text-muted);
    line-height: 1.6;
    text-align: center;
  }

  .jp-hint code {
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
    color: var(--text);
  }

  .jp-switch-btn {
    display: block;
    margin: 0 12px 12px;
    width: calc(100% - 24px);
    min-height: 30px;
    border: 1px solid var(--brand);
    border-radius: 7px;
    background: var(--brand-soft);
    color: var(--brand-strong);
    font-size: 12px;
    font-weight: 800;
    cursor: pointer;
  }

  .jp-load-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    margin: 16px 12px;
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.45;
    text-align: center;
  }

  .jp-load-state strong {
    color: var(--text);
    font-size: 12px;
  }

  .jp-load-state button {
    min-height: 28px;
    padding: 0 12px;
    border: 1px solid var(--brand);
    border-radius: 6px;
    background: var(--brand-soft);
    color: var(--brand-strong);
    font-size: 12px;
    font-weight: 900;
    cursor: pointer;
  }

  .jp-load-error span {
    overflow-wrap: anywhere;
  }

  .jp-target-bar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border-bottom: 1px solid var(--border-3);
    background: var(--surface-3);
  }

  .jp-target-label {
    font-size: 12px;
    font-weight: 800;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .jp-target-value {
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
    font-weight: 800;
    color: var(--brand-strong);
  }

  .jp-context-note {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 7px 12px;
    border-bottom: 1px solid var(--border-3);
    background: color-mix(in srgb, var(--brand-soft) 34%, var(--surface-2));
  }

  .jp-context-note strong {
    font-size: 12px;
    font-weight: 900;
    color: var(--text);
  }

  .jp-context-note span {
    font-size: 12px;
    line-height: 1.35;
    color: var(--text-muted);
  }

  .jp-design-safe-note {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 7px 12px;
    border-bottom: 1px solid var(--border-3);
    background: var(--surface-3);
  }

  .jp-design-safe-note strong {
    color: var(--brand-strong);
    font-size: 12px;
    font-weight: 900;
  }

  .jp-design-safe-note span {
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.35;
  }
</style>
