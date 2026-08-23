import type { PreviewStructuralCommandIdentity } from "$lib/preview/contracts";
import type {
  FileBufferRequestIdentity,
  WorkspaceEntryMutationReceipt,
} from "$lib/project/workspace-contract";
import type { WorkspaceCatalogProjectionReceipt } from "$lib/source-graph/graph-contract";
import {
  type CreateListingItemInput,
  type CreateSemanticTemplateInput,
  type DeleteListingItemInput,
  type DeleteTemplateInput,
  type DuplicateTemplateInput,
  type OverrideThemeTemplateInput,
  type RenameTemplateInput,
  type SetTemplateAssignmentInput,
  type SetTemplateParentInput,
  TEMPLATE_CATALOG_SCHEMA_VERSION,
  type TemplateCatalogSnapshot,
} from "$lib/templates/contracts";
import { invoke } from "@tauri-apps/api/core";
import { t } from "$lib/i18n/runtime.svelte";
import { schemaMismatch } from "$lib/contracts/io-schema";
import { requireWorkspaceCatalogProjectionReceipt } from "$lib/session/catalog-receipt";
import { invokeWorkspaceEntryMutation } from "$lib/session/workspace-entry-io";

export function readTemplateCatalog(
  identity: PreviewStructuralCommandIdentity,
  expectedWorkspaceRevision?: number,
): Promise<TemplateCatalogSnapshot> {
  if (!identity.expectedProjectRoot.trim() || !identity.expectedSessionId.trim()) {
    return Promise.reject(new Error(
      t("io-template-catalog-identity-invalid"),
    ));
  }
  return invoke<WorkspaceCatalogProjectionReceipt<TemplateCatalogSnapshot>>(
    "read_template_catalog",
    { identity },
  ).then((receipt) => {
    requireWorkspaceCatalogProjectionReceipt(
      "templates",
      identity,
      expectedWorkspaceRevision,
      receipt,
    );
    const snapshot = receipt.catalog;
    if (snapshot.schemaVersion !== TEMPLATE_CATALOG_SCHEMA_VERSION) {
      throw schemaMismatch(
        t("io-resource-template-catalog"),
        snapshot.schemaVersion,
        TEMPLATE_CATALOG_SCHEMA_VERSION,
      );
    }
    return snapshot;
  });
}

export function createListingItem(
  input: CreateListingItemInput,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "workspace_create_listing_item",
    { input, identity },
    identity,
  );
}

export function deleteListingItem(
  input: DeleteListingItemInput,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "workspace_delete_listing_item",
    { input, identity },
    identity,
  );
}

export function createSemanticTemplate(
  input: CreateSemanticTemplateInput,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "workspace_create_semantic_template",
    { input, identity },
    identity,
  );
}

export function duplicateTemplate(
  input: DuplicateTemplateInput,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation("workspace_duplicate_template", { input, identity }, identity);
}

export function overrideThemeTemplate(
  input: OverrideThemeTemplateInput,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "workspace_override_theme_template",
    { input, identity },
    identity,
  );
}

export function renameTemplate(
  input: RenameTemplateInput,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation("workspace_rename_template", { input, identity }, identity);
}

export function setTemplateParent(
  input: SetTemplateParentInput,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "workspace_set_template_parent",
    { input, identity },
    identity,
  );
}

export function setTemplateAssignment(
  input: SetTemplateAssignmentInput,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "workspace_set_template_assignment",
    { input, identity },
    identity,
  );
}

export function deleteTemplate(
  input: DeleteTemplateInput,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation("workspace_delete_template", { input, identity }, identity);
}
