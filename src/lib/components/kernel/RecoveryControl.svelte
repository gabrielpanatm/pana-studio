<script lang="ts">
  import { IconAlertTriangle, IconCircleCheck, IconRefresh, IconShieldCheck } from "@tabler/icons-svelte";
  import RecoveryDiagnosticsList from "$lib/components/kernel/RecoveryDiagnosticsList.svelte";
  import RecoveryProjectTransitionDecisionJournalCard from "$lib/components/kernel/RecoveryProjectTransitionDecisionJournalCard.svelte";
  import RecoveryProjectWorkspaceSaveJournalCard from "$lib/components/kernel/RecoveryProjectWorkspaceSaveJournalCard.svelte";
  import {
    readRecoveryCoordinator,
    recoverProjectTransitionDecisionRetentionHotJournal,
    recoverProjectWorkspaceSave,
  } from "$lib/project/io";
  import type {
    KernelProjectTransitionDecisionRetentionHotJournal,
    KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
    ProjectWorkspaceSaveHotJournal,
    ProjectWorkspaceSaveRecoveryAction,
    RecoveryCoordinatorScan,
  } from "$lib/types";
  import {
    formatRecoveryTime,
    normalizeRecoveryDiagnostic,
    recoveryCoordinatorSummary,
    recoveryDiagnosticIsActionable,
    recoveryJournalFamilyActionLabel,
    recoveryJournalFamilyLabel,
    recoveryJournalFamilyStateLabel,
    recoveryJournalFamilyStatusLabel,
  } from "$lib/kernel/recovery-control";

  let {
    projectKey = "",
    refreshToken = 0,
    onStatusUpdate = undefined as ((text: string, kind: "restored" | "saving" | "error") => void) | undefined,
    onChanged = undefined as (() => void) | undefined,
  }: {
    projectKey?: string;
    refreshToken?: number;
    onStatusUpdate?: (text: string, kind: "restored" | "saving" | "error") => void;
    onChanged?: () => void;
  } = $props();

  let scan = $state<RecoveryCoordinatorScan | null>(null);
  let loading = $state(false);
  let loadError = $state("");
  let activeProjectKey = $state("");
  let activeRefreshToken = $state<number | null>(null);
  let busyId = $state<string | null>(null);
  let notes = $state<Record<string, string>>({});
  let noteErrors = $state<Record<string, string>>({});
  let requestSequence = 0;

  const summary = $derived(recoveryCoordinatorSummary(scan));

  $effect(() => {
    if (!projectKey) return;
    const projectChanged = projectKey !== activeProjectKey;
    const tokenChanged = refreshToken !== activeRefreshToken;
    if (!projectChanged && !tokenChanged) return;
    activeProjectKey = projectKey;
    activeRefreshToken = refreshToken;
    if (projectChanged) {
      scan = null;
      notes = {};
      noteErrors = {};
    }
    void refresh();
  });

  async function refresh() {
    const requestId = ++requestSequence;
    loading = true;
    loadError = "";
    try {
      const next = await readRecoveryCoordinator();
      if (requestId !== requestSequence) return;
      scan = next;
      pruneNotes(next);
    } catch (error) {
      if (requestId !== requestSequence) return;
      loadError = errorMessage(error);
      onStatusUpdate?.(`Recovery Coordinator nu a putut fi citit: ${loadError}`, "error");
    } finally {
      if (requestId === requestSequence) loading = false;
    }
  }

  function updateNote(id: string, value: string) {
    notes = { ...notes, [id]: value };
    if (noteErrors[id]) noteErrors = { ...noteErrors, [id]: "" };
  }

  function diagnosticFor(id: string): string | null {
    const diagnostic = normalizeRecoveryDiagnostic(notes[id] ?? "");
    if (recoveryDiagnosticIsActionable(diagnostic)) return diagnostic;
    noteErrors = { ...noteErrors, [id]: "Descrie verificarea făcută în cel puțin 12 caractere." };
    return null;
  }

  async function recoverWorkspaceSave(
    journal: ProjectWorkspaceSaveHotJournal,
    action: ProjectWorkspaceSaveRecoveryAction,
  ) {
    const diagnostic = diagnosticFor(journal.transactionId);
    if (!diagnostic) return;
    busyId = journal.transactionId;
    try {
      const result = await recoverProjectWorkspaceSave(journal.transactionId, action, diagnostic);
      scan = result.recoveryCoordinator;
      pruneNotes(scan);
      onStatusUpdate?.(
        `ProjectWorkspace Save recovery finalizat pentru ${journal.transactionId}.`,
        "restored",
      );
      onChanged?.();
    } catch (error) {
      onStatusUpdate?.(`ProjectWorkspace Save recovery a eșuat: ${errorMessage(error)}`, "error");
      await refresh();
    } finally {
      busyId = null;
    }
  }

  async function recoverProjectTransition(
    journal: KernelProjectTransitionDecisionRetentionHotJournal,
    action: KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
  ) {
    const diagnostic = diagnosticFor(journal.retentionId);
    if (!diagnostic) return;
    busyId = journal.retentionId;
    try {
      const result = await recoverProjectTransitionDecisionRetentionHotJournal(
        journal.retentionId,
        action,
        diagnostic,
      );
      scan = result.recoveryCoordinator;
      pruneNotes(scan);
      onStatusUpdate?.(`ProjectTransition recovery finalizat pentru ${journal.retentionId}.`, "restored");
      onChanged?.();
    } catch (error) {
      onStatusUpdate?.(`ProjectTransition recovery a eșuat: ${errorMessage(error)}`, "error");
      await refresh();
    } finally {
      busyId = null;
    }
  }

  function pruneNotes(next: RecoveryCoordinatorScan | null) {
    const validIds = new Set<string>();
    for (const journal of next?.hotProjectWorkspaceSaveJournals ?? []) validIds.add(journal.transactionId);
    for (const journal of next?.hotProjectTransitionDecisionRetentionJournals ?? []) validIds.add(journal.retentionId);
    notes = Object.fromEntries(Object.entries(notes).filter(([id]) => validIds.has(id)));
    noteErrors = Object.fromEntries(Object.entries(noteErrors).filter(([id]) => validIds.has(id)));
  }

  function errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }
</script>

<section class="recovery-control" aria-labelledby="recovery-coordinator-title">
  <header class="toolbar">
    <div class={`summary ${summary.tone}`} role={summary.blocked ? "alert" : "status"}>
      {#if summary.tone === "clean"}
        <IconCircleCheck size={18} stroke={1.9} />
      {:else if summary.tone === "idle"}
        <IconShieldCheck size={18} stroke={1.9} />
      {:else}
        <IconAlertTriangle size={18} stroke={1.9} />
      {/if}
      <div>
        <strong id="recovery-coordinator-title">{summary.label}</strong>
        <span>{loading ? "Se scanează jurnalele active..." : summary.detail}</span>
      </div>
    </div>
    <button type="button" title="Recitește Recovery Coordinator" disabled={loading || busyId !== null} onclick={() => void refresh()}>
      <IconRefresh size={15} stroke={1.9} />
    </button>
  </header>

  {#if loadError}<p class="message error" role="alert">{loadError}</p>{/if}

  {#if scan}
    <div class="scan-meta">
      <span>scan {formatRecoveryTime(scan.scannedAtMs)}</span>
      <span>session {scan.sessionId}</span>
      <span>schema {scan.schemaVersion}</span>
    </div>

    {#if scan.hotJournalFamilies.length}
      <div class="families" aria-label="Familii de recovery active">
        {#each scan.hotJournalFamilies as family (family.family)}
          <article class:manual={family.status === "manual_review_required"}>
            <strong>{recoveryJournalFamilyLabel(family.family)}</strong>
            <span>{recoveryJournalFamilyStatusLabel(family.status)} · {family.count}</span>
            <small>{recoveryJournalFamilyActionLabel(family)}</small>
            <small>{recoveryJournalFamilyStateLabel(family)}</small>
          </article>
        {/each}
      </div>
    {/if}

    <RecoveryDiagnosticsList diagnostics={scan.diagnostics} compact label="Diagnostice Recovery Coordinator" />

    {#if scan.hotProjectWorkspaceSaveJournals.length}
      <div class="journal-group">
        <h3>Save întrerupt</h3>
        {#each scan.hotProjectWorkspaceSaveJournals as journal (journal.transactionId)}
          <RecoveryProjectWorkspaceSaveJournalCard
            {journal}
            note={notes[journal.transactionId] ?? ""}
            noteError={noteErrors[journal.transactionId] ?? ""}
            busy={busyId === journal.transactionId}
            disabled={busyId !== null}
            canSubmit={recoveryDiagnosticIsActionable(notes[journal.transactionId] ?? "")}
            onNoteChange={updateNote}
            onRecover={recoverWorkspaceSave}
          />
        {/each}
      </div>
    {/if}

    {#if scan.hotProjectTransitionDecisionRetentionJournals.length}
      <div class="journal-group">
        <h3>ProjectTransition Decision Retention</h3>
        {#each scan.hotProjectTransitionDecisionRetentionJournals as journal (journal.retentionId)}
          <RecoveryProjectTransitionDecisionJournalCard
            {journal}
            note={notes[journal.retentionId] ?? ""}
            noteError={noteErrors[journal.retentionId] ?? ""}
            busy={busyId === journal.retentionId}
            disabled={busyId !== null}
            canSubmit={recoveryDiagnosticIsActionable(notes[journal.retentionId] ?? "")}
            onNoteChange={updateNote}
            onRecover={recoverProjectTransition}
          />
        {/each}
      </div>
    {/if}

  {/if}
</section>

<style>
  .recovery-control { display: grid; gap: 12px; padding: 12px; border: 1px solid var(--border); border-radius: 9px; background: var(--surface-3); }
  .toolbar { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 8px; }
  .summary { display: flex; align-items: center; gap: 9px; min-width: 0; padding: 10px; border: 1px solid var(--border); border-radius: 8px; background: var(--surface); }
  .summary.blocked { border-color: color-mix(in srgb, #f59e0b 45%, var(--border)); }
  .summary.error { border-color: color-mix(in srgb, #ef4444 45%, var(--border)); }
  .summary div { display: grid; gap: 3px; min-width: 0; }
  .summary strong { color: var(--text-strong); font-size: 13px; }
  .summary span,
  .scan-meta,
  .families span,
  .families small { color: var(--text-muted); font-size: 11px; }
  .toolbar button { width: 36px; border: 1px solid var(--border); border-radius: 8px; background: var(--surface); color: var(--text); }
  .scan-meta { display: flex; flex-wrap: wrap; gap: 12px; }
  .families { display: grid; grid-template-columns: repeat(auto-fit, minmax(210px, 1fr)); gap: 8px; }
  .families article { display: grid; gap: 4px; padding: 9px; border: 1px solid var(--border); border-radius: 7px; background: var(--surface); }
  .families article.manual { border-color: color-mix(in srgb, #ef4444 40%, var(--border)); }
  .families strong { font-size: 12px; }
  .journal-group { display: grid; gap: 9px; }
  .journal-group h3 { margin: 2px 0; color: var(--text-strong); font-size: 12px; text-transform: uppercase; }
  .message { margin: 0; font-size: 12px; }
  .message.error { color: #ef4444; }
</style>
