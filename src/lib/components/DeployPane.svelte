<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import {
    IconBuildingFactory2,
    IconHammer,
    IconRocket,
    IconEye,
    IconEyeOff,
    IconX,
  } from "@tabler/icons-svelte";
  import {
    readProjectAppConfig,
    readProjectEnv,
    readZolaProjectSettings,
    saveProjectAppConfig,
    saveProjectEnv,
    saveZolaProjectSettings,
    zolaInit,
    zolaBuild,
    deployToBunny,
    cancelPublishOperation,
  } from "$lib/project/io";
  import {
    BUNNY_ENV_KEYS,
    appConfigDraftFromConfig,
    appConfigFromDraft,
    bunnyEnvVarsFromDraft,
    createDefaultZolaSettings,
    textFieldsFromZolaSettings,
    zolaSettingsWithTextFields,
    type ProjectAppConfig,
  } from "$lib/project/deploy-settings";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import type { ZolaProjectSettings } from "$lib/types";
  import { errorMessage } from "$lib/util";

  let {
    scannedProject = false,
    isZola = false,
    isEmpty = false,
    cachebustAssets = false,
    workspaceMode = false,
    actionsOnly = false,
    projectRoot = "",
    runtimeSessionId = "",
    onStatusUpdate = undefined as ((text: string, kind: string) => void) | undefined,
    onCachebustAssetsChange = undefined as ((value: boolean) => void) | undefined,
  }: {
    scannedProject?: boolean;
    isZola?: boolean;
    isEmpty?: boolean;
    cachebustAssets?: boolean;
    workspaceMode?: boolean;
    actionsOnly?: boolean;
    projectRoot?: string;
    runtimeSessionId?: string;
    onStatusUpdate?: (text: string, kind: string) => void;
    onCachebustAssetsChange?: (value: boolean) => void;
  } = $props();

  // Init is only safe and useful for empty folders
  const canInit = $derived(scannedProject && isEmpty && !isZola);

  let zolaSettings = $state<ZolaProjectSettings>(createDefaultZolaSettings());
  let envVars = $state<Record<string, string>>({});
  let cachebustAssetsDraft = $state(false);
  let optimizeImagesDraft = $state(false);
  let imageMaxDimensionText = $state("1920");
  let imageExcludeSuffix = $state("-nr");
  let imageReplaceOnlyIfSmaller = $state(true);
  let feedFilenamesText = $state("");
  let feedLimitText = $state("");
  let searchTruncateText = $state("");
  let loading = $state(false);
  let configLoaded = $state(false);
  let configDirty = $state(false);
  let showSecrets = $state<Record<string, boolean>>({});

  let initRunning = $state(false);
  let buildRunning = $state(false);
  let deployRunning = $state(false);
  let cancelRunning = $state(false);
  const insertAnchorOptions = ["none", "left", "right", "heading"];
  const searchIndexFormatOptions = ["elasticlunr_javascript", "elasticlunr_json", "fuse_javascript", "fuse_json"];
  let actionLog = $state("");
  let actionOk = $state<boolean | null>(null);

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
      const [settings, env, appConfig] = await Promise.all([
        readZolaProjectSettings(),
        readProjectEnv(),
        readProjectAppConfig(),
      ]);
      zolaSettings = settings;
      envVars = env;
      syncAppConfigFields(appConfig);
      syncTextFields(settings);
      onCachebustAssetsChange?.(appConfig.cachebustAssets);
      configDirty = false;
    } catch (e) {
      onStatusUpdate?.(`Eroare la încărcarea configurației: ${errorMessage(e)}`, "error");
    }
    configLoaded = true;
    loading = false;
  }

  async function saveConfig() {
    onStatusUpdate?.("Se salvează configurația...", "saving");
    try {
      const settingsToSave = zolaSettingsWithTextFields(zolaSettings, {
        feedFilenamesText,
        feedLimitText,
        searchTruncateText,
      });
      const [savedSettings, , appConfig] = await Promise.all([
        saveZolaProjectSettings(settingsToSave),
        saveProjectEnv(bunnyEnvVarsFromDraft(envVars)),
        saveProjectAppConfig(appConfigFromDraft({
          cachebustAssetsDraft,
          optimizeImagesDraft,
          imageMaxDimensionText,
          imageExcludeSuffix,
          imageReplaceOnlyIfSmaller,
        })),
      ]);
      zolaSettings = savedSettings;
      syncAppConfigFields(appConfig);
      syncTextFields(savedSettings);
      configDirty = false;
      onCachebustAssetsChange?.(appConfig.cachebustAssets);
      onStatusUpdate?.("Setările proiectului au fost salvate.", "saved");
    } catch (e) {
      onStatusUpdate?.(`Eroare config: ${errorMessage(e)}`, "error");
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
    optimizeImagesDraft = draft.optimizeImagesDraft;
    imageMaxDimensionText = draft.imageMaxDimensionText;
    imageExcludeSuffix = draft.imageExcludeSuffix;
    imageReplaceOnlyIfSmaller = draft.imageReplaceOnlyIfSmaller;
  }

  function markConfigDirty() {
    if (!configLoaded || loading) return;
    if (!configDirty) {
      onStatusUpdate?.("Setări modificate — folosește Salvează configurația.", "unsaved");
    }
    configDirty = true;
  }

  function setSetting<K extends keyof ZolaProjectSettings>(key: K, value: ZolaProjectSettings[K]) {
    zolaSettings = { ...zolaSettings, [key]: value };
    markConfigDirty();
  }

  function setVar(key: string, value: string) {
    envVars = { ...envVars, [key]: value };
    markConfigDirty();
  }

  function toggleSecret(key: string) {
    showSecrets = { ...showSecrets, [key]: !showSecrets[key] };
  }

  async function runInit() {
    const selected = await open({ directory: true, multiple: false, title: "Alege folderul pentru proiectul nou" });
    if (!selected || Array.isArray(selected)) return;
    initRunning = true;
    actionLog = "";
    actionOk = null;
    try {
      actionLog = await zolaInit(selected);
      actionOk = true;
    } catch (e) {
      actionLog = errorMessage(e);
      actionOk = false;
    }
    initRunning = false;
  }

  async function runBuild() {
    buildRunning = true;
    actionLog = "";
    actionOk = null;
    onStatusUpdate?.("Se construiește proiectul cu Zola...", "saving");
    try {
      actionLog = await zolaBuild();
      actionOk = true;
      onStatusUpdate?.("Construirea Zola s-a încheiat.", "saved");
    } catch (e) {
      actionLog = errorMessage(e);
      actionOk = false;
      onStatusUpdate?.(`Eroare la construire: ${actionLog}`, "error");
    }
    buildRunning = false;
  }

  async function runDeploy() {
    deployRunning = true;
    actionLog = "";
    actionOk = null;
    onStatusUpdate?.("Se publică proiectul...", "saving");
    try {
      actionLog = await deployToBunny();
      actionOk = true;
      onStatusUpdate?.("Publicarea s-a încheiat.", "saved");
    } catch (e) {
      actionLog = errorMessage(e);
      actionOk = false;
      onStatusUpdate?.(`Eroare la publicare: ${actionLog}`, "error");
    }
    deployRunning = false;
  }

  async function cancelRunningOperation() {
    if (cancelRunning || (!buildRunning && !deployRunning)) return;
    if (!projectRoot || !runtimeSessionId) {
      onStatusUpdate?.("Operația nu poate fi anulată fără identitatea sesiunii proiectului.", "error");
      return;
    }
    cancelRunning = true;
    try {
      const receipt = await cancelPublishOperation({
        expectedProjectRoot: projectRoot,
        expectedSessionId: runtimeSessionId,
      });
      actionLog = `Anulare solicitată pentru ${receipt.kind} (${receipt.operationId}).`;
      actionOk = null;
      onStatusUpdate?.("Anularea operației de publicare a fost solicitată.", "saving");
    } catch (error) {
      onStatusUpdate?.(`Anularea publicării a eșuat: ${errorMessage(error)}`, "error");
    } finally {
      cancelRunning = false;
    }
  }
</script>

<div class:workspace-mode={workspaceMode} class:actions-only={actionsOnly} class="deploy-pane">

  {#if canInit}
    <section class="actions-section">
      <button type="button" class="action-btn init-btn" onclick={runInit} disabled={initRunning}>
        <IconBuildingFactory2 size={14} stroke={1.8} />
        {initRunning ? "Se inițializează..." : "Init proiect nou"}
      </button>
    </section>
  {:else if !scannedProject}
    <p class="hint">Deschide un dosar pentru configurarea proiectului.</p>
  {:else if !isZola && !isEmpty}
    <p class="hint">Setările și publicarea Zola sunt disponibile doar pentru proiecte Zola.</p>
  {:else if loading}
    <p class="hint">Se încarcă configurația...</p>
  {:else}
    <div class="sticky-config-actions">
      <span class:dirty={configDirty}>{configDirty ? "Modificări nesalvate" : "Configurație sincronizată"}</span>
      <div class="sticky-action-buttons">
        <button type="button" class="save-config-btn compact-save" onclick={saveConfig}>
          Salvează configurația
        </button>
        {#if workspaceMode}
          <button type="button" class="action-btn build-btn compact-action" onclick={runBuild} disabled={buildRunning || configDirty} title={configDirty ? "Salvează configurația înainte de construire" : "Construiește proiectul cu Zola"}>
            <IconHammer size={14} stroke={1.8} />
            {buildRunning ? "Se construiește…" : "Construiește"}
          </button>
          <button type="button" class="action-btn deploy-btn compact-action" onclick={runDeploy} disabled={deployRunning || configDirty} title={configDirty ? "Salvează configurația înainte de publicare" : "Publică pe Bunny CDN"}>
            <IconRocket size={14} stroke={1.8} />
            {deployRunning ? "Publicare…" : "Publică"}
          </button>
          {#if buildRunning || deployRunning}
            <button type="button" class="action-btn cancel-btn compact-action" onclick={cancelRunningOperation} disabled={cancelRunning}>
              <IconX size={14} stroke={2} /> {cancelRunning ? "Se anulează…" : "Anulează"}
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

    {#if !actionsOnly}
    <section class="config-section">
      <div class="section-title-row">
        <h3 class="section-label">PROIECT</h3>
        <code>{zolaSettings.configPath}</code>
      </div>
      <label class="config-field">
        <span>base_url</span>
        <input type="url" class="config-input" placeholder="https://exemplu.ro" value={zolaSettings.baseUrl}
          oninput={(event) => setSetting("baseUrl", event.currentTarget.value)} />
      </label>
      <label class="config-field">
        <span>title</span>
        <input class="config-input" placeholder="Numele site-ului" value={zolaSettings.title}
          oninput={(event) => setSetting("title", event.currentTarget.value)} />
      </label>
      <label class="config-field">
        <span>description</span>
        <textarea class="config-textarea" rows="3" placeholder="Descriere scurtă pentru site"
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
          <input class="config-input" placeholder="Autor" value={zolaSettings.author}
            oninput={(event) => setSetting("author", event.currentTarget.value)} />
        </label>
      </div>
    </section>

    <section class="config-section">
      <h3 class="section-label">BUILD</h3>
      <label class="switch-field">
        <span><strong>Compilează Sass</strong><small>Activează procesarea folderului <code>sass/</code>.</small></span>
        <input type="checkbox" role="switch" checked={zolaSettings.compileSass}
          onchange={(event) => setSetting("compileSass", event.currentTarget.checked)} />
        <i aria-hidden="true"></i>
      </label>
      <label class="switch-field">
        <span><strong>Minifică HTML</strong><small>Reduce output-ul final generat de Zola.</small></span>
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
          <strong>Cache bust asset-uri generate</strong>
          <small>Normalizează link-urile CSS/JS locale între URL simplu și <code>get_url(..., cachebust=true)</code>.</small>
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
      <label class="switch-field">
        <span>
          <strong>Optimizează imaginile după build</strong>
          <small>Procesează doar rezultatul Zola din <code>{zolaSettings.outputDir || "public"}</code>; sursele rămân neatinse.</small>
        </span>
        <input
          type="checkbox"
          role="switch"
          checked={optimizeImagesDraft}
          onchange={(event) => {
            optimizeImagesDraft = event.currentTarget.checked;
            markConfigDirty();
          }}
        />
        <i aria-hidden="true"></i>
      </label>
      {#if optimizeImagesDraft}
        <div class="image-settings">
          <div class="field-grid">
            <label class="config-field">
              <span>Latură maximă</span>
              <input class="config-input" type="number" min="1" value={imageMaxDimensionText}
                oninput={(event) => {
                  imageMaxDimensionText = event.currentTarget.value;
                  markConfigDirty();
                }} />
            </label>
            <label class="config-field">
              <span>Exclude sufix</span>
              <input class="config-input" placeholder="-nr" value={imageExcludeSuffix}
                oninput={(event) => {
                  imageExcludeSuffix = event.currentTarget.value;
                  markConfigDirty();
                }} />
            </label>
          </div>
          <label class="switch-field compact">
            <span>
              <strong>Doar dacă WebP e mai mic</strong>
              <small>Formatul curent este WebP lossless, cu metadata eliminată.</small>
            </span>
            <input
              type="checkbox"
              role="switch"
              checked={imageReplaceOnlyIfSmaller}
              onchange={(event) => {
                imageReplaceOnlyIfSmaller = event.currentTarget.checked;
                markConfigDirty();
              }}
            />
            <i aria-hidden="true"></i>
          </label>
        </div>
      {/if}
    </section>

    <section class="config-section">
      <h3 class="section-label">SEO & INDEXARE</h3>
      <label class="switch-field">
        <span><strong>Sitemap XML</strong><small>Generează sitemap.xml.</small></span>
        <input type="checkbox" role="switch" checked={zolaSettings.generateSitemap}
          onchange={(event) => setSetting("generateSitemap", event.currentTarget.checked)} />
        <i aria-hidden="true"></i>
      </label>
      <label class="switch-field">
        <span><strong>robots.txt</strong><small>Generează robots.txt.</small></span>
        <input type="checkbox" role="switch" checked={zolaSettings.generateRobotsTxt}
          onchange={(event) => setSetting("generateRobotsTxt", event.currentTarget.checked)} />
        <i aria-hidden="true"></i>
      </label>
      <label class="switch-field">
        <span><strong>Exclude pagini paginate</strong><small>Nu include paginile paginate în sitemap.</small></span>
        <input type="checkbox" role="switch" checked={zolaSettings.excludePaginatedPagesInSitemap}
          onchange={(event) => setSetting("excludePaginatedPagesInSitemap", event.currentTarget.checked)} />
        <i aria-hidden="true"></i>
      </label>
      <label class="switch-field">
        <span><strong>Feeds</strong><small>Generează feed-uri RSS/Atom.</small></span>
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
          <input class="config-input" type="number" min="0" placeholder="gol = nelimitat" value={feedLimitText}
            oninput={(event) => {
              feedLimitText = event.currentTarget.value;
              markConfigDirty();
            }} />
        </label>
      </div>
    </section>

    <section class="config-section">
      <h3 class="section-label">MARKDOWN</h3>
      <div class="field-grid">
        <label class="switch-field compact">
          <span><strong>Emoji</strong></span>
          <input type="checkbox" role="switch" checked={zolaSettings.renderEmoji}
            onchange={(event) => setSetting("renderEmoji", event.currentTarget.checked)} />
          <i aria-hidden="true"></i>
        </label>
        <label class="switch-field compact">
          <span><strong>Punctuație smart</strong></span>
          <input type="checkbox" role="switch" checked={zolaSettings.smartPunctuation}
            onchange={(event) => setSetting("smartPunctuation", event.currentTarget.checked)} />
          <i aria-hidden="true"></i>
        </label>
        <label class="switch-field compact">
          <span><strong>Imagini lazy async</strong></span>
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
          <span><strong>Footnotes jos</strong></span>
          <input type="checkbox" role="switch" checked={zolaSettings.bottomFootnotes}
            onchange={(event) => setSetting("bottomFootnotes", event.currentTarget.checked)} />
          <i aria-hidden="true"></i>
        </label>
      </div>
      <label class="config-field">
        <span>insert_anchor_links</span>
        <SelectControl value={zolaSettings.insertAnchorLinks} options={insertAnchorOptions} ariaLabel="Inserare linkuri ancoră" onchange={(value) => setSetting("insertAnchorLinks", value)} />
      </label>
      <h4 class="subsection-label">Link-uri externe</h4>
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
      <h3 class="section-label">SEARCH</h3>
      <label class="switch-field">
        <span><strong>Construiește indexul de căutare</strong><small>Generează indexul de căutare Zola.</small></span>
        <input type="checkbox" role="switch" checked={zolaSettings.buildSearchIndex}
          onchange={(event) => setSetting("buildSearchIndex", event.currentTarget.checked)} />
        <i aria-hidden="true"></i>
      </label>
      <label class="config-field">
        <span>index_format</span>
        <SelectControl value={zolaSettings.searchIndexFormat} options={searchIndexFormatOptions} ariaLabel="Formatul indexului de căutare" onchange={(value) => setSetting("searchIndexFormat", value)} />
      </label>
      <div class="field-grid">
        <label class="switch-field compact">
          <span><strong>Titlu</strong></span>
          <input type="checkbox" role="switch" checked={zolaSettings.searchIncludeTitle}
            onchange={(event) => setSetting("searchIncludeTitle", event.currentTarget.checked)} />
          <i aria-hidden="true"></i>
        </label>
        <label class="switch-field compact">
          <span><strong>Descriere</strong></span>
          <input type="checkbox" role="switch" checked={zolaSettings.searchIncludeDescription}
            onchange={(event) => setSetting("searchIncludeDescription", event.currentTarget.checked)} />
          <i aria-hidden="true"></i>
        </label>
        <label class="switch-field compact">
          <span><strong>Dată</strong></span>
          <input type="checkbox" role="switch" checked={zolaSettings.searchIncludeDate}
            onchange={(event) => setSetting("searchIncludeDate", event.currentTarget.checked)} />
          <i aria-hidden="true"></i>
        </label>
        <label class="switch-field compact">
          <span><strong>Path</strong></span>
          <input type="checkbox" role="switch" checked={zolaSettings.searchIncludePath}
            onchange={(event) => setSetting("searchIncludePath", event.currentTarget.checked)} />
          <i aria-hidden="true"></i>
        </label>
        <label class="switch-field compact">
          <span><strong>Conținut</strong></span>
          <input type="checkbox" role="switch" checked={zolaSettings.searchIncludeContent}
            onchange={(event) => setSetting("searchIncludeContent", event.currentTarget.checked)} />
          <i aria-hidden="true"></i>
        </label>
      </div>
      <label class="config-field">
        <span>truncate_content_length</span>
        <input class="config-input" type="number" min="0" placeholder="gol = complet" value={searchTruncateText}
          oninput={(event) => {
            searchTruncateText = event.currentTarget.value;
            markConfigDirty();
          }} />
      </label>
    </section>

    <section class="config-section">
      <h3 class="section-label">BUNNY CDN</h3>
      {#each BUNNY_ENV_KEYS as { key, label, secret }}
        <label class="config-field">
          <span>{label}</span>
          <div class="secret-row">
            <input
              class="config-input"
              type={secret && !showSecrets[key] ? "password" : "text"}
              value={envVars[key] ?? ""}
              oninput={(e) => setVar(key, e.currentTarget.value)}
              placeholder={key}
              autocomplete="off"
            />
            {#if secret}
              <button type="button" class="toggle-secret" onclick={() => toggleSecret(key)}
                title={showSecrets[key] ? "Ascunde" : "Arata"}>
                {#if showSecrets[key]}
                  <IconEyeOff size={13} stroke={1.8} />
                {:else}
                  <IconEye size={13} stroke={1.8} />
                {/if}
              </button>
            {/if}
          </div>
        </label>
      {/each}
      <p class="env-note">Salvat in <code>.env</code> — nu se commitează in git.</p>
    </section>

    <button type="button" class="save-config-btn" onclick={saveConfig}>
      Salvează configurația
    </button>

    <div class="divider"></div>

    {#if !workspaceMode}<section class="actions-section">
      <button type="button" class="action-btn build-btn" onclick={runBuild} disabled={buildRunning}>
        <IconHammer size={14} stroke={1.8} />
        {buildRunning ? "Se construiește..." : "Construire Zola"}
      </button>
      <button type="button" class="action-btn deploy-btn" onclick={runDeploy} disabled={deployRunning}>
        <IconRocket size={14} stroke={1.8} />
        {deployRunning ? "Se publică..." : "Publică"}
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

  .image-settings {
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding: 8px;
    border: 1px dashed var(--border-3);
    border-radius: 7px;
    background: color-mix(in srgb, var(--surface-5) 55%, transparent);
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

  .switch-field code {
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
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

  .secret-row { display: flex; gap: 4px; align-items: center; }
  .secret-row .config-input { flex: 1; }

  .toggle-secret {
    width: 26px; height: 28px; flex-shrink: 0;
    border: 1px solid var(--border-4); border-radius: 6px;
    background: var(--surface-4); color: var(--text-muted);
    cursor: pointer;
    display: flex; align-items: center; justify-content: center;
  }

  .toggle-secret:hover { border-color: var(--brand); color: var(--brand); }

  .env-note {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
    opacity: 0.65;
  }

  .env-note code { font-family: "JetBrains Mono", monospace; font-size: 12px; }

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

  .init-btn {
    border: 1px solid var(--border-3);
    background: var(--surface-4);
    color: var(--text);
  }

  .build-btn {
    border: 1px solid color-mix(in srgb, #f59e0b 50%, transparent);
    background: color-mix(in srgb, #f59e0b 15%, transparent);
    color: #b45309;
  }

  .deploy-btn {
    border: 1px solid var(--brand);
    background: var(--brand);
    color: #fff;
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
