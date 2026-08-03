import { scannedCacheKey, zolaRelativePath } from "$lib/project/files";
import { updateProjectPageFrontmatterField } from "$lib/project/io";
import { queueFileBufferDraftTextTransitionForPath } from "$lib/session/file-buffer-draft-sync";
import {
  flushWorkspaceMutationInputs,
  settleProjectWorkspaceMutation,
  type WorkspaceMutationSettlementHost,
} from "$lib/session/workspace-mutation-coordinator";
import type { GlobalStatusKind } from "$lib/status/global-status";
import { t } from "$lib/i18n/runtime.svelte";
import type {
  PageFrontmatterField,
  PageFrontmatterMutationValue,
} from "$lib/markdown/frontmatter";

export type PageSettingsControllerHost = WorkspaceMutationSettlementHost & {
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
};

export async function updatePageFrontmatterField(
  host: PageSettingsControllerHost,
  relativePath: string,
  field: PageFrontmatterField,
  value: PageFrontmatterMutationValue,
): Promise<string> {
  const zolaPath = zolaRelativePath(relativePath);
  if (!zolaPath.startsWith("content/") || !zolaPath.endsWith(".md")) {
    throw new Error("Setările paginii pot modifica doar documente Markdown din `content/`.");
  }

  await flushWorkspaceMutationInputs("manual");
  const workspace = host.projectWorkspaceSnapshot;
  if (
    !workspace
    || workspace.projectRoot !== host.sessionProjectRoot
    || workspace.runtimeSessionId !== host.kernelProjectSessionId
  ) {
    throw new Error(t("workbench-metadata-session-stale"));
  }

  const receipt = await updateProjectPageFrontmatterField({
    relativePath: zolaPath,
    field,
    value,
  }, {
    expectedProjectRoot: workspace.projectRoot,
    expectedSessionId: workspace.runtimeSessionId,
  });
  const settlement = await settleProjectWorkspaceMutation(host, receipt, {
    preferredRelativePath: zolaPath,
    warningLabel: "Actualizare setări pagină",
  });
  const cacheKey = scannedCacheKey({ relativePath: zolaPath });
  const nextSource = receipt.mutation.documents
    .find((projection) => projection.relativePath === zolaPath)
    ?.snapshot?.text
    ?? host.sourceCache[cacheKey]
    ?? (host.activeScannedPath === zolaPath ? host.source : "");
  if (!nextSource) {
    throw new Error(`Documentul ${zolaPath} nu a putut fi reproiectat după actualizare.`);
  }

  host.setGlobalStatus(
    settlement.warnings.length > 0
      ? `${t("page-settings-frontmatter-changed", { path: zolaPath })} ${settlement.warnings.join(" ")}`
      : t("page-settings-frontmatter-changed", { path: zolaPath }),
    "unsaved",
  );
  return nextSource;
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

  host.setGlobalStatus(t("page-settings-frontmatter-changed", {
    path: relativePath,
  }), "unsaved");
}
