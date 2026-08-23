import type { SourceLanguage } from "$lib/application/contracts";
import type {
  ProjectFile,
  ProjectScan,
} from "$lib/project/lifecycle-contract";

export function scannedCacheKey(file: Pick<ProjectFile, "relativePath">) {
  return `scanned:${file.relativePath}`;
}

export function templateNameForPath(relativePath: string) {
  const templateName = logicalTemplateName(relativePath);
  return templateName || relativePath.split("/").pop() || relativePath;
}

export function zolaRelativePath(relativePath: string) {
  return relativePath;
}

export function projectRelativeZolaPath(relativePath: string) {
  return relativePath;
}

export function normalizedZolaPath(relativePath: string) {
  return zolaRelativePath(relativePath).trim().replace(/^\/+/, "").replaceAll("\\", "/");
}

export function themeNameForZolaPath(relativePath: string) {
  return normalizedZolaPath(relativePath).match(/^themes\/([^/]+)\//)?.[1] ?? null;
}

export function logicalTemplateName(templatePath: string) {
  const normalized = normalizedZolaPath(templatePath);
  const themed = normalized.match(/^themes\/[^/]+\/templates\/(.+)$/);
  if (themed) return themed[1];
  return normalized.replace(/^templates\//, "");
}

export function isZolaTemplatePath(relativePath: string) {
  const normalized = normalizedZolaPath(relativePath);
  return normalized.startsWith("templates/") || /^themes\/[^/]+\/templates\//.test(normalized);
}

export function isActiveThemeTemplatePath(relativePath: string, activeTheme?: string | null) {
  const normalized = normalizedZolaPath(relativePath);
  if (normalized.startsWith("templates/")) return true;
  return Boolean(activeTheme && normalized.startsWith(`themes/${activeTheme}/templates/`));
}

export function detectSourceLanguage(path: string): SourceLanguage {
  const lowerPath = path.toLowerCase();

  if (lowerPath.endsWith(".html") || lowerPath.endsWith(".htm")) {
    return "html";
  }

  if (lowerPath.endsWith(".css")) {
    return "css";
  }

  if (lowerPath.endsWith(".scss")) {
    return "scss";
  }

  if (lowerPath.endsWith(".js") || lowerPath.endsWith(".mjs")) {
    return "js";
  }

  if (lowerPath.endsWith(".md") || lowerPath.endsWith(".markdown")) {
    return "markdown";
  }

  return "plain";
}

export function previewUrlForScannedFile(file: ProjectFile, options: { previewBaseUrl?: string | null }) {
  if (options.previewBaseUrl && file.previewPath) {
    return new URL(file.previewPath, `${options.previewBaseUrl}/`).toString();
  }

  return "about:blank";
}

export function currentHtmlRelativePath(activePreviewPath: string) {
  return activePreviewPath;
}

export function currentSourceRelativePath(activeScannedPath: string | null) {
  return activeScannedPath ?? "";
}

export function currentContentSection(activeScannedPath: string | null): string {
  if (!activeScannedPath) return "";

  const zolaPath = zolaRelativePath(activeScannedPath);
  if (!zolaPath.startsWith("content/") || !zolaPath.endsWith(".md")) {
    return "";
  }

  const relativeContentPath = zolaPath.slice("content/".length);
  const lastSlash = relativeContentPath.lastIndexOf("/");
  return lastSlash <= 0 ? "" : relativeContentPath.slice(0, lastSlash);
}

export function slugifyPageTitle(value: string) {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

export function projectStatusFor(project: ProjectScan) {
  const files = project.files.filter((file) => file.kind !== "DIR");
  const pages = files.filter((file) => file.role === "page").length;
  const templates = files.filter((file) => file.role === "template").length;
  const styles = files.filter((file) => file.role === "style").length;
  const scripts = files.filter((file) => file.role === "script").length;
  const assets = files.filter((file) => file.role === "asset").length;
  const previewWarning = project.previewWarning ? ` Preview indisponibil: ${project.previewWarning}` : "";

  return `${pages} pagini, ${templates} template-uri, ${styles} CSS, ${scripts} scripturi si ${assets} assets gasite.${previewWarning}`;
}
