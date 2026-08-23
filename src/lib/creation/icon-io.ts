import type {
  IconCatalogPage,
  IconCatalogSearchInput,
  IconCatalogSummary,
} from "$lib/creation/contracts";
import { invoke } from "@tauri-apps/api/core";

export function readIconCatalog(): Promise<IconCatalogSummary> {
  return invoke<IconCatalogSummary>("read_icon_catalog");
}

export function searchIconCatalog(input: IconCatalogSearchInput): Promise<IconCatalogPage> {
  return invoke<IconCatalogPage>("search_icon_catalog", { input });
}
