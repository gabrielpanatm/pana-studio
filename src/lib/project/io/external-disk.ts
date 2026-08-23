import type {
  KernelExternalDiskReconcileInput,
  KernelExternalDiskReconcileReceipt,
  ProjectDiskManifest,
} from "$lib/project/external-disk-contract";
import { invoke } from "@tauri-apps/api/core";
import { schemaMismatch } from "$lib/contracts/io-schema";

export function readCurrentProjectDiskManifest(): Promise<ProjectDiskManifest> {
  return invoke<ProjectDiskManifest>("read_current_project_disk_manifest");
}

type ProjectDiskWatchIdentity = {
  expectedProjectRoot: string;
  expectedSessionId: string;
};

type ProjectDiskWatchReceipt = {
  projectRoot: string;
  runtimeSessionId: string;
  watchGeneration: number;
};

export type ProjectDiskWatchStopIdentity = ProjectDiskWatchIdentity & {
  expectedWatchGeneration: number;
};

export function startProjectDiskWatch(
  input: ProjectDiskWatchIdentity,
): Promise<ProjectDiskWatchReceipt> {
  return invoke<ProjectDiskWatchReceipt>("start_project_disk_watch", { input });
}

export function stopProjectDiskWatch(
  input: ProjectDiskWatchStopIdentity,
): Promise<void> {
  return invoke<void>("stop_project_disk_watch", { input });
}

export async function reconcileCleanExternalProjectFiles(
  input: KernelExternalDiskReconcileInput,
): Promise<KernelExternalDiskReconcileReceipt> {
  const receipt = await invoke<KernelExternalDiskReconcileReceipt>(
    "reconcile_clean_external_project_files",
    { input },
  );
  if (receipt.schemaVersion !== 2) {
    throw schemaMismatch("External disk reconcile", receipt.schemaVersion, 2);
  }
  return receipt;
}
