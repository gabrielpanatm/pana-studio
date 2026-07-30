import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export const PROJECT_DISK_CHANGED_EVENT = "pana-project-disk-changed";

export type ProjectDiskChangeNotice = {
  schemaVersion: 1;
  projectRoot: string;
  runtimeSessionId: string;
  watchGeneration: number;
  watchRevision: number;
  changedPaths: string[];
  overflowed: boolean;
};

function isProjectDiskChangeNotice(value: unknown): value is ProjectDiskChangeNotice {
  if (!value || typeof value !== "object") return false;
  const notice = value as Partial<ProjectDiskChangeNotice>;
  return notice.schemaVersion === 1
    && typeof notice.projectRoot === "string"
    && notice.projectRoot.length > 0
    && typeof notice.runtimeSessionId === "string"
    && notice.runtimeSessionId.length > 0
    && Number.isSafeInteger(notice.watchGeneration)
    && (notice.watchGeneration ?? 0) > 0
    && Number.isSafeInteger(notice.watchRevision)
    && (notice.watchRevision ?? 0) > 0
    && Array.isArray(notice.changedPaths)
    && notice.changedPaths.every((path) => typeof path === "string")
    && typeof notice.overflowed === "boolean";
}

export function subscribeProjectDiskChanges(
  listener: (notice: ProjectDiskChangeNotice) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(PROJECT_DISK_CHANGED_EVENT, (event) => {
    if (!isProjectDiskChangeNotice(event.payload)) {
      console.error("[Pană Studio] Project disk watch event invalid", event.payload);
      return;
    }
    listener(event.payload);
  });
}
