<script lang="ts">
  import { IconAlertTriangle, IconCircleCheck, IconRefresh, IconRestore, IconShieldLock } from "@tabler/icons-svelte";
  import { l10n, t } from "$lib/i18n/runtime.svelte";
  import {
  readWriteAuthorityRecoveryScan,
  resolveWriteAuthorityRecovery,
} from "$lib/kernel/recovery-io";
  import type {
    WriteAuthorityRecoveryClassification,
    WriteAuthorityRecoveryItem,
    WriteAuthorityRecoveryResolutionAction,
    WriteAuthorityRecoveryScan,
  } from "$lib/kernel/recovery-contract";

  let {
    refreshToken = 0,
    onScanUpdate = undefined as ((scan: WriteAuthorityRecoveryScan | null) => void) | undefined,
    onStatusUpdate = undefined as
      | ((text: string, kind: "restored" | "saving" | "error") => void)
      | undefined,
  }: {
    refreshToken?: number;
    onScanUpdate?: (scan: WriteAuthorityRecoveryScan | null) => void;
    onStatusUpdate?: (text: string, kind: "restored" | "saving" | "error") => void;
  } = $props();

  let scan = $state<WriteAuthorityRecoveryScan | null>(null);
  let loading = $state(false);
  let loadError = $state("");
  let resolvingOperationId = $state<string | null>(null);
  let activeRefreshToken = $state<number | null>(null);

  $effect(() => {
    if (refreshToken === activeRefreshToken) return;
    activeRefreshToken = refreshToken;
    void refresh();
  });

  async function refresh() {
    loading = true;
    loadError = "";
    try {
      scan = await readWriteAuthorityRecoveryScan();
      onScanUpdate?.(scan);
    } catch (error) {
      scan = null;
      onScanUpdate?.(null);
      loadError = errorMessage(error);
      onStatusUpdate?.(t("wal-scan-failed", { error: loadError }), "error");
    } finally {
      loading = false;
    }
  }

  function errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }

  function classificationLabel(value: WriteAuthorityRecoveryClassification): string {
    switch (value) {
      case "no_effect": return t("wal-class-no-effect");
      case "staged_only": return t("wal-class-staged-only");
      case "effect_committed": return t("wal-class-effect-committed");
      case "rollback_completed": return t("wal-class-rollback-completed");
      case "cleanup_required": return t("wal-class-cleanup-required");
      case "partial_append": return t("wal-class-partial-append");
      case "partial_namespace_creation": return t("wal-class-partial-namespace-creation");
      case "partial_tree_removal": return t("wal-class-partial-tree-removal");
      case "conflict": return t("wal-class-conflict");
      case "unreadable_or_corrupt": return t("wal-class-unreadable");
    }
  }

  function resolutionLabel(action: WriteAuthorityRecoveryResolutionAction): string {
    const labels: Record<WriteAuthorityRecoveryResolutionAction, string> = {
      discard_staged_write: t("wal-resolution-discard-staged"),
      restore_original: t("wal-resolution-restore-original"),
      accept_restored_state: t("wal-resolution-accept-restored"),
      accept_current_state: t("wal-resolution-accept-current"),
      continue_tree_removal: t("wal-resolution-continue-removal"),
      restore_remaining_tree: t("wal-resolution-restore-remaining"),
    };
    return labels[action];
  }

  async function resolveItem(
    item: WriteAuthorityRecoveryItem,
    action: WriteAuthorityRecoveryResolutionAction,
  ) {
    if (!item.operationId || !item.phase || !item.evidenceHash) return;
    if (
      (action === "discard_staged_write" ||
        action === "accept_current_state" ||
        action === "continue_tree_removal") &&
      !window.confirm(
        action === "discard_staged_write"
          ? t("wal-confirm-discard-staged")
          : action === "accept_current_state"
          ? t("wal-confirm-current")
          : action === "continue_tree_removal"
            ? t("wal-confirm-removal")
            : "",
      )
    ) {
      return;
    }
    resolvingOperationId = item.operationId;
    loadError = "";
    try {
      const receipt = await resolveWriteAuthorityRecovery({
        operationId: item.operationId,
        expectedPhase: item.phase,
        evidenceHash: item.evidenceHash,
        action,
      });
      scan = receipt.recoveryScan;
      onScanUpdate?.(scan);
      onStatusUpdate?.(t("wal-resolution-completed"), "restored");
    } catch (error) {
      loadError = errorMessage(error);
      onStatusUpdate?.(t("wal-resolution-failed", { error: loadError }), "error");
      await refresh();
    } finally {
      resolvingOperationId = null;
    }
  }

  function phaseLabel(phase: WriteAuthorityRecoveryItem["phase"]) {
    switch (phase) {
      case "preparing": return t("wal-phase-preparing");
      case "prepared": return t("wal-phase-prepared");
      case "auxiliary_durable": return t("wal-phase-auxiliary-durable");
      case "effect_visible": return t("wal-phase-effect-visible");
      case "target_durable": return t("wal-phase-target-durable");
      default: return t("wal-phase-unknown");
    }
  }
</script>

<section class="kernel-section wal-section" aria-labelledby="write-authority-wal-title">
  <div class="wal-heading">
    <div class="wal-title">
      <IconShieldLock size={18} stroke={1.9} />
      <div>
        <h2 id="write-authority-wal-title">{t("wal-title")}</h2>
        <p>{t("wal-description")}</p>
      </div>
    </div>
    <button type="button" onclick={() => void refresh()} disabled={loading}>
      <span class:spinning={loading}><IconRefresh size={15} stroke={1.9} /></span>
      <span>{loading ? t("wal-scanning") : t("wal-refresh")}</span>
    </button>
  </div>

  {#if loadError}
    <p class="wal-message error" role="alert">{loadError}</p>
  {:else if scan}
    <div class:blocked={scan.blocked} class="wal-status" role={scan.blocked ? "alert" : "status"}>
      {#if scan.blocked}
        <IconAlertTriangle size={18} stroke={1.9} />
      {:else}
        <IconCircleCheck size={18} stroke={1.9} />
      {/if}
      <div>
        <strong>{scan.blocked ? t("wal-blocked") : t("wal-clean")}</strong>
        <span>{t("wal-summary", {
          records: l10n.formatNumber(scan.recordCount),
          bytes: l10n.formatNumber(scan.totalBytes),
        })}</span>
      </div>
    </div>

    {#if scan.items.length}
      <div class="wal-items">
        {#each scan.items as item (item.fileName)}
          <article>
            <div class="wal-item-title">
              <strong>{item.operationId ?? item.fileName}</strong>
              <span>{classificationLabel(item.classification)}</span>
            </div>
            <p>{item.diagnostic || t("wal-item-diagnostic")}</p>
            <small>
              {t("wal-phase", { phase: phaseLabel(item.phase) })} ·
              {item.automaticRecoveryAvailable ? t("wal-auto-recovery") : t("wal-manual-review")}
            </small>
            {#if item.availableResolutionActions.length}
              <div class="wal-actions">
                {#each item.availableResolutionActions as action}
                  <button
                    type="button"
                    class:danger={action === "continue_tree_removal"}
                    class="resolution-action"
                    disabled={resolvingOperationId !== null}
                    onclick={() => void resolveItem(item, action)}
                  >
                    {#if action === "restore_original" || action === "restore_remaining_tree" || action === "discard_staged_write"}
                      <IconRestore size={14} stroke={1.9} />
                    {:else if action === "continue_tree_removal"}
                      <IconAlertTriangle size={14} stroke={1.9} />
                    {:else}
                      <IconCircleCheck size={14} stroke={1.9} />
                    {/if}
                    <span>{resolvingOperationId === item.operationId ? t("wal-checking") : resolutionLabel(action)}</span>
                  </button>
                {/each}
              </div>
            {/if}
          </article>
        {/each}
      </div>
    {/if}
  {:else}
    <p class="wal-message">{t("wal-reading")}</p>
  {/if}
</section>

<style>
  .wal-section {
    grid-column: 1 / -1;
    display: grid;
    gap: 14px;
  }

  .wal-heading,
  .wal-title,
  .wal-status,
  .wal-item-title {
    display: flex;
    align-items: center;
  }

  .wal-heading {
    justify-content: space-between;
    gap: 16px;
  }

  .wal-title {
    gap: 10px;
    min-width: 0;
  }

  h2,
  p {
    margin: 0;
  }

  h2 {
    font-size: 15px;
  }

  .wal-title p,
  .wal-status span,
  article p,
  article small {
    color: var(--text-muted);
    font-size: 12px;
  }

  button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 32px;
    padding: 0 10px;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--surface);
    color: var(--text);
    cursor: pointer;
  }

  button:disabled {
    cursor: wait;
    opacity: 0.65;
  }

  .wal-status {
    gap: 9px;
    padding: 10px 12px;
    border: 1px solid color-mix(in srgb, var(--success) 36%, var(--border));
    border-radius: 8px;
    background: color-mix(in srgb, var(--success) 8%, var(--surface));
    color: var(--success);
  }

  .wal-status > div {
    display: grid;
    gap: 2px;
  }

  .wal-status.blocked {
    border-color: color-mix(in srgb, var(--danger) 42%, var(--border));
    background: color-mix(in srgb, var(--danger) 8%, var(--surface));
    color: var(--danger);
  }

  .wal-items {
    display: grid;
    gap: 8px;
  }

  article {
    display: grid;
    gap: 6px;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface-2);
  }

  .wal-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    padding-top: 2px;
  }

  .resolution-action {
    min-height: 30px;
    border-color: color-mix(in srgb, var(--brand) 45%, var(--border));
    color: var(--brand);
  }

  .resolution-action.danger {
    border-color: color-mix(in srgb, var(--danger) 55%, var(--border));
    color: var(--danger);
  }

  .wal-item-title {
    justify-content: space-between;
    gap: 10px;
  }

  .wal-item-title span {
    padding: 2px 7px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--danger) 10%, var(--surface));
    color: var(--danger);
    font-size: 12px;
  }

  .wal-message {
    color: var(--text-muted);
    font-size: 12px;
  }

  .wal-message.error {
    color: var(--danger);
  }

  .spinning {
    display: inline-flex;
    animation: spin 0.85s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
