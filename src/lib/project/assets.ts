import { zolaRelativePath } from "$lib/project/files";
import type { ProjectFile } from "$lib/types";

export type ProjectAssetOrigin = {
  kind: "local" | "theme" | "project";
  themeName: string | null;
};

export function projectAssetOrigin(asset: ProjectFile): ProjectAssetOrigin {
  const path = normalizedZolaAssetPath(asset);
  const themeStatic = path.match(/^themes\/([^/]+)\/static\/(.+)$/);
  if (themeStatic) return { kind: "theme", themeName: themeStatic[1] };
  if (path.startsWith("static/")) return { kind: "local", themeName: null };
  return { kind: "project", themeName: null };
}

export function projectAssetOriginLabel(asset: ProjectFile) {
  const origin = projectAssetOrigin(asset);
  if (origin.kind === "theme") return origin.themeName ? `theme ${origin.themeName}` : "theme";
  if (origin.kind === "local") return "local";
  return "proiect";
}

export function projectAssetPublicUrl(asset: ProjectFile): string {
  const path = normalizedZolaAssetPath(asset);
  const themeStatic = path.match(/^themes\/[^/]+\/static\/(.+)$/);
  const publicPath = themeStatic?.[1] ?? path.replace(/^static\//, "");
  return `/${publicPath.replace(/^\/+/, "")}`;
}

/**
 * Resolves a public project resource against one immutable Rust Preview
 * generation. Encoding each segment keeps filesystem names containing spaces,
 * diacritics, query markers or fragments inside the URL path.
 */
export function projectPreviewResourceUrl(
  previewBaseUrl: string | null | undefined,
  publicPath: string,
  previewRevision: string | null | undefined,
): string {
  const normalizedPath = publicPath.replaceAll("\\", "/").replace(/^\/+/, "");
  if (!previewBaseUrl?.trim() || !normalizedPath) return "";
  try {
    const encodedPath = normalizedPath.split("/").map(encodeURIComponent).join("/");
    const url = new URL(`/${encodedPath}`, previewBaseUrl);
    const revision = previewRevision?.trim();
    if (revision) url.searchParams.set("__pana_preview_revision", revision);
    return url.href;
  } catch {
    return "";
  }
}

function normalizedZolaAssetPath(asset: ProjectFile) {
  return zolaRelativePath(asset.relativePath).replaceAll("\\", "/").replace(/^\/+/, "");
}
