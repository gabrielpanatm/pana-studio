import type { CanvasProjectionIdentity } from "$lib/contracts/canvas-projection";
import type {
  PreviewRuntimeEventKind,
  PreviewStylesheetPromotionMetrics,
} from "$lib/contracts/canvas-projection";
import type { EditorActionOutcome } from "$lib/editor-runtime/action-outcome";
import type { EditorRuntime } from "$lib/editor-runtime/runtime";
import type { GlobalStatusKind } from "$lib/status/global-status";
import type { EditorSelectionSessionController } from "$lib/state/editor-selection-session.svelte";
import type {
  ApplicationSurface,
  CenterView,
} from "$lib/application/contracts";
import type { CoordinatedElementSelection } from "$lib/canvas/contracts";
import type { EditorMovePlan } from "$lib/editor/contracts";
import type { ProjectMovePosition } from "$lib/preview/contracts";
import type { ProjectScan } from "$lib/project/lifecycle-contract";
import type { SourceGraph } from "$lib/source-graph/graph-contract";
import type { WorkbenchSnapshot } from "$lib/workbench/contracts";

export type CanvasInteractionControllerHost = {
  session: {
    activeCanvasIdentity: CanvasProjectionIdentity | null;
    activeCanvasUrl: string;
    activeScannedPath: string | null;
    applicationSurface: ApplicationSurface;
    browserPreviewRoute: string;
    centerView: CenterView;
    previewFrame: HTMLIFrameElement | undefined;
    previewSrc: string;
    scannedProject: ProjectScan | null;
    workbenchSnapshot: WorkbenchSnapshot | null;
  };
  selection: {
    coordinatedElementSelection: CoordinatedElementSelection | null;
    editorSelection: Pick<
      EditorSelectionSessionController,
      | "acceptObservation"
      | "applyHoverIntent"
      | "applySelectionIntent"
      | "beginCanvasHoverProjection"
      | "clearSelectionProjection"
      | "editScopeGrant"
      | "editScopeId"
      | "navigationSnapshot"
      | "projectCanvasHoverReceipt"
      | "refreshNavigationSnapshot"
      | "selectionSnapshot"
    >;
    sourceGraph: SourceGraph | null;
  };
  runtime: {
    editorRuntime: EditorRuntime;
    gridOverlayEnabled: boolean;
  };
  commands: {
    closeContextMenu: () => void;
    moveEditorNavigationNode: (
      sourceNodeId: string,
      targetNodeId: string,
      position: ProjectMovePosition,
      preplanned?: EditorMovePlan | null,
      inputEmittedAtMs?: number,
    ) => Promise<EditorActionOutcome>;
    postPreviewMessage: (payload: Record<string, unknown>) => void;
    previewEditorNavigationMove: (
      sourceNodeId: string,
      targetNodeId: string,
      position: ProjectMovePosition,
    ) => Promise<EditorMovePlan>;
    recordCanvasProjectionRuntimeEvent: (
      kind: PreviewRuntimeEventKind,
      identity: CanvasProjectionIdentity,
      durationMs: number,
      diagnostic: string | null,
      stylesheetMetrics?: PreviewStylesheetPromotionMetrics | null,
    ) => Promise<void>;
    setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
    syncCodeSelectionHighlight: (reveal?: boolean) => void;
  };
};
