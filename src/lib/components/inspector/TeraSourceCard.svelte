<script lang="ts">
  import { IconCode, IconEdit, IconTrash } from "@tabler/icons-svelte";
  import { buildTeraEditorContext, teraKindLabel } from "$lib/source-graph/context";
  import { formatSourceEditLocation } from "$lib/source-graph/location";
  import { sourceOriginLabel } from "$lib/source-graph/view";
  import { projectRelativeZolaPath } from "$lib/project/files";
  import { deleteTeraNodeCapability } from "$lib/tera/mutations";
  import { canRequestTemplateEditGateKind, templateEditGateReason } from "$lib/tera/template-edit-gate";
  import type { SourceGraph, SourceGraphNode } from "$lib/types";

  export let node: SourceGraphNode | null = null;
  export let graph: SourceGraph | null = null;
  export let previewSelector: string | null = null;
  export let editSelectedTeraLayer: () => void | Promise<void>;
  export let openSelectedTeraSource: () => void | Promise<void>;
  export let deleteSelectedTeraNode: () => void | Promise<void>;

  $: context = buildTeraEditorContext(graph, node?.id ?? null);
  $: deleteCapability = deleteTeraNodeCapability(node);
  $: editGateReason = templateEditGateReason(node?.kind, Boolean(previewSelector));
  $: canRequestEditGate = Boolean(previewSelector && canRequestTemplateEditGateKind(node?.kind));
  $: sourceDisplay = node?.range
    ? formatSourceEditLocation({
        file: projectRelativeZolaPath(node.file),
        line: node.range.line,
        column: node.range.column,
      })
    : (node ? projectRelativeZolaPath(node.file) : "");
  $: originLabel = node ? sourceOriginLabel(node.origin, node.themeName) : "Necunoscut";
</script>

<section class="tera-source-card">
  {#if node}
    <div class="tera-card-head">
      <span class="tera-kind">{teraKindLabel(node.kind)}</span>
      <strong>{node.label}</strong>
    </div>

    <dl class="tera-meta">
      <div>
        <dt>Sursă</dt>
        <dd>{sourceDisplay}</dd>
      </div>
      <div>
        <dt>Origine</dt>
        <dd>{originLabel}</dd>
      </div>
      <div>
        <dt>Impact</dt>
        <dd>{context.impactLabel}</dd>
      </div>
      <div>
        <dt>Editare</dt>
        <dd>{context.editabilityReason}</dd>
      </div>
    </dl>

    <div class="tera-actions">
      <button
        type="button"
        disabled={!canRequestEditGate}
        title={editGateReason}
        onclick={() => { void editSelectedTeraLayer(); }}
      >
        <IconEdit size={13} stroke={2} />
        <span>Editează</span>
      </button>
      <button
        type="button"
        title="Deschide sursa Tera în editorul de cod"
        onclick={() => { void openSelectedTeraSource(); }}
      >
        <IconCode size={13} stroke={2} />
        <span>Cod</span>
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
    <p class="tera-empty">Selectează un nod Tera din preview sau din Straturi.</p>
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
