import type { EditableAttributes } from "$lib/types";
import { htmlAttributePreviewMode } from "./editor-schema";

export type LiveProjectableHtmlAttributeDraft = Readonly<{
  attributes: EditableAttributes;
  baselineNames: string[];
}>;

/**
 * Produces the speculative Editare sigură projection only. The canonical source
 * mutation still receives every source-editable attribute, including values
 * which Editare sigură deliberately neutralizes (target, download, action, ...).
 */
export function liveProjectableHtmlAttributeDraft(
  tag: string,
  attributes: Readonly<EditableAttributes>,
  baselineNames: readonly string[],
): LiveProjectableHtmlAttributeDraft {
  const normalizedTag = tag.trim().toLowerCase();
  const projectedAttributes: EditableAttributes = {};
  for (const [name, value] of Object.entries(attributes)) {
    const normalizedName = name.trim().toLowerCase();
    if (htmlAttributePreviewMode(normalizedName, normalizedTag) !== "live") continue;
    projectedAttributes[normalizedName] = value;
  }
  return {
    attributes: projectedAttributes,
    baselineNames: baselineNames
      .map((name) => name.trim().toLowerCase())
      .filter((name) => htmlAttributePreviewMode(name, normalizedTag) === "live"),
  };
}

export function isLatestHtmlAttributeDraftSettlement(
  activeSessionId: string | null,
  latestIssuedEpoch: number,
  settledSessionId: string,
  settledEpoch: number,
) {
  return activeSessionId === settledSessionId && latestIssuedEpoch === settledEpoch;
}
