import { diffDiskManifests } from "$lib/project/disk-manifest";
import type {
  ExternalDiskState as ExternalDiskSnapshot,
  KernelExternalDiskReconcileReceipt,
  ProjectDiskManifest,
} from "$lib/project/external-disk-contract";
import type {
  ExternalChangeFlags,
  ExternalDiskContext,
} from "$lib/session/external-disk/contracts";
import {
  EXTERNAL_CHANGE_KEEP_SESSION_ACTION_ID,
  EXTERNAL_CHANGE_NOTIFICATION_ID,
  EXTERNAL_CHANGE_RELOAD_ACTION_ID,
} from "$lib/session/external-disk/contracts";
import { errorMessage } from "$lib/util";
import { t } from "$lib/i18n/runtime.svelte";

export function createExternalDiskSnapshot(): ExternalDiskSnapshot {
  return {
    baseline: null,
    reconciling: false,
    changed: false,
    changedFiles: [],
    activeFileChanged: false,
    previewRelevantChanged: false,
    blockedByDirtySession: false,
    lastDetectedAt: null,
    lastDetectedFiles: [],
    lastDetectedActiveFileChanged: false,
    lastDetectedPreviewRelevantChanged: false,
    lastAppliedAt: null,
    lastAppliedFiles: [],
    lastCheckedAt: null,
    checking: false,
    workspaceProjectionRecoveryRequired: false,
    truncated: false,
  };
}

export function detachExternalDiskCheck(context: ExternalDiskContext) {
  context.runtime.checkGeneration += 1;
  context.runtime.checkInFlight = null;
}

export function invalidateExternalDiskOperations(context: ExternalDiskContext) {
  context.runtime.reconcileGeneration += 1;
  context.environment.projections.invalidateProjectSession();
  detachExternalDiskCheck(context);
}

export function resetExternalDiskSnapshot(context: ExternalDiskContext) {
  invalidateExternalDiskOperations(context);
  context.runtime.snapshot = createExternalDiskSnapshot();
  context.environment.commands.clearStatus(EXTERNAL_CHANGE_NOTIFICATION_ID);
}

export function invalidateExternalDiskForTransition(context: ExternalDiskContext) {
  invalidateExternalDiskOperations(context);
  const snapshot = context.runtime.snapshot;
  const reconcileMayHaveCommitted = snapshot.reconciling;
  context.runtime.snapshot = {
    ...snapshot,
    reconciling: true,
    checking: false,
    changed: reconcileMayHaveCommitted || snapshot.changed,
    blockedByDirtySession: reconcileMayHaveCommitted || snapshot.blockedByDirtySession,
    workspaceProjectionRecoveryRequired:
      reconcileMayHaveCommitted || snapshot.workspaceProjectionRecoveryRequired,
  };
  context.environment.commands.quiesceInteractions();
}

export function rollbackExternalDiskTransition(context: ExternalDiskContext) {
  invalidateExternalDiskOperations(context);
  context.runtime.snapshot = {
    ...context.runtime.snapshot,
    reconciling: false,
    checking: false,
  };
}

export function markExternalDiskProjectionRecovery(
  context: ExternalDiskContext,
  message: string,
) {
  invalidateExternalDiskOperations(context);
  const snapshot = context.runtime.snapshot;
  context.runtime.snapshot = {
    ...snapshot,
    reconciling: false,
    checking: false,
    changed: true,
    blockedByDirtySession: true,
    workspaceProjectionRecoveryRequired: true,
  };
  context.environment.commands.escalateStatus({
    id: EXTERNAL_CHANGE_NOTIFICATION_ID,
    level: "error",
    title: t("external-disk-reprojection-title"),
    message,
    actionLabel: t("external-disk-reload"),
    actionId: EXTERNAL_CHANGE_RELOAD_ACTION_ID,
  });
}

export function establishExternalDiskBaseline(context: ExternalDiskContext) {
  const { environment, runtime } = context;
  if (!environment.session.project) return;
  if (
    runtime.snapshot.checking
    || runtime.snapshot.reconciling
    || runtime.snapshot.workspaceProjectionRecoveryRequired
  ) return;
  const expectedRoot = environment.session.project.root;
  const manifest = environment.session.project.acceptedDiskManifest;
  const acceptedDiskGeneration = environment.session.project.acceptedDiskGeneration;
  if (
    !manifest
    || manifest.root !== expectedRoot
    || manifest.truncated
    || !Number.isSafeInteger(acceptedDiskGeneration)
    || (acceptedDiskGeneration ?? 0) < 1
    || !environment.session.project.kernelSessionId
    || environment.session.project.kernelSessionId !== environment.session.runtimeSessionId
  ) {
    preserveUninitializedExternalMonitor(context, expectedRoot);
    return;
  }
  runtime.snapshot = {
    ...createExternalDiskSnapshot(),
    baseline: manifest,
    lastCheckedAt: Date.now(),
    truncated: false,
  };
  environment.commands.clearStatus(EXTERNAL_CHANGE_NOTIFICATION_ID);
}

export function acceptExternalDiskSaveBaseline(
  context: ExternalDiskContext,
  acceptedManifest: ProjectDiskManifest,
  acceptedDiskGeneration: number,
) {
  const project = context.environment.session.project;
  if (
    !project
    || project.root !== acceptedManifest.root
    || project.kernelSessionId !== context.environment.session.runtimeSessionId
    || !Number.isSafeInteger(acceptedDiskGeneration)
    || acceptedDiskGeneration < 1
    || acceptedManifest.truncated
  ) {
    throw new Error(t("external-disk-save-baseline-invalid"));
  }
  context.runtime.checkGeneration += 1;
  context.environment.projections.acceptProject({
    ...project,
    acceptedDiskGeneration,
    acceptedDiskManifest: acceptedManifest,
  });
  context.runtime.snapshot = {
    ...createExternalDiskSnapshot(),
    baseline: acceptedManifest,
    lastCheckedAt: Date.now(),
  };
  context.environment.commands.clearStatus(EXTERNAL_CHANGE_NOTIFICATION_ID);
}

export function beginExternalDiskCheck(context: ExternalDiskContext) {
  context.runtime.snapshot = {
    ...context.runtime.snapshot,
    checking: true,
  };
}

export function finishExternalDiskCheck(context: ExternalDiskContext) {
  context.runtime.snapshot = {
    ...context.runtime.snapshot,
    checking: false,
    lastCheckedAt: Date.now(),
  };
}

export function acceptUnchangedExternalManifest(
  context: ExternalDiskContext,
  current: ProjectDiskManifest,
) {
  const snapshot = context.runtime.snapshot;
  context.runtime.snapshot = {
    ...snapshot,
    baseline: current,
    reconciling: false,
    changed: false,
    changedFiles: [],
    activeFileChanged: false,
    previewRelevantChanged: false,
    blockedByDirtySession: false,
    lastCheckedAt: Date.now(),
    checking: false,
    workspaceProjectionRecoveryRequired: false,
    truncated: current.truncated,
  };
  context.environment.commands.clearStatus(EXTERNAL_CHANGE_NOTIFICATION_ID);
}

export function publishDetectedExternalChanges(
  context: ExternalDiskContext,
  current: ProjectDiskManifest,
  changedFiles: string[],
  flags: ExternalChangeFlags,
  blockedByDirtySession: boolean,
) {
  const snapshot = context.runtime.snapshot;
  context.runtime.snapshot = {
    ...snapshot,
    reconciling: false,
    changed: true,
    changedFiles,
    activeFileChanged: flags.activeFileChanged,
    previewRelevantChanged: flags.previewRelevantChanged,
    blockedByDirtySession,
    lastDetectedAt: Date.now(),
    lastDetectedFiles: changedFiles,
    lastDetectedActiveFileChanged: flags.activeFileChanged,
    lastDetectedPreviewRelevantChanged: flags.previewRelevantChanged,
    lastCheckedAt: Date.now(),
    checking: false,
    truncated: current.truncated,
  };
}

export function beginExternalDiskReconcile(context: ExternalDiskContext) {
  context.runtime.snapshot = {
    ...context.runtime.snapshot,
    reconciling: true,
    checking: true,
    workspaceProjectionRecoveryRequired: false,
  };
}

export function finishExternalDiskReconcile(context: ExternalDiskContext) {
  context.runtime.snapshot = {
    ...context.runtime.snapshot,
    reconciling: false,
    checking: false,
  };
}

export function acceptAppliedExternalReconcile(
  context: ExternalDiskContext,
  manifest: ProjectDiskManifest,
  changedFiles: string[],
) {
  const snapshot = context.runtime.snapshot;
  context.runtime.snapshot = {
    ...snapshot,
    baseline: manifest,
    reconciling: true,
    changed: false,
    changedFiles: [],
    activeFileChanged: false,
    previewRelevantChanged: false,
    blockedByDirtySession: false,
    lastAppliedAt: Date.now(),
    lastAppliedFiles: changedFiles,
    lastCheckedAt: Date.now(),
    checking: false,
    workspaceProjectionRecoveryRequired: false,
    truncated: manifest.truncated,
  };
  context.environment.commands.clearStatus(EXTERNAL_CHANGE_NOTIFICATION_ID);
  context.environment.commands.setStatus(
    t("external-disk-reloaded", { files: formatChangedFiles(changedFiles) }),
    "restored",
  );
}

export function preserveDirtyExternalChange(
  context: ExternalDiskContext,
  changedFiles: string[],
  flags: ExternalChangeFlags,
) {
  context.runtime.snapshot = conflictSnapshot(context, changedFiles, flags, {
    blockedByDirtySession: true,
  });
  escalateBlockedExternalChange(context, changedFiles);
}

export function preserveConcurrentUiMutationAfterCommit(
  context: ExternalDiskContext,
  changedFiles: string[],
  flags: ExternalChangeFlags,
) {
  const message = t("external-disk-concurrent-ui-message");
  context.runtime.snapshot = recoverySnapshot(context, changedFiles, flags);
  context.environment.commands.escalateStatus({
    id: EXTERNAL_CHANGE_NOTIFICATION_ID,
    level: "error",
    title: t("external-disk-concurrent-ui-title"),
    message,
    statusMessage: message,
    actionLabel: t("external-disk-reload"),
    actionId: EXTERNAL_CHANGE_RELOAD_ACTION_ID,
  });
}

export function preserveUninitializedExternalMonitor(
  context: ExternalDiskContext,
  observedRoot: string,
) {
  const message = t("external-disk-baseline-unverified-message", { root: observedRoot });
  const snapshot = context.runtime.snapshot;
  context.runtime.snapshot = recoverySnapshot(context, snapshot.changedFiles, {
    activeFileChanged: snapshot.activeFileChanged,
    previewRelevantChanged: snapshot.previewRelevantChanged,
  });
  context.environment.commands.escalateStatus({
    id: EXTERNAL_CHANGE_NOTIFICATION_ID,
    level: "error",
    title: t("external-disk-baseline-unverified-title"),
    message,
    statusMessage: message,
    actionLabel: t("external-disk-reload"),
    actionId: EXTERNAL_CHANGE_RELOAD_ACTION_ID,
  });
}

export function preserveProjectionFailureAfterCommit(
  context: ExternalDiskContext,
  changedFiles: string[],
  flags: ExternalChangeFlags,
  error: unknown,
) {
  const message = t("external-disk-projection-failed-message", {
    message: errorMessage(error),
  });
  context.runtime.snapshot = recoverySnapshot(context, changedFiles, flags);
  context.environment.commands.escalateStatus({
    id: EXTERNAL_CHANGE_NOTIFICATION_ID,
    level: "error",
    title: t("external-disk-projection-recovery-title"),
    message,
    statusMessage: message,
    actionLabel: t("external-disk-reload"),
    actionId: EXTERNAL_CHANGE_RELOAD_ACTION_ID,
  });
}

export function preserveBlockedExternalReceipt(
  context: ExternalDiskContext,
  changedFiles: string[],
  flags: ExternalChangeFlags,
  receipt: KernelExternalDiskReconcileReceipt,
) {
  context.runtime.snapshot = conflictSnapshot(context, changedFiles, flags, {
    blockedByDirtySession: true,
  });
  const verdict = localizedExternalReconcileVerdict(receipt);
  context.environment.commands.escalateStatus({
    id: EXTERNAL_CHANGE_NOTIFICATION_ID,
    level: "warning",
    title: t("external-disk-reconcile-blocked-title"),
    message: verdict,
    statusMessage: verdict,
    actionLabel: t("external-disk-reload"),
    actionId: EXTERNAL_CHANGE_RELOAD_ACTION_ID,
    secondaryActionLabel: t("external-disk-keep-session"),
    secondaryActionId: EXTERNAL_CHANGE_KEEP_SESSION_ACTION_ID,
  });
}

export function preserveReloadRequiredExternalReceipt(
  context: ExternalDiskContext,
  changedFiles: string[],
  flags: ExternalChangeFlags,
  receipt: KernelExternalDiskReconcileReceipt,
) {
  context.runtime.snapshot = conflictSnapshot(context, changedFiles, flags, {
    blockedByDirtySession: false,
  });
  if (context.environment.session.aiLocked) {
    context.environment.commands.setStatus(t("external-disk-ai-structure-detected"), "saving");
    return;
  }
  const verdict = localizedExternalReconcileVerdict(receipt);
  context.environment.commands.escalateStatus({
    id: EXTERNAL_CHANGE_NOTIFICATION_ID,
    level: "warning",
    title: t("external-disk-structure-changed-title"),
    message: verdict,
    statusMessage: verdict,
    actionLabel: t("external-disk-reload"),
    actionId: EXTERNAL_CHANGE_RELOAD_ACTION_ID,
    secondaryActionLabel: t("external-disk-keep-session"),
    secondaryActionId: EXTERNAL_CHANGE_KEEP_SESSION_ACTION_ID,
  });
}

export function escalateBlockedExternalChange(
  context: ExternalDiskContext,
  changedFiles: string[],
) {
  context.environment.commands.escalateStatus({
    id: EXTERNAL_CHANGE_NOTIFICATION_ID,
    level: "warning",
    title: t("external-disk-files-changed-title"),
    message: t("external-disk-files-changed-message", {
      files: formatChangedFiles(changedFiles),
    }),
    statusMessage: t("external-disk-files-changed-status"),
    actionLabel: t("external-disk-reload"),
    actionId: EXTERNAL_CHANGE_RELOAD_ACTION_ID,
    secondaryActionLabel: t("external-disk-keep-session"),
    secondaryActionId: EXTERNAL_CHANGE_KEEP_SESSION_ACTION_ID,
  });
}

export function requireAcceptedExternalDiskGeneration(
  receipt: KernelExternalDiskReconcileReceipt,
  currentGeneration: number | undefined,
  beforeManifest: ProjectDiskManifest | null,
  acceptedManifest: ProjectDiskManifest,
): number {
  const acceptedDiskGeneration = receipt.acceptedDiskGeneration;
  if (
    !Number.isSafeInteger(currentGeneration)
    || (currentGeneration ?? 0) < 1
    || acceptedDiskGeneration === null
    || !Number.isSafeInteger(acceptedDiskGeneration)
    || acceptedDiskGeneration < 1
    || !beforeManifest
    || beforeManifest.root !== acceptedManifest.root
    || beforeManifest.truncated
  ) {
    throw new Error(t("external-disk-generation-invalid"));
  }
  const changedFiles = diffDiskManifests(beforeManifest, acceptedManifest).changedFiles;
  const expectedGeneration = currentGeneration! + (changedFiles.length > 0 ? 1 : 0);
  if (acceptedDiskGeneration !== expectedGeneration) {
    throw new Error(t("external-disk-generation-stale", {
      expected: expectedGeneration,
      actual: acceptedDiskGeneration,
    }));
  }
  return acceptedDiskGeneration;
}

function conflictSnapshot(
  context: ExternalDiskContext,
  changedFiles: string[],
  flags: ExternalChangeFlags,
  patch: Pick<ExternalDiskSnapshot, "blockedByDirtySession">,
): ExternalDiskSnapshot {
  return {
    ...context.runtime.snapshot,
    changed: true,
    changedFiles,
    activeFileChanged: flags.activeFileChanged,
    previewRelevantChanged: flags.previewRelevantChanged,
    blockedByDirtySession: patch.blockedByDirtySession,
    checking: false,
    lastCheckedAt: Date.now(),
  };
}

function recoverySnapshot(
  context: ExternalDiskContext,
  changedFiles: string[],
  flags: ExternalChangeFlags,
): ExternalDiskSnapshot {
  return {
    ...conflictSnapshot(context, changedFiles, flags, { blockedByDirtySession: true }),
    reconciling: false,
    workspaceProjectionRecoveryRequired: true,
  };
}

function formatChangedFiles(files: string[]) {
  if (files.length <= 3) return files.join(", ");
  return `${files.slice(0, 3).join(", ")} +${files.length - 3}`;
}

function localizedExternalReconcileVerdict(
  receipt: KernelExternalDiskReconcileReceipt,
): string {
  const diagnostic = receipt.diagnostics[0]?.messageDiagnostic;
  if (diagnostic) return errorMessage(diagnostic);
  if (receipt.status === "reload_required") return t("external-disk-verdict-reload-required");
  if (receipt.status === "stale_evidence") return t("external-disk-verdict-stale");
  if (receipt.status === "blocked") return t("external-disk-verdict-blocked");
  if (receipt.status === "applied") {
    return t("external-disk-verdict-applied", {
      content: receipt.reconciledCount,
      metadata: receipt.metadataRefreshedCount,
    });
  }
  return t("external-disk-verdict-noop");
}
