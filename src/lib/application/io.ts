import { invoke } from "@tauri-apps/api/core";
import type {
  ApplicationSettingsSnapshot,
  ApplicationTheme,
  AppHomeSnapshot,
} from "$lib/types";

export function readApplicationSettings(): Promise<ApplicationSettingsSnapshot> {
  return invoke<ApplicationSettingsSnapshot>("read_application_settings");
}

export function saveApplicationSettings(
  expectedRevision: number,
  theme: ApplicationTheme,
  blockPropertiesHeight: number,
  blockPropertiesCollapsed: boolean,
): Promise<ApplicationSettingsSnapshot> {
  return invoke<ApplicationSettingsSnapshot>("save_application_settings", {
    settings: {
      expectedRevision,
      theme,
      blockPropertiesHeight,
      blockPropertiesCollapsed,
    },
  });
}

export function readAppHome(): Promise<AppHomeSnapshot> {
  return invoke<AppHomeSnapshot>("read_app_home");
}
