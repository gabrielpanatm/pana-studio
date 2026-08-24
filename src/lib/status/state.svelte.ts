import type { NotificationCenterState } from "$lib/notifications/store.svelte";
import {
  publishKernelGlobalStatus,
  readKernelGlobalStatus,
  resolveKernelGlobalStatus,
} from "$lib/status/io";
import {
  applyGlobalStatusSnapshot,
  clearGlobalStatusExpiryTimer,
  currentGlobalStatus,
  publishGlobalStatus,
  type StatusControllerHost,
} from "$lib/status/controller";
import {
  globalStatusInputFromKind,
  type GlobalStatusEscalationRequest,
  type GlobalStatusEvent,
  type GlobalStatusInput,
  type GlobalStatusKind,
  type GlobalStatusPublishOptions,
  type GlobalStatusSnapshot,
} from "$lib/status/global-status";
import { t } from "$lib/i18n/runtime.svelte";

export class GlobalStatusState implements StatusControllerHost {
  globalStatusEvents = $state<GlobalStatusEvent[]>([]);
  globalStatusRevision = 0;
  globalStatusSequence = 0;
  globalStatusExpiryTimer: number | null = null;
  readonly notificationCenter: NotificationCenterState;
  private kernelTail: Promise<void> = Promise.resolve();

  constructor(notificationCenter: NotificationCenterState) {
    this.notificationCenter = notificationCenter;
  }

  get current() {
    return currentGlobalStatus(this);
  }

  set(
    text: string,
    kind: GlobalStatusKind,
    options: GlobalStatusPublishOptions = {},
  ) {
    const input = globalStatusInputFromKind(text, kind, options);
    void this.queueKernelCommand(() => publishKernelGlobalStatus(input));
  }

  escalate(notification: GlobalStatusEscalationRequest) {
    const input: GlobalStatusInput = {
      code: notification.id,
      source: notification.id.split(".")[0] || "application",
      severity: notification.level === "error"
        ? "error"
        : notification.level === "warning"
          ? "warning"
          : "info",
      phase: "settled",
      message: notification.statusMessage ?? notification.title,
      detail: notification.message,
      lifecycle: "until_resolved",
      escalation: "notification",
      dedupeKey: notification.id,
      resolutionKey: notification.id,
      notification: {
        title: notification.title,
        message: notification.message,
        level: notification.level,
        actionLabel: notification.actionLabel,
        actionId: notification.actionId,
        secondaryActionLabel: notification.secondaryActionLabel,
        secondaryActionId: notification.secondaryActionId,
      },
    };
    void this.queueKernelCommand(() => publishKernelGlobalStatus(input));
  }

  clear(id: string) {
    const hasOpenStatus = this.globalStatusEvents.some((event) => (
      event.resolution === "open"
      && (
        event.id === id
        || event.dedupeKey === id
        || event.resolutionKey === id
      )
    ));
    if (
      !this.notificationCenter.has(id)
      && !hasOpenStatus
      && !this.notificationCenter.wasDismissed(id)
    ) return;
    this.resolve(id);
  }

  /** Enqueues an authoritative resolution even if its publication is still in flight. */
  resolve(id: string) {
    void this.queueKernelCommand(() => resolveKernelGlobalStatus(id));
  }

  refreshGlobalStatusFromKernel() {
    return this.queueKernelCommand(readKernelGlobalStatus);
  }

  async settled() {
    await this.kernelTail;
  }

  destroy() {
    clearGlobalStatusExpiryTimer(this);
  }

  private projectKernelFailure(error: unknown) {
    const detail = error instanceof Error ? error.message : String(error);
    publishGlobalStatus(this, {
      code: "global-status.kernel-command-failed",
      source: "global-status",
      severity: "error",
      message: t("app-session-global-status-kernel-failed"),
      detail,
      lifecycle: "until_resolved",
      escalation: "notification",
      dedupeKey: "global-status.kernel",
      resolutionKey: "global-status.kernel",
      notification: {
        title: t("app-session-global-status-unavailable"),
        message: detail,
        level: "error",
      },
    });
  }

  private queueKernelCommand(command: () => Promise<GlobalStatusSnapshot>) {
    const operation = this.kernelTail
      .catch(() => undefined)
      .then(async () => {
        try {
          const snapshot = await command();
          applyGlobalStatusSnapshot(this, snapshot);
        } catch (error) {
          this.projectKernelFailure(error);
        }
      });
    this.kernelTail = operation;
    return operation;
  }
}
