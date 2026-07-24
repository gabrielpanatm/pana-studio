<script lang="ts">
  import {
    IconAlertTriangle,
    IconArrowRight,
    IconShieldCheck,
    IconX,
  } from "@tabler/icons-svelte";
  import {
    projectTransitionDecisionMetrics,
    transitionActionLabel,
    transitionReasonLabel,
    type ProjectTransitionDecisionRequest,
  } from "$lib/project/transition-decision";
  import { errorMessage } from "$lib/util";

  let {
    request = null,
    confirm,
    cancel,
  }: {
    request?: ProjectTransitionDecisionRequest | null;
    confirm: (requestId: string, diagnostic: string) => Promise<void>;
    cancel: (requestId: string) => void;
  } = $props();

  let activeRequestId = $state<string | null>(null);
  let diagnostic = $state("");
  let submitting = $state(false);
  let error = $state<string | null>(null);

  const metrics = $derived(request ? projectTransitionDecisionMetrics(request.policy) : []);
  const canSubmit = $derived(Boolean(request) && diagnostic.trim().length >= 12 && !submitting);

  $effect(() => {
    if (request?.id === activeRequestId) return;
    activeRequestId = request?.id ?? null;
    diagnostic = "";
    submitting = false;
    error = null;
  });

  function close() {
    if (!request || submitting) return;
    cancel(request.id);
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    if (!request) return;
    if (event.key === "Escape") {
      event.preventDefault();
      close();
    }
    if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
      event.preventDefault();
      void submit();
    }
  }

  async function submit() {
    if (!request || !canSubmit) return;
    submitting = true;
    error = null;
    try {
      await confirm(request.id, diagnostic.trim());
    } catch (submitError) {
      error = errorMessage(submitError);
      submitting = false;
    }
  }
</script>

<svelte:window onkeydown={handleWindowKeydown} />

{#if request}
  <div class="transition-backdrop" role="presentation" onclick={close}></div>
  <div
    class="transition-dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby="project-transition-title"
    aria-describedby="project-transition-message"
  >
    <header class="dialog-header">
      <div class="dialog-title">
        <span class="dialog-icon" aria-hidden="true">
          <IconShieldCheck size={18} stroke={1.9} />
        </span>
        <div>
          <p>{transitionActionLabel(request.action)}</p>
          <h2 id="project-transition-title">{request.policy.title}</h2>
        </div>
      </div>
      <button type="button" class="icon-button" title="Anulează tranziția" disabled={submitting} onclick={close}>
        <IconX size={16} stroke={1.9} />
      </button>
    </header>

    <div class="policy-summary">
      <span class="reason-pill">{transitionReasonLabel(request.policy.reason)}</span>
      <p id="project-transition-message">{request.policy.message}</p>
      <p class="target-path">{request.targetRoot}</p>
    </div>

    <div class="metric-list" aria-label="Evidență stare proiect">
      {#each metrics as metric}
        <div class="metric-row" class:warning={metric.tone === "warning"} class:danger={metric.tone === "danger"}>
          <span>{metric.label}</span>
          <strong>{metric.value}</strong>
        </div>
      {/each}
    </div>

    <section class="evidence-block" aria-label="Evidență nucleu Rust">
      <div class="section-heading">
        <IconAlertTriangle size={15} stroke={1.8} />
        <h3>Evidență nucleu Rust</h3>
      </div>
      <p>{request.policy.evidence}</p>
      <p class="recommendation">{request.policy.recommendedAction}</p>
    </section>

    <label class="diagnostic-field">
      <span>Diagnostic operator</span>
      <textarea
        bind:value={diagnostic}
        rows="4"
        disabled={submitting}
        placeholder="Ex.: Confirm tranziția după revizuirea ciornelor locale și accept pierderea sesiunii curente."
      ></textarea>
    </label>

    {#if error}
      <p class="error-message">{error}</p>
    {/if}

    <footer class="dialog-actions">
      <button type="button" class="secondary-button" disabled={submitting} onclick={close}>Anulează</button>
      <button type="button" class="primary-button" disabled={!canSubmit} onclick={() => void submit()}>
        {#if submitting}
          Se înregistrează...
        {:else}
          <IconArrowRight size={15} stroke={1.9} />
          Înregistrează și continuă
        {/if}
      </button>
    </footer>
  </div>
{/if}

<style>
  .transition-backdrop {
    position: fixed;
    inset: 0;
    z-index: 88;
    background: rgba(8, 12, 11, 0.5);
    backdrop-filter: blur(2px);
  }

  .transition-dialog {
    position: fixed;
    top: 50%;
    left: 50%;
    z-index: 89;
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
  .section-heading,
  .dialog-actions {
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
    border: 1px solid color-mix(in srgb, var(--brand) 50%, var(--border-4));
    border-radius: 8px;
    color: var(--brand-strong);
    background: var(--brand-soft);
  }

  .dialog-title p,
  .dialog-title h2,
  .policy-summary p,
  .evidence-block p,
  .section-heading h3,
  .error-message {
    margin: 0;
  }

  .dialog-title p {
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 850;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .dialog-title h2 {
    margin-top: 2px;
    overflow-wrap: anywhere;
    color: var(--text-strong);
    font-size: 18px;
    font-weight: 850;
    letter-spacing: 0;
  }

  .icon-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    padding: 0;
    border: 1px solid var(--border-3);
    border-radius: 7px;
    background: var(--surface-4);
    color: var(--text-muted);
    cursor: pointer;
  }

  .icon-button:hover:not(:disabled) {
    color: var(--text);
    border-color: var(--border-4);
  }

  .icon-button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .policy-summary {
    display: grid;
    gap: 8px;
    padding: 12px;
    border: 1px solid var(--border-3);
    border-radius: 8px;
    background: var(--surface-2);
  }

  .reason-pill {
    justify-self: start;
    padding: 3px 7px;
    border: 1px solid #d49b24;
    border-radius: 999px;
    color: #f3bd55;
    font-size: 12px;
    font-weight: 850;
    text-transform: uppercase;
  }

  .policy-summary p {
    color: var(--text);
    font-size: 13px;
    line-height: 1.45;
  }

  .policy-summary .target-path {
    padding-top: 2px;
    overflow-wrap: anywhere;
    color: var(--text-muted);
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace;
    font-size: 12px;
  }

  .metric-list {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
  }

  .metric-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    min-width: 0;
    padding: 8px 9px;
    border: 1px solid var(--border-3);
    border-radius: 7px;
    background: var(--surface-2);
  }

  .metric-row span {
    min-width: 0;
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 750;
  }

  .metric-row strong {
    color: var(--text-strong);
    font-size: 12px;
    font-weight: 900;
  }

  .metric-row.warning {
    border-color: color-mix(in srgb, #d49b24 45%, var(--border-3));
  }

  .metric-row.danger {
    border-color: color-mix(in srgb, #d44a4a 55%, var(--border-3));
  }

  .section-heading {
    gap: 7px;
    color: var(--text-muted);
  }

  .section-heading h3 {
    font-size: 12px;
    font-weight: 850;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .evidence-block {
    display: grid;
    gap: 8px;
    padding: 11px;
    border: 1px solid var(--border-3);
    border-radius: 8px;
    background: color-mix(in srgb, var(--surface-3) 72%, transparent);
  }

  .evidence-block p {
    overflow-wrap: anywhere;
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.45;
  }

  .evidence-block .recommendation {
    color: var(--text);
  }

  .diagnostic-field {
    display: grid;
    gap: 7px;
  }

  .diagnostic-field span {
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 850;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  textarea {
    width: 100%;
    min-height: 96px;
    resize: vertical;
    padding: 10px;
    border: 1px solid var(--border-4);
    border-radius: 7px;
    color: var(--text);
    background: var(--surface-2);
    font: inherit;
    font-size: 13px;
    line-height: 1.45;
  }

  textarea:focus {
    outline: 2px solid color-mix(in srgb, var(--brand) 45%, transparent);
    outline-offset: 1px;
  }

  .error-message {
    padding: 8px 10px;
    border: 1px solid color-mix(in srgb, #d44a4a 60%, var(--border-3));
    border-radius: 7px;
    color: #f5a5a5;
    background: color-mix(in srgb, #d44a4a 12%, transparent);
    font-size: 12px;
    line-height: 1.4;
  }

  .dialog-actions {
    justify-content: flex-end;
    gap: 8px;
  }

  .secondary-button,
  .primary-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    min-height: 32px;
    padding: 0 12px;
    border: 1px solid var(--border-4);
    border-radius: 7px;
    font-size: 12px;
    font-weight: 850;
    cursor: pointer;
  }

  .secondary-button {
    color: var(--text-muted);
    background: transparent;
  }

  .primary-button {
    color: #ffffff;
    border-color: var(--brand);
    background: var(--brand);
  }

  .secondary-button:hover:not(:disabled) {
    color: var(--text);
    background: var(--surface-3);
  }

  .primary-button:hover:not(:disabled) {
    border-color: var(--brand-strong);
    background: var(--brand-strong);
  }

  .secondary-button:disabled,
  .primary-button:disabled {
    opacity: 0.48;
    cursor: not-allowed;
  }

  @media (max-width: 680px) {
    .transition-dialog {
      top: 16px;
      transform: translateX(-50%);
    }

    .metric-list {
      grid-template-columns: 1fr;
    }

    .dialog-actions {
      flex-direction: column-reverse;
      align-items: stretch;
    }
  }
</style>
