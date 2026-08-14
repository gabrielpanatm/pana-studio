<script lang="ts">
  import {
    IconAlertTriangle,
    IconBrowser,
    IconCircleCheck,
    IconClock,
    IconDatabase,
    IconFileText,
    IconFolder,
    IconRefresh,
    IconShieldLock,
    IconTrash,
  } from "@tabler/icons-svelte";
  import { onMount } from "svelte";
  import {
    clearApplicationCacheStorage,
    clearApplicationLogStorage,
    deleteApplicationSessionStorage,
    readApplicationStorageInventory,
  } from "$lib/application/storage";
  import { l10n, t } from "$lib/i18n/runtime.svelte";
  import type { AppState } from "$lib/state/app.svelte";
  import type {
    ApplicationStorageSnapshot,
    StorageCleanupReceipt,
    StorageSessionSnapshot,
  } from "$lib/types";

  type Confirmation = "cache" | "logs" | "sessions" | null;

  let { app }: { app: AppState } = $props();

  let snapshot = $state<ApplicationStorageSnapshot | null>(null);
  let loading = $state(false);
  let busy = $state<Exclude<Confirmation, null> | null>(null);
  let error = $state("");
  let lastReceipt = $state<StorageCleanupReceipt | null>(null);
  let confirmation = $state<Confirmation>(null);
  let selectedSessionIds = $state<string[]>([]);
  let initializedSelection = $state(false);
  let baselineDefaultSelectedIds = $state<string[]>([]);

  const selectedSessions = $derived.by(() => {
    if (!snapshot) return [];
    const selected = new Set(selectedSessionIds);
    return snapshot.sessions.items.filter((item) => selected.has(item.id));
  });
  const selectedBytes = $derived(
    selectedSessions.reduce((total, item) => total + item.bytes, 0),
  );
  const selectedRecoveryCount = $derived(
    selectedSessions.filter((item) => item.hasRecovery).length,
  );

  onMount(() => {
    void refresh();
  });

  async function refresh() {
    loading = true;
    error = "";
    lastReceipt = null;
    confirmation = null;
    try {
      applySnapshot(await readApplicationStorageInventory());
    } catch (cause) {
      error = errorMessage(cause);
      app.setGlobalStatus(t("settings-storage-read-failed", { error }), "error");
    } finally {
      loading = false;
    }
  }

  function applySnapshot(next: ApplicationStorageSnapshot, resetSelection = false) {
    snapshot = next;
    const available = new Set(next.sessions.items.filter((item) => item.deletable).map((item) => item.id));
    if (!initializedSelection || resetSelection) {
      const defaultSelected = next.sessions.items
        .filter((item) => item.defaultSelected)
        .map((item) => item.id);
      baselineDefaultSelectedIds = defaultSelected;
      selectedSessionIds = [...defaultSelected];
      initializedSelection = true;
      return;
    }
    selectedSessionIds = selectedSessionIds.filter((id) => available.has(id));
  }

  function toggleSession(item: StorageSessionSnapshot, checked: boolean) {
    if (!item.deletable) return;
    const selected = new Set(selectedSessionIds);
    if (checked) selected.add(item.id);
    else selected.delete(item.id);
    selectedSessionIds = [...selected];
    confirmation = null;
  }

  function selectSafeOrphans() {
    if (!snapshot) return;
    selectedSessionIds = snapshot.sessions.items
      .filter((item) => item.defaultSelected)
      .map((item) => item.id);
    confirmation = null;
  }

  function selectAllDeletableSessions() {
    if (!snapshot) return;
    selectedSessionIds = snapshot.sessions.items
      .filter((item) => item.deletable)
      .map((item) => item.id);
    confirmation = null;
  }

  function clearSessionSelection() {
    selectedSessionIds = [];
    confirmation = null;
  }

  function selectionChangedFromDefault() {
    if (selectedSessionIds.length !== baselineDefaultSelectedIds.length) return true;
    const baseline = new Set(baselineDefaultSelectedIds);
    return selectedSessionIds.some((id) => !baseline.has(id));
  }

  async function clearCache() {
    await runCleanup("cache", clearApplicationCacheStorage);
  }

  async function clearLogs() {
    await runCleanup("logs", clearApplicationLogStorage);
  }

  async function deleteSessions() {
    if (!snapshot || selectedSessions.length === 0) return;
    const selectionWasCustomized = selectionChangedFromDefault();
    busy = "sessions";
    error = "";
    lastReceipt = null;
    try {
      const receipt = await deleteApplicationSessionStorage({
        expectedRevision: snapshot.sessions.revision,
        sessionIds: selectedSessions.map((item) => item.id),
        confirmedRecoverySessionIds: selectedSessions
          .filter((item) => item.hasRecovery)
          .map((item) => item.id),
      });
      applyReceipt(receipt, !selectionWasCustomized);
    } catch (cause) {
      handleCleanupError(cause);
      await refreshAfterFailure();
    } finally {
      busy = null;
      confirmation = null;
    }
  }

  async function runCleanup(
    operation: "cache" | "logs",
    action: () => Promise<StorageCleanupReceipt>,
  ) {
    busy = operation;
    error = "";
    lastReceipt = null;
    try {
      applyReceipt(await action(), false);
    } catch (cause) {
      handleCleanupError(cause);
      await refreshAfterFailure();
    } finally {
      busy = null;
      confirmation = null;
    }
  }

  function applyReceipt(receipt: StorageCleanupReceipt, resetSelection: boolean) {
    lastReceipt = receipt;
    applySnapshot(receipt.snapshot, resetSelection);
    if (receipt.failures.length > 0) {
      error = receipt.failures.join("\n");
      app.setGlobalStatus(
        t("settings-storage-cleanup-partial", { bytes: formatBytes(receipt.freedBytes) }),
        "error",
      );
      return;
    }
    app.setGlobalStatus(
      t("settings-storage-cleanup-success", {
        bytes: formatBytes(receipt.freedBytes),
        count: receipt.removedItems,
      }),
      "restored",
    );
  }

  function handleCleanupError(cause: unknown) {
    error = errorMessage(cause);
    app.setGlobalStatus(t("settings-storage-cleanup-failed", { error }), "error");
  }

  async function refreshAfterFailure() {
    try {
      applySnapshot(await readApplicationStorageInventory());
    } catch {
      // Păstrează diagnosticul mutației; următorul Refresh reia inventarul.
    }
  }

  function errorMessage(cause: unknown) {
    return cause instanceof Error ? cause.message : String(cause);
  }

  function formatBytes(bytes: number) {
    if (bytes < 1_024) return `${l10n.formatNumber(bytes)} B`;
    if (bytes < 1_048_576) {
      return `${l10n.formatNumber(bytes / 1_024, { maximumFractionDigits: 1 })} KB`;
    }
    if (bytes < 1_073_741_824) {
      return `${l10n.formatNumber(bytes / 1_048_576, { maximumFractionDigits: 1 })} MB`;
    }
    return `${l10n.formatNumber(bytes / 1_073_741_824, { maximumFractionDigits: 2 })} GB`;
  }

  function formatLastSeen(timestamp: number) {
    if (!timestamp) return t("common-unknown");
    return l10n.formatDate(timestamp, {
      dateStyle: "medium",
      timeStyle: "short",
    });
  }
</script>

<div class="storage-pane">
  <section class="storage-introduction" aria-labelledby="storage-title">
    <div>
      <h2 id="storage-title">{t("settings-storage-title")}</h2>
      <p>{t("settings-storage-description")}</p>
    </div>
    <button
      type="button"
      class="icon-action"
      title={t("settings-storage-refresh")}
      aria-label={t("settings-storage-refresh")}
      disabled={loading || busy !== null}
      onclick={() => void refresh()}
    >
      <span class:spinning={loading}><IconRefresh size={15} stroke={1.9} /></span>
    </button>
  </section>

  {#if error}
    <pre class="storage-error" role="alert">{error}</pre>
  {/if}

  {#if lastReceipt && lastReceipt.failures.length === 0}
    <div class="storage-success" role="status">
      <IconCircleCheck size={17} stroke={1.9} />
      <p>
        <strong>{t("settings-storage-receipt-title")}</strong>
        <span>{t("settings-storage-cleanup-success", {
          bytes: formatBytes(lastReceipt.freedBytes),
          count: lastReceipt.removedItems,
        })}</span>
        {#if lastReceipt.protectedBytes > 0}
          <small>{t("settings-storage-receipt-protected", { bytes: formatBytes(lastReceipt.protectedBytes) })}</small>
        {/if}
      </p>
    </div>
  {/if}

  {#if loading && !snapshot}
    <div class="storage-loading"><IconDatabase size={20} stroke={1.8} /> {t("settings-storage-loading")}</div>
  {:else if snapshot}
    <section class="storage-summary" aria-label={t("settings-storage-summary-label")}>
      <div>
        <span>{t("settings-storage-total")}</span>
        <strong>{formatBytes(snapshot.totalBytes)}</strong>
      </div>
      <div class="reclaimable">
        <span>{t("settings-storage-reclaimable")}</span>
        <strong>{formatBytes(snapshot.reclaimableBytes)}</strong>
      </div>
      <div>
        <span>{t("settings-storage-sessions-protected")}</span>
        <strong>{l10n.formatNumber(snapshot.sessions.recoveryCount + snapshot.sessions.activeCount)}</strong>
      </div>
      <small>{t("settings-storage-last-scan", { date: formatLastSeen(snapshot.scannedAtMs) })}</small>
    </section>

    <section class="storage-card" aria-labelledby="storage-cache-title">
      <div class="card-heading">
        <div class="heading-copy">
          <span class="card-icon"><IconBrowser size={18} stroke={1.8} /></span>
          <div>
            <h3 id="storage-cache-title">{t("settings-storage-cache-title")}</h3>
            <p>{t("settings-storage-cache-description")}</p>
          </div>
        </div>
        <strong class="area-total">{formatBytes(snapshot.cache.totalBytes)}</strong>
      </div>
      <div class="area-list">
        <div>
          <span><strong>{t("settings-storage-webkit")}</strong><small>{snapshot.cache.webkit.path}</small></span>
          <b>{formatBytes(snapshot.cache.webkit.bytes)}</b>
        </div>
        <div>
          <span><strong>{t("settings-storage-preview")}</strong><small>{snapshot.cache.preview.path}</small></span>
          <b>{formatBytes(snapshot.cache.preview.bytes)}</b>
        </div>
      </div>
      {#if snapshot.cache.protectedPreviewBytes > 0}
        <p class="protected-note"><IconShieldLock size={14} stroke={1.8} /> {t("settings-storage-preview-protected", { bytes: formatBytes(snapshot.cache.protectedPreviewBytes) })}</p>
      {/if}
      <div class="card-actions">
        <span>{t("settings-storage-cache-reclaimable", { bytes: formatBytes(snapshot.cache.reclaimableBytes) })}</span>
        <button
          type="button"
          class="ui-button danger"
          disabled={busy !== null || snapshot.cache.reclaimableBytes === 0}
          onclick={() => { confirmation = "cache"; }}
        ><IconTrash size={14} stroke={1.9} /> {t("settings-storage-clear-cache")}</button>
      </div>
      {#if confirmation === "cache"}
        <div class="confirmation" role="alert">
          <IconAlertTriangle size={17} stroke={1.9} />
          <p><strong>{t("settings-storage-confirm-cache-title")}</strong><span>{t("settings-storage-confirm-cache-description")}</span></p>
          <button type="button" disabled={busy !== null} onclick={() => { confirmation = null; }}>{t("settings-storage-cancel")}</button>
          <button type="button" class="danger-confirm" disabled={busy !== null} onclick={() => void clearCache()}>{busy === "cache" ? t("settings-storage-cleaning") : t("settings-storage-confirm")}</button>
        </div>
      {/if}
    </section>

    <section class="storage-card" aria-labelledby="storage-logs-title">
      <div class="card-heading">
        <div class="heading-copy">
          <span class="card-icon"><IconFileText size={18} stroke={1.8} /></span>
          <div>
            <h3 id="storage-logs-title">{t("settings-storage-logs-title")}</h3>
            <p>{t("settings-storage-logs-description", { count: snapshot.logs.archiveCount })}</p>
          </div>
        </div>
        <strong class="area-total">{formatBytes(snapshot.logs.area.bytes)}</strong>
      </div>
      <div class="area-list compact">
        <div>
          <span><strong>{t("settings-storage-active-log")}</strong><small>{snapshot.logs.area.path}</small></span>
          <b>{formatBytes(snapshot.logs.activeBytes)}</b>
        </div>
      </div>
      <div class="card-actions">
        <span>{t("settings-storage-logs-note")}</span>
        <button
          type="button"
          class="ui-button danger"
          disabled={busy !== null || snapshot.logs.area.bytes === 0}
          onclick={() => { confirmation = "logs"; }}
        ><IconTrash size={14} stroke={1.9} /> {t("settings-storage-clear-logs")}</button>
      </div>
      {#if confirmation === "logs"}
        <div class="confirmation" role="alert">
          <IconAlertTriangle size={17} stroke={1.9} />
          <p><strong>{t("settings-storage-confirm-logs-title")}</strong><span>{t("settings-storage-confirm-logs-description")}</span></p>
          <button type="button" disabled={busy !== null} onclick={() => { confirmation = null; }}>{t("settings-storage-cancel")}</button>
          <button type="button" class="danger-confirm" disabled={busy !== null} onclick={() => void clearLogs()}>{busy === "logs" ? t("settings-storage-cleaning") : t("settings-storage-confirm")}</button>
        </div>
      {/if}
    </section>

    <section class="storage-card sessions-card" aria-labelledby="storage-sessions-title">
      <div class="card-heading">
        <div class="heading-copy">
          <span class="card-icon"><IconFolder size={18} stroke={1.8} /></span>
          <div>
            <h3 id="storage-sessions-title">{t("settings-storage-sessions-title")}</h3>
            <p>{t("settings-storage-sessions-description")}</p>
          </div>
        </div>
        <strong class="area-total">{formatBytes(snapshot.sessions.totalBytes)}</strong>
      </div>
      <div class="session-stats">
        <span>{t("settings-storage-session-count", { count: snapshot.sessions.count })}</span>
        <span>{t("settings-storage-orphan-count", { count: snapshot.sessions.orphanCount })}</span>
        <span>{t("settings-storage-recovery-count", { count: snapshot.sessions.recoveryCount })}</span>
      </div>
      <div class="selection-toolbar">
        <span>{t("settings-storage-selected", { count: selectedSessions.length, bytes: formatBytes(selectedBytes) })}</span>
        <button type="button" disabled={busy !== null} onclick={selectSafeOrphans}>{t("settings-storage-select-safe")}</button>
        <button type="button" disabled={busy !== null} onclick={selectAllDeletableSessions}>{t("settings-storage-select-all")}</button>
        <button type="button" disabled={busy !== null || selectedSessions.length === 0} onclick={clearSessionSelection}>{t("settings-storage-clear-selection")}</button>
      </div>

      {#if snapshot.sessions.items.length === 0}
        <p class="empty-state"><IconCircleCheck size={17} stroke={1.8} /> {t("settings-storage-no-sessions")}</p>
      {:else}
        <div class="session-list">
          {#each snapshot.sessions.items as item (item.id)}
            <label class:active={item.active} class:recovery={item.hasRecovery}>
              <input
                type="checkbox"
                checked={selectedSessionIds.includes(item.id)}
                disabled={!item.deletable || busy !== null}
                onchange={(event) => toggleSession(item, event.currentTarget.checked)}
              />
              <span class="session-main">
                <strong>{item.projectName}</strong>
                <code title={item.projectRoot || item.id}>{item.projectRoot || item.id}</code>
                <small><IconClock size={12} stroke={1.8} /> {formatLastSeen(item.lastSeenAtMs)}</small>
              </span>
              <span class="session-state">
                {#if item.active}<em class="active-badge">{t("settings-storage-session-active")}</em>{/if}
                {#if item.hasRecovery}<em class="recovery-badge" title={item.recoverySignals.join("\n")}>{t("settings-storage-session-recovery")}</em>{/if}
                {#if !item.projectExists}<em>{t("settings-storage-session-missing")}</em>{/if}
                <b>{formatBytes(item.bytes)}</b>
              </span>
            </label>
          {/each}
        </div>
      {/if}

      <div class="card-actions">
        <span>{t("settings-storage-session-safety")}</span>
        <button
          type="button"
          class="ui-button danger"
          disabled={busy !== null || selectedSessions.length === 0}
          onclick={() => { confirmation = "sessions"; }}
        ><IconTrash size={14} stroke={1.9} /> {t("settings-storage-delete-sessions")}</button>
      </div>
      {#if confirmation === "sessions"}
        <div class="confirmation" class:critical={selectedRecoveryCount > 0} role="alert">
          <IconAlertTriangle size={17} stroke={1.9} />
          <p>
            <strong>{t("settings-storage-confirm-sessions-title", { count: selectedSessions.length })}</strong>
            <span>{selectedRecoveryCount > 0
              ? t("settings-storage-confirm-recovery-description", { count: selectedRecoveryCount })
              : t("settings-storage-confirm-sessions-description", { bytes: formatBytes(selectedBytes) })}</span>
          </p>
          <button type="button" disabled={busy !== null} onclick={() => { confirmation = null; }}>{t("settings-storage-cancel")}</button>
          <button type="button" class="danger-confirm" disabled={busy !== null} onclick={() => void deleteSessions()}>{busy === "sessions" ? t("settings-storage-cleaning") : t("settings-storage-confirm-delete")}</button>
        </div>
      {/if}
    </section>

    <p class="storage-boundary"><IconShieldLock size={15} stroke={1.8} /> {t("settings-storage-boundary")}</p>
  {/if}
</div>

<style>
  .storage-pane { display: grid; gap: 14px; width: min(100%, 980px); margin: 0 auto; }
  .storage-introduction, .storage-card, .storage-summary { border: 1px solid var(--wb-border-subtle, var(--border)); border-radius: 10px; background: var(--wb-surface-chrome, var(--surface-2)); }
  .storage-introduction { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; padding: 0 0 4px; border: 0; background: transparent; }
  h2, h3, p, pre { margin: 0; }
  h2 { font-size: 14px; font-weight: 850; }
  h3 { font-size: 13px; font-weight: 850; }
  .storage-introduction p, .card-heading p { margin-top: 3px; color: var(--wb-text-muted, var(--text-muted)); font-size: 12px; line-height: 1.45; }
  .icon-action { display: grid; width: 30px; height: 30px; place-items: center; border: 1px solid var(--border); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--control-bg, var(--surface)); }
  .spinning { display: inline-flex; animation: spin .8s linear infinite; }
  .storage-error { padding: 10px 12px; overflow: auto; border: 1px solid color-mix(in srgb, var(--danger) 35%, var(--border)); border-radius: 8px; color: var(--danger); background: color-mix(in srgb, var(--danger) 7%, var(--surface)); font: inherit; font-size: 11px; white-space: pre-wrap; }
  .storage-success { display: flex; align-items: flex-start; gap: 9px; padding: 10px 12px; border: 1px solid color-mix(in srgb, var(--brand-strong) 35%, var(--border)); border-radius: 8px; color: var(--brand-strong); background: color-mix(in srgb, var(--brand-strong) 7%, var(--surface)); }
  .storage-success p { display: grid; gap: 2px; }
  .storage-success strong, .storage-success span, .storage-success small { font-size: 11px; }
  .storage-success span { color: var(--wb-text-primary); }
  .storage-success small { color: var(--wb-text-muted); }
  .storage-loading { display: flex; align-items: center; justify-content: center; gap: 9px; min-height: 160px; color: var(--wb-text-muted); font-size: 12px; }
  .storage-summary { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 1px; overflow: hidden; background: var(--border); }
  .storage-summary > div { display: grid; gap: 4px; padding: 14px 16px; background: var(--wb-surface-chrome, var(--surface-2)); }
  .storage-summary span { color: var(--wb-text-muted); font-size: 11px; font-weight: 800; letter-spacing: .04em; text-transform: uppercase; }
  .storage-summary strong { font-size: 20px; letter-spacing: -.025em; }
  .storage-summary .reclaimable strong { color: var(--brand-strong); }
  .storage-summary > small { grid-column: 1 / -1; padding: 7px 12px; color: var(--wb-text-muted); background: var(--wb-surface-chrome, var(--surface-2)); font-size: 11px; text-align: right; }
  .storage-card { display: grid; gap: 13px; padding: 15px; }
  .card-heading { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; }
  .heading-copy { display: flex; align-items: flex-start; gap: 10px; min-width: 0; }
  .card-icon { display: grid; flex: 0 0 32px; width: 32px; height: 32px; place-items: center; border: 1px solid var(--border); border-radius: 8px; color: var(--brand-strong); background: var(--control-selected); }
  .area-total { flex: 0 0 auto; font-size: 14px; }
  .area-list { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
  .area-list.compact { grid-template-columns: 1fr; }
  .area-list > div { display: flex; align-items: center; justify-content: space-between; gap: 12px; min-width: 0; padding: 9px 10px; border: 1px solid var(--border); border-radius: 8px; background: var(--surface); }
  .area-list span { display: grid; min-width: 0; }
  .area-list strong { font-size: 11px; }
  .area-list small { overflow: hidden; margin-top: 2px; color: var(--wb-text-muted); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .area-list b { flex: 0 0 auto; font-size: 11px; }
  .protected-note, .storage-boundary { display: flex; align-items: center; gap: 7px; color: var(--wb-text-muted); font-size: 11px; }
  .card-actions { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding-top: 2px; }
  .card-actions > span { color: var(--wb-text-muted); font-size: 11px; }
  .card-actions button { display: inline-flex; align-items: center; gap: 6px; }
  .confirmation { display: grid; grid-template-columns: auto minmax(0, 1fr) auto auto; align-items: center; gap: 9px; padding: 10px; border: 1px solid color-mix(in srgb, var(--danger) 32%, var(--border)); border-radius: 8px; color: var(--danger); background: color-mix(in srgb, var(--danger) 7%, var(--surface)); }
  .confirmation p { display: grid; gap: 2px; font-size: 11px; }
  .confirmation p span { color: var(--wb-text-primary); font-size: 11px; }
  .confirmation button { min-height: 28px; padding: 0 9px; border: 1px solid var(--border); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--control-bg); font: inherit; font-size: 11px; font-weight: 750; }
  .confirmation .danger-confirm { border-color: color-mix(in srgb, var(--danger) 55%, var(--border)); color: white; background: var(--danger); }
  .session-stats { display: flex; flex-wrap: wrap; gap: 7px; }
  .session-stats span { padding: 4px 7px; border-radius: 999px; color: var(--wb-text-muted); background: var(--surface); font-size: 11px; font-weight: 700; }
  .selection-toolbar { display: flex; align-items: center; gap: 7px; }
  .selection-toolbar span { margin-right: auto; color: var(--wb-text-muted); font-size: 11px; font-weight: 700; }
  .selection-toolbar button { min-height: 26px; padding: 0 8px; border: 1px solid var(--border); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--control-bg); font: inherit; font-size: 11px; }
  .session-list { display: grid; gap: 6px; max-height: 330px; overflow: auto; }
  .session-list label { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: center; gap: 10px; min-width: 0; padding: 9px 10px; border: 1px solid var(--border); border-radius: 8px; background: var(--surface); }
  .session-list label.active { opacity: .72; }
  .session-list label.recovery { border-color: color-mix(in srgb, var(--warning, #c88719) 35%, var(--border)); }
  .session-main { display: grid; min-width: 0; gap: 2px; }
  .session-main strong { font-size: 11px; }
  .session-main code { overflow: hidden; color: var(--wb-text-muted); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .session-main small { display: flex; align-items: center; gap: 4px; color: var(--wb-text-muted); font-size: 11px; }
  .session-state { display: flex; align-items: center; justify-content: flex-end; gap: 5px; }
  .session-state em { padding: 3px 6px; border-radius: 999px; color: var(--wb-text-muted); background: var(--surface-2); font-size: 11px; font-style: normal; font-weight: 750; }
  .session-state .active-badge { color: var(--brand-strong); background: var(--control-selected); }
  .session-state .recovery-badge { color: var(--warning-strong, #946000); background: color-mix(in srgb, var(--warning, #c88719) 13%, var(--surface)); }
  .session-state b { min-width: 56px; font-size: 11px; text-align: right; }
  .empty-state { display: flex; align-items: center; gap: 7px; padding: 12px; color: var(--brand-strong); font-size: 11px; }
  .storage-boundary { justify-content: center; padding: 2px 12px; text-align: center; }
  button:disabled, input:disabled { cursor: not-allowed; opacity: .55; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (max-width: 760px) {
    .storage-summary, .area-list { grid-template-columns: 1fr; }
    .storage-summary > small { grid-column: auto; }
    .confirmation { grid-template-columns: auto 1fr; }
    .confirmation button { grid-column: span 1; }
    .session-list label { grid-template-columns: auto minmax(0, 1fr); }
    .session-state { grid-column: 2; justify-content: flex-start; flex-wrap: wrap; }
  }
</style>
