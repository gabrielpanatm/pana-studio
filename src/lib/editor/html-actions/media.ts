import {
  blockedAction,
  editorActionSucceeded,
  noopAction,
  type EditorActionOutcome,
} from "$lib/editor-runtime/action-outcome";
import {
  createZolaImageIntent,
  resolveZolaImageSource,
  zolaImageSourceFailureMessage,
} from "$lib/html/zola-image";
import type {
  NativeIconMutationIntent,
} from "$lib/blocks/contracts";
import type {
  NativeBlockOptionIntent,
  ProjectZolaImageIntent,
} from "$lib/preview/contracts";
import type { SourceEditLocation } from "$lib/source-graph/contracts";
import type { HtmlActionsHost } from "$lib/editor/html-actions/host";
import {
  captureHtmlActionTarget,
  currentSelectionMatchesTarget,
  freezeHtmlActionTarget,
} from "$lib/editor/html-actions/target";
import {
  actionErrorOutcome,
  executeSelectedHtmlAttributes,
} from "$lib/editor/html-actions/execution";
import { t } from "$lib/i18n/runtime.svelte";

export type ApplyNativeBlockOptionRequest = {
  providerId: string;
  optionId: string;
  value: NativeBlockOptionIntent["value"];
  rootTag: string;
  rootSourceId: string | null;
  rootLocation: SourceEditLocation | null;
  rootSessionId: string | null;
};

export async function applyNativeBlockOptionToHtml(
  host: HtmlActionsHost,
  request: ApplyNativeBlockOptionRequest,
): Promise<EditorActionOutcome> {
  const target = freezeHtmlActionTarget({
    tag: request.rootTag,
    sourceId: request.rootSourceId,
    sourceLocation: request.rootLocation,
    sessionId: request.rootSessionId,
  });
  try {
    const result = await executeSelectedHtmlAttributes(host, {
      target,
      attributes: {},
      project: () => {},
      nativeBlockOption: {
        providerId: request.providerId,
        optionId: request.optionId,
        value: request.value,
      },
    });
    if (result.status === "committed") {
      host.commands.setStatus(t("html-actions-block-property-confirmed"), "unsaved");
    }
    return result;
  } catch (error) {
    const outcome = actionErrorOutcome(error);
    host.commands.setStatus(t("html-actions-block-property-failed", {
      message: outcome.reason ?? outcome.status,
    }), "error");
    return outcome;
  }
}

export type ApplyNativeIconRequest = {
  intent: NativeIconMutationIntent;
  rootTag: string;
  rootSourceId: string | null;
  rootLocation: SourceEditLocation | null;
  rootSessionId: string | null;
};

export async function applyNativeIconToHtml(
  host: HtmlActionsHost,
  request: ApplyNativeIconRequest,
): Promise<EditorActionOutcome> {
  const target = freezeHtmlActionTarget({
    tag: request.rootTag,
    sourceId: request.rootSourceId,
    sourceLocation: request.rootLocation,
    sessionId: request.rootSessionId,
  });
  try {
    const result = await executeSelectedHtmlAttributes(host, {
      target,
      attributes: {},
      project: () => {},
      nativeIcon: request.intent,
    });
    if (result.status === "committed") {
      host.commands.setStatus(t("html-actions-icon-property-confirmed"), "unsaved");
    }
    return result;
  } catch (error) {
    const outcome = actionErrorOutcome(error);
    host.commands.setStatus(t("html-actions-icon-property-failed", {
      message: outcome.reason ?? outcome.status,
    }), "error");
    return outcome;
  }
}

export async function applyZolaImageProcessingToHtml(
  host: HtmlActionsHost,
  intent: ProjectZolaImageIntent,
): Promise<EditorActionOutcome> {
  const target = captureHtmlActionTarget(host.context().coordinatedSelection);
  if (!target || target.tag !== "img") {
    host.html.imageStatus = t("html-actions-zola-image-select");
    return blockedAction(host.html.imageStatus);
  }

  host.commands.setPending("image", true);
  host.html.imageStatus = intent.enabled
    ? t("html-actions-zola-image-configuring")
    : t("html-actions-zola-image-removing");
  try {
    const result = await executeSelectedHtmlAttributes(host, {
      target,
      attributes: {},
      project: (capturedTarget) => {
        if (!currentSelectionMatchesTarget(host, capturedTarget)) return;
        host.html.imageStatus = intent.enabled
          ? t("html-actions-zola-image-managed")
          : t("html-actions-zola-image-removed");
      },
      zolaImage: intent,
    });
    host.commands.setPending("image", false);
    return result;
  } catch (error) {
    const result = actionErrorOutcome(error);
    host.html.imageStatus = t("html-actions-zola-image-failed", {
      message: result.reason ?? result.status,
    });
    host.commands.setStatus(t("html-actions-zola-image-error", {
      message: result.reason ?? result.status,
    }), "error");
    host.commands.setPending("image", false);
    return result;
  }
}

export async function applyImageSourceToHtml(
  host: HtmlActionsHost,
  sourceOverride?: string,
): Promise<EditorActionOutcome> {
  const target = captureHtmlActionTarget(host.context().coordinatedSelection);
  if (!target || target.tag !== "img") {
    host.html.imageStatus = t("html-actions-image-select");
    return blockedAction(host.html.imageStatus);
  }

  const src = (sourceOverride ?? host.html.imageSourceValue).trim();
  host.html.imageSourceValue = src;
  const zolaImage = host.context().coordinatedSelection?.observation.zolaImage ?? null;
  if (zolaImage) {
    const source = resolveZolaImageSource(src, host.context().project?.files ?? []);
    if (!source.eligible) {
      host.html.imageStatus = zolaImageSourceFailureMessage(source.code);
      return blockedAction(host.html.imageStatus);
    }
    return await applyZolaImageProcessingToHtml(host, createZolaImageIntent({
      enabled: true,
      source,
      width: zolaImage.width,
      height: zolaImage.height,
      operation: zolaImage.operation,
      format: zolaImage.format,
      quality: zolaImage.quality,
    }));
  }
  if ((target.attributes?.src ?? "").trim() === src) {
    host.commands.setPending("image", false);
    host.html.imageStatus = t("html-actions-image-no-changes");
    return noopAction(host.html.imageStatus);
  }
  host.commands.setPending("image", true);
  try {
    const result = await executeSelectedHtmlAttributes(host, {
      target,
      attributes: { src: src || null },
      project: (capturedTarget) => {
        if (currentSelectionMatchesTarget(host, capturedTarget)) {
          host.html.imageSourceValue = src;
          host.html.imageStatus = t("html-actions-image-applied");
        }
      },
    });
    if (
      editorActionSucceeded(result)
      && currentSelectionMatchesTarget(host, target)
      && host.html.imageSourceValue.trim() === src
    ) {
      host.commands.setPending("image", false);
    }
    return result;
  } catch (error) {
    const result = actionErrorOutcome(error);
    host.html.imageStatus = t("html-actions-image-failed", {
      message: result.reason ?? result.status,
    });
    host.commands.setStatus(t("html-actions-image-error", {
      message: result.reason ?? result.status,
    }), "error");
    return result;
  }
}
