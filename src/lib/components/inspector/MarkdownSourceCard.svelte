<script lang="ts">
  import { IconCode, IconEdit, IconMarkdown } from "@tabler/icons-svelte";
  import { editorSourceReferenceDisplay } from "$lib/source-provenance";
  import { t } from "$lib/i18n/runtime.svelte";
  import type { EditorNavigationNode } from "$lib/types";

  let {
    navigationNode = null,
    editSelectedContent,
    openSelectedSource,
  }: {
    navigationNode?: EditorNavigationNode | null;
    editSelectedContent: () => void | Promise<void>;
    openSelectedSource: () => void | Promise<void>;
  } = $props();

  const definition = $derived(navigationNode?.sourceProvenance.definition ?? null);
  const composition = $derived(navigationNode?.sourceProvenance.composition ?? null);
  const resolved = $derived(
    navigationNode?.kind === "markdownBoundary"
      && navigationNode.sourceProvenance.resolution === "resolved"
      && Boolean(definition),
  );
</script>

<section class="markdown-source-card">
  <div class="card-head">
    <span class="kind"><IconMarkdown size={14} stroke={2} /> Markdown</span>
    <strong>{navigationNode?.label ?? t("markdown-boundary")}</strong>
  </div>

  <dl>
    <div>
      <dt>{t("markdown-boundary-source")}</dt>
      <dd>{definition
        ? editorSourceReferenceDisplay(definition)
        : t("markdown-boundary-unresolved")}</dd>
    </div>
    {#if composition}
      <div>
        <dt>{t("source-provenance-composition")}</dt>
        <dd>{editorSourceReferenceDisplay(composition)}</dd>
      </div>
    {/if}
    <div>
      <dt>{t("inspector-tera-editing")}</dt>
      <dd>{resolved
        ? t("markdown-boundary-edit-only-content")
        : t("markdown-boundary-readonly")}</dd>
    </div>
  </dl>

  <div class="card-actions">
    <button
      type="button"
      disabled={!resolved}
      title={resolved ? t("markdown-boundary-edit-content") : t("markdown-boundary-unresolved")}
      onclick={() => { void editSelectedContent(); }}
    >
      <IconEdit size={14} stroke={2} />
      <span>{t("markdown-boundary-edit-content")}</span>
    </button>
    <button
      type="button"
      disabled={!resolved}
      title={resolved ? t("markdown-boundary-open-source") : t("markdown-boundary-unresolved")}
      onclick={() => { void openSelectedSource(); }}
    >
      <IconCode size={14} stroke={2} />
      <span>{t("markdown-boundary-open-source")}</span>
    </button>
  </div>
</section>

<style>
  .markdown-source-card {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 10px;
    border: 1px solid color-mix(in srgb, var(--markdown) 38%, var(--border));
    border-radius: 9px;
    background: color-mix(in srgb, var(--markdown-soft) 62%, var(--surface-2));
  }

  .card-head,
  .kind,
  .card-actions,
  button {
    display: flex;
    align-items: center;
  }

  .card-head { gap: 8px; min-width: 0; }
  .card-head strong {
    min-width: 0;
    overflow: hidden;
    color: var(--text-strong);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .kind {
    flex: 0 0 auto;
    gap: 4px;
    padding: 3px 6px;
    border: 1px solid color-mix(in srgb, var(--markdown) 42%, transparent);
    border-radius: 6px;
    color: var(--markdown);
    font-size: 11px;
    font-weight: 850;
    background: var(--surface);
  }
  dl { display: grid; gap: 6px; margin: 0; }
  dl div { display: grid; grid-template-columns: 66px minmax(0, 1fr); gap: 8px; }
  dt { color: var(--text-muted); font-size: 12px; font-weight: 800; }
  dd { margin: 0; color: var(--text); font-size: 12px; line-height: 1.35; overflow-wrap: anywhere; }
  .card-actions { align-items: stretch; gap: 6px; }
  .card-actions button { flex: 1; justify-content: center; gap: 5px; min-height: 28px; }
</style>
