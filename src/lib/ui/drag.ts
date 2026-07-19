export type DropPosition = "before" | "after" | "inside";

export function dropPositionFromPointer(
  event: Pick<MouseEvent, "clientY">,
  element: HTMLElement,
  options: { allowInside?: boolean } = {},
): DropPosition {
  const rect = element.getBoundingClientRect();
  const relativeY = rect.height > 0 ? (event.clientY - rect.top) / rect.height : 0.5;
  const allowInside = options.allowInside !== false;

  if (!allowInside) {
    return relativeY < 0.5 ? "before" : "after";
  }

  if (relativeY < 0.25) return "before";
  if (relativeY > 0.75) return "after";
  return "inside";
}
