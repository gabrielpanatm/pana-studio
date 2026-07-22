<script lang="ts">
  import { IconArchive, IconClock, IconDatabase, IconRefresh } from "@tabler/icons-svelte";
  import type {
    KernelProjectTransitionDecisionRetentionHotJournal,
    KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
  } from "$lib/types";
  import {
    compactKernelPath,
    formatRecoveryTime,
    projectTransitionDecisionRetentionCandidateIdsLabel,
    projectTransitionRetentionActionLabel,
    projectTransitionRetentionStateLabel,
    shortHash,
  } from "$lib/kernel/recovery-control";

  let {
    journal,
    note = "",
    noteError = "",
    busy = false,
    disabled = false,
    canSubmit = false,
    highlighted = false,
    onNoteChange = undefined as ((retentionId: string, value: string) => void) | undefined,
    onRecover = undefined as ((
      journal: KernelProjectTransitionDecisionRetentionHotJournal,
      action: KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
    ) => void) | undefined,
  }: {
    journal: KernelProjectTransitionDecisionRetentionHotJournal;
    note?: string;
    noteError?: string;
    busy?: boolean;
    disabled?: boolean;
    canSubmit?: boolean;
    highlighted?: boolean;
    onNoteChange?: (retentionId: string, value: string) => void;
    onRecover?: (
      journal: KernelProjectTransitionDecisionRetentionHotJournal,
      action: KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
    ) => void;
  } = $props();

  const canRunAction = $derived(
    (journal.recoveryPlan.canClearJournal || journal.recoveryPlan.canRestoreBeforeJournal) &&
      canSubmit &&
      !disabled &&
      !busy,
  );
  const noteId = $derived(`project-transition-retention-note-${journal.retentionId.replace(/[^a-zA-Z0-9_-]/g, "-")}`);
  const candidateIdsLabel = $derived(projectTransitionDecisionRetentionCandidateIdsLabel(journal));
  const hashLabel = $derived(
    `înainte ${shortHash(journal.beforeJournalHash)} · după ${shortHash(journal.afterJournalHash)} · arhivă ${shortHash(journal.archiveHash)}`,
  );

  function actionButtonLabel(action: KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction): string {
    if (action === "restore_before_journal") return "Restaurează starea anterioară";
    if (action === "manual_review_conflict") return "Revizuire manuală";
    return "Curăță jurnalul";
  }
</script>

<article
  class={`project-transition-retention-journal ${journal.diskState}`}
  class:kernel-context-focus={highlighted}
  aria-label="Jurnal activ pentru retenția deciziilor de tranziție"
>
  <header class="retention-journal-header">
    <div>
      <span class="journal-kind">retenția deciziei de tranziție</span>
      <h3>{journal.retentionId}</h3>
    </div>
    <span class="journal-time">
      <IconClock size={14} stroke={1.8} />
      {formatRecoveryTime(journal.createdAtMs)}
    </span>
  </header>

  <p class="journal-detail">{journal.recoveryPlan.summary}</p>

  <dl class="journal-facts">
    <div>
      <dt>Stare disc</dt>
      <dd>{projectTransitionRetentionStateLabel(journal.diskState)}</dd>
    </div>
    <div>
      <dt>Acțiune</dt>
      <dd>{projectTransitionRetentionActionLabel(journal.recoveryPlan.action)}</dd>
    </div>
    <div>
      <dt>Candidați</dt>
      <dd title={candidateIdsLabel}>{journal.candidateCount}</dd>
    </div>
    <div>
      <dt>Păstrate</dt>
      <dd>{journal.keptRecordCount}</dd>
    </div>
  </dl>

  <div class="journal-path" title={journal.path}>
    <IconDatabase size={15} stroke={1.8} />
    <span>{compactKernelPath(journal.path)}</span>
  </div>

  <div class="journal-path" title={journal.archivePath}>
    <IconArchive size={15} stroke={1.8} />
    <span>{compactKernelPath(journal.archivePath)}</span>
  </div>

  <section class="journal-plan" aria-label="Plan de recuperare a retenției deciziei">
    <strong>{journal.recoveryPlan.title}</strong>
    <p>{journal.recoveryPlan.summary}</p>
    <ul>
      {#each journal.recoveryPlan.requiredChecks as check}
        <li>{check}</li>
      {/each}
    </ul>
  </section>

  <section class="journal-integrity" aria-label="Integritatea jurnalului activ de retenție">
    <strong title={hashLabel}>{hashLabel}</strong>
    <span title={candidateIdsLabel}>{candidateIdsLabel}</span>
    {#if journal.diagnostics.length}
      <ul>
        {#each journal.diagnostics as diagnostic}
          <li>{diagnostic}</li>
        {/each}
      </ul>
    {/if}
  </section>

  {#if journal.recoveryPlan.canClearJournal || journal.recoveryPlan.canRestoreBeforeJournal}
    <section class="journal-action" aria-label="Acțiune de recuperare a retenției deciziei">
      <label class="journal-note-field" for={noteId}>
        <span>Diagnostic operator pentru recuperare</span>
        <textarea
          id={noteId}
          rows="2"
          value={note}
          aria-invalid={Boolean(noteError)}
          disabled={disabled || busy}
          oninput={(event) => onNoteChange?.(journal.retentionId, (event.currentTarget as HTMLTextAreaElement).value)}
        ></textarea>
      </label>
      {#if noteError}
        <p class="journal-note-error" role="alert">{noteError}</p>
      {/if}
      <button
        type="button"
        class="journal-recovery-btn"
        disabled={!canRunAction}
        onclick={() => onRecover?.(journal, journal.recoveryPlan.action)}
      >
        <IconRefresh size={14} stroke={1.9} />
        <span>{busy ? "Se execută..." : actionButtonLabel(journal.recoveryPlan.action)}</span>
      </button>
    </section>
  {/if}
</article>

<style>
  .project-transition-retention-journal {
    display: grid;
    gap: 12px;
    padding: 12px;
    border: 1px solid var(--border-3);
    border-radius: 8px;
    background: var(--surface-2);
  }

  .project-transition-retention-journal.no_effect,
  .project-transition-retention-journal.completed_retention {
    border-color: color-mix(in srgb, #f59e0b 38%, var(--border));
    background: color-mix(in srgb, #f59e0b 7%, var(--surface-2));
  }

  .project-transition-retention-journal.partial_retention,
  .project-transition-retention-journal.conflict_state {
    border-color: color-mix(in srgb, #ef4444 42%, var(--border));
    background: color-mix(in srgb, #ef4444 8%, var(--surface-2));
  }

  .retention-journal-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }

  .journal-kind {
    color: var(--brand-strong);
    font-size: 12px;
    font-weight: 850;
    letter-spacing: 0;
    text-transform: uppercase;
  }

  .retention-journal-header h3 {
    margin: 4px 0 0;
    color: var(--text-strong);
    font-size: 14px;
    line-height: 1.25;
  }

  .journal-time {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    flex: 0 0 auto;
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 800;
  }

  .journal-detail {
    margin: 0;
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.45;
  }

  .journal-facts {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 8px;
    margin: 0;
  }

  .journal-facts div,
  .journal-path,
  .journal-plan,
  .journal-integrity,
  .journal-action {
    border: 1px solid var(--border-2);
    border-radius: 7px;
    background: var(--surface-3);
  }

  .journal-facts div {
    min-width: 0;
    padding: 9px;
  }

  .journal-facts dt,
  .journal-note-field span {
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 850;
    letter-spacing: 0;
    text-transform: uppercase;
  }

  .journal-facts dd {
    margin: 5px 0 0;
    overflow: hidden;
    color: var(--text-strong);
    font-size: 12px;
    font-weight: 750;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .journal-path {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
    padding: 9px 10px;
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 750;
  }

  .journal-path span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .journal-plan,
  .journal-integrity,
  .journal-action {
    padding: 10px;
  }

  .journal-plan strong,
  .journal-integrity strong {
    display: block;
    color: var(--text-strong);
    font-size: 12px;
    font-weight: 850;
  }

  .journal-plan p,
  .journal-integrity span {
    display: block;
    margin: 5px 0 0;
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.45;
  }

  .journal-plan ul,
  .journal-integrity ul {
    display: grid;
    gap: 4px;
    margin: 8px 0 0;
    padding: 0 0 0 16px;
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.45;
  }

  .journal-note-field {
    display: grid;
    gap: 6px;
  }

  .journal-note-field textarea {
    width: 100%;
    min-height: 58px;
    resize: vertical;
    border: 1px solid var(--border-3);
    border-radius: 7px;
    color: var(--text);
    background: var(--surface-2);
    font: inherit;
    font-size: 12px;
    line-height: 1.45;
  }

  .journal-note-error {
    margin: 7px 0 0;
    color: #fca5a5;
    font-size: 12px;
    line-height: 1.4;
  }

  .journal-recovery-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    width: fit-content;
    margin-top: 8px;
    padding: 7px 10px;
    border: 1px solid var(--brand);
    border-radius: 7px;
    color: var(--brand-strong);
    background: var(--brand-soft);
    font-size: 12px;
    font-weight: 850;
    cursor: pointer;
  }

  .journal-recovery-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  @media (max-width: 760px) {
    .journal-facts {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .retention-journal-header {
      display: grid;
    }
  }
</style>
