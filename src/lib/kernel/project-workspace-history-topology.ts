import type { CommittedHistoryProjectionContext } from "$lib/state/project-controller";
import type { ProjectWorkspaceUndoRedoCommandReceipt } from "$lib/types";

export type ProjectWorkspaceHistoryTopologyHost = {
  activeScannedPath: string | null;
  rescanCurrentProjectForCommittedHistory: (
    context: CommittedHistoryProjectionContext,
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
  context: CommittedHistoryProjectionContext,
) {
  if (!projectWorkspaceHistoryChangesTopology(receipt)) return false;
  await host.rescanCurrentProjectForCommittedHistory(
    context,
    host.activeScannedPath,
    { strict: true, deferPreviewRefresh: true },
  );
  return true;
}
