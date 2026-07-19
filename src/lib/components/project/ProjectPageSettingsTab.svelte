<script lang="ts">
  import { IconFileText } from "@tabler/icons-svelte";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import type { ProjectFile } from "$lib/types";
  import {
    parsePageFrontmatter,
    updatePageFrontmatter,
    type PageFrontmatterField,
  } from "$lib/markdown/frontmatter";
  import { isActiveThemeTemplatePath, templateNameForPath } from "$lib/project/files";

  export let activeScannedPath: string | null = null;
  export let scannedPages: ProjectFile[] = [];
  export let scannedTemplates: ProjectFile[] = [];
  export let activeTheme: string | null = null;
  export let pageSource = "";
  export let updatePageFrontmatterSource: (relativePath: string, source: string) => void;

  $: activePage = scannedPages.find((page) => page.relativePath === activeScannedPath) ?? null;
  $: parsed = parsePageFrontmatter(pageSource);
  $: values = parsed.values;
  const ogTypeOptions = [
    { value: "", label: "none" },
    "website",
    "article",
    "profile",
  ];

  function setField(field: PageFrontmatterField, value: string | boolean) {
    if (!activePage) return;
    const nextSource = updatePageFrontmatter(pageSource, { ...values, [field]: value });
    updatePageFrontmatterSource(activePage.relativePath, nextSource);
  }
</script>

<section class="panel-card page-settings-panel">
  <div class="section-heading">
    <h3>Pagină Markdown</h3>
    {#if activePage}<span>MD</span>{/if}
  </div>

  {#if activePage && parsed.kind !== "yaml"}
    <div class="page-file-chip">
      <IconFileText size={14} stroke={1.8} />
      <span>{activePage.relativePath}</span>
    </div>

    <div class="metadata-groups" aria-label="Frontmatter">
      <section class="metadata-group">
        <h4>General</h4>
        <label class="field">
          <span>Title</span>
          <input value={values.title} oninput={(event) => setField("title", event.currentTarget.value)} />
        </label>
        <label class="field">
          <span>Description</span>
          <textarea rows="3" value={values.description} oninput={(event) => setField("description", event.currentTarget.value)}></textarea>
        </label>
        <div class="field-grid">
          <label class="field">
            <span>Date</span>
            <input type="date" value={values.date} oninput={(event) => setField("date", event.currentTarget.value)} />
          </label>
          <label class="field">
            <span>Weight</span>
            <input type="number" step="1" value={values.weight} oninput={(event) => setField("weight", event.currentTarget.value)} />
          </label>
        </div>
        <label class="field">
          <span>Template</span>
          <input list="page-template-options" value={values.template} oninput={(event) => setField("template", event.currentTarget.value)} />
          <datalist id="page-template-options">
            {#each scannedTemplates.filter((template) => isActiveThemeTemplatePath(template.relativePath, activeTheme)) as template}
              <option value={templateNameForPath(template.relativePath)}></option>
            {/each}
          </datalist>
        </label>
        <label class="field">
          <span>Slug</span>
          <input value={values.slug} oninput={(event) => setField("slug", event.currentTarget.value)} />
        </label>
        <label class="toggle-field">
          <input type="checkbox" checked={values.draft} onchange={(event) => setField("draft", event.currentTarget.checked)} />
          <span>Draft</span>
        </label>
      </section>

      <section class="metadata-group">
        <h4>SEO</h4>
        <label class="field">
          <span>SEO title</span>
          <input value={values.seoTitle} oninput={(event) => setField("seoTitle", event.currentTarget.value)} />
        </label>
        <label class="field">
          <span>SEO description</span>
          <textarea rows="3" value={values.seoDescription} oninput={(event) => setField("seoDescription", event.currentTarget.value)}></textarea>
        </label>
        <label class="field">
          <span>Canonical URL</span>
          <input type="url" value={values.canonicalUrl} oninput={(event) => setField("canonicalUrl", event.currentTarget.value)} />
        </label>
        <label class="field">
          <span>Robots</span>
          <input placeholder="index, follow" value={values.robots} oninput={(event) => setField("robots", event.currentTarget.value)} />
        </label>
      </section>

      <section class="metadata-group">
        <h4>OpenGraph</h4>
        <label class="field">
          <span>OG title</span>
          <input value={values.ogTitle} oninput={(event) => setField("ogTitle", event.currentTarget.value)} />
        </label>
        <label class="field">
          <span>OG description</span>
          <textarea rows="3" value={values.ogDescription} oninput={(event) => setField("ogDescription", event.currentTarget.value)}></textarea>
        </label>
        <label class="field">
          <span>OG image</span>
          <input value={values.ogImage} oninput={(event) => setField("ogImage", event.currentTarget.value)} />
        </label>
        <label class="field">
          <span>OG type</span>
          <SelectControl value={values.ogType} options={ogTypeOptions} ariaLabel="OpenGraph type" onchange={(value) => setField("ogType", value)} />
        </label>
      </section>
    </div>
  {:else if activePage && parsed.kind === "yaml"}
    <p class="empty-text">Pagina folosește frontmatter YAML. Editează-l momentan în cod.</p>
  {:else}
    <p class="empty-text">Selectează o pagină Markdown.</p>
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
    font-size: 10px;
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
    font-size: 10px;
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
    font-size: 11px;
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
    font-size: 10px;
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
    font-size: 11px;
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
    font-size: 11px;
    line-height: 1.45;
  }
</style>
