import type {
  DeleteDynamicWidgetInput,
  DynamicWidgetSnapshot,
  DynamicWidgetSnapshotRequest,
  UpdateDynamicWidgetInput,
} from "$lib/content-models/contracts";
import type { WorkspaceEntryMutationReceipt } from "$lib/project/workspace-contract";
import { invoke } from "@tauri-apps/api/core";
import {
  requireProjectFileRequestIdentity,
  invokeWorkspaceEntryMutation,
} from "$lib/session/workspace-entry-io";

export async function readDynamicWidgetSnapshot(
  request: DynamicWidgetSnapshotRequest,
): Promise<DynamicWidgetSnapshot> {
  requireProjectFileRequestIdentity(request.identity);
  const snapshot = await invoke<DynamicWidgetSnapshot>("read_dynamic_widget_snapshot", {
    request,
  });
  if (
    snapshot.schemaVersion !== 1
    || snapshot.projectRoot !== request.identity.expectedProjectRoot
    || snapshot.runtimeSessionId !== request.identity.expectedSessionId
    || snapshot.workspaceRevision !== request.expectedWorkspaceRevision
    || snapshot.modelRevision !== request.expectedModelRevision
    || snapshot.previewRevision !== request.previewRevision
    || snapshot.sourceInstance.id !== request.sourceInstanceId
  ) {
    throw new Error("Snapshotul widgetului dinamic nu mai aparține selecției active.");
  }
  return snapshot;
}

export function updateDynamicWidget(
  input: UpdateDynamicWidgetInput,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "update_dynamic_widget",
    { input },
    input.request.identity,
  );
}

export function deleteDynamicWidget(
  input: DeleteDynamicWidgetInput,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "delete_dynamic_widget",
    { input },
    input.request.identity,
  );
}
