import type { ProjectScan } from "$lib/types";

export type DiskMutationKind = "scan" | "save" | "delete" | "move" | "rename" | "discard";

export type DiskState = {
  projectRoot: string;
  revision: number;
  scannedAt: number | null;
  fileCount: number;
  directoryCount: number;
  lastMutation: {
    kind: DiskMutationKind;
    at: number;
    path: string | null;
  } | null;
};

export function createDiskState(projectRoot = ""): DiskState {
  return {
    projectRoot,
    revision: 0,
    scannedAt: null,
    fileCount: 0,
    directoryCount: 0,
    lastMutation: null,
  };
}

export function diskStateFromProjectScan(project: ProjectScan, previous?: DiskState | null): DiskState {
  const directories = project.files.filter((file) => file.kind === "DIR").length;
  return {
    projectRoot: project.root,
    revision: previous?.projectRoot === project.root ? previous.revision : 0,
    scannedAt: Date.now(),
    fileCount: project.files.length - directories,
    directoryCount: directories,
    lastMutation: previous?.projectRoot === project.root ? previous.lastMutation : null,
  };
}

export function markDiskMutation(
  state: DiskState,
  kind: DiskMutationKind,
  path: string | null = null,
): DiskState {
  return {
    ...state,
    revision: state.revision + 1,
    lastMutation: {
      kind,
      at: Date.now(),
      path,
    },
  };
}

export function diskRuntimeSummary(state: DiskState) {
  return {
    revision: state.revision,
    files: `${state.fileCount} fișiere / ${state.directoryCount} dosare`,
    scannedAt: state.scannedAt
      ? new Intl.DateTimeFormat("ro-RO", {
          hour: "2-digit",
          minute: "2-digit",
          second: "2-digit",
        }).format(new Date(state.scannedAt))
      : "nescanat",
    lastMutation: state.lastMutation
      ? `${state.lastMutation.kind}${state.lastMutation.path ? `: ${state.lastMutation.path}` : ""}`
      : "fără mutații",
  };
}
