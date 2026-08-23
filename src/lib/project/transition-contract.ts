import type { KernelObservabilityLogSourceFilter } from "$lib/kernel/observability-contract";
import type { WriteReceipt } from "$lib/kernel/recovery-contract";

type KernelProjectStateStatus = "idle" | "clean" | "info" | "dirty" | "warning" | "blocked";

type KernelProjectStateReason =
  | "no_project"
  | "project_session_missing"
  | "project_workspace_missing"
  | "disk_conflict_snapshot_missing"
  | "disk_unverifiable"
  | "disk_conflict"
  | "workspace_dirty"
  | "metadata_changed"
  | "clean";

type KernelProjectStateSnapshot = {
  schemaVersion: number;
  status: KernelProjectStateStatus;
  reason: KernelProjectStateReason;
  verdictReason: string;
  projectOpen: boolean;
  sessionId: string | null;
  projectRoot: string | null;
  isClean: boolean;
  writeBlocked: boolean;
  projectWorkspaceAvailable: boolean;
  diskConflictSnapshotAvailable: boolean;
  workspaceDirty: boolean;
  workspaceRevision: number | null;
  workspaceDiskGeneration: number | null;
  workspaceDirtyResourceCount: number;
  workspaceDirtyDocumentCount: number;
  workspaceCreatedDocumentCount: number;
  workspaceDeletedDocumentCount: number;
  workspaceDirtyPageJsCount: number;
  workspaceUndoCount: number;
  workspaceRedoCount: number;
  dirtyOnlyCount: number;
  metadataChangedCount: number;
  diskConflictCount: number;
  diskBlockingCount: number;
  unreadableFileCount: number;
};

export type KernelProjectTransitionAction = "open_project" | "reload_project" | "close_project";

type KernelProjectTransitionDecision = "allow" | "confirm" | "block";

export type KernelProjectTransitionReason =
  | "no_open_project"
  | "clean"
  | "metadata_changed"
  | "workspace_dirty"
  | "disk_conflict"
  | "blocked_project_state"
  | "unknown_warning";

export type KernelProjectTransitionPolicy = {
  schemaVersion: number;
  action: KernelProjectTransitionAction;
  decision: KernelProjectTransitionDecision;
  reason: KernelProjectTransitionReason;
  projectStateStatus: KernelProjectStateStatus;
  projectStateReason: KernelProjectStateReason;
  projectRoot: string | null;
  sessionId: string | null;
  requiresOperatorConfirmation: boolean;
  blocksTransition: boolean;
  workspaceDirtyResourceCount: number;
  workspaceRevision: number | null;
  workspaceUndoCount: number;
  workspaceRedoCount: number;
  diskConflictCount: number;
  diskBlockingCount: number;
  metadataChangedCount: number;
};

export type KernelProjectTransitionPolicyMatrixSnapshot = {
  schemaVersion: number;
  projectState: KernelProjectStateSnapshot;
  policies: KernelProjectTransitionPolicy[];
};

type KernelProjectTransitionBlockedCause =
  | "disk_conflict"
  | "workspace_dirty"
  | "blocked_project_state"
  | "unknown";

type KernelProjectTransitionResolutionSurface =
  | "disk_conflict"
  | "project_workspace"
  | "overview"
  | "observability";

type KernelProjectTransitionBlockedHealthStatus =
  | "clean"
  | "recently_blocked"
  | "repeatedly_blocked"
  | "degraded";

type KernelProjectTransitionBlockedHealthSnapshot = {
  schemaVersion: number;
  status: KernelProjectTransitionBlockedHealthStatus;
  recordCount: number;
  actionCount: number;
  repeatedActionCount: number;
  causeCount: number;
  repeatedCauseCount: number;
  latestRecordId: string | null;
  latestAction: KernelProjectTransitionAction | null;
  latestBlockedAtMs: number | null;
  summary: string;
  detail: string;
  recommendedAction: string;
};

type KernelProjectTransitionBlockedCauseSummary = {
  schemaVersion: number;
  cause: KernelProjectTransitionBlockedCause;
  surface: KernelProjectTransitionResolutionSurface;
  count: number;
  latestBlockedAtMs: number;
  latestRecordId: string | null;
  recordIds: string[];
  title: string;
  detail: string;
  recommendedAction: string;
};

type KernelProjectTransitionBlockedActionSummary = {
  schemaVersion: number;
  action: KernelProjectTransitionAction;
  count: number;
  latestRecordId: string;
  latestBlockedAtMs: number;
  cause: KernelProjectTransitionBlockedCause;
  surface: KernelProjectTransitionResolutionSurface;
  decision: KernelProjectTransitionDecision | null;
  reason: KernelProjectTransitionReason | null;
  projectStateStatus: KernelProjectStateStatus | null;
  projectStateReason: KernelProjectStateReason | null;
  currentProjectRoot: string | null;
  targetProjectRoot: string | null;
  sessionId: string | null;
  title: string;
  detail: string;
  recommendedAction: string;
};

type KernelProjectTransitionBlockedRecord = {
  schemaVersion: number;
  id: string;
  blockedAtMs: number;
  sourceLabel: string;
  action: KernelProjectTransitionAction | null;
  decision: KernelProjectTransitionDecision | null;
  reason: KernelProjectTransitionReason | null;
  projectStateStatus: KernelProjectStateStatus | null;
  projectStateReason: KernelProjectStateReason | null;
  currentProjectRoot: string | null;
  targetProjectRoot: string | null;
  sessionId: string | null;
  operation: string;
  target: string | null;
  message: string;
  diagnostic: string | null;
  workspaceDirtyResourceCount: number;
  workspaceRevision: number | null;
  workspaceUndoCount: number;
  workspaceRedoCount: number;
  diskConflictCount: number;
  diskBlockingCount: number;
};

export type KernelProjectTransitionBlockedAuditSnapshot = {
  schemaVersion: number;
  logPath: string;
  logExists: boolean;
  truncated: boolean;
  scannedLineCount: number;
  unreadableCount: number;
  matchingEventCount: number;
  returnedCount: number;
  includeArchives: boolean;
  sourceFilter: KernelObservabilityLogSourceFilter;
  health: KernelProjectTransitionBlockedHealthSnapshot;
  latestByAction: KernelProjectTransitionBlockedActionSummary[];
  causes: KernelProjectTransitionBlockedCauseSummary[];
  records: KernelProjectTransitionBlockedRecord[];
  diagnostics: string[];
};

type KernelProjectTransitionDecisionKind =
  | "discard_local_drafts_for_transition"
  | "acknowledge_dirty_history_for_transition"
  | "discard_session_for_external_reload";

export type KernelProjectTransitionDecisionJournalHealthStatus =
  | "clean"
  | "has_decisions"
  | "integrity_warning"
  | "degraded";

type KernelProjectTransitionDecisionReuseStatus =
  | "no_decisions"
  | "exact_evidence_only"
  | "repeated_context"
  | "blocked_by_integrity";

type KernelProjectTransitionDecisionRecoveryPlanStatus =
  | "clean_noop"
  | "verified_audit"
  | "retention_review"
  | "integrity_blocked";

type KernelProjectTransitionDecisionRecoveryAckKind =
  | "acknowledge_integrity_blocked"
  | "acknowledge_retention_review";

type KernelProjectTransitionDecisionRecoveryAckJournalHealthStatus =
  | "clean"
  | "has_acknowledgements"
  | "integrity_warning"
  | "degraded";

type KernelProjectTransitionDecisionRecoveryIssueKind =
  | "read_diagnostic"
  | "invalid_evidence_hash"
  | "duplicate_decision_id"
  | "superseded_record";

type KernelProjectTransitionDecisionRecoveryIssueSeverity = "info" | "warning" | "error";

type KernelProjectTransitionDirtyFileEvidence = {
  relativePath: string;
  baselineHash: string;
  currentHash: string;
  currentBytes: number;
  revision: number;
};

type KernelProjectTransitionDiskFileEvidence = {
  relativePath: string;
  kind: string;
  baselineHash: string;
  diskHash: string | null;
  revision: number;
};

type KernelProjectTransitionWorkspaceEvidence = {
  revision: number;
  diskGeneration: number;
  dirty: boolean;
  dirtyDocumentCount: number;
  createdDocumentCount: number;
  deletedDocumentCount: number;
  dirtyPageJsCount: number;
  undoCount: number;
  redoCount: number;
  fingerprint: string;
};

type KernelProjectTransitionDecisionEvidence = {
  schemaVersion: number;
  action: KernelProjectTransitionAction;
  targetProjectRoot: string;
  sessionId: string;
  projectRoot: string;
  projectStateStatus: KernelProjectStateStatus;
  projectStateReason: KernelProjectStateReason;
  transitionDecision: KernelProjectTransitionDecision;
  transitionReason: KernelProjectTransitionReason;
  workspaceDirtyResourceCount: number;
  dirtyFiles: KernelProjectTransitionDirtyFileEvidence[];
  diskFiles: KernelProjectTransitionDiskFileEvidence[];
  workspace: KernelProjectTransitionWorkspaceEvidence;
};

type KernelProjectTransitionDecisionRecord = {
  schemaVersion: number;
  id: string;
  decidedAtMs: number;
  decisionKind: KernelProjectTransitionDecisionKind;
  diagnostic: string;
  evidenceHash: string;
  evidence: KernelProjectTransitionDecisionEvidence;
};

export type KernelProjectTransitionDecisionReceipt = {
  schemaVersion: number;
  decision: KernelProjectTransitionDecisionRecord;
};

type KernelProjectTransitionDecisionRecoveryAckEvidence = {
  schemaVersion: number;
  sessionId: string;
  projectRoot: string;
  decisionJournalPath: string;
  recoveryPlanEvidenceHash: string;
  recoveryPlanStatus: KernelProjectTransitionDecisionRecoveryPlanStatus;
  integrityTrusted: boolean;
  recordCount: number;
  readDiagnosticCount: number;
  invalidEvidenceHashCount: number;
  duplicateIdCount: number;
  supersededRecordCount: number;
  retentionCandidateCount: number;
  issueCount: number;
};

type KernelProjectTransitionDecisionRecoveryAckRecord = {
  schemaVersion: number;
  id: string;
  acknowledgedAtMs: number;
  ackKind: KernelProjectTransitionDecisionRecoveryAckKind;
  diagnostic: string;
  evidenceHash: string;
  evidence: KernelProjectTransitionDecisionRecoveryAckEvidence;
};

export type KernelProjectTransitionDecisionRecoveryAckReceipt = {
  schemaVersion: number;
  acknowledgement: KernelProjectTransitionDecisionRecoveryAckRecord;
};

type KernelProjectTransitionDecisionRecoveryAckJournalHealthSnapshot = {
  schemaVersion: number;
  status: KernelProjectTransitionDecisionRecoveryAckJournalHealthStatus;
  recordCount: number;
  returnedCount: number;
  diagnosticCount: number;
  invalidEvidenceHashCount: number;
  duplicateIdCount: number;
  latestRecordId: string | null;
  latestAcknowledgedAtMs: number | null;
  latestAckKind: KernelProjectTransitionDecisionRecoveryAckKind | null;
  latestRecoveryPlanEvidenceHash: string | null;
  summary: string;
  detail: string;
  recommendedAction: string;
};

export type KernelProjectTransitionDecisionRecoveryAckJournalSnapshot = {
  schemaVersion: number;
  path: string;
  health: KernelProjectTransitionDecisionRecoveryAckJournalHealthSnapshot;
  recordCount: number;
  returnedCount: number;
  records: KernelProjectTransitionDecisionRecoveryAckRecord[];
  diagnostics: string[];
};

type KernelProjectTransitionDecisionRetentionStatus =
  | "clean_noop"
  | "committed"
  | "recovery_attention";

export type KernelProjectTransitionDecisionRetentionHotJournalDiskState =
  | "no_effect"
  | "completed_retention"
  | "partial_retention"
  | "conflict_state";

export type KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction =
  | "clear_no_effect_journal"
  | "clear_completed_journal"
  | "restore_before_journal"
  | "manual_review_conflict";

export type KernelProjectTransitionDecisionRetentionReceipt = {
  schemaVersion: number;
  retentionId: string;
  sessionId: string;
  decisionJournalPath: string;
  archivePath: string | null;
  hotJournalPath: string | null;
  status: KernelProjectTransitionDecisionRetentionStatus;
  startedAtMs: number;
  completedAtMs: number;
  acknowledgementId: string;
  recoveryPlanEvidenceHash: string;
  diagnostic: string;
  candidateRecordIds: string[];
  beforeJournalHash: string;
  afterJournalHash: string;
  archiveHash: string;
  hotJournalWritten: boolean;
  archiveWritten: boolean;
  activeJournalWritten: boolean;
  hotJournalCleared: boolean;
  retentionCandidateCount: number;
  archivedRecordCount: number;
  keptRecordCount: number;
  writeReceipts: WriteReceipt[];
  recoveryDiagnostic: string | null;
};

type KernelProjectTransitionDecisionRetentionHotJournalRecoveryPlan = {
  action: KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction;
  title: string;
  summary: string;
  requiredChecks: string[];
  canClearJournal: boolean;
  canRestoreBeforeJournal: boolean;
};

export type KernelProjectTransitionDecisionRetentionHotJournal = {
  schemaVersion: number;
  retentionId: string;
  path: string;
  sessionId: string;
  projectRoot: string;
  decisionJournalPath: string;
  archivePath: string;
  createdAtMs: number;
  acknowledgementId: string;
  recoveryPlanEvidenceHash: string;
  candidateRecordIds: string[];
  candidateCount: number;
  archivedRecordCount: number;
  keptRecordCount: number;
  beforeJournalHash: string;
  afterJournalHash: string;
  archiveHash: string;
  currentJournalHash: string | null;
  archiveDiskHash: string | null;
  diskState: KernelProjectTransitionDecisionRetentionHotJournalDiskState;
  recoveryPlan: KernelProjectTransitionDecisionRetentionHotJournalRecoveryPlan;
  diagnostics: string[];
};

export type KernelProjectTransitionDecisionRetentionRecoveryReceipt = {
  schemaVersion: number;
  retentionId: string;
  action: KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction;
  journalPath: string;
  decisionJournalPath: string;
  archivePath: string;
  diskStateBefore: KernelProjectTransitionDecisionRetentionHotJournalDiskState;
  journalCleared: boolean;
  restoredBeforeJournal: boolean;
  candidateCount: number;
  archivedRecordCount: number;
  keptRecordCount: number;
  operatorDiagnostic: string;
  writeReceipts: WriteReceipt[];
};

type KernelProjectTransitionDecisionJournalHealthSnapshot = {
  schemaVersion: number;
  status: KernelProjectTransitionDecisionJournalHealthStatus;
  recordCount: number;
  returnedCount: number;
  diagnosticCount: number;
  invalidEvidenceHashCount: number;
  duplicateIdCount: number;
  latestRecordId: string | null;
  latestDecidedAtMs: number | null;
  latestDecisionKind: KernelProjectTransitionDecisionKind | null;
  summary: string;
  detail: string;
  recommendedAction: string;
};

type KernelProjectTransitionDecisionActionSummary = {
  schemaVersion: number;
  action: KernelProjectTransitionAction;
  count: number;
  latestRecordId: string;
  latestDecidedAtMs: number;
  latestDecisionKind: KernelProjectTransitionDecisionKind;
  latestTransitionReason: KernelProjectTransitionReason;
  latestProjectStateStatus: KernelProjectStateStatus;
  latestProjectStateReason: KernelProjectStateReason;
  latestTargetProjectRoot: string;
  latestSessionId: string;
  title: string;
  detail: string;
  recommendedAction: string;
};

type KernelProjectTransitionDecisionKindSummary = {
  schemaVersion: number;
  decisionKind: KernelProjectTransitionDecisionKind;
  count: number;
  latestRecordId: string;
  latestDecidedAtMs: number;
  latestAction: KernelProjectTransitionAction;
  latestTransitionReason: KernelProjectTransitionReason;
  latestTargetProjectRoot: string;
  title: string;
  detail: string;
  recommendedAction: string;
};

type KernelProjectTransitionDecisionReuseContextSummary = {
  schemaVersion: number;
  action: KernelProjectTransitionAction;
  decisionKind: KernelProjectTransitionDecisionKind;
  targetProjectRoot: string;
  count: number;
  latestRecordId: string;
  latestDecidedAtMs: number;
  latestTransitionReason: KernelProjectTransitionReason;
  supersededRecordIds: string[];
  title: string;
  detail: string;
  recommendedAction: string;
};

type KernelProjectTransitionDecisionReuseGuidanceSnapshot = {
  schemaVersion: number;
  status: KernelProjectTransitionDecisionReuseStatus;
  exactEvidenceOnly: boolean;
  blockedByIntegrity: boolean;
  recordCount: number;
  contextCount: number;
  repeatedContextCount: number;
  supersededRecordCount: number;
  latestContextRecordId: string | null;
  latestDecidedAtMs: number | null;
  summary: string;
  detail: string;
  recommendedAction: string;
  contexts: KernelProjectTransitionDecisionReuseContextSummary[];
};

type KernelProjectTransitionDecisionRecoveryIssue = {
  schemaVersion: number;
  kind: KernelProjectTransitionDecisionRecoveryIssueKind;
  severity: KernelProjectTransitionDecisionRecoveryIssueSeverity;
  recordId: string | null;
  count: number;
  title: string;
  detail: string;
  recommendedAction: string;
};

type KernelProjectTransitionDecisionRetentionCandidate = {
  schemaVersion: number;
  recordId: string;
  supersededByRecordId: string;
  action: KernelProjectTransitionAction;
  decisionKind: KernelProjectTransitionDecisionKind;
  targetProjectRoot: string;
  decidedAtMs: number;
  transitionReason: KernelProjectTransitionReason;
  title: string;
  detail: string;
  recommendedAction: string;
};

type KernelProjectTransitionDecisionRecoveryPlanSnapshot = {
  schemaVersion: number;
  evidenceHash: string;
  status: KernelProjectTransitionDecisionRecoveryPlanStatus;
  readOnly: boolean;
  mutationAllowed: boolean;
  integrityTrusted: boolean;
  recordCount: number;
  readDiagnosticCount: number;
  invalidEvidenceHashCount: number;
  duplicateIdCount: number;
  supersededRecordCount: number;
  retentionCandidateCount: number;
  issueCount: number;
  summary: string;
  detail: string;
  recommendedAction: string;
  issues: KernelProjectTransitionDecisionRecoveryIssue[];
  retentionCandidates: KernelProjectTransitionDecisionRetentionCandidate[];
};

export type KernelProjectTransitionDecisionJournalSnapshot = {
  schemaVersion: number;
  path: string;
  health: KernelProjectTransitionDecisionJournalHealthSnapshot;
  latestByAction: KernelProjectTransitionDecisionActionSummary[];
  byDecisionKind: KernelProjectTransitionDecisionKindSummary[];
  reuseGuidance: KernelProjectTransitionDecisionReuseGuidanceSnapshot;
  recoveryPlan: KernelProjectTransitionDecisionRecoveryPlanSnapshot;
  recordCount: number;
  returnedCount: number;
  records: KernelProjectTransitionDecisionRecord[];
  diagnostics: string[];
};
