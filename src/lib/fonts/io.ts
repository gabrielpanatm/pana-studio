import type {
  BundledFontCatalogFamily,
  BundledFontInstallReceipt,
  BundledFontPreview,
  FontDeliveryMutationReceipt,
  FontFamilyRemovalPlan,
  FontFamilyRemovalReceipt,
  FontManagerSnapshot,
  FontPreviewAsset,
  FontRoleAssignmentReceipt,
  FontRoleId,
  GoogleFontAxis,
  GoogleFontCatalogFamily,
  GoogleFontInstallReceipt,
  LocalFontImportPlan,
  LocalFontImportReceipt,
} from "$lib/fonts/contracts";
import type { ProjectWorkspaceIdentity } from "$lib/project/workspace-contract";
import { invoke } from "@tauri-apps/api/core";
import {
  open as openDialog,
} from "@tauri-apps/plugin-dialog";
import { t } from "$lib/i18n/runtime.svelte";

export async function chooseFontFiles(): Promise<string[]> {
  const selected = await openDialog({
    directory: false,
    multiple: true,
    title: t("io-dialog-import-fonts"),
    filters: [{
      name: t("io-dialog-web-fonts"),
      extensions: ["woff2", "woff", "ttf", "otf"],
    }],
  });
  if (!selected) return [];
  return Array.isArray(selected) ? selected : [selected];
}

export function getFontManager(
  identity: ProjectWorkspaceIdentity,
): Promise<FontManagerSnapshot> {
  return invoke<FontManagerSnapshot>("get_font_manager", { identity });
}

export function getFontPreviewAsset(
  file: string,
  identity: ProjectWorkspaceIdentity,
): Promise<FontPreviewAsset> {
  return invoke<FontPreviewAsset>("get_font_preview_asset", { file, identity });
}

export function getBundledFontCatalog(): Promise<BundledFontCatalogFamily[]> {
  return invoke<BundledFontCatalogFamily[]>("get_bundled_font_catalog");
}

export function getBundledFontPreview(
  familyId: string,
  style: "normal" | "italic" = "normal",
): Promise<BundledFontPreview> {
  return invoke<BundledFontPreview>("get_bundled_font_preview", { familyId, style });
}

export function installBundledFontFamily(
  familyId: string,
  identity: ProjectWorkspaceIdentity,
): Promise<BundledFontInstallReceipt> {
  return invoke<BundledFontInstallReceipt>("install_bundled_font_family", {
    familyId,
    identity,
  });
}

export function assignFontRole(
  roleId: FontRoleId,
  familyId: string,
  identity: ProjectWorkspaceIdentity,
): Promise<FontRoleAssignmentReceipt> {
  return invoke<FontRoleAssignmentReceipt>("assign_font_role", {
    roleId,
    familyId,
    identity,
  });
}

export function setFontDisplay(
  familyId: string,
  display: "auto" | "block" | "swap" | "fallback" | "optional",
  identity: ProjectWorkspaceIdentity,
): Promise<FontDeliveryMutationReceipt> {
  return invoke<FontDeliveryMutationReceipt>("set_font_display", {
    familyId,
    display,
    identity,
  });
}

export function setFontPreload(
  file: string,
  enabled: boolean,
  identity: ProjectWorkspaceIdentity,
): Promise<FontDeliveryMutationReceipt> {
  return invoke<FontDeliveryMutationReceipt>("set_font_preload", {
    file,
    enabled,
    identity,
  });
}

export function planFontFamilyRemoval(
  familyId: string,
  identity: ProjectWorkspaceIdentity,
): Promise<FontFamilyRemovalPlan> {
  return invoke<FontFamilyRemovalPlan>("plan_font_family_removal", {
    familyId,
    identity,
  });
}

export function removeFontFamily(
  familyId: string,
  expectedPlanToken: string,
  identity: ProjectWorkspaceIdentity,
): Promise<FontFamilyRemovalReceipt> {
  return invoke<FontFamilyRemovalReceipt>("remove_font_family", {
    familyId,
    expectedPlanToken,
    identity,
  });
}

export function downloadGoogleFontFamily(
  family: string,
  weights: number[],
  styles: string[],
  variable: boolean,
  axes: GoogleFontAxis[],
  characterSet: string | null,
  identity: ProjectWorkspaceIdentity,
): Promise<GoogleFontInstallReceipt> {
  return invoke<GoogleFontInstallReceipt>("download_google_font_family", {
    family,
    weights,
    styles,
    variable,
    axes,
    characterSet,
    identity,
  });
}

export function searchGoogleFonts(query: string, limit = 40, offset = 0): Promise<GoogleFontCatalogFamily[]> {
  return invoke<GoogleFontCatalogFamily[]>("search_google_fonts", { query, limit, offset });
}

export function planLocalFontImport(
  sourcePaths: string[],
  identity: ProjectWorkspaceIdentity,
): Promise<LocalFontImportPlan> {
  return invoke<LocalFontImportPlan>("plan_local_font_import", { sourcePaths, identity });
}

export function applyLocalFontImport(
  sourcePaths: string[],
  expectedPlanToken: string,
  identity: ProjectWorkspaceIdentity,
): Promise<LocalFontImportReceipt> {
  return invoke<LocalFontImportReceipt>("apply_local_font_import", {
    sourcePaths,
    expectedPlanToken,
    identity,
  });
}
