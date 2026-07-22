<script lang="ts">
  import { IconClock, IconDeviceFloppy, IconRefresh } from "@tabler/icons-svelte";
  import type {
    ProjectWorkspaceSaveHotJournal,
    ProjectWorkspaceSaveRecoveryAction,
  } from "$lib/types";
  import { compactKernelPath, formatRecoveryTime, shortHash } from "$lib/kernel/recovery-control";

  let {
    journal,
    note = "",
    noteError = "",
    busy = false,
    disabled = false,
    canSubmit = false,
    onNoteChange = undefined as ((transactionId: string, value: string) => void) | undefined,
    onRecover = undefined as ((
      journal: ProjectWorkspaceSaveHotJournal,
      action: ProjectWorkspaceSaveRecoveryAction,
    ) => void) | undefined,
  }: {
    journal: ProjectWorkspaceSaveHotJournal;
    note?: string;
    noteError?: string;
    busy?: boolean;
    disabled?: boolean;
    canSubmit?: boolean;
    onNoteChange?: (transactionId: string, value: string) => void;
    onRecover?: (
      journal: ProjectWorkspaceSaveHotJournal,
      action: ProjectWorkspaceSaveRecoveryAction,
    ) => void;
  } = $props();

  const actionable = $derived(
    (journal.recoveryPlan.canClearJournal || journal.recoveryPlan.canRollback) &&
      canSubmit &&
      !disabled &&
      !busy,
  );
  const noteId = $derived(`workspace-save-note-${journal.transactionId.replace(/[^a-zA-Z0-9_-]/g, "-")}`);

  function actionLabel(action: ProjectWorkspaceSaveRecoveryAction): string {
    if (action === "clear_stale_journal") return "Curăță jurnalul stale";
    if (action === "rollback_to_before") return "Revino la starea dinainte de salvare";
    return "Revizuire manuală obligatorie";
  }
</script>

<article class={`workspace-save-journal ${journal.diskState}`}>
  <header>
    <div>
      <span>Jurnal de salvare</span>
      <h3>{journal.transactionId}</h3>
    </div>
    <time>
      <IconClock size={14} stroke={1.8} />
      {formatRecoveryTime(journal.preparedAtMs)}
    </time>
  </header>

  <div class="facts">
    <span><em>REVIZIE</em><strong>{journal.revision}</strong></span>
    <span><em>STARE DISC</em><strong>{journal.diskState.replaceAll("_", " ")}</strong></span>
    <span><em>FIȘIERE</em><strong>{journal.fileCount}</strong></span>
    <span><em>RECUPERARE</em><strong>{actionLabel(journal.recoveryPlan.action)}</strong></span>
  </div>

  <p class="plan">{journal.recoveryPlan.summary}</p>

  <div class="path" title={journal.path}>
    <IconDeviceFloppy size={15} stroke={1.8} />
    <span>{compactKernelPath(journal.path, 92)}</span>
  </div>

  <div class="files">
    {#each journal.files as file (file.relativePath)}
      <div class={`file ${file.diskState}`}>
        <strong title={file.relativePath}>{compactKernelPath(file.relativePath, 62)}</strong>
        <span>{file.diskState} · before {shortHash(file.beforeHash)} · planned {shortHash(file.plannedHash)} · disk {shortHash(file.diskHash)}</span>
        {#if file.diagnostic}<p>{file.diagnostic}</p>{/if}
      </div>
    {/each}
  </div>

  {#if journal.recoveryPlan.canClearJournal || journal.recoveryPlan.canRollback}
    <div class="action">
      <label for={noteId}>
        <span>Diagnostic operator</span>
        <textarea
          id={noteId}
          rows="2"
          value={note}
          aria-invalid={Boolean(noteError)}
          disabled={disabled || busy}
          oninput={(event) => onNoteChange?.(journal.transactionId, event.currentTarget.value)}
        ></textarea>
      </label>
      {#if noteError}<p class="error" role="alert">{noteError}</p>{/if}
      <button
        type="button"
        disabled={!actionable}
        onclick={() => onRecover?.(journal, journal.recoveryPlan.action)}
      >
        <IconRefresh size={14} stroke={1.9} />
        {busy ? "Se execută..." : actionLabel(journal.recoveryPlan.action)}
      </button>
    </div>
  {/if}
</article>

<style>
  .workspace-save-journal {
    display: grid;
    gap: 11px;
    padding: 12px;
    border: 1px solid var(--border-3);
    border-radius: 8px;
    background: var(--surface-2);
  }
  .workspace-save-journal.mixed_state,
  .workspace-save-journal.conflict_state {
    border-color: color-mix(in srgb, #ef4444 42%, var(--border));
  }
  header,
  time,
  .path,
  button {
    display: flex;
    align-items: center;
  }
  header { justify-content: space-between; gap: 12px; }
  header span,
  em,
  label span { color: var(--text-muted); font-size: 12px; font-weight: 850; font-style: normal; text-transform: uppercase; }
  h3 { margin: 4px 0 0; color: var(--text-strong); font-size: 13px; }
  time,
  .path { gap: 5px; color: var(--text-muted); font-size: 12px; }
  .facts { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 8px; }
  .facts span,
  .file,
  .action { padding: 9px; border: 1px solid var(--border); border-radius: 7px; background: var(--surface); }
  .facts strong { display: block; margin-top: 4px; color: var(--text-strong); font-size: 12px; }
  .plan { margin: 0; color: var(--text-muted); font-size: 12px; line-height: 1.45; }
  .files { display: grid; gap: 6px; }
  .file { display: grid; gap: 3px; }
  .file strong { font-size: 12px; }
  .file span,
  .file p { margin: 0; color: var(--text-muted); font-size: 12px; }
  .action { display: grid; gap: 7px; }
  label { display: grid; gap: 5px; }
  textarea { width: 100%; box-sizing: border-box; resize: vertical; }
  button { justify-self: start; gap: 6px; min-height: 32px; padding: 0 10px; border: 1px solid var(--border); border-radius: 7px; background: var(--surface-3); color: var(--text); cursor: pointer; }
  button:disabled { cursor: not-allowed; opacity: 0.55; }
  .error { margin: 0; color: #ef4444; font-size: 12px; }
  @media (max-width: 920px) { .facts { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
</style>
