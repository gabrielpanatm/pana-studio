export type VersioningOperationLifecycle = Readonly<{
  busyAction: string;
  error: string;
}>;

export function beginVersioningOperation(
  action: string,
): VersioningOperationLifecycle {
  return { busyAction: action, error: "" };
}

export function failVersioningOperation(
  current: VersioningOperationLifecycle,
  error: string,
): VersioningOperationLifecycle {
  return { ...current, error };
}

export function finishVersioningOperation(
  current: VersioningOperationLifecycle,
): VersioningOperationLifecycle {
  return { ...current, busyAction: "" };
}
