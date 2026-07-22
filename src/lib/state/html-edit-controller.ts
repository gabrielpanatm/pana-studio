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
import type { HtmlPendingArea, ProjectHtmlTagPatch, SaveState, SelectionInfo, SourceEditLocation, SourceEditTarget } from "$lib/types";

export type HtmlEditControllerHost = PreviewStructuralCanonicalProjectionHost & {
  htmlMutationRevision: number;
  selectedElement: SelectionInfo | null;
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
  setGlobalStatus: (text: string, kind: SaveState) => void;
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
  project: (patch: ProjectHtmlTagPatch, selection: SelectionInfo) => Promise<void> | void,
): Promise<EditorActionOutcome> {
  try {
    const committed = await runInPreviewStructuralLane(host, async (lease) => {
      if (host.htmlMutationRevision !== revision) {
        throw new Error(
          "O modificare HTML mai nouă a înlocuit schimbarea de tag înainte de commit.",
        );
      }
      const selection = host.selectedElement;
      if (!selection || selection.tag !== oldTag || selection.domPath !== selector) {
        throw new Error("Ținta schimbării de tag nu mai corespunde selecției active.");
      }
      const liveTarget = host.resolveSourceEditTargetForSourceId(selection.sourceId);
      const liveLocation =
        (liveTarget ? sourceLocationForEditTarget(liveTarget) : null)
        ?? selection.sourceLocation
        ?? activeHtmlSourceLocationForTarget(host, {
          selector: selection.domPath,
          cssSelector: selection.cssSelector,
          tag: oldTag,
        })
        ?? targetLocation;
      const receipt = await executePreviewHtmlTagIntent({
        intent: {
          messageType: "preview-html-tag",
          selector,
          sourceId: selection.sourceId,
          sourceTag: oldTag,
          elementTag: newTag,
        },
        tagIntent: {
          targetSourceId: selection.sourceId,
          targetLocation: projectSourceLocation(liveLocation),
          targetTag: oldTag,
          targetSelector: selector,
          newTag,
        },
      }, previewStructuralCommandIdentity(lease));
      const patch = requireCommittedPreviewStructuralPatch(
        receipt,
        "HTML Tag Engine-ul a blocat schimbarea tag-ului.",
      );
      await projectCommittedPreviewStructuralMutation(host, lease, receipt, patch, async () => {
        if (host.htmlMutationRevision !== revision) {
          throw new Error(
            "O modificare HTML mai nouă a înlocuit proiecția tag-ului deja comis.",
          );
        }
        cacheKernelTagPatch(host, patch);
        clearPendingTag(host);
        await project(patch, selection);
      });
      return true;
    });
    return committed === true
      ? committedAction()
      : cancelledAction("Schimbarea de tag a fost anulată odată cu sesiunea structurală.");
  } catch (error) {
    const reason = errorMessage(error);
    host.tagStatus = `eroare: ${reason}`;
    host.setGlobalStatus(`Eroare tag: ${reason}`, "error");
    return failedAction(reason);
  }
}

export async function changeElementTag(
  host: HtmlEditControllerHost,
  newTag: string,
): Promise<EditorActionOutcome> {
  if (!host.selectedElement) {
    return blockedAction("Selectează un element înainte să schimbi tag-ul.");
  }
  if (newTag === host.selectedElement.tag) {
    clearPendingTag(host);
    return noopAction("Tag-ul selectat este deja aplicat.");
  }
  const revision = ++host.htmlMutationRevision;
  const originalTag = host.selectedElement.tag;
  const sourceEditTarget = host.resolveSourceEditTargetForSourceId(host.selectedElement.sourceId);
  const sourceLocationTarget =
    (sourceEditTarget ? sourceLocationForEditTarget(sourceEditTarget) : null) ??
    host.selectedElement.sourceLocation;
  const selector = host.selectedElement.domPath;
  const kernelTargetLocation =
    sourceLocationTarget ??
    activeHtmlSourceLocationForTarget(host, {
      selector,
      cssSelector: host.selectedElement.cssSelector,
      tag: originalTag,
    });
  if (!kernelTargetLocation) {
    const message = host.isActivePreviewHtmlSource
      ? "Nu pot executa schimbarea de tag: ținta nu are locație sursă unică pentru kernel."
      : "Elementul nu are sursă HTML editabilă pentru schimbarea de tag.";
    host.tagStatus = message;
    host.setGlobalStatus(message, "error");
    return blockedAction(message);
  }
  host.pendingTag = newTag;
  host.pendingTagOriginal = originalTag;
  host.pendingTagSourceLocation = kernelTargetLocation;
  host.tagStatus = `Tag ales: <${newTag}>. Se trimite la kernel...`;
  host.setHtmlPending("tag", true);
  host.setGlobalStatus(
    `<${originalTag}> → <${newTag}> se execută prin kernel`,
    "saving",
  );

  return await executePendingKernelTagChange(
    host,
    originalTag,
    newTag,
    kernelTargetLocation,
    selector,
    revision,
    (_patch, _selection) => {
      host.tagStatus = `Tag modificat prin kernel: <${originalTag}> → <${newTag}>`;
    },
  );
}

export async function applyTagChange(host: HtmlEditControllerHost): Promise<EditorActionOutcome> {
  if (!host.pendingTag && !host.htmlPending.tag) {
    return noopAction("Nu există o schimbare de tag pending.");
  }
  return blockedAction(
    "Schimbarea etichetei este încă în așteptare sau a eșuat; salvarea nu o poate declara aplicată fără un commit confirmat de nucleul Rust.",
  );
}
