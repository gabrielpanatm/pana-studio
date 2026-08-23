export type CanvasProjectionPhase =
  | "prepared"
  | "resourcesReady"
  | "committed"
  | "styledReady"
  | "canonicalVerified"
  | "failed";

export type CanvasProjectionIdentity = {
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number;
  transactionId: string;
  previewRevision: string;
};

export type CanvasResourceEntry = {
  url: string;
  contentHash: string;
  sizeBytes: number;
  contentType: string;
  kind: "stylesheet" | "script" | "font" | "image" | "media" | "other";
};

export type CanvasProjectionPlan = {
  schemaVersion: number;
  identity: CanvasProjectionIdentity;
  workspaceTransactionId: string | null;
  phase: CanvasProjectionPhase;
  impact: {
    kinds: string[];
    paths: string[];
    requiresFullDocument: boolean;
  };
  resources: {
    schemaVersion: number;
    previewRevision: string;
    totalBytes: number;
    entries: CanvasResourceEntry[];
  };
};

export type PreviewPhaseReceipt = {
  schemaVersion: number;
  identity: CanvasProjectionIdentity;
  phase: "resourcesReady" | "committed" | "styledReady" | "failed";
  phaseTimingsMs: Record<string, number>;
  diagnostic: string | null;
};

export type PreviewRuntimeEventKind =
  | "interactive_js_restarted"
  | "interactive_js_failed"
  | "canvas_patch_applied"
  | "canvas_patch_refused"
  | "canvas_patch_rolled_back"
  | "canvas_drag_preview_applied"
  | "canvas_drag_preview_skipped"
  | "canvas_fallback"
  | "canvas_stylesheets_promoted"
  | "canvas_ack_timeout";

export type PreviewStylesheetPromotionMetrics = {
  reused: number;
  staged: number;
  retired: number;
  preloadsReused?: number;
  preloadsStaged?: number;
  preloadsRetired?: number;
  headNodesReused?: number;
  headNodesCreated?: number;
  headNodesRetired?: number;
  headNodesReordered?: number;
  stylesheetAttributeMutations?: number;
  preloadAttributeMutations?: number;
  fontInvalidationCount?: number;
  fontFallbackFrames?: number;
  maxTextMetricDelta?: number;
  fontActivationErrorCount?: number;
  fontActivationDiagnostic?: string | null;
  fontsReadyMs?: number;
  activationToStyledMs: number;
};

export type PreviewRuntimeEventInput = {
  schemaVersion: 1;
  identity: CanvasProjectionIdentity;
  kind: PreviewRuntimeEventKind;
  durationMs: number;
  diagnostic: string | null;
  stylesheetMetrics?: PreviewStylesheetPromotionMetrics | null;
};

export type PreviewRuntimeEventReceipt = {
  schemaVersion: 1;
  identity: CanvasProjectionIdentity;
  kind: PreviewRuntimeEventKind;
  accepted: boolean;
};
