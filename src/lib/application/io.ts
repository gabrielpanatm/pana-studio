import { invoke } from "@tauri-apps/api/core";
import type {
  AppHomeSnapshot,
  ApplicationSettingsPatch,
  ApplicationSettingsSnapshot,
} from "$lib/application/contracts";

export function readApplicationSettings(): Promise<ApplicationSettingsSnapshot> {
  return invoke<ApplicationSettingsSnapshot>("read_application_settings");
}

export function saveApplicationSettings(
  expectedRevision: number,
  patch: ApplicationSettingsPatch,
): Promise<ApplicationSettingsSnapshot> {
  return invoke<ApplicationSettingsSnapshot>("save_application_settings", {
    settings: {
      expectedRevision,
      patch,
    },
  });
}

export function readAppHome(): Promise<AppHomeSnapshot> {
  return invoke<AppHomeSnapshot>("read_app_home");
}
