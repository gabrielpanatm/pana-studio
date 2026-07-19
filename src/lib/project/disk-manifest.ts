import type { ProjectDiskManifest } from "$lib/types";

export type ManifestDiff = {
  changedFiles: string[];
  previewRelevantChanged: boolean;
};

export function diffDiskManifests(
  previous: ProjectDiskManifest | null,
  current: ProjectDiskManifest,
): ManifestDiff {
  if (!previous || previous.root !== current.root) {
    return { changedFiles: [], previewRelevantChanged: false };
  }

  const previousFiles = manifestMap(previous);
  const currentFiles = manifestMap(current);
  const changed = new Set<string>();

  for (const [path, entry] of currentFiles) {
    const old = previousFiles.get(path);
    if (
      !old ||
      old.modifiedMs !== entry.modifiedMs ||
      old.size !== entry.size ||
      (old.versionToken ?? "") !== (entry.versionToken ?? "")
    ) {
      changed.add(path);
    }
  }

  for (const path of previousFiles.keys()) {
    if (!currentFiles.has(path)) changed.add(path);
  }

  const changedFiles = [...changed].sort();
  return {
    changedFiles,
    previewRelevantChanged: changedFiles.some(isPreviewRelevantPath),
  };
}

export function isPreviewRelevantPath(path: string) {
  if (path === "sursa/zola.toml" || path === "sursa/config.toml") return true;
  return path.startsWith("sursa/content/")
    || path.startsWith("sursa/templates/")
    || path.startsWith("sursa/themes/")
    || path.startsWith("sursa/sass/")
    || path.startsWith("sursa/static/")
    || path.endsWith(".html")
    || path.endsWith(".css")
    || path.endsWith(".scss")
    || path.endsWith(".js")
    || path.endsWith(".md");
}

function manifestMap(manifest: ProjectDiskManifest) {
  return new Map(manifest.files.map((entry) => [entry.relativePath, entry]));
}
