import {
  blockedAction,
  cancelledAction,
  committedAction,
  failedAction,
  noopAction,
  type EditorActionOutcome,
} from "$lib/editor-runtime/action-outcome";
import {
  projectCommittedPreviewStructuralMutation,
  requireCommittedPreviewStructuralPatch,
} from "$lib/kernel/preview-projection-control";
import {
  previewStructuralCommandIdentity,
  type PreviewStructuralSessionLease,
} from "$lib/kernel/preview-structural-lane";
import { scannedCacheKey } from "$lib/project/files";
import {
  executePreviewHtmlTagIntent,
} from "$lib/preview/structural-io";
import { errorMessage } from "$lib/util";
import type {
  CoordinatedElementSelection,
  HtmlPendingArea,
} from "$lib/canvas/contracts";
import type { ProjectHtmlTagPatch } from "$lib/preview/contracts";
import type { SourceEditLocation } from "$lib/source-graph/contracts";
import type { GlobalStatusKind } from "$lib/status/global-status";
import { t } from "$lib/i18n/runtime.svelte";

export type HtmlEditControllerHost = {
  context: () => Readonly<{
    coordinatedSelection: CoordinatedElementSelection | null;
    activeScannedPath: string | null;
  }>;
  html: {
    mutationRevision: number;
    pendingTag: string | null;
    pendingTagOriginal: string | null;
    pendingTagSourceLocation: SourceEditLocation | null;
    htmlPending: Record<HtmlPendingArea, boolean>;
    tagStatus: string;
  };
  source: { source: string; sourceCache: Record<string, string> };
  runStructural: <T>(
    operation: (lease: PreviewStructuralSessionLease) => Promise<T>,
  ) => Promise<T | null>;
  projectCommitted: (
    lease: PreviewStructuralSessionLease,
    receipt: Parameters<typeof projectCommittedPreviewStructuralMutation>[2],
    patch: Parameters<typeof projectCommittedPreviewStructuralMutation>[3],
    projectLocalState: Parameters<typeof projectCommittedPreviewStructuralMutation>[4],
  ) => ReturnType<typeof projectCommittedPreviewStructuralMutation>;
  setHtmlPending: (area: HtmlPendingArea, pending: boolean) => void;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
};

function cacheKernelTagPatch(host: HtmlEditControllerHost, patch: ProjectHtmlTagPatch) {
  host.source.sourceCache = {
    ...host.source.sourceCache,
    [scannedCacheKey({ relativePath: patch.file })]: patch.contents,
  };
  if (host.context().activeScannedPath === patch.file) {
    host.source.source = patch.contents;
  }
}

function clearPendingTag(host: HtmlEditControllerHost) {
  host.html.pendingTag = null;
  host.html.pendingTagOriginal = null;
  host.html.pendingTagSourceLocation = null;
  host.setHtmlPending("tag", false);
}

async function executePendingKernelTagChange(
  host: HtmlEditControllerHost,
  oldTag: string,
  newTag: string,
  sourceNodeId: string,
  selector: string | null,
  revision: number,
  capturedSelection: CoordinatedElementSelection,
  project: (
    patch: ProjectHtmlTagPatch,
    selection: CoordinatedElementSelection,
  ) => Promise<void> | void,
): Promise<EditorActionOutcome> {
  try {
    const committed = await host.runStructural(async (lease) => {
      if (host.html.mutationRevision !== revision) {
        throw new Error(
          t("html-tag-newer-change-before-commit"),
        );
      }
      const current = host.context().coordinatedSelection;
      const observation = capturedSelection.observation;
      if (
        !current
        || current.snapshot.selectionRevision !== capturedSelection.snapshot.selectionRevision
        || current.renderInstanceId !== capturedSelection.renderInstanceId
        || observation.tag !== oldTag
        || observation.domPath !== selector
      ) {
        throw new Error(t("html-tag-target-selection-mismatch"));
      }
      const receipt = await executePreviewHtmlTagIntent({
        intent: {
          messageType: "preview-html-tag",
          sourceId: sourceNodeId,
          sourceTag: oldTag,
          elementTag: newTag,
        },
        tagIntent: {
          targetSourceId: sourceNodeId,
          targetTag: oldTag,
          newTag,
        },
      }, previewStructuralCommandIdentity(lease, true));
      const patch = requireCommittedPreviewStructuralPatch(
        receipt,
        t("html-tag-engine-blocked"),
      );
      await host.projectCommitted(lease, receipt, patch, async () => {
        if (host.html.mutationRevision !== revision) {
          throw new Error(
            t("html-tag-newer-change-after-commit"),
          );
        }
        cacheKernelTagPatch(host, patch);
        clearPendingTag(host);
        await project(patch, capturedSelection);
      });
      return true;
    });
    return committed === true
      ? committedAction()
      : cancelledAction(t("html-tag-session-cancelled"));
  } catch (error) {
    const reason = errorMessage(error);
    host.html.tagStatus = t("html-tag-error-short", { message: reason });
    host.setGlobalStatus(t("html-tag-error", { message: reason }), "error");
    return failedAction(reason);
  }
}

export async function changeElementTag(
  host: HtmlEditControllerHost,
  newTag: string,
): Promise<EditorActionOutcome> {
  const selection = host.context().coordinatedSelection;
  if (!selection) {
    return blockedAction(t("html-tag-select-element"));
  }
  const sourceNodeId = selection.sourceNodeId;
  if (!sourceNodeId) {
    const message = t("html-tag-identity-missing");
    host.html.tagStatus = message;
    host.setGlobalStatus(message, "error");
    return blockedAction(message);
  }
  const observation = selection.observation;
  if (newTag === observation.tag) {
    clearPendingTag(host);
    return noopAction(t("html-tag-already-applied"));
  }
  const revision = ++host.html.mutationRevision;
  const originalTag = observation.tag;
  const selector = observation.domPath;
  host.html.pendingTag = newTag;
  host.html.pendingTagOriginal = originalTag;
  host.html.pendingTagSourceLocation = selection.sourceLocation;
  host.html.tagStatus = t("html-tag-sending", { tag: newTag });
  host.setHtmlPending("tag", true);
  host.setGlobalStatus(
    t("html-tag-executing", { oldTag: originalTag, newTag }),
    "saving",
  );

  return await executePendingKernelTagChange(
    host,
    originalTag,
    newTag,
    sourceNodeId,
    selector,
    revision,
    selection,
    (_patch, _selection) => {
      host.html.tagStatus = t("html-tag-changed", { oldTag: originalTag, newTag });
    },
  );
}

export async function applyTagChange(host: HtmlEditControllerHost): Promise<EditorActionOutcome> {
  if (!host.html.pendingTag && !host.html.htmlPending.tag) {
    return noopAction(t("html-tag-no-pending-change"));
  }
  return blockedAction(
    t("html-tag-save-unconfirmed"),
  );
}
