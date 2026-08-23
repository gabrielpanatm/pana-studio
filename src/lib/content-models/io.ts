import {
  CONTENT_MODEL_SCHEMA_VERSION,
  type ContentModelCatalog,
  type ContentModelMutationApplyReceipt,
  type ContentModelMutationInput,
  type ContentModelMutationPlan,
} from "$lib/content-models/contracts";
import type {
  FileBufferCommandReceipt,
  FileBufferRequestIdentity,
} from "$lib/project/workspace-contract";
import { invoke } from "@tauri-apps/api/core";
import { t } from "$lib/i18n/runtime.svelte";
import { schemaMismatch } from "$lib/contracts/io-schema";

export async function readContentModelCatalog(
  identity: FileBufferRequestIdentity,
  expectedWorkspaceRevision?: number,
): Promise<ContentModelCatalog> {
  const receipt = await invoke<FileBufferCommandReceipt<ContentModelCatalog>>(
    "read_content_model_catalog",
    { identity },
  );
  if (
    receipt.projectRoot !== identity.expectedProjectRoot
    || receipt.runtimeSessionId !== identity.expectedSessionId
  ) {
    throw new Error(t("io-workspace-catalog-identity-invalid", {
      resource: t("io-resource-content-model-catalog"),
    }));
  }
  if (
    expectedWorkspaceRevision !== undefined
    && receipt.workspaceRevision !== expectedWorkspaceRevision
  ) {
    throw new Error(t("io-workspace-catalog-revision-mismatch", {
      resource: t("io-resource-content-model-catalog"),
      actual: receipt.workspaceRevision,
      expected: expectedWorkspaceRevision,
    }));
  }
  if (receipt.payload.schemaVersion !== CONTENT_MODEL_SCHEMA_VERSION) {
    throw schemaMismatch(
      t("io-resource-content-model-catalog"),
      receipt.payload.schemaVersion,
      CONTENT_MODEL_SCHEMA_VERSION,
    );
  }
  return receipt.payload;
}

export async function planContentModelMutation(
  input: ContentModelMutationInput,
  identity: FileBufferRequestIdentity,
): Promise<ContentModelMutationPlan> {
  const plan = await invoke<ContentModelMutationPlan>("plan_content_model_mutation", {
    input,
    identity,
  });
  if (plan.schemaVersion !== CONTENT_MODEL_SCHEMA_VERSION) {
    throw schemaMismatch(
      t("io-resource-content-model-plan"),
      plan.schemaVersion,
      CONTENT_MODEL_SCHEMA_VERSION,
    );
  }
  return plan;
}

export async function applyContentModelMutation(
  input: ContentModelMutationInput,
  expectedPlanId: string,
  identity: FileBufferRequestIdentity,
): Promise<ContentModelMutationApplyReceipt> {
  const receipt = await invoke<ContentModelMutationApplyReceipt>(
    "apply_content_model_mutation",
    { input, expectedPlanId, identity },
  );
  if (receipt.plan.schemaVersion !== CONTENT_MODEL_SCHEMA_VERSION) {
    throw schemaMismatch(
      t("io-resource-content-model-receipt"),
      receipt.plan.schemaVersion,
      CONTENT_MODEL_SCHEMA_VERSION,
    );
  }
  return receipt;
}
