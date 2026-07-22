import type {
  KernelProjectTransitionAction,
  KernelProjectTransitionPolicy,
  KernelProjectTransitionReason,
} from "$lib/types";

export type ProjectTransitionDecisionMetric = {
  label: string;
  value: string;
  tone: "neutral" | "warning" | "danger";
};

export type ProjectTransitionContinuation =
  | { kind: "open_project" }
  | { kind: "close_project" }
  | {
      kind: "reload_project";
      mode: "purge" | "discard";
      preferredRelativePath: string | null;
    };

export type ProjectTransitionDecisionRequest = {
  id: string;
  targetRoot: string;
  action: KernelProjectTransitionAction;
  policy: KernelProjectTransitionPolicy;
  continuation: ProjectTransitionContinuation;
  requestedAt: number;
};

export const PROJECT_TRANSITION_CONFIRM_NOTIFICATION_ID = "project.transition.confirm";
export const PROJECT_TRANSITION_BLOCKED_NOTIFICATION_ID = "project.transition.blocked";

export function projectTransitionActionForTarget(
  targetRoot: string,
  currentProjectRoot: string | null | undefined,
): KernelProjectTransitionAction {
  if (currentProjectRoot && normalizeUiPath(targetRoot) === normalizeUiPath(currentProjectRoot)) {
    return "reload_project";
  }
  return "open_project";
}

export function projectTransitionActionForContinuation(
  targetRoot: string,
  currentProjectRoot: string | null | undefined,
  continuation: ProjectTransitionContinuation,
): KernelProjectTransitionAction {
  if (continuation.kind === "close_project") return "close_project";
  if (continuation.kind === "reload_project") return "reload_project";
  return projectTransitionActionForTarget(targetRoot, currentProjectRoot);
}

export function createProjectTransitionDecisionRequest(
  targetRoot: string,
  currentProjectRoot: string | null | undefined,
  policy: KernelProjectTransitionPolicy,
  continuation: ProjectTransitionContinuation,
): ProjectTransitionDecisionRequest {
  const action = projectTransitionActionForContinuation(targetRoot, currentProjectRoot, continuation);
  return {
    id: [
      "project-transition",
      policy.sessionId ?? "no-session",
      action,
      policy.reason,
      Date.now().toString(36),
    ].join(":"),
    targetRoot,
    action,
    policy,
    continuation,
    requestedAt: Date.now(),
  };
}

export function transitionReasonLabel(reason: KernelProjectTransitionReason) {
  const labels: Record<KernelProjectTransitionReason, string> = {
    no_open_project: "fără proiect curent",
    clean: "sesiune curată",
    metadata_changed: "metadata schimbată",
    workspace_dirty: "modificări nesalvate",
    disk_conflict: "conflict pe disc",
    blocked_project_state: "stare proiect blocată",
    unknown_warning: "avertisment necunoscut",
  };
  return labels[reason];
}

export function transitionActionLabel(action: KernelProjectTransitionAction) {
  const labels: Record<KernelProjectTransitionAction, string> = {
    open_project: "Deschide proiectul",
    reload_project: "Reîncarcă proiectul",
    close_project: "Închide proiectul",
  };
  return labels[action];
}

export function projectTransitionDecisionMetrics(
  policy: KernelProjectTransitionPolicy,
): ProjectTransitionDecisionMetric[] {
  return [
    metric(
      "Modificări în sesiune",
      policy.workspaceDirtyResourceCount,
      policy.workspaceDirtyResourceCount > 0 ? "warning" : "neutral",
    ),
    metric("Revizie sesiune", policy.workspaceRevision ?? "—", "neutral"),
    metric("Istoric", `${policy.workspaceUndoCount} undo / ${policy.workspaceRedoCount} redo`, "neutral"),
    metric("Conflicte pe disc", policy.diskConflictCount, policy.diskBlockingCount > 0 ? "danger" : "neutral"),
    metric("Metadate schimbate", policy.metadataChangedCount, policy.metadataChangedCount > 0 ? "warning" : "neutral"),
  ];
}

function metric(
  label: string,
  value: string | number,
  tone: ProjectTransitionDecisionMetric["tone"],
): ProjectTransitionDecisionMetric {
  return { label, value: String(value), tone };
}

function normalizeUiPath(path: string) {
  return path.replace(/[\\/]+$/, "");
}
