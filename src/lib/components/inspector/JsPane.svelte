<script lang="ts">
  import type { InspectorSelectionSummarySnapshot } from "$lib/types";
  import type { MotionWorkspaceState } from "$lib/state/motion-workspace.svelte";
  import MotionStudioPanel from "$lib/components/inspector/js/MotionStudioPanel.svelte";
  import { t } from "$lib/i18n/runtime.svelte";

  let {
    selectionSummary = null,
    dataAnim = null,
    workspace,
    onSwitchToHtml = undefined,
  }: {
    selectionSummary?: InspectorSelectionSummarySnapshot | null;
    dataAnim?: string | null;
    workspace: MotionWorkspaceState;
    onSwitchToHtml?: () => void;
  } = $props();

  const templatePath = $derived(workspace.owner?.templatePath ?? null);
  const hasElementSelection = $derived(
    selectionSummary?.state === "resolved"
      && (
        selectionSummary.subjectKind === "htmlElement"
        || selectionSummary.subjectKind === "runtimeElement"
      ),
  );
</script>

<div class="js-pane">
  {#if !hasElementSelection}
    <div class="jp-state">
      <strong>Motion</strong>
      <span>{t("inspector-js-select-element")}</span>
    </div>
  {:else if !dataAnim}
    <div class="jp-state">
      <strong>{t("inspector-js-no-motion-identity")}</strong>
      <span>{t("inspector-js-add-data-anim-before")} <code>data-anim</code> {t("inspector-js-add-data-anim-after")}</span>
      {#if onSwitchToHtml}
        <button type="button" onclick={onSwitchToHtml}>{t("inspector-js-go-html")}</button>
      {/if}
    </div>
  {:else if !templatePath}
    <div class="jp-state jp-error">
      <strong>{t("inspector-js-unavailable")}</strong>
      <span>{t("inspector-js-no-active-template")}</span>
    </div>
  {:else if workspace.loadState === "error"}
    <div class="jp-state jp-error" role="alert">
      <strong>{t("inspector-js-motion-load-failed")}</strong>
      <span>{workspace.error}</span>
      <button type="button" onclick={() => { void workspace.reload(); }}>{t("inspector-js-retry")}</button>
    </div>
  {:else if workspace.loadState !== "ready"}
    <div class="jp-state" aria-live="polite">
      <strong>Motion v2</strong>
      <span>{t("inspector-js-reading-rust")}</span>
    </div>
  {:else}
    <div class="jp-target">
      <div>
        <span>{t("inspector-js-element")}</span>
        <strong>[data-anim="{dataAnim}"]</strong>
      </div>
      <small>Anime.js 4.4.1</small>
    </div>
    <MotionStudioPanel {workspace} {dataAnim} />
  {/if}
</div>

<style>
  .js-pane {
    display: flex;
    min-width: 0;
    flex-direction: column;
  }

  .jp-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 7px;
    padding: 24px 14px;
    text-align: center;
    color: var(--text-muted);
    font-size: 11px;
    line-height: 1.45;
  }

  .jp-state strong { color:var(--text); font-size:12px; }
  .jp-state code { color:var(--brand-strong); }
  .jp-state button {
    min-height: 29px;
    padding: 0 10px;
    border: 1px solid color-mix(in srgb, var(--brand) 38%, var(--border-subtle));
    border-radius: var(--radius-control);
    color: var(--brand-strong);
    background: var(--material-control);
    box-shadow: var(--shadow-control);
    font-weight: 800;
    cursor: pointer;
  }
  .jp-error span { color:var(--danger); overflow-wrap:anywhere; }
  .jp-target {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 8px 10px;
    border-bottom: 1px solid var(--border-subtle);
    background: transparent;
  }
  .jp-target > div { min-width:0; }
  .jp-target span { display:block; color:var(--text-muted); font-size:11px; font-weight:650; }
  .jp-target strong {
    display: block;
    overflow: hidden;
    margin-top: 3px;
    padding: 2px 5px;
    border-radius: calc(var(--radius-control) - 3px);
    color: var(--brand-strong);
    background: var(--code-bg);
    text-overflow: ellipsis;
    font: 11px "JetBrains Mono",monospace;
  }
  .jp-target small { flex:0 0 auto; color:var(--text-muted); font-size:11px; }
</style>
