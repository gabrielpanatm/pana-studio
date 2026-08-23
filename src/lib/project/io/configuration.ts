import { validateDeploySettings } from "$lib/deploy/io";
import {
  PROJECT_SETTINGS_SCHEMA_VERSION,
  type ProjectConfigurationSnapshot,
  type ZolaProjectSettings,
} from "$lib/project/lifecycle-contract";
import {
  PROJECT_OPEN_BOOTSTRAP_SCHEMA_VERSION,
  type ProjectOpenBootstrapReceipt,
} from "$lib/project/lifecycle-contract";
import { invoke } from "@tauri-apps/api/core";
import { schemaMismatch } from "$lib/contracts/io-schema";

export function validateProjectOpenBootstrapReceipt(receipt: ProjectOpenBootstrapReceipt) {
  if (receipt.schemaVersion !== PROJECT_OPEN_BOOTSTRAP_SCHEMA_VERSION) {
    throw schemaMismatch(
      "ProjectOpenBootstrap",
      receipt.schemaVersion,
      PROJECT_OPEN_BOOTSTRAP_SCHEMA_VERSION,
    );
  }
  validateProjectSettingsSnapshot(receipt.projectSettings);
  validateDeploySettings(receipt.deploySettings);
}

function validateProjectSettingsSnapshot(snapshot: ProjectConfigurationSnapshot["projectSettings"]) {
  if (snapshot.schemaVersion !== PROJECT_SETTINGS_SCHEMA_VERSION) {
    throw schemaMismatch("ProjectSettings", snapshot.schemaVersion, PROJECT_SETTINGS_SCHEMA_VERSION);
  }
}

function validateProjectConfigurationSnapshot(snapshot: ProjectConfigurationSnapshot) {
  validateProjectSettingsSnapshot(snapshot.projectSettings);
}

export async function readProjectConfiguration(): Promise<ProjectConfigurationSnapshot> {
  const snapshot = await invoke<ProjectConfigurationSnapshot>("read_project_configuration");
  validateProjectConfigurationSnapshot(snapshot);
  return snapshot;
}

export async function saveProjectConfiguration(config: {
  projectSettings: {
    expectedWorkspaceRevision: number;
    cachebustAssets: boolean;
  };
  zolaSettings: ZolaProjectSettings;
}): Promise<ProjectConfigurationSnapshot> {
  const snapshot = await invoke<ProjectConfigurationSnapshot>("save_project_configuration", { config });
  validateProjectConfigurationSnapshot(snapshot);
  return snapshot;
}
