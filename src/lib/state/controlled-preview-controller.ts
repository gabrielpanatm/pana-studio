import { zolaCheck, zolaCheckWorkspace } from "$lib/project/io";
import {
  markPreviewCanonical,
  markPreviewRefreshError,
  markPreviewRefreshing,
  markZolaInvalid,
  markZolaQueued,
  markZolaRunning,
  markZolaValid,
  type ControlledPreviewState,
  type PreviewRefreshReason,
  type ZolaValidationReason,
} from "$lib/preview/controlled";
import type { ProjectScan } from "$lib/types";
import type {
  GlobalStatusKind,
  GlobalStatusPublishOptions,
} from "$lib/status/global-status";
import { errorMessage } from "$lib/util";
import {
  beginPreviewRefreshLease,
  previewRefreshLeaseMatches,
  type PreviewRefreshLease,
  type PreviewRefreshLeaseHost,
} from "$lib/state/preview-controller";
import { t } from "$lib/i18n/runtime.svelte";

const WORKSPACE_PREVIEW_PENDING_PREFIX = "PANA_WORKSPACE_PREVIEW_PENDING:";

export type ControlledPreviewControllerHost = PreviewRefreshLeaseHost & {
  controlledPreview: ControlledPreviewState;
  zolaValidationTimer: number | null;
  zolaValidationSerial: number;
  scannedProject: ProjectScan | null;
  projectStatus: string;
  reloadPreview: (lease?: PreviewRefreshLease) => Promise<boolean>;
  setGlobalStatus: (
    text: string,
    kind: GlobalStatusKind,
    options?: GlobalStatusPublishOptions,
  ) => void;
};

export type ControlledPreviewRefreshOptions = {
  publishFailure?: boolean;
};

export async function requestControlledPreviewRefresh(
  host: ControlledPreviewControllerHost,
  reason: PreviewRefreshReason,
  options: ControlledPreviewRefreshOptions = {},
) {
  if (!host.scannedProject) return false;
  const lease = beginPreviewRefreshLease(host);
  if (!lease || !previewRefreshLeaseMatches(host, lease)) return false;
  const statusBeforeRefresh = host.projectStatus;
  host.controlledPreview = markPreviewRefreshing(host.controlledPreview, reason);
  if (options.publishFailure !== false) {
    host.setGlobalStatus(host.controlledPreview.message, "saving", {
      code: "preview.refresh",
      source: "preview",
      lifecycle: "until_replaced",
      escalation: "status_only",
      dedupeKey: "preview.refresh",
      resolutionKey: "preview.refresh",
    });
  }
  try {
    const refreshed = await host.reloadPreview(lease);
    if (!previewRefreshLeaseMatches(host, lease)) return false;
    if (!refreshed) {
      const message = host.projectStatus !== statusBeforeRefresh
        ? host.projectStatus
        : t("controlled-preview-refresh-current-failed");
      host.controlledPreview = markPreviewRefreshError(host.controlledPreview, reason, message);
      host.projectStatus = message;
      if (options.publishFailure !== false) {
        host.setGlobalStatus(message, "error", {
          code: "preview.refresh-failed",
          source: "preview",
          dedupeKey: "preview.refresh",
          resolutionKey: "preview.refresh",
        });
      }
      return false;
    }
    host.controlledPreview = markPreviewCanonical(host.controlledPreview, reason);
    if (options.publishFailure !== false) {
      host.setGlobalStatus(host.controlledPreview.message, "restored", {
        code: "preview.canonical",
        source: "preview",
        dedupeKey: "preview.refresh",
        resolutionKey: "preview.refresh",
      });
    }
    if (reason !== "manual") {
      host.projectStatus = host.controlledPreview.message;
    }
    scheduleZolaValidation(host, reason === "external-change" ? "external-change" : "refresh");
    return true;
  } catch (error) {
    if (!previewRefreshLeaseMatches(host, lease)) return false;
    const message = t("controlled-preview-refresh-failed", {
      message: errorMessage(error),
    });
    host.controlledPreview = markPreviewRefreshError(host.controlledPreview, reason, message);
    host.projectStatus = message;
    if (options.publishFailure !== false) {
      host.setGlobalStatus(message, "error", {
        code: "preview.refresh-failed",
        source: "preview",
        dedupeKey: "preview.refresh",
        resolutionKey: "preview.refresh",
      });
    }
    return false;
  }
}

export function scheduleZolaValidation(
  host: ControlledPreviewControllerHost,
  reason: ZolaValidationReason,
  delayMs = 900,
) {
  if (!host.scannedProject || typeof window === "undefined") return;
  if (host.zolaValidationTimer !== null) {
    window.clearTimeout(host.zolaValidationTimer);
  }
  host.controlledPreview = markZolaQueued(host.controlledPreview, reason);
  host.zolaValidationTimer = window.setTimeout(() => {
    host.zolaValidationTimer = null;
    void runZolaValidation(host, reason);
  }, delayMs);
}

export async function runZolaValidation(
  host: ControlledPreviewControllerHost,
  reason: ZolaValidationReason,
) {
  if (!host.scannedProject) {
    host.setGlobalStatus(t("controlled-preview-zola-project-only"), "error", {
      code: "zola.validation-project-required",
      source: "zola",
      dedupeKey: "zola.validation",
      resolutionKey: "zola.validation",
    });
    return false;
  }
  if (host.zolaValidationTimer !== null && typeof window !== "undefined") {
    window.clearTimeout(host.zolaValidationTimer);
    host.zolaValidationTimer = null;
  }

  const serial = ++host.zolaValidationSerial;
  host.controlledPreview = markZolaRunning(host.controlledPreview, reason);
  const validatesCanonicalDisk = reason === "manual";
  host.setGlobalStatus(
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
    if (serial !== host.zolaValidationSerial) return false;
    const firstLine = log.split("\n").find((line) => line.trim().length > 0)?.trim();
    const message = firstLine || t("controlled-preview-zola-passed");
    host.controlledPreview = markZolaValid(
      host.controlledPreview,
      reason,
      message,
    );
    host.projectStatus = host.controlledPreview.validationMessage;
    host.setGlobalStatus(t("controlled-preview-zola-complete", { message }), "saved", {
      code: "zola.validation-valid",
      source: "zola",
      dedupeKey: "zola.validation",
      resolutionKey: "zola.validation",
    });
    return true;
  } catch (error) {
    if (serial !== host.zolaValidationSerial) return false;
    const message = errorMessage(error);
    if (!validatesCanonicalDisk && message.startsWith(WORKSPACE_PREVIEW_PENDING_PREFIX)) {
      const pendingMessage = message.slice(WORKSPACE_PREVIEW_PENDING_PREFIX.length).trim();
      host.controlledPreview = {
        ...markZolaQueued(host.controlledPreview, reason),
        validationMessage: pendingMessage,
      };
      host.projectStatus = pendingMessage;
      host.setGlobalStatus(pendingMessage, "unsaved", {
        code: "zola.validation-queued",
        source: "zola",
        lifecycle: "until_replaced",
        escalation: "status_only",
        dedupeKey: "zola.validation",
        resolutionKey: "zola.validation",
      });
      return false;
    }
    host.controlledPreview = markZolaInvalid(
      host.controlledPreview,
      reason,
      message,
    );
    host.projectStatus = t("controlled-preview-zola-failed", { message });
    host.setGlobalStatus(host.projectStatus, "error", {
      code: "zola.validation-invalid",
      source: "zola",
      dedupeKey: "zola.validation",
      resolutionKey: "zola.validation",
    });
    return false;
  }
}
