<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    panelId,
    tabId,
    detailLabel,
    list,
    detail,
  }: {
    panelId: string;
    tabId: string;
    detailLabel: string;
    list: Snippet;
    detail: Snippet;
  } = $props();
</script>

<div class="workspace-body">
  <div class="resource-list" id={panelId} role="tabpanel" aria-labelledby={tabId}>
    {@render list()}
  </div>
  <aside class="resource-detail" aria-label={detailLabel}>
    {@render detail()}
  </aside>
</div>

<style>
  .workspace-body { display: grid; grid-template-columns: minmax(340px, 1fr) minmax(290px, .58fr); min-width: 0; min-height: 0; }
  :global(.resource-detail .detail-heading) { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
  :global(.resource-detail .detail-heading > div) { min-width: 0; }
  :global(.resource-detail .detail-kicker) { color: var(--wb-accent-strong); font-size: 12px; font-weight: 850; text-transform: uppercase; }
  :global(.resource-detail .detail-heading h2) { margin: 5px 0 0; color: var(--text-strong); font-size: 19px; }
  :global(.resource-detail .detail-heading p) { margin: 5px 0 0; color: var(--wb-text-muted); font-size: 12px; line-height: 1.5; }
  :global(.resource-detail .resource-form) { display: grid; gap: 11px; }
  :global(.resource-detail .form-actions) { display: flex; justify-content: flex-end; gap: 7px; padding-top: 3px; }
  :global(.resource-detail .info-grid) { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 7px; margin: 13px 0 0; }
  :global(.resource-detail .info-grid div) { min-width: 0; padding: 8px 9px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  :global(.resource-detail .info-grid dt) { color: var(--wb-text-muted); font-size: 11px; }
  :global(.resource-detail .info-grid dd) { margin: 3px 0 0; overflow: hidden; color: var(--text-strong); font-size: 13px; text-overflow: ellipsis; white-space: nowrap; }
  :global(.resource-detail .detail-actions) { display: flex; align-items: stretch; gap: 7px; margin-top: 10px; }
  :global(.resource-detail .primary-action), :global(.resource-detail .secondary-action) { width: 100%; margin-top: 8px; }
  :global(.resource-detail .detail-actions .primary-action), :global(.resource-detail .detail-actions .secondary-action) { margin-top: 0; }
  @media (max-width: 900px) {
    .workspace-body { grid-template-columns: 1fr; }
    .resource-detail { display: none; }
    .resource-list { border-right: 0; }
  }
</style>
