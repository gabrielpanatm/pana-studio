import type {
  ProjectOpenInspectionReceipt,
  ProjectOpenRecoveryAssessment,
  ProjectOpenRecoveryDecisionInput,
} from "$lib/project/lifecycle-contract";
import { t } from "$lib/i18n/runtime.svelte";

export const PROJECT_OPEN_RECOVERY_NOTIFICATION_ID = "project.open.recovery-decision";

export type ProjectOpenRecoveryDecisionRequest = {
  id: string;
  targetRoot: string;
  assessment: ProjectOpenRecoveryAssessment;
  operationId: string | null;
  candidateToken: string | null;
  inspection: ProjectOpenInspectionReceipt | null;
  operatorDecisionId: string | null;
  requestedAt: number;
};

export function createProjectOpenRecoveryDecisionRequest(
  targetRoot: string,
  assessment: ProjectOpenRecoveryAssessment,
  operatorDecisionId: string | null,
  operationId: string | null = null,
  candidateToken: string | null = null,
  inspection: ProjectOpenInspectionReceipt | null = null,
): ProjectOpenRecoveryDecisionRequest {
  if (assessment.status !== "decision_required" || !assessment.assessmentToken) {
    throw new Error(t("project-recovery-decision-not-required"));
  }
  return {
    id: [
      "project-open-recovery",
      assessment.assessmentToken.slice(0, 16),
      Date.now().toString(36),
    ].join(":"),
    targetRoot,
    assessment,
    operationId,
    candidateToken,
    inspection,
    operatorDecisionId,
    requestedAt: Date.now(),
  };
}

export function projectOpenRecoveryAbandonDecision(
  request: ProjectOpenRecoveryDecisionRequest,
): ProjectOpenRecoveryDecisionInput {
  const assessmentToken = request.assessment.assessmentToken;
  if (!assessmentToken || request.assessment.status !== "decision_required") {
    throw new Error(t("project-recovery-token-missing"));
  }
  return { action: "abandon", assessmentToken };
}
