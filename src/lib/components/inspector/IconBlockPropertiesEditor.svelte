<script lang="ts">
  import { IconChevronLeft, IconChevronRight, IconSearch } from "@tabler/icons-svelte";
  import CheckboxControl from "$lib/components/ui/CheckboxControl.svelte";
  import EmptyState from "$lib/components/ui/EmptyState.svelte";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import type { EditorActionOutcome } from "$lib/editor-runtime/action-outcome";
  import { t } from "$lib/i18n/runtime.svelte";
  import {
  readIconCatalog,
  searchIconCatalog,
} from "$lib/creation/icon-io";
  import type {
    NativeIconMutationIntent,
    UiBlockSourceInstance,
  } from "$lib/blocks/contracts";
  import type {
    IconCatalogPage,
    IconCatalogSummary,
  } from "$lib/creation/contracts";
  import { errorMessage } from "$lib/util";

  let {
    sourceInstance,
    disabled = false,
    onApply,
  }: {
    sourceInstance: UiBlockSourceInstance;
    disabled?: boolean;
    onApply: (intent: NativeIconMutationIntent) => Promise<EditorActionOutcome>;
  } = $props();

  let summary = $state<IconCatalogSummary | null>(null);
  let page = $state<IconCatalogPage | null>(null);
  let query = $state("");
  let category = $state("");
  let offset = $state(0);
  let loading = $state(false);
  let loadError = $state("");
  let pending = $state(false);
  let status = $state("");
  let requestSerial = 0;
  let summaryRequested = false;
  let draftKey = "";
  let iconIdentity = $state("");
  let size = $state(24);
  let strokeWidth = $state("2");
  let decorative = $state(true);
  let accessibleLabel = $state("");

  $effect(() => {
    if (summaryRequested) return;
    summaryRequested = true;
    void readIconCatalog().then((value) => {
      summary = value;
    }).catch((cause) => {
      loadError = errorMessage(cause);
    });
  });

  $effect(() => {
    const icon = sourceInstance.icon;
    const key = icon
      ? `${sourceInstance.id}\u0000${icon.iconIdentity}\u0000${icon.size}\u0000${icon.strokeWidth}\u0000${icon.decorative}\u0000${icon.accessibleLabel ?? ""}`
      : `${sourceInstance.id}\u0000missing`;
    if (!icon || key === draftKey) return;
    draftKey = key;
    iconIdentity = icon.iconIdentity;
    size = icon.size;
    strokeWidth = icon.strokeWidth;
    decorative = icon.decorative;
    accessibleLabel = icon.accessibleLabel ?? "";
    status = "";
  });

  $effect(() => {
    const currentQuery = query;
    const currentCategory = category;
    const currentOffset = offset;
    const serial = ++requestSerial;
    loading = true;
    loadError = "";
    const timer = window.setTimeout(() => {
      void searchIconCatalog({
        query: currentQuery,
        category: currentCategory || null,
        offset: currentOffset,
        limit: 48,
      }).then((value) => {
        if (serial === requestSerial) page = value;
      }).catch((cause) => {
        if (serial === requestSerial) loadError = errorMessage(cause);
      }).finally(() => {
        if (serial === requestSerial) loading = false;
      });
    }, 140);
    return () => window.clearTimeout(timer);
  });

  function chooseIcon(id: string) {
    iconIdentity = `tabler-outline:${id}`;
    status = "";
  }

  function previousPage() {
    if (!page) return;
    offset = Math.max(0, offset - page.limit);
  }

  function nextPage() {
    if (!page || !page.hasMore) return;
    offset += page.limit;
  }

  async function apply() {
    if (pending || disabled || !iconIdentity) return;
    pending = true;
    status = t("inspector-icon-applying");
    try {
      const outcome = await onApply({
        iconIdentity,
        size,
        strokeWidth,
        decorative,
        accessibleLabel: decorative ? null : accessibleLabel.trim() || null,
      });
      status = outcome.status === "committed"
        ? t("inspector-icon-applied")
        : outcome.reason ?? t("inspector-icon-apply-failed");
    } catch (cause) {
      status = errorMessage(cause);
    } finally {
      pending = false;
    }
  }
</script>

<div class="icon-editor">
  <div class="catalog-controls">
    <label class="search-field">
      <span>{t("inspector-icon-search")}</span>
      <span class="search-input"><IconSearch size={14} /><input class="ui-input compact"
        value={query}
        placeholder={t("inspector-icon-search-placeholder")}
        oninput={(event) => { query = event.currentTarget.value; offset = 0; }}
      /></span>
    </label>
    <div class="ui-form-field">
      <span>{t("inspector-icon-category")}</span>
      <SelectControl value={category} options={[{ value: "", label: t("inspector-icon-all-categories") }, ...(summary?.categories ?? [])]} ariaLabel={t("inspector-icon-category")} onchange={(value) => { category = value; offset = 0; }} />
    </div>
  </div>

  {#if loadError}
    <p class="diagnostic" role="alert">{loadError}</p>
  {:else if !page || loading}
    <EmptyState compact title={t("inspector-icon-loading")} />
  {:else if page.items.length === 0}
    <EmptyState compact title={t("inspector-icon-empty")} />
  {:else}
    <div class="icon-grid" aria-label={t("inspector-icon-results")}>
      {#each page.items as item (item.id)}
        <button
          type="button"
          class:selected={iconIdentity === `${page.packId}:${item.id}`}
          title={`${item.label} · ${item.category}`}
          aria-label={item.label}
          aria-pressed={iconIdentity === `${page.packId}:${item.id}`}
          onclick={() => chooseIcon(item.id)}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            {#each item.nodes as node}
              <path
                d={node.attributes.d ?? ""}
                fill={node.attributes.fill ?? undefined}
                stroke={node.attributes.stroke ?? undefined}
                opacity={node.attributes.opacity ?? undefined}
              ></path>
            {/each}
          </svg>
        </button>
      {/each}
    </div>
    <div class="pagination">
      <button class="ui-icon-button mini" type="button" disabled={offset === 0 || loading} onclick={previousPage} aria-label={t("inspector-icon-previous")}><IconChevronLeft size={15} /></button>
      <span>{offset + 1}–{Math.min(offset + page.items.length, page.total)} / {page.total}</span>
      <button class="ui-icon-button mini" type="button" disabled={!page.hasMore || loading} onclick={nextPage} aria-label={t("inspector-icon-next")}><IconChevronRight size={15} /></button>
    </div>
  {/if}

  <div class="property-grid">
    <label>
      <span>{t("inspector-icon-size")}</span>
      <input class="ui-input compact" type="number" min="8" max="512" step="1" bind:value={size} disabled={disabled || pending} />
    </label>
    <label>
      <span>{t("inspector-icon-stroke")}</span>
      <input class="ui-input compact" type="number" min="0.5" max="4" step="0.25" value={strokeWidth} oninput={(event) => { strokeWidth = event.currentTarget.value; }} disabled={disabled || pending} />
    </label>
  </div>
  <CheckboxControl compact label={t("inspector-icon-decorative")} bind:checked={decorative} disabled={disabled || pending} />
  {#if !decorative}
    <label>
      <span>{t("inspector-icon-accessible-label")}</span>
      <input class="ui-input compact" maxlength="160" bind:value={accessibleLabel} disabled={disabled || pending} />
    </label>
  {/if}
  <div class="apply-row">
    <code>{iconIdentity}</code>
    <button class="ui-button primary compact" type="button" disabled={disabled || pending || !iconIdentity || (!decorative && !accessibleLabel.trim())} onclick={apply}>
      {pending ? t("inspector-icon-applying") : t("inspector-icon-apply")}
    </button>
  </div>
  {#if status}<p class="status" aria-live="polite">{status}</p>{/if}
</div>

<style>
  .icon-editor { display: grid; gap: 10px; }
  .catalog-controls, .property-grid { display: grid; grid-template-columns: minmax(0, 1fr) minmax(110px, 0.55fr); gap: 8px; }
  label { display: grid; gap: 4px; color: var(--muted); font-size: 11px; font-weight: 700; }
  .search-input { display: flex; align-items: center; gap: 5px; border: 1px solid var(--border); border-radius: 6px; padding-left: 7px; background: var(--surface-1); }
  .search-input input { width: 100%; border: 0; background: transparent; padding-left: 0; }
  .icon-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(38px, 1fr)); gap: 5px; max-height: 176px; overflow: auto; }
  .icon-grid button { display: grid; place-items: center; aspect-ratio: 1; border: 1px solid var(--border); border-radius: 7px; background: var(--surface-1); color: var(--text); cursor: pointer; }
  .icon-grid button:hover, .icon-grid button.selected { border-color: var(--brand); color: var(--brand); background: color-mix(in srgb, var(--brand) 10%, var(--surface-1)); }
  .icon-grid svg { width: 21px; height: 21px; }
  .pagination, .apply-row { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  .pagination span { color: var(--muted); font-size: 11px; }
  .apply-row code { min-width: 0; overflow: hidden; text-overflow: ellipsis; color: var(--muted); font-size: 11px; }
  input:disabled { opacity: 0.55; cursor: not-allowed; }
  .diagnostic, .status { margin: 0; font-size: 11px; color: var(--muted); }
  .diagnostic { color: var(--danger); }
</style>
