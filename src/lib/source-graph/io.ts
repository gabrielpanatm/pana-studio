import type { PreviewStructuralCommandIdentity } from "$lib/preview/contracts";
import type { SourceGraphProjectionReceipt } from "$lib/source-graph/graph-contract";
import { invoke } from "@tauri-apps/api/core";
import { t } from "$lib/i18n/runtime.svelte";

export function readSourceGraph(
  identity: PreviewStructuralCommandIdentity,
): Promise<SourceGraphProjectionReceipt> {
  if (!identity.expectedProjectRoot.trim() || !identity.expectedSessionId.trim()) {
    return Promise.reject(new Error(
      t("io-source-graph-identity-invalid"),
    ));
  }
  return invoke<SourceGraphProjectionReceipt>("read_source_graph", { identity });
}
