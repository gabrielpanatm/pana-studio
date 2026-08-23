<script lang="ts">
  import { IconDeviceFloppy, IconTrash } from "@tabler/icons-svelte";
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
      <label class="wide">
        <span>{t("inspector-dynamic-context")}</span>
        <select
          value={dynamicField.binding.context}
          disabled={busy || contextLocked}
          onchange={(event) => chooseContext(event.currentTarget.value as DynamicFieldScope)}
        >
          {#each contextChoices as context (context)}
            <option value={context}>
              {context === "page" ? t("inspector-dynamic-context-page")
                : context === "collectionItem" ? t("inspector-dynamic-context-collection")
                : context === "section" ? t("inspector-dynamic-context-section")
                : context === "site" ? t("inspector-dynamic-context-site")
                : context === "repeaterItem" ? t("inspector-dynamic-context-repeater")
                : t("inspector-dynamic-context-taxonomy")}
            </option>
          {/each}
        </select>
      </label>
      {#if listingItemContext}
        <p class="context-contract wide">
          {t("inspector-dynamic-context-locked", { label: listingItemContext.label })}
          {#if listingItemContext.modelId} · {t("inspector-dynamic-model")} <code>{listingItemContext.modelId}</code>{/if}
        </p>
      {/if}
      <label class="wide">
        <span>{t("inspector-dynamic-source")}</span>
        <select
          value={effectiveSourceGroup}
          disabled={busy || sourceGroups.length === 0}
          onchange={(event) => chooseSourceGroup(event.currentTarget.value)}
        >
          {#each sourceGroups as group (group)}
            <option value={group}>{group}</option>
          {/each}
        </select>
      </label>
      <label class="wide">
        <span>{t("inspector-dynamic-value-search")}</span>
        <input
          type="search"
          bind:value={valueSearch}
          placeholder={t("inspector-dynamic-value-search-placeholder")}
          disabled={busy || contextValues.length === 0}
        />
      </label>
      <label class="wide">
        <span>{t("inspector-dynamic-field")}</span>
        <select
          value={selectedValue?.id ?? ""}
          disabled={busy || availableValues.length === 0}
          onchange={(event) => chooseValue(event.currentTarget.value)}
        >
          {#if !selectedValue}
            <option value="" disabled>{t("inspector-dynamic-source-unavailable")}</option>
          {/if}
          {#each availableValues as definition (definition.id)}
            <option value={definition.id}>{definition.label} · {definition.valueType}</option>
          {/each}
        </select>
      </label>
      {#if selectedValue}<p class="value-description wide">{selectedValue.description}</p>{/if}
      <label class="wide">
        <span>{t("inspector-dynamic-presentation")}</span>
        <select value={dynamicField.presentation} disabled={busy} onchange={(event) => choosePresentation(event.currentTarget.value as DynamicFieldPresentation)}>
          {#each selectedValue?.compatiblePresentations ?? [dynamicField.presentation] as presentation (presentation)}
            <option value={presentation}>{presentationLabel(presentation)}</option>
          {/each}
        </select>
      </label>
      <label>
        <span>{t("inspector-dynamic-html-tag")}</span>
        <input value={dynamicField.tag} disabled={busy} oninput={(event) => patchDynamicField({ tag: event.currentTarget.value })} />
      </label>
      {#if dynamicField.presentation === "date"}
        <label class="wide">
          <span>{t("inspector-dynamic-date-format")}</span>
          <input value={dynamicField.format.dateFormat} placeholder="%d.%m.%Y" disabled={busy} oninput={(event) => patchDynamicField({ format: { ...dynamicField.format, dateFormat: event.currentTarget.value } })} />
        </label>
      {/if}
      {#if dynamicField.presentation === "number" || dynamicField.presentation === "currency" || dynamicField.presentation === "percent"}
        <label>
          <span>{t("inspector-dynamic-decimals")}</span>
          <input type="number" min="0" max="12" value={dynamicField.format.decimals ?? ""} disabled={busy} oninput={(event) => patchDynamicField({ format: { ...dynamicField.format, decimals: event.currentTarget.value ? event.currentTarget.valueAsNumber : null } })} />
        </label>
      {/if}
      {#if dynamicField.presentation === "currency"}
        <label>
          <span>{t("inspector-dynamic-currency")}</span>
          <input value={dynamicField.format.currency} placeholder="RON" disabled={busy} oninput={(event) => patchDynamicField({ format: { ...dynamicField.format, currency: event.currentTarget.value } })} />
        </label>
      {/if}
      <label>
        <span>{t("inspector-dynamic-empty-behavior")}</span>
        <select value={dynamicField.emptyBehavior} disabled={busy} onchange={(event) => patchDynamicField({ emptyBehavior: event.currentTarget.value as typeof dynamicField.emptyBehavior })}>
          <option value="renderEmpty">{t("inspector-dynamic-empty-render")}</option>
          <option value="fallback">{t("inspector-dynamic-empty-fallback")}</option>
          <option value="hide">{t("inspector-dynamic-empty-hide")}</option>
        </select>
      </label>
      <label>
        <span>{t("inspector-dynamic-prefix")}</span>
        <input value={dynamicField.prefix} disabled={busy} oninput={(event) => patchDynamicField({ prefix: event.currentTarget.value })} />
      </label>
      <label>
        <span>{t("inspector-dynamic-suffix")}</span>
        <input value={dynamicField.suffix} disabled={busy} oninput={(event) => patchDynamicField({ suffix: event.currentTarget.value })} />
      </label>
      <label class="wide">
        <span>{t("inspector-dynamic-fallback")}</span>
        <input value={dynamicField.fallback} disabled={busy} oninput={(event) => patchDynamicField({ fallback: event.currentTarget.value })} />
      </label>
      {#if dynamicField.presentation === "image" || dynamicField.presentation === "link" || dynamicField.presentation === "button"}
        <label class="wide">
          <span>{t("inspector-dynamic-accessible-label")}</span>
          <input value={dynamicField.label} disabled={busy} oninput={(event) => patchDynamicField({ label: event.currentTarget.value })} />
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
      <label class="wide">
        <span>{t("inspector-dynamic-section")}</span>
        <select value={listing.sectionPath} disabled={busy} onchange={(event) => chooseSection(event.currentTarget.value)}>
          {#each sectionChoices as section (section.file)}
            <option value={section.file}>{section.title} · {section.file}</option>
          {/each}
        </select>
      </label>
      <label class="wide">
        <span>{t("inspector-dynamic-listing-item")}</span>
        <select value={listing.listingItemId} disabled={busy} onchange={(event) => chooseListingItem(event.currentTarget.value)}>
          {#each listingItemChoices as item (item.id)}
            <option value={item.id}>{item.label} · {item.templateName}</option>
          {/each}
        </select>
      </label>
      <label>
        <span>{t("inspector-dynamic-sort")}</span>
        <select value={listing.sortBy} disabled={busy} onchange={(event) => patchListing({ sortBy: event.currentTarget.value as typeof listing.sortBy })}>
          <option value="none">{t("inspector-dynamic-sort-section")}</option>
          <option value="date">{t("inspector-dynamic-sort-date")}</option>
          <option value="updated">{t("inspector-dynamic-sort-updated")}</option>
          <option value="title">{t("inspector-dynamic-sort-title")}</option>
          <option value="weight">{t("inspector-dynamic-sort-weight")}</option>
          <option value="slug">{t("inspector-dynamic-sort-slug")}</option>
        </select>
      </label>
      <label>
        <span>{t("inspector-dynamic-order")}</span>
        <select value={listing.sortOrder} disabled={busy} onchange={(event) => patchListing({ sortOrder: event.currentTarget.value as typeof listing.sortOrder })}>
          <option value="asc">{t("inspector-dynamic-order-asc")}</option>
          <option value="desc">{t("inspector-dynamic-order-desc")}</option>
        </select>
      </label>
      <label>
        <span>{t("inspector-dynamic-limit")}</span>
        <input type="number" min="1" value={listing.limit ?? ""} placeholder={t("inspector-dynamic-no-limit")} disabled={busy} oninput={(event) => patchListing({ limit: event.currentTarget.value ? event.currentTarget.valueAsNumber : null })} />
      </label>
      <label>
        <span>{t("inspector-dynamic-offset")}</span>
        <input type="number" min="0" value={listing.offset} disabled={busy} oninput={(event) => patchListing({ offset: Number.isFinite(event.currentTarget.valueAsNumber) ? event.currentTarget.valueAsNumber : 0 })} />
      </label>
      <label>
        <span>{t("inspector-dynamic-html-tag")}</span>
        <input value={listing.tag} disabled={busy} oninput={(event) => patchListing({ tag: event.currentTarget.value })} />
      </label>
      <label>
        <span>{t("inspector-dynamic-class")}</span>
        <input value={listing.className} disabled={busy} oninput={(event) => patchListing({ className: event.currentTarget.value })} />
      </label>
      <label class="wide check-row">
        <input type="checkbox" checked={listing.includeSubsections} disabled={busy} onchange={(event) => patchListing({ includeSubsections: event.currentTarget.checked })} />
        <span>{t("inspector-dynamic-include-subsections")}</span>
      </label>
      <label class="wide">
        <span>{t("inspector-dynamic-empty-text")}</span>
        <input value={listing.emptyText} disabled={busy} oninput={(event) => patchListing({ emptyText: event.currentTarget.value })} />
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
    <button class="danger" type="button" disabled={busy} onclick={() => { void remove(); }}>
      <IconTrash size={14} /> {t("inspector-dynamic-delete")}
    </button>
    <button class="primary" type="button" disabled={busy || !changed} onclick={() => { void apply(); }}>
      <IconDeviceFloppy size={14} /> {t("inspector-dynamic-apply")}
    </button>
  </div>
  {#if status}<p class="status" aria-live="polite">{status}</p>{/if}
{/if}

<style>
  .field-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
  label { display: grid; gap: 4px; min-width: 0; }
  label.wide { grid-column: 1 / -1; }
  .wide { grid-column: 1 / -1; }
  label > span { color: var(--text-muted); font-size: 11px; font-weight: 700; }
  input, select { width: 100%; min-width: 0; height: 29px; padding: 0 7px; border: 1px solid var(--border); border-radius: var(--radius-control); color: var(--text); background: var(--surface-2); font: inherit; font-size: 11px; }
  .check-row { display: flex; flex-direction: row; align-items: center; }
  .check-row input { width: 16px; height: 16px; accent-color: var(--brand); }
  .diagnostics { margin-bottom: 8px; padding: 6px 8px; border: 1px solid color-mix(in srgb, var(--danger) 34%, var(--border)); border-radius: var(--radius-control); background: color-mix(in srgb, var(--danger) 7%, transparent); }
  .diagnostics p, .diagnostic, .status { margin: 0; color: var(--danger); font-size: 11px; line-height: 1.4; }
  .projection-note { display: flex; justify-content: space-between; gap: 8px; margin-top: 10px; padding-top: 7px; border-top: 1px solid var(--border-subtle); color: var(--text-muted); font-size: 11px; }
  .projection-note code { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .actions { display: flex; justify-content: space-between; gap: 8px; margin-top: 8px; }
  button { display: inline-flex; min-height: 28px; align-items: center; gap: 5px; padding: 0 9px; border: 1px solid var(--border); border-radius: var(--radius-control); color: var(--text); background: var(--surface-2); font: inherit; font-size: 11px; font-weight: 700; }
  button.primary { margin-left: auto; border-color: var(--brand); color: var(--on-brand); background: var(--brand); }
  button.danger { color: var(--danger); }
  button:disabled { cursor: not-allowed; opacity: .5; }
  .status { margin-top: 7px; color: var(--text-muted); }
  .context-contract, .value-description { margin: 0; padding: 6px 7px; border-radius: var(--radius-control); background: var(--surface-2); color: var(--text-muted); font-size: 11px; line-height: 1.4; }
  .advanced { padding: 6px 7px; border: 1px solid var(--border-subtle); border-radius: var(--radius-control); color: var(--text-muted); font-size: 11px; }
  .advanced summary { cursor: pointer; font-weight: 700; }
  .advanced span, .advanced code { display: block; margin-top: 6px; }
  .advanced code { overflow-wrap: anywhere; color: var(--text); }
</style>
