import type ActivityRail from "$lib/components/workbench/ActivityRail.svelte";
import type WorkspaceCenterArea from "$lib/components/workspace/WorkspaceCenterArea.svelte";
import type WorkspaceInspectorArea from "$lib/components/workspace/WorkspaceInspectorArea.svelte";
import type WorkspaceProjectArea from "$lib/components/workspace/WorkspaceProjectArea.svelte";

export type WorkspaceSurfaces = Readonly<{
  activityRail: typeof ActivityRail;
  centerArea: typeof WorkspaceCenterArea;
  inspectorArea: typeof WorkspaceInspectorArea;
  projectArea: typeof WorkspaceProjectArea;
}>;

let workspaceSurfaceLoad: Promise<WorkspaceSurfaces> | null = null;

/** Loads the complete project workspace boundary once, without boot-time imports. */
export function loadWorkspaceSurfaces(): Promise<WorkspaceSurfaces> {
  if (workspaceSurfaceLoad) return workspaceSurfaceLoad;
  workspaceSurfaceLoad = Promise.all([
    import("$lib/components/workbench/ActivityRail.svelte"),
    import("$lib/components/workspace/WorkspaceCenterArea.svelte"),
    import("$lib/components/workspace/WorkspaceInspectorArea.svelte"),
    import("$lib/components/workspace/WorkspaceProjectArea.svelte"),
  ]).then(([activityRail, centerArea, inspectorArea, projectArea]) => ({
    activityRail: activityRail.default,
    centerArea: centerArea.default,
    inspectorArea: inspectorArea.default,
    projectArea: projectArea.default,
  })).catch((error) => {
    workspaceSurfaceLoad = null;
    throw error;
  });
  return workspaceSurfaceLoad;
}
