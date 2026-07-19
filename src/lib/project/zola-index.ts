import {
  extractTemplateNameFromSource,
  isActiveThemeTemplatePath,
  isLocalTemplatePath,
  logicalTemplateName,
  scannedCacheKey,
  templateNameForPath,
  zolaRelativePath,
} from "$lib/project/files";
import { readProjectFile } from "$lib/project/io";
import type { ProjectFile, ProjectScan } from "$lib/types";

function findTemplate(project: ProjectScan, templateName: string | null) {
  if (!templateName) return null;
  const logicalName = logicalTemplateName(templateName);
  if (!logicalName) return null;

  const candidates = project.files.filter(
    (file) =>
      file.role === "template"
      && templateNameForPath(file.relativePath) === logicalName
      && isActiveThemeTemplatePath(file.relativePath, project.activeTheme),
  );

  return candidates.find((file) => isLocalTemplatePath(file.relativePath)) ?? candidates[0] ?? null;
}

export async function resolveZolaIndexTemplateFile(
  project: ProjectScan,
  sourceCache: Record<string, string>,
  onCacheUpdate: (relativePath: string, cacheKey: string, source: string) => void,
): Promise<ProjectFile | null> {
  if (!project.isZola) return null;

  const rootPage =
    project.files.find((file) => file.role === "page" && zolaRelativePath(file.relativePath) === "content/_index.md") ?? null;

  if (rootPage) {
    const cacheKey = scannedCacheKey(rootPage);
    let rootPageSource = sourceCache[cacheKey];

    if (!rootPageSource) {
      rootPageSource = await readProjectFile(rootPage.relativePath);
      onCacheUpdate(rootPage.relativePath, cacheKey, rootPageSource);
    }

    const explicitTemplate = findTemplate(project, extractTemplateNameFromSource(rootPageSource));
    if (explicitTemplate) return explicitTemplate;
  }

  return findTemplate(project, "index.html");
}
