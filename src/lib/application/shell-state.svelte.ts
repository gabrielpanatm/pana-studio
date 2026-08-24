import type {
  ApplicationSurface,
  CenterView,
  InspectorTab,
} from "$lib/application/contracts";

export type ApplicationSettingsSection = "general" | "ai" | "system" | "storage" | "about";

/** Owns only application-shell navigation, never project or editor state. */
export class ApplicationShellState {
  centerView = $state<CenterView>("preview");
  surface = $state<ApplicationSurface>("workbench");
  settingsSection = $state<ApplicationSettingsSection>("general");
  inspectorTab = $state<InspectorTab>("html");
  nativeWindowClosePending = false;
  nativeWindowCloseInProgress = false;

  openSettings(section: ApplicationSettingsSection = "general") {
    this.settingsSection = section;
    this.surface = "settings";
  }

  openWorkbench() {
    this.surface = "workbench";
  }
}
