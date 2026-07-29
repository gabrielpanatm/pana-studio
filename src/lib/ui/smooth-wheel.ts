const PIXEL_DELTA_MODE = 0;
const LINE_DELTA_MODE = 1;
const CONTINUOUS_PIXEL_THRESHOLD = 40;
const LINE_HEIGHT_PX = 16;
const EASING_TIME_CONSTANT_MS = 72;

type ScrollAxis = "x" | "y";

type ScrollAnimation = {
  axis: ScrollAxis;
  element: HTMLElement;
  frame: number | null;
  lastTime: number;
  target: number;
};

type WheelDelta = {
  axis: ScrollAxis;
  amount: number;
};

type WheelDeltaInput = {
  deltaMode: number;
  deltaX: number;
  deltaY: number;
  pageHeightPx: number;
  pageWidthPx: number;
  shiftKey: boolean;
};

function finite(value: number): number {
  return Number.isFinite(value) ? value : 0;
}

export function isContinuousPixelWheel(
  deltaMode: number,
  deltaX: number,
  deltaY: number,
): boolean {
  return deltaMode === PIXEL_DELTA_MODE
    && Math.max(Math.abs(finite(deltaX)), Math.abs(finite(deltaY))) < CONTINUOUS_PIXEL_THRESHOLD;
}

export function normalizeWheelDelta(input: WheelDeltaInput): WheelDelta {
  let deltaX = finite(input.deltaX);
  let deltaY = finite(input.deltaY);
  if (input.deltaMode === LINE_DELTA_MODE) {
    deltaX *= LINE_HEIGHT_PX;
    deltaY *= LINE_HEIGHT_PX;
  } else if (input.deltaMode !== PIXEL_DELTA_MODE) {
    deltaX *= Math.max(1, finite(input.pageWidthPx));
    deltaY *= Math.max(1, finite(input.pageHeightPx));
  }

  if (input.shiftKey && Math.abs(deltaY) >= Math.abs(deltaX)) {
    return { axis: "x", amount: deltaY };
  }
  return Math.abs(deltaX) > Math.abs(deltaY)
    ? { axis: "x", amount: deltaX }
    : { axis: "y", amount: deltaY };
}

function overflowAllowsScroll(value: string): boolean {
  return value === "auto" || value === "scroll" || value === "overlay";
}

function scrollLimit(element: HTMLElement, axis: ScrollAxis): number {
  return axis === "x"
    ? Math.max(0, element.scrollWidth - element.clientWidth)
    : Math.max(0, element.scrollHeight - element.clientHeight);
}

function scrollPosition(element: HTMLElement, axis: ScrollAxis): number {
  return axis === "x" ? element.scrollLeft : element.scrollTop;
}

function setScrollPosition(element: HTMLElement, axis: ScrollAxis, value: number): void {
  if (axis === "x") element.scrollLeft = value;
  else element.scrollTop = value;
}

function canMove(element: HTMLElement, axis: ScrollAxis, amount: number): boolean {
  const limit = scrollLimit(element, axis);
  if (limit <= 0) return false;
  const position = scrollPosition(element, axis);
  return amount < 0 ? position > 0.5 : position < limit - 0.5;
}

function scrollableAxes(element: HTMLElement): { x: boolean; y: boolean } {
  const style = element.ownerDocument.defaultView?.getComputedStyle(element);
  if (!style) return { x: false, y: false };
  return {
    x: overflowAllowsScroll(style.overflowX),
    y: overflowAllowsScroll(style.overflowY),
  };
}

function elementPath(event: Event): HTMLElement[] {
  return event.composedPath().filter(
    (candidate): candidate is HTMLElement => candidate instanceof HTMLElement,
  );
}

export class SmoothWheelController {
  private readonly animations = new Map<HTMLElement, ScrollAnimation>();
  private readonly reducedMotionQuery: MediaQueryList;
  private readonly targetWindow: Window;

  constructor(targetWindow: Window) {
    this.targetWindow = targetWindow;
    this.reducedMotionQuery = targetWindow.matchMedia("(prefers-reduced-motion: reduce)");
    targetWindow.addEventListener("wheel", this.handleWheel, { passive: false });
    targetWindow.addEventListener("pointerdown", this.stopAll, { capture: true });
    targetWindow.addEventListener("keydown", this.stopAll, { capture: true });
    targetWindow.addEventListener("blur", this.stopAll);
  }

  dispose(): void {
    this.targetWindow.removeEventListener("wheel", this.handleWheel);
    this.targetWindow.removeEventListener("pointerdown", this.stopAll, { capture: true });
    this.targetWindow.removeEventListener("keydown", this.stopAll, { capture: true });
    this.targetWindow.removeEventListener("blur", this.stopAll);
    this.stopAll();
  }

  private readonly stopAll = () => {
    for (const animation of this.animations.values()) {
      if (animation.frame !== null) {
        this.targetWindow.cancelAnimationFrame(animation.frame);
      }
    }
    this.animations.clear();
  };

  private stopPathAnimations(path: HTMLElement[]): void {
    for (const element of path) {
      const animation = this.animations.get(element);
      if (!animation) continue;
      if (animation.frame !== null) {
        this.targetWindow.cancelAnimationFrame(animation.frame);
      }
      this.animations.delete(element);
    }
  }

  private resolveTarget(
    path: HTMLElement[],
    preferredAxis: ScrollAxis,
    amount: number,
  ): { axis: ScrollAxis; element: HTMLElement } | null {
    for (const element of path) {
      if (element.dataset.panaWheelSmoothing === "native") return null;
      const axes = scrollableAxes(element);
      if (axes[preferredAxis] && canMove(element, preferredAxis, amount)) {
        return { axis: preferredAxis, element };
      }
      if (axes[preferredAxis]) continue;
      const fallbackAxis = preferredAxis === "x" ? "y" : "x";
      if (axes[fallbackAxis] && canMove(element, fallbackAxis, amount)) {
        return { axis: fallbackAxis, element };
      }
    }
    return null;
  }

  private animate = (animation: ScrollAnimation, time: number) => {
    if (!animation.element.isConnected) {
      this.animations.delete(animation.element);
      return;
    }

    const elapsed = animation.lastTime === 0
      ? 16
      : Math.min(32, time - animation.lastTime);
    animation.lastTime = time;
    const current = scrollPosition(animation.element, animation.axis);
    const distance = animation.target - current;
    if (Math.abs(distance) < 0.5) {
      setScrollPosition(animation.element, animation.axis, animation.target);
      animation.frame = null;
      animation.lastTime = 0;
      this.animations.delete(animation.element);
      return;
    }

    const progress = 1 - Math.exp(-elapsed / EASING_TIME_CONSTANT_MS);
    setScrollPosition(
      animation.element,
      animation.axis,
      current + distance * progress,
    );
    animation.frame = this.targetWindow.requestAnimationFrame(
      (nextTime) => this.animate(animation, nextTime),
    );
  };

  private readonly handleWheel = (event: WheelEvent) => {
    if (
      event.defaultPrevented
      || event.ctrlKey
      || event.metaKey
      || this.reducedMotionQuery.matches
    ) {
      return;
    }

    const path = elementPath(event);
    if (isContinuousPixelWheel(event.deltaMode, event.deltaX, event.deltaY)) {
      this.stopPathAnimations(path);
      return;
    }

    const delta = normalizeWheelDelta({
      deltaMode: event.deltaMode,
      deltaX: event.deltaX,
      deltaY: event.deltaY,
      pageHeightPx: this.targetWindow.innerHeight,
      pageWidthPx: this.targetWindow.innerWidth,
      shiftKey: event.shiftKey,
    });
    if (delta.amount === 0) return;

    const target = this.resolveTarget(path, delta.axis, delta.amount);
    if (!target) return;

    const limit = scrollLimit(target.element, target.axis);
    const existing = this.animations.get(target.element);
    const currentTarget = existing?.axis === target.axis
      ? existing.target
      : scrollPosition(target.element, target.axis);
    const nextTarget = Math.min(limit, Math.max(0, currentTarget + delta.amount));
    if (Math.abs(nextTarget - currentTarget) < 0.5) return;

    event.preventDefault();
    if (existing && existing.axis !== target.axis && existing.frame !== null) {
      this.targetWindow.cancelAnimationFrame(existing.frame);
    }
    const animation: ScrollAnimation = existing?.axis === target.axis
      ? existing
      : {
          axis: target.axis,
          element: target.element,
          frame: null,
          lastTime: 0,
          target: currentTarget,
        };
    animation.target = nextTarget;
    this.animations.set(target.element, animation);
    if (animation.frame === null) {
      animation.lastTime = 0;
      animation.frame = this.targetWindow.requestAnimationFrame(
        (time) => this.animate(animation, time),
      );
    }
  };
}

export function installSmoothWheelScrolling(
  targetWindow: Window = window,
): () => void {
  const controller = new SmoothWheelController(targetWindow);
  return () => controller.dispose();
}
