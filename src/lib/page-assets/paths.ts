import { logicalTemplateName, themeNameForZolaPath } from "$lib/project/files";

export function templateToPageSlug(templatePath: string) {
  const normalized = logicalTemplateName(templatePath)
    .trim()
    .replace(/\.html$/i, "")
    .replaceAll("/", "-")
    .replaceAll("_", "-");
  return normalized || "index";
}

export function pageScssRelativePath(templatePath: string) {
  const theme = themeNameForZolaPath(templatePath);
  const styleRoot = theme ? `themes/${theme}/sass` : "sass";
  return `${styleRoot}/pagini/${templateToPageSlug(templatePath)}.scss`;
}
