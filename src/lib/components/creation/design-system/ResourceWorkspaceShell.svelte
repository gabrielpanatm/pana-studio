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
  :global(.resource-detail .resource-form > label) { display: grid; gap: 5px; color: var(--wb-text-muted); font-size: 12px; font-weight: 700; }
  :global(.resource-detail .resource-form > label > input:not([type="checkbox"])) { width: 100%; height: 34px; padding: 0 9px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; color: var(--text-strong); background: var(--wb-surface-document); font-size: 12px; }
  :global(.resource-detail .resource-form > label > input:not([type="checkbox"]):focus) { border-color: var(--wb-accent); }
  :global(.resource-detail .form-actions) { display: flex; justify-content: flex-end; gap: 7px; padding-top: 3px; }
  :global(.resource-detail .form-actions button) { display: inline-flex; min-height: 32px; align-items: center; justify-content: center; gap: 5px; padding: 0 11px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; font-weight: 650; }
  :global(.resource-detail .form-actions button.primary) { border-color: var(--wb-accent); color: #fff; background: var(--wb-accent); }
  :global(.resource-detail .form-error) { display: flex; align-items: flex-start; gap: 6px; margin: 0; padding: 8px; border: 1px solid color-mix(in srgb, var(--danger) 36%, var(--wb-border-subtle)); border-radius: 6px; color: var(--danger); background: color-mix(in srgb, var(--danger) 7%, var(--wb-surface-document)); font-size: 12px; line-height: 1.4; }
  :global(.resource-detail .info-grid) { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 7px; margin: 13px 0 0; }
  :global(.resource-detail .info-grid div) { min-width: 0; padding: 8px 9px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  :global(.resource-detail .info-grid dt) { color: var(--wb-text-muted); font-size: 11px; }
  :global(.resource-detail .info-grid dd) { margin: 3px 0 0; overflow: hidden; color: var(--text-strong); font-size: 13px; text-overflow: ellipsis; white-space: nowrap; }
  :global(.resource-detail .detail-actions) { display: flex; align-items: stretch; gap: 7px; margin-top: 10px; }
  :global(.resource-detail .primary-action), :global(.resource-detail .secondary-action) { display: inline-flex; width: 100%; min-height: 32px; align-items: center; justify-content: center; gap: 6px; margin-top: 8px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; font-weight: 600; }
  :global(.resource-detail .detail-actions .primary-action), :global(.resource-detail .detail-actions .secondary-action) { margin-top: 0; }
  :global(.workspace-state) { display: grid; min-height: 180px; place-items: center; color: var(--wb-text-muted); font-size: 12px; text-align: center; }
  :global(.workspace-state.error) { color: var(--danger); }
  :global(.resource-detail button:disabled) { opacity: .5; }
  :global(.resource-detail button:not(:disabled)), :global(.resource-list button:not(:disabled)) { cursor: pointer; }
  :global(.resource-detail button:focus-visible), :global(.resource-detail input:focus-visible), :global(.resource-detail select:focus-visible), :global(.resource-list button:focus-visible) { outline: 2px solid var(--wb-focus-ring); outline-offset: 1px; }
  @media (max-width: 900px) {
    .workspace-body { grid-template-columns: 1fr; }
    .resource-detail { display: none; }
    .resource-list { border-right: 0; }
  }
</style>
