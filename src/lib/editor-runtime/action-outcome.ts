export type EditorActionStatus =
  | "committed"
  | "noop"
  | "blocked"
  | "cancelled"
  | "failed";

export type EditorActionOutcome = Readonly<{
  status: EditorActionStatus;
  reason?: string;
}>;

function outcome(status: EditorActionStatus, reason?: string): EditorActionOutcome {
  return Object.freeze(reason ? { status, reason } : { status });
}

export function committedAction(): EditorActionOutcome {
  return outcome("committed");
}

export function noopAction(reason?: string): EditorActionOutcome {
  return outcome("noop", reason);
}

export function blockedAction(reason: string): EditorActionOutcome {
  return outcome("blocked", reason);
}

export function cancelledAction(reason?: string): EditorActionOutcome {
  return outcome("cancelled", reason);
}

export function failedAction(reason: string): EditorActionOutcome {
  return outcome("failed", reason);
}

export function editorActionSucceeded(result: EditorActionOutcome): boolean {
  return result.status === "committed" || result.status === "noop";
}
