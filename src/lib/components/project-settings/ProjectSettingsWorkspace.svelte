<script lang="ts">
  import { IconDeviceFloppy, IconSettings } from "@tabler/icons-svelte";
  import { onDestroy } from "svelte";
  import EmptyState from "$lib/components/ui/EmptyState.svelte";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import SwitchControl from "$lib/components/ui/SwitchControl.svelte";
  import type { PublishWorkspaceState } from "$lib/deploy/publish-state.svelte";
  import { t } from "$lib/i18n/runtime.svelte";
  import {
    createDefaultZolaSettings,
    projectSettingsDraftFromSnapshot,
    projectSettingsFromDraft,
    textFieldsFromZolaSettings,
    zolaSettingsWithTextFields,
  } from "$lib/project/deploy-settings";
  import { readProjectConfiguration, saveProjectConfiguration } from "$lib/project/io/configuration";
  import type { ProjectSettingsSnapshot, ZolaProjectSettings } from "$lib/project/lifecycle-contract";
  import { registerEditFlushHandler } from "$lib/session/edit-flush-registry";
  import type { GlobalStatusState } from "$lib/status/state.svelte";
  import { errorMessage } from "$lib/util";

  let {
    scannedProject,
    projectRoot,
    workspaceRevision,
    publishWorkspace,
    globalStatus,
  }: {
    scannedProject: boolean;
    projectRoot: string;
    workspaceRevision: number;
    publishWorkspace: PublishWorkspaceState;
    globalStatus: GlobalStatusState;
  } = $props();

  type BooleanSettingKey =
    | "compileSass" | "minifyHtml" | "generateSitemap" | "generateRobotsTxt"
    | "excludePaginatedPagesInSitemap" | "generateFeeds" | "renderEmoji"
    | "smartPunctuation" | "lazyAsyncImage" | "githubAlerts" | "bottomFootnotes"
    | "externalLinksTargetBlank" | "externalLinksNoFollow" | "externalLinksNoReferrer"
    | "buildSearchIndex" | "searchIncludeTitle" | "searchIncludeDescription"
    | "searchIncludeDate" | "searchIncludePath" | "searchIncludeContent";

  type ToggleField = Readonly<{
    key: BooleanSettingKey;
    label: string;
    help?: string;
  }>;

  let zolaSettings = $state<ZolaProjectSettings>(createDefaultZolaSettings());
  let cachebustAssetsDraft = $state(false);
  let projectSettingsRevision = $state(0);
  let feedFilenamesText = $state("");
  let feedLimitText = $state("");
  let searchTruncateText = $state("");
  let loading = $state(false);
  let loaded = $state(false);
  let dirty = $state(false);
  let saving = $state(false);
  let loadedAuthorityKey = $state("");
  let savePromise: Promise<void> | null = null;

  const insertAnchorOptions = ["none", "left", "right", "heading"];
  const searchIndexFormatOptions = [
    "elasticlunr_javascript", "elasticlunr_json", "fuse_javascript", "fuse_json",
  ];
  const buildFields = $derived<ToggleField[]>([
    { key: "compileSass", label: t("deploy-compile-sass"), help: t("deploy-compile-sass-help") },
    { key: "minifyHtml", label: t("deploy-minify-html"), help: t("deploy-minify-html-help") },
  ]);
  const seoFields = $derived<ToggleField[]>([
    { key: "generateSitemap", label: t("deploy-sitemap"), help: t("deploy-sitemap-help") },
    { key: "generateRobotsTxt", label: "robots.txt", help: t("deploy-robots-help") },
    { key: "excludePaginatedPagesInSitemap", label: t("deploy-exclude-paginated"), help: t("deploy-exclude-paginated-help") },
    { key: "generateFeeds", label: t("deploy-feeds"), help: t("deploy-feeds-help") },
  ]);
  const markdownFields = $derived<ToggleField[]>([
    { key: "renderEmoji", label: "Emoji" },
    { key: "smartPunctuation", label: t("deploy-smart-punctuation") },
    { key: "lazyAsyncImage", label: t("deploy-lazy-images") },
    { key: "githubAlerts", label: "GitHub alerts" },
    { key: "bottomFootnotes", label: t("deploy-bottom-footnotes") },
  ]);
  const externalLinkFields = $derived<ToggleField[]>([
    { key: "externalLinksTargetBlank", label: "target blank" },
    { key: "externalLinksNoFollow", label: "nofollow" },
    { key: "externalLinksNoReferrer", label: "noreferrer" },
  ]);
  const searchFields = $derived<ToggleField[]>([
    { key: "searchIncludeTitle", label: t("deploy-search-title") },
    { key: "searchIncludeDescription", label: t("deploy-search-description") },
    { key: "searchIncludeDate", label: t("deploy-search-date") },
    { key: "searchIncludePath", label: t("deploy-search-path") },
    { key: "searchIncludeContent", label: t("deploy-search-content") },
  ]);

  $effect(() => {
    const authorityKey = `${projectRoot}\u0000${workspaceRevision}`;
    if (!scannedProject || !projectRoot || dirty || loading || authorityKey === loadedAuthorityKey) return;
    loadedAuthorityKey = authorityKey;
    void loadConfiguration();
  });

  const unregisterFlush = registerEditFlushHandler(
    "project-settings-workspace",
    async () => { await persistConfiguration(); },
    () => dirty,
  );
  onDestroy(unregisterFlush);

  async function loadConfiguration() {
    loading = true;
    loaded = false;
    try {
      const config = await readProjectConfiguration();
      zolaSettings = config.zolaSettings;
      syncProjectSettings(config.projectSettings);
      syncTextFields(config.zolaSettings);
      publishWorkspace.cachebustAssets = config.projectSettings.cachebustAssets;
      dirty = false;
      publishWorkspace.invalidate();
    } catch (error) {
      globalStatus.set(t("deploy-config-load-error", { error: errorMessage(error) }), "error");
    } finally {
      loaded = true;
      loading = false;
    }
  }

  function persistConfiguration(): Promise<void> {
    if (savePromise) return savePromise;
    if (!dirty) return Promise.resolve();
    const operation = persistConfigurationOperation();
    savePromise = operation;
    void operation.finally(() => {
      if (savePromise === operation) savePromise = null;
    }).catch(() => {});
    return operation;
  }

  async function persistConfigurationOperation() {
    saving = true;
    globalStatus.set(t("deploy-config-saving"), "saving");
    try {
      const config = await saveProjectConfiguration({
        projectSettings: projectSettingsFromDraft(
          { cachebustAssetsDraft },
          projectSettingsRevision,
        ),
        zolaSettings: zolaSettingsWithTextFields(zolaSettings, {
          feedFilenamesText,
          feedLimitText,
          searchTruncateText,
        }),
      });
      zolaSettings = config.zolaSettings;
      syncProjectSettings(config.projectSettings);
      syncTextFields(config.zolaSettings);
      publishWorkspace.cachebustAssets = config.projectSettings.cachebustAssets;
      publishWorkspace.invalidate();
      dirty = false;
      globalStatus.set(t("project-settings-staged"), "unsaved");
    } catch (error) {
      globalStatus.set(t("deploy-config-error", { error: errorMessage(error) }), "error");
      throw error;
    } finally {
      saving = false;
    }
  }

  async function saveFromButton() {
    try {
      await persistConfiguration();
    } catch {
      // persistConfiguration already published the actionable diagnostic.
    }
  }

  function syncTextFields(settings: ZolaProjectSettings) {
    const fields = textFieldsFromZolaSettings(settings);
    feedFilenamesText = fields.feedFilenamesText;
    feedLimitText = fields.feedLimitText;
    searchTruncateText = fields.searchTruncateText;
  }

  function syncProjectSettings(settings: ProjectSettingsSnapshot) {
    cachebustAssetsDraft = projectSettingsDraftFromSnapshot(settings).cachebustAssetsDraft;
    projectSettingsRevision = settings.workspaceRevision;
  }

  function markDirty() {
    if (!loaded || loading) return;
    dirty = true;
    globalStatus.set(t("deploy-config-dirty-status"), "unsaved");
  }

  function setSetting<K extends keyof ZolaProjectSettings>(key: K, value: ZolaProjectSettings[K]) {
    zolaSettings = { ...zolaSettings, [key]: value };
    markDirty();
  }

  function setBoolean(key: BooleanSettingKey, value: boolean) {
    setSetting(key, value);
  }
</script>

<section
  class="activity-workspace activity-workspace-header-content project-settings-workspace"
  aria-labelledby="project-settings-title"
>
  <header class="workspace-header">
    <div>
      <span class="eyebrow"><IconSettings size={15} stroke={1.9} /> {t("project-settings-eyebrow")}</span>
      <h1 id="project-settings-title">{t("project-settings-title")}</h1>
      <p>{t("project-settings-description")}</p>
    </div>
    <button
      class="ui-button primary"
      type="button"
      disabled={!dirty || saving}
      onclick={() => { void saveFromButton(); }}
    >
      <IconDeviceFloppy size={15} />
      {saving ? t("deploy-config-saving") : t("deploy-save-config")}
    </button>
  </header>

  <div class="configuration-scroll">
    {#if !scannedProject}
      <EmptyState title={t("deploy-open-folder")} compact />
    {:else if loading}
      <EmptyState title={t("deploy-config-loading")} compact />
    {:else}
      <fieldset class="configuration-grid" disabled={saving}>
        <section class="ui-form-section">
          <div class="ui-form-section-heading">
            <h2>{t("deploy-section-project")}</h2>
            <code>{zolaSettings.configPath}</code>
          </div>
          <label class="ui-form-field">
            <span class="ui-form-label">base_url</span>
            <input class="ui-input" type="url" placeholder={t("deploy-placeholder-url")} value={zolaSettings.baseUrl} oninput={(event) => setSetting("baseUrl", event.currentTarget.value)} />
          </label>
          <label class="ui-form-field">
            <span class="ui-form-label">title</span>
            <input class="ui-input" placeholder={t("deploy-placeholder-title")} value={zolaSettings.title} oninput={(event) => setSetting("title", event.currentTarget.value)} />
          </label>
          <label class="ui-form-field">
            <span class="ui-form-label">description</span>
            <textarea class="ui-textarea" rows="3" placeholder={t("deploy-placeholder-description")} value={zolaSettings.description} oninput={(event) => setSetting("description", event.currentTarget.value)}></textarea>
          </label>
          <div class="ui-form-grid columns-2">
            <label class="ui-form-field">
              <span class="ui-form-label">default_language</span>
              <input class="ui-input" placeholder="ro" value={zolaSettings.defaultLanguage} oninput={(event) => setSetting("defaultLanguage", event.currentTarget.value)} />
            </label>
            <label class="ui-form-field">
              <span class="ui-form-label">author</span>
              <input class="ui-input" placeholder={t("deploy-placeholder-author")} value={zolaSettings.author} oninput={(event) => setSetting("author", event.currentTarget.value)} />
            </label>
          </div>
        </section>

        <section class="ui-form-section">
          <h2>{t("deploy-section-build")}</h2>
          {#each buildFields as field (field.key)}
            <SwitchControl label={field.label} description={field.help} checked={zolaSettings[field.key]} disabled={saving} onchange={(checked) => setBoolean(field.key, checked)} />
          {/each}
          <label class="ui-form-field">
            <span class="ui-form-label">output_dir</span>
            <input class="ui-input" placeholder="public" value={zolaSettings.outputDir} oninput={(event) => setSetting("outputDir", event.currentTarget.value)} />
          </label>
          <SwitchControl
            label={t("deploy-cachebust")}
            description={t("deploy-cachebust-help")}
            checked={cachebustAssetsDraft}
            disabled={saving}
            onchange={(checked) => { cachebustAssetsDraft = checked; markDirty(); }}
          />
        </section>

        <section class="ui-form-section">
          <h2>{t("deploy-section-seo")}</h2>
          {#each seoFields as field (field.key)}
            <SwitchControl label={field.label} description={field.help} checked={zolaSettings[field.key]} disabled={saving} onchange={(checked) => setBoolean(field.key, checked)} />
          {/each}
          <div class="ui-form-grid columns-2">
            <label class="ui-form-field">
              <span class="ui-form-label">feed_filenames</span>
              <input class="ui-input" placeholder="atom.xml, rss.xml" value={feedFilenamesText} oninput={(event) => { feedFilenamesText = event.currentTarget.value; markDirty(); }} />
            </label>
            <label class="ui-form-field">
              <span class="ui-form-label">feed_limit</span>
              <input class="ui-input" type="number" min="0" placeholder={t("deploy-unlimited-placeholder")} value={feedLimitText} oninput={(event) => { feedLimitText = event.currentTarget.value; markDirty(); }} />
            </label>
          </div>
        </section>

        <section class="ui-form-section">
          <h2>{t("deploy-section-markdown")}</h2>
          <div class="ui-form-grid columns-2">
            {#each markdownFields as field (field.key)}
              <SwitchControl compact label={field.label} checked={zolaSettings[field.key]} disabled={saving} onchange={(checked) => setBoolean(field.key, checked)} />
            {/each}
          </div>
          <label class="ui-form-field">
            <span class="ui-form-label">insert_anchor_links</span>
            <SelectControl size="default" value={zolaSettings.insertAnchorLinks} options={insertAnchorOptions} disabled={saving} ariaLabel={t("deploy-anchor-links-label")} onchange={(value) => setSetting("insertAnchorLinks", value)} />
          </label>
          <h3 class="ui-form-subheading">{t("deploy-external-links")}</h3>
          <div class="ui-form-grid columns-2">
            {#each externalLinkFields as field (field.key)}
              <SwitchControl compact label={field.label} checked={zolaSettings[field.key]} disabled={saving} onchange={(checked) => setBoolean(field.key, checked)} />
            {/each}
          </div>
        </section>

        <section class="ui-form-section wide">
          <h2>{t("deploy-section-search")}</h2>
          <SwitchControl label={t("deploy-search-index")} description={t("deploy-search-index-help")} checked={zolaSettings.buildSearchIndex} disabled={saving} onchange={(checked) => setBoolean("buildSearchIndex", checked)} />
          <label class="ui-form-field">
            <span class="ui-form-label">index_format</span>
            <SelectControl size="default" value={zolaSettings.searchIndexFormat} options={searchIndexFormatOptions} disabled={saving} ariaLabel={t("deploy-search-format-label")} onchange={(value) => setSetting("searchIndexFormat", value)} />
          </label>
          <div class="ui-form-grid columns-3 search-fields">
            {#each searchFields as field (field.key)}
              <SwitchControl compact label={field.label} checked={zolaSettings[field.key]} disabled={saving} onchange={(checked) => setBoolean(field.key, checked)} />
            {/each}
          </div>
          <label class="ui-form-field">
            <span class="ui-form-label">truncate_content_length</span>
            <input class="ui-input" type="number" min="0" placeholder={t("deploy-complete-placeholder")} value={searchTruncateText} oninput={(event) => { searchTruncateText = event.currentTarget.value; markDirty(); }} />
          </label>
        </section>
      </fieldset>
    {/if}
  </div>
</section>

<style>
  .workspace-header > button { align-self: center; white-space: nowrap; }
  .configuration-scroll { min-height: 0; padding: 12px; overflow: auto; }
  .configuration-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; min-width: 0; max-width: 1180px; margin: 0 auto; padding: 0; border: 0; }
  .configuration-grid > :global(.ui-form-section.wide) { grid-column: 1 / -1; }
  .configuration-grid :global(.ui-form-section-heading code) { overflow: hidden; color: var(--text-muted); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .configuration-scroll > :global(.ui-empty-state) { margin: 0; min-height: 120px; }
  @media (max-width: 920px) { .configuration-grid { grid-template-columns: 1fr; } .configuration-grid > :global(.ui-form-section.wide) { grid-column: auto; } .configuration-grid :global(.ui-form-grid.columns-3) { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
</style>
