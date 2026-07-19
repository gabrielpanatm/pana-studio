import type { FlatProjectFileNode } from "$lib/project/pane-tree";
import type { ProjectFile } from "$lib/types";

export type FileMoveRequest = {
  sourcePath: string;
  sourceType?: "dir" | "file";
  targetDirectory: string;
};

export type FileDropValidation = {
  allowed: boolean;
  code?:
    | "blocked"
    | "conflict"
    | "descendant"
    | "missing-source"
    | "missing-target"
    | "protected"
    | "same-folder"
    | "target-folder";
  reason?: string;
};

type FileDropContext = {
  files?: ProjectFile[];
  blockedReason?: string;
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

export function fileParentDirectory(path: string) {
  const index = path.lastIndexOf("/");
  return index === -1 ? "" : path.slice(0, index);
}

function fileNameFromPath(path: string) {
  return path.split("/").pop() ?? path;
}

function destinationPathForMove(sourcePath: string, targetDirectory: string) {
  const fileName = fileNameFromPath(sourcePath);
  return targetDirectory ? `${targetDirectory}/${fileName}` : fileName;
}

function fileNodeCommandPath(node: FlatProjectFileNode) {
  return node.commandPath;
}

function containsIgnoredDirectory(path: string) {
  return path.split("/").some((segment) => ignoredProjectDirs.has(segment));
}

function destinationConflict(
  files: ProjectFile[] | undefined,
  sourcePath: string,
  targetDirectory: string,
  sourceType: "dir" | "file",
) {
  if (!files) return false;
  const destinationPath = destinationPathForMove(sourcePath, targetDirectory);
  return files.some((file) => {
    if (file.relativePath === sourcePath || file.relativePath.startsWith(`${sourcePath}/`)) return false;
    if (sourceType === "file") return file.relativePath === destinationPath;
    return file.relativePath === destinationPath || file.relativePath.startsWith(`${destinationPath}/`);
  });
}

export function projectDirectoriesFromFiles(files: ProjectFile[]) {
  const directories = new Set<string>([""]);
  for (const file of files) {
    const parts = file.relativePath.split("/");
    let current = "";
    for (let index = 0; index < parts.length - 1; index += 1) {
      current = current ? `${current}/${parts[index]}` : parts[index];
      directories.add(current);
    }
  }
  return directories;
}

export function validateFileDrop(
  source: FlatProjectFileNode | null | undefined,
  target: FlatProjectFileNode | null | undefined,
  context: FileDropContext = {},
): FileDropValidation {
  if (!source) {
    return { allowed: false, code: "missing-source", reason: "Nu am găsit elementul de mutat." };
  }
  if (context.blockedReason) {
    return { allowed: false, code: "blocked", reason: context.blockedReason };
  }
  if (!target) {
    return { allowed: false, code: "missing-target", reason: "Alege un dosar destinație." };
  }
  if (target.type !== "dir") {
    return { allowed: false, code: "target-folder", reason: "Elementele pot fi mutate doar în dosare." };
  }
  const sourcePath = fileNodeCommandPath(source);
  const targetPath = fileNodeCommandPath(target);
  if (containsIgnoredDirectory(sourcePath) || containsIgnoredDirectory(targetPath)) {
    return { allowed: false, code: "protected", reason: "Nu pot muta în foldere ignorate de proiect." };
  }
  if (source.type === "dir" && (targetPath === sourcePath || targetPath.startsWith(`${sourcePath}/`))) {
    return { allowed: false, code: "descendant", reason: "Nu poți muta un dosar în el însuși sau în propriul copil." };
  }
  if (fileParentDirectory(sourcePath) === targetPath) {
    return { allowed: false, code: "same-folder", reason: "Elementul este deja în acest dosar." };
  }
  if (destinationConflict(context.files, sourcePath, targetPath, source.type)) {
    const fileName = fileNameFromPath(sourcePath);
    return {
      allowed: false,
      code: "conflict",
      reason: `Există deja un element numit ${fileName} în acest dosar.`,
    };
  }
  return { allowed: true };
}

export function validateFileMoveRequest(files: ProjectFile[], request: FileMoveRequest): FileDropValidation {
  const directories = projectDirectoriesFromFiles(files);
  const sourceFile = files.find((file) => file.relativePath === request.sourcePath);
  const sourceType = request.sourceType ?? (sourceFile ? "file" : directories.has(request.sourcePath) ? "dir" : null);
  if (!sourceType) {
    return { allowed: false, code: "missing-source", reason: "Elementul sursă nu mai există în proiect." };
  }
  if (containsIgnoredDirectory(request.sourcePath) || containsIgnoredDirectory(request.targetDirectory)) {
    return { allowed: false, code: "protected", reason: "Nu pot muta în foldere ignorate de proiect." };
  }
  if (sourceType === "dir" && (request.targetDirectory === request.sourcePath || request.targetDirectory.startsWith(`${request.sourcePath}/`))) {
    return { allowed: false, code: "descendant", reason: "Nu poți muta un dosar în el însuși sau în propriul copil." };
  }
  if (fileParentDirectory(request.sourcePath) === request.targetDirectory) {
    return { allowed: false, code: "same-folder", reason: "Elementul este deja în acest dosar." };
  }
  if (destinationConflict(files, request.sourcePath, request.targetDirectory, sourceType)) {
    const fileName = fileNameFromPath(request.sourcePath);
    return {
      allowed: false,
      code: "conflict",
      reason: `Există deja un element numit ${fileName} în acest dosar.`,
    };
  }
  if (!directories.has(request.targetDirectory)) {
    return { allowed: false, code: "missing-target", reason: "Folderul destinație nu mai există în proiect." };
  }
  return { allowed: true };
}

export function fileMoveHintLabel(sourcePath: string, targetDirectory: string) {
  const fileName = fileNameFromPath(sourcePath);
  const directoryName = targetDirectory ? fileNameFromPath(targetDirectory) : "root";
  return `Mută ${fileName} în ${directoryName}`;
}

export function fileDropBlockedLabel(validation: FileDropValidation) {
  switch (validation.code) {
    case "blocked":
      return "Salvează";
    case "conflict":
      return "Există deja";
    case "descendant":
      return "În copil";
    case "protected":
      return "Protejat";
    case "same-folder":
      return "Deja aici";
    case "target-folder":
    case "missing-target":
      return "Alege dosar";
    default:
      return "Interzis";
  }
}
