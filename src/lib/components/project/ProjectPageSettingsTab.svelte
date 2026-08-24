<script lang="ts">
  import { IconFileText } from "@tabler/icons-svelte";
  import CheckboxControl from "$lib/components/ui/CheckboxControl.svelte";
  import EmptyState from "$lib/components/ui/EmptyState.svelte";
  import InlineMessage from "$lib/components/ui/InlineMessage.svelte";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import {
    legacyTranslator,
    localeRevision,
  } from "$lib/i18n/runtime.svelte";

  $: t = legacyTranslator($localeRevision);
  import type { ProjectFile } from "$lib/project/lifecycle-contract";
  import type { SourcePageKind } from "$lib/source-graph/contracts";
  import {
    pageFrontmatterMutationValue,
    parsePageFrontmatter,
    type PageFrontmatterField,
    type PageFrontmatterMutationValue,
  } from "$lib/markdown/frontmatter";
  import { isActiveThemeTemplatePath, templateNameForPath } from "$lib/project/files";
  import { errorMessage } from "$lib/util";

  export let activeScannedPath: string | null = null;
  export let scannedPages: ProjectFile[] = [];
  export let scannedTemplates: ProjectFile[] = [];
  export let activeTheme: string | null = null;
  export let pageSource = "";
  export let pageKind: SourcePageKind = "page";
  export let updatePageFrontmatterField: (
    relativePath: string,
    field: PageFrontmatterField,
    value: PageFrontmatterMutationValue,
  ) => Promise<string | void>;
  export let view: "settings" | "seo" = "settings";

  let mutationTail: Promise<void> = Promise.resolve();
  let mutationError = "";
  let pendingMutations = 0;

  $: activePage = scannedPages.find((page) => page.relativePath === activeScannedPath) ?? null;
  $: parsed = parsePageFrontmatter(pageSource);
  $: values = parsed.values;
  $: isSection = pageKind === "section";
  $: ogTypeOptions = [
    { value: "", label: t("content-settings-option-none") },
    "website",
    "article",
    "profile",
  ];

  function setField(field: PageFrontmatterField, value: string | boolean) {
    if (!activePage) return;
    const relativePath = activePage.relativePath;
    let typedValue: PageFrontmatterMutationValue;
    try {
      typedValue = pageFrontmatterMutationValue(field, value);
    } catch (error) {
      mutationError = errorMessage(error);
      return;
    }

    mutationError = "";
    pendingMutations += 1;
    const mutation = mutationTail.then(async () => {
      await updatePageFrontmatterField(relativePath, field, typedValue);
    });
    mutationTail = mutation
      .catch((error) => {
        mutationError = errorMessage(error);
      })
      .finally(() => {
        pendingMutations = Math.max(0, pendingMutations - 1);
      });
  }
</script>

<section class="panel-card page-settings-panel" aria-busy={pendingMutations > 0}>
  <div class="section-heading">
    <h3>{t("content-settings-markdown-title")}</h3>
    {#if activePage}<span>MD</span>{/if}
  </div>

  {#if activePage && parsed.kind !== "yaml"}
    <div class="page-file-chip">
      <IconFileText size={14} stroke={1.8} />
      <span>{activePage.relativePath}</span>
    </div>

    <div class="metadata-groups" aria-label={t("content-settings-frontmatter-group")}>
      {#if view === "settings"}<section class="metadata-group">
        <h4>{t("content-settings-general")}</h4>
        <label class="ui-form-field">
          <span class="ui-form-label">{t("content-settings-field-title")}</span>
          <input class="ui-input compact" value={values.title} onchange={(event) => setField("title", event.currentTarget.value)} />
        </label>
        <label class="ui-form-field">
          <span class="ui-form-label">{t("content-settings-field-description")}</span>
          <textarea class="ui-textarea" rows="3" value={values.description} onchange={(event) => setField("description", event.currentTarget.value)}></textarea>
        </label>
        <div class="field-grid" class:section-grid={isSection}>
          {#if !isSection}<label class="ui-form-field">
            <span class="ui-form-label">{t("content-settings-field-date")}</span>
            <input class="ui-input compact" type="date" value={values.date} onchange={(event) => setField("date", event.currentTarget.value)} />
          </label>{/if}
          <label class="ui-form-field">
            <span class="ui-form-label">{t("content-settings-field-weight")}</span>
            <input class="ui-input compact" type="number" min="0" step="1" value={values.weight} onchange={(event) => setField("weight", event.currentTarget.value)} />
          </label>
          {#if isSection}<label class="ui-form-field">
            <span class="ui-form-label">{t("content-settings-field-paginate-by")}</span>
            <input class="ui-input compact" type="number" min="1" step="1" required value={values.paginateBy || "6"} onchange={(event) => setField("paginateBy", event.currentTarget.value)} />
          </label>{/if}
        </div>
        <label class="ui-form-field">
          <span class="ui-form-label">{t("content-settings-field-template")}</span>
          <input class="ui-input compact" list="page-template-options" value={values.template} onchange={(event) => setField("template", event.currentTarget.value)} />
          <datalist id="page-template-options">
            {#each scannedTemplates.filter((template) => isActiveThemeTemplatePath(template.relativePath, activeTheme)) as template}
              <option value={templateNameForPath(template.relativePath)}></option>
            {/each}
          </datalist>
        </label>
        {#if !isSection}<label class="ui-form-field">
          <span class="ui-form-label">{t("content-settings-field-slug")}</span>
          <input class="ui-input compact" value={values.slug} onchange={(event) => setField("slug", event.currentTarget.value)} />
        </label>{/if}
        <CheckboxControl compact label={t("content-settings-field-draft")} checked={values.draft} onchange={(checked) => setField("draft", checked)} />
      </section>{/if}

      {#if view === "seo"}<section class="metadata-group">
        <h4>SEO</h4>
        <label class="ui-form-field">
          <span class="ui-form-label">{t("content-settings-seo-title")}</span>
          <input class="ui-input compact" value={values.seoTitle} onchange={(event) => setField("seoTitle", event.currentTarget.value)} />
        </label>
        <label class="ui-form-field">
          <span class="ui-form-label">{t("content-settings-seo-description")}</span>
          <textarea class="ui-textarea" rows="3" value={values.seoDescription} onchange={(event) => setField("seoDescription", event.currentTarget.value)}></textarea>
        </label>
        <label class="ui-form-field">
          <span class="ui-form-label">{t("content-settings-canonical-url")}</span>
          <input class="ui-input compact" type="url" value={values.canonicalUrl} onchange={(event) => setField("canonicalUrl", event.currentTarget.value)} />
        </label>
        <label class="ui-form-field">
          <span class="ui-form-label">{t("content-settings-robots")}</span>
          <input class="ui-input compact" placeholder="index, follow" value={values.robots} onchange={(event) => setField("robots", event.currentTarget.value)} />
        </label>
      </section>

      <section class="metadata-group">
        <h4>OpenGraph</h4>
        <label class="ui-form-field">
          <span class="ui-form-label">{t("content-settings-og-title")}</span>
          <input class="ui-input compact" value={values.ogTitle} onchange={(event) => setField("ogTitle", event.currentTarget.value)} />
        </label>
        <label class="ui-form-field">
          <span class="ui-form-label">{t("content-settings-og-description")}</span>
          <textarea class="ui-textarea" rows="3" value={values.ogDescription} onchange={(event) => setField("ogDescription", event.currentTarget.value)}></textarea>
        </label>
        <label class="ui-form-field">
          <span class="ui-form-label">{t("content-settings-og-image")}</span>
          <input class="ui-input compact" value={values.ogImage} onchange={(event) => setField("ogImage", event.currentTarget.value)} />
        </label>
        <div class="ui-form-field">
          <span class="ui-form-label">{t("content-settings-og-type")}</span>
          <SelectControl value={values.ogType} options={ogTypeOptions} ariaLabel={t("content-settings-og-type")} onchange={(value) => setField("ogType", value)} />
        </div>
      </section>{/if}
    </div>
    {#if mutationError}<InlineMessage tone="error" message={mutationError} />{/if}
  {:else if activePage && parsed.kind === "yaml"}
    <EmptyState title={t("content-settings-yaml-help")} compact />
  {:else}
    <EmptyState title={t("content-settings-select-page")} compact />
  {/if}
</section>

<style>
  .panel-card {
    padding: 9px;
    border: 1px solid var(--border-2);
    border-radius: 8px;
    background: var(--surface-2);
  }

  .page-settings-panel {
    display: flex;
    flex-direction: column;
    gap: 9px;
  }

  .section-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .section-heading h3,
  .metadata-groups h4 {
    margin: 0;
  }

  .section-heading h3 {
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 900;
    letter-spacing: 0.07em;
    text-transform: uppercase;
  }

  .section-heading span {
    padding: 2px 6px;
    border: 1px solid var(--border-3);
    border-radius: 999px;
    color: var(--text-muted);
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight: 800;
  }

  .page-file-chip {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    padding: 7px 8px;
    border: 1px solid var(--border-3);
    border-radius: 7px;
    color: var(--text);
    background: var(--surface-4);
  }

  .page-file-chip span {
    min-width: 0;
    overflow: hidden;
    font-family: var(--font-mono);
    font-size: 12px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .metadata-groups {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .metadata-group {
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding: 8px;
    border: 1px solid var(--border-2);
    border-radius: 7px;
    background: var(--surface-3);
  }

  .metadata-groups h4 {
    color: var(--text);
    font-size: 12px;
    font-weight: 800;
  }

  .field-grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 82px;
    gap: 6px;
  }

</style>
