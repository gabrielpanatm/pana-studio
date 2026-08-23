import type { LocalizedDiagnostic } from "$lib/contracts/localized-diagnostic";
import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";

type VersionRepositoryState =
  | "uninitialized"
  | "ready"
  | "invalid"
  | "unsupported"
  | "git_unavailable";

type VersionFileKind =
  | "added"
  | "modified"
  | "deleted"
  | "renamed"
  | "copied"
  | "type_changed"
  | "untracked"
  | "conflicted"
  | "unknown";

type VersionPublicationStatus = "published" | "published_refresh_required";

export type VersionDiffKind = "unstaged" | "staged" | "commit" | "integration";

type VersionSyncState =
  | "no_upstream"
  | "upstream_missing"
  | "unborn"
  | "up_to_date"
  | "ahead"
  | "behind"
  | "diverged";

export type VersioningSessionIdentity = {
  expectedProjectRoot: string;
  expectedSessionId: string;
};

export type VersioningMutationIdentity = VersioningSessionIdentity & {
  expectedStatusToken: string;
  expectedHeadOid: string | null;
};

export type VersionFileStatus = {
  path: string;
  originalPath: string | null;
  kind: VersionFileKind;
  indexStatus: string;
  worktreeStatus: string;
  staged: boolean;
  unstaged: boolean;
  conflicted: boolean;
};

type VersionRemote = {
  name: string;
  fetchUrl: string;
  pushUrl: string;
  usable: boolean;
  diagnostic: string | null;
};

type VersionBranch = {
  name: string;
  oid: string | null;
  current: boolean;
  upstreamRef: string | null;
  upstreamOid: string | null;
  ahead: number;
  behind: number;
  syncState: VersionSyncState;
};

type VersionRemoteBranch = {
  remote: string;
  name: string;
  refName: string;
  oid: string;
};

type VersionUpstream = {
  localBranch: string;
  remote: string;
  remoteBranch: string;
  refName: string;
  oid: string | null;
  ahead: number;
  behind: number;
  syncState: VersionSyncState;
};

export type VersioningSnapshot = {
  schemaVersion: number;
  projectRoot: string;
  repositoryRoot: string;
  repositoryState: VersionRepositoryState;
  diagnostic: string | null;
  gitVersion: string | null;
  objectFormat: string | null;
  branch: string | null;
  detachedHead: boolean;
  unbornHead: boolean;
  headOid: string | null;
  statusToken: string;
  clean: boolean;
  stagedCount: number;
  unstagedCount: number;
  conflictedCount: number;
  files: VersionFileStatus[];
  userName: string | null;
  userEmail: string | null;
  remotes: VersionRemote[];
  branches: VersionBranch[];
  remoteBranches: VersionRemoteBranch[];
  upstream: VersionUpstream | null;
  syncState: VersionSyncState;
};

type VersionNetworkOperationKind = "fetch" | "push";

type VersionNetworkOperationStatus =
  | "started"
  | "progress"
  | "completed"
  | "failed"
  | "cancelled";

export type VersionNetworkProgressEvent = {
  schemaVersion: number;
  projectRoot: string;
  sessionId: string;
  operationId: string;
  kind: VersionNetworkOperationKind;
  status: VersionNetworkOperationStatus;
  messageDiagnostic: LocalizedDiagnostic;
};

export type VersionNetworkReceipt = {
  schemaVersion: number;
  operationId: string;
  kind: VersionNetworkOperationKind;
  remote: string;
  branch: string | null;
  changed: boolean;
  diagnostic: string | null;
  snapshot: VersioningSnapshot;
};

export type VersionNetworkCancelReceipt = {
  schemaVersion: number;
  operationId: string;
  cancellationRequested: boolean;
};

export type VersionIntegrationMode = "fast_forward" | "merge";

type VersionIntegrationRelationship =
  | "same"
  | "fast_forward"
  | "local_ahead"
  | "diverged";

type VersionIntegrationKind =
  | "fast_forward"
  | "merge_clean"
  | "merge_conflict"
  | "merge_resolved"
  | "switch_branch";

export type VersionIntegrationPlan = {
  schemaVersion: number;
  headOid: string;
  targetRef: string;
  targetOid: string;
  relationship: VersionIntegrationRelationship;
  ahead: number;
  behind: number;
  localOnly: VersionHistoryEntry[];
  targetOnly: VersionHistoryEntry[];
  fastForwardAllowed: boolean;
  mergeAllowed: boolean;
  repositoryClean: boolean;
  diagnostic: string;
};

type VersionIntegrationStatus =
  | "applied"
  | "noop"
  | "conflict_resolution_required"
  | "recovery_required";

export type VersionIntegrationReceipt = {
  schemaVersion: number;
  status: VersionIntegrationStatus;
  projectRoot: string;
  sessionId: string;
  transactionId: string | null;
  recoveryRef: string | null;
  kind: VersionIntegrationKind | null;
  previousHeadOid: string;
  targetRef: string;
  targetOid: string;
  resultCommitOid: string | null;
  changedPaths: string[];
  conflictPaths: string[];
  diagnostic: string | null;
  snapshot: VersioningSnapshot | null;
  workspace: ProjectWorkspaceSnapshot | null;
};

export type VersionIntegrationRecoveryAction =
  | "finalize"
  | "continue"
  | "rollback"
  | "cleanup";

type VersionIntegrationRecoveryState =
  | "ready_to_finalize"
  | "conflict_resolution"
  | "ready_to_rollback"
  | "cleanup_required"
  | "manual_review";

export type VersionIntegrationRecoveryItem = {
  transactionId: string;
  recoveryRef: string;
  kind: VersionIntegrationKind;
  previousHeadOid: string;
  targetRef: string;
  targetOid: string;
  resultCommitOid: string | null;
  conflictPaths: string[];
  state: VersionIntegrationRecoveryState;
  availableActions: VersionIntegrationRecoveryAction[];
  diagnostic: string;
};

export type VersionIntegrationRecoveryScan = {
  schemaVersion: number;
  projectRoot: string;
  sessionId: string;
  items: VersionIntegrationRecoveryItem[];
};

export type VersionIntegrationRecoveryResolutionReceipt = {
  schemaVersion: number;
  projectRoot: string;
  sessionId: string;
  transactionId: string;
  recoveryRef: string;
  action: VersionIntegrationRecoveryAction;
  resolved: boolean;
  diagnostic: string | null;
  snapshot: VersioningSnapshot | null;
  workspace: ProjectWorkspaceSnapshot | null;
};

export type VersioningMutationReceipt = {
  schemaVersion: number;
  changed: boolean;
  touchedPaths: string[];
  snapshot: VersioningSnapshot;
};

export type VersioningCommitReceipt = {
  schemaVersion: number;
  commitOid: string;
  parentOid: string | null;
  message: string;
  publicationStatus: VersionPublicationStatus;
  diagnostic: string | null;
  snapshot: VersioningSnapshot | null;
};

export type VersionHistoryEntry = {
  oid: string;
  shortOid: string;
  parentOids: string[];
  authorName: string;
  authorEmail: string;
  authoredAt: string;
  subject: string;
};

export type VersionHistoryPage = {
  schemaVersion: number;
  offset: number;
  limit: number;
  hasMore: boolean;
  entries: VersionHistoryEntry[];
};

export type VersionDiffInput = {
  kind: VersionDiffKind;
  path?: string | null;
  commitOid?: string | null;
  targetRef?: string | null;
  expectedTargetOid?: string | null;
};

export type VersionDiffReceipt = {
  schemaVersion: number;
  kind: VersionDiffKind;
  path: string | null;
  commitOid: string | null;
  binary: boolean;
  truncated: boolean;
  patch: string;
};

export type VersionPreviewReceipt = {
  schemaVersion: number;
  projectRoot: string;
  sessionId: string;
  commitOid: string;
  shortOid: string;
  previewUrl: string;
  fileCount: number;
  totalBytes: number;
};

type VersionRestoreStatus = "restored" | "noop" | "recovery_required";

export type VersionRestoreReceipt = {
  schemaVersion: number;
  status: VersionRestoreStatus;
  projectRoot: string;
  sessionId: string;
  transactionId: string | null;
  recoveryRef: string | null;
  targetCommitOid: string;
  previousHeadOid: string | null;
  restoreCommitOid: string | null;
  changedPaths: string[];
  diagnostic: string | null;
  snapshot: VersioningSnapshot | null;
  workspace: ProjectWorkspaceSnapshot | null;
};

export type VersionRestoreRecoveryAction = "finalize" | "rollback" | "cleanup";

type VersionRestoreRecoveryState =
  | "ready_to_finalize"
  | "ready_to_rollback"
  | "cleanup_required"
  | "manual_review";

export type VersionRestoreRecoveryItem = {
  transactionId: string;
  recoveryRef: string;
  targetCommitOid: string;
  previousHeadOid: string;
  restoreCommitOid: string;
  state: VersionRestoreRecoveryState;
  availableActions: VersionRestoreRecoveryAction[];
  diagnostic: string;
};

export type VersionRestoreRecoveryScan = {
  schemaVersion: number;
  projectRoot: string;
  sessionId: string;
  items: VersionRestoreRecoveryItem[];
};

export type VersionRestoreRecoveryResolutionReceipt = {
  schemaVersion: number;
  projectRoot: string;
  sessionId: string;
  transactionId: string;
  recoveryRef: string;
  action: VersionRestoreRecoveryAction;
  resolved: boolean;
  diagnostic: string | null;
  snapshot: VersioningSnapshot | null;
  workspace: ProjectWorkspaceSnapshot | null;
};
