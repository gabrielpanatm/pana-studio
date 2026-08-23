import type { LocalizedDiagnostic } from "$lib/contracts/localized-diagnostic";
import type {
  KernelProjectTransitionDecisionRetentionHotJournal,
  KernelProjectTransitionDecisionRetentionRecoveryReceipt,
} from "$lib/project/transition-contract";
import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";

export type WriteReceipt = {
  id: string;
  category: string;
  owner: string;
  operation: string;
  target: string;
  bytesWritten: number;
  startedAtMs: number;
  completedAtMs: number;
  status: string;
};

export type KernelProjectTransitionDecisionRetentionHotJournalRecoveryCommandResult = {
  receipt: KernelProjectTransitionDecisionRetentionRecoveryReceipt;
  recoveryCoordinator: RecoveryCoordinatorScan;
};

export type ProjectWorkspaceSaveHotJournalDiskState =
  | "before_state"
  | "planned_state"
  | "mixed_state"
  | "conflict_state";

export type ProjectWorkspaceSaveHotJournalFileDiskState =
  | "before"
  | "planned"
  | "conflict"
  | "unreadable";

type ProjectWorkspaceSaveJournalContentKind = "text" | "binary";

export type ProjectWorkspaceSaveRecoveryAction =
  | "clear_stale_journal"
  | "rollback_to_before"
  | "manual_review_mixed_state"
  | "manual_review_conflict";

type ProjectWorkspaceSaveRecoveryPlan = {
  action: ProjectWorkspaceSaveRecoveryAction;
  canClearJournal: boolean;
  canRollback: boolean;
  summary: string;
};

type ProjectWorkspaceSaveHotJournalFile = {
  relativePath: string;
  contentKind: ProjectWorkspaceSaveJournalContentKind;
  existedBefore: boolean;
  existsAfter: boolean;
  beforeHash: string;
  plannedHash: string | null;
  diskHash: string | null;
  diskState: ProjectWorkspaceSaveHotJournalFileDiskState;
  diagnostic: string | null;
};

export type ProjectWorkspaceSaveHotJournal = {
  schemaVersion: number;
  transactionId: string;
  path: string;
  runtimeSessionId: string;
  projectRoot: string;
  revision: number;
  preparedAtMs: number;
  touchedFiles: string[];
  fileCount: number;
  bytesBefore: number;
  diskState: ProjectWorkspaceSaveHotJournalDiskState;
  recoveryPlan: ProjectWorkspaceSaveRecoveryPlan;
  files: ProjectWorkspaceSaveHotJournalFile[];
};

type ProjectWorkspaceSaveRecoveryReceipt = {
  schemaVersion: number;
  transactionId: string;
  action: ProjectWorkspaceSaveRecoveryAction;
  projectRoot: string;
  restoredFiles: string[];
  alreadyBeforeFiles: string[];
  journalCleared: boolean;
  writeReceipts: WriteReceipt[];
  operatorDiagnostic: string;
};

type RecoveryCoordinatorStatus = "clean" | "needs_attention" | "unreadable";

type RecoveryCoordinatorDiagnosticSeverity = "warning" | "error";

export type RecoveryCoordinatorDiagnostic = {
  severity: RecoveryCoordinatorDiagnosticSeverity;
  code: string;
  transactionId: string | null;
  messageDiagnostic: LocalizedDiagnostic;
};

export type RecoveryJournalFamily =
  | "project_workspace_save"
  | "project_transition_decision_retention";

export type RecoveryJournalFamilyStatus = "needs_attention" | "manual_review_required";

type RecoveryJournalValueCount = {
  value: string;
  count: number;
};

export type RecoveryJournalFamilySummary = {
  family: RecoveryJournalFamily;
  status: RecoveryJournalFamilyStatus;
  label: string;
  count: number;
  clearableCount: number;
  rollbackCount: number;
  restoreCount: number;
  manualReviewCount: number;
  newestCreatedAtMs: number | null;
  stateCounts: RecoveryJournalValueCount[];
  actionCounts: RecoveryJournalValueCount[];
};

export type RecoveryCoordinatorScan = {
  schemaVersion: number;
  sessionId: string;
  projectRoot: string;
  scannedAtMs: number;
  status: RecoveryCoordinatorStatus;
  hotProjectWorkspaceSaveJournals: ProjectWorkspaceSaveHotJournal[];
  hotProjectTransitionDecisionRetentionJournals: KernelProjectTransitionDecisionRetentionHotJournal[];
  hotJournalFamilies: RecoveryJournalFamilySummary[];
  diagnostics: RecoveryCoordinatorDiagnostic[];
};

export type ProjectWorkspaceSaveRecoveryCommandResult = {
  receipt: ProjectWorkspaceSaveRecoveryReceipt;
  recoveryCoordinator: RecoveryCoordinatorScan;
  workspace: ProjectWorkspaceSnapshot;
};

type WriteAuthorityWalPhase =
  | "preparing"
  | "prepared"
  | "auxiliary_durable"
  | "effect_visible"
  | "target_durable";

export type WriteAuthorityRecoveryClassification =
  | "no_effect"
  | "staged_only"
  | "effect_committed"
  | "rollback_completed"
  | "cleanup_required"
  | "partial_append"
  | "partial_namespace_creation"
  | "partial_tree_removal"
  | "conflict"
  | "unreadable_or_corrupt";

export type WriteAuthorityRecoveryResolutionAction =
  | "discard_staged_write"
  | "restore_original"
  | "accept_restored_state"
  | "accept_current_state"
  | "continue_tree_removal"
  | "restore_remaining_tree";

export type WriteAuthorityRecoveryResolutionInput = {
  operationId: string;
  expectedPhase: WriteAuthorityWalPhase;
  evidenceHash: string;
  action: WriteAuthorityRecoveryResolutionAction;
};

export type WriteAuthorityRecoveryItem = {
  fileName: string;
  operationId: string | null;
  phase: WriteAuthorityWalPhase | null;
  classification: WriteAuthorityRecoveryClassification;
  automaticRecoveryAvailable: boolean;
  evidenceHash: string | null;
  availableResolutionActions: WriteAuthorityRecoveryResolutionAction[];
  diagnostic: string;
};

export type WriteAuthorityRecoveryScan = {
  schemaVersion: number;
  scannedAtMs: number;
  blocked: boolean;
  recordCount: number;
  totalBytes: number;
  items: WriteAuthorityRecoveryItem[];
};

export type WriteAuthorityRecoveryResolutionReceipt = {
  schemaVersion: number;
  operationId: string;
  action: WriteAuthorityRecoveryResolutionAction;
  diagnostic: string;
  recoveryScan: WriteAuthorityRecoveryScan;
};
