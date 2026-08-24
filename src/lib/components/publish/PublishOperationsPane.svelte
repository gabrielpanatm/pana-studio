<script lang="ts">
  import { IconHammer, IconX } from "@tabler/icons-svelte";
  import DeployTargetsPanel from "$lib/components/deploy/DeployTargetsPanel.svelte";
  import { cancelPublishOperation } from "$lib/deploy/io";
  import type { PublishBuildReceipt, PublishPreflightReceipt } from "$lib/deploy/contracts";
  import { t } from "$lib/i18n/runtime.svelte";
  import { errorMessage } from "$lib/util";

  let {
    mode,
    scannedProject,
    projectRoot,
    runtimeSessionId,
    publishPreflight,
    publishBuild,
    invalidatePublishAuthorization,
    buildForPublish,
    onStatusUpdate,
  }: {
    mode: "release" | "configuration";
    scannedProject: boolean;
    projectRoot: string;
    runtimeSessionId: string;
    publishPreflight: PublishPreflightReceipt | null;
    publishBuild: PublishBuildReceipt | null;
    invalidatePublishAuthorization: () => void;
    buildForPublish: () => Promise<PublishBuildReceipt>;
    onStatusUpdate: (text: string, kind: string) => void;
  } = $props();

  let buildRunning = $state(false);
  let deployRunning = $state(false);
  let cancelRunning = $state(false);
  let actionLog = $state("");
  let actionOk = $state<boolean | null>(null);
  const publishReady = $derived(publishPreflight?.status === "ready");

  async function runBuild() {
    if (buildRunning || !publishReady) return;
    buildRunning = true;
    actionLog = "";
    actionOk = null;
    onStatusUpdate(t("deploy-build-running-status"), "saving");
    try {
      const receipt = await buildForPublish();
      actionLog = receipt.log || t("publish-build-receipt-summary", {
        files: receipt.artifactFiles,
        bytes: receipt.artifactBytes,
      });
      actionOk = true;
      onStatusUpdate(t("deploy-build-complete"), "saved");
    } catch (error) {
      actionLog = errorMessage(error);
      actionOk = false;
      onStatusUpdate(t("deploy-build-error", { error: actionLog }), "error");
    } finally {
      buildRunning = false;
    }
  }

  async function cancelRunningOperation() {
    if (cancelRunning || !buildRunning || !projectRoot || !runtimeSessionId) return;
    cancelRunning = true;
    try {
      const receipt = await cancelPublishOperation({
        expectedProjectRoot: projectRoot,
        expectedSessionId: runtimeSessionId,
      });
      actionLog = t("deploy-cancel-log", { kind: receipt.kind, operation: receipt.operationId });
      actionOk = null;
      onStatusUpdate(t("deploy-cancel-requested"), "saving");
    } catch (error) {
      onStatusUpdate(t("deploy-cancel-failed", { error: errorMessage(error) }), "error");
    } finally {
      cancelRunning = false;
    }
  }
</script>

<div class="publish-operations-pane">
  {#if !scannedProject}
    <p class="hint">{t("deploy-open-folder")}</p>
  {:else}
    {#if mode === "release"}
      <div class="build-actions">
        <button class="ui-button primary" type="button" onclick={() => { void runBuild(); }} disabled={buildRunning || deployRunning || !publishReady} title={!publishReady ? t("publish-build-requires-preflight") : t("deploy-build-title")}>
          <IconHammer size={14} stroke={1.8} />
          {buildRunning ? t("deploy-building") : t("deploy-build")}
        </button>
        {#if buildRunning}
          <button class="ui-button danger" type="button" onclick={() => { void cancelRunningOperation(); }} disabled={cancelRunning}>
            <IconX size={14} stroke={2} /> {cancelRunning ? t("deploy-cancelling") : t("deploy-cancel")}
          </button>
        {/if}
      </div>
      {#if actionLog}
        <div class="log-box" class:log-ok={actionOk === true} class:log-err={actionOk === false} aria-live="polite">
          <pre>{actionLog}</pre>
        </div>
      {/if}
    {/if}

    <DeployTargetsPanel
      {publishPreflight}
      {publishBuild}
      {invalidatePublishAuthorization}
      {scannedProject}
      {mode}
      {projectRoot}
      {runtimeSessionId}
      disabled={buildRunning}
      {onStatusUpdate}
      onRunningChange={(running) => { deployRunning = running; }}
    />
  {/if}
</div>

<style>
  .publish-operations-pane { display: grid; gap: 10px; min-width: 0; }
  .build-actions { display: flex; gap: 7px; }
  .build-actions > button:first-child { flex: 1; }
  .hint { margin: 0; color: var(--wb-text-muted); font-size: 12px; }
  .log-box { overflow: hidden; max-height: 240px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); }
  .log-box.log-ok { border-color: color-mix(in srgb, var(--success) 42%, var(--wb-border-subtle)); }
  .log-box.log-err { border-color: color-mix(in srgb, var(--danger) 42%, var(--wb-border-subtle)); }
  pre { overflow: auto; max-height: 220px; margin: 0; padding: 8px 10px; color: var(--wb-text-muted); background: var(--material-control); font: 11px/1.55 var(--font-mono); white-space: pre-wrap; overflow-wrap: anywhere; }
</style>
