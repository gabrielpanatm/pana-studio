import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export const PROJECT_WORKSPACE_MUTATED_EVENT = "pana-project-workspace-mutated";

export type ProjectWorkspaceMutationNotice = {
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number;
  dirty: boolean;
  previewProjectionRequired: boolean;
};

type ProjectWorkspaceMutationListener = (
  notice: ProjectWorkspaceMutationNotice,
) => void;

function validNotice(value: unknown): value is ProjectWorkspaceMutationNotice {
  if (!value || typeof value !== "object") return false;
  const notice = value as Partial<ProjectWorkspaceMutationNotice>;
  return typeof notice.projectRoot === "string"
    && notice.projectRoot.trim().length > 0
    && typeof notice.runtimeSessionId === "string"
    && notice.runtimeSessionId.trim().length > 0
    && Number.isSafeInteger(notice.workspaceRevision)
    && (notice.workspaceRevision ?? -1) >= 0
    && typeof notice.dirty === "boolean"
    && typeof notice.previewProjectionRequired === "boolean";
}

/**
 * Rust emits this event only after the authoritative ProjectWorkspace recovery
 * snapshot is durable. Frontend consumers may invalidate derived views, but
 * they never infer or mutate workspace state from the event payload.
 */
export function subscribeProjectWorkspaceMutations(
  listener: ProjectWorkspaceMutationListener,
): Promise<UnlistenFn> {
  return listen<unknown>(PROJECT_WORKSPACE_MUTATED_EVENT, (event) => {
    if (!validNotice(event.payload)) {
      console.error("[Pană Studio] ProjectWorkspace mutation event invalid", event.payload);
      return;
    }
    listener(event.payload);
  });
}
