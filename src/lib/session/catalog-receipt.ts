import type { PreviewStructuralCommandIdentity } from "$lib/preview/contracts";
import type { WorkspaceCatalogProjectionReceipt } from "$lib/source-graph/graph-contract";
import { t } from "$lib/i18n/runtime.svelte";

/** @internal Shared receipt validation for session-bound catalog IO. */
export function requireWorkspaceCatalogProjectionReceipt<T>(
  kind: "templates" | "taxonomies",
  identity: PreviewStructuralCommandIdentity,
  expectedWorkspaceRevision: number | undefined,
  receipt: WorkspaceCatalogProjectionReceipt<T>,
) {
  const resource = kind === "templates"
    ? t("io-resource-template-catalog")
    : t("io-resource-taxonomy-catalog");
  if (
    receipt.projectRoot !== identity.expectedProjectRoot
    || receipt.runtimeSessionId !== identity.expectedSessionId
    || !Number.isSafeInteger(receipt.workspaceRevision)
    || receipt.workspaceRevision < 0
  ) {
    throw new Error(t("io-workspace-catalog-identity-invalid", { resource }));
  }
  if (
    expectedWorkspaceRevision !== undefined
    && receipt.workspaceRevision !== expectedWorkspaceRevision
  ) {
    throw new Error(
      t("io-workspace-catalog-revision-mismatch", {
        resource,
        actual: receipt.workspaceRevision,
        expected: expectedWorkspaceRevision,
      }),
    );
  }
}
