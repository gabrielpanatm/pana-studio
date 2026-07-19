import type { KernelUndoRedoProjectionLease } from "$lib/kernel/undo-redo-projection-lease";
import type { ProjectWorkspaceUndoRedoCommandReceipt } from "$lib/types";

export type ProjectWorkspaceHistoryTopologyHost = {
  activeScannedPath: string | null;
  rescanCurrentProjectWithinKernelUndoRedoLease: (
    lease: KernelUndoRedoProjectionLease,
    preferredRelativePath: string | null,
    options: { strict?: boolean; deferPreviewRefresh?: boolean },
  ) => Promise<void>;
};

export function projectWorkspaceHistoryChangesTopology(
  receipt: ProjectWorkspaceUndoRedoCommandReceipt,
) {
  return receipt.result.entry.topologyPaths.length > 0;
}

/**
 * Rebuilds the ProjectSession catalog from the exact ProjectWorkspace
 * revision before Preview loads a route from that revision. If the active
 * page disappeared, the project rescan deterministically selects the first
 * renderable page and clears the stale Preview selection.
 */
export async function reconcileProjectWorkspaceTopologyAfterHistory(
  host: ProjectWorkspaceHistoryTopologyHost,
  receipt: ProjectWorkspaceUndoRedoCommandReceipt,
  lease: KernelUndoRedoProjectionLease,
) {
  if (!projectWorkspaceHistoryChangesTopology(receipt)) return false;
  await host.rescanCurrentProjectWithinKernelUndoRedoLease(
    lease,
    host.activeScannedPath,
    { strict: true, deferPreviewRefresh: true },
  );
  return true;
}
