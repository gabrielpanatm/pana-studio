export type KernelUndoRedoProjectionLease = {
  expectedProjectRoot: string;
  expectedSessionId: string;
  expectedSessionEpoch: number;
};

export type KernelUndoRedoProjectionLeaseHost = {
  sessionProjectRoot: string;
  kernelProjectSessionId: string;
  projectSessionEpoch: number;
  kernelUndoRedoFrontendLeaseActive?: boolean;
};

export function kernelUndoRedoProjectionLeaseMatches(
  host: KernelUndoRedoProjectionLeaseHost,
  lease: KernelUndoRedoProjectionLease,
) {
  return (
    host.kernelUndoRedoFrontendLeaseActive === true
    && host.sessionProjectRoot === lease.expectedProjectRoot
    && host.kernelProjectSessionId === lease.expectedSessionId
    && host.projectSessionEpoch === lease.expectedSessionEpoch
  );
}

export function requireCurrentKernelUndoRedoProjectionLease(
  host: KernelUndoRedoProjectionLeaseHost,
  lease: KernelUndoRedoProjectionLease,
  operation: string,
) {
  if (!kernelUndoRedoProjectionLeaseMatches(host, lease)) {
    throw new Error(
      t("history-projection-lease-expired", { operation }),
    );
  }
}
import { t } from "$lib/i18n/runtime.svelte";
