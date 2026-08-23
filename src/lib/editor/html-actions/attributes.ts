import {
  blockedAction,
  editorActionSucceeded,
  noopAction,
  type EditorActionOutcome,
} from "$lib/editor-runtime/action-outcome";
import { normalizeClassTokens } from "$lib/html/mutations";
import { committedDraftCanSettle } from "$lib/session/committed-draft-settlement";
import { isZolaTemplatePath } from "$lib/project/files";
import type { EditableAttributes } from "$lib/canvas/contracts";
import type { HtmlActionsHost } from "$lib/editor/html-actions/host";
import type { HtmlActionTarget } from "$lib/editor/html-actions/target";
import {
  captureHtmlActionTarget,
  currentSelectionMatchesTarget,
} from "$lib/editor/html-actions/target";
import {
  actionErrorOutcome,
  executeSelectedHtmlAttributes,
  executeSelectionBatch,
  hasMultiElementSelection,
} from "$lib/editor/html-actions/execution";
import {
  attributeDraftMatches,
  attributeDraftToken,
  batchCommonAttributeMutations,
  htmlAttributeRecordForKernel,
} from "$lib/editor/html-actions/attribute-values";
import { t } from "$lib/i18n/runtime.svelte";

function generatedPanaClass(className: string) {
  return /^ps-[a-z0-9-]+-[a-z0-9]{6,}$/i.test(className.trim());
}

export async function applyClassesToHtml(host: HtmlActionsHost): Promise<EditorActionOutcome> {
  const target = captureHtmlActionTarget(host.context().coordinatedSelection);
  if (!target) {
    host.html.classStatus = t("html-actions-classes-select");
    return blockedAction(host.html.classStatus);
  }

  const normalizedClasses = normalizeClassTokens(host.html.classEditorValue);
  if (hasMultiElementSelection(host)) {
    const primaryClasses = normalizeClassTokens(target.classes?.join(" ") ?? "");
    const nextClasses = new Set(normalizedClasses);
    const previousClasses = new Set(primaryClasses);
    const add = normalizedClasses.filter((className) => !previousClasses.has(className));
    const remove = primaryClasses.filter((className) => !nextClasses.has(className));
    if (add.length === 0 && remove.length === 0) {
      host.commands.setPending("classes", false);
      host.html.classStatus = t("html-actions-classes-no-changes");
      return noopAction(host.html.classStatus);
    }
    host.commands.setPending("classes", true);
    try {
      const result = await executeSelectionBatch(host, {
        kind: "mutateClasses",
        add,
        remove,
      });
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
  return await applyClassesToTarget(host, target, normalizedClasses);
}

async function applyClassesToTarget(
  host: HtmlActionsHost,
  target: HtmlActionTarget,
  classes: readonly string[],
  options: { markPending?: boolean } = {},
): Promise<EditorActionOutcome> {
  const normalizedClasses = normalizeClassTokens(classes.join(" "));
  const submittedClasses = normalizedClasses.join(" ");
  const baselineClasses = normalizeClassTokens(host.html.classEditorValue).join(" ");
  const targetClassTokens = normalizeClassTokens(target.classes?.join(" ") ?? "");
  const targetClasses = targetClassTokens.join(" ");
  const submittedClassSet = new Set(normalizedClasses);
  const removedGeneratedClass = targetClassTokens.some(
    (className) => generatedPanaClass(className) && !submittedClassSet.has(className),
  );
  if (submittedClasses === targetClasses) {
    host.commands.setPending("classes", false);
    host.html.classStatus = t("html-actions-classes-no-changes");
    return noopAction(host.html.classStatus);
  }
  if (options.markPending !== false) host.commands.setPending("classes", true);
  let submittedDraftProjected = false;
  try {
    const result = await executeSelectedHtmlAttributes(host, {
      target,
      attributes: { class: submittedClasses || null },
      project: async (capturedTarget, patch) => {
        const currentClasses = normalizeClassTokens(host.html.classEditorValue).join(" ");
        if (
          currentSelectionMatchesTarget(host, capturedTarget)
          && committedDraftCanSettle(currentClasses, submittedClasses, baselineClasses)
        ) {
          host.html.classEditorValue = submittedClasses;
          host.html.classStatus = t("html-actions-classes-applied");
          submittedDraftProjected = true;
        }
        if (removedGeneratedClass && isZolaTemplatePath(patch.file)) {
          await host.commands.reconcilePageAssets(patch.targetLocation);
        }
      },
    });
    if (
      editorActionSucceeded(result)
      && normalizeClassTokens(host.html.classEditorValue).join(" ") === submittedClasses
      && (submittedDraftProjected || currentSelectionMatchesTarget(host, target))
    ) {
      host.commands.setPending("classes", false);
    }
    if (!editorActionSucceeded(result)) {
      const reason = result.reason ?? t("html-actions-classes-kernel-refused");
      host.html.classStatus = t("html-actions-classes-not-applied", { reason });
      host.commands.setStatus(host.html.classStatus, "error");
    }
    return result;
  } catch (error) {
    const result = actionErrorOutcome(error);
    host.html.classStatus = t("html-actions-generic-error", {
      message: result.reason ?? result.status,
    });
    host.commands.setStatus(t("html-actions-classes-error", {
      message: result.reason ?? result.status,
    }), "error");
    return result;
  }
}

export async function applyAttributesToHtml(
  host: HtmlActionsHost,
  attributeOverride: EditableAttributes | null = null,
): Promise<EditorActionOutcome> {
  const target = captureHtmlActionTarget(host.context().coordinatedSelection);
  if (!target) {
    host.draft.attributeStatus = t("html-actions-attributes-select");
    return blockedAction(host.draft.attributeStatus);
  }

  const attributeValues = Object.freeze({
    ...(attributeOverride ?? host.draft.attributeValues),
  });
  if (hasMultiElementSelection(host)) {
    const mutations = batchCommonAttributeMutations(attributeValues, target.attributes ?? {});
    if (mutations.length === 0) {
      host.commands.setPending("attributes", false);
      host.draft.attributeStatus = t("inspector-no-canonical-difference");
      return noopAction(host.draft.attributeStatus);
    }
    host.commands.setPending("attributes", true);
    try {
      const result = await executeSelectionBatch(host, {
        kind: "setAttributes",
        attributes: mutations,
      });
      host.draft.attributeStatus = editorActionSucceeded(result)
        ? t("html-actions-attributes-applied")
        : (result.reason ?? t("html-actions-attributes-kernel-refused"));
      return result;
    } catch (error) {
      const result = actionErrorOutcome(error);
      host.draft.attributeStatus = result.reason ?? result.status;
      host.commands.setStatus(host.draft.attributeStatus, "error");
      return result;
    } finally {
      host.commands.setPending("attributes", false);
    }
  }
  return await applyAttributesToTarget(host, target, attributeValues);
}

async function applyAttributesToTarget(
  host: HtmlActionsHost,
  target: HtmlActionTarget,
  capturedAttributeValues: Readonly<EditableAttributes>,
  options: { markPending?: boolean } = {},
): Promise<EditorActionOutcome> {
  const attributeValues: EditableAttributes = { ...capturedAttributeValues };
  const baselineAttributeDraft = attributeDraftToken(host.draft.attributeValues);
  const submittedAttributeDraft = attributeDraftToken(attributeValues);
  const nextKernelAttributes = htmlAttributeRecordForKernel(
    attributeValues,
    target.attributes,
    Boolean(target.zolaImage),
  );
  const targetDataAnim = target.attributes?.["data-anim"]?.trim() ?? "";
  const submittedDataAnim = nextKernelAttributes["data-anim"]?.trim() ?? "";
  const removedOrReplacedDataAnim = targetDataAnim.length > 0 && targetDataAnim !== submittedDataAnim;

  if (options.markPending !== false) host.commands.setPending("attributes", true);
  let submittedDraftProjected = false;
  try {
    const result = await executeSelectedHtmlAttributes(host, {
      target,
      attributes: nextKernelAttributes,
      project: async (capturedTarget, patch) => {
        if (
          currentSelectionMatchesTarget(host, capturedTarget)
          && committedDraftCanSettle(
            attributeDraftToken(host.draft.attributeValues),
            submittedAttributeDraft,
            baselineAttributeDraft,
          )
        ) {
          host.draft.attributeValues = { ...attributeValues };
          host.draft.attributeStatus = t("html-actions-attributes-applied");
          submittedDraftProjected = true;
        }
        if (removedOrReplacedDataAnim && isZolaTemplatePath(patch.file)) {
          await host.commands.reconcilePageAssets(patch.targetLocation);
        }
      },
    });
    if (
      editorActionSucceeded(result)
      && attributeDraftMatches(host.draft.attributeValues, attributeValues)
      && (submittedDraftProjected || currentSelectionMatchesTarget(host, target))
    ) {
      host.commands.setPending("attributes", false);
    }
    if (!editorActionSucceeded(result)) {
      const reason = result.reason ?? t("html-actions-attributes-kernel-refused");
      host.draft.attributeStatus = t("html-actions-attributes-not-applied", { reason });
      host.commands.setStatus(host.draft.attributeStatus, "error");
    }
    return result;
  } catch (error) {
    const result = actionErrorOutcome(error);
    host.draft.attributeStatus = t("html-actions-generic-error", {
      message: result.reason ?? result.status,
    });
    host.commands.setStatus(t("html-actions-attributes-error", {
      message: result.reason ?? result.status,
    }), "error");
    return result;
  }
}

export async function applyAttributesToCapturedHtmlTarget(
  host: HtmlActionsHost,
  target: HtmlActionTarget,
  attributeValues: Readonly<EditableAttributes>,
): Promise<EditorActionOutcome> {
  return await applyAttributesToTarget(host, target, attributeValues);
}
