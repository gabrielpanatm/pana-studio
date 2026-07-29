import {
  blockedAction,
  cancelledAction,
  committedAction,
  failedAction,
  noopAction,
  type EditorActionOutcome,
} from "$lib/editor-runtime/action-outcome";
import { htmlVoidTags } from "$lib/html/mutations";
import { parseHtmlSourceNodes } from "$lib/html/parser";
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
import { sourceLocationForEditTarget } from "$lib/source-graph/location";
import { errorMessage } from "$lib/util";
import type {
  CoordinatedElementSelection,
  HtmlPendingArea,
  ProjectHtmlTagPatch,
  SourceEditLocation,
  SourceEditTarget,
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
  currentHtmlRelativePath: string;
  isActivePreviewHtmlSource: boolean;
  setHtmlPending: (area: HtmlPendingArea, pending: boolean) => void;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
  resolveSourceEditTargetForSourceId: (sourceId: string | null | undefined) => SourceEditTarget | null;
};

type HtmlTagTarget = {
  selector: string;
  cssSelector?: string | null;
  tag: string;
};

function projectSourceLocation(tpl: SourceEditLocation) {
  return {
    file: tpl.file,
    line: tpl.line,
    column: tpl.column ?? 0,
  };
}

function sourceLocationAtOffset(file: string, sourceText: string, offset: number): SourceEditLocation {
  const before = sourceText.slice(0, Math.max(0, offset));
  const lines = before.split("\n");
  const linePrefix = lines[lines.length - 1] ?? "";
  return {
    file,
    line: lines.length,
    column: Array.from(linePrefix).length + 1,
  };
}

function selectorVariants(selector: string) {
  const trimmed = selector.trim();
  const variants = new Set<string>();
  if (!trimmed) return variants;
  variants.add(trimmed);

  const htmlPrefix = "html:nth-of-type(1) > ";
  if (trimmed.startsWith(htmlPrefix)) {
    variants.add(trimmed.slice(htmlPrefix.length));
  } else {
    variants.add(`${htmlPrefix}${trimmed}`);
  }

  return variants;
}

function currentActiveHtmlSource(host: HtmlEditControllerHost) {
  const cacheKey = scannedCacheKey({ relativePath: host.currentHtmlRelativePath });
  if (host.activeScannedPath === host.currentHtmlRelativePath) {
    return host.source || host.sourceCache[cacheKey] || "";
  }
  return host.sourceCache[cacheKey] || "";
}

function uniqueSourceNode<T>(items: T[]) {
  return items.length === 1 ? items[0] : null;
}

function activeHtmlSourceLocationForTarget(
  host: HtmlEditControllerHost,
  target: HtmlTagTarget,
): SourceEditLocation | null {
  if (!host.isActivePreviewHtmlSource || !host.currentHtmlRelativePath) return null;

  const sourceText = currentActiveHtmlSource(host);
  if (!sourceText) return null;

  const variants = selectorVariants(target.selector);
  const nodes = parseHtmlSourceNodes(sourceText, htmlVoidTags);
  const selectorMatches = nodes.filter((node) =>
    node.tag === target.tag && variants.has(node.selector),
  );
  const selected =
    selectorMatches.length === 1
      ? selectorMatches[0]
      : target.cssSelector
        ? uniqueSourceNode(nodes.filter((node) =>
            node.tag === target.tag && node.cssSelector === target.cssSelector,
          ))
        : null;

  return selected
    ? sourceLocationAtOffset(host.currentHtmlRelativePath, sourceText, selected.openStart)
    : null;
}

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
  targetLocation: SourceEditLocation,
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
      const liveTarget = host.resolveSourceEditTargetForSourceId(capturedSelection.sourceNodeId);
      const liveLocation =
        (liveTarget ? sourceLocationForEditTarget(liveTarget) : null)
        ?? capturedSelection.sourceLocation
        ?? activeHtmlSourceLocationForTarget(host, {
          selector: observation.domPath,
          cssSelector: observation.cssSelector,
          tag: oldTag,
        })
        ?? targetLocation;
      const receipt = await executePreviewHtmlTagIntent({
        intent: {
          messageType: "preview-html-tag",
          selector,
          sourceId: capturedSelection.sourceNodeId,
          sourceTag: oldTag,
          elementTag: newTag,
        },
        tagIntent: {
          targetSourceId: capturedSelection.sourceNodeId,
          targetLocation: projectSourceLocation(liveLocation),
          targetTag: oldTag,
          targetSelector: selector,
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
  const observation = selection.observation;
  if (newTag === observation.tag) {
    clearPendingTag(host);
    return noopAction(t("html-tag-already-applied"));
  }
  const revision = ++host.htmlMutationRevision;
  const originalTag = observation.tag;
  const sourceEditTarget = host.resolveSourceEditTargetForSourceId(selection.sourceNodeId);
  const sourceLocationTarget =
    (sourceEditTarget ? sourceLocationForEditTarget(sourceEditTarget) : null) ??
    selection.sourceLocation;
  const selector = observation.domPath;
  const kernelTargetLocation =
    sourceLocationTarget ??
    activeHtmlSourceLocationForTarget(host, {
      selector,
      cssSelector: observation.cssSelector,
      tag: originalTag,
    });
  if (!kernelTargetLocation) {
    const message = host.isActivePreviewHtmlSource
      ? t("html-tag-location-missing")
      : t("html-tag-source-not-editable");
    host.tagStatus = message;
    host.setGlobalStatus(message, "error");
    return blockedAction(message);
  }
  host.pendingTag = newTag;
  host.pendingTagOriginal = originalTag;
  host.pendingTagSourceLocation = kernelTargetLocation;
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
    kernelTargetLocation,
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
