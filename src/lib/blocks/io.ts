import {
  INSERT_CATALOG_SCHEMA_VERSION,
  type NativeBlockRegistrySnapshot,
  type UiBlockGraphSnapshot,
  type InsertCatalogContext,
  type InsertCatalogSnapshot,
} from "$lib/blocks/contracts";
import type { FileBufferRequestIdentity } from "$lib/project/workspace-contract";
import { invoke } from "@tauri-apps/api/core";
import { t } from "$lib/i18n/runtime.svelte";
import { requireProjectFileRequestIdentity } from "$lib/session/workspace-entry-io";

export function readNativeBlockRegistry(): Promise<NativeBlockRegistrySnapshot> {
  return invoke<NativeBlockRegistrySnapshot>("read_native_block_registry");
}

export async function readUiBlockGraph(
  identity: FileBufferRequestIdentity,
): Promise<UiBlockGraphSnapshot> {
  requireProjectFileRequestIdentity(identity);
  const snapshot = await invoke<UiBlockGraphSnapshot>("read_ui_block_graph", {
    identity,
  });
  if (
    snapshot.schemaVersion !== 4
    || snapshot.projectRoot !== identity.expectedProjectRoot
    || snapshot.runtimeSessionId !== identity.expectedSessionId
  ) {
    throw new Error(t("io-ui-block-graph-session-mismatch"));
  }
  return snapshot;
}

export async function readInsertCatalog(
  identity: FileBufferRequestIdentity,
  expectedWorkspaceRevision: number,
  context: InsertCatalogContext,
): Promise<InsertCatalogSnapshot> {
  requireProjectFileRequestIdentity(identity);
  const snapshot = await invoke<InsertCatalogSnapshot>("read_insert_catalog", {
    request: {
      identity,
      expectedWorkspaceRevision,
      context,
    },
  });
  if (
    snapshot.schemaVersion !== INSERT_CATALOG_SCHEMA_VERSION
    || snapshot.projectRoot !== identity.expectedProjectRoot
    || snapshot.runtimeSessionId !== identity.expectedSessionId
    || snapshot.workspaceRevision !== expectedWorkspaceRevision
  ) {
    throw new Error("Catalogul de inserare nu mai aparține reviziei active.");
  }
  return snapshot;
}
