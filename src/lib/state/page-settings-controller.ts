import { scannedCacheKey, zolaRelativePath } from "$lib/project/files";
import { queueFileBufferDraftTextTransitionForPath } from "$lib/session/file-buffer-draft-sync";
import type { SaveState } from "$lib/types";

export type PageSettingsControllerHost = {
  activeScannedPath: string | null;
  source: string;
  sourceCache: Record<string, string>;
  setGlobalStatus: (text: string, kind: SaveState) => void;
};

export function pageSettingsSource(host: PageSettingsControllerHost) {
  const relativePath = host.activeScannedPath;
  if (!relativePath) return "";
  const zolaPath = zolaRelativePath(relativePath);
  if (!zolaPath.startsWith("content/") || !zolaPath.endsWith(".md")) return "";
  const cacheKey = scannedCacheKey({ relativePath });
  return host.sourceCache[cacheKey] ?? host.source;
}

export function updatePageFrontmatterSource(
  host: PageSettingsControllerHost,
  relativePath: string,
  nextSource: string,
) {
  const zolaPath = zolaRelativePath(relativePath);
  if (!zolaPath.startsWith("content/") || !zolaPath.endsWith(".md")) return;
  const cacheKey = scannedCacheKey({ relativePath });
  const currentSource = host.sourceCache[cacheKey] ?? (host.activeScannedPath === relativePath ? host.source : "");
  if (currentSource === nextSource) return;

  queueFileBufferDraftTextTransitionForPath(relativePath, currentSource, nextSource, "page_settings.frontmatter");
  host.sourceCache = { ...host.sourceCache, [cacheKey]: nextSource };

  if (host.activeScannedPath === relativePath) {
    host.source = nextSource;
  }

  host.setGlobalStatus(`Frontmatter modificat în ${relativePath} — Ctrl+S pentru salvare`, "unsaved");
}
