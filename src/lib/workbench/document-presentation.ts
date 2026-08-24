import type { ProjectFile } from "$lib/project/lifecycle-contract";
import type {
  WorkbenchDocumentPresentation,
  WorkbenchDocumentSnapshot,
  WorkbenchSnapshot,
} from "$lib/workbench/contracts";

/** Projects the Rust ProjectFile kind into the Workbench command contract. */
export function workbenchPresentationForProjectFile(
  file: Pick<ProjectFile, "kind">,
): WorkbenchDocumentPresentation {
  return file.kind === "HTML" ? "html" : "code_only";
}

export function activeWorkbenchDocument(
  snapshot: WorkbenchSnapshot | null,
): WorkbenchDocumentSnapshot | null {
  if (!snapshot) return null;
  const group = snapshot.groups.find(
    (candidate) => candidate.groupId === snapshot.activeGroupId,
  );
  return group?.documents.find(
    (candidate) => candidate.documentId === group.activeDocumentId,
  ) ?? null;
}
