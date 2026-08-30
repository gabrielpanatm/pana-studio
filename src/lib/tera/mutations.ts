import type { SourceGraphNode } from "$lib/source-graph/contracts";
import { t } from "$lib/i18n/runtime.svelte";

const templateLevelTeraKinds = new Set(["extends", "block", "componentDefinition"]);
const deletableTeraKinds = new Set([
  "include",
  "for",
  "if",
  "set",
  "teraVariable",
  "teraComment",
]);

export type TeraMutationCapability = {
  canRun: boolean;
  label: string;
  reason: string;
};

/**
 * UI capability only. Rust resolves and validates the exact SourceNodeId again
 * when the command is committed; frontend source ranges never authorize a write.
 */
export function deleteTeraNodeCapability(node: SourceGraphNode | null): TeraMutationCapability {
  if (!node) {
    return {
      canRun: false,
      label: t("tera-mutation-delete-node"),
      reason: t("tera-mutation-select-node"),
    };
  }

  if (templateLevelTeraKinds.has(node.kind)) {
    return {
      canRun: false,
      label: t("tera-mutation-delete"),
      reason: t("tera-mutation-template-level-code-only"),
    };
  }

  if (node.kind === "tera") {
    return {
      canRun: false,
      label: t("tera-mutation-delete"),
      reason: t("tera-mutation-generic-code-only"),
    };
  }

  if (node.kind === "raw") {
    return {
      canRun: false,
      label: t("tera-mutation-delete"),
      reason: t("tera-mutation-raw-code-only"),
    };
  }

  if (!deletableTeraKinds.has(node.kind)) {
    return {
      canRun: false,
      label: t("tera-mutation-delete-node"),
      reason: t("tera-mutation-not-editable"),
    };
  }

  return {
    canRun: true,
    label: node.kind === "include" ? t("tera-mutation-delete-include") : t("tera-mutation-delete"),
    reason: t("tera-mutation-delete-description"),
  };
}
