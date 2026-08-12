import type { CoordinatedElementSelection, EditableAttributes, HtmlPendingArea } from "$lib/types";
import type { GlobalStatusKind } from "$lib/status/global-status";
import { t } from "$lib/i18n/runtime.svelte";

export type HtmlDraftControllerHost = {
  coordinatedElementSelection: CoordinatedElementSelection | null;
  attributeValues: EditableAttributes;
  textContentValue: string;
  textEditOriginalKey: string | null;
  textEditOriginalText: string | null;
  attributeStatus: string;
  textStatus: string;
  setHtmlPending: (area: HtmlPendingArea, pending: boolean) => void;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
};

export function htmlTextSelectionKey(selection: CoordinatedElementSelection) {
  if (!selection.sourceNodeId) return null;
  return [
    selection.snapshot.runtimeSessionId,
    selection.snapshot.selectionRevision,
    selection.sourceNodeId,
    selection.renderInstanceId,
  ].join("::");
}

/** Keeps the inspector draft; AppState owns the acknowledged speculative Canvas projection. */
export function updateAttributeValue(
  host: HtmlDraftControllerHost,
  property: string,
  value: string,
) {
  host.attributeValues = { ...host.attributeValues, [property]: value };
  host.setHtmlPending("attributes", true);
  host.setGlobalStatus(
    t("html-draft-attribute-changed", { property }),
    "unsaved",
  );
  host.attributeStatus = t("html-draft-attribute-pending");
}

export function updateTextContentValue(host: HtmlDraftControllerHost, value: string) {
  host.textContentValue = value;
  const selection = host.coordinatedElementSelection;
  if (!selection || selection.observation.hasChildElements) {
    host.textStatus = t("html-draft-text-simple-only");
    return;
  }
  const key = htmlTextSelectionKey(selection);
  if (!key) {
    host.textStatus = t("html-actions-identity-missing", {
      action: t("html-actions-text-noun"),
    });
    host.setGlobalStatus(host.textStatus, "error");
    return;
  }
  if (host.textEditOriginalKey !== key) {
    host.textEditOriginalKey = key;
    host.textEditOriginalText = selection.observation.rawText ?? "";
  }
  host.setHtmlPending("text", true);
  host.setGlobalStatus(t("html-draft-text-changed"), "unsaved");
  host.textStatus = t("html-draft-text-pending");
}

export function removeAttribute(host: HtmlDraftControllerHost, name: string) {
  const { [name]: _removed, ...rest } = host.attributeValues;
  host.attributeValues = rest;
  host.setHtmlPending("attributes", true);
  host.setGlobalStatus(
    t("html-draft-attribute-removed", { name }),
    "unsaved",
  );
  host.attributeStatus = t("html-draft-attribute-removal-pending");
}
