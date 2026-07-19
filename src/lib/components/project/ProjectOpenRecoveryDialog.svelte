<script lang="ts">
  import { IconAlertTriangle, IconFolderOpen, IconShieldCheck, IconX } from "@tabler/icons-svelte";
  import {
    projectOpenRecoveryReasonLabel,
    type ProjectOpenRecoveryDecisionRequest,
  } from "$lib/project/open-recovery";
  import { errorMessage } from "$lib/util";

  let {
    request = null,
    abandon,
    cancel,
  }: {
    request?: ProjectOpenRecoveryDecisionRequest | null;
    abandon: (requestId: string) => Promise<void>;
    cancel: (requestId: string) => void;
  } = $props();

  let submitting = $state(false);
  let error = $state<string | null>(null);
  let activeRequestId = $state<string | null>(null);

  $effect(() => {
    if (request?.id === activeRequestId) return;
    activeRequestId = request?.id ?? null;
    submitting = false;
    error = null;
  });

  function close() {
    if (!request || submitting) return;
    cancel(request.id);
  }

  async function confirmAbandonment() {
    if (!request || submitting) return;
    submitting = true;
    error = null;
    try {
      await abandon(request.id);
    } catch (submitError) {
      error = errorMessage(submitError);
      submitting = false;
    }
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    if (!request || event.key !== "Escape") return;
    event.preventDefault();
    close();
  }
</script>

<svelte:window onkeydown={handleWindowKeydown} />

{#if request}
  <div class="recovery-backdrop" role="presentation" onclick={close}></div>
  <div
    class="recovery-dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby="project-open-recovery-title"
    aria-describedby="project-open-recovery-message"
  >
    <header class="dialog-header">
      <div class="dialog-title">
        <span class="dialog-icon" aria-hidden="true">
          <IconShieldCheck size={18} stroke={1.9} />
        </span>
        <div>
          <p>Open Project</p>
          <h2 id="project-open-recovery-title">Sesiune recuperabilă incompatibilă</h2>
        </div>
      </div>
      <button type="button" class="icon-button" title="Anulează și păstrează recovery-ul" disabled={submitting} onclick={close}>
        <IconX size={16} stroke={1.9} />
      </button>
    </header>

    <div class="recovery-summary">
      <span class="reason-pill">{projectOpenRecoveryReasonLabel(request.assessment)}</span>
      <p id="project-open-recovery-message">
        La această cale există drafturi dintr-o sesiune mai veche, dar dosarul de pe disk nu mai
        corespunde baseline-ului lor. Aplicarea automată ar putea combina două proiecte diferite.
      </p>
      <p class="target-path">{request.targetRoot}</p>
    </div>

    <div class="metric-list" aria-label="Evidență recovery">
      <div class="metric-row">
        <span>Documente modificate</span>
        <strong>{request.assessment.dirtyDocumentCount}</strong>
      </div>
      <div class="metric-row">
        <span>Istoric recuperabil</span>
        <strong>{request.assessment.undoCount} undo / {request.assessment.redoCount} redo</strong>
      </div>
      <div class="metric-row">
        <span>Fișiere în baseline / acum</span>
        <strong>{request.assessment.acceptedFileCount} / {request.assessment.currentFileCount}</strong>
      </div>
      <div class="metric-row">
        <span>Resurse binare în draft</span>
        <strong>{request.assessment.stagedBinaryResourceCount}</strong>
      </div>
    </div>

    <section class="warning-block" aria-label="Consecința alegerii">
      <IconAlertTriangle size={17} stroke={1.9} />
      <div>
        <strong>Recovery-ul rămâne neatins dacă anulezi.</strong>
        <p>
          Continuarea deschide adevărul actual de pe disk și abandonează explicit drafturile vechi
          numai după ce noua sesiune a fost publicată cu succes.
        </p>
      </div>
    </section>

    {#if request.assessment.diagnostic}
      <p class="diagnostic">{request.assessment.diagnostic}</p>
    {/if}
    {#if error}
      <p class="error-message">{error}</p>
    {/if}

    <footer class="dialog-actions">
      <button type="button" class="secondary-button" disabled={submitting} onclick={close}>
        Păstrează recovery-ul și anulează
      </button>
      <button type="button" class="danger-button" disabled={submitting} onclick={() => void confirmAbandonment()}>
        {#if submitting}
          Se verifică și se deschide...
        {:else}
          <IconFolderOpen size={16} stroke={1.9} />
          Deschide dosarul actual și abandonează drafturile
        {/if}
      </button>
    </footer>
  </div>
{/if}

<style>
  .recovery-backdrop {
    position: fixed;
    inset: 0;
    z-index: 90;
    background: rgba(8, 12, 11, 0.52);
    backdrop-filter: blur(2px);
  }

  .recovery-dialog {
    position: fixed;
    top: 50%;
    left: 50%;
    z-index: 91;
    display: flex;
    flex-direction: column;
    gap: 14px;
    width: min(680px, calc(100vw - 28px));
    max-height: calc(100vh - 32px);
    padding: 16px;
    border: 1px solid var(--border-4);
    border-radius: 8px;
    color: var(--text);
    background: var(--surface);
    box-shadow: 0 24px 70px rgba(0, 0, 0, 0.34);
    overflow-y: auto;
    transform: translate(-50%, -50%);
  }

  .dialog-header,
  .dialog-title,
  .dialog-actions,
  .warning-block,
  .danger-button {
    display: flex;
    align-items: center;
  }

  .dialog-header {
    justify-content: space-between;
    gap: 12px;
  }

  .dialog-title {
    gap: 10px;
    min-width: 0;
  }

  .dialog-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border: 1px solid color-mix(in srgb, var(--source-origin-theme) 55%, var(--border-4));
    border-radius: 8px;
    color: var(--source-origin-theme);
    background: color-mix(in srgb, var(--source-origin-theme) 10%, var(--surface));
  }

  .dialog-title p,
  .dialog-title h2,
  .recovery-summary p,
  .warning-block p,
  .diagnostic,
  .error-message {
    margin: 0;
  }

  .dialog-title p {
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 850;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .dialog-title h2 {
    margin-top: 2px;
    color: var(--text-strong);
    font-size: 18px;
    font-weight: 850;
  }

  .icon-button {
    width: 30px;
    height: 30px;
    border: 1px solid var(--border-4);
    border-radius: 6px;
    color: var(--text-muted);
    background: var(--surface);
  }

  .recovery-summary {
    display: grid;
    gap: 9px;
  }

  .reason-pill {
    justify-self: start;
    padding: 3px 7px;
    border: 1px solid color-mix(in srgb, var(--source-origin-theme) 55%, var(--border-4));
    border-radius: 999px;
    color: var(--source-origin-theme);
    font-size: 11px;
    font-weight: 800;
    text-transform: uppercase;
  }

  .target-path,
  .diagnostic {
    overflow-wrap: anywhere;
    color: var(--text-muted);
    font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
    font-size: 12px;
  }

  .metric-list {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
  }

  .metric-row {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    padding: 9px 10px;
    border: 1px solid var(--border-4);
    border-radius: 6px;
    background: var(--surface-3);
  }

  .metric-row span {
    color: var(--text-muted);
    font-size: 12px;
  }

  .metric-row strong {
    color: var(--text-strong);
    font-size: 12px;
  }

  .warning-block {
    align-items: flex-start;
    gap: 10px;
    padding: 11px;
    border: 1px solid color-mix(in srgb, var(--source-origin-theme) 45%, var(--border-4));
    border-radius: 7px;
    color: var(--source-origin-theme);
    background: color-mix(in srgb, var(--source-origin-theme) 8%, var(--surface));
  }

  .warning-block div {
    display: grid;
    gap: 4px;
  }

  .warning-block strong,
  .warning-block p {
    font-size: 12px;
  }

  .warning-block p {
    color: var(--text-muted);
    line-height: 1.45;
  }

  .error-message {
    color: var(--danger, #dc2626);
    font-size: 12px;
  }

  .dialog-actions {
    justify-content: flex-end;
    gap: 8px;
  }

  .secondary-button,
  .danger-button {
    min-height: 34px;
    padding: 0 12px;
    border-radius: 6px;
    font-size: 12px;
    font-weight: 750;
  }

  .secondary-button {
    border: 1px solid var(--border-4);
    color: var(--text);
    background: var(--surface);
  }

  .danger-button {
    justify-content: center;
    gap: 7px;
    border: 1px solid var(--danger, #dc2626);
    color: white;
    background: var(--danger, #dc2626);
  }

  button:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  @media (max-width: 700px) {
    .metric-list {
      grid-template-columns: 1fr;
    }

    .dialog-actions {
      align-items: stretch;
      flex-direction: column-reverse;
    }
  }
</style>
