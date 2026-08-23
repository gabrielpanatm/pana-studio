import { scannedCacheKey, zolaRelativePath } from "$lib/project/files";
import {
  updateProjectPageFrontmatterField,
} from "$lib/content/io";
import { queueFileBufferDraftTextTransitionForPath } from "$lib/session/file-buffer-draft-sync";
import {
  flushWorkspaceMutationInputs,
  type WorkspaceMutationAuthorityReceipt,
  type WorkspaceMutationSettlement,
  type WorkspaceMutationSettlementOptions,
} from "$lib/session/workspace-mutation-coordinator";
import type { GlobalStatusKind } from "$lib/status/global-status";
import { t } from "$lib/i18n/runtime.svelte";
import type {
  PageFrontmatterField,
  PageFrontmatterMutationValue,
} from "$lib/markdown/frontmatter";
import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";

export type PageSettingsControllerHost = {
  context: () => Readonly<{
    projectRoot: string;
    runtimeSessionId: string;
    workspace: ProjectWorkspaceSnapshot | null;
    activeScannedPath: string | null;
  }>;
  source: {
    source: string;
    sourceCache: Record<string, string>;
  };
  settleMutation: (
    receipt: WorkspaceMutationAuthorityReceipt,
    options?: WorkspaceMutationSettlementOptions,
  ) => Promise<WorkspaceMutationSettlement>;
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
  const context = host.context();
  const workspace = context.workspace;
  if (
    !workspace
    || workspace.projectRoot !== context.projectRoot
    || workspace.runtimeSessionId !== context.runtimeSessionId
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
  const settlement = await host.settleMutation(receipt, {
    preferredRelativePath: zolaPath,
    warningLabel: "Actualizare setări pagină",
  });
  const cacheKey = scannedCacheKey({ relativePath: zolaPath });
  const nextSource = receipt.mutation.documents
    .find((projection) => projection.relativePath === zolaPath)
    ?.snapshot?.text
    ?? host.source.sourceCache[cacheKey]
    ?? (context.activeScannedPath === zolaPath ? host.source.source : "");
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
  const context = host.context();
  const cacheKey = scannedCacheKey({ relativePath });
  const currentSource = host.source.sourceCache[cacheKey]
    ?? (context.activeScannedPath === relativePath ? host.source.source : "");
  if (currentSource === nextSource) return;

  queueFileBufferDraftTextTransitionForPath(relativePath, currentSource, nextSource, "page_settings.frontmatter");
  host.source.sourceCache = { ...host.source.sourceCache, [cacheKey]: nextSource };

  if (context.activeScannedPath === relativePath) {
    host.source.source = nextSource;
  }

  host.setGlobalStatus(t("page-settings-frontmatter-changed", {
    path: relativePath,
  }), "unsaved");
}
