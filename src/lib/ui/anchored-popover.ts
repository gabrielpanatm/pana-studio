export type AnchoredPopoverPlacement = {
  left: number;
  top: number;
  width: number;
  maxHeight: number;
};

export type AnchoredPopoverLayout = {
  anchorRect: DOMRect;
  scopeRect?: DOMRect;
  itemCount: number;
  itemHeight?: number;
  groupCount?: number;
  groupHeight?: number;
  chromeHeight?: number;
  minHeight?: number;
  maxHeight?: number;
  viewportMargin?: number;
  gap?: number;
  preferredWidth?: number;
  horizontalAlign?: "start" | "end";
  viewportWidth?: number;
  viewportHeight?: number;
};

const SCROLLABLE_OVERFLOW = /^(?:auto|scroll|overlay)$/;

export function anchoredPopoverScrollParents(anchor: HTMLElement): HTMLElement[] {
  const parents: HTMLElement[] = [];
  for (let parent = anchor.parentElement; parent; parent = parent.parentElement) {
    const style = window.getComputedStyle(parent);
    if (
      SCROLLABLE_OVERFLOW.test(style.overflowX) ||
      SCROLLABLE_OVERFLOW.test(style.overflowY)
    ) {
      parents.push(parent);
    }
  }
  return parents;
}

export function observeAnchoredPopoverPosition(
  anchor: HTMLElement,
  update: () => void,
): () => void {
  const scrollParents = anchoredPopoverScrollParents(anchor);
  for (const parent of scrollParents) {
    parent.addEventListener("scroll", update, { passive: true });
  }
  window.addEventListener("resize", update);
  window.addEventListener("scroll", update, { passive: true });

  return () => {
    for (const parent of scrollParents) {
      parent.removeEventListener("scroll", update);
    }
    window.removeEventListener("resize", update);
    window.removeEventListener("scroll", update);
  };
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(Math.max(value, minimum), maximum);
}

export function calculateAnchoredPopoverPlacement({
  anchorRect,
  scopeRect = anchorRect,
  itemCount,
  itemHeight = 28,
  groupCount = 0,
  groupHeight = 24,
  chromeHeight = 8,
  minHeight = 80,
  maxHeight = 240,
  viewportMargin = 8,
  gap = 4,
  preferredWidth,
  horizontalAlign = "start",
  viewportWidth = window.innerWidth,
  viewportHeight = window.innerHeight,
}: AnchoredPopoverLayout): AnchoredPopoverPlacement {
  const maximumViewportWidth = Math.max(0, viewportWidth - viewportMargin * 2);
  const width = Math.min(
    maximumViewportWidth,
    Math.max(
      anchorRect.width,
      preferredWidth ?? Math.min(scopeRect.width, maximumViewportWidth),
    ),
  );
  const left = clamp(
    horizontalAlign === "end" ? anchorRect.right - width : scopeRect.left,
    viewportMargin,
    Math.max(viewportMargin, viewportWidth - width - viewportMargin),
  );
  const spaceBelow = viewportHeight - anchorRect.bottom - viewportMargin;
  const spaceAbove = anchorRect.top - viewportMargin;
  const openAbove = spaceBelow < 180 && spaceAbove > spaceBelow;
  const availableSpace = Math.max(minHeight, openAbove ? spaceAbove : spaceBelow);
  const contentHeight = itemCount * itemHeight + groupCount * groupHeight + chromeHeight;
  const resolvedMaxHeight = Math.max(
    Math.min(minHeight, contentHeight),
    Math.min(maxHeight, contentHeight, Math.max(minHeight, availableSpace - gap)),
  );
  const top = openAbove
    ? Math.max(viewportMargin, anchorRect.top - gap - resolvedMaxHeight)
    : Math.min(anchorRect.bottom + gap, viewportHeight - viewportMargin - resolvedMaxHeight);

  return { left, top, width, maxHeight: resolvedMaxHeight };
}
