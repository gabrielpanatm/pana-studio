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
        label: "Recuperare — editarea este blocată",
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

<div
  class="ai-authority"
  class:user={view.tone === "user"}
  class:pending={view.tone === "pending"}
  class:ai={view.tone === "ai"}
  class:conflict={view.tone === "conflict"}
  aria-label={`${view.label}. ${view.detail}`}
  title={view.detail}
>
  <span class="lamp" aria-hidden="true"></span>
  <span class="copy">
    <strong>{view.label}</strong>
  </span>
</div>

<style>
  .ai-authority {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    max-width: 210px;
    color: var(--text-muted);
  }

  .lamp {
    flex: 0 0 auto;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #78808c;
    box-shadow: 0 0 0 3px rgb(120 128 140 / 14%);
  }

  .user .lamp { background: #35c46a; box-shadow: 0 0 0 3px rgb(53 196 106 / 16%); }
  .pending .lamp { background: #e5ac38; box-shadow: 0 0 0 3px rgb(229 172 56 / 16%); }
  .ai .lamp { background: #7c8cff; box-shadow: 0 0 0 3px rgb(124 140 255 / 18%); }
  .conflict .lamp { background: #ef5b62; box-shadow: 0 0 0 3px rgb(239 91 98 / 18%); }

  .copy,
  strong {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  strong {
    display: block;
    font-size: 12px;
    font-weight: 700;
    line-height: 1.2;
  }

  .conflict strong { color: #ef5b62; }
  .pending strong { color: #b7791f; }
  .ai strong { color: #6366f1; }
</style>
