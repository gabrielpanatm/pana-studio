<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import {
    IconActivity,
    IconClipboard,
    IconCpu,
    IconDatabase,
    IconDeviceDesktop,
    IconFolder,
    IconInfoCircle,
    IconLanguage,
    IconLayout,
    IconMoonStars,
    IconPalette,
    IconRefresh,
    IconSettings,
    IconSun,
  } from "@tabler/icons-svelte";
  import { onMount } from "svelte";
  import EmptyState from "$lib/components/ui/EmptyState.svelte";
  import InlineMessage from "$lib/components/ui/InlineMessage.svelte";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import ObservabilityLogControl from "$lib/components/kernel/ObservabilityLogControl.svelte";
  import WriteAuthorityRecoveryControl from "$lib/components/kernel/WriteAuthorityRecoveryControl.svelte";
  import AiIntegrationPane from "$lib/components/settings/AiIntegrationPane.svelte";
  import StoragePane from "$lib/components/settings/StoragePane.svelte";
  import { readAppHome } from "$lib/application/io";
  import { localeOptions, l10n, t } from "$lib/i18n/runtime.svelte";
  import type { ApplicationPreferencesState } from "$lib/application/preferences.svelte";
  import type { GlobalStatusState } from "$lib/status/state.svelte";
  import type { GlobalStatusKind } from "$lib/status/global-status";
  import type { WorkspaceLayoutState } from "$lib/ui/workspace-layout.svelte";
  import type { AiContextStatus } from "$lib/ai/contracts";
  import type { AppHomeSnapshot } from "$lib/application/contracts";
  import type { ApplicationSettingsSection } from "$lib/application/shell-state.svelte";

  let {
    aiContextStatus,
    applicationPreferences,
    globalStatus,
    workspaceLayout,
    requestedSection = "general",
  }: {
    aiContextStatus: AiContextStatus | null;
    applicationPreferences: ApplicationPreferencesState;
    globalStatus: GlobalStatusState;
    workspaceLayout: WorkspaceLayoutState;
    requestedSection?: ApplicationSettingsSection;
  } = $props();

  let activeSection = $state<ApplicationSettingsSection>("general");
  let appHome = $state<AppHomeSnapshot | null>(null);
  let appVersion = $state("");
  let informationLoading = $state(false);
  let informationError = $state("");
  let diagnosticsRefreshToken = $state(0);
  const settingsSections: ApplicationSettingsSection[] = ["general", "ai", "system", "storage", "about"];

  $effect(() => {
    if (settingsSections.includes(requestedSection)) activeSection = requestedSection;
  });

  const directoryEntries = $derived.by(() => {
    if (!appHome) return [];
    return [
      { label: t("settings-directory-configuration"), value: appHome.configDir },
      { label: t("settings-directory-data"), value: appHome.dataDir },
      { label: t("settings-directory-cache"), value: appHome.cacheDir },
      { label: t("settings-directory-logs"), value: appHome.appLogsDir },
      { label: t("settings-directory-mcp"), value: appHome.mcpDir },
      { label: t("settings-directory-sessions"), value: appHome.sessionsDir },
      { label: t("settings-directory-kernel"), value: appHome.kernelDir },
      { label: t("settings-directory-write-authority"), value: appHome.writeAuthorityWalDir },
    ];
  });
  const effectiveLanguageName = $derived(l10n.nativeName(applicationPreferences.locale));
  const effectiveThemeName = $derived(
    t(applicationPreferences.theme === "dark" ? "common-dark" : "common-light"),
  );
  const languagePreferenceValue = $derived.by(() => {
    const preference = applicationPreferences.snapshot?.preferences.language;
    return preference?.mode === "fixed" ? preference.value : "system";
  });
  const themePreference = $derived(
    applicationPreferences.snapshot?.preferences.theme ?? { mode: "system" as const },
  );
  const accentPreference = $derived(
    applicationPreferences.snapshot?.preferences.accent ?? { mode: "system" as const },
  );
  const fixedAccentValue = $derived(
    accentPreference.mode === "fixed" ? accentPreference.value : applicationPreferences.accent,
  );

  onMount(() => {
    void loadApplicationInformation();
  });

  async function loadApplicationInformation() {
    informationLoading = true;
    informationError = "";
    try {
      const [home, version] = await Promise.all([readAppHome(), getVersion()]);
      appHome = home;
      appVersion = version;
    } catch (error) {
      informationError = error instanceof Error ? error.message : String(error);
    } finally {
      informationLoading = false;
    }
  }

  async function copyValue(value: string, label: string) {
    if (!value) return;
    try {
      await navigator.clipboard.writeText(value);
      globalStatus.set(t("settings-directory-copy-success", { label }), "saved");
    } catch {
      globalStatus.set(t("settings-directory-copy-failure", { label }), "error");
    }
  }

  function resetWorkspaceLayout() {
    workspaceLayout.resetResize("left");
    workspaceLayout.resetResize("right");
    workspaceLayout.resetResize("terminal");
    workspaceLayout.expandSidebars();
    globalStatus.set(t("settings-workspace-reset-status"), "restored");
  }

  function selectSettingsSection(section: ApplicationSettingsSection) {
    activeSection = section;
  }

  function handleSettingsTabKeydown(event: KeyboardEvent, section: ApplicationSettingsSection) {
    const index = settingsSections.indexOf(section);
    let nextIndex = index;
    if (event.key === "ArrowRight") nextIndex = (index + 1) % settingsSections.length;
    else if (event.key === "ArrowLeft") {
      nextIndex = (index - 1 + settingsSections.length) % settingsSections.length;
    } else if (event.key === "Home") nextIndex = 0;
    else if (event.key === "End") nextIndex = settingsSections.length - 1;
    else return;
    event.preventDefault();
    const next = settingsSections[nextIndex];
    if (!next) return;
    selectSettingsSection(next);
    requestAnimationFrame(() => document.getElementById(`settings-tab-${next}`)?.focus());
  }

  function updateLanguage(value: string) {
    void applicationPreferences.persistPatch(
      {
        language: value === "system"
          ? { mode: "system" }
          : { mode: "fixed", value },
      },
      t("diagnostic-application-settings-save-failed"),
    );
  }

  function setAccentPreference(mode: "system" | "brand") {
    void applicationPreferences.persistPatch(
      { accent: { mode } },
      t("diagnostic-application-settings-save-failed"),
    );
  }

  function setFixedAccent(event: Event) {
    const value = (event.currentTarget as HTMLInputElement).value;
    void applicationPreferences.persistPatch(
      { accent: { mode: "fixed", value } },
      t("diagnostic-application-settings-save-failed"),
    );
  }
</script>

<section class="settings-workspace" aria-labelledby="application-settings-title">
  <header class="workspace-heading">
    <div class="heading-icon" aria-hidden="true">
      <IconSettings size={21} stroke={1.8} />
    </div>
    <div>
      <h1 id="application-settings-title">{t("settings-title")}</h1>
      <p>{t("settings-description")}</p>
    </div>
  </header>

  <div
    class="ui-tabs settings-navigation"
    role="tablist"
    aria-label={t("settings-navigation-label")}
  >
    <button
      id="settings-tab-general"
      class="ui-tab"
      type="button"
      role="tab"
      class:active={activeSection === "general"}
      aria-selected={activeSection === "general"}
      aria-controls="settings-tab-panel"
      tabindex={activeSection === "general" ? 0 : -1}
      onclick={() => selectSettingsSection("general")}
      onkeydown={(event) => handleSettingsTabKeydown(event, "general")}
    >
      <IconSettings size={16} stroke={1.8} />
      <span>{t("settings-section-general")}</span>
    </button>
    <button
      id="settings-tab-ai"
      class="ui-tab"
      type="button"
      role="tab"
      class:active={activeSection === "ai"}
      aria-selected={activeSection === "ai"}
      aria-controls="settings-tab-panel"
      tabindex={activeSection === "ai" ? 0 : -1}
      onclick={() => selectSettingsSection("ai")}
      onkeydown={(event) => handleSettingsTabKeydown(event, "ai")}
    >
      <IconCpu size={16} stroke={1.8} />
      <span>{t("settings-section-ai")}</span>
    </button>
    <button
      id="settings-tab-system"
      class="ui-tab"
      type="button"
      role="tab"
      class:active={activeSection === "system"}
      aria-selected={activeSection === "system"}
      aria-controls="settings-tab-panel"
      tabindex={activeSection === "system" ? 0 : -1}
      onclick={() => selectSettingsSection("system")}
      onkeydown={(event) => handleSettingsTabKeydown(event, "system")}
    >
      <IconActivity size={16} stroke={1.8} />
      <span>{t("settings-section-system")}</span>
    </button>
    <button
      id="settings-tab-storage"
      class="ui-tab"
      type="button"
      role="tab"
      class:active={activeSection === "storage"}
      aria-selected={activeSection === "storage"}
      aria-controls="settings-tab-panel"
      tabindex={activeSection === "storage" ? 0 : -1}
      onclick={() => selectSettingsSection("storage")}
      onkeydown={(event) => handleSettingsTabKeydown(event, "storage")}
    >
      <IconDatabase size={16} stroke={1.8} />
      <span>{t("settings-section-storage")}</span>
    </button>
    <button
      id="settings-tab-about"
      class="ui-tab"
      type="button"
      role="tab"
      class:active={activeSection === "about"}
      aria-selected={activeSection === "about"}
      aria-controls="settings-tab-panel"
      tabindex={activeSection === "about" ? 0 : -1}
      onclick={() => selectSettingsSection("about")}
      onkeydown={(event) => handleSettingsTabKeydown(event, "about")}
    >
      <IconInfoCircle size={16} stroke={1.8} />
      <span>{t("settings-section-about")}</span>
    </button>
  </div>

  <div
    id="settings-tab-panel"
    class="settings-scroll"
    role="tabpanel"
    aria-labelledby={`settings-tab-${activeSection}`}
  >
    {#if activeSection === "general"}
      <div class="content-column">
        <section class="ui-card settings-card" aria-labelledby="appearance-title">
          <div class="card-heading">
            <div>
              <h2 id="appearance-title">{t("settings-appearance-title")}</h2>
              <p>{t("settings-appearance-description")}</p>
            </div>
            {#if applicationPreferences.loading}
              <span class="subtle-status">{t("common-loading")}</span>
            {/if}
          </div>

          <div class="preference-field">
            <label for="application-language">
              <IconLanguage size={16} stroke={1.8} aria-hidden="true" />
              <span>
                <strong>{t("settings-language-title")}</strong>
                <small>{t("settings-language-description")}</small>
              </span>
            </label>
            <SelectControl
              value={languagePreferenceValue}
              options={[{ value: "system", label: t("settings-language-system-option", { language: effectiveLanguageName }) }, ...localeOptions.map((option) => ({ value: option.locale, label: option.nativeName }))]}
              disabled={applicationPreferences.loading}
              ariaLabel={t("settings-language-title")}
              onchange={updateLanguage}
            />
          </div>

          <div class="preference-group">
            <div class="preference-label">
              <strong>{t("settings-theme-title")}</strong>
            </div>
            <div class="theme-options" aria-label={t("settings-theme-title")}>
              <button
                type="button"
                class:selected={themePreference.mode === "system"}
                aria-pressed={themePreference.mode === "system"}
                onclick={() => applicationPreferences.setThemePreference({ mode: "system" })}
              >
                <span class="theme-preview system"><IconDeviceDesktop size={20} stroke={1.8} /></span>
                <span>
                  <strong>{t("settings-theme-system-option", { theme: effectiveThemeName })}</strong>
                  <small>{t("settings-theme-system-description")}</small>
                </span>
              </button>
            <button
              type="button"
              class:selected={themePreference.mode === "fixed" && themePreference.value === "light"}
              aria-pressed={themePreference.mode === "fixed" && themePreference.value === "light"}
              onclick={() => applicationPreferences.setTheme("light")}
            >
              <span class="theme-preview light"><IconSun size={20} stroke={1.8} /></span>
              <span>
                <strong>{t("settings-theme-light-title")}</strong>
                <small>{t("settings-theme-light-description")}</small>
              </span>
            </button>
            <button
              type="button"
              class:selected={themePreference.mode === "fixed" && themePreference.value === "dark"}
              aria-pressed={themePreference.mode === "fixed" && themePreference.value === "dark"}
              onclick={() => applicationPreferences.setTheme("dark")}
            >
              <span class="theme-preview dark"><IconMoonStars size={20} stroke={1.8} /></span>
              <span>
                <strong>{t("settings-theme-dark-title")}</strong>
                <small>{t("settings-theme-dark-description")}</small>
              </span>
            </button>
            </div>
          </div>

          <div class="preference-group">
            <div class="preference-label">
              <IconPalette size={16} stroke={1.8} aria-hidden="true" />
              <strong>{t("settings-accent-title")}</strong>
            </div>
            <div class="accent-options" aria-label={t("settings-accent-title")}>
              <button
                type="button"
                class:selected={accentPreference.mode === "system"}
                aria-pressed={accentPreference.mode === "system"}
                onclick={() => setAccentPreference("system")}
              >
                <span class="accent-swatch" style={`--swatch: ${applicationPreferences.accent}`}></span>
                <span>
                  <strong>{t("settings-accent-system")}</strong>
                  <small>{t("settings-accent-system-value", { accent: applicationPreferences.accent })}</small>
                  {#if applicationPreferences.snapshot?.effective.accentSource === "fallback"}
                    <small>{t("settings-accent-fallback")}</small>
                  {/if}
                </span>
              </button>
              <button
                type="button"
                class:selected={accentPreference.mode === "brand"}
                aria-pressed={accentPreference.mode === "brand"}
                onclick={() => setAccentPreference("brand")}
              >
                <span
                  class="accent-swatch"
                  style={`--swatch: ${applicationPreferences.snapshot?.brandAccent ?? applicationPreferences.accent}`}
                ></span>
                <span>
                  <strong>{t("settings-accent-brand")}</strong>
                  <small>{t("settings-accent-brand-description")}</small>
                </span>
              </button>
              <label class:selected={accentPreference.mode === "fixed"} class="accent-custom-option">
                <input
                  type="color"
                  value={fixedAccentValue}
                  aria-label={t("settings-accent-custom-picker")}
                  onchange={setFixedAccent}
                />
                <span>
                  <strong>{t("settings-accent-custom")}</strong>
                  <small>{t("settings-accent-custom-description", { accent: fixedAccentValue })}</small>
                </span>
              </label>
            </div>
          </div>
        </section>

        <section class="ui-card settings-card" aria-labelledby="layout-title">
          <div class="card-heading">
            <div>
              <h2 id="layout-title">{t("settings-workspace-title")}</h2>
              <p>{t("settings-workspace-description")}</p>
            </div>
            <IconLayout size={20} stroke={1.7} aria-hidden="true" />
          </div>
          <button type="button" class="ui-button compact secondary-action" onclick={resetWorkspaceLayout}>
            <IconRefresh size={15} stroke={1.9} />
            <span>{t("settings-workspace-reset")}</span>
          </button>
        </section>
      </div>
    {:else if activeSection === "ai"}
      <div class="content-column">
        <section class="section-introduction">
          <h2>{t("settings-section-ai")}</h2>
          <p>{t("settings-ai-description")}</p>
        </section>
        <AiIntegrationPane
          status={aiContextStatus}
          onStatusUpdate={(text, kind) => globalStatus.set(text, kind as GlobalStatusKind)}
        />
      </div>
    {:else if activeSection === "system"}
      <div class="content-column wide">
        <section class="ui-card settings-card" aria-labelledby="directories-title">
          <div class="card-heading">
            <div>
              <h2 id="directories-title">{t("settings-directories-title")}</h2>
              <p>{t("settings-directories-description")}</p>
            </div>
            <button
              type="button"
              class="ui-icon-button mini"
              title={t("settings-directories-refresh-title")}
              aria-label={t("settings-directories-refresh-title")}
              disabled={informationLoading}
              onclick={() => void loadApplicationInformation()}
            >
              <IconRefresh size={15} stroke={1.9} />
            </button>
          </div>

          {#if informationError}
            <InlineMessage message={informationError} tone="error" />
          {:else if informationLoading && !appHome}
            <EmptyState compact title={t("settings-directories-loading")} />
          {:else}
            <div class="directory-list">
              {#each directoryEntries as entry (entry.label)}
                <div class="directory-row">
                  <IconFolder size={15} stroke={1.8} aria-hidden="true" />
                  <span>{entry.label}</span>
                  <code title={entry.value}>{entry.value}</code>
                  <button
                    class="ui-icon-button mini"
                    type="button"
                    title={t("settings-directory-copy-title", { label: entry.label })}
                    aria-label={t("settings-directory-copy-title", { label: entry.label })}
                    onclick={() => void copyValue(entry.value, entry.label)}
                  >
                    <IconClipboard size={14} stroke={1.9} />
                  </button>
                </div>
              {/each}
            </div>
          {/if}
        </section>

        <WriteAuthorityRecoveryControl
          refreshToken={diagnosticsRefreshToken}
          onStatusUpdate={(text, kind) => globalStatus.set(text, kind)}
        />

        <ObservabilityLogControl
          projectKey="application"
          refreshToken={diagnosticsRefreshToken}
          onStatusUpdate={(text, kind) => globalStatus.set(text, kind)}
        />
      </div>
    {:else if activeSection === "storage"}
      <StoragePane {globalStatus} />
    {:else}
      <div class="content-column">
        <section class="ui-card about-card">
          <div class="about-mark" aria-hidden="true">P</div>
          <div>
            <h2>Pană Studio</h2>
            <p>{t("settings-about-description")}</p>
          </div>
          <dl>
            <div>
              <dt>{t("settings-about-version")}</dt>
              <dd>{appVersion || (informationLoading ? t("settings-about-reading") : t("common-unknown"))}</dd>
            </div>
            <div><dt>{t("settings-about-identifier")}</dt><dd>{appHome?.identifier ?? "com.gabriel.panastudio"}</dd></div>
            <div><dt>{t("settings-about-kernel")}</dt><dd>Rust + Tauri</dd></div>
            <div>
              <dt>{t("settings-about-generator")}</dt>
              <dd>
                Zola {appHome?.embeddedZolaVersion
                  ?? (informationLoading ? t("settings-about-reading") : t("common-unknown"))}
              </dd>
            </div>
            <div><dt>{t("settings-about-license")}</dt><dd>EUPL-1.2-or-later</dd></div>
          </dl>
          {#if informationError}
            <InlineMessage message={informationError} tone="error" />
          {/if}
        </section>
      </div>
    {/if}
  </div>
</section>

<style>
  .settings-workspace {
    display: grid;
    grid-template-rows: auto auto minmax(0, 1fr);
    width: 100%;
    height: 100%;
    min-height: 0;
    border: 1px solid var(--wb-border-subtle, var(--border));
    border-radius: var(--radius-panel);
    overflow: hidden;
    color: var(--wb-text-primary, var(--text));
    background: var(--material-panel);
    box-shadow: var(--shadow-panel);
  }

  .workspace-heading {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 18px 24px 14px;
    border-bottom: 1px solid var(--wb-border-subtle, var(--border));
    background: var(--wb-surface-chrome, var(--surface-2));
  }

  .heading-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 38px;
    height: 38px;
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    color: var(--brand-strong);
    background: var(--control-selected);
  }

  h1,
  h2,
  p,
  dl,
  dt,
  dd {
    margin: 0;
  }

  h1 {
    font-size: 18px;
    font-weight: 850;
    letter-spacing: -0.01em;
  }

  .workspace-heading p,
  .card-heading p,
  .section-introduction p,
  .about-card > div > p {
    margin-top: 3px;
    color: var(--wb-text-muted, var(--text-muted));
    font-size: 12px;
    line-height: 1.45;
  }

  .settings-navigation {
    display: flex;
    align-self: center;
    justify-self: start;
    width: max-content;
    max-width: calc(100% - 48px);
    min-width: 0;
    margin: 8px 24px;
    overflow-x: auto;
  }

  .settings-navigation .ui-tab {
    flex: 0 0 auto;
    white-space: nowrap;
  }

  .settings-scroll {
    min-height: 0;
    padding: 22px 24px 40px;
    overflow: auto;
  }

  .content-column {
    display: grid;
    gap: 14px;
    width: min(100%, 760px);
    margin: 0 auto;
  }

  .content-column.wide {
    width: min(100%, 980px);
  }

  .settings-card,
  .about-card,
  .section-introduction {
    padding: 16px;
  }

  .section-introduction {
    padding: 0 0 4px;
    border: 0;
    background: transparent;
  }

  .settings-card,
  .about-card {
    display: grid;
    gap: 14px;
  }

  .card-heading {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
  }

  h2 {
    font-size: 14px;
    font-weight: 850;
  }

  .subtle-status {
    color: var(--wb-text-muted, var(--text-muted));
    font-size: 12px;
  }

  .preference-field,
  .preference-group {
    display: grid;
    gap: 8px;
  }

  .preference-field {
    grid-template-columns: minmax(180px, 1fr) minmax(220px, 0.8fr);
    align-items: center;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface);
  }

  .preference-field label,
  .preference-label {
    display: flex;
    align-items: center;
    gap: 9px;
  }

  .preference-field label > span {
    min-width: 0;
  }

  .preference-field strong,
  .preference-field small,
  .preference-label strong {
    display: block;
  }

  .preference-field strong,
  .preference-label strong {
    font-size: 12px;
  }

  .preference-field small {
    margin-top: 3px;
    color: var(--wb-text-muted, var(--text-muted));
    font-size: 11px;
    line-height: 1.35;
  }

  .theme-options {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
  }

  .theme-options > button {
    display: grid;
    grid-template-columns: 42px minmax(0, 1fr);
    align-items: center;
    gap: 11px;
    min-height: 70px;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: 9px;
    color: var(--wb-text-primary, var(--text));
    background: var(--surface);
    text-align: left;
    cursor: pointer;
  }

  .theme-options > button:hover {
    border-color: var(--border-4);
  }

  .theme-options > button.selected {
    border-color: var(--wb-accent, var(--brand));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--wb-accent, var(--brand)) 35%, transparent);
  }

  .theme-preview {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 42px;
    height: 42px;
    border: 1px solid var(--border);
    border-radius: 8px;
  }

  .theme-preview.light {
    color: #8b5d00;
    background: #f8f6ee;
  }

  .theme-preview.dark {
    color: #b9c7ff;
    background: #20242b;
  }

  .theme-preview.system {
    color: var(--wb-accent-strong, var(--brand-strong));
    background:
      linear-gradient(135deg, #f8f6ee 0 50%, #20242b 50% 100%);
  }

  .theme-options strong,
  .theme-options small {
    display: block;
  }

  .theme-options strong {
    font-size: 12px;
  }

  .theme-options small {
    margin-top: 4px;
    color: var(--wb-text-muted, var(--text-muted));
    font-size: 11px;
    line-height: 1.35;
  }

  .accent-options {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
  }

  .accent-options > button,
  .accent-options > label {
    display: grid;
    grid-template-columns: 32px minmax(0, 1fr);
    align-items: center;
    gap: 10px;
    min-height: 58px;
    padding: 9px 10px;
    border: 1px solid var(--border);
    border-radius: 9px;
    color: var(--wb-text-primary, var(--text));
    background: var(--surface);
    text-align: left;
    cursor: pointer;
  }

  .accent-options > button:hover,
  .accent-options > label:hover {
    border-color: var(--border-4);
  }

  .accent-options > button.selected,
  .accent-options > label.selected {
    border-color: var(--wb-accent, var(--brand));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--wb-accent, var(--brand)) 35%, transparent);
  }

  .accent-options strong,
  .accent-options small {
    display: block;
  }

  .accent-options strong {
    font-size: 12px;
  }

  .accent-options small {
    margin-top: 3px;
    color: var(--wb-text-muted, var(--text-muted));
    font-size: 11px;
    line-height: 1.35;
  }

  .accent-swatch {
    width: 28px;
    height: 28px;
    border: 2px solid color-mix(in srgb, var(--swatch, var(--brand)) 74%, white);
    border-radius: 999px;
    background: var(--swatch, var(--brand));
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--swatch, var(--brand)) 42%, black);
  }

  .accent-custom-option input[type="color"] {
    width: 30px;
    height: 30px;
    padding: 2px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface-2);
    cursor: pointer;
  }

  .secondary-action {
    justify-self: start;
  }

  .directory-list {
    display: grid;
    gap: 6px;
  }

  .directory-row {
    display: grid;
    grid-template-columns: 18px 132px minmax(0, 1fr) 28px;
    align-items: center;
    gap: 8px;
    min-height: 38px;
    padding: 4px 6px 4px 10px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
  }

  .directory-row > span {
    color: var(--wb-text-muted, var(--text-muted));
    font-size: 11px;
    font-weight: 800;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .directory-row code {
    min-width: 0;
    overflow: hidden;
    color: var(--wb-text-primary, var(--text));
    font-size: 11px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .about-card {
    grid-template-columns: 52px minmax(0, 1fr);
    align-items: center;
  }

  .about-mark {
    display: grid;
    width: 52px;
    height: 52px;
    place-items: center;
    border-radius: 12px;
    color: #fff;
    background: var(--brand-strong);
    font-size: 22px;
    font-weight: 900;
  }

  .about-card dl {
    grid-column: 1 / -1;
    display: grid;
    gap: 1px;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--border);
  }

  .about-card dl > div {
    display: grid;
    grid-template-columns: 150px minmax(0, 1fr);
    gap: 12px;
    padding: 9px 10px;
    background: var(--surface);
  }

  .about-card dt,
  .about-card dd {
    font-size: 12px;
  }

  .about-card dt {
    color: var(--wb-text-muted, var(--text-muted));
  }

  .about-card dd {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  @media (max-width: 760px) {
    .workspace-heading,
    .settings-scroll {
      padding-right: 14px;
      padding-left: 14px;
    }

    .settings-navigation {
      max-width: calc(100% - 28px);
      margin-right: 14px;
      margin-left: 14px;
    }

    .theme-options,
    .accent-options {
      grid-template-columns: 1fr;
    }

    .preference-field {
      grid-template-columns: 1fr;
    }

    .directory-row {
      grid-template-columns: 18px minmax(0, 1fr) 28px;
    }

    .directory-row > span {
      grid-column: 2;
    }

    .directory-row code {
      grid-column: 1 / 3;
      grid-row: 2;
      padding-left: 26px;
    }

    .directory-row > button {
      grid-column: 3;
      grid-row: 1 / 3;
    }
  }
</style>
