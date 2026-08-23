import { createCssRequestIdentity, getScssVariables } from "$lib/css/io";
import {
  readCurrentProjectDiskManifest,
  reconcileCleanExternalProjectFiles,
} from "$lib/project/io/external-disk";
import { readProjectWorkspaceState } from "$lib/project/io/workspace";
import { scanProject } from "$lib/project/io/startup";
import { diffDiskManifests } from "$lib/project/disk-manifest";
import { preservePreviewBaseUrl } from "$lib/project/session";
import {
  acceptedExternalReconcileManifest,
  externalReconcileUiLeaseMatches,
  projectExternalReconcileSources,
  type ExternalReconcileUiLease,
} from "$lib/project/external-reconcile-projection";
import {
  invalidateFileBufferDraftSyncCursor,
} from "$lib/session/file-buffer-draft-sync";
import { flushWorkspaceMutationInputs } from "$lib/session/workspace-mutation-coordinator";
import type {
  KernelExternalDiskReconcileInput,
  KernelExternalDiskReconcileReceipt,
  ProjectDiskManifest,
} from "$lib/project/external-disk-contract";
import type { ProjectScan } from "$lib/project/lifecycle-contract";
import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";
import type {
  ExternalChangeFlags,
  ExternalDiskCheckLease,
  ExternalDiskContext,
} from "$lib/session/external-disk/contracts";
import {
  acceptAppliedExternalReconcile,
  acceptUnchangedExternalManifest,
  beginExternalDiskCheck,
  beginExternalDiskReconcile,
  finishExternalDiskCheck,
  finishExternalDiskReconcile,
  preserveBlockedExternalReceipt,
  preserveConcurrentUiMutationAfterCommit,
  preserveDirtyExternalChange,
  preserveProjectionFailureAfterCommit,
  preserveReloadRequiredExternalReceipt,
  preserveUninitializedExternalMonitor,
  publishDetectedExternalChanges,
  requireAcceptedExternalDiskGeneration,
} from "$lib/session/external-disk/state";
import { errorMessage } from "$lib/util";
import { t } from "$lib/i18n/runtime.svelte";

const EXTERNAL_PROJECTION_DEADLINE_MS = 30_000;

export type ExternalDiskReconcilePort = Readonly<{
  readManifest: () => Promise<ProjectDiskManifest>;
  reconcile: (
    input: KernelExternalDiskReconcileInput,
  ) => Promise<KernelExternalDiskReconcileReceipt>;
  readWorkspace: () => Promise<ProjectWorkspaceSnapshot | null>;
  scan: (root: string) => Promise<ProjectScan>;
  readScssVariables: typeof getScssVariables;
  flushInputs: typeof flushWorkspaceMutationInputs;
  projectionDeadlineMs: number;
}>;

const defaultReconcilePort: ExternalDiskReconcilePort = {
  readManifest: readCurrentProjectDiskManifest,
  reconcile: reconcileCleanExternalProjectFiles,
  readWorkspace: readProjectWorkspaceState,
  scan: scanProject,
  readScssVariables: getScssVariables,
  flushInputs: flushWorkspaceMutationInputs,
  projectionDeadlineMs: EXTERNAL_PROJECTION_DEADLINE_MS,
};

export function currentExternalDiskCheckLease(
  context: ExternalDiskContext,
): ExternalDiskCheckLease | null {
  const project = context.environment.session.project;
  if (!project?.root || !context.environment.session.runtimeSessionId) return null;
  return {
    projectRoot: project.root,
    runtimeSessionId: context.environment.session.runtimeSessionId,
    projectSessionEpoch: context.environment.session.epoch,
    generation: context.runtime.checkGeneration,
  };
}

export function externalDiskCheckBelongsToCurrentSession(
  context: ExternalDiskContext,
  lease: ExternalDiskCheckLease,
) {
  const { session } = context.environment;
  return Boolean(
    session.project
    && session.project.root === lease.projectRoot
    && session.runtimeSessionId === lease.runtimeSessionId
    && session.epoch === lease.projectSessionEpoch
  );
}

export function externalDiskCheckLeaseMatches(
  context: ExternalDiskContext,
  lease: ExternalDiskCheckLease,
) {
  return externalDiskCheckBelongsToCurrentSession(context, lease)
    && context.runtime.checkGeneration === lease.generation;
}

export async function runExternalDiskCheck(
  context: ExternalDiskContext,
  checkLease: ExternalDiskCheckLease,
  port: ExternalDiskReconcilePort = defaultReconcilePort,
) {
  const { environment, runtime } = context;
  if (
    !environment.session.project
    || runtime.suspended
    || environment.session.transitionLocked
    || environment.session.historyLocked
    || runtime.snapshot.checking
    || runtime.snapshot.reconciling
    || runtime.snapshot.workspaceProjectionRecoveryRequired
  ) return;
  if (!externalDiskCheckLeaseMatches(context, checkLease)) return;
  const expectedRoot = checkLease.projectRoot;
  const expectedSessionEpoch = checkLease.projectSessionEpoch;
  const reconcileGenerationAtStart = runtime.reconcileGeneration;
  beginExternalDiskCheck(context);

  try {
    if (runtime.suspended) {
      finishExternalDiskCheck(context);
      return;
    }
    const current = await port.readManifest();
    if (
      !externalDiskCheckLeaseMatches(context, checkLease)
      || reconcileGenerationAtStart !== runtime.reconcileGeneration
      || runtime.snapshot.reconciling
      || runtime.snapshot.workspaceProjectionRecoveryRequired
      || environment.session.epoch !== expectedSessionEpoch
      || environment.session.project?.root !== expectedRoot
      || current.root !== expectedRoot
    ) return;
    if (current.truncated || runtime.snapshot.baseline?.truncated) {
      preserveUninitializedExternalMonitor(context, current.root);
      return;
    }
    if (runtime.suspended) {
      finishExternalDiskCheck(context);
      return;
    }
    if (!runtime.snapshot.baseline || runtime.snapshot.baseline.root !== current.root) {
      preserveUninitializedExternalMonitor(context, current.root);
      return;
    }

    const diff = diffDiskManifests(runtime.snapshot.baseline, current);
    if (diff.changedFiles.length === 0) {
      acceptUnchangedExternalManifest(context, current);
      return;
    }
    const flags = {
      activeFileChanged: Boolean(
        environment.editor.activeScannedPath
        && diff.changedFiles.includes(environment.editor.activeScannedPath),
      ),
      previewRelevantChanged: diff.previewRelevantChanged,
    };
    const blockedByDirtySession = environment.editor.dirty;
    publishDetectedExternalChanges(
      context,
      current,
      diff.changedFiles,
      flags,
      blockedByDirtySession,
    );
    if (blockedByDirtySession) {
      preserveDirtyExternalChange(context, diff.changedFiles, flags);
      return;
    }
    await applyCleanExternalChanges(context, current, diff.changedFiles, flags, port);
  } catch (error) {
    if (
      !externalDiskCheckLeaseMatches(context, checkLease)
      || reconcileGenerationAtStart !== runtime.reconcileGeneration
      || environment.session.epoch !== expectedSessionEpoch
      || environment.session.project?.root !== expectedRoot
    ) return;
    finishExternalDiskCheck(context);
    environment.projections.setProjectStatus(t("external-disk-monitor-failed", {
      message: errorMessage(error),
    }));
  }
}

async function applyCleanExternalChanges(
  context: ExternalDiskContext,
  current: ProjectDiskManifest,
  changedFiles: string[],
  flags: ExternalChangeFlags,
  port: ExternalDiskReconcilePort,
) {
  const { environment, runtime } = context;
  if (!environment.session.project) return;
  if (runtime.snapshot.reconciling || runtime.snapshot.workspaceProjectionRecoveryRequired) return;
  const projectBeforeReconcile = environment.session.project;
  const expectedRoot = projectBeforeReconcile.root;
  const reconcileGeneration = ++runtime.reconcileGeneration;
  let rustReceiptAccepted = false;

  beginExternalDiskReconcile(context);
  environment.commands.quiesceInteractions();
  await environment.commands.waitForInteractionLock();

  try {
    await port.flushInputs("manual");
    if (!isCurrentReconcile(context, expectedRoot, reconcileGeneration)) return;
    const uiLease = currentExternalReconcileUiLease(context, expectedRoot);

    if (environment.editor.dirty) {
      preserveDirtyExternalChange(context, changedFiles, flags);
      return;
    }
    const receipt = await port.reconcile({
      expectedProjectRoot: expectedRoot,
      expectedSessionId: environment.session.runtimeSessionId,
      observedManifest: current,
      relativePaths: changedFiles,
      activeRelativePath: environment.editor.activeScannedPath,
    });
    if (!isCurrentReconcile(context, expectedRoot, reconcileGeneration)) return;
    if (
      receipt.projectRoot !== expectedRoot
      || receipt.sessionId !== environment.session.runtimeSessionId
    ) {
      throw new Error(t("external-disk-receipt-session-mismatch"));
    }
    if (receipt.status === "blocked" || receipt.status === "stale_evidence") {
      preserveBlockedExternalReceipt(context, changedFiles, flags, receipt);
      return;
    }
    if (receipt.status === "reload_required") {
      preserveReloadRequiredExternalReceipt(context, changedFiles, flags, receipt);
      return;
    }
    rustReceiptAccepted = true;
    if (receipt.workspaceRevision === null) {
      throw new Error(t("external-disk-revision-missing"));
    }

    const workspaceAfterCommit = await port.readWorkspace();
    if (!isCurrentReconcile(context, expectedRoot, reconcileGeneration)) return;
    if (
      !workspaceAfterCommit
      || workspaceAfterCommit.projectRoot !== expectedRoot
      || workspaceAfterCommit.runtimeSessionId !== environment.session.runtimeSessionId
      || workspaceAfterCommit.revision !== receipt.workspaceRevision
      || workspaceAfterCommit.dirty
    ) {
      throw new Error(t("external-disk-snapshot-mismatch"));
    }
    environment.projections.acceptWorkspace(workspaceAfterCommit);

    if (!externalReconcileUiLeaseMatches(
      uiLease,
      currentExternalReconcileUiLease(context, expectedRoot),
    )) {
      preserveConcurrentUiMutationAfterCommit(context, changedFiles, flags);
      return;
    }

    const acceptedManifest = acceptedExternalReconcileManifest(receipt, expectedRoot);
    const acceptedDiskGeneration = requireAcceptedExternalDiskGeneration(
      receipt,
      projectBeforeReconcile.acceptedDiskGeneration,
      runtime.snapshot.baseline,
      acceptedManifest,
    );
    for (const relativePath of receipt.invalidatedPaths) {
      invalidateFileBufferDraftSyncCursor(relativePath);
    }
    const sourceProjection = projectExternalReconcileSources(
      environment.editor.sourceCache,
      receipt,
      environment.editor.activeScannedPath,
      flags.activeFileChanged,
    );
    environment.projections.acceptSources(
      sourceProjection.sourceCache,
      sourceProjection.activeSource,
    );

    if (receipt.historyInvalidated) {
      await environment.commands.resetHistory();
      if (!isCurrentReconcile(context, expectedRoot, reconcileGeneration)) return;
    }
    if (receipt.projectionHints.projectRescan) {
      const scanned = await port.scan(expectedRoot);
      if (
        scanned.root !== receipt.projectRoot
        || scanned.kernelSessionId !== receipt.sessionId
        || scanned.workspaceRevision !== receipt.workspaceRevision
      ) {
        throw new Error(t("external-disk-scan-mismatch"));
      }
      const project = preservePreviewBaseUrl(scanned, projectBeforeReconcile);
      if (!isCurrentReconcile(context, expectedRoot, reconcileGeneration)) return;
      environment.projections.acceptProject(project);
    }
    if (receipt.projectionHints.sourceGraph) {
      if (!receipt.sourceGraphInvalidated) {
        throw new Error(t("external-disk-source-graph-not-invalidated"));
      }
      await environment.commands.refreshSourceGraph({ strict: true });
      if (!isCurrentReconcile(context, expectedRoot, reconcileGeneration)) return;
    }
    if (receipt.projectionHints.scss) {
      const cssIdentity = createCssRequestIdentity(receipt.projectRoot, receipt.sessionId);
      const nextScssVariables = await port.readScssVariables(
        cssIdentity,
        receipt.workspaceRevision ?? undefined,
      );
      if (
        !isCurrentReconcile(context, expectedRoot, reconcileGeneration)
        || environment.session.project?.root !== cssIdentity.expectedProjectRoot
        || environment.session.runtimeSessionId !== cssIdentity.expectedSessionId
      ) return;
      environment.projections.acceptScssVariables(nextScssVariables);
    }
    environment.projections.invalidateDerived();
    if (receipt.projectionHints.pageJs) environment.projections.invalidatePageJs();

    if (receipt.projectionHints.preview) {
      await withExternalProjectionDeadline(
        environment.commands.projectLatestPreview({
          reason: "external-change",
          minimumWorkspaceRevision: receipt.workspaceRevision,
          requestedPaths: receipt.requestedPaths,
        }),
        port.projectionDeadlineMs,
      );
      if (!isCurrentReconcile(context, expectedRoot, reconcileGeneration)) return;
    }
    if (!isCurrentReconcile(context, expectedRoot, reconcileGeneration)) return;
    if (!externalReconcileUiLeaseMatches(
      uiLease,
      currentExternalReconcileUiLease(context, expectedRoot),
    )) {
      preserveConcurrentUiMutationAfterCommit(context, changedFiles, flags);
      return;
    }

    const currentProject = environment.session.project;
    if (!currentProject) throw new Error(t("external-disk-receipt-session-mismatch"));
    environment.projections.acceptProject({
      ...currentProject,
      acceptedDiskGeneration,
      acceptedDiskManifest: acceptedManifest,
    });
    acceptAppliedExternalReconcile(context, acceptedManifest, changedFiles);
  } catch (error) {
    if (rustReceiptAccepted && isCurrentReconcile(context, expectedRoot, reconcileGeneration)) {
      preserveProjectionFailureAfterCommit(context, changedFiles, flags, error);
      return;
    }
    throw error;
  } finally {
    if (isCurrentReconcile(context, expectedRoot, reconcileGeneration)) {
      finishExternalDiskReconcile(context);
    }
  }
}

async function withExternalProjectionDeadline<T>(
  operation: Promise<T>,
  deadlineMs: number,
): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | null = null;
  const deadline = new Promise<never>((_resolve, reject) => {
    timer = setTimeout(() => {
      reject(new Error(t("external-disk-projection-timeout", {
        seconds: deadlineMs / 1000,
      })));
    }, deadlineMs);
  });
  try {
    return await Promise.race([operation, deadline]);
  } finally {
    if (timer !== null) clearTimeout(timer);
  }
}

function currentExternalReconcileUiLease(
  context: ExternalDiskContext,
  projectRoot: string,
): ExternalReconcileUiLease {
  return {
    projectRoot,
    kernelSessionId: context.environment.session.runtimeSessionId,
    projectSessionEpoch: context.environment.session.epoch,
    activeRelativePath: context.environment.editor.activeScannedPath,
    editorMutationEpoch: context.environment.editor.mutationEpoch,
    selectionEpoch: context.environment.editor.selectionEpoch,
  };
}

function isCurrentReconcile(
  context: ExternalDiskContext,
  expectedRoot: string,
  generation: number,
) {
  return generation === context.runtime.reconcileGeneration
    && context.environment.session.project?.root === expectedRoot;
}
