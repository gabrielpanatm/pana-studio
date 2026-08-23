import type {
  ProjectPreviewRequestIdentity,
} from "$lib/preview/io";
import type {
  ProjectBootstrapInitialSurface,
  ProjectOpenInspectionReceipt,
  ProjectOpenRecoveryDecisionInput,
} from "$lib/project/lifecycle-contract";
import type { StartupCandidateSnapshot } from "$lib/project/lifecycle-contract";

export type OpenProjectRootOptions = {
  operatorDecisionId?: string | null;
  recoveryDecision?: ProjectOpenRecoveryDecisionInput | null;
  startupCandidate?: StartupCandidateSnapshot | null;
  inspection?: ProjectOpenInspectionReceipt | null;
};

export type FrontendProjectAttachmentMode = "open" | "reattach" | "reload";

export type FrontendProjectAttachment = ProjectPreviewRequestIdentity & {
  expectedProjectTransitionGeneration: number;
  initialSurface?: ProjectBootstrapInitialSurface | null;
  previewWarning?: string | null;
};

export type ProjectPreviewStartOutcome =
  | { status: "canonical"; projectSessionId: string }
  | { status: "deferred"; projectSessionId: string }
  | { status: "degraded"; projectSessionId: string; message: string }
  | { status: "stale"; projectSessionId: string };

export type ProjectReloadOutcome =
  | {
      status: "completed";
      projectSessionId: string;
      previewStatus: "canonical" | "deferred" | "degraded";
      message: string | null;
    }
  | { status: "cancelled"; projectSessionId: null; message: string }
  | { status: "failed"; projectSessionId: string | null; message: string };

export type ReconcileWorkspaceDerivedStateOptions = {
  expectedProjectRoot: string;
  expectedSessionId: string;
  expectedWorkspaceRevision: number;
  topologyChanged: boolean;
  preferredRelativePath?: string | null;
  refreshSourceGraph?: boolean;
  refreshScss?: boolean;
};

export type CommittedHistoryProjectionContext = Readonly<{
  projectRoot: string;
  runtimeSessionId: string;
  projectSessionEpoch: number;
  workspaceRevision: number;
}>;
