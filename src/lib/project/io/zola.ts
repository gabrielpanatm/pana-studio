import { invoke } from "@tauri-apps/api/core";

export function zolaBuild(): Promise<string> {
  return invoke<string>("zola_build");
}

export function zolaCheck(): Promise<string> {
  return invoke<string>("zola_check");
}

export function zolaCheckWorkspace(): Promise<string> {
  return invoke<string>("zola_check_workspace");
}
