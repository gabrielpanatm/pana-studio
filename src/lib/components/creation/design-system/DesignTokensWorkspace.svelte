<script lang="ts">
  import {
    IconAlertTriangle,
    IconDeviceFloppy,
    IconEdit,
    IconExternalLink,
    IconX,
  } from "@tabler/icons-svelte";
  import type { ScssVariable } from "$lib/css/contracts";
  import type { DesignTokenSnapshot } from "$lib/css/design-system-contract";
  import type { SourceGraph } from "$lib/source-graph/graph-contract";
  import { errorMessage } from "$lib/util";
  import { t } from "$lib/i18n/runtime.svelte";
  import DesignTokenCatalog from "../DesignTokenCatalog.svelte";
  import type { DesignTokenCatalogState } from "./catalog-state.svelte";
  import type { DetailMode } from "./contracts";
  import ResourceWorkspaceShell from "./ResourceWorkspaceShell.svelte";

  let {
    catalogState,
    sourceGraph,
    query,
    category,
    createRequest,
    busy = $bindable(false),
    createVariable,
    updateVariable,
    openWorkspaceSource,
  }: {
    catalogState: DesignTokenCatalogState;
    sourceGraph: SourceGraph | null;
    query: string;
    category: string;
    createRequest: number;
    busy?: boolean;
    createVariable: (path: string, name: string, value: string) => Promise<boolean>;
    updateVariable: (variable: ScssVariable, value: string) => Promise<boolean>;
    openWorkspaceSource: (path: string) => void | Promise<void>;
  } = $props();

  let selectedTokenKey = $state("");
  let detailMode = $state<DetailMode>("info");
  let formName = $state("");
  let formValue = $state("");
  let formPath = $state("");
  let formError = $state("");
  let mutating = $state(false);
  let lastCreateRequest = 0;
  let createRequestReady = false;

  const normalizedQuery = $derived(query.trim().toLocaleLowerCase());
  const visibleTokens = $derived(
    (catalogState.snapshot?.tokens ?? []).filter((token) => (
      (category === "all" || token.categoryId === category)
      && (!normalizedQuery || `${token.name} ${token.rawValue} ${token.resolvedValue ?? ""} ${token.sourcePath} ${token.groupLabel}`
        .toLocaleLowerCase()
        .includes(normalizedQuery))
    )),
  );
  const selectedToken = $derived(
    (catalogState.snapshot?.tokens ?? []).find((token) => token.id === selectedTokenKey)
      ?? visibleTokens[0]
      ?? null,
  );

  $effect(() => {
    busy = mutating;
  });

  $effect(() => {
    const request = createRequest;
    if (!createRequestReady) {
      createRequestReady = true;
      lastCreateRequest = request;
      return;
    }
    if (request === lastCreateRequest) return;
    lastCreateRequest = request;
    beginCreate();
  });

  function resetPanel() {
    detailMode = "info";
    formName = "";
    formValue = "";
    formPath = "";
    formError = "";
  }

  function selectToken(token: DesignTokenSnapshot) {
    selectedTokenKey = token.id;
    resetPanel();
  }

  function defaultStylePath() {
    return selectedToken?.sourcePath
      ?? sourceGraph?.styles.find((style) => style.file.endsWith(".scss"))?.file
      ?? "sass/css-framework/_variabile.scss";
  }

  function beginCreate() {
    if (mutating) return;
    resetPanel();
    detailMode = "create";
    formName = "token-nou";
    formValue = "0";
    formPath = defaultStylePath();
  }

  function beginEdit() {
    if (!selectedToken || mutating) return;
    resetPanel();
    detailMode = "edit";
    formName = selectedToken.name;
    formValue = selectedToken.rawValue;
    formPath = selectedToken.sourcePath;
  }

  async function createToken() {
    if (mutating) return;
    formError = "";
    mutating = true;
    try {
      const created = await createVariable(formPath, formName, formValue);
      if (created) {
        await catalogState.refresh(true);
        selectedTokenKey = catalogState.snapshot?.tokens.find((token) => (
          token.sourcePath === formPath && token.name === formName.replace(/^\$/, "")
        ))?.id ?? "";
      }
      resetPanel();
    } catch (cause) {
      formError = errorMessage(cause);
    } finally {
      mutating = false;
    }
  }

  async function saveToken() {
    if (mutating || !selectedToken) return;
    formError = "";
    mutating = true;
    try {
      const changed = await updateVariable({
        name: selectedToken.name,
        value: selectedToken.rawValue,
        file: selectedToken.sourcePath,
      }, formValue);
      if (changed) await catalogState.refresh(true);
      resetPanel();
    } catch (cause) {
      formError = errorMessage(cause);
    } finally {
      mutating = false;
    }
  }
</script>

{#snippet list()}
  <DesignTokenCatalog
    catalog={catalogState.snapshot}
    loading={catalogState.loading}
    error={catalogState.error}
    tokens={visibleTokens}
    selectedId={selectedToken?.id ?? ""}
    {selectToken}
  />
{/snippet}

{#snippet detail()}
  {#if detailMode === "create" || detailMode === "edit"}
    <form class="resource-form" onsubmit={(event) => {
      event.preventDefault();
      void (detailMode === "create" ? createToken() : saveToken());
    }}>
      <header class="detail-heading">
        <div>
          <span class="detail-kicker">{detailMode === "create" ? t("design-new-resource") : t("design-controlled-change")}</span>
          <h2>{detailMode === "create" ? t("design-add-resource", { resource: t("design-view-tokens").toLocaleLowerCase() }) : `$${selectedToken?.name ?? ""}`}</h2>
          <p>{detailMode === "create" ? t("design-create-description") : t("design-change-description")}</p>
        </div>
        <button class="ui-icon-button ui-close-button" type="button" aria-label={t("design-cancel-create")} disabled={mutating} onclick={resetPanel}><IconX size={14} /></button>
      </header>
      <label><span>{t("design-token-name")}</span><input bind:value={formName} disabled={mutating || detailMode === "edit"} placeholder="color-accent" /></label>
      <label><span>{t("design-scss-value")}</span><input bind:value={formValue} disabled={mutating} placeholder="#16836f" /></label>
      <label><span>{t("design-scss-file")}</span><input bind:value={formPath} disabled={mutating || detailMode === "edit"} /></label>
      {#if formError}<p class="form-error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>{/if}
      <div class="form-actions">
        <button type="button" disabled={mutating} onclick={resetPanel}>{t("design-cancel")}</button>
        <button class="primary" type="submit" disabled={mutating || !formName.trim() || !formPath.trim()}>
          <IconDeviceFloppy size={14} /> {mutating ? (detailMode === "create" ? t("design-creating-rust") : t("design-updating-rust")) : detailMode === "create" ? t("design-create-session") : t("design-save-changes")}
        </button>
      </div>
    </form>
  {:else if selectedToken}
    <header class="detail-heading">
      <div><span class="detail-kicker">{t("design-view-tokens")} · {selectedToken.groupLabel}</span><h2>${selectedToken.name}</h2><p>{t("design-token-description")}</p></div>
    </header>
    <dl class="info-grid">
      <div><dt>{t("design-scss-value")}</dt><dd>{selectedToken.rawValue}</dd></div>
      <div><dt>{t("design-resolved-value")}</dt><dd>{selectedToken.resolvedValue ?? t("design-unresolved")}</dd></div>
      <div><dt>{t("design-category")}</dt><dd>{selectedToken.groupLabel}</dd></div>
      <div><dt>{t("design-dependencies")}</dt><dd>{selectedToken.dependencies.length}</dd></div>
    </dl>
    {#if selectedToken.dependencies.length > 0}
      <div class="source-card"><span>{t("design-token-chain")}</span><code>{selectedToken.dependencies.map((dependency) => `$${dependency}`).join(" → ")}</code></div>
    {/if}
    {#if selectedToken.diagnostic}
      <p class="token-diagnostic" role="alert" title={selectedToken.diagnostic}><IconAlertTriangle size={14} /> {t("design-token-resolution-failed")}</p>
    {/if}
    <div class="source-card"><span>{t("design-source")}</span><code>{selectedToken.sourcePath}:{selectedToken.sourceLine}</code></div>
    <div class="detail-actions">
      <button class="ui-button secondary-action" type="button" onclick={() => openWorkspaceSource(selectedToken.sourcePath)}><IconExternalLink size={14} /> {t("design-open-source")}</button>
      <button class="ui-button primary primary-action" type="button" disabled={!selectedToken.editable} onclick={beginEdit}><IconEdit size={14} /> {t("design-edit")}</button>
    </div>
  {:else}
    <div class="workspace-state">{t("design-token-empty")}</div>
  {/if}
{/snippet}

<ResourceWorkspaceShell
  panelId="design-panel-tokens"
  tabId="design-tab-tokens"
  detailLabel={t("design-detail-label")}
  {list}
  {detail}
/>

<style>
  .source-card { display: grid; gap: 4px; margin-top: 9px; padding: 9px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  .source-card span { color: var(--wb-text-muted); font-size: 12px; font-weight: 800; text-transform: uppercase; }
  .source-card code { overflow: hidden; color: var(--wb-text-primary); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .token-diagnostic { display: flex; align-items: flex-start; gap: 6px; margin: 10px 0 0; padding: 8px; border: 1px solid color-mix(in srgb, var(--danger) 35%, var(--wb-border-subtle)); border-radius: 6px; color: var(--danger-strong, #b42318); background: color-mix(in srgb, var(--danger) 7%, var(--wb-surface-document)); font-size: 11px; line-height: 1.4; }
</style>
