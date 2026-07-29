export type PreviewFreshness =
  | "idle"
  | "live"
  | "saved"
  | "refreshing"
  | "canonical"
  | "stale"
  | "error";

export type ZolaValidationState =
  | "idle"
  | "queued"
  | "running"
  | "valid"
  | "invalid"
  | "error";

export type PreviewRefreshReason =
  | "manual"
  | "session-refresh"
  | "project-rescan"
  | "discard"
  | "external-change"
  | "workspace-mutation"
  | "tera-structural"
  | "html-structural"
  | "history-restore"
  | "after-save"
  | "unknown";

export type ZolaValidationReason =
  | "manual"
  | "save"
  | "refresh"
  | "project-open"
  | "external-change";

export type ControlledPreviewState = {
  freshness: PreviewFreshness;
  validation: ZolaValidationState;
  message: string;
  validationMessage: string;
  lastLiveAt: number | null;
  lastSavedAt: number | null;
  lastRefreshAt: number | null;
  lastValidatedAt: number | null;
  refreshReason: PreviewRefreshReason | null;
  validationReason: ZolaValidationReason | null;
};

export function createControlledPreviewState(): ControlledPreviewState {
  return {
    freshness: "idle",
    validation: "idle",
    message: t("controlled-preview-ready"),
    validationMessage: t("controlled-preview-zola-unvalidated-session"),
    lastLiveAt: null,
    lastSavedAt: null,
    lastRefreshAt: null,
    lastValidatedAt: null,
    refreshReason: null,
    validationReason: null,
  };
}

export function markPreviewLive(
  state: ControlledPreviewState,
  message = t("controlled-preview-live-updated"),
): ControlledPreviewState {
  return {
    ...state,
    freshness: "live",
    message,
    lastLiveAt: Date.now(),
  };
}

export function markPreviewSaved(
  state: ControlledPreviewState,
  message = t("controlled-preview-saved-live"),
): ControlledPreviewState {
  return {
    ...state,
    freshness: "saved",
    message,
    lastSavedAt: Date.now(),
  };
}

export function markPreviewRefreshing(
  state: ControlledPreviewState,
  reason: PreviewRefreshReason,
): ControlledPreviewState {
  return {
    ...state,
    freshness: "refreshing",
    message: previewRefreshReasonLabel(reason),
    refreshReason: reason,
  };
}

export function markPreviewCanonical(
  state: ControlledPreviewState,
  reason: PreviewRefreshReason,
): ControlledPreviewState {
  return {
    ...state,
    freshness: "canonical",
    message: t("controlled-preview-canonical", {
      reason: previewRefreshReasonShortLabel(reason),
    }),
    lastRefreshAt: Date.now(),
    refreshReason: reason,
  };
}

export function markPreviewRefreshError(
  state: ControlledPreviewState,
  reason: PreviewRefreshReason,
  message: string,
): ControlledPreviewState {
  return {
    ...state,
    freshness: "error",
    message,
    lastRefreshAt: Date.now(),
    refreshReason: reason,
  };
}

export function markZolaQueued(
  state: ControlledPreviewState,
  reason: ZolaValidationReason,
): ControlledPreviewState {
  return {
    ...state,
    validation: "queued",
    validationMessage: t("controlled-preview-zola-queued"),
    validationReason: reason,
  };
}

export function markZolaRunning(
  state: ControlledPreviewState,
  reason: ZolaValidationReason,
): ControlledPreviewState {
  return {
    ...state,
    validation: "running",
    validationMessage: t("controlled-preview-zola-running"),
    validationReason: reason,
  };
}

export function markZolaValid(
  state: ControlledPreviewState,
  reason: ZolaValidationReason,
  message = t("controlled-preview-zola-passed"),
): ControlledPreviewState {
  return {
    ...state,
    validation: "valid",
    validationMessage: message,
    validationReason: reason,
    lastValidatedAt: Date.now(),
  };
}

export function markZolaInvalid(
  state: ControlledPreviewState,
  reason: ZolaValidationReason,
  message: string,
): ControlledPreviewState {
  return {
    ...state,
    validation: "invalid",
    validationMessage: message,
    validationReason: reason,
    lastValidatedAt: Date.now(),
  };
}

export function previewRefreshReasonShortLabel(reason: PreviewRefreshReason) {
  switch (reason) {
    case "manual":
      return t("controlled-preview-reason-manual");
    case "session-refresh":
      return t("controlled-preview-reason-session");
    case "project-rescan":
      return t("controlled-preview-reason-project");
    case "discard":
      return t("controlled-preview-reason-disk");
    case "external-change":
      return t("controlled-preview-reason-external");
    case "workspace-mutation":
      return t("controlled-preview-reason-session");
    case "tera-structural":
      return "Tera";
    case "html-structural":
      return "HTML";
    case "history-restore":
      return t("controlled-preview-reason-history");
    case "after-save":
      return t("controlled-preview-reason-after-save");
    default:
      return t("controlled-preview-reason-unknown");
  }
}

function previewRefreshReasonLabel(reason: PreviewRefreshReason) {
  return t("controlled-preview-refreshing", {
    reason: previewRefreshReasonShortLabel(reason),
  });
}
import { t } from "$lib/i18n/runtime.svelte";
