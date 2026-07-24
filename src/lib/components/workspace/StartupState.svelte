<script lang="ts">
  import { IconCheck, IconPalette } from "@tabler/icons-svelte";
  import { readThemeCatalog } from "$lib/project/io";
  import type {
    ProjectWorkspaceIdentity,
    ProjectWorkspaceSnapshot,
    ThemeCatalogSnapshot,
  } from "$lib/types";
  import { errorMessage } from "$lib/util";

  let {
    scannedProject = false,
    isEmpty = false,
    isZola = false,
    openProjectFolder,
    initZolaProject,
    workspaceSnapshot = null,
  }: {
    scannedProject?: boolean;
    isEmpty?: boolean;
    isZola?: boolean;
    openProjectFolder: () => void | Promise<void>;
    initZolaProject: (themeId: string) => void | Promise<void>;
    workspaceSnapshot?: ProjectWorkspaceSnapshot | null;
  } = $props();

  let catalog = $state<ThemeCatalogSnapshot | null>(null);
  let selectedThemeId = $state("");
  let catalogError = $state("");
  let catalogLoading = $state(false);
  let catalogRequested = false;

  $effect(() => {
    if (!isEmpty || catalogRequested) return;
    catalogRequested = true;
    void loadCatalog();
  });

  function catalogIdentity(): ProjectWorkspaceIdentity | null {
    if (!workspaceSnapshot) return null;
    return {
      expectedProjectRoot: workspaceSnapshot.projectRoot,
      expectedSessionId: workspaceSnapshot.runtimeSessionId,
      expectedRevision: workspaceSnapshot.revision,
    };
  }

  async function loadCatalog() {
    catalogLoading = true;
    catalogError = "";
    try {
      catalog = await readThemeCatalog(catalogIdentity());
      selectedThemeId = catalog.themes[0]?.id ?? "";
    } catch (error) {
      catalogError = errorMessage(error);
    } finally {
      catalogLoading = false;
    }
  }
</script>

{#if !scannedProject}
  <div class="empty-state">
    <p class="empty-title">Pană Studio</p>
    <p class="empty-sub">Studio local pentru proiecte web Zola.<br>Deschide direct dosarul-rădăcină Zola pentru a începe.</p>
    <button type="button" class="empty-open-btn" onclick={openProjectFolder}>
      Deschide dosar
    </button>
  </div>
{:else if isEmpty}
  <div class="empty-state create-state">
    <p class="empty-title">Dosar gol</p>
    <p class="empty-sub">
      Alege pachetul de temă. Pană Studio va publica starterul neutru, tema și rețeta ei într-o singură inițializare validată.
    </p>
    <div class="starter-themes" aria-label="Alege tema proiectului" aria-busy={catalogLoading}>
      {#each catalog?.themes ?? [] as theme (theme.id)}
        <button
          type="button"
          class:selected={selectedThemeId === theme.id}
          class="starter-theme"
          onclick={() => { selectedThemeId = theme.id; }}
        >
          <img src={theme.previewDataUrl} alt="" />
          <span>
            <strong>{theme.name}</strong>
            <small>{theme.description}</small>
          </span>
          {#if selectedThemeId === theme.id}
            <IconCheck size={17} stroke={2} />
          {:else}
            <IconPalette size={17} stroke={1.7} />
          {/if}
        </button>
      {:else}
        <p class="catalog-state">
          {catalogLoading ? "Se încarcă temele bundled..." : "Catalogul de teme nu este disponibil."}
        </p>
      {/each}
    </div>
    {#if catalogError}
      <p class="catalog-error" role="alert">{catalogError}</p>
    {/if}
    <button
      type="button"
      class="empty-open-btn"
      disabled={!selectedThemeId || catalogLoading}
      onclick={() => initZolaProject(selectedThemeId)}
    >
      Creează proiectul cu tema aleasă
    </button>
    <button type="button" class="empty-secondary-btn" onclick={openProjectFolder}>
      Deschide alt dosar
    </button>
  </div>
{:else if !isZola}
  <div class="empty-state">
    <p class="empty-title">Nu este un proiect Pană Studio</p>
    <p class="empty-sub">
      Deschide direct dosarul care conține <code>zola.toml</code> și <code>content/</code>.<br>
      Sau alege un dosar gol pentru inițializare.
    </p>
    <button type="button" class="empty-open-btn" onclick={openProjectFolder}>
      Deschide alt dosar
    </button>
  </div>
{/if}

<style>
  .empty-state {
    position: absolute;
    inset: 0;
    z-index: 10;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 24px;
    border: 1px solid var(--border);
    border-radius: 10px;
    overflow: hidden;
    text-align: center;
    background:
      linear-gradient(135deg, color-mix(in srgb, var(--brand) 13%, transparent), transparent 34%),
      linear-gradient(315deg, color-mix(in srgb, var(--brand-strong) 10%, transparent), transparent 38%),
      repeating-linear-gradient(
        90deg,
        color-mix(in srgb, var(--border-3) 26%, transparent) 0,
        color-mix(in srgb, var(--border-3) 26%, transparent) 1px,
        transparent 1px,
        transparent 56px
      ),
      repeating-linear-gradient(
        0deg,
        color-mix(in srgb, var(--border-3) 18%, transparent) 0,
        color-mix(in srgb, var(--border-3) 18%, transparent) 1px,
        transparent 1px,
        transparent 56px
      ),
      var(--surface-6);
    box-shadow: var(--shadow);
  }

  .create-state {
    justify-content: flex-start;
    overflow: auto;
    padding-top: clamp(36px, 7vh, 72px);
  }

  .empty-state::before {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(180deg, color-mix(in srgb, var(--surface-2) 54%, transparent), transparent 42%);
    pointer-events: none;
  }

  .empty-state > * {
    position: relative;
    z-index: 1;
  }

  .empty-title {
    margin: 0;
    color: var(--text);
    font-size: 22px;
    font-weight: 800;
    letter-spacing: 0;
  }

  .empty-sub {
    max-width: 480px;
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
    line-height: 1.65;
  }

  .empty-open-btn {
    height: 34px;
    margin-top: 4px;
    padding: 0 22px;
    border: none;
    border-radius: 8px;
    color: #ffffff;
    font-size: 14px;
    font-weight: 700;
    background: var(--brand);
    cursor: pointer;
    transition: opacity 80ms;
  }

  .empty-open-btn:hover {
    opacity: 0.88;
  }

  .empty-open-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .starter-themes {
    display: grid;
    width: min(760px, 100%);
    gap: 8px;
    margin-top: 10px;
  }

  .starter-theme {
    display: grid;
    grid-template-columns: 150px minmax(0, 1fr) 24px;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 8px;
    border: 1px solid var(--border-3);
    border-radius: var(--radius-panel);
    color: var(--text);
    text-align: left;
    background: var(--surface-3);
  }

  .starter-theme:hover,
  .starter-theme.selected {
    border-color: var(--brand);
    background: var(--control-selected);
  }

  .starter-theme img {
    width: 150px;
    aspect-ratio: 16 / 10;
    border-radius: 6px;
    object-fit: cover;
  }

  .starter-theme span {
    display: grid;
    gap: 5px;
  }

  .starter-theme small {
    color: var(--text-muted);
    line-height: 1.4;
  }

  .catalog-state,
  .catalog-error {
    margin: 0;
    padding: 14px;
    color: var(--text-muted);
    font-size: 12px;
  }

  .catalog-error {
    max-width: 760px;
    color: var(--danger);
  }

  .empty-secondary-btn {
    width: 240px;
    height: 34px;
    border: 1px solid var(--border-4);
    border-radius: 8px;
    color: var(--text-muted);
    font-size: 13px;
    font-weight: 600;
    background: var(--surface-4);
    cursor: pointer;
    transition: opacity 80ms;
  }

  .empty-secondary-btn:hover {
    opacity: 0.8;
  }
</style>
