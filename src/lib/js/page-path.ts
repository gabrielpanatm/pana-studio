import { logicalTemplateName } from "$lib/project/files";

export function normalizePageJsTemplatePath(templatePath: string | null | undefined): string {
  return String(templatePath ?? "")
    .trim()
    .replaceAll("\\", "/")
    .replace(/^\.\//, "");
}

export function templateToPageJsSlug(templatePath: string) {
  const normalized = logicalTemplateName(templatePath)
    .trim()
    .replace(/\.html$/, "")
    .replace(/[/_]/g, "-");
  return `pana-${normalized}`;
}

export function pageJsRelativePath(templatePath: string) {
  return `static/js/${templateToPageJsSlug(templatePath)}.js`;
}
