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
import type { ProjectScan, SaveState } from "$lib/types";
import { errorMessage } from "$lib/util";
import {
  beginPreviewRefreshLease,
  previewRefreshLeaseMatches,
  type PreviewRefreshLease,
  type PreviewRefreshLeaseHost,
} from "$lib/state/preview-controller";

export type ControlledPreviewControllerHost = PreviewRefreshLeaseHost & {
  controlledPreview: ControlledPreviewState;
  zolaValidationTimer: number | null;
  zolaValidationSerial: number;
  scannedProject: ProjectScan | null;
  projectStatus: string;
  reloadPreview: (lease?: PreviewRefreshLease) => Promise<boolean>;
  setGlobalStatus: (text: string, kind: SaveState) => void;
};

export type ControlledPreviewRefreshOptions = {
  publishFailure?: boolean;
};

export async function requestControlledPreviewRefresh(
  host: ControlledPreviewControllerHost,
  reason: PreviewRefreshReason,
  options: ControlledPreviewRefreshOptions = {},
) {
  if (!host.scannedProject?.isZola) return false;
  const lease = beginPreviewRefreshLease(host);
  if (!lease || !previewRefreshLeaseMatches(host, lease)) return false;
  host.controlledPreview = markPreviewRefreshing(host.controlledPreview, reason);
  try {
    const refreshed = await host.reloadPreview(lease);
    if (!previewRefreshLeaseMatches(host, lease)) return false;
    if (!refreshed) {
      const message = host.projectStatus.startsWith("Randarea previzualizării a eșuat:")
        ? host.projectStatus
        : "Reîmprospătarea previzualizării a eșuat: randarea curentă nu a putut fi reîncărcată.";
      host.controlledPreview = markPreviewRefreshError(host.controlledPreview, reason, message);
      host.projectStatus = message;
      if (options.publishFailure !== false) host.setGlobalStatus(message, "error");
      return false;
    }
    host.controlledPreview = markPreviewCanonical(host.controlledPreview, reason);
    if (reason !== "manual") {
      host.projectStatus = host.controlledPreview.message;
    }
    scheduleZolaValidation(host, reason === "external-change" ? "external-change" : "refresh");
    return true;
  } catch (error) {
    if (!previewRefreshLeaseMatches(host, lease)) return false;
    const message = `Reîmprospătarea previzualizării a eșuat: ${errorMessage(error)}`;
    host.controlledPreview = markPreviewRefreshError(host.controlledPreview, reason, message);
    host.projectStatus = message;
    if (options.publishFailure !== false) host.setGlobalStatus(message, "error");
    return false;
  }
}

export function scheduleZolaValidation(
  host: ControlledPreviewControllerHost,
  reason: ZolaValidationReason,
  delayMs = 900,
) {
  if (!host.scannedProject?.isZola || typeof window === "undefined") return;
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
  if (!host.scannedProject?.isZola) {
    host.setGlobalStatus("Verificarea Zola este disponibilă doar pentru proiecte Zola.", "error");
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
      ? "Se validează sursele salvate cu motorul Zola embedded..."
      : "Se confirmă revizia ProjectWorkspace în motorul Zola embedded...",
    "saving",
  );
  try {
    const log = validatesCanonicalDisk ? await zolaCheck() : await zolaCheckWorkspace();
    if (serial !== host.zolaValidationSerial) return false;
    const firstLine = log.split("\n").find((line) => line.trim().length > 0)?.trim();
    const message = firstLine || "Validarea Zola embedded a trecut.";
    host.controlledPreview = markZolaValid(
      host.controlledPreview,
      reason,
      message,
    );
    host.projectStatus = host.controlledPreview.validationMessage;
    host.setGlobalStatus(`Validare Zola embedded finalizată: proiect valid. ${message}`, "saved");
    return true;
  } catch (error) {
    if (serial !== host.zolaValidationSerial) return false;
    const message = errorMessage(error);
    host.controlledPreview = markZolaInvalid(
      host.controlledPreview,
      reason,
      message,
    );
    host.projectStatus = `Validarea Zola embedded a eșuat: ${message}`;
    host.setGlobalStatus(host.projectStatus, "error");
    return false;
  }
}
