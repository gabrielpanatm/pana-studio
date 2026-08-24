<script lang="ts">
  import {
    IconBolt,
    IconBox,
    IconCode,
    IconFileCode,
    IconMarkdown,
  } from "@tabler/icons-svelte";
  import { t } from "$lib/i18n/runtime.svelte";
  import type {
    InspectorSelectionSummarySnapshot,
    InspectorSelectionSummaryState,
    SelectionSnapshot,
  } from "$lib/editor/contracts";

  let {
    summary = null,
    selection = null,
    initializing = false,
    authoringDocumentPath = null,
    selectClass,
  }: {
    summary?: InspectorSelectionSummarySnapshot | null;
    selection?: SelectionSnapshot | null;
    initializing?: boolean;
    authoringDocumentPath?: string | null;
    selectClass: (className: string) => Promise<"allowed" | "blocked">;
  } = $props();

  const resolvedElement = $derived(
    summary?.state === "resolved"
      && (
        summary.subjectKind === "htmlElement"
        || summary.subjectKind === "runtimeElement"
      ),
  );
  const displayLabel = $derived(summaryLabel(summary));
  const stateDescription = $derived(summaryStateDescription(summary));
  const memberCount = $derived(selection?.aggregateCapabilities.memberCount ?? 0);
  const aggregateHtmlFacts = $derived(selection?.aggregateHtmlFacts ?? null);
  const commonAttributes = $derived(Object.entries(aggregateHtmlFacts?.commonAttributes ?? {}));

  function attributeLabel(name: string, value: string | null) {
    return value === null ? name : `${name}="${value}"`;
  }

  function summaryLabel(value: InspectorSelectionSummarySnapshot | null) {
    if (!value) {
      return initializing ? t("inspector-summary-loading") : t("inspector-no-selection");
    }
    if (value.state !== "resolved") return summaryStateLabel(value.state);
    if (authoringDocumentPath) return fileName(authoringDocumentPath);
    return value.selector
      ?? value.label
      ?? value.tag
      ?? t("inspector-summary-uninspectable");
  }

  function summaryStateLabel(state: InspectorSelectionSummaryState) {
    switch (state) {
      case "empty":
        return t("inspector-no-selection");
      case "resolving":
        return t("inspector-summary-resolving");
      case "notRendered":
        return t("inspector-summary-not-rendered");
      case "ambiguous":
        return t("inspector-summary-ambiguous");
      case "uninspectable":
        return t("inspector-summary-uninspectable");
      case "resolved":
        return t("inspector-summary-resolved");
    }
  }

  function summaryStateDescription(value: InspectorSelectionSummarySnapshot | null) {
    if (!value) {
      return initializing ? t("inspector-summary-loading") : t("inspector-no-selection");
    }
    const localized = summaryStateLabel(value.state);
    const diagnostic = value.diagnostics[0]?.message;
    return diagnostic ? `${localized} ${diagnostic}` : localized;
  }

  function subjectLabel(value: InspectorSelectionSummarySnapshot) {
    if (value.subjectKind === "boundary" && authoringDocumentPath) {
      return t("inspector-summary-kind-document");
    }
    if (value.subjectKind === "boundary" && value.boundaryKind === "markdown") {
      return t("markdown-boundary");
    }
    if (value.subjectKind === "boundary" && value.boundaryKind === "component") {
      return t("inspector-summary-kind-component");
    }
    if (value.subjectKind === "boundary") return t("inspector-summary-kind-template");
    if (value.subjectKind === "runtimeElement") return t("inspector-summary-kind-runtime");
    if (value.subjectKind === "cssRule") return t("inspector-summary-kind-css");
    return t("inspector-summary-kind-html");
  }

  function entityKind(value: InspectorSelectionSummarySnapshot) {
    if (value.subjectKind === "boundary") return value.boundaryKind ?? "template";
    if (value.subjectKind === "runtimeElement") return "runtime";
    if (value.subjectKind === "htmlElement") return "html";
    return "readonly";
  }

  function fileName(path: string) {
    return path.replaceAll("\\", "/").split("/").filter(Boolean).at(-1) ?? path;
  }

  function handleClassKeydown(event: KeyboardEvent) {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
    const current = event.currentTarget as HTMLButtonElement;
    const group = current.closest<HTMLElement>('[data-selection-class-group="true"]');
    const buttons = Array.from(
      group?.querySelectorAll<HTMLButtonElement>("button.class-chip:not(:disabled)") ?? [],
    );
    const index = buttons.indexOf(current);
    if (index < 0 || buttons.length === 0) return;
    event.preventDefault();
    const nextIndex = event.key === "Home"
      ? 0
      : event.key === "End"
        ? buttons.length - 1
        : event.key === "ArrowRight"
          ? (index + 1) % buttons.length
          : (index - 1 + buttons.length) % buttons.length;
    buttons[nextIndex]?.focus();
  }
</script>

<section
  class="selection-card"
  aria-labelledby="inspector-selection-summary-title"
  aria-describedby="inspector-selection-summary-state"
  aria-busy={(initializing && !summary) || summary?.state === "resolving"}
  data-summary-state={summary?.state ?? (initializing ? "loading" : "empty")}
>
  <h2 id="inspector-selection-summary-title" class="sr-only">
    {t("inspector-summary-label")}
  </h2>
  <span
    id="inspector-selection-summary-state"
    class="sr-only"
    role="status"
    aria-live="polite"
    aria-atomic="true"
  >
    {stateDescription}
  </span>

  <div class="selection-heading">
    <p class="selector">{displayLabel}</p>
    {#if memberCount > 1}
      <span class="selection-count">
        {t("inspector-multi-selected", { count: memberCount })}
      </span>
    {/if}
    {#if summary?.state === "resolved" && summary.blockContext}
      <span class="block-chip">
        {summary.blockContext.providerId} › &lt;{summary.tag ?? summary.blockContext.rootTag}&gt;
      </span>
    {/if}
  </div>

  {#if summary?.state === "resolved"}
    <div
      class="selection-meta"
      data-selection-class-group={resolvedElement ? "true" : undefined}
    >
      {#if resolvedElement}
        {#if memberCount > 1}
          <span class="subtle-chip">
            {selection?.aggregateCapabilities.canBatchAttributes
              ? t("inspector-multi-attributes-compatible")
              : t("inspector-multi-batch-limited")}
          </span>
          {#if aggregateHtmlFacts?.complete}
            <span class="subtle-chip" data-selection-aggregate="common">
              {t("inspector-multi-common")}
            </span>
            {#each aggregateHtmlFacts.commonClasses as className}
              <button
                class="class-chip ui-entity-selectable"
                type="button"
                title={t("inspector-edit-class", { name: className })}
                aria-pressed="false"
                onclick={() => { void selectClass(className); }}
                onkeydown={handleClassKeydown}
              >
                .{className}
              </button>
            {/each}
            {#each commonAttributes as [name, value]}
              <span class="subtle-chip">{attributeLabel(name, value)}</span>
            {/each}
            {#if aggregateHtmlFacts.commonClasses.length === 0 && commonAttributes.length === 0}
              <span class="subtle-chip">{t("inspector-multi-none")}</span>
            {/if}

            <span class="subtle-chip" data-selection-aggregate="mixed">
              {t("inspector-multi-mixed")}
            </span>
            {#each aggregateHtmlFacts.mixedClasses as className}
              <span class="subtle-chip">.{className}</span>
            {/each}
            {#each aggregateHtmlFacts.mixedAttributeNames as name}
              <span class="subtle-chip">{name}</span>
            {/each}
            {#if aggregateHtmlFacts.mixedClasses.length === 0 && aggregateHtmlFacts.mixedAttributeNames.length === 0}
              <span class="subtle-chip">{t("inspector-multi-none")}</span>
            {/if}
          {:else}
            <span class="subtle-chip">{t("inspector-multi-source-facts-unavailable")}</span>
          {/if}
        {:else if summary.classes.length}
          {#each summary.classes as className}
            <button
              class="class-chip ui-entity-selectable"
              data-ui-selected={summary.activeCssClass === className ? "true" : undefined}
              type="button"
              title={t("inspector-edit-class", { name: className })}
              aria-pressed={summary.activeCssClass === className}
              onclick={() => { void selectClass(className); }}
              onkeydown={handleClassKeydown}
            >
              {className}
            </button>
          {/each}
        {:else if memberCount <= 1}
          <span class="subtle-chip">{t("inspector-without-classes")}</span>
        {/if}
      {:else if summary.subjectKind}
        <span class="entity-chip" data-entity-kind={entityKind(summary)}>
          {#if summary.subjectKind === "boundary" && summary.boundaryKind === "component"}
            <IconBox size={12} stroke={2} />
          {:else if summary.subjectKind === "boundary" && summary.boundaryKind === "markdown"}
            <IconMarkdown size={12} stroke={2} />
          {:else if summary.subjectKind === "boundary"}
            <IconFileCode size={12} stroke={2} />
          {:else if summary.subjectKind === "runtimeElement"}
            <IconBolt size={12} stroke={2} />
          {:else}
            <IconCode size={12} stroke={2} />
          {/if}
          {subjectLabel(summary)}
        </span>
      {/if}
    </div>
  {:else if summary && summary.state !== "empty"}
    <div class="selection-meta">
      <span class="subtle-chip" title={summary.diagnostics[0]?.message}>
        {summaryStateLabel(summary.state)}
      </span>
    </div>
  {/if}
</section>

<style>
  .selection-card {
    padding: 10px;
    border: 1px solid var(--border-2);
    border-radius: var(--radius-control);
    background: var(--material-inset);
    box-shadow: var(--shadow-inset);
  }

  .entity-chip {
    --selection-entity-color: var(--entity-readonly);
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 3px 6px;
    border: 1px solid color-mix(in srgb, var(--selection-entity-color) 38%, var(--border));
    border-radius: var(--radius-control);
    color: var(--selection-entity-color);
    background: color-mix(in srgb, var(--selection-entity-color) 11%, transparent);
    font-size: 11px;
    font-weight: 700;
  }

  .entity-chip[data-entity-kind="html"] { --selection-entity-color: var(--entity-html); }
  .entity-chip[data-entity-kind="template"] { --selection-entity-color: var(--entity-template); }
  .entity-chip[data-entity-kind="component"] { --selection-entity-color: var(--entity-component); }
  .entity-chip[data-entity-kind="markdown"] { --selection-entity-color: var(--entity-markdown); }
  .entity-chip[data-entity-kind="runtime"] { --selection-entity-color: var(--entity-runtime); }

  .selection-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    min-width: 0;
  }

  .selector {
    display: inline-flex;
    max-width: 100%;
    margin: 0;
    padding: 5px 7px;
    border-radius: var(--radius-control);
    color: #ffffff;
    font-family: var(--font-mono);
    font-size: 12px;
    background: var(--selector-bg);
  }

  .block-chip {
    overflow: hidden;
    max-width: 48%;
    padding: 3px 6px;
    border-radius: var(--radius-control);
    color: var(--brand-strong);
    background: var(--brand-soft);
    font-size: 11px;
    font-weight: 700;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .selection-count {
    flex: 0 0 auto;
    padding: 3px 6px;
    border: 1px solid var(--chip-border);
    border-radius: var(--radius-control);
    color: var(--brand-strong);
    background: var(--brand-soft);
    font-size: 11px;
    font-weight: 700;
  }

  .selection-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 8px;
  }

  .class-chip,
  .subtle-chip {
    display: inline-flex;
    align-items: center;
    min-height: 24px;
    padding: 0 8px;
    border: 1px solid var(--chip-border);
    border-radius: var(--radius-control);
    color: var(--text);
    background: var(--chip-bg);
    font-size: 12px;
    font-weight: 600;
  }

  .class-chip {
    --ui-entity-background: var(--chip-bg);
    --ui-entity-border-color: var(--chip-border);
    --ui-entity-color: var(--text);
    cursor: pointer;
  }

  .subtle-chip {
    border-color: var(--border-3);
    color: var(--text-muted);
    background: var(--surface-9);
    cursor: default;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }
</style>
