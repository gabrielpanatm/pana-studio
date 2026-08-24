import type { ProjectPaneTab } from "$lib/application/contracts";

export function availableProjectPaneTabs(
  layersAvailable: boolean,
): ProjectPaneTab[] {
  return layersAvailable ? ["layers", "files"] : ["files"];
}

export function reconcileProjectPaneTab(
  current: ProjectPaneTab,
  layersAvailable: boolean,
): ProjectPaneTab {
  return layersAvailable ? current : "files";
}
