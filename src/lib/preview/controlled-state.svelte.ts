import { t } from "$lib/i18n/runtime.svelte";
import {
  zolaCheck,
  zolaCheckWorkspace,
} from "$lib/project/io/zola";
import {
  createControlledPreviewState,
  markPreviewCanonical,
  markPreviewLive,
  markPreviewRefreshError,
  markPreviewRefreshing,
  markPreviewSaved,
  markZolaInvalid,
  markZolaQueued,
  markZolaRunning,
  markZolaValid,
  type ControlledPreviewState,
  type PreviewRefreshReason,
  type ZolaValidationReason,
} from "$lib/preview/controlled";
import type { PreviewRefreshLease } from "$lib/state/preview-controller";
import type {
  GlobalStatusKind,
  GlobalStatusPublishOptions,
} from "$lib/status/global-status";
import { errorMessage } from "$lib/util";

const WORKSPACE_PREVIEW_PENDING_PREFIX = "PANA_WORKSPACE_PREVIEW_PENDING:";

export type ControlledPreviewContext = Readonly<{
  projectPresent: boolean;
  projectStatus: string;
}>;

export type ControlledPreviewCommands = {
  context: () => ControlledPreviewContext;
  beginRefreshLease: () => PreviewRefreshLease | null;
  refreshLeaseCurrent: (lease: PreviewRefreshLease) => boolean;
  reloadPreview: (lease: PreviewRefreshLease) => Promise<boolean>;
  setProjectStatus: (status: string) => void;
  setGlobalStatus: (
    text: string,
    kind: GlobalStatusKind,
    options?: GlobalStatusPublishOptions,
  ) => void;
};

/** Owns controlled-preview refresh and Zola validation state/timers. */
export class ControlledPreviewWorkspaceState {
  snapshot = $state<ControlledPreviewState>(createControlledPreviewState());
  validationTimer: number | null = null;
  validationSerial = 0;
  private readonly commands: ControlledPreviewCommands;

  constructor(commands: ControlledPreviewCommands) {
    this.commands = commands;
  }

  reset() {
    if (this.validationTimer !== null && typeof window !== "undefined") {
      window.clearTimeout(this.validationTimer);
    }
    this.validationTimer = null;
    this.validationSerial += 1;
    this.snapshot = createControlledPreviewState();
  }

  markLive(message?: string) {
    this.snapshot = markPreviewLive(this.snapshot, message);
  }

  markSaved(message?: string) {
    this.snapshot = markPreviewSaved(this.snapshot, message);
  }

  async requestRefresh(
    reason: PreviewRefreshReason,
    options: { publishFailure?: boolean } = {},
  ) {
    if (!this.commands.context().projectPresent) return false;
    const lease = this.commands.beginRefreshLease();
    if (!lease || !this.commands.refreshLeaseCurrent(lease)) return false;
    const statusBeforeRefresh = this.commands.context().projectStatus;
    this.snapshot = markPreviewRefreshing(this.snapshot, reason);
    if (options.publishFailure !== false) {
      this.commands.setGlobalStatus(this.snapshot.message, "saving", {
        code: "preview.refresh",
        source: "preview",
        lifecycle: "until_replaced",
        escalation: "status_only",
        dedupeKey: "preview.refresh",
        resolutionKey: "preview.refresh",
      });
    }
    try {
      const refreshed = await this.commands.reloadPreview(lease);
      if (!this.commands.refreshLeaseCurrent(lease)) return false;
      if (!refreshed) {
        const currentStatus = this.commands.context().projectStatus;
        const message = currentStatus !== statusBeforeRefresh
          ? currentStatus
          : t("controlled-preview-refresh-current-failed");
        this.snapshot = markPreviewRefreshError(this.snapshot, reason, message);
        this.commands.setProjectStatus(message);
        if (options.publishFailure !== false) {
          this.commands.setGlobalStatus(message, "error", {
            code: "preview.refresh-failed",
            source: "preview",
            dedupeKey: "preview.refresh",
            resolutionKey: "preview.refresh",
          });
        }
        return false;
      }
      this.snapshot = markPreviewCanonical(this.snapshot, reason);
      if (options.publishFailure !== false) {
        this.commands.setGlobalStatus(this.snapshot.message, "restored", {
          code: "preview.canonical",
          source: "preview",
          dedupeKey: "preview.refresh",
          resolutionKey: "preview.refresh",
        });
      }
      if (reason !== "manual") this.commands.setProjectStatus(this.snapshot.message);
      this.scheduleValidation(reason === "external-change" ? "external-change" : "refresh");
      return true;
    } catch (error) {
      if (!this.commands.refreshLeaseCurrent(lease)) return false;
      const message = t("controlled-preview-refresh-failed", { message: errorMessage(error) });
      this.snapshot = markPreviewRefreshError(this.snapshot, reason, message);
      this.commands.setProjectStatus(message);
      if (options.publishFailure !== false) {
        this.commands.setGlobalStatus(message, "error", {
          code: "preview.refresh-failed",
          source: "preview",
          dedupeKey: "preview.refresh",
          resolutionKey: "preview.refresh",
        });
      }
      return false;
    }
  }

  scheduleValidation(reason: ZolaValidationReason, delayMs = 900) {
    if (!this.commands.context().projectPresent || typeof window === "undefined") return;
    if (this.validationTimer !== null) window.clearTimeout(this.validationTimer);
    this.snapshot = markZolaQueued(this.snapshot, reason);
    this.validationTimer = window.setTimeout(() => {
      this.validationTimer = null;
      void this.runValidation(reason);
    }, delayMs);
  }

  async runValidation(reason: ZolaValidationReason) {
    if (!this.commands.context().projectPresent) {
      this.commands.setGlobalStatus(t("controlled-preview-zola-project-only"), "error", {
        code: "zola.validation-project-required",
        source: "zola",
        dedupeKey: "zola.validation",
        resolutionKey: "zola.validation",
      });
      return false;
    }
    if (this.validationTimer !== null && typeof window !== "undefined") {
      window.clearTimeout(this.validationTimer);
      this.validationTimer = null;
    }
    const serial = ++this.validationSerial;
    this.snapshot = markZolaRunning(this.snapshot, reason);
    const validatesCanonicalDisk = reason === "manual";
    this.commands.setGlobalStatus(
      validatesCanonicalDisk
        ? t("controlled-preview-zola-validating-disk")
        : t("controlled-preview-zola-validating-workspace"),
      "saving",
      {
        code: "zola.validation-running",
        source: "zola",
        lifecycle: "until_replaced",
        escalation: "status_only",
        dedupeKey: "zola.validation",
        resolutionKey: "zola.validation",
      },
    );
    try {
      const log = validatesCanonicalDisk ? await zolaCheck() : await zolaCheckWorkspace();
      if (serial !== this.validationSerial) return false;
      const firstLine = log.split("\n").find((line) => line.trim().length > 0)?.trim();
      const message = firstLine || t("controlled-preview-zola-passed");
      this.snapshot = markZolaValid(this.snapshot, reason, message);
      this.commands.setProjectStatus(this.snapshot.validationMessage);
      this.commands.setGlobalStatus(t("controlled-preview-zola-complete", { message }), "saved", {
        code: "zola.validation-valid",
        source: "zola",
        dedupeKey: "zola.validation",
        resolutionKey: "zola.validation",
      });
      return true;
    } catch (error) {
      if (serial !== this.validationSerial) return false;
      const message = errorMessage(error);
      if (!validatesCanonicalDisk && message.startsWith(WORKSPACE_PREVIEW_PENDING_PREFIX)) {
        const pendingMessage = message.slice(WORKSPACE_PREVIEW_PENDING_PREFIX.length).trim();
        this.snapshot = {
          ...markZolaQueued(this.snapshot, reason),
          validationMessage: pendingMessage,
        };
        this.commands.setProjectStatus(pendingMessage);
        this.commands.setGlobalStatus(pendingMessage, "unsaved", {
          code: "zola.validation-queued",
          source: "zola",
          lifecycle: "until_replaced",
          escalation: "status_only",
          dedupeKey: "zola.validation",
          resolutionKey: "zola.validation",
        });
        return false;
      }
      this.snapshot = markZolaInvalid(this.snapshot, reason, message);
      const status = t("controlled-preview-zola-failed", { message });
      this.commands.setProjectStatus(status);
      this.commands.setGlobalStatus(status, "error", {
        code: "zola.validation-invalid",
        source: "zola",
        dedupeKey: "zola.validation",
        resolutionKey: "zola.validation",
      });
      return false;
    }
  }
}
