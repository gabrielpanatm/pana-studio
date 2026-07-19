import { fileParentDirectory, projectDirectoriesFromFiles } from "$lib/project/files-drag";
import type { ProjectFile } from "$lib/types";

export type FileRenameRequest = {
  path: string;
  type?: "dir" | "file";
  newName: string;
};

export type FileRenameValidation = {
  allowed: boolean;
  code?:
    | "conflict"
    | "empty"
    | "invalid-name"
    | "missing-source"
    | "protected"
    | "same-name";
  reason?: string;
};

const ignoredProjectDirs = new Set([
  ".git",
  ".svelte-kit",
  "build",
  "dist",
  "node_modules",
  "target",
  ".panastudio_preview",
  ".panastudio",
]);

export function fileNameFromPath(path: string) {
  return path.split("/").pop() ?? path;
}

export function destinationPathForRename(sourcePath: string, newName: string) {
  const parent = fileParentDirectory(sourcePath);
  return parent ? `${parent}/${newName}` : newName;
}

export function remapPathAfterRename(activePath: string | null, sourcePath: string, destinationPath: string) {
  if (!activePath) return null;
  if (activePath === sourcePath) return destinationPath;
  if (activePath.startsWith(`${sourcePath}/`)) return `${destinationPath}${activePath.slice(sourcePath.length)}`;
  return activePath;
}

export function validateFileRenameRequest(
  files: ProjectFile[],
  request: FileRenameRequest,
): FileRenameValidation {
  const sourcePath = request.path.trim();
  const newName = request.newName.trim();
  if (!sourcePath) {
    return { allowed: false, code: "missing-source", reason: "Nu pot redenumi rădăcina proiectului." };
  }
  if (!newName) {
    return { allowed: false, code: "empty", reason: "Numele nu poate fi gol." };
  }
  if (newName === "." || newName === ".." || newName.includes("/") || newName.includes("\\") || newName.includes("\0")) {
    return { allowed: false, code: "invalid-name", reason: "Numele nu poate conține separatori, NUL, . sau ..." };
  }
  if (containsIgnoredDirectory(sourcePath)) {
    return { allowed: false, code: "protected", reason: "Nu pot redenumi elemente din foldere ignorate de proiect." };
  }

  const directories = projectDirectoriesFromFiles(files);
  const sourceFile = files.find((file) => file.relativePath === sourcePath);
  const sourceType = request.type ?? (sourceFile ? "file" : directories.has(sourcePath) ? "dir" : null);
  if (!sourceType) {
    return { allowed: false, code: "missing-source", reason: "Elementul sursă nu mai există în proiect." };
  }
  if (fileNameFromPath(sourcePath) === newName) {
    return { allowed: false, code: "same-name", reason: "Elementul are deja acest nume." };
  }

  const destinationPath = destinationPathForRename(sourcePath, newName);
  if (containsIgnoredDirectory(destinationPath)) {
    return { allowed: false, code: "protected", reason: "Nu pot redenumi către foldere ignorate de proiect." };
  }
  if (destinationConflict(files, sourcePath, destinationPath, sourceType)) {
    return {
      allowed: false,
      code: "conflict",
      reason: `Există deja un element numit ${newName} în acest dosar.`,
    };
  }

  return { allowed: true };
}

function containsIgnoredDirectory(path: string) {
  return path.split("/").some((segment) => ignoredProjectDirs.has(segment));
}

function destinationConflict(
  files: ProjectFile[],
  sourcePath: string,
  destinationPath: string,
  sourceType: "dir" | "file",
) {
  return files.some((file) => {
    if (file.relativePath === sourcePath || file.relativePath.startsWith(`${sourcePath}/`)) return false;
    if (sourceType === "file") return file.relativePath === destinationPath;
    return file.relativePath === destinationPath || file.relativePath.startsWith(`${destinationPath}/`);
  });
}
