import {
  blockedAction,
  committedAction,
  editorActionSucceeded,
  type EditorActionOutcome,
} from "$lib/editor-runtime/action-outcome";
import { previewStructuralCommandIdentity } from "$lib/kernel/preview-structural-lane";
import { executePreviewSelectionBatchIntent } from "$lib/preview/structural-io";
import { normalizeClassTokens } from "$lib/html/mutations";
import { committedDraftCanSettle } from "$lib/session/committed-draft-settlement";
import type { HtmlPendingArea } from "$lib/canvas/contracts";
import type { ProjectGeneratedIdentityKind } from "$lib/preview/contracts";
import type { HtmlActionsHost } from "$lib/editor/html-actions/host";
import type { HtmlActionTarget } from "$lib/editor/html-actions/target";
import {
  captureHtmlActionTarget,
  currentSelectionMatchesTarget,
} from "$lib/editor/html-actions/target";
import {
  actionErrorOutcome,
  commitSelectionBatchReceipt,
  executeSelectedHtmlAttributes,
  hasMultiElementSelection,
  runHtmlStructuralAction,
} from "$lib/editor/html-actions/execution";
import { attributeDraftToken } from "$lib/editor/html-actions/attribute-values";
import { t } from "$lib/i18n/runtime.svelte";

export async function generateClassForSelectedHtml(
  host: HtmlActionsHost,
): Promise<EditorActionOutcome> {
  const target = captureHtmlActionTarget(host.context().coordinatedSelection);
  if (!target) {
    host.html.classStatus = t("html-actions-class-select");
    return blockedAction(host.html.classStatus);
  }
  if (hasMultiElementSelection(host)) {
    host.commands.setPending("classes", true);
    host.html.classStatus = t("inspector-applying");
    try {
      const result = await runHtmlStructuralAction(host, async (lease) => {
        if (!lease.selection || lease.selection.members.length < 2) {
          return blockedAction(t("html-actions-shared-class-minimum"));
        }
        const receipt = await executePreviewSelectionBatchIntent({
          schemaVersion: 1,
          action: { kind: "generateSharedClass" },
        }, previewStructuralCommandIdentity(lease, true));
        if (receipt.status !== "committed" || !receipt.generatedClass) {
          return blockedAction(
            receipt.diagnostics[0] || t("html-actions-shared-class-receipt-missing"),
          );
        }
        await commitSelectionBatchReceipt(host, lease, receipt);
        const classes = normalizeClassTokens(host.html.classEditorValue);
        if (!classes.includes(receipt.generatedClass)) {
          host.html.classEditorValue = [...classes, receipt.generatedClass].join(" ");
        }
        return committedAction();
      }, t("html-actions-shared-class-cancelled"));
      host.html.classStatus = editorActionSucceeded(result)
        ? t("html-actions-classes-applied")
        : (result.reason ?? t("html-actions-classes-kernel-refused"));
      return result;
    } catch (error) {
      const result = actionErrorOutcome(error);
      host.html.classStatus = result.reason ?? result.status;
      host.commands.setStatus(host.html.classStatus, "error");
      return result;
    } finally {
      host.commands.setPending("classes", false);
    }
  }
  return await generateIdentityForTarget(host, target, "class");
}

export async function generateDataAnimForSelectedHtml(
  host: HtmlActionsHost,
): Promise<EditorActionOutcome> {
  const target = captureHtmlActionTarget(host.context().coordinatedSelection);
  if (!target) {
    host.draft.attributeStatus = t("html-actions-data-anim-select");
    return blockedAction(host.draft.attributeStatus);
  }
  return await generateIdentityForTarget(host, target, "dataAnim");
}

async function generateIdentityForTarget(
  host: HtmlActionsHost,
  target: HtmlActionTarget,
  kind: ProjectGeneratedIdentityKind,
): Promise<EditorActionOutcome> {
  const pendingArea: HtmlPendingArea = kind === "class" ? "classes" : "attributes";
  const baselineClasses = normalizeClassTokens(host.html.classEditorValue).join(" ");
  const baselineAttributeValues = Object.freeze({ ...host.draft.attributeValues });
  const baselineAttributes = attributeDraftToken(baselineAttributeValues);
  host.commands.setPending(pendingArea, true);
  if (kind === "class") host.html.classStatus = t("inspector-applying");
  else host.draft.attributeStatus = t("inspector-applying");

  try {
    const result = await executeSelectedHtmlAttributes(host, {
      target,
      attributes: {},
      project: (capturedTarget, patch) => {
        const projection = patch.generatedIdentity;
        if (!projection || projection.kind !== kind) {
          throw new Error(t("html-actions-generated-identity-receipt-invalid"));
        }
        if (!currentSelectionMatchesTarget(host, capturedTarget)) return;

        if (kind === "class") {
          const submittedClasses = normalizeClassTokens(projection.classes.join(" ")).join(" ");
          const currentClasses = normalizeClassTokens(host.html.classEditorValue).join(" ");
          if (committedDraftCanSettle(currentClasses, submittedClasses, baselineClasses)) {
            host.html.classEditorValue = submittedClasses;
          }
          host.html.classStatus = projection.alreadyPresent
            ? t("html-actions-class-already-generated", { name: projection.value })
            : t("html-actions-classes-applied");
          return;
        }

        const nextAttributes = { ...baselineAttributeValues };
        if (projection.dataAnim) nextAttributes["data-anim"] = projection.dataAnim;
        else delete nextAttributes["data-anim"];
        const submittedAttributes = attributeDraftToken(nextAttributes);
        if (committedDraftCanSettle(
          attributeDraftToken(host.draft.attributeValues),
          submittedAttributes,
          baselineAttributes,
        )) {
          host.draft.attributeValues = nextAttributes;
        }
        host.draft.attributeStatus = projection.alreadyPresent
          ? t("html-actions-data-anim-exists", { value: projection.value })
          : t("html-actions-attributes-applied");
      },
      generatedIdentity: { kind },
    });
    host.commands.setPending(pendingArea, false);
    if (!editorActionSucceeded(result)) {
      const reason = result.reason ?? t("html-actions-attributes-kernel-refused");
      if (kind === "class") host.html.classStatus = reason;
      else host.draft.attributeStatus = reason;
      host.commands.setStatus(reason, "error");
    }
    return result;
  } catch (error) {
    host.commands.setPending(pendingArea, false);
    const result = actionErrorOutcome(error);
    const message = t("html-actions-generic-error", {
      message: result.reason ?? result.status,
    });
    if (kind === "class") host.html.classStatus = message;
    else host.draft.attributeStatus = message;
    host.commands.setStatus(message, "error");
    return result;
  }
}
