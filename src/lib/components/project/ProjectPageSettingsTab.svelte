<script lang="ts">
  import { IconFileText } from "@tabler/icons-svelte";
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
        <label class="field">
          <span>{t("content-settings-field-title")}</span>
          <input value={values.title} onchange={(event) => setField("title", event.currentTarget.value)} />
        </label>
        <label class="field">
          <span>{t("content-settings-field-description")}</span>
          <textarea rows="3" value={values.description} onchange={(event) => setField("description", event.currentTarget.value)}></textarea>
        </label>
        <div class="field-grid" class:section-grid={isSection}>
          {#if !isSection}<label class="field">
            <span>{t("content-settings-field-date")}</span>
            <input type="date" value={values.date} onchange={(event) => setField("date", event.currentTarget.value)} />
          </label>{/if}
          <label class="field">
            <span>{t("content-settings-field-weight")}</span>
            <input type="number" min="0" step="1" value={values.weight} onchange={(event) => setField("weight", event.currentTarget.value)} />
          </label>
          {#if isSection}<label class="field">
            <span>{t("content-settings-field-paginate-by")}</span>
            <input type="number" min="1" step="1" required value={values.paginateBy || "6"} onchange={(event) => setField("paginateBy", event.currentTarget.value)} />
          </label>{/if}
        </div>
        <label class="field">
          <span>{t("content-settings-field-template")}</span>
          <input list="page-template-options" value={values.template} onchange={(event) => setField("template", event.currentTarget.value)} />
          <datalist id="page-template-options">
            {#each scannedTemplates.filter((template) => isActiveThemeTemplatePath(template.relativePath, activeTheme)) as template}
              <option value={templateNameForPath(template.relativePath)}></option>
            {/each}
          </datalist>
        </label>
        {#if !isSection}<label class="field">
          <span>{t("content-settings-field-slug")}</span>
          <input value={values.slug} onchange={(event) => setField("slug", event.currentTarget.value)} />
        </label>{/if}
        <label class="toggle-field">
          <input type="checkbox" checked={values.draft} onchange={(event) => setField("draft", event.currentTarget.checked)} />
          <span>{t("content-settings-field-draft")}</span>
        </label>
      </section>{/if}

      {#if view === "seo"}<section class="metadata-group">
        <h4>SEO</h4>
        <label class="field">
          <span>{t("content-settings-seo-title")}</span>
          <input value={values.seoTitle} onchange={(event) => setField("seoTitle", event.currentTarget.value)} />
        </label>
        <label class="field">
          <span>{t("content-settings-seo-description")}</span>
          <textarea rows="3" value={values.seoDescription} onchange={(event) => setField("seoDescription", event.currentTarget.value)}></textarea>
        </label>
        <label class="field">
          <span>{t("content-settings-canonical-url")}</span>
          <input type="url" value={values.canonicalUrl} onchange={(event) => setField("canonicalUrl", event.currentTarget.value)} />
        </label>
        <label class="field">
          <span>{t("content-settings-robots")}</span>
          <input placeholder="index, follow" value={values.robots} onchange={(event) => setField("robots", event.currentTarget.value)} />
        </label>
      </section>

      <section class="metadata-group">
        <h4>OpenGraph</h4>
        <label class="field">
          <span>{t("content-settings-og-title")}</span>
          <input value={values.ogTitle} onchange={(event) => setField("ogTitle", event.currentTarget.value)} />
        </label>
        <label class="field">
          <span>{t("content-settings-og-description")}</span>
          <textarea rows="3" value={values.ogDescription} onchange={(event) => setField("ogDescription", event.currentTarget.value)}></textarea>
        </label>
        <label class="field">
          <span>{t("content-settings-og-image")}</span>
          <input value={values.ogImage} onchange={(event) => setField("ogImage", event.currentTarget.value)} />
        </label>
        <label class="field">
          <span>{t("content-settings-og-type")}</span>
          <SelectControl value={values.ogType} options={ogTypeOptions} ariaLabel={t("content-settings-og-type")} onchange={(value) => setField("ogType", value)} />
        </label>
      </section>{/if}
    </div>
    {#if mutationError}<p class="mutation-error" role="alert">{mutationError}</p>{/if}
  {:else if activePage && parsed.kind === "yaml"}
    <p class="empty-text">{t("content-settings-yaml-help")}</p>
  {:else}
    <p class="empty-text">{t("content-settings-select-page")}</p>
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
  .metadata-groups h4,
  .empty-text {
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
    font-family: "JetBrains Mono", monospace;
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
    font-family: "JetBrains Mono", monospace;
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

  .field,
  .toggle-field {
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 800;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .field-grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 82px;
    gap: 6px;
  }

  .field input,
  .field textarea {
    width: 100%;
    min-width: 0;
    border: 1px solid var(--border-4);
    border-radius: 6px;
    color: var(--text);
    font-size: 12px;
    letter-spacing: 0;
    text-transform: none;
    background: var(--surface-5);
    outline: none;
  }

  .field input {
    min-height: 28px;
    padding: 0 7px;
  }

  .field textarea {
    resize: vertical;
    min-height: 58px;
    padding: 7px;
    line-height: 1.45;
  }

  .field input:focus,
  .field textarea:focus {
    border-color: var(--brand);
  }

  .toggle-field {
    display: inline-flex;
    align-items: center;
    gap: 7px;
  }

  .toggle-field input {
    width: 15px;
    height: 15px;
    accent-color: var(--brand);
  }

  .empty-text {
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.45;
  }

  .mutation-error {
    margin: 0;
    padding: 7px 8px;
    border: 1px solid color-mix(in srgb, var(--danger) 45%, var(--border-2));
    border-radius: 6px;
    color: var(--danger);
    background: color-mix(in srgb, var(--danger) 7%, var(--surface-3));
    font-size: 12px;
    line-height: 1.4;
  }
</style>
