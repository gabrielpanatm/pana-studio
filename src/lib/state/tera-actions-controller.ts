import {
  projectCommittedPreviewStructuralMutation,
  previewStructuralBlockingDiagnostic,
  requireCommittedPreviewStructuralPatch,
  type PreviewStructuralCanonicalProjectionHost,
  type PreviewStructuralExecutionReceipt,
} from "$lib/kernel/preview-projection-control";
import {
  blockedAction,
  cancelledAction,
  committedAction,
  failedAction,
  type EditorActionOutcome,
} from "$lib/editor-runtime/action-outcome";
import {
  previewStructuralCommandIdentity,
  runInPreviewStructuralLane,
  type PreviewStructuralSessionLease,
} from "$lib/kernel/preview-structural-lane";
import { projectRelativeZolaPath, scannedCacheKey } from "$lib/project/files";
import {
  executePreviewTeraDeleteIntent,
  executePreviewTeraInsertDropIntent,
} from "$lib/project/io";
import { resolveTeraDropTarget } from "$lib/tera/drop-targets";
import { deleteTeraNodeCapability } from "$lib/tera/mutations";
import type { TeraDropRequest } from "$lib/tera/model";
import type { SourceGraph, SourceGraphNode } from "$lib/types";
import type { GlobalStatusKind } from "$lib/status/global-status";
import { errorMessage } from "$lib/util";
import { t } from "$lib/i18n/runtime.svelte";

export type TeraActionsControllerHost = PreviewStructuralCanonicalProjectionHost & {
  sourceGraph: SourceGraph | null;
  selectedTemplateSourceNode: SourceGraphNode | null;
  activeScannedPath: string | null;
  activeRenderedTemplatePath: string | null;
  source: string;
  sourceCache: Record<string, string>;
  clearPreviewSelection: (options?: { clearCanvasOverlay?: boolean }) => void;
  refreshSourceGraph: (options?: { strict?: boolean }) => Promise<void>;
  selectDynamicWidgetSourceInstance?: (instanceId: string) => Promise<boolean>;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
};

export function dynamicWidgetInstanceIdFromSnippet(snippet: string): string | null {
  if (!/\bpana:widget\b/.test(snippet)) return null;
  return snippet.match(/\binstance=([A-Za-z0-9_-]+)/)?.[1] ?? null;
}

export function captureTeraActionTarget(
  node: SourceGraphNode | null | undefined,
): SourceGraphNode | null {
  if (!node) return null;
  return Object.freeze({
    ...node,
    range: node.range ? Object.freeze({ ...node.range }) : null,
    children: Object.freeze([...node.children]) as unknown as string[],
    capabilities: Object.freeze({ ...node.capabilities }),
  });
}

function dropPositionLabel(position: TeraDropRequest["position"]) {
  if (position === "before") return t("tera-actions-position-before");
  if (position === "after") return t("tera-actions-position-after");
  return t("tera-actions-position-inside");
}

function blockedTeraReceiptOutcome(
  receipt: PreviewStructuralExecutionReceipt,
  fallback: string,
): EditorActionOutcome | null {
  if (receipt.status === "committed") return null;
  const diagnostic = previewStructuralBlockingDiagnostic(receipt);
  return blockedAction(
    (diagnostic ? errorMessage(diagnostic.diagnostic) : "")
      || errorMessage(receipt.messageDiagnostic)
      || fallback,
  );
}

function projectCommittedTeraSource(
  host: TeraActionsControllerHost,
  patch: { file: string; contents: string },
) {
  host.sourceCache = {
    ...host.sourceCache,
    [scannedCacheKey({ relativePath: patch.file })]: patch.contents,
  };
  if (host.activeScannedPath === patch.file) {
    host.source = patch.contents;
  }
}

export async function insertTeraPaletteItemAtTarget(
  host: TeraActionsControllerHost,
  request: TeraDropRequest,
): Promise<EditorActionOutcome> {
  try {
    const result = await runInPreviewStructuralLane(host, (lease) =>
      insertTeraPaletteItemAtTargetInLane(host, request, lease));
    return result ?? cancelledAction(t("tera-actions-insert-session-cancelled"));
  } catch (error) {
    const reason = errorMessage(error);
    host.setGlobalStatus(t("tera-actions-insert-error", { message: reason }), "error");
    return failedAction(reason);
  }
}

async function insertTeraPaletteItemAtTargetInLane(
  host: TeraActionsControllerHost,
  request: TeraDropRequest,
  lease: PreviewStructuralSessionLease,
): Promise<EditorActionOutcome> {
  const resolution = resolveTeraDropTarget(host.sourceGraph, request);
  if (!resolution.allowed) {
    host.setGlobalStatus(resolution.reason, "error");
    return blockedAction(resolution.reason);
  }

  const anchor = resolution.anchor;
  const range = anchor.range;
  if (!range) {
    const message = t("tera-actions-insert-anchor-missing");
    host.setGlobalStatus(message, "error");
    return blockedAction(message);
  }

  try {
    const receipt = await executePreviewTeraInsertDropIntent({
      intent: {
        messageType: "preview-tera-drop",
        targetSelector: request.targetSelector,
        targetSourceId: request.targetSourceId,
        targetTemplateSourceId: request.targetTemplateSourceId,
        targetSessionId: request.targetSessionId,
        targetTag: request.targetTag,
        targetKind: anchor.kind,
        position: request.position,
        itemKind: request.item.kind,
      },
      insertIntent: {
        targetSourceId: anchor.id,
        targetLocation: {
          file: anchor.file,
          line: range.line,
          column: range.column,
        },
        targetKind: anchor.kind,
        targetTag: request.targetTag,
        targetSelector: request.targetSelector,
        position: request.position,
        item: {
          kind: request.item.kind,
          label: request.item.label,
          target: request.item.target ?? null,
          name: request.item.name ?? null,
          expression: request.item.expression ?? null,
          dynamicBinding: request.item.dynamicBinding ?? null,
          dynamicWidget: request.item.dynamicWidget ?? null,
        },
      },
    }, previewStructuralCommandIdentity(lease));
    const blocked = blockedTeraReceiptOutcome(
      receipt,
      t("tera-actions-insert-engine-blocked"),
    );
    if (blocked) return blocked;
    const patch = requireCommittedPreviewStructuralPatch(
      receipt,
      t("tera-actions-insert-engine-blocked"),
    );
    const settlement = await projectCommittedPreviewStructuralMutation(host, lease, receipt, patch, () => {
      projectCommittedTeraSource(host, patch);
    });
    const insertedDynamicWidgetId = dynamicWidgetInstanceIdFromSnippet(patch.snippet);
    if (
      insertedDynamicWidgetId
      && settlement.projections.sourceGraph === "current"
      && settlement.projections.preview === "current"
    ) {
      try {
        await host.selectDynamicWidgetSourceInstance?.(insertedDynamicWidgetId);
      } catch {
        // Selection is a derived convenience after the Rust commit. It must
        // never turn a committed insertion into a second mutation attempt.
      }
    }
    host.setGlobalStatus(
      settlement.warnings.length > 0
        ? t("tera-actions-inserted-resync", { label: resolution.label })
        : t("tera-actions-inserted", {
          label: resolution.label,
          position: dropPositionLabel(resolution.position),
          path: projectRelativeZolaPath(patch.file),
        }),
      "unsaved",
    );
    return committedAction();
  } catch (error) {
    const reason = errorMessage(error);
    host.setGlobalStatus(t("tera-actions-insert-error", { message: reason }), "error");
    return failedAction(reason);
  }
}

export async function deleteSelectedTeraNode(
  host: TeraActionsControllerHost,
  requestedNode?: SourceGraphNode | null,
): Promise<EditorActionOutcome> {
  const targetNode = captureTeraActionTarget(
    requestedNode === undefined ? host.selectedTemplateSourceNode : requestedNode,
  );
  try {
    const result = await runInPreviewStructuralLane(host, (lease) =>
      deleteSelectedTeraNodeInLane(host, targetNode, lease));
    return result ?? cancelledAction(t("tera-actions-delete-session-cancelled"));
  } catch (error) {
    const reason = errorMessage(error);
    host.setGlobalStatus(t("tera-actions-delete-error", { message: reason }), "error");
    return failedAction(reason);
  }
}

async function deleteSelectedTeraNodeInLane(
  host: TeraActionsControllerHost,
  node: SourceGraphNode | null,
  lease: PreviewStructuralSessionLease,
): Promise<EditorActionOutcome> {
  const capability = deleteTeraNodeCapability(node);
  if (!node || !capability.canRun || !node.range) {
    host.setGlobalStatus(capability.reason, "error");
    return blockedAction(capability.reason);
  }

  try {
    const receipt = await executePreviewTeraDeleteIntent({
      intent: {
        messageType: "preview-template-delete-selected",
        sourceId: node.id,
      },
      deleteIntent: {
        targetSourceId: node.id,
        targetLocation: {
          file: node.file,
          line: node.range.line,
          column: node.range.column,
        },
        targetKind: node.kind,
        targetLabel: node.label,
      },
    }, previewStructuralCommandIdentity(lease, true));
    const blocked = blockedTeraReceiptOutcome(
      receipt,
      t("tera-actions-delete-engine-blocked"),
    );
    if (blocked) return blocked;
    const patch = requireCommittedPreviewStructuralPatch(
      receipt,
      t("tera-actions-delete-engine-blocked"),
    );
    const settlement = await projectCommittedPreviewStructuralMutation(host, lease, receipt, patch, () => {
      projectCommittedTeraSource(host, patch);
      host.clearPreviewSelection({ clearCanvasOverlay: true });
    });
    host.setGlobalStatus(
      settlement.warnings.length > 0
        ? t("tera-actions-deleted-resync", { label: capability.label })
        : t("tera-actions-deleted", {
          label: capability.label,
          path: projectRelativeZolaPath(patch.file),
        }),
      "unsaved",
    );
    return committedAction();
  } catch (error) {
    const reason = errorMessage(error);
    host.setGlobalStatus(t("tera-actions-delete-error", { message: reason }), "error");
    return failedAction(reason);
  }
}
