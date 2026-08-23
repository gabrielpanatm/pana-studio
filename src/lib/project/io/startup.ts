import type { ProjectScan } from "$lib/project/lifecycle-contract";
import type {
  StartupCreationApplyRequest,
  StartupCreationCatalog,
  StartupCreationPlan,
  StartupCreationPlanRequest,
  StartupCreationReceipt,
  StartupFlowSnapshot,
} from "$lib/project/lifecycle-contract";
import { invoke } from "@tauri-apps/api/core";
import { homeDir } from "@tauri-apps/api/path";
import {
  open as openDialog,
} from "@tauri-apps/plugin-dialog";
import { t } from "$lib/i18n/runtime.svelte";

export async function chooseProjectFolder(): Promise<string | null> {
  // The project chooser deliberately starts from the current account home on
  // every invocation. No previous project path is persisted or reused.
  const defaultPath = await homeDir().catch(() => undefined);
  const selected = await openDialog({
    directory: true,
    defaultPath,
    multiple: false,
    title: t("io-dialog-open-project"),
  });
  if (!selected || Array.isArray(selected)) return null;
  return selected;
}

export function readStartupFlow(): Promise<StartupFlowSnapshot> {
  return invoke<StartupFlowSnapshot>("read_startup_flow");
}

export function inspectStartupFolder(path: string): Promise<StartupFlowSnapshot> {
  return invoke<StartupFlowSnapshot>("inspect_startup_folder", { path });
}

export function readStartupCreationCatalog(
  expectedSnapshotToken: string,
): Promise<StartupCreationCatalog> {
  return invoke<StartupCreationCatalog>("read_startup_creation_catalog", {
    expectedSnapshotToken,
  });
}

export function planStartupCreation(
  request: StartupCreationPlanRequest,
): Promise<StartupCreationPlan> {
  return invoke<StartupCreationPlan>("plan_startup_creation", { request });
}

export function applyStartupCreation(
  request: StartupCreationApplyRequest,
): Promise<StartupCreationReceipt> {
  return invoke<StartupCreationReceipt>("apply_startup_creation", { request });
}

export function scanProject(path: string): Promise<ProjectScan> {
  return invoke<ProjectScan>("scan_project", { path });
}
