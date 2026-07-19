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
  | "site-workspace"
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
    message: "Preview pregătit.",
    validationMessage: "Zola nevalidat în această sesiune.",
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
  message = "Preview live actualizat de Pană Studio.",
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
  message = "Fișiere salvate pe disk. Preview-ul live rămâne activ.",
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
    message: `Randare Zola reîmprospătată: ${previewRefreshReasonShortLabel(reason)}.`,
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
    validationMessage: "Validarea Zola este programată.",
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
    validationMessage: "Se rulează zola check în fundal.",
    validationReason: reason,
  };
}

export function markZolaValid(
  state: ControlledPreviewState,
  reason: ZolaValidationReason,
  message = "Zola check a trecut.",
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

export function previewFreshnessLabel(state: ControlledPreviewState) {
  switch (state.freshness) {
    case "live":
      return "Preview live";
    case "saved":
      return "Salvat pe disk";
    case "refreshing":
      return "Refresh Zola";
    case "canonical":
      return "Randare Zola";
    case "stale":
      return "Preview nevalidat";
    case "error":
      return "Preview eroare";
    default:
      return "Preview";
  }
}

export function zolaValidationLabel(state: ControlledPreviewState) {
  switch (state.validation) {
    case "queued":
      return "Zola în coadă";
    case "running":
      return "Zola check";
    case "valid":
      return "Zola valid";
    case "invalid":
      return "Zola invalid";
    case "error":
      return "Zola eroare";
    default:
      return "Zola nevalidat";
  }
}

export function previewRefreshReasonShortLabel(reason: PreviewRefreshReason) {
  switch (reason) {
    case "manual":
      return "manual";
    case "session-refresh":
      return "sesiune";
    case "project-rescan":
      return "proiect";
    case "discard":
      return "disk";
    case "external-change":
      return "schimbări externe";
    case "workspace-mutation":
      return "ProjectWorkspace";
    case "tera-structural":
      return "Tera";
    case "html-structural":
      return "HTML";
    case "history-restore":
      return "History";
    case "site-workspace":
      return "Site Editor";
    case "after-save":
      return "după Save";
    default:
      return "necunoscut";
  }
}

function previewRefreshReasonLabel(reason: PreviewRefreshReason) {
  return `Se reîmprospătează randarea Zola (${previewRefreshReasonShortLabel(reason)}).`;
}
