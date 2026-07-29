import type { ProjectScan } from "$lib/types";
import { l10n, t } from "$lib/i18n/runtime.svelte";

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
    files: t("disk-state-counts", {
      files: state.fileCount,
      directories: state.directoryCount,
    }),
    scannedAt: state.scannedAt
      ? l10n.formatDate(state.scannedAt, {
          hour: "2-digit",
          minute: "2-digit",
          second: "2-digit",
        })
      : t("disk-state-not-scanned"),
    lastMutation: state.lastMutation
      ? t("disk-state-last-mutation", {
        kind: diskMutationKindLabel(state.lastMutation.kind),
        path: state.lastMutation.path ? `: ${state.lastMutation.path}` : "",
      })
      : t("disk-state-no-mutations"),
  };
}

function diskMutationKindLabel(kind: DiskMutationKind) {
  switch (kind) {
    case "scan": return t("disk-state-mutation-scan");
    case "save": return t("disk-state-mutation-save");
    case "delete": return t("disk-state-mutation-delete");
    case "move": return t("disk-state-mutation-move");
    case "rename": return t("disk-state-mutation-rename");
    case "discard": return t("disk-state-mutation-discard");
  }
}
