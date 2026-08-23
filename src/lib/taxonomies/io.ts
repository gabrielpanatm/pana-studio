import type { PreviewStructuralCommandIdentity } from "$lib/preview/contracts";
import type { FileBufferRequestIdentity } from "$lib/project/workspace-contract";
import type { WorkspaceCatalogProjectionReceipt } from "$lib/source-graph/graph-contract";
import {
  TAXONOMY_CATALOG_SCHEMA_VERSION,
  TAXONOMY_MUTATION_SCHEMA_VERSION,
  type TaxonomyCatalogSnapshot,
  type TaxonomyMutationApplyReceipt,
  type TaxonomyMutationInput,
  type TaxonomyMutationPlan,
} from "$lib/taxonomies/contracts";
import { invoke } from "@tauri-apps/api/core";
import { t } from "$lib/i18n/runtime.svelte";
import { schemaMismatch } from "$lib/contracts/io-schema";
import { requireWorkspaceCatalogProjectionReceipt } from "$lib/session/catalog-receipt";

export function readTaxonomyCatalog(
  identity: PreviewStructuralCommandIdentity,
  expectedWorkspaceRevision?: number,
): Promise<TaxonomyCatalogSnapshot> {
  if (!identity.expectedProjectRoot.trim() || !identity.expectedSessionId.trim()) {
    return Promise.reject(new Error(
      t("io-taxonomy-catalog-identity-invalid"),
    ));
  }
  return invoke<WorkspaceCatalogProjectionReceipt<TaxonomyCatalogSnapshot>>(
    "read_taxonomy_catalog",
    { identity },
  ).then((receipt) => {
    requireWorkspaceCatalogProjectionReceipt(
      "taxonomies",
      identity,
      expectedWorkspaceRevision,
      receipt,
    );
    const snapshot = receipt.catalog;
    if (snapshot.schemaVersion !== TAXONOMY_CATALOG_SCHEMA_VERSION) {
      throw schemaMismatch(
        t("io-resource-taxonomy-catalog"),
        snapshot.schemaVersion,
        TAXONOMY_CATALOG_SCHEMA_VERSION,
      );
    }
    return snapshot;
  });
}

export async function planTaxonomyMutation(
  input: TaxonomyMutationInput,
  identity: FileBufferRequestIdentity,
): Promise<TaxonomyMutationPlan> {
  const plan = await invoke<TaxonomyMutationPlan>("plan_taxonomy_mutation", { input, identity });
  if (plan.schemaVersion !== TAXONOMY_MUTATION_SCHEMA_VERSION) {
    throw schemaMismatch(
      t("io-resource-taxonomy-plan"),
      plan.schemaVersion,
      TAXONOMY_MUTATION_SCHEMA_VERSION,
    );
  }
  return plan;
}

export async function applyTaxonomyMutation(
  input: TaxonomyMutationInput,
  expectedPlanId: string,
  identity: FileBufferRequestIdentity,
): Promise<TaxonomyMutationApplyReceipt> {
  const receipt = await invoke<TaxonomyMutationApplyReceipt>("apply_taxonomy_mutation", {
    input,
    expectedPlanId,
    identity,
  });
  if (receipt.plan.schemaVersion !== TAXONOMY_MUTATION_SCHEMA_VERSION) {
    throw schemaMismatch(
      t("io-resource-taxonomy-receipt"),
      receipt.plan.schemaVersion,
      TAXONOMY_MUTATION_SCHEMA_VERSION,
    );
  }
  return receipt;
}
