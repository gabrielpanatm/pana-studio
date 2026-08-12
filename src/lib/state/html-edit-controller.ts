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
  type PreviewStructuralCanonicalProjectionHost,
} from "$lib/kernel/preview-projection-control";
import {
  previewStructuralCommandIdentity,
  runInPreviewStructuralLane,
} from "$lib/kernel/preview-structural-lane";
import { scannedCacheKey } from "$lib/project/files";
import { executePreviewHtmlTagIntent } from "$lib/project/io";
import { errorMessage } from "$lib/util";
import type {
  CoordinatedElementSelection,
  HtmlPendingArea,
  ProjectHtmlTagPatch,
  SourceEditLocation,
} from "$lib/types";
import type { GlobalStatusKind } from "$lib/status/global-status";
import { t } from "$lib/i18n/runtime.svelte";

export type HtmlEditControllerHost = PreviewStructuralCanonicalProjectionHost & {
  htmlMutationRevision: number;
  coordinatedElementSelection: CoordinatedElementSelection | null;
  pendingTag: string | null;
  pendingTagOriginal: string | null;
  pendingTagSourceLocation: SourceEditLocation | null;
  htmlPending: Record<HtmlPendingArea, boolean>;
  tagStatus: string;
  source: string;
  sourceCache: Record<string, string>;
  activeScannedPath: string | null;
  setHtmlPending: (area: HtmlPendingArea, pending: boolean) => void;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
};

function cacheKernelTagPatch(host: HtmlEditControllerHost, patch: ProjectHtmlTagPatch) {
  host.sourceCache = {
    ...host.sourceCache,
    [scannedCacheKey({ relativePath: patch.file })]: patch.contents,
  };
  if (host.activeScannedPath === patch.file) {
    host.source = patch.contents;
  }
}

function clearPendingTag(host: HtmlEditControllerHost) {
  host.pendingTag = null;
  host.pendingTagOriginal = null;
  host.pendingTagSourceLocation = null;
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
    const committed = await runInPreviewStructuralLane(host, async (lease) => {
      if (host.htmlMutationRevision !== revision) {
        throw new Error(
          t("html-tag-newer-change-before-commit"),
        );
      }
      const current = host.coordinatedElementSelection;
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
      await projectCommittedPreviewStructuralMutation(host, lease, receipt, patch, async () => {
        if (host.htmlMutationRevision !== revision) {
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
    host.tagStatus = t("html-tag-error-short", { message: reason });
    host.setGlobalStatus(t("html-tag-error", { message: reason }), "error");
    return failedAction(reason);
  }
}

export async function changeElementTag(
  host: HtmlEditControllerHost,
  newTag: string,
): Promise<EditorActionOutcome> {
  const selection = host.coordinatedElementSelection;
  if (!selection) {
    return blockedAction(t("html-tag-select-element"));
  }
  const sourceNodeId = selection.sourceNodeId;
  if (!sourceNodeId) {
    const message = t("html-tag-identity-missing");
    host.tagStatus = message;
    host.setGlobalStatus(message, "error");
    return blockedAction(message);
  }
  const observation = selection.observation;
  if (newTag === observation.tag) {
    clearPendingTag(host);
    return noopAction(t("html-tag-already-applied"));
  }
  const revision = ++host.htmlMutationRevision;
  const originalTag = observation.tag;
  const selector = observation.domPath;
  host.pendingTag = newTag;
  host.pendingTagOriginal = originalTag;
  host.pendingTagSourceLocation = selection.sourceLocation;
  host.tagStatus = t("html-tag-sending", { tag: newTag });
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
      host.tagStatus = t("html-tag-changed", { oldTag: originalTag, newTag });
    },
  );
}

export async function applyTagChange(host: HtmlEditControllerHost): Promise<EditorActionOutcome> {
  if (!host.pendingTag && !host.htmlPending.tag) {
    return noopAction(t("html-tag-no-pending-change"));
  }
  return blockedAction(
    t("html-tag-save-unconfirmed"),
  );
}
