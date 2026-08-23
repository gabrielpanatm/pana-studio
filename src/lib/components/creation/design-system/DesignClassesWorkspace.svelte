<script lang="ts">
  import { IconAlertTriangle, IconDeviceFloppy, IconEdit, IconTags, IconX } from "@tabler/icons-svelte";
  import type { DesignClassInventorySnapshot } from "$lib/css/design-system-contract";
  import { l10n, t } from "$lib/i18n/runtime.svelte";
  import type { SourceGraph } from "$lib/source-graph/graph-contract";
  import { errorMessage } from "$lib/util";
  import type { DetailMode } from "./contracts";
  import ResourceWorkspaceShell from "./ResourceWorkspaceShell.svelte";

  let {
    inventory,
    loading,
    error,
    sourceGraph,
    query,
    createRequest,
    busy = $bindable(false),
    createClass,
    renameClass,
    openWorkspaceSource,
  }: {
    inventory: DesignClassInventorySnapshot | null;
    loading: boolean;
    error: string;
    sourceGraph: SourceGraph | null;
    query: string;
    createRequest: number;
    busy?: boolean;
    createClass: (name: string, path: string) => Promise<boolean>;
    renameClass: (oldName: string, newName: string) => Promise<boolean>;
    openWorkspaceSource: (path: string) => void | Promise<void>;
  } = $props();

  let selectedClassName = $state("");
  let detailMode = $state<DetailMode>("info");
  let formName = $state("");
  let formPath = $state("");
  let formError = $state("");
  let mutating = $state(false);
  let lastCreateRequest = 0;
  let createRequestReady = false;

  const normalizedQuery = $derived(query.trim().toLocaleLowerCase(l10n.locale));
  const classes = $derived(
    (inventory?.classes ?? []).filter((entry) => (
      !normalizedQuery
      || `${entry.name} ${entry.files.join(" ")}`.toLocaleLowerCase(l10n.locale).includes(normalizedQuery)
    )),
  );
  const selectedClass = $derived(
    (inventory?.classes ?? []).find((entry) => entry.name === selectedClassName)
      ?? classes[0]
      ?? null,
  );

  $effect(() => { busy = mutating; });
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
    formPath = "";
    formError = "";
  }

  function selectClass(name: string) {
    selectedClassName = name;
    resetPanel();
  }

  function defaultStylePath() {
    return selectedClass?.files.find((file) => /\.(?:s?css)$/i.test(file))
      ?? sourceGraph?.styles.find((style) => style.file.endsWith(".scss"))?.file
      ?? "sass/css-framework/_componente.scss";
  }

  function beginCreate() {
    if (mutating) return;
    resetPanel();
    detailMode = "create";
    formName = "clasa-noua";
    formPath = defaultStylePath();
  }

  function beginEdit() {
    if (!selectedClass || mutating) return;
    resetPanel();
    detailMode = "edit";
    formName = selectedClass.name;
  }

  async function createDesignClass() {
    if (mutating) return;
    formError = "";
    mutating = true;
    try {
      if (await createClass(formName, formPath)) selectedClassName = formName.replace(/^\./, "");
      resetPanel();
    } catch (cause) {
      formError = errorMessage(cause);
    } finally {
      mutating = false;
    }
  }

  async function saveDesignClass() {
    if (mutating || !selectedClass) return;
    formError = "";
    mutating = true;
    try {
      if (await renameClass(selectedClass.name, formName)) selectedClassName = formName.replace(/^\./, "");
      resetPanel();
    } catch (cause) {
      formError = errorMessage(cause);
    } finally {
      mutating = false;
    }
  }
</script>

{#snippet list()}
  {#if error}
    <div class="workspace-state error" role="alert">{error}</div>
  {:else if loading && !inventory}
    <div class="workspace-state">{t("design-loading-classes")}</div>
  {:else}
    {#each classes as entry (entry.name)}
      <button type="button" class="class-row ui-entity-selectable" data-ui-selected={selectedClass?.name === entry.name ? "true" : undefined} aria-pressed={selectedClass?.name === entry.name} onclick={() => selectClass(entry.name)}>
        <span class="resource-icon"><IconTags size={16} stroke={1.8} /></span>
        <span><strong>.{entry.name}</strong><small>{t("design-files-count", { count: entry.files.length })}</small></span>
        <code>{t("design-markup-count", { count: entry.markupOccurrences })}</code>
        <small>{t("design-selectors-count", { count: entry.selectorOccurrences })}</small>
      </button>
    {:else}
      <div class="workspace-state">{t("design-empty-classes")}</div>
    {/each}
  {/if}
{/snippet}

{#snippet detail()}
  {#if detailMode === "create" || detailMode === "edit"}
    <form class="resource-form" onsubmit={(event) => {
      event.preventDefault();
      void (detailMode === "create" ? createDesignClass() : saveDesignClass());
    }}>
      <header class="detail-heading">
        <div><span class="detail-kicker">{detailMode === "create" ? t("design-new-resource") : t("design-controlled-change")}</span><h2>{detailMode === "create" ? t("design-add-resource", { resource: t("design-view-classes").toLocaleLowerCase(l10n.locale) }) : `.${selectedClass?.name ?? ""}`}</h2><p>{detailMode === "create" ? t("design-create-description") : t("design-change-description")}</p></div>
        <button class="ui-icon-button ui-close-button" type="button" aria-label={t("design-cancel-edit")} disabled={mutating} onclick={resetPanel}><IconX size={14} /></button>
      </header>
      <label><span>{t("design-class-name")}</span><input bind:value={formName} disabled={mutating} placeholder="service-card" /></label>
      {#if detailMode === "create"}<label><span>{t("design-destination-stylesheet")}</span><input bind:value={formPath} disabled={mutating} /></label>{/if}
      {#if formError}<p class="form-error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>{/if}
      <div class="form-actions"><button type="button" disabled={mutating} onclick={resetPanel}>{t("design-cancel")}</button><button class="primary" type="submit" disabled={mutating || !formName.trim() || (detailMode === "create" && !formPath.trim())}><IconDeviceFloppy size={14} /> {detailMode === "create" ? t("design-create-session") : t("design-save-changes")}</button></div>
    </form>
  {:else if selectedClass}
    <span class="detail-kicker">{t("design-class-inventory")}</span>
    <h2>.{selectedClass.name}</h2>
    <p>{t("design-class-summary", { markup: selectedClass.markupOccurrences, selectors: selectedClass.selectorOccurrences })}</p>
    <dl class="info-grid"><div><dt>{t("design-markup")}</dt><dd>{l10n.formatNumber(selectedClass.markupOccurrences)}</dd></div><div><dt>{t("design-selectors")}</dt><dd>{l10n.formatNumber(selectedClass.selectorOccurrences)}</dd></div></dl>
    <div class="detail-actions"><button class="ui-button primary primary-action" type="button" onclick={beginEdit}><IconEdit size={14} /> {t("design-edit")}</button></div>
    <div class="occurrence-list" aria-label={t("design-class-occurrences")}>
      {#each selectedClass.occurrences.slice(0, 40) as occurrence (`${occurrence.file}:${occurrence.range.start}`)}
        <button type="button" onclick={() => openWorkspaceSource(occurrence.file)}><span>{occurrence.kind === "markup" ? t("design-markup") : t("design-selectors")}</span><code>{occurrence.file}:{occurrence.range.line}:{occurrence.range.column}</code></button>
      {/each}
    </div>
  {:else}<div class="workspace-state">{t("design-empty-classes")}</div>{/if}
{/snippet}

<ResourceWorkspaceShell panelId="design-panel-classes" tabId="design-tab-classes" detailLabel={t("design-detail-label")} {list} {detail} />

<style>
  .class-row { display: grid; width: 100%; grid-template-columns: 34px minmax(0, 1fr) auto 70px; align-items: center; gap: 9px; min-height: 52px; padding: 7px 9px; border: 1px solid transparent; border-radius: 7px; color: var(--wb-text-primary); background: transparent; text-align: left; }
  .class-row > span:nth-child(2) { display: grid; min-width: 0; gap: 3px; }
  .class-row strong, .class-row small, .class-row code { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .class-row strong { color: var(--text-strong); font-size: 12px; }
  .class-row small, .class-row code { color: var(--wb-text-muted); font-size: 12px; }
  .class-row code { text-align: right; }
  .resource-icon { display: grid; width: 29px; height: 29px; place-items: center; border-radius: 6px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .occurrence-list { display: grid; max-height: 270px; margin-top: 10px; overflow: auto; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .occurrence-list button { display: grid; gap: 3px; padding: 7px 8px; border: 0; border-bottom: 1px solid var(--wb-border-subtle); color: var(--wb-text-primary); background: transparent; text-align: left; }
  .occurrence-list button:last-child { border-bottom: 0; }
  .occurrence-list button:hover { background: var(--wb-control-hover); }
  .occurrence-list span { color: var(--wb-accent-strong); font-size: 12px; font-weight: 800; text-transform: uppercase; }
  .occurrence-list code { overflow: hidden; color: var(--wb-text-muted); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
</style>
