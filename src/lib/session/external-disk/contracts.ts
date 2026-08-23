import type { ScssVariable } from "$lib/css/contracts";
import type { ProjectDiskChangeNotice } from "$lib/kernel/project-disk-events";
import type {
  ProjectWorkspacePreviewProjectionOptions,
  ProjectWorkspacePreviewProjectionOutcome,
} from "$lib/kernel/project-workspace-preview-coordinator";
import type {
  ExternalDiskState as ExternalDiskSnapshot,
  ProjectDiskManifest,
} from "$lib/project/external-disk-contract";
import type { ProjectScan } from "$lib/project/lifecycle-contract";
import type { ProjectDiskWatchStopIdentity } from "$lib/project/io/external-disk";
import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";
import type {
  GlobalStatusEscalationRequest,
  GlobalStatusKind,
} from "$lib/status/global-status";

export const EXTERNAL_CHANGE_NOTIFICATION_ID = "project.external-disk-change";
export const EXTERNAL_CHANGE_RELOAD_ACTION_ID = "external-disk.reload";
export const EXTERNAL_CHANGE_KEEP_SESSION_ACTION_ID = "external-disk.keep-session";

export type ExternalDiskEnvironment = Readonly<{
  session: Readonly<{
    runtimeSessionId: string;
    epoch: number;
    project: ProjectScan | null;
    transitionLocked: boolean;
    historyLocked: boolean;
    aiLocked: boolean;
  }>;
  editor: Readonly<{
    activeScannedPath: string | null;
    sourceCache: Record<string, string>;
    mutationEpoch: number;
    selectionEpoch: number;
    dirty: boolean;
  }>;
  projections: Readonly<{
    invalidateProjectSession: () => void;
    acceptProject: (project: ProjectScan) => void;
    acceptWorkspace: (workspace: ProjectWorkspaceSnapshot) => void;
    setProjectStatus: (status: string) => void;
    acceptSources: (
      sourceCache: Record<string, string>,
      activeSource: string | null,
    ) => void;
    acceptScssVariables: (variables: ScssVariable[]) => void;
    invalidateDerived: () => void;
    invalidatePageJs: () => void;
  }>;
  commands: Readonly<{
    setStatus: (text: string, kind: GlobalStatusKind) => void;
    escalateStatus: (notification: GlobalStatusEscalationRequest) => void;
    clearStatus: (id: string) => void;
    refreshSourceGraph: (options?: { strict?: boolean }) => Promise<void>;
    quiesceInteractions: () => void;
    waitForInteractionLock: () => Promise<void>;
    resetHistory: () => Promise<void>;
    projectLatestPreview: (
      options: ProjectWorkspacePreviewProjectionOptions<"external-change">,
    ) => Promise<ProjectWorkspacePreviewProjectionOutcome>;
  }>;
}>;

export type ExternalDiskCheckLease = Readonly<{
  projectRoot: string;
  runtimeSessionId: string;
  projectSessionEpoch: number;
  generation: number;
}>;

export type ExternalDiskCheckInFlight = ExternalDiskCheckLease & Readonly<{
  promise: Promise<void>;
}>;

export type ExternalDiskRuntime = {
  snapshot: ExternalDiskSnapshot;
  auditTimer: number | null;
  watchUnlisten: (() => void) | null;
  watchGeneration: number | null;
  watchStopIdentity: ProjectDiskWatchStopIdentity | null;
  watchRevision: number;
  watchSubscriptionGeneration: number;
  pendingWatchNotice: ProjectDiskChangeNotice | null;
  watchEventPending: boolean;
  watchEventDrainInFlight: boolean;
  suspended: boolean;
  checkInFlight: ExternalDiskCheckInFlight | null;
  checkGeneration: number;
  reconcileGeneration: number;
};

export type ExternalDiskContext = Readonly<{
  runtime: ExternalDiskRuntime;
  environment: ExternalDiskEnvironment;
}>;

export type ExternalChangeFlags = Readonly<{
  activeFileChanged: boolean;
  previewRelevantChanged: boolean;
}>;

export type ExternalDiskBaseline = Readonly<{
  manifest: ProjectDiskManifest;
  acceptedDiskGeneration: number;
}>;
