import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("instalarea Google Fonts este o singură mutație text plus binar în ProjectWorkspace", () => {
  const command = source("../src-tauri/src/commands/fonts.rs");
  const fontKernel = source("../src-tauri/src/fonts/mod.rs");

  assert.match(command, /spawn_blocking/);
  assert.match(command, /workspace\.require_identity\(&identity\)/);
  assert.match(command, /stage_project_bundle_changes/);
  assert.match(command, /WorkspaceBinaryRestoreChange/);
  assert.match(command, /WorkspaceResourceMutation/);
  assert.match(command, /upsert_managed_font_face_block/);
  assert.match(command, /license_text/);
  assert.match(fontKernel, /raw\.githubusercontent\.com\/google\/fonts\/main/);
  assert.match(fontKernel, /LICENTA\.txt/);
  assert.match(fontKernel, /pana-studio-font:\{\}:start/);
  assert.doesNotMatch(command, /fs::(?:write|copy|rename)/);
});

test("catalogul de fonturi diferențiază fișierul instalat de înregistrarea CSS", () => {
  const workspace = source("../src/lib/components/creation/DesignSystemWorkspace.svelte");
  const fontKernel = source("../src-tauri/src/fonts/mod.rs");

  assert.match(fontKernel, /pub struct FontCssRegistration/);
  assert.match(fontKernel, /annotate_font_registrations/);
  assert.match(workspace, /t\("design-font-unregistered"\)/);
  assert.match(workspace, /registration\.managed/);
  assert.match(workspace, /t\("design-font-face-declarations"\)/);
});

test("interfața Fonturi caută catalogul Rust și selectează explicit variantele", () => {
  const workspace = source("../src/lib/components/creation/DesignSystemWorkspace.svelte");
  const io = source("../src/lib/project/io.ts");
  const fontKernel = source("../src-tauri/src/fonts/mod.rs");

  assert.match(workspace, /searchGoogleFonts/);
  assert.match(workspace, /t\("design-google-catalog"\)/);
  assert.match(workspace, /selectedGoogleFont\.weights/);
  assert.match(workspace, /t\("design-installed-styles"\)/);
  assert.match(workspace, /t\("design-full-variable-range"\)/);
  assert.match(workspace, /t\("design-advanced-axes"\)/);
  assert.match(workspace, /t\("design-character-optimization"\)/);
  assert.match(workspace, /file\.textOptimized/);
  assert.match(fontKernel, /normalized_google_font_axes/);
  assert.match(fontKernel, /normalize_google_character_set/);
  assert.match(fontKernel, /pub text_optimized: bool/);
  assert.match(fontKernel, /percent_encode_query_value/);
  assert.match(io, /"search_google_fonts"/);
  assert.match(io, /"download_google_font_family"[\s\S]*identity/);
});

test("importul local este multiplu, planificat și aplicat prin aceeași autoritate Rust", () => {
  const command = source("../src-tauri/src/commands/fonts.rs");
  const localImport = source("../src-tauri/src/fonts/local_import.rs");
  const workspace = source("../src/lib/components/creation/DesignSystemWorkspace.svelte");
  const io = source("../src/lib/project/io.ts");

  assert.match(localImport, /FontData<'_>/);
  assert.match(localImport, /NameTable::TYPOGRAPHIC_FAMILY_NAME/);
  assert.match(localImport, /tag::OS_2/);
  assert.match(localImport, /tag::FVAR/);
  assert.match(localImport, /symlink/);
  assert.match(localImport, /static\/fonturi\/\{family_slug\}/);
  assert.match(command, /plan_local_font_import/);
  assert.match(command, /apply_local_font_import/);
  assert.match(command, /expected_plan_token/);
  assert.match(command, /stage_project_bundle_changes/);
  assert.match(io, /multiple:\s*true/);
  assert.match(io, /"plan_local_font_import"/);
  assert.match(io, /"apply_local_font_import"/);
  assert.match(workspace, /t\("design-from-computer"\)/);
  assert.match(workspace, /t\("design-confirm-import"\)/);
});

test("rolurile semantice sunt citite și mutate în Rust, separat de instalarea fontului", () => {
  const command = source("../src-tauri/src/commands/fonts.rs");
  const roles = source("../src-tauri/src/fonts/roles.rs");
  const workspace = source("../src/lib/components/creation/DesignSystemWorkspace.svelte");

  assert.match(roles, /FontRoleId::Text/);
  assert.match(roles, /"font-primary"/);
  assert.match(roles, /"font-display"/);
  assert.match(roles, /"font-ui"/);
  assert.match(roles, /"font-mono"/);
  assert.match(roles, /update_variable_in_source/);
  assert.match(command, /assign_font_role/);
  assert.match(command, /stage_project_bundle_changes/);
  assert.match(command, /font_install\.local/);
  assert.match(command, /font_role\.assign/);
  assert.match(workspace, /t\("design-semantic-use"\)/);
  assert.match(workspace, /t\("design-use-family-for"\)/);
});

test("font-display și preload sunt livrate per fișier prin ProjectWorkspace, nu reconstruite în frontend", () => {
  const command = source("../src-tauri/src/commands/fonts.rs");
  const delivery = source("../src-tauri/src/fonts/delivery.rs");
  const workspace = source("../src/lib/components/creation/DesignSystemWorkspace.svelte");
  const typography = source("../src/lib/components/inspector/sections/TypographySection.svelte");
  const io = source("../src/lib/project/io.ts");

  assert.match(command, /set_font_display/);
  assert.match(command, /set_font_preload/);
  assert.match(command, /font_delivery\.display/);
  assert.match(command, /font_delivery\.preload/);
  assert.match(command, /stage_project_bundle_changes/);
  assert.match(delivery, /pana-studio-font-preload:start/);
  assert.match(delivery, /font-display/);
  assert.match(delivery, /preload_budget_exceeded/);
  assert.match(workspace, /t\("design-font-display-policy"\)/);
  assert.match(workspace, /t\("design-preload-active"\)/);
  assert.match(typography, /fontFamilies\.map/);
  assert.match(typography, /insertValue:\s*quoteFontFamily/);
  assert.match(io, /"set_font_display"/);
  assert.match(io, /"set_font_preload"/);
});

test("eliminarea fonturilor locale este planificată, confirmată și aplicată atomic în Rust", () => {
  const command = source("../src-tauri/src/commands/fonts.rs");
  const fontKernel = source("../src-tauri/src/fonts/mod.rs");
  const workspace = source("../src/lib/components/creation/DesignSystemWorkspace.svelte");
  const io = source("../src/lib/project/io.ts");

  assert.match(command, /plan_font_family_removal/);
  assert.match(command, /remove_font_family/);
  assert.match(command, /expected_plan_token/);
  assert.match(command, /blocked_reasons/);
  assert.match(command, /project_workspace\.font_remove\.family/);
  assert.match(command, /WorkspaceResourceDelete/);
  assert.match(command, /WorkspaceBinaryRestoreChange/);
  assert.match(command, /stage_project_bundle_changes/);
  assert.match(fontKernel, /remove_managed_font_face_block/);
  assert.match(workspace, /t\("design-controlled-removal"\)/);
  assert.match(workspace, /t\("design-confirm-removal"\)/);
  assert.match(io, /"plan_font_family_removal"/);
  assert.match(io, /"remove_font_family"/);
  assert.doesNotMatch(command, /fs::remove_(?:file|dir)/);
});
