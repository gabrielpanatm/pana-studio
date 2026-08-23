import { invoke } from "@tauri-apps/api/core";
import type {
  AiContextStatus,
  AiCoordinationSnapshot,
  CodexMcpStatus,
  EditTransitionReceipt,
  UiContextProjection,
  UiQuiescenceAcknowledgement,
} from "$lib/ai/contracts";

export function readAiContextStatus(): Promise<AiContextStatus> {
  return invoke<AiContextStatus>("read_ai_context_status");
}

export function readAiCoordinationState(): Promise<AiCoordinationSnapshot> {
  return invoke<AiCoordinationSnapshot>("read_ai_coordination_state");
}

export function acknowledgeAiEditQuiescence(
  clientSessionId: string,
  acknowledgement: UiQuiescenceAcknowledgement,
): Promise<EditTransitionReceipt> {
  return invoke<EditTransitionReceipt>("acknowledge_ai_edit_quiescence", {
    clientSessionId,
    acknowledgement,
  });
}

export function completeAiEditReconciliation(
  leaseId: string,
  expectedProjectSessionId: string,
  expectedProjectRevision: number,
  observedChangedFiles: string[],
): Promise<EditTransitionReceipt> {
  return invoke<EditTransitionReceipt>("complete_ai_edit_reconciliation", {
    leaseId,
    expectedProjectSessionId,
    expectedProjectRevision,
    observedChangedFiles,
  });
}

export function acceptAiEditConflictForReconciliation(): Promise<EditTransitionReceipt> {
  return invoke<EditTransitionReceipt>("accept_ai_edit_conflict_for_reconciliation");
}

export function authorizeAiReconciliationRecoveryReload(): Promise<EditTransitionReceipt> {
  return invoke<EditTransitionReceipt>("authorize_ai_reconciliation_recovery_reload");
}

export function completeAiReconciliationRecoveryReload(
  leaseId: string,
  expectedReplacementSessionId: string,
): Promise<EditTransitionReceipt> {
  return invoke<EditTransitionReceipt>("complete_ai_reconciliation_recovery_reload", {
    leaseId,
    expectedReplacementSessionId,
  });
}

export function saveAiContextSnapshot(snapshot: UiContextProjection): Promise<AiContextStatus> {
  return invoke<AiContextStatus>("save_ai_context_snapshot", { snapshot });
}

export function readCodexMcpStatus(): Promise<CodexMcpStatus> {
  return invoke<CodexMcpStatus>("read_codex_mcp_status");
}

export function configureCodexMcp(): Promise<CodexMcpStatus> {
  return invoke<CodexMcpStatus>("configure_codex_mcp");
}
