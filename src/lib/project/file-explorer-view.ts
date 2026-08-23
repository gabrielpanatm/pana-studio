import type { FileExplorerEntry } from "$lib/project/file-explorer-contract";

export type FileExplorerViewRow = {
  type: "dir" | "file";
  name: string;
  path: string;
  depth: number;
  entry?: FileExplorerEntry;
  hasChildren: boolean;
  expanded: boolean;
};

export type FileExplorerRevealPlan = {
  entryId: string;
  collapsedDirs: Set<string>;
};

export function projectFileExplorerRows(
  entries: FileExplorerEntry[],
  collapsedDirs: Set<string>,
): FileExplorerViewRow[] {
  const entriesById = new Map(entries.map((entry) => [entry.id, entry]));
  const parentIds = new Set(
    entries
      .map((entry) => entry.parentId)
      .filter((parentId): parentId is string => Boolean(parentId)),
  );

  return entries
    .filter((entry) => {
      let parentId = entry.parentId;
      while (parentId) {
        const parent = entriesById.get(parentId);
        if (!parent || collapsedDirs.has(parent.relativePath)) return false;
        parentId = parent.parentId;
      }
      return true;
    })
    .map((entry) => {
      const hasChildren = parentIds.has(entry.id);
      return {
        type: entry.kind === "directory" ? "dir" : "file",
        name: entry.name,
        path: entry.relativePath,
        depth: entry.depth,
        entry,
        hasChildren,
        expanded:
          entry.kind === "directory"
          && hasChildren
          && !collapsedDirs.has(entry.relativePath),
      };
    });
}

export function planFileExplorerEntryReveal(
  entries: FileExplorerEntry[],
  collapsedDirs: Set<string>,
  relativePath: string,
): FileExplorerRevealPlan | null {
  const target = entries.find((entry) => entry.relativePath === relativePath);
  if (!target) return null;

  const entriesById = new Map(entries.map((entry) => [entry.id, entry]));
  const nextCollapsed = new Set(collapsedDirs);
  let parentId = target.parentId;
  while (parentId) {
    const parent = entriesById.get(parentId);
    if (!parent) break;
    nextCollapsed.delete(parent.relativePath);
    parentId = parent.parentId;
  }

  return {
    entryId: target.id,
    collapsedDirs: nextCollapsed,
  };
}
