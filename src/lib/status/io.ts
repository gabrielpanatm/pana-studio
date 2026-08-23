import {
  GLOBAL_STATUS_SCHEMA_VERSION,
  type GlobalStatusInput,
  type GlobalStatusSnapshot,
} from "$lib/status/global-status";
import { invoke } from "@tauri-apps/api/core";

export function publishKernelGlobalStatus(
  input: GlobalStatusInput,
): Promise<GlobalStatusSnapshot> {
  return invoke<GlobalStatusSnapshot>("publish_global_status", {
    input: {
      ...input,
      schemaVersion: GLOBAL_STATUS_SCHEMA_VERSION,
    },
  });
}

export function resolveKernelGlobalStatus(
  key: string,
): Promise<GlobalStatusSnapshot> {
  return invoke<GlobalStatusSnapshot>("resolve_global_status", { key });
}

export function readKernelGlobalStatus(): Promise<GlobalStatusSnapshot> {
  return invoke<GlobalStatusSnapshot>("read_global_status");
}
