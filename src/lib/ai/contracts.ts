import type {
  CenterView,
  SourceLanguage,
} from "$lib/application/contracts";
import type { CanvasElementObservation } from "$lib/canvas/contracts";
import type { SourceEditLocation } from "$lib/source-graph/contracts";

export type AiContextStatus = {
  contextPath: string;
  discoveryPath: string;
  endpoint: string;
  contextExists: boolean;
  discoveryExists: boolean;
  updatedAt: string | null;
  mode: string;
  serverRunning: boolean;
};

type AiPresenceStatus = "active" | "idle" | "expired";

type AiClientSessionSnapshot = {
  sessionId: string;
  clientName: string;
  clientVersion: string | null;
  initializedAtMs: number;
  lastSeenAtMs: number;
  contextRevisionSeen: number | null;
  presence: AiPresenceStatus;
  ownsEditLease: boolean;
};

type EditLeaseRequest = {
  clientSessionId: string;
  expectedProjectSessionId: string;
  expectedProjectRevision: number;
  requestId: string;
  intent: string;
};

type EditLease = {
  id: string;
  requestId: string;
  clientSessionId: string;
  projectSessionId: string;
  basisProjectRevision: number;
  intent: string;
  grantedAtMs: number;
  expiresAtMs: number;
};

export type EditAuthority =
  | { state: "user_active" }
  | {
      state: "ai_requested";
      detail: { request: EditLeaseRequest; requestedAtMs: number };
    }
  | { state: "ai_active"; detail: { lease: EditLease } }
  | {
      state: "ai_orphaned";
      detail: {
        leaseId: string;
        clientSessionId: string;
        projectSessionId: string;
        basisProjectRevision: number;
        expiredAtMs: number;
        reason: string;
      };
    }
  | {
      state: "reconciling";
      detail: {
        leaseId: string;
        clientSessionId: string;
        projectSessionId: string;
        basisProjectRevision: number;
        releasedAtMs: number;
        expectedChangedFiles: string[];
        observedChangedFiles: string[];
        declarationReviewedByUser: boolean;
        recoveryReloadAuthorized: boolean;
        recoveryReloadReplacementSessionId: string | null;
        summary: string | null;
        reason: string;
      };
    }
  | {
      state: "conflict";
      detail: {
        projectSessionId: string;
        detectedAtMs: number;
        files: string[];
        reason: string;
      };
    };

type EditLeaseStatus =
  | "pending_ui_quiescence"
  | "granted"
  | "blocked"
  | "busy"
  | "stale"
  | "orphaned"
  | "reconciling"
  | "released_to_user"
  | "conflict";

type RequiredUserAction =
  | "save_or_discard"
  | "wait_for_ai"
  | "recover_interrupted_ai"
  | "resolve_conflict"
  | "reopen_project";

export type EditTransitionReceipt = {
  status: EditLeaseStatus;
  coordinationRevision: number;
  authority: EditAuthority;
  lease: EditLease | null;
  reason: string | null;
  requiredUserAction: RequiredUserAction | null;
  dirtyFiles: string[];
};

export type AiCoordinationSnapshot = {
  schemaVersion: 2;
  coordinationRevision: number;
  projectSessionId: string | null;
  authority: EditAuthority;
  clients: AiClientSessionSnapshot[];
};

export type UiQuiescenceAcknowledgement = {
  requestId: string;
  projectSessionId: string;
  projectRevision: number;
  uiRevision: number;
  uiQuiescent: boolean;
  blockerReason: string | null;
  dirtyFiles: string[];
};

export type CodexMcpStatus = {
  configPath: string;
  configExists: boolean;
  configured: boolean;
  authenticated: boolean;
  securePermissions: boolean;
  configuredUrl: string | null;
  expectedUrl: string;
};

export type UiContextProjection = {
  schemaVersion: 4;
  uiRevision: number;
  expectedProjectSessionId: string | null;
  expectedProjectRevision: number | null;
  project: {
    isOpen: boolean;
    previewBaseUrl: string | null;
    previewWarning: string | null;
  };
  workspace: {
    centerView: CenterView;
    previewDevice: "desktop" | "tablet" | "mobile";
    activeFile: string | null;
    activePreviewPath: string | null;
    sourceLanguage: SourceLanguage;
  };
  selection: {
    hasSelection: boolean;
    primaryMemberId: string | null;
    memberIds: string[];
    memberCount: number;
    selector: string | null;
    cssSelector: string | null;
    tag: string | null;
    id: string | null;
    classes: string[];
    text: string | null;
    imageSrc: string | null;
    sourceLocation: SourceEditLocation | null;
    sourceId: string | null;
    templateSourceId: string | null;
    sessionId: string | null;
    rect: CanvasElementObservation["rect"] | null;
  };
  css: {
    activeSelector: string | null;
    targetFile: string | null;
    variablesCount: number;
  };
  uiDirtyState: {
    dirty: boolean;
    canSave: boolean;
    areas: string[];
    blockedReason: string;
  };
  externalDisk: {
    changed: boolean;
    changedFiles: string[];
    activeFileChanged: boolean;
    previewRelevantChanged: boolean;
    blockedByDirtySession: boolean;
    lastDetectedAt: number | null;
    lastDetectedFiles: string[];
    lastDetectedActiveFileChanged: boolean;
    lastDetectedPreviewRelevantChanged: boolean;
    lastAppliedAt: number | null;
    lastAppliedFiles: string[];
    lastCheckedAt: number | null;
    checking: boolean;
    reconciling: boolean;
    workspaceProjectionRecoveryRequired: boolean;
    truncated: boolean;
  };
};
