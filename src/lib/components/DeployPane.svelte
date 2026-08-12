<script lang="ts">
  import {
    IconHammer,
    IconX,
  } from "@tabler/icons-svelte";
  import { t } from "$lib/i18n/runtime.svelte";
  import {
    readProjectAppConfig,
    readZolaProjectSettings,
    saveProjectAppConfig,
    saveZolaProjectSettings,
    zolaBuild,
    cancelPublishOperation,
  } from "$lib/project/io";
  import {
    appConfigDraftFromConfig,
    appConfigFromDraft,
    createDefaultZolaSettings,
    textFieldsFromZolaSettings,
    zolaSettingsWithTextFields,
    type ProjectAppConfig,
  } from "$lib/project/deploy-settings";
  import DeployTargetsPanel from "$lib/components/deploy/DeployTargetsPanel.svelte";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import type { ZolaProjectSettings } from "$lib/types";
  import type { AppState } from "$lib/state/app.svelte";
  import { errorMessage } from "$lib/util";

  let {
    scannedProject = false,
    cachebustAssets = false,
    workspaceMode = false,
    actionsOnly = false,
    projectRoot = "",
    runtimeSessionId = "",
    app = undefined as AppState | undefined,
    onStatusUpdate = undefined as ((text: string, kind: string) => void) | undefined,
    onCachebustAssetsChange = undefined as ((value: boolean) => void) | undefined,
  }: {
    scannedProject?: boolean;
    cachebustAssets?: boolean;
    workspaceMode?: boolean;
    actionsOnly?: boolean;
    projectRoot?: string;
    runtimeSessionId?: string;
    app?: AppState;
    onStatusUpdate?: (text: string, kind: string) => void;
    onCachebustAssetsChange?: (value: boolean) => void;
  } = $props();

  let zolaSettings = $state<ZolaProjectSettings>(createDefaultZolaSettings());
  let cachebustAssetsDraft = $state(false);
  let feedFilenamesText = $state("");
  let feedLimitText = $state("");
  let searchTruncateText = $state("");
  let loading = $state(false);
  let configLoaded = $state(false);
  let configDirty = $state(false);

  let buildRunning = $state(false);
  let deployRunning = $state(false);
  let cancelRunning = $state(false);
  const insertAnchorOptions = ["none", "left", "right", "heading"];
  const searchIndexFormatOptions = ["elasticlunr_javascript", "elasticlunr_json", "fuse_javascript", "fuse_json"];
  let actionLog = $state("");
  let actionOk = $state<boolean | null>(null);
  const publishReady = $derived(app?.currentPublishPreflightReceipt()?.status === "ready");

  $effect(() => {
    if (scannedProject) loadConfig();
  });

  $effect(() => {
    cachebustAssetsDraft = cachebustAssets;
  });

  async function loadConfig() {
    loading = true;
    configLoaded = false;
    try {
      const [settings, appConfig] = await Promise.all([
        readZolaProjectSettings(),
        readProjectAppConfig(),
      ]);
      zolaSettings = settings;
      syncAppConfigFields(appConfig);
      syncTextFields(settings);
      onCachebustAssetsChange?.(appConfig.cachebustAssets);
      configDirty = false;
      app?.invalidatePublishAuthorization();
    } catch (e) {
      onStatusUpdate?.(t("deploy-config-load-error", { error: errorMessage(e) }), "error");
    }
    configLoaded = true;
    loading = false;
  }

  async function saveConfig() {
    onStatusUpdate?.(t("deploy-config-saving"), "saving");
    try {
      const settingsToSave = zolaSettingsWithTextFields(zolaSettings, {
        feedFilenamesText,
        feedLimitText,
        searchTruncateText,
      });
      const [savedSettings, appConfig] = await Promise.all([
        saveZolaProjectSettings(settingsToSave),
        saveProjectAppConfig(appConfigFromDraft({
          cachebustAssetsDraft,
        })),
      ]);
      zolaSettings = savedSettings;
      syncAppConfigFields(appConfig);
      syncTextFields(savedSettings);
      configDirty = false;
      onCachebustAssetsChange?.(appConfig.cachebustAssets);
      onStatusUpdate?.(t("deploy-config-saved"), "saved");
    } catch (e) {
      onStatusUpdate?.(t("deploy-config-error", { error: errorMessage(e) }), "error");
    }
  }

  function syncTextFields(settings: ZolaProjectSettings) {
    const textFields = textFieldsFromZolaSettings(settings);
    feedFilenamesText = textFields.feedFilenamesText;
    feedLimitText = textFields.feedLimitText;
    searchTruncateText = textFields.searchTruncateText;
  }

  function syncAppConfigFields(config: ProjectAppConfig) {
    const draft = appConfigDraftFromConfig(config);
    cachebustAssetsDraft = draft.cachebustAssetsDraft;
  }

  function markConfigDirty() {
    if (!configLoaded || loading) return;
    if (!configDirty) {
      onStatusUpdate?.(t("deploy-config-dirty-status"), "unsaved");
    }
    configDirty = true;
  }

  function setSetting<K extends keyof ZolaProjectSettings>(key: K, value: ZolaProjectSettings[K]) {
    zolaSettings = { ...zolaSettings, [key]: value };
    markConfigDirty();
  }

  async function runBuild() {
    buildRunning = true;
    actionLog = "";
    actionOk = null;
    onStatusUpdate?.(t("deploy-build-running-status"), "saving");
    try {
      if (workspaceMode) {
        if (!app) throw new Error(t("publish-build-requires-preflight"));
        const receipt = await app.buildForPublish();
        actionLog = receipt.log || t("publish-build-receipt-summary", {
          files: receipt.artifactFiles,
          bytes: receipt.artifactBytes,
        });
      } else {
        actionLog = await zolaBuild();
      }
      actionOk = true;
      onStatusUpdate?.(t("deploy-build-complete"), "saved");
    } catch (e) {
      actionLog = errorMessage(e);
      actionOk = false;
      onStatusUpdate?.(t("deploy-build-error", { error: actionLog }), "error");
    }
    buildRunning = false;
  }

  async function cancelRunningOperation() {
    if (cancelRunning || !buildRunning) return;
    if (!projectRoot || !runtimeSessionId) {
      onStatusUpdate?.(t("deploy-cancel-no-session"), "error");
      return;
    }
    cancelRunning = true;
    try {
      const receipt = await cancelPublishOperation({
        expectedProjectRoot: projectRoot,
        expectedSessionId: runtimeSessionId,
      });
      actionLog = t("deploy-cancel-log", { kind: receipt.kind, operation: receipt.operationId });
      actionOk = null;
      onStatusUpdate?.(t("deploy-cancel-requested"), "saving");
    } catch (error) {
      onStatusUpdate?.(t("deploy-cancel-failed", { error: errorMessage(error) }), "error");
    } finally {
      cancelRunning = false;
    }
  }
</script>

<div class:workspace-mode={workspaceMode} class:actions-only={actionsOnly} class="deploy-pane">

  {#if !scannedProject}
    <p class="hint">{t("deploy-open-folder")}</p>
  {:else if loading}
    <p class="hint">{t("deploy-config-loading")}</p>
  {:else}
    <div class="sticky-config-actions">
      <span class:dirty={configDirty}>{configDirty ? t("deploy-unsaved") : t("deploy-synchronized")}</span>
      <div class="sticky-action-buttons">
        <button type="button" class="save-config-btn compact-save" onclick={saveConfig}>
          {t("deploy-save-config")}
        </button>
        {#if workspaceMode}
          <button type="button" class="action-btn build-btn compact-action" onclick={runBuild} disabled={buildRunning || deployRunning || configDirty || !publishReady} title={configDirty ? t("deploy-save-before-build") : !publishReady ? t("publish-build-requires-preflight") : t("deploy-build-title")}>
            <IconHammer size={14} stroke={1.8} />
            {buildRunning ? t("deploy-building") : t("deploy-build")}
          </button>
          {#if buildRunning}
            <button type="button" class="action-btn cancel-btn compact-action" onclick={cancelRunningOperation} disabled={cancelRunning}>
              <IconX size={14} stroke={2} /> {cancelRunning ? t("deploy-cancelling") : t("deploy-cancel")}
            </button>
          {/if}
        {/if}
      </div>
    </div>

    {#if workspaceMode && actionLog}
      <div class="log-box workspace-log" class:log-ok={actionOk === true} class:log-err={actionOk === false} aria-live="polite">
        <pre class="log-text">{actionLog}</pre>
      </div>
    {/if}

    <DeployTargetsPanel
      {app}
      {scannedProject}
      {actionsOnly}
      {projectRoot}
      {runtimeSessionId}
      disabled={configDirty || buildRunning}
      {onStatusUpdate}
      onRunningChange={(running) => { deployRunning = running; }}
    />

    {#if !actionsOnly}
    <section class="config-section">
      <div class="section-title-row">
        <h3 class="section-label">{t("deploy-section-project")}</h3>
        <code>{zolaSettings.configPath}</code>
      </div>
      <label class="config-field">
        <span>base_url</span>
        <input type="url" class="config-input" placeholder={t("deploy-placeholder-url")} value={zolaSettings.baseUrl}
          oninput={(event) => setSetting("baseUrl", event.currentTarget.value)} />
      </label>
      <label class="config-field">
        <span>title</span>
        <input class="config-input" placeholder={t("deploy-placeholder-title")} value={zolaSettings.title}
          oninput={(event) => setSetting("title", event.currentTarget.value)} />
      </label>
      <label class="config-field">
        <span>description</span>
        <textarea class="config-textarea" rows="3" placeholder={t("deploy-placeholder-description")}
          value={zolaSettings.description}
          oninput={(event) => setSetting("description", event.currentTarget.value)}></textarea>
      </label>
      <div class="field-grid">
        <label class="config-field">
          <span>default_language</span>
          <input class="config-input" placeholder="ro" value={zolaSettings.defaultLanguage}
            oninput={(event) => setSetting("defaultLanguage", event.currentTarget.value)} />
        </label>
        <label class="config-field">
          <span>author</span>
          <input class="config-input" placeholder={t("deploy-placeholder-author")} value={zolaSettings.author}
            oninput={(event) => setSetting("author", event.currentTarget.value)} />
        </label>
      </div>
    </section>

    <section class="config-section">
      <h3 class="section-label">{t("deploy-section-build")}</h3>
      <label class="switch-field">
        <span><strong>{t("deploy-compile-sass")}</strong><small>{t("deploy-compile-sass-help")}</small></span>
        <input type="checkbox" role="switch" checked={zolaSettings.compileSass}
          onchange={(event) => setSetting("compileSass", event.currentTarget.checked)} />
        <i aria-hidden="true"></i>
      </label>
      <label class="switch-field">
        <span><strong>{t("deploy-minify-html")}</strong><small>{t("deploy-minify-html-help")}</small></span>
        <input type="checkbox" role="switch" checked={zolaSettings.minifyHtml}
          onchange={(event) => setSetting("minifyHtml", event.currentTarget.checked)} />
        <i aria-hidden="true"></i>
      </label>
      <label class="config-field">
        <span>output_dir</span>
        <input class="config-input" placeholder="public" value={zolaSettings.outputDir}
          oninput={(event) => setSetting("outputDir", event.currentTarget.value)} />
      </label>
      <label class="switch-field">
        <span>
          <strong>{t("deploy-cachebust")}</strong>
          <small>{t("deploy-cachebust-help")}</small>
        </span>
        <input
          type="checkbox"
          role="switch"
          checked={cachebustAssetsDraft}
          onchange={(event) => {
            cachebustAssetsDraft = event.currentTarget.checked;
            markConfigDirty();
          }}
        />
        <i aria-hidden="true"></i>
      </label>
    </section>

    <section class="config-section">
      <h3 class="section-label">{t("deploy-section-seo")}</h3>
      <label class="switch-field">
        <span><strong>{t("deploy-sitemap")}</strong><small>{t("deploy-sitemap-help")}</small></span>
        <input type="checkbox" role="switch" checked={zolaSettings.generateSitemap}
          onchange={(event) => setSetting("generateSitemap", event.currentTarget.checked)} />
        <i aria-hidden="true"></i>
      </label>
      <label class="switch-field">
        <span><strong>robots.txt</strong><small>{t("deploy-robots-help")}</small></span>
        <input type="checkbox" role="switch" checked={zolaSettings.generateRobotsTxt}
          onchange={(event) => setSetting("generateRobotsTxt", event.currentTarget.checked)} />
        <i aria-hidden="true"></i>
      </label>
      <label class="switch-field">
        <span><strong>{t("deploy-exclude-paginated")}</strong><small>{t("deploy-exclude-paginated-help")}</small></span>
        <input type="checkbox" role="switch" checked={zolaSettings.excludePaginatedPagesInSitemap}
          onchange={(event) => setSetting("excludePaginatedPagesInSitemap", event.currentTarget.checked)} />
        <i aria-hidden="true"></i>
      </label>
      <label class="switch-field">
        <span><strong>{t("deploy-feeds")}</strong><small>{t("deploy-feeds-help")}</small></span>
        <input type="checkbox" role="switch" checked={zolaSettings.generateFeeds}
          onchange={(event) => setSetting("generateFeeds", event.currentTarget.checked)} />
        <i aria-hidden="true"></i>
      </label>
      <div class="field-grid">
        <label class="config-field">
          <span>feed_filenames</span>
          <input class="config-input" placeholder="atom.xml, rss.xml" value={feedFilenamesText}
            oninput={(event) => {
              feedFilenamesText = event.currentTarget.value;
              markConfigDirty();
            }} />
        </label>
        <label class="config-field">
          <span>feed_limit</span>
          <input class="config-input" type="number" min="0" placeholder={t("deploy-unlimited-placeholder")} value={feedLimitText}
            oninput={(event) => {
              feedLimitText = event.currentTarget.value;
              markConfigDirty();
            }} />
        </label>
      </div>
    </section>

    <section class="config-section">
      <h3 class="section-label">{t("deploy-section-markdown")}</h3>
      <div class="field-grid">
        <label class="switch-field compact">
          <span><strong>Emoji</strong></span>
          <input type="checkbox" role="switch" checked={zolaSettings.renderEmoji}
            onchange={(event) => setSetting("renderEmoji", event.currentTarget.checked)} />
          <i aria-hidden="true"></i>
        </label>
        <label class="switch-field compact">
          <span><strong>{t("deploy-smart-punctuation")}</strong></span>
          <input type="checkbox" role="switch" checked={zolaSettings.smartPunctuation}
            onchange={(event) => setSetting("smartPunctuation", event.currentTarget.checked)} />
          <i aria-hidden="true"></i>
        </label>
        <label class="switch-field compact">
          <span><strong>{t("deploy-lazy-images")}</strong></span>
          <input type="checkbox" role="switch" checked={zolaSettings.lazyAsyncImage}
            onchange={(event) => setSetting("lazyAsyncImage", event.currentTarget.checked)} />
          <i aria-hidden="true"></i>
        </label>
        <label class="switch-field compact">
          <span><strong>GitHub alerts</strong></span>
          <input type="checkbox" role="switch" checked={zolaSettings.githubAlerts}
            onchange={(event) => setSetting("githubAlerts", event.currentTarget.checked)} />
          <i aria-hidden="true"></i>
        </label>
        <label class="switch-field compact">
          <span><strong>{t("deploy-bottom-footnotes")}</strong></span>
          <input type="checkbox" role="switch" checked={zolaSettings.bottomFootnotes}
            onchange={(event) => setSetting("bottomFootnotes", event.currentTarget.checked)} />
          <i aria-hidden="true"></i>
        </label>
      </div>
      <label class="config-field">
        <span>insert_anchor_links</span>
        <SelectControl value={zolaSettings.insertAnchorLinks} options={insertAnchorOptions} ariaLabel={t("deploy-anchor-links-label")} onchange={(value) => setSetting("insertAnchorLinks", value)} />
      </label>
      <h4 class="subsection-label">{t("deploy-external-links")}</h4>
      <div class="field-grid">
        <label class="switch-field compact">
          <span><strong>target blank</strong></span>
          <input type="checkbox" role="switch" checked={zolaSettings.externalLinksTargetBlank}
            onchange={(event) => setSetting("externalLinksTargetBlank", event.currentTarget.checked)} />
          <i aria-hidden="true"></i>
        </label>
        <label class="switch-field compact">
          <span><strong>nofollow</strong></span>
          <input type="checkbox" role="switch" checked={zolaSettings.externalLinksNoFollow}
            onchange={(event) => setSetting("externalLinksNoFollow", event.currentTarget.checked)} />
          <i aria-hidden="true"></i>
        </label>
        <label class="switch-field compact">
          <span><strong>noreferrer</strong></span>
          <input type="checkbox" role="switch" checked={zolaSettings.externalLinksNoReferrer}
            onchange={(event) => setSetting("externalLinksNoReferrer", event.currentTarget.checked)} />
          <i aria-hidden="true"></i>
        </label>
      </div>
    </section>

    <section class="config-section">
      <h3 class="section-label">{t("deploy-section-search")}</h3>
      <label class="switch-field">
        <span><strong>{t("deploy-search-index")}</strong><small>{t("deploy-search-index-help")}</small></span>
        <input type="checkbox" role="switch" checked={zolaSettings.buildSearchIndex}
          onchange={(event) => setSetting("buildSearchIndex", event.currentTarget.checked)} />
        <i aria-hidden="true"></i>
      </label>
      <label class="config-field">
        <span>index_format</span>
        <SelectControl value={zolaSettings.searchIndexFormat} options={searchIndexFormatOptions} ariaLabel={t("deploy-search-format-label")} onchange={(value) => setSetting("searchIndexFormat", value)} />
      </label>
      <div class="field-grid">
        <label class="switch-field compact">
          <span><strong>{t("deploy-search-title")}</strong></span>
          <input type="checkbox" role="switch" checked={zolaSettings.searchIncludeTitle}
            onchange={(event) => setSetting("searchIncludeTitle", event.currentTarget.checked)} />
          <i aria-hidden="true"></i>
        </label>
        <label class="switch-field compact">
          <span><strong>{t("deploy-search-description")}</strong></span>
          <input type="checkbox" role="switch" checked={zolaSettings.searchIncludeDescription}
            onchange={(event) => setSetting("searchIncludeDescription", event.currentTarget.checked)} />
          <i aria-hidden="true"></i>
        </label>
        <label class="switch-field compact">
          <span><strong>{t("deploy-search-date")}</strong></span>
          <input type="checkbox" role="switch" checked={zolaSettings.searchIncludeDate}
            onchange={(event) => setSetting("searchIncludeDate", event.currentTarget.checked)} />
          <i aria-hidden="true"></i>
        </label>
        <label class="switch-field compact">
          <span><strong>{t("deploy-search-path")}</strong></span>
          <input type="checkbox" role="switch" checked={zolaSettings.searchIncludePath}
            onchange={(event) => setSetting("searchIncludePath", event.currentTarget.checked)} />
          <i aria-hidden="true"></i>
        </label>
        <label class="switch-field compact">
          <span><strong>{t("deploy-search-content")}</strong></span>
          <input type="checkbox" role="switch" checked={zolaSettings.searchIncludeContent}
            onchange={(event) => setSetting("searchIncludeContent", event.currentTarget.checked)} />
          <i aria-hidden="true"></i>
        </label>
      </div>
      <label class="config-field">
        <span>truncate_content_length</span>
        <input class="config-input" type="number" min="0" placeholder={t("deploy-complete-placeholder")} value={searchTruncateText}
          oninput={(event) => {
            searchTruncateText = event.currentTarget.value;
            markConfigDirty();
          }} />
      </label>
    </section>

    <button type="button" class="save-config-btn" onclick={saveConfig}>
      {t("deploy-save-config")}
    </button>

    <div class="divider"></div>

    {#if !workspaceMode}<section class="actions-section">
      <button type="button" class="action-btn build-btn" onclick={runBuild} disabled={buildRunning || deployRunning}>
        <IconHammer size={14} stroke={1.8} />
        {buildRunning ? t("deploy-building") : t("deploy-build-zola")}
      </button>
    </section>{/if}
    {/if}

  {/if}

  {#if actionLog && !workspaceMode}
    <div class="log-box" class:log-ok={actionOk === true} class:log-err={actionOk === false}>
      <pre class="log-text">{actionLog}</pre>
    </div>
  {/if}

</div>

<style>
  .deploy-pane {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 2px 0;
  }

  .sticky-config-actions {
    position: sticky;
    top: -2px;
    z-index: 4;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    padding: 7px;
    border: 1px solid var(--border-3);
    border-radius: 8px;
    background: color-mix(in srgb, var(--surface-2) 92%, transparent);
    box-shadow: 0 10px 24px color-mix(in srgb, #000 8%, transparent);
    backdrop-filter: blur(10px);
  }

  .sticky-action-buttons {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .compact-action {
    width: auto;
    min-height: 28px;
    padding: 0 10px;
    border-radius: 7px;
    white-space: nowrap;
  }

  .workspace-mode {
    gap: 12px;
    min-width: 0;
  }

  .workspace-mode .sticky-config-actions {
    top: 0;
    grid-template-columns: minmax(130px, 1fr) auto;
    border-color: var(--wb-border-subtle, var(--border-3));
    background: color-mix(in srgb, var(--wb-surface-chrome, var(--surface-2)) 94%, transparent);
  }

  .workspace-mode.actions-only .sticky-config-actions {
    position: static;
  }

  .workspace-log {
    max-height: 240px;
    overflow: auto;
  }

  .sticky-config-actions span {
    min-width: 0;
    overflow: hidden;
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 800;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sticky-config-actions span.dirty {
    color: #b45309;
  }

  .hint {
    margin: 0;
    color: var(--text-muted);
    font-size: 12px;
  }

  .config-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px;
    border: 1px solid var(--border-2);
    border-radius: 8px;
    background: color-mix(in srgb, var(--surface-4) 62%, transparent);
  }

  .section-title-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .section-title-row code {
    max-width: 150px;
    overflow: hidden;
    color: var(--text-muted);
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .section-label {
    margin: 0;
    font-size: 12px;
    font-weight: 900;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .subsection-label {
    margin: 2px 0 0;
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 850;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  .field-grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 7px;
  }

  .config-field {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 12px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
  }

  .config-input {
    width: 100%;
    height: 28px;
    padding: 0 7px;
    border: 1px solid var(--border-4);
    border-radius: 7px;
    background: var(--surface-5);
    color: var(--text);
    font-size: 12px;
    font-family: "JetBrains Mono", monospace;
    outline: none;
    box-sizing: border-box;
    transition: border-color 80ms;
  }

  .config-input:focus { border-color: var(--brand); }

  .config-textarea {
    width: 100%;
    min-height: 68px;
    resize: vertical;
    padding: 7px;
    border: 1px solid var(--border-4);
    border-radius: 7px;
    background: var(--surface-5);
    color: var(--text);
    font-size: 12px;
    font-family: inherit;
    outline: none;
  }

  .config-textarea:focus { border-color: var(--brand); }

  .switch-field {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 8px;
    border: 1px solid var(--border-3);
    border-radius: 7px;
    background: var(--surface-4);
    color: var(--text);
    cursor: pointer;
  }

  .switch-field.compact {
    min-height: 34px;
    padding: 6px 7px;
  }

  .switch-field.compact strong {
    font-size: 12px;
  }

  .switch-field span {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .switch-field strong {
    font-size: 12px;
    line-height: 1.2;
  }

  .switch-field small {
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.35;
  }

  .switch-field input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }

  .switch-field i {
    position: relative;
    flex: 0 0 auto;
    width: 38px;
    height: 22px;
    border: 1px solid var(--border-4);
    border-radius: 999px;
    background: var(--surface-5);
    transition: background 120ms, border-color 120ms;
  }

  .switch-field i::after {
    content: "";
    position: absolute;
    top: 3px;
    left: 3px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--text-muted);
    transition: transform 120ms, background 120ms;
  }

  .switch-field input:checked + i {
    border-color: var(--brand);
    background: color-mix(in srgb, var(--brand) 18%, var(--surface-5));
  }

  .switch-field input:checked + i::after {
    transform: translateX(16px);
    background: var(--brand);
  }

  .save-config-btn {
    width: 100%;
    min-height: 30px;
    border: 1px solid var(--brand);
    border-radius: 7px;
    background: var(--brand);
    color: #fff;
    font-size: 12px;
    font-weight: 700;
    cursor: pointer;
  }

  .save-config-btn.compact-save {
    width: auto;
    min-height: 28px;
    padding: 0 10px;
    white-space: nowrap;
  }

  .divider {
    height: 1px;
    background: var(--border-2);
    margin: 0 -16px;
  }

  .actions-section {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .action-btn {
    width: 100%;
    min-height: 32px;
    border-radius: 8px;
    font-size: 12px;
    font-weight: 800;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    transition: opacity 80ms;
  }

  .action-btn:disabled { opacity: 0.5; cursor: not-allowed; }
  .action-btn:not(:disabled):hover { opacity: 0.88; }

  .build-btn {
    border: 1px solid color-mix(in srgb, #f59e0b 50%, transparent);
    background: color-mix(in srgb, #f59e0b 15%, transparent);
    color: #b45309;
  }

  .cancel-btn {
    border: 1px solid color-mix(in srgb, var(--danger, #dc2626) 48%, transparent);
    color: var(--danger, #dc2626);
    background: color-mix(in srgb, var(--danger, #dc2626) 9%, var(--surface));
  }

  .log-box {
    border: 1px solid var(--border-3);
    border-radius: 8px;
    overflow: hidden;
  }

  .log-box.log-ok { border-color: color-mix(in srgb, #10b981 40%, transparent); }
  .log-box.log-err { border-color: color-mix(in srgb, #ef4444 40%, transparent); }

  .log-text {
    margin: 0;
    padding: 8px 10px;
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
    line-height: 1.6;
    color: var(--text-muted);
    white-space: pre-wrap;
    word-break: break-all;
    max-height: 220px;
    overflow-y: auto;
    background: var(--surface-3);
  }

  .log-box.log-ok .log-text { color: #065f46; background: color-mix(in srgb, #10b981 8%, transparent); }
  .log-box.log-err .log-text { color: #991b1b; background: color-mix(in srgb, #ef4444 8%, transparent); }
</style>
