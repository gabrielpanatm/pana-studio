import type {
  ApplicationSurface,
  CenterView,
  InspectorTab,
} from "$lib/application/contracts";

/** Owns only application-shell navigation, never project or editor state. */
export class ApplicationShellState {
  centerView = $state<CenterView>("preview");
  surface = $state<ApplicationSurface>("workbench");
  inspectorTab = $state<InspectorTab>("html");
  nativeWindowClosePending = false;
  nativeWindowCloseInProgress = false;

  openSettings() {
    this.surface = "settings";
  }

  openWorkbench() {
    this.surface = "workbench";
  }
}
