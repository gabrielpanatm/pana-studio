import type {
  ProjectOpenRecoveryAssessment,
  ProjectOpenRecoveryDecisionInput,
} from "$lib/types";
import { t } from "$lib/i18n/runtime.svelte";

export const PROJECT_OPEN_RECOVERY_NOTIFICATION_ID = "project.open.recovery-decision";

export type ProjectOpenRecoveryDecisionRequest = {
  id: string;
  targetRoot: string;
  assessment: ProjectOpenRecoveryAssessment;
  operatorDecisionId: string | null;
  requestedAt: number;
};

export function createProjectOpenRecoveryDecisionRequest(
  targetRoot: string,
  assessment: ProjectOpenRecoveryAssessment,
  operatorDecisionId: string | null,
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

export function projectOpenRecoveryReasonLabel(
  assessment: ProjectOpenRecoveryAssessment,
) {
  switch (assessment.conflictReason) {
    case "project_root_replaced":
      return t("project-recovery-reason-root-replaced");
    case "recovery_invalid":
      return t("project-recovery-reason-invalid");
    case "disk_baseline_changed":
    default:
      return t("project-recovery-reason-disk-changed");
  }
}
