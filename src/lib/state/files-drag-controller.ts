import { semanticMoveProjectEntry } from "$lib/project/io";
import { validateFileMoveRequest, type FileMoveRequest } from "$lib/project/files-drag";
import type { ProjectScan, SaveState } from "$lib/types";
import { errorMessage } from "$lib/util";
import {
  previewStructuralCommandIdentity,
  previewStructuralSessionLeaseMatches,
  requireCurrentPreviewStructuralSession,
  runInPreviewStructuralLane,
  type PreviewStructuralSessionHost,
  type PreviewStructuralSessionLease,
} from "$lib/kernel/preview-structural-lane";

export type FilesDragControllerHost = PreviewStructuralSessionHost & {
  scannedProject: ProjectScan | null;
  activeScannedPath: string | null;
  rescanCurrentProject: (
    preferredRelativePath?: string | null,
    options?: { strict?: boolean },
  ) => Promise<void>;
  rescanCurrentProjectWithinStructuralLane: (
    lease: PreviewStructuralSessionLease,
    preferredRelativePath?: string | null,
    options?: { strict?: boolean },
  ) => Promise<void>;
  setGlobalStatus: (text: string, kind: SaveState) => void;
};

function remapProjectEntryPath(
  path: string | null,
  sourcePath: string,
  destinationPath: string,
) {
  if (!path) return null;
  if (path === sourcePath) return destinationPath;
  if (path.startsWith(`${sourcePath}/`)) {
    return `${destinationPath}${path.slice(sourcePath.length)}`;
  }
  return path;
}

export async function moveProjectFile(host: FilesDragControllerHost, request: FileMoveRequest) {
  const project = host.scannedProject;
  if (!project) {
    host.setGlobalStatus("Nu există proiect deschis pentru mutare.", "error");
    return;
  }

  const validation = validateFileMoveRequest(project.files, request);
  if (!validation.allowed) {
    host.setGlobalStatus(validation.reason ?? "Mutare invalidă.", "error");
    return;
  }

  await runInPreviewStructuralLane(host, async (lease) => {
    try {
      const identity = previewStructuralCommandIdentity(lease);
      const receipt = await semanticMoveProjectEntry(
        request.sourcePath,
        request.targetDirectory,
        identity,
      );
      requireCurrentPreviewStructuralSession(host, lease);
      const destinationPath = receipt.relativePath;
      if (!destinationPath) {
        throw new Error("Receipt-ul mutării nu conține path-ul destinație.");
      }
      requireCurrentPreviewStructuralSession(host, lease);
      const preferredPath = remapProjectEntryPath(
        host.activeScannedPath,
        request.sourcePath,
        destinationPath,
      );
      await host.rescanCurrentProjectWithinStructuralLane(lease, preferredPath, { strict: true });
      requireCurrentPreviewStructuralSession(host, lease);
      host.setGlobalStatus(`Fișier mutat în sesiune: ${request.sourcePath} → ${destinationPath} — Ctrl+S persistă pe disc`, "unsaved");
    } catch (error) {
      if (!previewStructuralSessionLeaseMatches(host, lease)) return;
      host.setGlobalStatus(`Eroare mutare fișier: ${errorMessage(error)}`, "error");
    }
  });
}
