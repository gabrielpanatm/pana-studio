<script lang="ts">
  import { IconCode, IconEdit, IconTrash } from "@tabler/icons-svelte";
  import {
    editorSourceReferenceDisplay,
    teraSourceKindLabel,
  } from "$lib/source-provenance";
  import { sourceOriginLabel } from "$lib/source-graph/view";
  import { sourceCapabilityReason } from "$lib/source-graph/capabilities";
  import { deleteTeraNodeCapability } from "$lib/tera/mutations";
  import type {
    EditorNavigationNode,
    EditorSourceReference,
    SourceGraphNode,
  } from "$lib/types";
  import {
    legacyTranslator,
    localeRevision,
  } from "$lib/i18n/runtime.svelte";

  $: t = legacyTranslator($localeRevision);

  export let node: SourceGraphNode | null = null;
  export let navigationNode: EditorNavigationNode | null = null;
  export let enterTeraBoundary: (scopeId: string) => void | Promise<void>;
  export let openSelectedTeraSource: () => void | Promise<void>;
  export let deleteSelectedTeraNode: () => void | Promise<void>;

  $: deleteCapability = deleteTeraNodeCapability(node);
  $: selectedBoundary = navigationNode?.kind === "teraBoundary"
    && navigationNode.boundary?.sourceNodeId === node?.id
      ? navigationNode
      : null;
  $: canEnterBoundary = selectedBoundary?.capabilities.canEnterBoundary === true;
  $: enterBoundaryReason = canEnterBoundary
    ? t("project-navigation-enter-scope")
    : selectedBoundary
      ? sourceCapabilityReason(selectedBoundary.capabilities)
      : t("editor-navigation-boundary-missing");
  $: definition = navigationNode?.sourceProvenance.definition ?? null;
  $: composition = navigationNode?.sourceProvenance.composition ?? null;
  $: sourceDisplay = definition
    ? editorSourceReferenceDisplay(definition)
    : t("source-provenance-unresolved");
  $: compositionDisplay = composition
    ? editorSourceReferenceDisplay(composition)
    : "";
  $: originLabel = definition
    ? editorReferenceOriginLabel(definition)
    : t("inspector-tera-unknown");
  $: impactLabel = navigationNode?.boundary?.effectScope === "allRenderedInstances"
    ? t("source-provenance-impact-all", {
        count: navigationNode.boundary.renderedInstanceCount,
      })
    : navigationNode?.boundary?.effectScope === "sharedDefinition"
      ? t("source-provenance-impact-shared")
      : t("source-provenance-impact-single");
  $: editingLabel = navigationNode?.capabilities.canEnterBoundary
    ? t("source-provenance-edit-boundary")
    : navigationNode
      ? sourceCapabilityReason(navigationNode.capabilities)
      : t("editor-navigation-boundary-missing");

  function editorReferenceOriginLabel(reference: EditorSourceReference) {
    if (reference.origin === "theme") {
      return sourceOriginLabel("theme", reference.themeName);
    }
    if (reference.origin === "project") return sourceOriginLabel("local");
    return t("inspector-tera-unknown");
  }
</script>

<section class="tera-source-card">
  {#if node}
    <div class="tera-card-head">
      <span class="tera-kind">{teraSourceKindLabel(node.kind)}</span>
      <strong>{node.label}</strong>
    </div>

    <dl class="tera-meta">
      <div>
        <dt>{t("inspector-tera-source")}</dt>
        <dd>{sourceDisplay}</dd>
      </div>
      {#if compositionDisplay}
        <div>
          <dt>{t("source-provenance-composition")}</dt>
          <dd>{compositionDisplay}</dd>
        </div>
      {/if}
      <div>
        <dt>{t("inspector-tera-origin")}</dt>
        <dd>{originLabel}</dd>
      </div>
      <div>
        <dt>{t("inspector-tera-impact")}</dt>
        <dd>{impactLabel}</dd>
      </div>
      <div>
        <dt>{t("inspector-tera-editing")}</dt>
        <dd>{editingLabel}</dd>
      </div>
    </dl>

    <div class="tera-actions">
      <button
        type="button"
        disabled={!canEnterBoundary || !selectedBoundary}
        title={enterBoundaryReason}
        onclick={() => {
          if (selectedBoundary) void enterTeraBoundary(selectedBoundary.id);
        }}
      >
        <IconEdit size={13} stroke={2} />
        <span>{t("inspector-tera-edit")}</span>
      </button>
      <button
        type="button"
        title={t("inspector-tera-open-source")}
        onclick={() => { void openSelectedTeraSource(); }}
      >
        <IconCode size={13} stroke={2} />
        <span>{t("inspector-tera-code")}</span>
      </button>
      <button
        class="danger"
        type="button"
        disabled={!deleteCapability.canRun}
        title={deleteCapability.reason}
        onclick={() => { void deleteSelectedTeraNode(); }}
      >
        <IconTrash size={13} stroke={2} />
        <span>{deleteCapability.label}</span>
      </button>
    </div>
  {:else}
    <p class="tera-empty">{t("inspector-tera-select-node")}</p>
  {/if}
</section>

<style>
  .tera-source-card {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 10px;
    border: 1px solid color-mix(in srgb, var(--source-origin-theme, #d97706) 34%, var(--border-2));
    border-radius: 9px;
    background: color-mix(in srgb, var(--source-origin-theme-soft, rgba(217,119,6,0.08)) 58%, var(--surface-2));
  }

  .tera-card-head {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
  }

  .tera-card-head strong {
    min-width: 0;
    overflow: hidden;
    color: var(--text-strong);
    font-size: 13px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tera-kind {
    flex: 0 0 auto;
    padding: 4px 6px;
    border: 1px solid color-mix(in srgb, var(--source-origin-theme, #d97706) 34%, transparent);
    border-radius: 6px;
    color: var(--source-origin-theme, #d97706);
    font-size: 12px;
    font-weight: 900;
    text-transform: uppercase;
    background: var(--surface);
  }

  .tera-meta {
    display: grid;
    gap: 6px;
    margin: 0;
  }

  .tera-meta div {
    display: grid;
    grid-template-columns: 58px minmax(0, 1fr);
    gap: 8px;
    align-items: start;
  }

  .tera-meta dt {
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 800;
  }

  .tera-meta dd {
    min-width: 0;
    margin: 0;
    color: var(--text);
    font-size: 12px;
    line-height: 1.35;
    overflow-wrap: anywhere;
  }

  .tera-actions {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 6px;
  }

  .tera-actions button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    min-width: 0;
    min-height: 28px;
    padding: 0 7px;
    border: 1px solid var(--border-4);
    border-radius: 7px;
    color: var(--text);
    font-size: 12px;
    font-weight: 800;
    background: var(--surface-4);
    cursor: pointer;
  }

  .tera-actions button:hover:not(:disabled) {
    border-color: var(--brand);
    color: var(--brand-strong);
    background: var(--brand-soft);
  }

  .tera-actions button.danger {
    color: #b91c1c;
  }

  .tera-actions button.danger:hover:not(:disabled) {
    border-color: rgba(185, 28, 28, 0.38);
    color: #991b1b;
    background: rgba(254, 242, 242, 0.96);
  }

  .tera-actions button:disabled {
    opacity: 0.48;
    cursor: not-allowed;
  }

  .tera-empty {
    margin: 0;
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.4;
  }
</style>
