import { SOURCE_LOADING_SENTINEL } from "$lib/editor-runtime/source-state";
import { t } from "$lib/i18n/runtime.svelte";
import { bindCanvasCandidateIdentityToPreviewUrl } from "$lib/project/preview-url";
import {
  createProjectContentPage,
} from "$lib/content/io";
import {
  readFileBufferText,
} from "$lib/project/io/workspace";
import type {
  CanvasProjectionPlan,
} from "$lib/contracts/canvas-projection";
import {
  planContentPageCreation,
  planScannedProjectFileLoad,
} from "$lib/project/session";
import { reanchorFileBufferDraftSyncCursor } from "$lib/session/file-buffer-draft-sync";
import {
  type WorkspaceMutationAuthorityReceipt,
  type WorkspaceMutationSettlement,
  type WorkspaceMutationSettlementOptions,
} from "$lib/session/workspace-mutation-coordinator";
import type { GlobalStatusKind } from "$lib/status/global-status";
import type { CenterView } from "$lib/application/contracts";
import type { PreviewStructuralCommandIdentity } from "$lib/preview/contracts";
import type {
  ProjectFile,
  ProjectScan,
} from "$lib/project/lifecycle-contract";
import type { TemplateWorkbenchPlan } from "$lib/project/template-workbench-contract";
import { errorMessage } from "$lib/util";

export type ProjectDocumentStructuralContext = {
  identity: PreviewStructuralCommandIdentity;
  isCurrent: () => boolean;
  requireCurrent: () => void;
};

export type ProjectDocumentHost = {
    source: string;
    sourceCache: Record<string, string>;
    activeScannedPath: string | null;
    activePreviewPath: string;
    browserPreviewRoute: string;
    previewSrc: string;
    previewDocumentMarkup: string | null;
    pendingCanvasProjection: CanvasProjectionPlan | null;
    centerView: CenterView;
    templateWorkbenchPlan: TemplateWorkbenchPlan | null;
    templateWorkbenchPreferredPagePath: string | null;
    templateWorkbenchPreferredRoute: string | null;
    templateWorkbenchActive: boolean;
    templateWorkbenchTarget: string | null;
    projectStatus: string;
    scannedProject: ProjectScan | null;
    kernelProjectSessionId: string;
    projectSessionEpoch: number;
    runProjectDocumentStructuralLane: <T>(
      operation: (context: ProjectDocumentStructuralContext) => Promise<T>,
    ) => Promise<T>;
    settleProjectDocumentMutation: (
      receipt: WorkspaceMutationAuthorityReceipt,
      options?: WorkspaceMutationSettlementOptions,
    ) => Promise<WorkspaceMutationSettlement>;
    flushInteractiveEditorDrafts: () => Promise<void>;
    previewUrlForScannedFile: (file: ProjectFile) => string;
    refreshRenderedPreviewDocument: () => Promise<boolean>;
    cancelPreviewSync: () => void;
    exitTemplateWorkbench: (options?: {
      deferPreviewRefresh?: boolean;
      returnPath?: string | null;
    }) => Promise<void>;
    updateTemplateWorkbenchContext: (
      project: ProjectScan,
      templateFile: ProjectFile,
      preferredPagePath?: string | null,
      options?: {
        deferPreviewRefresh?: boolean;
        preferredRoute?: string | null;
        preferredComponentName?: string | null;
        strict?: boolean;
      },
    ) => Promise<ProjectFile | null>;
    setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
};

export async function createContentPageFromInput(
  host: ProjectDocumentHost,
  input: { title: string; slug?: string | null; section?: string | null },
): Promise<string | null> {
  if (!host.scannedProject) {
    host.projectStatus = t("project-controller-page-project-required");
    return null;
  }
  const pagePlan = planContentPageCreation(input.title, host.activeScannedPath, {
    slug: input.slug,
    section: input.section,
  });
  if (!pagePlan.ok) {
    host.projectStatus = pagePlan.status;
    return null;
  }
  return await host.runProjectDocumentStructuralLane(async (context): Promise<string | null> => {
    host.projectStatus = pagePlan.creatingStatus;
    try {
      let receipt: Awaited<ReturnType<typeof createProjectContentPage>>;
      try {
        receipt = await createProjectContentPage({
          section: pagePlan.section,
          slug: pagePlan.slug,
          title: pagePlan.title,
        }, context.identity);
      } catch (error) {
        if (!context.isCurrent()) return null;
        host.projectStatus = t("project-controller-page-create-failed", {
          message: errorMessage(error),
        });
        return null;
      }
      context.requireCurrent();
      const relativePath = receipt.relativePath;
      if (!relativePath) {
        throw new Error(t("project-controller-page-path-missing"));
      }

      const settlement = await host.settleProjectDocumentMutation(receipt, {
        preferredRelativePath: relativePath,
        warningLabel: t("project-controller-page-create-operation"),
      });
      context.requireCurrent();
      host.setGlobalStatus(
        t("project-controller-page-created-save", { path: relativePath }),
        "unsaved",
      );
      host.projectStatus = settlement.warnings.length > 0
        ? t("project-controller-page-created-resync")
        : t("project-controller-page-created", { path: relativePath });
      return relativePath;
    } catch (error) {
      if (!context.isCurrent()) return null;
      host.projectStatus = t("project-controller-page-create-failed", {
        message: errorMessage(error),
      });
      return null;
    }
  });
}

export async function loadScannedProjectFile(
  host: ProjectDocumentHost,
  file: ProjectFile,
  options: {
    strict?: boolean;
    skipDraftFlush?: boolean;
    deferPreviewRefresh?: boolean;
    activateTemplateWorkbench?: boolean;
    preferredTemplatePagePath?: string | null;
    preferredTemplateRoute?: string | null;
    preferredComponentName?: string | null;
    syncWorkbench?: boolean;
  } = {},
) {
  if (!host.scannedProject) return;
  const expectedRoot = host.scannedProject.root;
  const expectedSessionId = host.kernelProjectSessionId;
  const expectedSessionEpoch = host.projectSessionEpoch;
  if (!options.skipDraftFlush) await host.flushInteractiveEditorDrafts();
  if (!projectLoadLeaseMatches(host, expectedRoot, expectedSessionId, expectedSessionEpoch)) return;
  const loadPlan = planScannedProjectFileLoad(file);
  const reusesActiveTemplateContext = loadPlan.isTemplateFile
    && host.templateWorkbenchActive
    && normalizedTemplateContextPath(host.templateWorkbenchTarget)
      === normalizedTemplateContextPath(file.relativePath);
  host.activeScannedPath = file.relativePath;
  host.source = SOURCE_LOADING_SENTINEL;
  host.centerView = loadPlan.centerView;

  const cachedSource = host.sourceCache[loadPlan.cacheKey];
  if (typeof cachedSource === "string") {
    host.source = cachedSource;
  } else {
    try {
      const snapshot = await readFileBufferText(file.relativePath, {
        expectedProjectRoot: expectedRoot,
        expectedSessionId,
      });
      const text = snapshot.text;
      if (
        host.activeScannedPath !== file.relativePath
        || !projectLoadLeaseMatches(host, expectedRoot, expectedSessionId, expectedSessionEpoch)
      ) return;
      reanchorFileBufferDraftSyncCursor(file.relativePath, {
        revision: snapshot.revision,
        hash: snapshot.hash,
      });
      host.sourceCache = { ...host.sourceCache, [loadPlan.cacheKey]: text };
      host.source = text;
    } catch (error) {
      if (
        host.activeScannedPath !== file.relativePath
        || !projectLoadLeaseMatches(host, expectedRoot, expectedSessionId, expectedSessionEpoch)
      ) return;
      if (options.strict) throw error;
      host.source = t("project-controller-file-load-failed", {
        path: file.relativePath,
        message: errorMessage(error),
      });
    }
  }

  if (loadPlan.isPreviewPage) {
    if (host.templateWorkbenchActive) {
      await host.exitTemplateWorkbench({
        deferPreviewRefresh: true,
        returnPath: file.relativePath,
      });
    }
    host.templateWorkbenchPlan = null;
    host.templateWorkbenchPreferredPagePath = null;
    host.templateWorkbenchPreferredRoute = null;
    const previewUrl = host.previewUrlForScannedFile(file);
    host.previewSrc = host.pendingCanvasProjection
      ? bindCanvasCandidateIdentityToPreviewUrl(
          previewUrl,
          host.pendingCanvasProjection.identity,
        )
      : previewUrl;
    host.activePreviewPath = file.relativePath;
    host.browserPreviewRoute = file.previewPath ?? "/";
    host.previewDocumentMarkup = null;
    host.cancelPreviewSync();
  }

  if (loadPlan.isTemplateFile && options.activateTemplateWorkbench !== false) {
    await host.updateTemplateWorkbenchContext(
      host.scannedProject,
      file,
      options.preferredTemplatePagePath !== undefined
        ? options.preferredTemplatePagePath
        : reusesActiveTemplateContext
          ? host.templateWorkbenchPreferredPagePath
          : null,
      {
        deferPreviewRefresh: options.deferPreviewRefresh,
        preferredRoute: options.preferredTemplateRoute !== undefined
          ? options.preferredTemplateRoute
          : reusesActiveTemplateContext
            ? host.templateWorkbenchPreferredRoute
            : null,
        preferredComponentName: options.preferredComponentName !== undefined
          ? options.preferredComponentName
          : reusesActiveTemplateContext
            ? host.templateWorkbenchPlan?.activeComponentName ?? null
            : null,
        strict: options.strict,
      },
    );
  }

  if (loadPlan.isPreviewPage && !options.deferPreviewRefresh) {
    await host.refreshRenderedPreviewDocument();
  }
}

function projectLoadLeaseMatches(
  host: ProjectDocumentHost,
  expectedRoot: string,
  expectedSessionId: string,
  expectedSessionEpoch: number,
) {
  return host.scannedProject?.root === expectedRoot
    && host.kernelProjectSessionId === expectedSessionId
    && host.projectSessionEpoch === expectedSessionEpoch;
}

function normalizedTemplateContextPath(path: string | null | undefined) {
  return path?.trim().replaceAll("\\", "/").replace(/^\.\/+/, "") ?? "";
}
