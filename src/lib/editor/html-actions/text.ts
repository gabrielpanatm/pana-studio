import {
  blockedAction,
  editorActionSucceeded,
  noopAction,
  type EditorActionOutcome,
} from "$lib/editor-runtime/action-outcome";
import type { HtmlActionsHost } from "$lib/editor/html-actions/host";
import type { HtmlActionTarget } from "$lib/editor/html-actions/target";
import { currentSelectionMatchesTarget } from "$lib/editor/html-actions/target";
import {
  actionErrorOutcome,
  executeSelectedHtmlText,
  missingKernelIdentityMessage,
  type ExecuteSelectedHtmlTextOptions,
} from "$lib/editor/html-actions/execution";
import { t } from "$lib/i18n/runtime.svelte";

export async function applyTextContentToCapturedHtmlTarget(
  host: HtmlActionsHost,
  target: HtmlActionTarget,
  committedText: string,
  options: ExecuteSelectedHtmlTextOptions = {},
): Promise<EditorActionOutcome> {
  if (target.hasChildElements) {
    host.draft.textStatus = t("html-actions-text-simple-only");
    return blockedAction(host.draft.textStatus);
  }
  if (!target.sourceId) {
    host.draft.textStatus = missingKernelIdentityMessage(t("html-actions-text-noun"));
    return blockedAction(host.draft.textStatus);
  }
  const selectionKey = [
    target.sessionId ?? "",
    target.selectionRevision ?? "",
    target.sourceId,
    target.renderInstanceId ?? "",
  ].join("::");
  const previousText = host.draft.textEditOriginalKey === selectionKey
    ? host.draft.textEditOriginalText ?? ""
    : target.rawText ?? "";
  if (!options.deferCanonicalProjection && committedText === previousText) {
    host.draft.textEditOriginalKey = null;
    host.draft.textEditOriginalText = null;
    host.commands.setPending("text", false);
    host.draft.textStatus = t("html-actions-text-no-changes");
    return noopAction(host.draft.textStatus);
  }

  host.commands.setPending("text", true);
  try {
    const result = await executeSelectedHtmlText(
      host,
      target,
      committedText,
      (_patch, capturedTarget) => {
        if (
          currentSelectionMatchesTarget(host, capturedTarget)
          && host.draft.textContentValue === committedText
        ) {
          host.draft.textStatus = options.deferCanonicalProjection
            ? t("html-actions-text-confirmed-recoverable")
            : t("html-actions-text-applied");
          if (!options.deferCanonicalProjection) {
            host.draft.textEditOriginalKey = null;
            host.draft.textEditOriginalText = null;
          }
        }
        // ProjectWorkspace history owns committed text. Canonical Preview
        // projection performs the frontend history handoff.
      },
      options,
    );
    if (
      editorActionSucceeded(result)
      && currentSelectionMatchesTarget(host, target)
      && host.draft.textContentValue === committedText
    ) {
      host.commands.setPending("text", false);
    }
    return result;
  } catch (error) {
    const result = actionErrorOutcome(error);
    host.draft.textStatus = t("html-actions-generic-error", {
      message: result.reason ?? result.status,
    });
    host.commands.setStatus(t("html-actions-text-error", {
      message: result.reason ?? result.status,
    }), "error");
    return result;
  }
}
