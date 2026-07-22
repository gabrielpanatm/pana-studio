import { loadStoredUiPreferences, saveUiTheme } from "$lib/ui/preferences";
import {
  beginResizeDrag,
  clampResizeValue,
  clearResizeBodyClasses,
  defaultResizeValue,
  type ResizeKind,
} from "$lib/ui/resize";

export const DEFAULT_PREVIEW_ZOOM = 100;
const MIN_PREVIEW_ZOOM = 25;
const MAX_PREVIEW_ZOOM = 200;

export type UiControllerHost = {
  uiTheme: "dark" | "light";
  leftPaneWidth: number;
  rightPaneWidth: number;
  terminalPaneHeight: number;
  previewZoom: number;
  activeResizeKind: ResizeKind | null;
  activeResizeCleanup: (() => void) | null;
};

export function initUiFromStorage(host: UiControllerHost, storage: Storage) {
  const prefs = loadStoredUiPreferences(storage);
  if (prefs.theme) host.uiTheme = prefs.theme;
  syncDocumentTheme(host.uiTheme);
  if (prefs.leftPaneWidth !== null) host.leftPaneWidth = clampResizeValue("left", prefs.leftPaneWidth);
  if (prefs.rightPaneWidth !== null) host.rightPaneWidth = clampResizeValue("right", prefs.rightPaneWidth);
  if (prefs.terminalPaneHeight !== null) host.terminalPaneHeight = clampResizeValue("terminal", prefs.terminalPaneHeight);
}

export function toggleUiTheme(host: UiControllerHost, storage: Storage = window.localStorage) {
  host.uiTheme = host.uiTheme === "dark" ? "light" : "dark";
  saveUiTheme(storage, host.uiTheme);
  syncDocumentTheme(host.uiTheme);
}

function syncDocumentTheme(theme: "dark" | "light") {
  if (typeof document === "undefined") return;
  document.documentElement.dataset.panaTheme = theme;
  document.documentElement.style.colorScheme = theme;
  const themeMeta = document.querySelector('meta[name="theme-color"]');
  themeMeta?.setAttribute("content", theme === "light" ? "#edf1ee" : "#111315");
}

export function setPreviewZoom(host: UiControllerHost, value: number) {
  const rounded = Math.round(value);
  host.previewZoom = Math.min(MAX_PREVIEW_ZOOM, Math.max(MIN_PREVIEW_ZOOM, rounded));
}

export function resetPreviewZoom(host: UiControllerHost) {
  host.previewZoom = DEFAULT_PREVIEW_ZOOM;
}

export function resetResize(host: UiControllerHost, kind: ResizeKind) {
  if (kind === "left") host.leftPaneWidth = defaultResizeValue("left");
  else if (kind === "right") host.rightPaneWidth = defaultResizeValue("right");
  else host.terminalPaneHeight = defaultResizeValue("terminal");
  applyLiveResizeState({
    leftPaneWidth: host.leftPaneWidth,
    rightPaneWidth: host.rightPaneWidth,
    terminalPaneHeight: host.terminalPaneHeight,
  });
}

export function stopResizeDrag(host: UiControllerHost) {
  host.activeResizeCleanup?.();
  host.activeResizeCleanup = null;
  host.activeResizeKind = null;
  clearResizeBodyClasses();
}

export function startResizeDrag(host: UiControllerHost, kind: ResizeKind, event: MouseEvent) {
  stopResizeDrag(host);
  host.activeResizeKind = kind;
  host.activeResizeCleanup = beginResizeDrag({
    kind,
    event,
    state: {
      leftPaneWidth: host.leftPaneWidth,
      rightPaneWidth: host.rightPaneWidth,
      terminalPaneHeight: host.terminalPaneHeight,
    },
    applyLiveState: (nextState) => applyLiveResizeState(nextState),
    onUpdate: (nextState) => {
      host.leftPaneWidth = nextState.leftPaneWidth;
      host.rightPaneWidth = nextState.rightPaneWidth;
      host.terminalPaneHeight = nextState.terminalPaneHeight;
      applyLiveResizeState(nextState);
    },
    onStop: () => stopResizeDrag(host),
  });
}

function applyLiveResizeState(nextState: {
  leftPaneWidth: number;
  rightPaneWidth: number;
  terminalPaneHeight: number;
}) {
  const workspace = document.querySelector<HTMLElement>(".workspace");
  workspace?.style.setProperty("--left-pane-width", `${nextState.leftPaneWidth}px`);
  workspace?.style.setProperty("--right-pane-width", `${nextState.rightPaneWidth}px`);
  const centerStack = document.querySelector<HTMLElement>(".center-stack");
  centerStack?.style.setProperty("--terminal-pane-height", `${nextState.terminalPaneHeight}px`);
}
