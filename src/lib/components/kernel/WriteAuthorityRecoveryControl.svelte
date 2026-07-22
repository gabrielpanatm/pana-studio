<script lang="ts">
  import { IconAlertTriangle, IconCircleCheck, IconRefresh, IconRestore, IconShieldLock } from "@tabler/icons-svelte";
  import { readWriteAuthorityRecoveryScan, resolveWriteAuthorityRecovery } from "$lib/project/io";
  import type {
    WriteAuthorityRecoveryClassification,
    WriteAuthorityRecoveryItem,
    WriteAuthorityRecoveryResolutionAction,
    WriteAuthorityRecoveryScan,
  } from "$lib/types";

  let {
    refreshToken = 0,
    onStatusUpdate = undefined as
      | ((text: string, kind: "restored" | "saving" | "error") => void)
      | undefined,
  }: {
    refreshToken?: number;
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
    } catch (error) {
      loadError = errorMessage(error);
      onStatusUpdate?.(`WriteAuthority WAL scan eșuat: ${loadError}`, "error");
    } finally {
      loading = false;
    }
  }

  function errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }

  function classificationLabel(value: WriteAuthorityRecoveryClassification): string {
    return value.replaceAll("_", " ");
  }

  function resolutionLabel(action: WriteAuthorityRecoveryResolutionAction): string {
    const labels: Record<WriteAuthorityRecoveryResolutionAction, string> = {
      restore_original: "Restaurează arborele intact",
      accept_restored_state: "Acceptă starea restaurată",
      accept_current_state: "Acceptă starea curentă verificată",
      continue_tree_removal: "Continuă ștergerea arborelui",
      restore_remaining_tree: "Restaurează ce a rămas",
    };
    return labels[action];
  }

  async function resolveItem(
    item: WriteAuthorityRecoveryItem,
    action: WriteAuthorityRecoveryResolutionAction,
  ) {
    if (!item.operationId || !item.phase || !item.evidenceHash) return;
    if (
      (action === "accept_current_state" ||
        action === "continue_tree_removal") &&
      !window.confirm(
        action === "accept_current_state"
          ? "Confirmi acceptarea stării filesystem curente verificate? Nucleul va reverifica tipul, lifetime-ul, starea și contractul specific operației, o va sincroniza fără mutații de conținut și va închide recordul WAL numai dacă tokenul scanării este încă exact."
          : action === "continue_tree_removal"
            ? "Confirmi ștergerea definitivă a tuturor descendenților rămași în quarantine? Acțiunea nu poate fi anulată."
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
      onStatusUpdate?.(receipt.diagnostic, "restored");
    } catch (error) {
      loadError = errorMessage(error);
      onStatusUpdate?.(`Rezoluția WriteAuthority WAL a eșuat: ${loadError}`, "error");
      await refresh();
    } finally {
      resolvingOperationId = null;
    }
  }
</script>

<section class="kernel-section wal-section" aria-labelledby="write-authority-wal-title">
  <div class="wal-heading">
    <div class="wal-title">
      <IconShieldLock size={18} stroke={1.9} />
      <div>
        <h2 id="write-authority-wal-title">Jurnal WAL al autorității de scriere</h2>
        <p>Recuperare globală, înainte de jurnalul tranzacțiilor și înainte de deschiderea proiectului.</p>
      </div>
    </div>
    <button type="button" onclick={() => void refresh()} disabled={loading}>
      <span class:spinning={loading}><IconRefresh size={15} stroke={1.9} /></span>
      <span>{loading ? "Scaneză..." : "Recitește"}</span>
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
        <strong>{scan.blocked ? "Mutațiile sunt blocate" : "WAL curat"}</strong>
        <span>{scan.recordCount} recorduri · {scan.totalBytes} bytes</span>
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
            <p>{item.diagnostic}</p>
            <small>
              fază {item.phase ?? "necunoscută"} ·
              {item.automaticRecoveryAvailable ? "recuperare automată disponibilă" : "revizuire manuală"}
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
                    {#if action === "restore_original" || action === "restore_remaining_tree"}
                      <IconRestore size={14} stroke={1.9} />
                    {:else if action === "continue_tree_removal"}
                      <IconAlertTriangle size={14} stroke={1.9} />
                    {:else}
                      <IconCircleCheck size={14} stroke={1.9} />
                    {/if}
                    <span>{resolvingOperationId === item.operationId ? "Se verifică..." : resolutionLabel(action)}</span>
                  </button>
                {/each}
              </div>
            {/if}
          </article>
        {/each}
      </div>
    {/if}
  {:else}
    <p class="wal-message">Se citește scanarea WAL...</p>
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
