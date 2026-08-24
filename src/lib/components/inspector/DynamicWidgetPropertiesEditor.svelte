<script lang="ts">
  import { IconDeviceFloppy, IconTrash } from "@tabler/icons-svelte";
  import CheckboxControl from "$lib/components/ui/CheckboxControl.svelte";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import type { EditorActionOutcome } from "$lib/editor-runtime/action-outcome";
  import { t } from "$lib/i18n/runtime.svelte";
  import type {
    DynamicFieldPresentation,
    DynamicFieldScope,
    DynamicValueDefinition,
    DynamicValueSource,
    DynamicWidgetProperties,
    DynamicWidgetSnapshot,
  } from "$lib/content-models/contracts";
  import type { SourceGraph } from "$lib/source-graph/graph-contract";

  let {
    snapshot,
    sourceGraph,
    onUpdate,
    onDelete,
  }: {
    snapshot: DynamicWidgetSnapshot;
    sourceGraph: SourceGraph | null;
    onUpdate: (
      snapshot: DynamicWidgetSnapshot,
      properties: DynamicWidgetProperties,
    ) => Promise<EditorActionOutcome>;
    onDelete: (snapshot: DynamicWidgetSnapshot) => Promise<EditorActionOutcome>;
  } = $props();

  let draft = $state<DynamicWidgetProperties | null>(null);
  let draftKey = "";
  let busy = $state(false);
  let status = $state("");
  let valueSearch = $state("");
  let valueSourceGroup = $state("");

  const dynamicField = $derived(draft?.kind === "dynamicField" ? draft.properties : null);
  const listing = $derived(draft?.kind === "listing" ? draft.properties : null);
  const listingItemContext = $derived(
    sourceGraph?.listingItems.items.find((item) => (
      item.file === snapshot.sourceInstance.file
      || `templates/${item.templateName}` === snapshot.sourceInstance.file
    )) ?? null,
  );
  const contextLocked = $derived(Boolean(listingItemContext));
  const contextChoices = $derived.by(() => {
    if (listingItemContext) return ["collectionItem"] as DynamicFieldScope[];
    const templateName = snapshot.sourceInstance.file.replace(/^templates\//, "");
    const consumers = (sourceGraph?.pages ?? []).filter((page) => (
      page.resolvedTemplate === templateName
    ));
    const choices: DynamicFieldScope[] = ["site"];
    if (consumers.length === 0 || consumers.some((page) => page.pageKind !== "section")) {
      choices.unshift("page");
    }
    if (consumers.length === 0 || consumers.some((page) => page.pageKind === "section")) {
      choices.push("section");
    }
    if (dynamicField?.binding.context === "repeaterItem") choices.push("repeaterItem");
    if (dynamicField?.binding.context === "taxonomyTerm") choices.push("taxonomyTerm");
    return [...new Set(choices)];
  });
  const contextValues = $derived.by(() => {
    if (!dynamicField) return [] as DynamicValueDefinition[];
    const context = dynamicField.binding.context;
    const modelId = listingItemContext?.modelId ?? null;
    return (sourceGraph?.dynamicWidgetGraph.valueCatalog ?? []).filter((definition) => (
      definition.contexts.includes(context)
      && (!modelId || definition.modelId === null || definition.modelId === modelId)
      && definition.valueType !== "listObject"
    ));
  });
  const selectedValue = $derived(
    dynamicField
      ? contextValues.find((definition) => sameSource(
          definition.source,
          dynamicField.binding.source,
        )) ?? null
      : null,
  );
  const sourceGroups = $derived([...new Set(contextValues.map((definition) => definition.group))]);
  const effectiveSourceGroup = $derived(
    valueSourceGroup || selectedValue?.group || sourceGroups[0] || "",
  );
  const availableValues = $derived.by(() => {
    const query = valueSearch.trim().toLocaleLowerCase();
    const matches = contextValues.filter((definition) => (
      definition.group === effectiveSourceGroup
      && (!query || `${definition.label} ${definition.description} ${definition.valueType}`
        .toLocaleLowerCase()
        .includes(query))
    ));
    if (
      selectedValue
      && selectedValue.group === effectiveSourceGroup
      && !matches.some((definition) => definition.id === selectedValue.id)
    ) {
      matches.unshift(selectedValue);
    }
    return matches;
  });
  const sectionChoices = $derived(
    sourceGraph?.pages.filter((page) => page.pageKind === "section") ?? [],
  );
  const listingItemChoices = $derived(
    listing
      ? sourceGraph?.listingItems.items.filter((item) => (
          item.status === "resolved"
          && item.compatibleSectionPaths.includes(listing.sectionPath)
        )) ?? []
      : [],
  );
  const changed = $derived(
    Boolean(draft && JSON.stringify(draft) !== JSON.stringify(snapshot.sourceInstance.properties)),
  );

  $effect(() => {
    const key = `${snapshot.sourceInstance.id}\u0000${snapshot.sourceInstance.sourceRevision}`;
    if (key === draftKey) return;
    draftKey = key;
    draft = snapshot.sourceInstance.properties
      ? cloneProperties(snapshot.sourceInstance.properties)
      : null;
    status = "";
    valueSearch = "";
    valueSourceGroup = "";
  });

  function cloneProperties(value: DynamicWidgetProperties): DynamicWidgetProperties {
    return JSON.parse(JSON.stringify(value)) as DynamicWidgetProperties;
  }

  function sameSource(left: DynamicValueSource, right: DynamicValueSource) {
    return JSON.stringify(left) === JSON.stringify(right);
  }

  function tagFor(presentation: DynamicFieldPresentation) {
    if (presentation === "heading") return "h2";
    if (presentation === "paragraph" || presentation === "trustedContent") return "div";
    if (presentation === "image") return "img";
    if (presentation === "link" || presentation === "button") return "a";
    return "span";
  }

  function presentationLabel(presentation: DynamicFieldPresentation) {
    return ({
      auto: t("inspector-dynamic-presentation-auto"),
      text: t("inspector-dynamic-presentation-text"),
      heading: t("inspector-dynamic-presentation-heading"),
      paragraph: t("inspector-dynamic-presentation-paragraph"),
      badge: t("inspector-dynamic-presentation-badge"),
      date: t("inspector-dynamic-presentation-date"),
      number: t("inspector-dynamic-presentation-number"),
      currency: t("inspector-dynamic-presentation-currency"),
      percent: t("inspector-dynamic-presentation-percent"),
      image: t("inspector-dynamic-presentation-image"),
      link: t("inspector-dynamic-presentation-link"),
      button: t("inspector-dynamic-presentation-button"),
      trustedContent: t("inspector-dynamic-presentation-trusted-content"),
    } satisfies Record<DynamicFieldPresentation, string>)[presentation];
  }

  function patchDynamicField(
    patch: Partial<NonNullable<typeof dynamicField>>,
  ) {
    if (!dynamicField) return;
    draft = {
      kind: "dynamicField",
      properties: { ...dynamicField, ...patch },
    };
    status = "";
  }

  function patchListing(patch: Partial<NonNullable<typeof listing>>) {
    if (!listing) return;
    draft = {
      kind: "listing",
      properties: { ...listing, ...patch },
    };
    status = "";
  }

  function chooseContext(context: DynamicFieldScope) {
    if (!dynamicField || contextLocked) return;
    const first = (sourceGraph?.dynamicWidgetGraph.valueCatalog ?? []).find((definition) => (
      definition.contexts.includes(context) && definition.valueType !== "listObject"
    ));
    if (!first) return;
    valueSearch = "";
    valueSourceGroup = first.group;
    patchDynamicField({
      binding: {
        context,
        source: first.source,
        valueType: first.valueType,
      },
      presentation: first.defaultPresentation,
      tag: first.defaultTag,
      label: first.label,
    });
  }

  function chooseValue(id: string) {
    if (!dynamicField) return;
    const definition = contextValues.find((candidate) => candidate.id === id);
    if (!definition) return;
    patchDynamicField({
      binding: {
        context: dynamicField.binding.context,
        source: definition.source,
        valueType: definition.valueType,
      },
      presentation: definition.defaultPresentation,
      tag: definition.defaultTag,
      label: definition.label,
    });
  }

  function chooseSourceGroup(group: string) {
    if (!dynamicField) return;
    valueSourceGroup = group;
    valueSearch = "";
    const first = contextValues.find((definition) => definition.group === group);
    if (first && first.id !== selectedValue?.id) chooseValue(first.id);
  }

  function choosePresentation(presentation: DynamicFieldPresentation) {
    patchDynamicField({
      presentation,
      tag: presentation === "auto"
        ? selectedValue?.defaultTag ?? "span"
        : tagFor(presentation),
    });
  }

  function chooseSection(sectionPath: string) {
    const first = sourceGraph?.listingItems.items.find((item) => (
      item.status === "resolved" && item.compatibleSectionPaths.includes(sectionPath)
    ));
    patchListing({
      sectionPath,
      ...(first
        ? { listingItemId: first.id, listingItemTemplate: first.templateName }
        : {}),
    });
  }

  function chooseListingItem(itemId: string) {
    const item = sourceGraph?.listingItems.items.find((candidate) => candidate.id === itemId);
    if (!item) return;
    patchListing({ listingItemId: item.id, listingItemTemplate: item.templateName });
  }

  async function apply() {
    if (!draft || busy || !changed) return;
    busy = true;
    status = t("inspector-dynamic-validating");
    const outcome = await onUpdate(snapshot, cloneProperties(draft));
    busy = false;
    status = outcome.status === "committed"
      ? t("inspector-dynamic-applied")
      : outcome.reason ?? t("inspector-dynamic-apply-failed");
  }

  async function remove() {
    if (busy || !window.confirm(t("inspector-dynamic-confirm-delete"))) return;
    busy = true;
    status = t("inspector-dynamic-deleting");
    const outcome = await onDelete(snapshot);
    busy = false;
    status = outcome.status === "committed"
      ? t("inspector-dynamic-deleted")
      : outcome.reason ?? t("inspector-dynamic-delete-failed");
  }
</script>

{#if !draft}
  <p class="diagnostic">{t("inspector-dynamic-contract-missing")}</p>
{:else}
  {#if snapshot.sourceInstance.diagnostics.length > 0}
    <div class="diagnostics">
      {#each snapshot.sourceInstance.diagnostics as diagnostic (diagnostic.code)}
        <p>{diagnostic.message}</p>
      {/each}
    </div>
  {/if}

  {#if dynamicField}
    <div class="field-grid">
      <div class="ui-form-field wide">
        <span class="ui-form-label">{t("inspector-dynamic-context")}</span>
        <SelectControl
          value={dynamicField.binding.context}
          options={contextChoices.map((context) => ({ value: context, label: context === "page" ? t("inspector-dynamic-context-page") : context === "collectionItem" ? t("inspector-dynamic-context-collection") : context === "section" ? t("inspector-dynamic-context-section") : context === "site" ? t("inspector-dynamic-context-site") : context === "repeaterItem" ? t("inspector-dynamic-context-repeater") : t("inspector-dynamic-context-taxonomy") }))}
          disabled={busy || contextLocked}
          ariaLabel={t("inspector-dynamic-context")}
          onchange={(value) => chooseContext(value as DynamicFieldScope)}
        />
      </div>
      {#if listingItemContext}
        <p class="context-contract wide">
          {t("inspector-dynamic-context-locked", { label: listingItemContext.label })}
          {#if listingItemContext.modelId} · {t("inspector-dynamic-model")} <code>{listingItemContext.modelId}</code>{/if}
        </p>
      {/if}
      <div class="ui-form-field wide">
        <span class="ui-form-label">{t("inspector-dynamic-source")}</span>
        <SelectControl
          value={effectiveSourceGroup}
          options={sourceGroups}
          disabled={busy || sourceGroups.length === 0}
          ariaLabel={t("inspector-dynamic-source")}
          onchange={chooseSourceGroup}
        />
      </div>
      <label class="ui-form-field wide">
        <span class="ui-form-label">{t("inspector-dynamic-value-search")}</span>
        <input class="ui-input compact"
          type="search"
          bind:value={valueSearch}
          placeholder={t("inspector-dynamic-value-search-placeholder")}
          disabled={busy || contextValues.length === 0}
        />
      </label>
      <div class="ui-form-field wide">
        <span class="ui-form-label">{t("inspector-dynamic-field")}</span>
        <SelectControl
          value={selectedValue?.id ?? ""}
          options={[...(!selectedValue ? [{ value: "", label: t("inspector-dynamic-source-unavailable") }] : []), ...availableValues.map((definition) => ({ value: definition.id, label: `${definition.label} · ${definition.valueType}` }))]}
          disabled={busy || availableValues.length === 0}
          ariaLabel={t("inspector-dynamic-field")}
          onchange={chooseValue}
        />
      </div>
      {#if selectedValue}<p class="value-description wide">{selectedValue.description}</p>{/if}
      <div class="ui-form-field wide">
        <span class="ui-form-label">{t("inspector-dynamic-presentation")}</span>
        <SelectControl value={dynamicField.presentation} options={(selectedValue?.compatiblePresentations ?? [dynamicField.presentation]).map((presentation) => ({ value: presentation, label: presentationLabel(presentation) }))} disabled={busy} ariaLabel={t("inspector-dynamic-presentation")} onchange={(value) => choosePresentation(value as DynamicFieldPresentation)} />
      </div>
      <label class="ui-form-field">
        <span class="ui-form-label">{t("inspector-dynamic-html-tag")}</span>
        <input class="ui-input compact" value={dynamicField.tag} disabled={busy} oninput={(event) => patchDynamicField({ tag: event.currentTarget.value })} />
      </label>
      {#if dynamicField.presentation === "date"}
        <label class="ui-form-field wide">
          <span class="ui-form-label">{t("inspector-dynamic-date-format")}</span>
          <input class="ui-input compact" value={dynamicField.format.dateFormat} placeholder="%d.%m.%Y" disabled={busy} oninput={(event) => patchDynamicField({ format: { ...dynamicField.format, dateFormat: event.currentTarget.value } })} />
        </label>
      {/if}
      {#if dynamicField.presentation === "number" || dynamicField.presentation === "currency" || dynamicField.presentation === "percent"}
        <label class="ui-form-field">
          <span class="ui-form-label">{t("inspector-dynamic-decimals")}</span>
          <input class="ui-input compact" type="number" min="0" max="12" value={dynamicField.format.decimals ?? ""} disabled={busy} oninput={(event) => patchDynamicField({ format: { ...dynamicField.format, decimals: event.currentTarget.value ? event.currentTarget.valueAsNumber : null } })} />
        </label>
      {/if}
      {#if dynamicField.presentation === "currency"}
        <label class="ui-form-field">
          <span class="ui-form-label">{t("inspector-dynamic-currency")}</span>
          <input class="ui-input compact" value={dynamicField.format.currency} placeholder="RON" disabled={busy} oninput={(event) => patchDynamicField({ format: { ...dynamicField.format, currency: event.currentTarget.value } })} />
        </label>
      {/if}
      <div class="ui-form-field">
        <span class="ui-form-label">{t("inspector-dynamic-empty-behavior")}</span>
        <SelectControl value={dynamicField.emptyBehavior} options={[{ value: "renderEmpty", label: t("inspector-dynamic-empty-render") }, { value: "fallback", label: t("inspector-dynamic-empty-fallback") }, { value: "hide", label: t("inspector-dynamic-empty-hide") }]} disabled={busy} ariaLabel={t("inspector-dynamic-empty-behavior")} onchange={(value) => patchDynamicField({ emptyBehavior: value as typeof dynamicField.emptyBehavior })} />
      </div>
      <label class="ui-form-field">
        <span class="ui-form-label">{t("inspector-dynamic-prefix")}</span>
        <input class="ui-input compact" value={dynamicField.prefix} disabled={busy} oninput={(event) => patchDynamicField({ prefix: event.currentTarget.value })} />
      </label>
      <label class="ui-form-field">
        <span class="ui-form-label">{t("inspector-dynamic-suffix")}</span>
        <input class="ui-input compact" value={dynamicField.suffix} disabled={busy} oninput={(event) => patchDynamicField({ suffix: event.currentTarget.value })} />
      </label>
      <label class="ui-form-field wide">
        <span class="ui-form-label">{t("inspector-dynamic-fallback")}</span>
        <input class="ui-input compact" value={dynamicField.fallback} disabled={busy} oninput={(event) => patchDynamicField({ fallback: event.currentTarget.value })} />
      </label>
      {#if dynamicField.presentation === "image" || dynamicField.presentation === "link" || dynamicField.presentation === "button"}
        <label class="ui-form-field wide">
          <span class="ui-form-label">{t("inspector-dynamic-accessible-label")}</span>
          <input class="ui-input compact" value={dynamicField.label} disabled={busy} oninput={(event) => patchDynamicField({ label: event.currentTarget.value })} />
        </label>
      {/if}
      <details class="advanced wide">
        <summary>{t("inspector-dynamic-advanced")}</summary>
        <span>{t("inspector-dynamic-expression")}</span>
        <code>{snapshot.sourceInstance.canonicalBindingExpression ?? t("inspector-dynamic-expression-unavailable")}</code>
      </details>
    </div>
  {:else if listing}
    <div class="field-grid">
      <div class="ui-form-field wide">
        <span class="ui-form-label">{t("inspector-dynamic-section")}</span>
        <SelectControl value={listing.sectionPath} options={sectionChoices.map((section) => ({ value: section.file, label: `${section.title} · ${section.file}` }))} disabled={busy} ariaLabel={t("inspector-dynamic-section")} onchange={chooseSection} />
      </div>
      <div class="ui-form-field wide">
        <span class="ui-form-label">{t("inspector-dynamic-listing-item")}</span>
        <SelectControl value={listing.listingItemId} options={listingItemChoices.map((item) => ({ value: item.id, label: `${item.label} · ${item.templateName}` }))} disabled={busy} ariaLabel={t("inspector-dynamic-listing-item")} onchange={chooseListingItem} />
      </div>
      <div class="ui-form-field">
        <span class="ui-form-label">{t("inspector-dynamic-sort")}</span>
        <SelectControl value={listing.sortBy} options={[{ value: "none", label: t("inspector-dynamic-sort-section") }, { value: "date", label: t("inspector-dynamic-sort-date") }, { value: "updated", label: t("inspector-dynamic-sort-updated") }, { value: "title", label: t("inspector-dynamic-sort-title") }, { value: "weight", label: t("inspector-dynamic-sort-weight") }, { value: "slug", label: t("inspector-dynamic-sort-slug") }]} disabled={busy} ariaLabel={t("inspector-dynamic-sort")} onchange={(value) => patchListing({ sortBy: value as typeof listing.sortBy })} />
      </div>
      <div class="ui-form-field">
        <span class="ui-form-label">{t("inspector-dynamic-order")}</span>
        <SelectControl value={listing.sortOrder} options={[{ value: "asc", label: t("inspector-dynamic-order-asc") }, { value: "desc", label: t("inspector-dynamic-order-desc") }]} disabled={busy} ariaLabel={t("inspector-dynamic-order")} onchange={(value) => patchListing({ sortOrder: value as typeof listing.sortOrder })} />
      </div>
      <label class="ui-form-field">
        <span class="ui-form-label">{t("inspector-dynamic-limit")}</span>
        <input class="ui-input compact" type="number" min="1" value={listing.limit ?? ""} placeholder={t("inspector-dynamic-no-limit")} disabled={busy} oninput={(event) => patchListing({ limit: event.currentTarget.value ? event.currentTarget.valueAsNumber : null })} />
      </label>
      <label class="ui-form-field">
        <span class="ui-form-label">{t("inspector-dynamic-offset")}</span>
        <input class="ui-input compact" type="number" min="0" value={listing.offset} disabled={busy} oninput={(event) => patchListing({ offset: Number.isFinite(event.currentTarget.valueAsNumber) ? event.currentTarget.valueAsNumber : 0 })} />
      </label>
      <label class="ui-form-field">
        <span class="ui-form-label">{t("inspector-dynamic-html-tag")}</span>
        <input class="ui-input compact" value={listing.tag} disabled={busy} oninput={(event) => patchListing({ tag: event.currentTarget.value })} />
      </label>
      <label class="ui-form-field">
        <span class="ui-form-label">{t("inspector-dynamic-class")}</span>
        <input class="ui-input compact" value={listing.className} disabled={busy} oninput={(event) => patchListing({ className: event.currentTarget.value })} />
      </label>
      <div class="wide"><CheckboxControl compact label={t("inspector-dynamic-include-subsections")} checked={listing.includeSubsections} disabled={busy} onchange={(checked) => patchListing({ includeSubsections: checked })} /></div>
      <label class="ui-form-field wide">
        <span class="ui-form-label">{t("inspector-dynamic-empty-text")}</span>
        <input class="ui-input compact" value={listing.emptyText} disabled={busy} oninput={(event) => patchListing({ emptyText: event.currentTarget.value })} />
      </label>
    </div>
  {/if}

  <div class="projection-note">
    <span>{snapshot.renderedInstances.length === 1
      ? t("inspector-dynamic-rendered-one")
      : t("inspector-dynamic-rendered-many", { count: snapshot.renderedInstances.length })}</span>
    <code>{snapshot.sourceInstance.file}</code>
  </div>
  <div class="actions">
    <button class="ui-button danger compact" type="button" disabled={busy} onclick={() => { void remove(); }}>
      <IconTrash size={14} /> {t("inspector-dynamic-delete")}
    </button>
    <button class="ui-button primary compact" type="button" disabled={busy || !changed} onclick={() => { void apply(); }}>
      <IconDeviceFloppy size={14} /> {t("inspector-dynamic-apply")}
    </button>
  </div>
  {#if status}<p class="status" aria-live="polite">{status}</p>{/if}
{/if}

<style>
  .field-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
  label { min-width: 0; }
  .wide { grid-column: 1 / -1; }
  .diagnostics { margin-bottom: 8px; padding: 6px 8px; border: 1px solid color-mix(in srgb, var(--danger) 34%, var(--border)); border-radius: var(--radius-control); background: color-mix(in srgb, var(--danger) 7%, transparent); }
  .diagnostics p, .diagnostic, .status { margin: 0; color: var(--danger); font-size: 11px; line-height: 1.4; }
  .projection-note { display: flex; justify-content: space-between; gap: 8px; margin-top: 10px; padding-top: 7px; border-top: 1px solid var(--border-subtle); color: var(--text-muted); font-size: 11px; }
  .projection-note code { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .actions { display: flex; justify-content: space-between; gap: 8px; margin-top: 8px; }
  .actions :global(.ui-button.primary) { margin-left: auto; }
  .status { margin-top: 7px; color: var(--text-muted); }
  .context-contract, .value-description { margin: 0; padding: 6px 7px; border-radius: var(--radius-control); background: var(--surface-2); color: var(--text-muted); font-size: 11px; line-height: 1.4; }
  .advanced { padding: 6px 7px; border: 1px solid var(--border-subtle); border-radius: var(--radius-control); color: var(--text-muted); font-size: 11px; }
  .advanced summary { cursor: pointer; font-weight: 700; }
  .advanced span, .advanced code { display: block; margin-top: 6px; }
  .advanced code { overflow-wrap: anywhere; color: var(--text); }
</style>
