import { validateFileRenameRequest, remapPathAfterRename, type FileRenameRequest } from "$lib/project/files-rename";
import { semanticRenameProjectEntry, trashProjectEntry } from "$lib/project/io";
import type { ProjectScan, SaveState } from "$lib/types";
import { errorMessage } from "$lib/util";
import {
  previewStructuralCommandIdentity,
  previewStructuralSessionLeaseMatches,
  requireCurrentPreviewStructuralSession,
  runInPreviewStructuralLane,
  type PreviewStructuralSessionHost,
} from "$lib/kernel/preview-structural-lane";

export type ProjectEntryDeleteRequest = {
  path: string;
  type: "file" | "dir";
};

export type ProjectEntryRenameRequest = FileRenameRequest & {
  type: "file" | "dir";
};

export type FilesControllerHost = PreviewStructuralSessionHost & {
  scannedProject: ProjectScan | null;
  activeScannedPath: string | null;
  rescanCurrentProject: (
    preferredRelativePath?: string | null,
    options?: { strict?: boolean },
  ) => Promise<void>;
  rescanCurrentProjectWithinStructuralLane: (
    lease: Parameters<typeof previewStructuralCommandIdentity>[0],
    preferredRelativePath?: string | null,
    options?: { strict?: boolean },
  ) => Promise<void>;
  setGlobalStatus: (text: string, kind: SaveState) => void;
};

function activePathSurvivesDelete(activePath: string | null, deletedPath: string, type: "file" | "dir") {
  if (!activePath) return null;
  if (type === "file") return activePath === deletedPath ? null : activePath;
  return activePath === deletedPath || activePath.startsWith(`${deletedPath}/`) ? null : activePath;
}

export async function deleteProjectFile(host: FilesControllerHost, request: ProjectEntryDeleteRequest) {
  const project = host.scannedProject;
  if (!project) {
    host.setGlobalStatus("Nu există proiect deschis pentru ștergere.", "error");
    return;
  }
  if (!request.path) {
    host.setGlobalStatus("Nu pot șterge rădăcina proiectului.", "error");
    return;
  }

  await runInPreviewStructuralLane(host, async (lease) => {
    try {
      const identity = previewStructuralCommandIdentity(lease);
      const receipt = await trashProjectEntry(request.path, identity);
      requireCurrentPreviewStructuralSession(host, lease);
      requireCurrentPreviewStructuralSession(host, lease);
      const preferredPath = activePathSurvivesDelete(host.activeScannedPath, request.path, request.type);
      await host.rescanCurrentProjectWithinStructuralLane(lease, preferredPath, { strict: true });
      requireCurrentPreviewStructuralSession(host, lease);
      host.setGlobalStatus(`${request.type === "dir" ? "Dosar șters din sesiune" : "Fișier șters din sesiune"}: ${request.path} — Ctrl+S persistă pe disc`, "unsaved");
    } catch (error) {
      if (!previewStructuralSessionLeaseMatches(host, lease)) return;
      host.setGlobalStatus(`Eroare ștergere din sesiune: ${errorMessage(error)}`, "error");
    }
  });
}

export async function renameProjectFile(
  host: FilesControllerHost,
  request: ProjectEntryRenameRequest,
): Promise<boolean> {
  const project = host.scannedProject;
  if (!project) {
    host.setGlobalStatus("Nu există proiect deschis pentru redenumire.", "error");
    return false;
  }

  const validation = validateFileRenameRequest(project.files, request);
  if (!validation.allowed) {
    host.setGlobalStatus(validation.reason ?? "Redenumire invalidă.", "error");
    return false;
  }

  const newName = request.newName.trim();
  return await runInPreviewStructuralLane(host, async (lease) => {
    try {
      const identity = previewStructuralCommandIdentity(lease);
      const receipt = await semanticRenameProjectEntry(request.path, newName, identity);
      requireCurrentPreviewStructuralSession(host, lease);
      const destinationPath = receipt.relativePath;
      if (!destinationPath) {
        throw new Error("Receipt-ul redenumirii nu conține path-ul destinație.");
      }
      requireCurrentPreviewStructuralSession(host, lease);
      const preferredPath = remapPathAfterRename(host.activeScannedPath, request.path, destinationPath);
      await host.rescanCurrentProjectWithinStructuralLane(lease, preferredPath, { strict: true });
      requireCurrentPreviewStructuralSession(host, lease);
      host.setGlobalStatus(`Redenumit în sesiune: ${request.path} → ${destinationPath} — Ctrl+S persistă pe disc`, "unsaved");
      return true;
    } catch (error) {
      if (!previewStructuralSessionLeaseMatches(host, lease)) return false;
      host.setGlobalStatus(`Eroare redenumire: ${errorMessage(error)}`, "error");
      return false;
    }
  });
}
