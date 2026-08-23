import { scannedCacheKey } from "$lib/project/files";
import type {
  KernelExternalDiskReconcileReceipt,
  ProjectDiskManifest,
} from "$lib/project/external-disk-contract";
import { t } from "$lib/i18n/runtime.svelte";

export type ExternalSourceProjection = {
  sourceCache: Record<string, string>;
  activeSource: string | null;
};

export type ExternalReconcileUiLease = {
  projectRoot: string;
  kernelSessionId: string;
  projectSessionEpoch: number;
  activeRelativePath: string | null;
  editorMutationEpoch: number;
  selectionEpoch: number;
};

export function externalReconcileUiLeaseMatches(
  lease: ExternalReconcileUiLease,
  current: ExternalReconcileUiLease,
): boolean {
  return (
    lease.projectRoot === current.projectRoot &&
    lease.kernelSessionId === current.kernelSessionId &&
    lease.projectSessionEpoch === current.projectSessionEpoch &&
    lease.activeRelativePath === current.activeRelativePath &&
    lease.editorMutationEpoch === current.editorMutationEpoch &&
    lease.selectionEpoch === current.selectionEpoch
  );
}

export function acceptedExternalReconcileManifest(
  receipt: KernelExternalDiskReconcileReceipt,
  expectedRoot: string,
): ProjectDiskManifest {
  if (receipt.status !== "applied" && receipt.status !== "noop") {
    throw new Error(t("external-reconcile-status-cannot-advance", {
      status: receipt.status,
    }));
  }
  if (!receipt.acceptedManifest || receipt.acceptedManifest.root !== expectedRoot) {
    throw new Error(t("external-reconcile-manifest-missing"));
  }
  if (
    receipt.acceptedDiskGeneration === null
    || !Number.isSafeInteger(receipt.acceptedDiskGeneration)
    || receipt.acceptedDiskGeneration < 1
  ) {
    throw new Error(t("external-reconcile-generation-missing"));
  }
  return receipt.acceptedManifest;
}

export function projectExternalReconcileSources(
  sourceCache: Record<string, string>,
  receipt: KernelExternalDiskReconcileReceipt,
  activeRelativePath: string | null,
  activeFileChanged: boolean,
): ExternalSourceProjection {
  const nextCache = { ...sourceCache };
  for (const relativePath of receipt.invalidatedPaths) {
    delete nextCache[scannedCacheKey({ relativePath })];
  }

  if (!activeFileChanged || !activeRelativePath) {
    return { sourceCache: nextCache, activeSource: null };
  }
  if (!receipt.activeFile || receipt.activeFile.relativePath !== activeRelativePath) {
    throw new Error(t("external-reconcile-active-buffer-missing", {
      path: activeRelativePath,
    }));
  }
  nextCache[scannedCacheKey({ relativePath: activeRelativePath })] = receipt.activeFile.text;
  return { sourceCache: nextCache, activeSource: receipt.activeFile.text };
}
