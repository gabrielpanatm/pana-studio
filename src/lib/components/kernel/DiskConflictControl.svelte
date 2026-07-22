<script lang="ts">
  import {
    IconAlertTriangle,
    IconCircleCheck,
    IconDeviceFloppy,
    IconInfoCircle,
    IconRefresh,
  } from "@tabler/icons-svelte";
  import { readKernelDiskConflicts } from "$lib/project/io";
  import type { KernelDiskConflictFileSnapshot, KernelDiskConflictSnapshot } from "$lib/types";
  import { compactKernelPath, formatBytes } from "$lib/kernel/recovery-control";

  let {
    projectKey = "",
    refreshToken = 0,
    onStatusUpdate = undefined as ((text: string, kind: "restored" | "saving" | "error") => void) | undefined,
  }: {
    projectKey?: string;
    refreshToken?: number;
    onStatusUpdate?: (text: string, kind: "restored" | "saving" | "error") => void;
  } = $props();

  let snapshot = $state<KernelDiskConflictSnapshot | null>(null);
  let loading = $state(false);
  let loadError = $state("");
  let activeProjectKey = $state("");
  let activeRefreshToken = $state<number | null>(null);
  let staleReason = $state("");
  let syncReason = $state("");
  let requestSequence = 0;

  const visibleFiles = $derived(snapshot?.files.filter((file) => file.status !== "clean").slice(0, 24) ?? []);
  const summaryTone = $derived(snapshot?.summary.status ?? "clean");
  const summaryLabel = $derived(snapshot?.summary.verdictReason ?? "Instantaneul conflictelor pe disc este indisponibil.");

  $effect(() => {
    if (!projectKey) return;
    const projectChanged = projectKey !== activeProjectKey;
    const refreshChanged = refreshToken !== activeRefreshToken;
    if (!projectChanged && !refreshChanged) return;
    activeProjectKey = projectKey;
    activeRefreshToken = refreshToken;
    if (projectChanged) {
      snapshot = null;
      staleReason = "";
      syncReason = "";
    }
    loadError = "";
    void refreshDiskConflicts(projectChanged ? "project" : "workspace");
  });

  async function refreshDiskConflicts(reason: "project" | "workspace" | "manual" = "manual") {
    const requestId = ++requestSequence;
    const reasonLabel = diskRefreshReasonLabel(reason);
    loading = true;
    syncReason = reasonLabel;
    loadError = "";
    try {
      const nextSnapshot = await readKernelDiskConflicts();
      if (requestId === requestSequence) {
        snapshot = nextSnapshot;
        staleReason = "";
      }
    } catch (error) {
      if (requestId === requestSequence) {
        loadError = errorMessage(error);
        staleReason = snapshot ? reasonLabel : "";
        onStatusUpdate?.(`Disk Conflict Snapshot eșuat: ${loadError}`, "error");
      }
    } finally {
      if (requestId === requestSequence) {
        loading = false;
        syncReason = "";
      }
    }
  }

  function diskRefreshReasonLabel(reason: "project" | "workspace" | "manual") {
    if (reason === "project") return "Sesiunea proiectului";
    if (reason === "workspace") return "Spațiul de lucru";
    return "actualizare manuală";
  }

  function errorMessage(error: unknown) {
    return error instanceof Error ? error.message : String(error);
  }

  function statusLabel(file: KernelDiskConflictFileSnapshot) {
    if (file.kind === "dirty_only") return "modificat";
    if (file.kind === "metadata_changed") return "metadate";
    if (file.kind === "disk_changed") return "schimbat pe disc";
    if (file.kind === "missing_on_disk") return "lipsește";
    if (file.kind === "readonly") return "doar citire";
    if (file.kind === "not_file") return "nu este fișier";
    if (file.kind === "oversized") return "prea mare";
    if (file.kind === "unreadable") return "necitibil";
    if (file.kind === "invalid_path") return "cale invalidă";
    return "curat";
  }

  function baselineLabel(file: KernelDiskConflictFileSnapshot) {
    const diskHash = file.disk?.hash ?? "absent";
    return `referință ${file.baseline.hash} · disc ${diskHash}`;
  }
</script>

<section
  class="disk-conflict-control"
  data-kernel-surface="disk_conflict"
  tabindex="-1"
  aria-labelledby="kernel-disk-conflict-title"
>
  <div class="disk-toolbar">
    <div
      class={`disk-summary ${summaryTone}`}
      role={summaryTone === "error" ? "alert" : "status"}
      aria-live="polite"
    >
      {#if summaryTone === "clean"}
        <IconCircleCheck size={17} stroke={1.9} />
      {:else if summaryTone === "info"}
        <IconInfoCircle size={17} stroke={1.9} />
      {:else}
        <IconAlertTriangle size={17} stroke={1.9} />
      {/if}
      <div>
        <strong id="kernel-disk-conflict-title">Instantaneu conflicte pe disc</strong>
        <span>{loading ? `Se verifică discul${syncReason ? ` după ${syncReason}` : ""}...` : summaryLabel}</span>
      </div>
    </div>

    <button
      type="button"
      class="disk-refresh"
      title="Recitește instantaneul conflictelor pe disc"
      onclick={() => void refreshDiskConflicts("manual")}
      disabled={loading}
    >
      <IconRefresh size={15} stroke={1.9} />
    </button>
  </div>

  {#if staleReason}
    <div class="disk-note warning" role="alert">
      <IconAlertTriangle size={15} stroke={1.9} />
      <span>Conflictele pe disc afișează ultimul instantaneu valid; actualizarea după {staleReason} a eșuat.</span>
    </div>
  {/if}

  {#if loadError}
    <p class="disk-message error" role="alert">{loadError}</p>
  {/if}

  {#if snapshot}
    <div class="disk-metrics" aria-label="Metrice conflicte pe disc">
      <span>
        <em>URMĂRITE</em>
        <strong>{snapshot.summary.trackedFileCount}</strong>
      </span>
      <span>
        <em>CONFLICTE</em>
        <strong>{snapshot.summary.conflictCount}</strong>
      </span>
      <span>
        <em>MODIFICATE</em>
        <strong>{snapshot.summary.dirtyOnlyCount}</strong>
      </span>
      <span>
        <em>SCHIMBATE</em>
        <strong>{snapshot.summary.diskChangedCount}</strong>
      </span>
      <span>
        <em>DOAR CITIRE</em>
        <strong>{snapshot.summary.readonlyCount}</strong>
      </span>
    </div>

    {#if visibleFiles.length}
      <div class="disk-file-list" aria-label="Fișiere cu semnal de conflict pe disc">
        {#each visibleFiles as file (file.relativePath)}
          <article class={`disk-file ${file.status}`}>
            <header>
              <IconDeviceFloppy size={15} stroke={1.9} />
              <div>
                <span>{statusLabel(file)} · rev {file.revision}</span>
                <strong title={file.relativePath}>{compactKernelPath(file.relativePath, 72)}</strong>
              </div>
              <em>{file.role}</em>
            </header>
            <p>{file.message}</p>
            <div class="disk-file-meta">
              <span title={baselineLabel(file)}>{baselineLabel(file)}</span>
              <span>{formatBytes(file.baseline.size)} referință</span>
              <span>{file.disk ? `${formatBytes(file.disk.size)} disc` : "absent pe disc"}</span>
              <span>{file.hasDraft ? "ciornă" : "fără ciornă"}</span>
            </div>
          </article>
        {/each}
      </div>
    {:else}
      <div class="empty-disk-state">
        <IconCircleCheck size={17} stroke={1.9} />
        <span>Nu există fișiere cu semnal de conflict în instantaneul curent.</span>
      </div>
    {/if}
  {:else if !loading && !loadError}
    <div class="empty-disk-state">
      <IconDeviceFloppy size={17} stroke={1.9} />
      <span>Instantaneul conflictelor pe disc nu este încă disponibil pentru sesiunea curentă.</span>
    </div>
  {/if}
</section>

<style>
  .disk-conflict-control {
    display: grid;
    gap: 10px;
    padding: 10px;
    border-top: 1px solid var(--border);
    background: var(--surface-3);
  }

  .disk-toolbar {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 8px;
    align-items: stretch;
  }

  .disk-summary {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    min-width: 0;
    padding: 10px;
    border: 1px solid var(--border-2);
    border-radius: 8px;
    background: var(--surface-2);
  }

  .disk-summary.clean {
    border-color: var(--chip-border);
    color: var(--brand-strong);
    background: var(--chip-bg);
  }

  .disk-summary.info {
    border-color: color-mix(in srgb, var(--brand) 28%, var(--border));
    color: var(--brand-strong);
    background: color-mix(in srgb, var(--brand) 9%, var(--surface-4));
  }

  .disk-summary.warning {
    border-color: color-mix(in srgb, #f0b25b 42%, var(--border));
    color: #f0b25b;
    background: color-mix(in srgb, #f0b25b 10%, var(--surface-4));
  }

  .disk-summary.error {
    border-color: color-mix(in srgb, #ef4444 42%, var(--border));
    color: #fca5a5;
    background: color-mix(in srgb, #ef4444 10%, var(--surface-4));
  }

  .disk-summary strong,
  .disk-summary span {
    display: block;
  }

  .disk-summary strong {
    color: var(--text-strong);
    font-size: 12px;
    font-weight: 850;
  }

  .disk-summary span {
    margin-top: 3px;
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.45;
  }

  .disk-refresh {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 38px;
    min-height: 38px;
    padding: 0;
    border: 1px solid var(--border-3);
    border-radius: 8px;
    color: var(--text);
    background: var(--surface-2);
    cursor: pointer;
  }

  .disk-refresh:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .disk-note,
  .disk-message {
    margin: 0;
    padding: 9px 10px;
    border: 1px solid var(--border-2);
    border-radius: 7px;
    font-size: 12px;
    line-height: 1.45;
  }

  .disk-note {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }

  .disk-note.warning {
    color: #f0b25b;
    background: color-mix(in srgb, #f0b25b 10%, var(--surface-4));
    border-color: color-mix(in srgb, #f0b25b 42%, var(--border));
  }

  .disk-message.error {
    border-color: color-mix(in srgb, #ef4444 42%, var(--border));
    color: #fca5a5;
    background: color-mix(in srgb, #ef4444 10%, var(--surface-4));
  }

  .disk-metrics {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 6px;
  }

  .disk-metrics span {
    display: grid;
    gap: 3px;
    min-width: 0;
    padding: 7px;
    border: 1px solid var(--border-2);
    border-radius: 7px;
    background: var(--surface-2);
  }

  .disk-metrics em {
    overflow: hidden;
    color: var(--text-muted);
    font-size: 12px;
    font-style: normal;
    font-weight: 900;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .disk-metrics strong {
    color: var(--text-strong);
    font-size: 15px;
    font-weight: 900;
  }

  .disk-file-list {
    display: grid;
    gap: 8px;
    max-height: 360px;
    overflow: auto;
  }

  .disk-file {
    display: grid;
    gap: 8px;
    min-width: 0;
    padding: 10px;
    border: 1px solid var(--border-2);
    border-radius: 8px;
    background: var(--surface-2);
  }

  .disk-file.info {
    border-color: color-mix(in srgb, var(--brand) 22%, var(--border));
  }

  .disk-file.warning {
    border-color: color-mix(in srgb, #f0b25b 38%, var(--border));
  }

  .disk-file.error {
    border-color: color-mix(in srgb, #ef4444 36%, var(--border));
  }

  .disk-file header {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    gap: 8px;
    align-items: start;
  }

  .disk-file header div {
    min-width: 0;
  }

  .disk-file header span,
  .disk-file header strong {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .disk-file header span {
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 800;
  }

  .disk-file header strong {
    margin-top: 3px;
    color: var(--text-strong);
    font-size: 12px;
    font-weight: 900;
  }

  .disk-file header em {
    padding: 3px 7px;
    border: 1px solid var(--border-3);
    border-radius: 999px;
    color: var(--text-muted);
    background: var(--surface-4);
    font-size: 12px;
    font-style: normal;
    font-weight: 950;
  }

  .disk-file p {
    margin: 0;
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.45;
  }

  .disk-file-meta {
    display: grid;
    grid-template-columns: minmax(0, 1.5fr) repeat(3, minmax(0, 0.65fr));
    gap: 6px;
  }

  .disk-file-meta span {
    min-width: 0;
    overflow: hidden;
    padding: 7px;
    border: 1px solid var(--border-2);
    border-radius: 7px;
    color: var(--text);
    background: var(--surface-3);
    font-size: 12px;
    font-weight: 800;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .empty-disk-state {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px;
    border: 1px dashed var(--border-3);
    border-radius: 7px;
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 750;
  }

  @media (max-width: 760px) {
    .disk-metrics,
    .disk-file-meta {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
