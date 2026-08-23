import { loadStoredUiPreferences } from "$lib/ui/preferences";
import {
  beginResizeDrag,
  clampResizeValue,
  clearResizeBodyClasses,
  defaultResizeValue,
  type ResizeKind,
} from "$lib/ui/resize";

export class WorkspaceLayoutState {
  leftPaneWidth = $state(defaultResizeValue("left"));
  rightPaneWidth = $state(defaultResizeValue("right"));
  terminalPaneHeight = $state(defaultResizeValue("terminal"));
  leftPaneCollapsed = $state(false);
  rightPaneCollapsed = $state(false);
  activeResizeKind = $state<ResizeKind | null>(null);
  activeResizeCleanup: (() => void) | null = null;

  toggleLeftPane() {
    this.leftPaneCollapsed = !this.leftPaneCollapsed;
  }

  toggleRightPane() {
    this.rightPaneCollapsed = !this.rightPaneCollapsed;
  }

  expandSidebars() {
    this.leftPaneCollapsed = false;
    this.rightPaneCollapsed = false;
  }

  initialize(storage: Storage) {
    const preferences = loadStoredUiPreferences(storage);
    if (preferences.leftPaneWidth !== null) {
      this.leftPaneWidth = clampResizeValue("left", preferences.leftPaneWidth);
    }
    if (preferences.rightPaneWidth !== null) {
      this.rightPaneWidth = clampResizeValue("right", preferences.rightPaneWidth);
    }
    if (preferences.terminalPaneHeight !== null) {
      this.terminalPaneHeight = clampResizeValue("terminal", preferences.terminalPaneHeight);
    }
  }

  resetResize(kind: ResizeKind) {
    if (kind === "left") this.leftPaneWidth = defaultResizeValue("left");
    else if (kind === "right") this.rightPaneWidth = defaultResizeValue("right");
    else this.terminalPaneHeight = defaultResizeValue("terminal");
    this.applyLiveResizeState();
  }

  stopResizeDrag() {
    const cleanup = this.activeResizeCleanup;
    this.activeResizeCleanup = null;
    this.activeResizeKind = null;
    cleanup?.();
    clearResizeBodyClasses();
  }

  startResizeDrag(kind: ResizeKind, event: PointerEvent) {
    if (event.button !== 0) return;
    this.stopResizeDrag();
    this.activeResizeKind = kind;
    this.activeResizeCleanup = beginResizeDrag({
      kind,
      event,
      state: this.dimensions(),
      applyLiveState: (nextState) => this.applyLiveResizeState(nextState),
      onUpdate: (nextState) => {
        this.leftPaneWidth = nextState.leftPaneWidth;
        this.rightPaneWidth = nextState.rightPaneWidth;
        this.terminalPaneHeight = nextState.terminalPaneHeight;
        this.applyLiveResizeState(nextState);
      },
      onStop: () => this.stopResizeDrag(),
    });
  }

  destroy() {
    this.stopResizeDrag();
  }

  private dimensions() {
    return {
      leftPaneWidth: this.leftPaneWidth,
      rightPaneWidth: this.rightPaneWidth,
      terminalPaneHeight: this.terminalPaneHeight,
    };
  }

  private applyLiveResizeState(nextState = this.dimensions()) {
    const workspace = document.querySelector<HTMLElement>(".workspace");
    workspace?.style.setProperty("--left-pane-width", `${nextState.leftPaneWidth}px`);
    workspace?.style.setProperty("--right-pane-width", `${nextState.rightPaneWidth}px`);
    const centerStack = document.querySelector<HTMLElement>(".center-stack");
    centerStack?.style.setProperty("--terminal-pane-height", `${nextState.terminalPaneHeight}px`);
  }
}
