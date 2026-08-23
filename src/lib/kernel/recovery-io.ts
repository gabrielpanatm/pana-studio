import type {
  KernelLogLevel,
  KernelObservabilityLogSnapshot,
  KernelObservabilityLogSourceFilter,
} from "$lib/kernel/observability-contract";
import type {
  KernelProjectTransitionDecisionRetentionHotJournalRecoveryCommandResult,
  ProjectWorkspaceSaveRecoveryAction,
  ProjectWorkspaceSaveRecoveryCommandResult,
  RecoveryCoordinatorScan,
} from "$lib/kernel/recovery-contract";
import type {
  WriteAuthorityRecoveryResolutionInput,
  WriteAuthorityRecoveryResolutionReceipt,
  WriteAuthorityRecoveryScan,
} from "$lib/kernel/recovery-contract";
import type { KernelDiskConflictSnapshot } from "$lib/project/external-disk-contract";
import type {
  KernelProjectTransitionAction,
  KernelProjectTransitionBlockedAuditSnapshot,
  KernelProjectTransitionDecisionJournalSnapshot,
  KernelProjectTransitionDecisionReceipt,
  KernelProjectTransitionDecisionRecoveryAckJournalSnapshot,
  KernelProjectTransitionDecisionRecoveryAckReceipt,
  KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
  KernelProjectTransitionDecisionRetentionReceipt,
  KernelProjectTransitionPolicy,
  KernelProjectTransitionPolicyMatrixSnapshot,
} from "$lib/project/transition-contract";
import { invoke } from "@tauri-apps/api/core";

export function readRecoveryCoordinator(): Promise<RecoveryCoordinatorScan | null> {
  return invoke<RecoveryCoordinatorScan | null>("read_recovery_coordinator_scan");
}

export function readKernelDiskConflicts(): Promise<KernelDiskConflictSnapshot | null> {
  return invoke<KernelDiskConflictSnapshot | null>("read_kernel_disk_conflicts");
}

export function readKernelObservabilityLog(
  limit = 80,
  recoveryOnly = true,
  includeArchives = false,
  levels: KernelLogLevel[] = ["info", "warn", "error"],
  sourceFilter: KernelObservabilityLogSourceFilter = "all",
): Promise<KernelObservabilityLogSnapshot> {
  return invoke<KernelObservabilityLogSnapshot>("read_kernel_observability_log", {
    limit,
    recoveryOnly,
    includeArchives,
    levels,
    sourceFilter,
  });
}

export function readWriteAuthorityRecoveryScan(): Promise<WriteAuthorityRecoveryScan> {
  return invoke<WriteAuthorityRecoveryScan>("read_write_authority_recovery_scan");
}

export function resolveWriteAuthorityRecovery(
  input: WriteAuthorityRecoveryResolutionInput,
): Promise<WriteAuthorityRecoveryResolutionReceipt> {
  return invoke<WriteAuthorityRecoveryResolutionReceipt>("resolve_write_authority_recovery", {
    input,
  });
}

export function readKernelProjectTransitionPolicy(
  action: KernelProjectTransitionAction,
): Promise<KernelProjectTransitionPolicy> {
  return invoke<KernelProjectTransitionPolicy>("read_kernel_project_transition_policy", { action });
}

export function readKernelProjectTransitionPolicyMatrix(): Promise<KernelProjectTransitionPolicyMatrixSnapshot> {
  return invoke<KernelProjectTransitionPolicyMatrixSnapshot>("read_kernel_project_transition_policy_matrix");
}

export function readKernelProjectTransitionBlockedAudit(
  limit = 40,
  includeArchives = false,
): Promise<KernelProjectTransitionBlockedAuditSnapshot> {
  return invoke<KernelProjectTransitionBlockedAuditSnapshot>("read_kernel_project_transition_blocked_audit", {
    limit,
    includeArchives,
  });
}

export function readKernelProjectTransitionDecisionJournal(
  limit = 80,
): Promise<KernelProjectTransitionDecisionJournalSnapshot | null> {
  return invoke<KernelProjectTransitionDecisionJournalSnapshot | null>(
    "read_kernel_project_transition_decision_journal",
    { limit },
  );
}

export function readKernelProjectTransitionDecisionRecoveryAckJournal(
  limit = 40,
): Promise<KernelProjectTransitionDecisionRecoveryAckJournalSnapshot | null> {
  return invoke<KernelProjectTransitionDecisionRecoveryAckJournalSnapshot | null>(
    "read_kernel_project_transition_decision_recovery_ack_journal",
    { limit },
  );
}

export function recordProjectTransitionOperatorDecision(
  targetRoot: string,
  diagnostic: string,
  action?: KernelProjectTransitionAction,
): Promise<KernelProjectTransitionDecisionReceipt> {
  return invoke<KernelProjectTransitionDecisionReceipt>("record_project_transition_operator_decision", {
    targetRoot,
    diagnostic,
    action,
  });
}

export function acknowledgeProjectTransitionDecisionRecoveryPlan(
  recoveryPlanEvidenceHash: string,
  diagnostic: string,
): Promise<KernelProjectTransitionDecisionRecoveryAckReceipt> {
  return invoke<KernelProjectTransitionDecisionRecoveryAckReceipt>(
    "acknowledge_project_transition_decision_recovery_plan",
    {
      recoveryPlanEvidenceHash,
      diagnostic,
    },
  );
}

export function executeProjectTransitionDecisionRetention(
  recoveryPlanEvidenceHash: string,
  acknowledgementId: string,
  diagnostic: string,
): Promise<KernelProjectTransitionDecisionRetentionReceipt> {
  return invoke<KernelProjectTransitionDecisionRetentionReceipt>("execute_project_transition_decision_retention", {
    recoveryPlanEvidenceHash,
    acknowledgementId,
    diagnostic,
  });
}

export function recoverProjectTransitionDecisionRetentionHotJournal(
  retentionId: string,
  action: KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
  diagnostic: string,
): Promise<KernelProjectTransitionDecisionRetentionHotJournalRecoveryCommandResult> {
  return invoke<KernelProjectTransitionDecisionRetentionHotJournalRecoveryCommandResult>(
    "recover_project_transition_decision_retention_hot_journal",
    {
      retentionId,
      action,
      diagnostic,
    },
  );
}

export function recoverProjectWorkspaceSave(
  transactionId: string,
  action: ProjectWorkspaceSaveRecoveryAction,
  diagnostic: string,
): Promise<ProjectWorkspaceSaveRecoveryCommandResult> {
  return invoke<ProjectWorkspaceSaveRecoveryCommandResult>("recover_project_workspace_save", {
    transactionId,
    action,
    diagnostic,
  });
}
