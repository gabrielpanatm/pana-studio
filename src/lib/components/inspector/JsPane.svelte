<script lang="ts">
  import type { InspectorSelectionSummarySnapshot } from "$lib/types";
  import type { MotionWorkspaceState } from "$lib/state/motion-workspace.svelte";
  import MotionStudioPanel from "$lib/components/inspector/js/MotionStudioPanel.svelte";
  import InspectorEmptyState from "$lib/components/inspector/InspectorEmptyState.svelte";
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
    <InspectorEmptyState kind="js" title="Motion" description={t("inspector-js-select-element")} />
  {:else if !dataAnim}
    <InspectorEmptyState
      kind="js"
      title={t("inspector-js-no-motion-identity")}
      description={t("inspector-js-add-data-anim-before")}
      codeToken="data-anim"
      descriptionAfter={t("inspector-js-add-data-anim-after")}
      actionLabel={onSwitchToHtml ? t("inspector-js-go-html") : ""}
      onAction={onSwitchToHtml}
    />
  {:else if !templatePath}
    <InspectorEmptyState kind="js" tone="danger" title={t("inspector-js-unavailable")} description={t("inspector-js-no-active-template")} />
  {:else if workspace.loadState === "error"}
    <InspectorEmptyState
      kind="js"
      tone="danger"
      title={t("inspector-js-motion-load-failed")}
      description={workspace.error}
      actionLabel={t("inspector-js-retry")}
      onAction={() => { void workspace.reload(); }}
    />
  {:else if workspace.loadState !== "ready"}
    <InspectorEmptyState kind="js" loading title="Motion v2" description={t("inspector-js-reading-rust")} />
  {:else}
    <div class="jp-target">
      <div>
        <span>{t("inspector-js-element")}</span>
        <strong>[data-anim="{dataAnim}"]</strong>
      </div>
      <small>Anime.js {workspace.runtimeContract?.animeVersion ?? "—"}</small>
    </div>
    <MotionStudioPanel {workspace} {dataAnim} />
  {/if}
</div>

<style>
  .js-pane {
    display: flex;
    min-width: 0;
    flex: 1 1 auto;
    flex-direction: column;
  }
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
