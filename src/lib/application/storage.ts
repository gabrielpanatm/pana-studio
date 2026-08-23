import { invoke } from "@tauri-apps/api/core";
import type {
  ApplicationStorageSnapshot,
  DeleteStorageSessionsRequest,
  StorageCleanupReceipt,
} from "$lib/application/contracts";

export function readApplicationStorageInventory(): Promise<ApplicationStorageSnapshot> {
  return invoke<ApplicationStorageSnapshot>("read_application_storage_inventory");
}

export function clearApplicationCacheStorage(): Promise<StorageCleanupReceipt> {
  return invoke<StorageCleanupReceipt>("clear_application_cache_storage");
}

export function clearApplicationLogStorage(): Promise<StorageCleanupReceipt> {
  return invoke<StorageCleanupReceipt>("clear_application_log_storage");
}

export function deleteApplicationSessionStorage(
  request: DeleteStorageSessionsRequest,
): Promise<StorageCleanupReceipt> {
  return invoke<StorageCleanupReceipt>("delete_application_session_storage", { request });
}
