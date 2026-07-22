<script lang="ts">
  import { IconArrowBackUp, IconArrowForwardUp, IconRotateClockwise, IconX } from "@tabler/icons-svelte";
  import { formatBytes } from "$lib/kernel/recovery-control";
  import type { ProjectWorkspaceSnapshot, WorkspaceHistoryEntrySnapshot } from "$lib/types";

  let {
    open = false,
    workspace = null,
    undoAction,
    redoAction,
    discardSession,
    close,
  }: {
    open?: boolean;
    workspace?: ProjectWorkspaceSnapshot | null;
    undoAction: () => void | Promise<void>;
    redoAction: () => void | Promise<void>;
    discardSession: () => void | Promise<void>;
    close: () => void;
  } = $props();

  const history = $derived(workspace?.history ?? null);

  function entryPath(entry: WorkspaceHistoryEntrySnapshot) {
    const paths = [...entry.documentPaths, ...entry.pageJsPaths];
    if (paths.length === 0) return "workspace";
    if (paths.length === 1) return paths[0];
    return `${paths[0]} +${paths.length - 1}`;
  }

  function formatTime(timestamp: number) {
    return new Intl.DateTimeFormat("ro-RO", {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    }).format(new Date(timestamp));
  }
</script>

{#if open}
  <div class="history-backdrop" role="presentation" onclick={close}></div>
  <aside class="history-panel" aria-label="Istoricul sesiunii proiectului">
    <header class="history-header">
      <div>
        <p class="eyebrow">Sesiunea proiectului</p>
        <h2>Istoric autoritativ</h2>
      </div>
      <button type="button" class="icon-button" title="Închide" onclick={close}>
        <IconX size={16} stroke={1.9} />
      </button>
    </header>

    {#if workspace && history}
      <section class="workspace-runtime" aria-label="Starea sesiunii proiectului">
        <span>Revizie <b>{workspace.revision}</b></span>
        <span>Disc <b>{workspace.diskGeneration}</b></span>
        <span>Documente modificate <b>{workspace.dirtyDocumentCount}</b></span>
        <span>JavaScript modificat <b>{workspace.dirtyPageJsCount}</b></span>
        <span>Anulări <b>{history.undoCount}</b></span>
        <span>Refaceri <b>{history.redoCount}</b></span>
        <span class="memory">Memorie <b>{formatBytes(history.retainedBytes)} / {formatBytes(history.retainedBytesLimit)}</b></span>
      </section>

      <div class="history-actions">
        <button type="button" disabled={!history.canUndo} onclick={undoAction}>
          <IconArrowBackUp size={16} stroke={1.9} />
          Anulează
        </button>
        <button type="button" disabled={!history.canRedo} onclick={redoAction}>
          <IconArrowForwardUp size={16} stroke={1.9} />
          Refă
        </button>
      </div>

      <button type="button" class="discard-session-btn" disabled={!workspace.dirty} onclick={discardSession}>
        <IconRotateClockwise size={16} stroke={1.9} />
        <span>Abandonează sesiunea și reîncarcă de pe disc</span>
      </button>

      {#if history.undoEntries.length === 0 && history.redoEntries.length === 0}
        <p class="empty-text">Nu există încă mutații reversibile în sesiunea curentă.</p>
      {:else}
        <div class="snapshot-list">
          {#if history.undoEntries.length > 0}
            <p class="section-label">De anulat — următoarea acțiune este prima</p>
            {#each history.undoEntries as entry, index}
              <article class="snapshot-item" class:next={index === 0}>
                <div class="snapshot-main">
                  <strong>{entry.label}</strong>
                  <span>{entryPath(entry)}</span>
                  <small>{entry.source} · {entry.mutationCount} mutație(i) · {formatTime(entry.updatedAtMs)}</small>
                </div>
                <code>{entry.transactionId}</code>
              </article>
            {/each}
          {/if}

          {#if history.redoEntries.length > 0}
            <p class="section-label redo">De refăcut — următoarea acțiune este prima</p>
            {#each history.redoEntries as entry, index}
              <article class="snapshot-item redo" class:next={index === 0}>
                <div class="snapshot-main">
                  <strong>{entry.label}</strong>
                  <span>{entryPath(entry)}</span>
                  <small>{entry.source} · {entry.mutationCount} mutație(i) · {formatTime(entry.updatedAtMs)}</small>
                </div>
                <code>{entry.transactionId}</code>
              </article>
            {/each}
          {/if}
        </div>
      {/if}
    {:else}
      <p class="empty-text">Deschide un proiect pentru istoricul sesiunii.</p>
    {/if}
  </aside>
{/if}

<style>
  .history-backdrop {
    position: fixed;
    inset: 0;
    z-index: 39;
    background: rgba(13, 18, 16, 0.18);
  }

  .history-panel {
    position: fixed;
    top: 0;
    right: 0;
    z-index: 40;
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: min(410px, calc(100vw - 24px));
    height: 100vh;
    padding: 12px;
    border-left: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
    box-shadow: -18px 0 42px rgba(0, 0, 0, 0.22);
    overflow: auto;
  }

  .history-header,
  .history-actions,
  .snapshot-item {
    display: flex;
    align-items: center;
  }

  .history-header {
    justify-content: space-between;
    gap: 8px;
  }

  .history-header h2,
  .eyebrow,
  .empty-text,
  .section-label {
    margin: 0;
  }

  .eyebrow,
  .section-label {
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 850;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .workspace-runtime {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
    padding: 9px;
    border: 1px solid var(--border-3);
    border-radius: 8px;
    background: var(--surface-2);
  }

  .workspace-runtime span {
    color: var(--text-muted);
    font-size: 12px;
  }

  .workspace-runtime .memory {
    grid-column: 1 / -1;
  }

  .workspace-runtime b {
    color: var(--text);
  }

  .history-actions {
    gap: 8px;
  }

  .history-actions button,
  .discard-session-btn,
  .icon-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    min-height: 32px;
    border: 1px solid var(--border-2);
    border-radius: 7px;
    background: var(--surface-2);
    color: var(--text);
    cursor: pointer;
  }

  .history-actions button {
    flex: 1;
  }

  button:disabled {
    cursor: default;
    opacity: 0.45;
  }

  .icon-button {
    width: 32px;
    padding: 0;
  }

  .discard-session-btn {
    width: 100%;
    padding: 7px 9px;
    color: var(--danger, #d64545);
  }

  .snapshot-list,
  .snapshot-main {
    display: grid;
    gap: 7px;
  }

  .section-label {
    padding-top: 5px;
  }

  .section-label.redo {
    margin-top: 8px;
  }

  .snapshot-item {
    align-items: flex-start;
    justify-content: space-between;
    gap: 10px;
    padding: 9px;
    border: 1px solid var(--border-3);
    border-radius: 8px;
    background: var(--surface-2);
  }

  .snapshot-item.next {
    border-color: var(--brand);
  }

  .snapshot-item.redo {
    opacity: 0.78;
  }

  .snapshot-main {
    min-width: 0;
    gap: 2px;
  }

  .snapshot-main span,
  .snapshot-main small {
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .snapshot-main span {
    font-size: 12px;
  }

  .snapshot-main small {
    font-size: 12px;
  }

  .snapshot-item code {
    max-width: 90px;
    color: var(--text-muted);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .empty-text {
    padding: 14px 6px;
    color: var(--text-muted);
    font-size: 12px;
    text-align: center;
  }
</style>
