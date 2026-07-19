<script lang="ts">
  import { IconAlertTriangle, IconCircleCheck, IconRefresh, IconRoute } from "@tabler/icons-svelte";
  import {
    acknowledgeProjectTransitionDecisionRecoveryPlan,
    executeProjectTransitionDecisionRetention,
    readKernelProjectTransitionBlockedAudit,
    readKernelProjectTransitionDecisionJournal,
    readKernelProjectTransitionDecisionRecoveryAckJournal,
    readKernelProjectTransitionPolicyMatrix,
  } from "$lib/project/io";
  import type {
    KernelProjectTransitionBlockedAuditSnapshot,
    KernelProjectTransitionDecisionJournalSnapshot,
    KernelProjectTransitionDecisionRecoveryAckJournalSnapshot,
    KernelProjectTransitionPolicyMatrixSnapshot,
  } from "$lib/types";
  import { compactKernelPath, formatRecoveryTime } from "$lib/kernel/recovery-control";

  let {
    projectKey = "",
    refreshToken = 0,
    onStatusUpdate = undefined as ((text: string, kind: "restored" | "saving" | "error") => void) | undefined,
  }: {
    projectKey?: string;
    refreshToken?: number;
    onStatusUpdate?: (text: string, kind: "restored" | "saving" | "error") => void;
  } = $props();

  let policyMatrix = $state<KernelProjectTransitionPolicyMatrixSnapshot | null>(null);
  let blockedAudit = $state<KernelProjectTransitionBlockedAuditSnapshot | null>(null);
  let decisionJournal = $state<KernelProjectTransitionDecisionJournalSnapshot | null>(null);
  let ackJournal = $state<KernelProjectTransitionDecisionRecoveryAckJournalSnapshot | null>(null);
  let loading = $state(false);
  let loadError = $state("");
  let activeProjectKey = $state("");
  let activeRefreshToken = $state<number | null>(null);
  let acknowledgementDiagnostic = $state("");
  let retentionDiagnostic = $state("");
  let mutating = $state(false);
  let mutationError = $state("");
  let requestSequence = 0;

  const recoveryPlan = $derived(decisionJournal?.recoveryPlan ?? null);
  const matchingAck = $derived(
    recoveryPlan && ackJournal
      ? (ackJournal.records.find(
          (record) => record.evidence.recoveryPlanEvidenceHash === recoveryPlan.evidenceHash,
        ) ?? null)
      : null,
  );
  const needsAcknowledgement = $derived(
    recoveryPlan?.status === "retention_review" || recoveryPlan?.status === "integrity_blocked",
  );
  const canAcknowledge = $derived(
    needsAcknowledgement && acknowledgementDiagnostic.trim().length >= 12 && !mutating,
  );
  const canRetain = $derived(
    recoveryPlan?.status === "retention_review" &&
      recoveryPlan.retentionCandidateCount > 0 &&
      matchingAck?.ackKind === "acknowledge_retention_review" &&
      retentionDiagnostic.trim().length >= 12 &&
      !mutating,
  );

  $effect(() => {
    if (!projectKey) return;
    const projectChanged = projectKey !== activeProjectKey;
    const tokenChanged = refreshToken !== activeRefreshToken;
    if (!projectChanged && !tokenChanged) return;
    activeProjectKey = projectKey;
    activeRefreshToken = refreshToken;
    if (projectChanged) {
      policyMatrix = null;
      blockedAudit = null;
      decisionJournal = null;
      ackJournal = null;
    }
    void refresh();
  });

  async function refresh() {
    const requestId = ++requestSequence;
    loading = true;
    loadError = "";
    try {
      const [nextMatrix, nextAudit, nextJournal, nextAcks] = await Promise.all([
        readKernelProjectTransitionPolicyMatrix(),
        readKernelProjectTransitionBlockedAudit(40, false),
        readKernelProjectTransitionDecisionJournal(80),
        readKernelProjectTransitionDecisionRecoveryAckJournal(40),
      ]);
      if (requestId !== requestSequence) return;
      policyMatrix = nextMatrix;
      blockedAudit = nextAudit;
      decisionJournal = nextJournal;
      ackJournal = nextAcks;
    } catch (error) {
      if (requestId !== requestSequence) return;
      loadError = errorMessage(error);
      onStatusUpdate?.(`ProjectTransition nu a putut fi citit: ${loadError}`, "error");
    } finally {
      if (requestId === requestSequence) loading = false;
    }
  }

  async function acknowledgeRecoveryPlan() {
    if (!recoveryPlan || !canAcknowledge) return;
    mutating = true;
    mutationError = "";
    try {
      await acknowledgeProjectTransitionDecisionRecoveryPlan(
        recoveryPlan.evidenceHash,
        acknowledgementDiagnostic.trim(),
      );
      acknowledgementDiagnostic = "";
      onStatusUpdate?.("Revizuirea ProjectTransition a fost confirmată.", "restored");
      await refresh();
    } catch (error) {
      mutationError = errorMessage(error);
      onStatusUpdate?.(`Confirmarea ProjectTransition a eșuat: ${mutationError}`, "error");
    } finally {
      mutating = false;
    }
  }

  async function executeRetention() {
    if (!recoveryPlan || !matchingAck || !canRetain) return;
    mutating = true;
    mutationError = "";
    try {
      const receipt = await executeProjectTransitionDecisionRetention(
        recoveryPlan.evidenceHash,
        matchingAck.id,
        retentionDiagnostic.trim(),
      );
      retentionDiagnostic = "";
      onStatusUpdate?.(
        `Retention ProjectTransition: ${receipt.archivedRecordCount} decizii arhivate.`,
        receipt.status === "recovery_attention" ? "error" : "restored",
      );
      await refresh();
    } catch (error) {
      mutationError = errorMessage(error);
      onStatusUpdate?.(`Retention ProjectTransition a eșuat: ${mutationError}`, "error");
    } finally {
      mutating = false;
    }
  }

  function errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }
</script>

<section class="transition-control" aria-labelledby="project-transition-title">
  <header>
    <div>
      <IconRoute size={18} stroke={1.8} />
      <div>
        <h2 id="project-transition-title">ProjectTransition</h2>
        <p>Open, Reload și Close sunt decise din starea ProjectWorkspace și din conflictele reale de disk.</p>
      </div>
    </div>
    <button type="button" disabled={loading || mutating} onclick={() => void refresh()} title="Recitește">
      <IconRefresh size={15} stroke={1.9} />
    </button>
  </header>

  {#if loadError}<p class="error" role="alert">{loadError}</p>{/if}

  {#if policyMatrix}
    <div class="policies">
      {#each policyMatrix.policies as policy (policy.action)}
        <article class:blocked={policy.blocksTransition} class:confirm={policy.requiresOperatorConfirmation}>
          <div>
            {#if policy.blocksTransition || policy.requiresOperatorConfirmation}
              <IconAlertTriangle size={15} stroke={1.9} />
            {:else}
              <IconCircleCheck size={15} stroke={1.9} />
            {/if}
            <strong>{policy.action.replaceAll("_", " ")}</strong>
            <em>{policy.decision}</em>
          </div>
          <p>{policy.message}</p>
          <small>{policy.reason} · workspace rev {policy.workspaceRevision ?? "—"} · {policy.workspaceDirtyResourceCount} resurse dirty</small>
        </article>
      {/each}
    </div>
  {/if}

  {#if blockedAudit && blockedAudit.records.length}
    <section class="audit">
      <h3>Tranziții blocate recent</h3>
      {#each blockedAudit.records.slice(0, 8) as record (record.id)}
        <article>
          <strong>{record.action?.replaceAll("_", " ") ?? record.operation}</strong>
          <span>{formatRecoveryTime(record.blockedAtMs)} · {record.message}</span>
          <small title={record.targetProjectRoot ?? ""}>{compactKernelPath(record.targetProjectRoot ?? record.target ?? "", 72)}</small>
        </article>
      {/each}
    </section>
  {/if}

  {#if decisionJournal}
    <section class="decisions">
      <div class="decision-summary">
        <strong>{decisionJournal.health.summary}</strong>
        <span>{decisionJournal.health.detail}</span>
        <small>{decisionJournal.recordCount} decizii · recovery {decisionJournal.recoveryPlan.status}</small>
      </div>

      {#if needsAcknowledgement && !matchingAck}
        <label>
          <span>Diagnostic de revizuire</span>
          <textarea rows="2" bind:value={acknowledgementDiagnostic}></textarea>
        </label>
        <button class="action" type="button" disabled={!canAcknowledge} onclick={() => void acknowledgeRecoveryPlan()}>
          Confirmă revizuirea
        </button>
      {/if}

      {#if recoveryPlan?.status === "retention_review" && matchingAck}
        <label>
          <span>Diagnostic pentru retention</span>
          <textarea rows="2" bind:value={retentionDiagnostic}></textarea>
        </label>
        <button class="action" type="button" disabled={!canRetain} onclick={() => void executeRetention()}>
          Arhivează {recoveryPlan.retentionCandidateCount} decizii superseded
        </button>
      {/if}
    </section>
  {/if}

  {#if mutationError}<p class="error" role="alert">{mutationError}</p>{/if}
</section>

<style>
  .transition-control { display: grid; gap: 12px; padding: 12px; border: 1px solid var(--border); border-radius: 9px; background: var(--surface-3); }
  header,
  header > div,
  .policies article > div { display: flex; align-items: center; }
  header { justify-content: space-between; gap: 12px; }
  header > div { gap: 9px; }
  h2,
  h3,
  p { margin: 0; }
  h2 { font-size: 14px; }
  h3 { font-size: 12px; text-transform: uppercase; }
  header p,
  article p,
  article span,
  article small,
  .decision-summary span,
  .decision-summary small { color: var(--text-muted); font-size: 11px; line-height: 1.4; }
  header button { width: 36px; height: 34px; border: 1px solid var(--border); border-radius: 7px; background: var(--surface); color: var(--text); }
  .policies { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; }
  .policies article,
  .audit article,
  .decisions { display: grid; gap: 5px; padding: 9px; border: 1px solid var(--border); border-radius: 7px; background: var(--surface); }
  .policies article.blocked { border-color: color-mix(in srgb, #ef4444 42%, var(--border)); }
  .policies article.confirm { border-color: color-mix(in srgb, #f59e0b 42%, var(--border)); }
  .policies article > div { gap: 5px; }
  .policies strong { font-size: 11px; text-transform: uppercase; }
  .policies em { margin-left: auto; color: var(--text-muted); font-size: 10px; font-style: normal; }
  .audit { display: grid; gap: 7px; }
  .audit article { grid-template-columns: minmax(120px, auto) minmax(0, 1fr); }
  .audit small { grid-column: 1 / -1; }
  .decisions label { display: grid; gap: 5px; }
  .decisions label span { color: var(--text-muted); font-size: 10px; font-weight: 800; text-transform: uppercase; }
  textarea { width: 100%; box-sizing: border-box; resize: vertical; }
  .action { justify-self: start; min-height: 32px; border: 1px solid var(--border); border-radius: 7px; background: var(--surface-3); color: var(--text); }
  button:disabled { opacity: 0.55; }
  .error { color: #ef4444; font-size: 11px; }
  @media (max-width: 900px) { .policies { grid-template-columns: 1fr; } }
</style>
