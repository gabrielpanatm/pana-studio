import type {
  ProjectOpenRecoveryAssessment,
  ProjectOpenRecoveryDecisionInput,
} from "$lib/types";

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
    throw new Error("Recovery preflight nu cere o decizie explicită validă.");
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
    throw new Error("Cererea de recovery nu mai conține tokenul inspectat.");
  }
  return { action: "abandon", assessmentToken };
}

export function projectOpenRecoveryReasonLabel(
  assessment: ProjectOpenRecoveryAssessment,
) {
  switch (assessment.conflictReason) {
    case "project_root_replaced":
      return "dosar fizic înlocuit";
    case "recovery_invalid":
      return "recovery incompatibil";
    case "disk_baseline_changed":
    default:
      return "conținut schimbat pe disk";
  }
}
