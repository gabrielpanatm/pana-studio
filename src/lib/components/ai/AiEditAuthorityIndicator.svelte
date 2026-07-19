<script lang="ts">
  import type { AiCoordinationSnapshot } from "$lib/types";

  let {
    snapshot = null,
    externalReconciling = false,
    projectionRecoveryRequired = false,
  }: {
    snapshot?: AiCoordinationSnapshot | null;
    externalReconciling?: boolean;
    projectionRecoveryRequired?: boolean;
  } = $props();

  const activeClients = $derived(
    snapshot?.clients.filter((client) => client.presence === "active").length ?? 0,
  );
  const view = $derived.by(() => {
    const authority = snapshot?.authority;
    if (!authority) {
      return { tone: "offline", label: "Context Hub se conectează", detail: "Starea AI nu este încă disponibilă." };
    }
    if (projectionRecoveryRequired) {
      return {
        tone: "conflict",
        label: "Recovery — editarea este blocată",
        detail: "Discul este confirmat, dar proiecția UI trebuie reconstruită înainte de editare.",
      };
    }
    if (externalReconciling && authority.state === "user_active") {
      return {
        tone: "pending",
        label: "Se reconciliază discul",
        detail: "Interacțiunile de editare revin numai după confirmarea proiecției UI.",
      };
    }
    switch (authority.state) {
      case "user_active":
        return {
          tone: "user",
          label: "Utilizatorul poate edita",
          detail: activeClients > 0
            ? `${activeClients} sesiune(i) AI observă aplicația; scrierea rămâne la utilizator.`
            : "Nicio sesiune AI activă nu deține scrierea.",
        };
      case "ai_requested":
        return {
          tone: "pending",
          label: "Se transferă autoritatea",
          detail: "Pană Studio verifică dirty state și quiescence.",
        };
      case "ai_active":
        return {
          tone: "ai",
          label: "AI editează sursele",
          detail: `Lease ${authority.detail.lease.id}`,
        };
      case "ai_orphaned":
        return {
          tone: "conflict",
          label: "Sesiune AI întreruptă — ambele părți sunt blocate",
          detail: authority.detail.reason,
        };
      case "reconciling":
        return {
          tone: "pending",
          label: "Se reconciliază discul",
          detail: authority.detail.reason,
        };
      case "conflict":
        return {
          tone: "conflict",
          label: "Conflict — editarea este blocată",
          detail: authority.detail.reason,
        };
    }
  });
</script>

<aside class="ai-authority" class:user={view.tone === "user"} class:pending={view.tone === "pending"} class:ai={view.tone === "ai"} class:conflict={view.tone === "conflict"} aria-live="polite" title={view.detail}>
  <span class="lamp" aria-hidden="true"></span>
  <span class="copy">
    <strong>{view.label}</strong>
    <small>{view.detail}</small>
  </span>
</aside>

<style>
  .ai-authority {
    position: fixed;
    z-index: 10000;
    right: 14px;
    bottom: 12px;
    display: flex;
    align-items: center;
    gap: 9px;
    max-width: min(430px, calc(100vw - 28px));
    padding: 8px 11px;
    border: 1px solid var(--border, #3a414d);
    border-radius: 10px;
    background: color-mix(in srgb, var(--surface-1, #171a20) 94%, transparent);
    box-shadow: 0 8px 28px rgb(0 0 0 / 28%);
    color: var(--text, #f2f4f8);
    backdrop-filter: blur(12px);
    pointer-events: none;
  }

  .lamp {
    flex: 0 0 auto;
    width: 11px;
    height: 11px;
    border-radius: 50%;
    background: #78808c;
    box-shadow: 0 0 0 4px rgb(120 128 140 / 14%);
  }

  .user .lamp { background: #35c46a; box-shadow: 0 0 0 4px rgb(53 196 106 / 16%); }
  .pending .lamp { background: #e5ac38; box-shadow: 0 0 0 4px rgb(229 172 56 / 16%); }
  .ai .lamp { background: #7c8cff; box-shadow: 0 0 0 4px rgb(124 140 255 / 18%); }
  .conflict .lamp { background: #ef5b62; box-shadow: 0 0 0 4px rgb(239 91 98 / 18%); }

  .copy { display: grid; min-width: 0; gap: 1px; }
  strong { font-size: 11px; line-height: 1.2; }
  small {
    overflow: hidden;
    color: var(--text-muted, #a8afba);
    font-size: 9px;
    line-height: 1.25;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
