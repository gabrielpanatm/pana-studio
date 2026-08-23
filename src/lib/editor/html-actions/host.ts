import type { HtmlDraftState } from "$lib/state/html-draft-session.svelte";
import type { GlobalStatusKind } from "$lib/status/global-status";
import type {
  CoordinatedElementSelection,
  HtmlPendingArea,
} from "$lib/canvas/contracts";
import type { SelectionSnapshot } from "$lib/editor/contracts";
import type {
  PreviewSelectionBatchExecutionReceipt,
} from "$lib/preview/contracts";
import type { PreviewStructuralExecutionReceipt } from "$lib/kernel/preview-projection-control";
import type {
  WorkspaceMutationAuthorityReceipt,
  WorkspaceMutationSettlement,
  WorkspaceMutationSettlementOptions,
} from "$lib/session/workspace-mutation-coordinator";
import type { PreviewStructuralSessionLease } from "$lib/kernel/preview-structural-lane";
import type {
  ProjectFile,
  ProjectScan,
} from "$lib/project/lifecycle-contract";
import type { SourceEditLocation } from "$lib/source-graph/contracts";

export type HtmlActionsHost = {
  context: () => Readonly<{
    coordinatedSelection: CoordinatedElementSelection | null;
    canEditStructure: boolean;
    activeScannedPath: string | null;
    project: ProjectScan | null;
  }>;
  html: {
    structureStatus: string;
    imageStatus: string;
    imageSourceValue: string;
    classStatus: string;
    classEditorValue: string;
  };
  draft: HtmlDraftState;
  source: {
    source: string;
    sourceCache: Record<string, string>;
  };
  editorSelection: {
    selectionSnapshot: SelectionSnapshot | null;
  };
  structural: {
    run: <T>(
      operation: (lease: PreviewStructuralSessionLease) => Promise<T>,
    ) => Promise<T>;
    leaseMatches: (lease: PreviewStructuralSessionLease) => boolean;
    projectCommitted: (
      lease: PreviewStructuralSessionLease,
      receipt: PreviewStructuralExecutionReceipt,
      patch: NonNullable<PreviewStructuralExecutionReceipt["patch"]>,
      projectLocalState: () => Promise<void> | void,
    ) => Promise<WorkspaceMutationSettlement>;
    projectCommittedBatch: (
      lease: PreviewStructuralSessionLease,
      receipt: PreviewSelectionBatchExecutionReceipt,
    ) => Promise<WorkspaceMutationSettlement>;
    settleMutation: (
      receipt: WorkspaceMutationAuthorityReceipt,
      options?: WorkspaceMutationSettlementOptions,
    ) => Promise<WorkspaceMutationSettlement>;
  };
  commands: {
    setPending: (area: HtmlPendingArea, pending: boolean) => void;
    setStatus: (text: string, kind: GlobalStatusKind) => void;
    loadProjectFile: (file: ProjectFile) => Promise<void>;
    reconcilePageAssets: (tpl: SourceEditLocation) => Promise<unknown>;
  };
};
