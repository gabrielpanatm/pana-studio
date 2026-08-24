import type {
  WorkbenchIntent,
  WorkbenchSnapshot,
} from "$lib/workbench/contracts";

/**
 * Applies the narrow Rust-confirmed activation delta without invalidating the
 * complete Svelte Workbench projection. Any structural difference fails
 * closed so the caller can publish the full authoritative snapshot instead.
 */
export function patchExactWorkbenchDocumentActivation(
  current: WorkbenchSnapshot | null,
  next: WorkbenchSnapshot,
  intent: Extract<WorkbenchIntent, { kind: "activate_document" }>,
): boolean {
  if (
    !current
    || current.schemaVersion !== next.schemaVersion
    || current.projectRoot !== next.projectRoot
    || current.projectSessionId !== next.projectSessionId
    || current.runtimeSessionId !== next.runtimeSessionId
    || current.activeActivity !== next.activeActivity
    || current.split !== next.split
    || current.splitRatioBasisPoints !== next.splitRatioBasisPoints
    || next.activeGroupId !== intent.groupId
    || !sameJsonProjection(current.canvasViewport, next.canvasViewport)
    || !sameJsonProjection(current.bottomPanel, next.bottomPanel)
    || !sameJsonProjection(current.contentWorkspace, next.contentWorkspace)
    || current.groups.length !== next.groups.length
  ) return false;

  let targetPath: string | null = null;
  for (let index = 0; index < current.groups.length; index += 1) {
    const currentGroup = current.groups[index];
    const nextGroup = next.groups[index];
    if (
      !nextGroup
      || currentGroup.groupId !== nextGroup.groupId
      || currentGroup.documents.length !== nextGroup.documents.length
    ) return false;
    for (let documentIndex = 0; documentIndex < currentGroup.documents.length; documentIndex += 1) {
      const currentDocument = currentGroup.documents[documentIndex];
      const nextDocument = nextGroup.documents[documentIndex];
      if (!nextDocument || !sameWorkbenchDocument(currentDocument, nextDocument)) return false;
      if (
        nextGroup.groupId === intent.groupId
        && nextDocument.documentId === intent.documentId
      ) targetPath = nextDocument.relativePath;
    }
    if (
      nextGroup.groupId === intent.groupId
        ? nextGroup.activeDocumentId !== intent.documentId
        : nextGroup.activeDocumentId !== currentGroup.activeDocumentId
    ) return false;
  }
  if (
    !targetPath
    || next.selectedProjectEntry?.kind !== "text"
    || next.selectedProjectEntry.relativePath !== targetPath
  ) return false;

  current.revision = next.revision;
  current.activeGroupId = next.activeGroupId;
  const currentTargetGroup = current.groups.find((group) => group.groupId === intent.groupId);
  const nextTargetGroup = next.groups.find((group) => group.groupId === intent.groupId);
  if (!currentTargetGroup || !nextTargetGroup) return false;
  currentTargetGroup.activeDocumentId = nextTargetGroup.activeDocumentId;
  current.selectedProjectEntry = { ...next.selectedProjectEntry };
  return true;
}

/**
 * Applies the Rust-confirmed activity delta while preserving the identity of
 * every unrelated Workbench projection. Entering Content is the only activity
 * transition that may also reset its bounded workspace state in Rust.
 */
export function patchExactWorkbenchActivityChange(
  current: WorkbenchSnapshot | null,
  next: WorkbenchSnapshot,
  intent: Extract<WorkbenchIntent, { kind: "set_activity" }>,
): boolean {
  if (
    !current
    || next.activeActivity !== intent.activity
    || current.schemaVersion !== next.schemaVersion
    || current.projectRoot !== next.projectRoot
    || current.projectSessionId !== next.projectSessionId
    || current.runtimeSessionId !== next.runtimeSessionId
    || current.activeGroupId !== next.activeGroupId
    || current.split !== next.split
    || current.splitRatioBasisPoints !== next.splitRatioBasisPoints
    || !validActivityRevision(current, next)
    || !sameJsonProjection(current.canvasViewport, next.canvasViewport)
    || !sameJsonProjection(current.groups, next.groups)
    || !sameJsonProjection(current.bottomPanel, next.bottomPanel)
    || !sameJsonProjection(current.selectedProjectEntry, next.selectedProjectEntry)
    || !validActivityContentProjection(current, next, intent.activity)
  ) return false;

  current.revision = next.revision;
  current.activeActivity = next.activeActivity;
  if (intent.activity === "content") {
    current.contentWorkspace.mode = next.contentWorkspace.mode;
    current.contentWorkspace.pagePath = next.contentWorkspace.pagePath;
  }
  return true;
}

function validActivityRevision(
  current: WorkbenchSnapshot,
  next: WorkbenchSnapshot,
) {
  const changed = current.activeActivity !== next.activeActivity
    || !sameJsonProjection(current.contentWorkspace, next.contentWorkspace);
  return next.revision === current.revision + (changed ? 1 : 0);
}

function validActivityContentProjection(
  current: WorkbenchSnapshot,
  next: WorkbenchSnapshot,
  activity: Extract<WorkbenchIntent, { kind: "set_activity" }>["activity"],
) {
  if (activity !== "content") {
    return sameJsonProjection(current.contentWorkspace, next.contentWorkspace);
  }
  return next.contentWorkspace.mode === "list"
    && next.contentWorkspace.pagePath === null;
}

function sameWorkbenchDocument(
  current: WorkbenchSnapshot["groups"][number]["documents"][number],
  next: WorkbenchSnapshot["groups"][number]["documents"][number],
) {
  return current.documentId === next.documentId
    && current.relativePath === next.relativePath
    && current.title === next.title
    && current.presentation === next.presentation
    && current.surface === next.surface
    && current.pinned === next.pinned;
}

function sameJsonProjection(current: unknown, next: unknown) {
  return JSON.stringify(current) === JSON.stringify(next);
}
