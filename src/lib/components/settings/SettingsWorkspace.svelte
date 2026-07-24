<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import {
    IconActivity,
    IconClipboard,
    IconCpu,
    IconFolder,
    IconInfoCircle,
    IconLayout,
    IconMoonStars,
    IconRefresh,
    IconSettings,
    IconSun,
  } from "@tabler/icons-svelte";
  import { onMount } from "svelte";
  import ObservabilityLogControl from "$lib/components/kernel/ObservabilityLogControl.svelte";
  import WriteAuthorityRecoveryControl from "$lib/components/kernel/WriteAuthorityRecoveryControl.svelte";
  import AiIntegrationPane from "$lib/components/settings/AiIntegrationPane.svelte";
  import { readAppHome } from "$lib/application/io";
  import type { AppState } from "$lib/state/app.svelte";
  import type { AppHomeSnapshot, SaveState } from "$lib/types";

  type SettingsSection = "general" | "ai" | "system" | "about";

  let { app }: { app: AppState } = $props();

  let activeSection = $state<SettingsSection>("general");
  let appHome = $state<AppHomeSnapshot | null>(null);
  let appVersion = $state("");
  let informationLoading = $state(false);
  let informationError = $state("");
  let diagnosticsRefreshToken = $state(0);

  const directoryEntries = $derived.by(() => {
    if (!appHome) return [];
    return [
      { label: "Configurație", value: appHome.configDir },
      { label: "Date aplicație", value: appHome.dataDir },
      { label: "Cache", value: appHome.cacheDir },
      { label: "Jurnale", value: appHome.appLogsDir },
      { label: "MCP", value: appHome.mcpDir },
      { label: "Sesiuni", value: appHome.sessionsDir },
      { label: "Nucleu", value: appHome.kernelDir },
      { label: "WriteAuthority WAL", value: appHome.writeAuthorityWalDir },
    ];
  });

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
      app.setGlobalStatus(`${label} copiat.`, "saved");
    } catch {
      app.setGlobalStatus(`${label} nu a putut fi copiat.`, "error");
    }
  }

  function resetWorkspaceLayout() {
    app.resetResize("left");
    app.resetResize("right");
    app.resetResize("terminal");
    app.leftPaneCollapsed = false;
    app.rightPaneCollapsed = false;
    app.setGlobalStatus("Aspectul spațiului de lucru a fost restabilit.", "restored");
  }
</script>

<section class="settings-workspace" aria-labelledby="application-settings-title">
  <header class="workspace-heading">
    <div class="heading-icon" aria-hidden="true">
      <IconSettings size={21} stroke={1.8} />
    </div>
    <div>
      <h1 id="application-settings-title">Setări Pană Studio</h1>
      <p>Preferințe și informații ale aplicației. Nicio opțiune de aici nu modifică site-ul deschis.</p>
    </div>
  </header>

  <nav class="settings-navigation" aria-label="Secțiuni setări">
    <button
      type="button"
      class:active={activeSection === "general"}
      aria-current={activeSection === "general" ? "page" : undefined}
      onclick={() => { activeSection = "general"; }}
    >
      <IconSettings size={16} stroke={1.8} />
      <span>Generale</span>
    </button>
    <button
      type="button"
      class:active={activeSection === "ai"}
      aria-current={activeSection === "ai" ? "page" : undefined}
      onclick={() => { activeSection = "ai"; }}
    >
      <IconCpu size={16} stroke={1.8} />
      <span>AI și MCP</span>
    </button>
    <button
      type="button"
      class:active={activeSection === "system"}
      aria-current={activeSection === "system" ? "page" : undefined}
      onclick={() => { activeSection = "system"; }}
    >
      <IconActivity size={16} stroke={1.8} />
      <span>Sistem</span>
    </button>
    <button
      type="button"
      class:active={activeSection === "about"}
      aria-current={activeSection === "about" ? "page" : undefined}
      onclick={() => { activeSection = "about"; }}
    >
      <IconInfoCircle size={16} stroke={1.8} />
      <span>Despre</span>
    </button>
  </nav>

  <div class="settings-scroll">
    {#if activeSection === "general"}
      <div class="content-column">
        <section class="settings-card" aria-labelledby="appearance-title">
          <div class="card-heading">
            <div>
              <h2 id="appearance-title">Aspect</h2>
              <p>Tema este salvată în configurația globală Pană Studio și se aplică indiferent de site.</p>
            </div>
            {#if app.applicationSettingsLoading}
              <span class="subtle-status">Se încarcă…</span>
            {/if}
          </div>

          <div class="theme-options" aria-label="Tema aplicației">
            <button
              type="button"
              class:selected={app.uiTheme === "light"}
              aria-pressed={app.uiTheme === "light"}
              onclick={() => app.setApplicationTheme("light")}
            >
              <span class="theme-preview light"><IconSun size={20} stroke={1.8} /></span>
              <span><strong>Luminoasă</strong><small>Suprafețe clare și contrast temperat.</small></span>
            </button>
            <button
              type="button"
              class:selected={app.uiTheme === "dark"}
              aria-pressed={app.uiTheme === "dark"}
              onclick={() => app.setApplicationTheme("dark")}
            >
              <span class="theme-preview dark"><IconMoonStars size={20} stroke={1.8} /></span>
              <span><strong>Întunecată</strong><small>Contrast redus pentru lucru în lumină slabă.</small></span>
            </button>
          </div>
        </section>

        <section class="settings-card" aria-labelledby="layout-title">
          <div class="card-heading">
            <div>
              <h2 id="layout-title">Spațiu de lucru</h2>
              <p>Restabilește dimensiunile și vizibilitatea panourilor aplicației.</p>
            </div>
            <IconLayout size={20} stroke={1.7} aria-hidden="true" />
          </div>
          <button type="button" class="secondary-action" onclick={resetWorkspaceLayout}>
            <IconRefresh size={15} stroke={1.9} />
            <span>Restabilește aspectul implicit</span>
          </button>
        </section>
      </div>
    {:else if activeSection === "ai"}
      <div class="content-column">
        <section class="section-introduction">
          <h2>AI și MCP</h2>
          <p>Integrarea agenților, descriptorii locali și starea conexiunii Pană Studio.</p>
        </section>
        <AiIntegrationPane
          status={app.aiContextStatus}
          onStatusUpdate={(text, kind) => app.setGlobalStatus(text, kind as SaveState)}
        />
      </div>
    {:else if activeSection === "system"}
      <div class="content-column wide">
        <section class="settings-card" aria-labelledby="directories-title">
          <div class="card-heading">
            <div>
              <h2 id="directories-title">Directoarele aplicației</h2>
              <p>Locații deținute de Pană Studio, independente de rădăcina site-ului curent.</p>
            </div>
            <button
              type="button"
              class="icon-action"
              title="Recitește informațiile"
              disabled={informationLoading}
              onclick={() => void loadApplicationInformation()}
            >
              <IconRefresh size={15} stroke={1.9} />
            </button>
          </div>

          {#if informationError}
            <p class="inline-error" role="alert">{informationError}</p>
          {:else if informationLoading && !appHome}
            <p class="empty-state">Se citesc directoarele aplicației…</p>
          {:else}
            <div class="directory-list">
              {#each directoryEntries as entry (entry.label)}
                <div class="directory-row">
                  <IconFolder size={15} stroke={1.8} aria-hidden="true" />
                  <span>{entry.label}</span>
                  <code title={entry.value}>{entry.value}</code>
                  <button
                    type="button"
                    title={`Copiază ${entry.label.toLowerCase()}`}
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
          onStatusUpdate={(text, kind) => app.setGlobalStatus(text, kind)}
        />

        <ObservabilityLogControl
          projectKey="application"
          refreshToken={diagnosticsRefreshToken}
          onStatusUpdate={(text, kind) => app.setGlobalStatus(text, kind)}
        />
      </div>
    {:else}
      <div class="content-column">
        <section class="about-card">
          <div class="about-mark" aria-hidden="true">P</div>
          <div>
            <h2>Pană Studio</h2>
            <p>Editor vizual local, Rust-first, pentru site-uri Zola.</p>
          </div>
          <dl>
            <div><dt>Versiune</dt><dd>{appVersion || (informationLoading ? "Se citește…" : "Necunoscută")}</dd></div>
            <div><dt>Identificator</dt><dd>{appHome?.identifier ?? "com.gabriel.panastudio"}</dd></div>
            <div><dt>Nucleu</dt><dd>Rust + Tauri</dd></div>
            <div><dt>Generator integrat</dt><dd>Zola 0.22.1</dd></div>
            <div><dt>Licență</dt><dd>EUPL-1.2-or-later</dd></div>
          </dl>
          {#if informationError}
            <p class="inline-error" role="alert">{informationError}</p>
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
    color: var(--wb-text-primary, var(--text));
    background: var(--wb-surface-canvas, var(--surface));
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
    gap: 2px;
    min-width: 0;
    padding: 6px 24px 0;
    border-bottom: 1px solid var(--wb-border-subtle, var(--border));
    background: var(--wb-surface-chrome, var(--surface-2));
    overflow-x: auto;
  }

  .settings-navigation button {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    min-height: 34px;
    padding: 0 11px 6px;
    border: 0;
    color: var(--wb-text-muted, var(--text-muted));
    background: transparent;
    font: inherit;
    font-size: 12px;
    font-weight: 750;
    white-space: nowrap;
    cursor: pointer;
  }

  .settings-navigation button:hover,
  .settings-navigation button.active {
    color: var(--wb-text-primary, var(--text));
  }

  .settings-navigation button.active::after {
    position: absolute;
    right: 8px;
    bottom: -1px;
    left: 8px;
    height: 2px;
    border-radius: 2px 2px 0 0;
    background: var(--wb-accent, var(--brand));
    content: "";
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
    border: 1px solid var(--wb-border-subtle, var(--border));
    border-radius: 10px;
    background: var(--wb-surface-chrome, var(--surface-2));
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

  .subtle-status,
  .empty-state {
    color: var(--wb-text-muted, var(--text-muted));
    font-size: 12px;
  }

  .theme-options {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
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

  .secondary-action,
  .icon-action {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    color: var(--wb-text-primary, var(--text));
    background: var(--surface);
    cursor: pointer;
  }

  .secondary-action {
    justify-self: start;
    gap: 7px;
    min-height: 32px;
    padding: 0 10px;
    font-size: 12px;
    font-weight: 750;
  }

  .icon-action {
    width: 30px;
    height: 30px;
    padding: 0;
  }

  .secondary-action:hover,
  .icon-action:hover:not(:disabled),
  .directory-row button:hover {
    border-color: var(--border-4);
    background: var(--control-hover);
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

  .directory-row button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border: 1px solid transparent;
    border-radius: 7px;
    color: var(--wb-text-muted, var(--text-muted));
    background: transparent;
    cursor: pointer;
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

  .inline-error {
    color: var(--danger, #dc2626);
    font-size: 12px;
    line-height: 1.45;
  }

  button:focus-visible {
    outline: 2px solid var(--wb-focus-ring, var(--brand-strong));
    outline-offset: 1px;
  }

  button:disabled {
    cursor: default;
    opacity: 0.55;
  }

  @media (max-width: 760px) {
    .workspace-heading,
    .settings-scroll {
      padding-right: 14px;
      padding-left: 14px;
    }

    .settings-navigation {
      padding-left: 14px;
    }

    .theme-options {
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

    .directory-row button {
      grid-column: 3;
      grid-row: 1 / 3;
    }
  }
</style>
