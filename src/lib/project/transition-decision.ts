import type {
  KernelProjectTransitionAction,
  KernelProjectTransitionPolicy,
  KernelProjectTransitionReason,
} from "$lib/project/transition-contract";
import { l10n, t } from "$lib/i18n/runtime.svelte";

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
  switch (reason) {
    case "no_open_project": return t("project-transition-reason-no-project");
    case "clean": return t("project-transition-reason-clean");
    case "metadata_changed": return t("project-transition-reason-metadata");
    case "workspace_dirty": return t("project-transition-reason-dirty");
    case "disk_conflict": return t("project-transition-reason-disk-conflict");
    case "blocked_project_state": return t("project-transition-reason-blocked");
    case "unknown_warning": return t("project-transition-reason-unknown");
  }
}

export function transitionActionLabel(action: KernelProjectTransitionAction) {
  switch (action) {
    case "open_project": return t("project-transition-action-open");
    case "reload_project": return t("project-transition-action-reload");
    case "close_project": return t("project-transition-action-close");
  }
}

export function localizedTransitionPolicyCopy(policy: KernelProjectTransitionPolicy) {
  const action = transitionActionLabel(policy.action);
  const reason = transitionReasonLabel(policy.reason);
  const title = policy.decision === "block"
    ? t("project-transition-policy-title-blocked", { action })
    : policy.decision === "confirm"
      ? t("project-transition-policy-title-confirm", { action })
      : t("project-transition-policy-title-allowed", { action });
  const message = t("project-transition-policy-message", { action, reason });
  const evidence = t("project-transition-policy-evidence", {
    dirty: policy.workspaceDirtyResourceCount,
    conflicts: policy.diskConflictCount,
    blocking: policy.diskBlockingCount,
    metadata: policy.metadataChangedCount,
  });
  const recommendedAction = policy.decision === "block"
    ? t("project-transition-policy-recommend-blocked")
    : policy.decision === "confirm"
      ? t("project-transition-policy-recommend-confirm")
      : t("project-transition-policy-recommend-allowed");
  return { title, message, evidence, recommendedAction };
}

export function projectTransitionDecisionMetrics(
  policy: KernelProjectTransitionPolicy,
): ProjectTransitionDecisionMetric[] {
  return [
    metric(
      t("project-transition-metric-dirty"),
      policy.workspaceDirtyResourceCount,
      policy.workspaceDirtyResourceCount > 0 ? "warning" : "neutral",
    ),
    metric(t("project-transition-metric-revision"), policy.workspaceRevision ?? "—", "neutral"),
    metric(
      t("project-transition-metric-history"),
      t("project-transition-history-value", {
        undo: policy.workspaceUndoCount,
        redo: policy.workspaceRedoCount,
      }),
      "neutral",
    ),
    metric(
      t("project-transition-metric-conflicts"),
      policy.diskConflictCount,
      policy.diskBlockingCount > 0 ? "danger" : "neutral",
    ),
    metric(
      t("project-transition-metric-metadata"),
      policy.metadataChangedCount,
      policy.metadataChangedCount > 0 ? "warning" : "neutral",
    ),
  ];
}

function metric(
  label: string,
  value: string | number,
  tone: ProjectTransitionDecisionMetric["tone"],
): ProjectTransitionDecisionMetric {
  return {
    label,
    value: typeof value === "number" ? l10n.formatNumber(value) : value,
    tone,
  };
}

function normalizeUiPath(path: string) {
  return path.replace(/[\\/]+$/, "");
}
