export type TopbarUndoRedoDirection = "undo" | "redo";

export type TopbarUndoRedoRoute =
  | "none"
  | "workspace";

export type TopbarUndoRedoRouteInput = {
  kernelCanUndo: boolean;
  kernelCanRedo: boolean;
};

export type TopbarUndoRedoState = {
  canUndo: boolean;
  canRedo: boolean;
  undoRoute: TopbarUndoRedoRoute;
  redoRoute: TopbarUndoRedoRoute;
};

export function selectTopbarUndoRedoRoute(
  direction: TopbarUndoRedoDirection,
  input: TopbarUndoRedoRouteInput,
): TopbarUndoRedoRoute {
  const wantsUndo = direction === "undo";

  const kernelAvailable = wantsUndo ? input.kernelCanUndo : input.kernelCanRedo;
  return kernelAvailable ? "workspace" : "none";
}

export function topbarUndoRedoState(input: TopbarUndoRedoRouteInput): TopbarUndoRedoState {
  const undoRoute = selectTopbarUndoRedoRoute("undo", input);
  const redoRoute = selectTopbarUndoRedoRoute("redo", input);

  return {
    canUndo: undoRoute !== "none",
    canRedo: redoRoute !== "none",
    undoRoute,
    redoRoute,
  };
}
