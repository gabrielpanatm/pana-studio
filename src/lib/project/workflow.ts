import type { ProjectFile, SourceLanguage } from "$lib/types";
import { zolaRelativePath } from "$lib/project/files";

export function isRenderedPreviewPageFile(file: ProjectFile) {
  return file.role === "page" && Boolean(file.previewPath);
}

export function isTemplateProjectFile(file: ProjectFile) {
  return file.role === "template";
}

export function preferredCenterViewForProjectFile(file: ProjectFile) {
  if (file.kind === "MD") {
    return "markdown";
  }

  if (isTemplateProjectFile(file) || (file.kind === "HTML" && isRenderedPreviewPageFile(file))) {
    return "preview";
  }

  return "code";
}

type PreviewEligibilityOptions = {
  activeScannedPath: string | null;
  sourceLanguage: SourceLanguage;
  hasActiveTemplateFile: boolean;
};

export function canPreviewCurrentSource(options: PreviewEligibilityOptions) {
  const zolaPath = options.activeScannedPath ? zolaRelativePath(options.activeScannedPath) : "";
  return (
    options.sourceLanguage === "html" ||
    (options.sourceLanguage === "markdown" && zolaPath.startsWith("content/")) ||
    options.hasActiveTemplateFile
  );
}
