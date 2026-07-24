<script lang="ts">
  import { IconX } from "@tabler/icons-svelte";
  import type { AppNotification } from "$lib/notifications/center";

  let {
    notifications = [],
    dismiss = () => {},
    save = () => {},
    action = undefined,
  }: {
    notifications?: AppNotification[];
    dismiss?: (id: string) => void;
    save?: () => void | Promise<unknown>;
    action?: (notification: AppNotification, actionId: string) => void | Promise<unknown>;
  } = $props();

  const orderedNotifications = $derived([...notifications].sort((a, b) => a.createdAt - b.createdAt));

  function runAction(notification: AppNotification, actionId?: string | null) {
    if (action) {
      void action(notification, actionId ?? notification.actionId ?? "save");
      return;
    }
    void save();
  }
</script>

{#if orderedNotifications.length > 0}
  <section class="notification-region" aria-label="Notificări aplicație" aria-live="polite">
    <div class="notification-stack">
      {#each orderedNotifications as notification (notification.id)}
        <article class="notification-card" class:warning={notification.level === "warning"} class:error={notification.level === "error"}>
          <div class="notification-body">
            <div class="notification-header">
              <span class="notification-level">{notification.level === "error" ? "Eroare" : notification.level === "warning" ? "Atenție" : "Info"}</span>
              <h2>{notification.title}</h2>
            </div>
            <p>{notification.message}</p>
            {#if notification.actionLabel || notification.secondaryActionLabel}
              <div class="notification-actions">
                {#if notification.actionLabel}
                  <button type="button" class="notification-action primary" onclick={() => runAction(notification, notification.actionId)}>
                    {notification.actionLabel}
                  </button>
                {/if}
                {#if notification.secondaryActionLabel}
                  <button
                    type="button"
                    class="notification-action secondary"
                    onclick={() => runAction(notification, notification.secondaryActionId)}
                  >
                    {notification.secondaryActionLabel}
                  </button>
                {/if}
              </div>
            {/if}
          </div>
          <button
            type="button"
            class="notification-close"
            aria-label={`Închide notificarea ${notification.title}`}
            title="Închide"
            onclick={() => dismiss(notification.id)}
          >
            <IconX size={14} stroke={1.9} />
          </button>
        </article>
      {/each}
    </div>
  </section>
{/if}

<style>
  .notification-region {
    position: fixed;
    left: 12px;
    right: 12px;
    bottom: 42px;
    z-index: 80;
    display: flex;
    justify-content: center;
    pointer-events: none;
  }

  .notification-stack {
    display: flex;
    flex-direction: column-reverse;
    gap: 8px;
    width: min(720px, 100%);
    max-height: min(56vh, calc(100vh - 72px));
    overflow-y: auto;
    padding: 2px;
    overscroll-behavior: contain;
    pointer-events: auto;
  }

  .notification-card {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 12px;
    padding: 12px;
    border: 1px solid var(--border-4);
    border-left: 4px solid var(--brand);
    border-radius: 8px;
    color: var(--text);
    background: color-mix(in srgb, var(--surface-2) 94%, transparent);
    box-shadow: var(--shadow);
  }

  .notification-card.warning {
    border-left-color: #d49b24;
  }

  .notification-card.error {
    border-left-color: #d44a4a;
  }

  .notification-body {
    display: grid;
    gap: 7px;
    min-width: 0;
  }

  .notification-header {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  .notification-level {
    flex: 0 0 auto;
    padding: 2px 6px;
    border: 1px solid var(--border-4);
    border-radius: 999px;
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 800;
    text-transform: uppercase;
  }

  h2 {
    min-width: 0;
    margin: 0;
    overflow-wrap: anywhere;
    color: var(--text-strong);
    font-size: 13px;
    font-weight: 800;
  }

  p {
    margin: 0;
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.45;
  }

  .notification-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .notification-action {
    justify-self: start;
    min-height: 32px;
    padding: 0 9px;
    border: 1px solid var(--border-4);
    border-radius: 6px;
    color: var(--text-strong);
    background: var(--surface);
    font-size: 12px;
    font-weight: 800;
    cursor: pointer;
  }

  .notification-action:hover {
    border-color: var(--brand);
  }

  .notification-action.secondary {
    color: var(--text-muted);
    background: transparent;
  }

  .notification-close {
    display: inline-grid;
    place-items: center;
    width: 32px;
    height: 32px;
    padding: 0;
    border: 1px solid var(--border-4);
    border-radius: 6px;
    color: var(--text-muted);
    background: transparent;
    font-size: 18px;
    line-height: 1;
    cursor: pointer;
  }

  .notification-close:hover {
    color: var(--text);
    background: var(--surface);
  }
</style>
