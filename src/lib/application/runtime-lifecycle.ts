import { clearPreviewTimers } from "$lib/state/preview-controller";
import type { AiContextState } from "$lib/ai/context-state.svelte";
import type { AiCoordinationState } from "$lib/ai/coordination-state.svelte";
import type { SourceWorkspaceState } from "$lib/editor/source-workspace.svelte";
import type { EditorInteractionRuntime } from "$lib/editor/interaction-runtime.svelte";
import type { ProjectStartupState } from "$lib/project/startup-state.svelte";
import type { PreviewWorkspaceState } from "$lib/preview/workspace-state.svelte";
import type { SelectionWorkspaceState } from "$lib/editor/selection-workspace.svelte";
import type { ExternalDiskState } from "$lib/session/external-disk-state.svelte";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import type { TerminalWorkspaceState } from "$lib/terminal/workspace.svelte";

export type ApplicationRuntimeLifecycleDependencies = {
  status: Pick<GlobalStatusState, "refreshGlobalStatusFromKernel">;
  project: {
    reattach: () => Promise<boolean>;
    startup: Pick<ProjectStartupState, "refreshFlow">;
  };
  preview: PreviewWorkspaceState;
  terminal: Pick<TerminalWorkspaceState, "destroy">;
  source: SourceWorkspaceState;
  selection: SelectionWorkspaceState;
  ai: {
    context: Pick<AiContextState, "clear">;
    coordination: Pick<AiCoordinationState, "start" | "stop">;
  };
  externalDisk: Pick<ExternalDiskState, "stop">;
  editor: Pick<EditorInteractionRuntime, "destroy">;
};

export async function initializeApplicationRuntime(
  dependencies: ApplicationRuntimeLifecycleDependencies,
) {
  await dependencies.status.refreshGlobalStatusFromKernel();
  dependencies.ai.coordination.start();
  try {
    const reattached = await dependencies.project.reattach();
    if (!reattached) await dependencies.project.startup.refreshFlow();
  } catch {
    // Reattachment already publishes its persistent diagnostic; keep Startup interactive.
    await dependencies.project.startup.refreshFlow().catch(() => undefined);
  }
}

export function destroyApplicationRuntime(
  dependencies: ApplicationRuntimeLifecycleDependencies,
) {
  dependencies.editor.destroy();
  dependencies.preview.runtime.reset();
  dependencies.terminal.destroy();
  dependencies.source.controller?.destroy();
  dependencies.source.controller = null;
  if (dependencies.selection.pendingRestoredTimer !== null) {
    window.clearTimeout(dependencies.selection.pendingRestoredTimer);
    dependencies.selection.pendingRestoredTimer = null;
  }
  clearPreviewTimers(dependencies.preview.commands());
  dependencies.ai.context.clear();
  dependencies.ai.coordination.stop();
  dependencies.externalDisk.stop();
}
