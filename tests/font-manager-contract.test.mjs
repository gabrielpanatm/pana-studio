import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

function fontWorkspace() {
  return [
    "../src/lib/components/creation/design-system/FontManagerWorkspace.svelte",
    "../src/lib/fonts/manager-state.svelte.ts",
    "../src/lib/components/creation/design-system/font-manager/controller.svelte.ts",
    "../src/lib/components/creation/design-system/font-manager/FontInstaller.svelte",
    "../src/lib/components/creation/design-system/font-manager/FontDetail.svelte",
  ].map(source).join("\n");
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

test("FontFaceGraph leagă familia CSS de sursa exactă și este unica autoritate", () => {
  const workspace = fontWorkspace();
  const fontKernel = source("../src-tauri/src/fonts/mod.rs");
  const graph = source("../src-tauri/src/fonts/graph.rs");
  const command = source("../src-tauri/src/commands/fonts.rs");
  const types = source("../src/lib/fonts/contracts.ts");
  const io = source("../src/lib/fonts/io.ts");

  assert.match(fontKernel, /pub struct FontFaceGraph/);
  assert.match(fontKernel, /pub struct FontFaceSource/);
  assert.match(fontKernel, /pub struct FontCssRegistration/);
  assert.match(graph, /classify_font_url/);
  assert.match(graph, /resolved_file/);
  assert.match(graph, /content_hash/);
  assert.match(command, /font_face_graph_for_workspace/);
  assert.match(command, /pub graph: FontFaceGraph/);
  assert.match(types, /type FontFaceGraph/);
  assert.match(types, /export type FontManagerSnapshot/);
  assert.match(workspace, /manager\.snapshot\?\.graph/);
  assert.doesNotMatch(fontKernel, /FontInventory|FontFamilyKey|annotate_font_registrations/);
  assert.doesNotMatch(types, /FontInventory|LocalFontFamily/);
  assert.doesNotMatch(io, /get_font_inventory/);
  assert.doesNotMatch(workspace, /manager\.inventory|fontInventory/);
  assert.match(workspace, /t\("design-font-unregistered"\)/);
  assert.match(workspace, /registration\.managed/);
  assert.match(workspace, /t\("design-font-face-declarations"\)/);
});

test("interfața Fonturi caută catalogul Rust și selectează explicit variantele", () => {
  const workspace = fontWorkspace();
  const io = source("../src/lib/fonts/io.ts");
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
  const fontMetadata = source("../src-tauri/src/fonts/metadata.rs");
  const workspace = fontWorkspace();
  const io = source("../src/lib/fonts/io.ts");

  assert.match(localImport, /parse_font_metadata/);
  assert.match(fontMetadata, /FontData<'_>/);
  assert.match(fontMetadata, /NameTable::TYPOGRAPHIC_FAMILY_NAME/);
  assert.match(fontMetadata, /tag::OS_2/);
  assert.match(fontMetadata, /tag::FVAR/);
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

test("biblioteca inclusă este offline, validată la build și instalată prin contractul unic", () => {
  const command = source("../src-tauri/src/commands/fonts.rs");
  const bundled = source("../src-tauri/src/fonts/bundled.rs");
  const buildValidation = source("../src-tauri/build/font_library.rs");
  const workspace = fontWorkspace();
  const io = source("../src/lib/fonts/io.ts");
  const registry = source("../src-tauri/src/tauri_command_registry.rs");
  const tauriConfig = source("../src-tauri/tauri.conf.json");
  const catalog = JSON.parse(source("../src-tauri/resources/font-library/catalog.json"));

  assert.equal(catalog.families.length, 36);
  for (const family of catalog.families) {
    assert.match(family.id, /^[a-z0-9]+(?:-[a-z0-9]+)*$/);
    assert.ok(family.files.length >= 2);
    assert.ok(family.files.every((file) => file.file.endsWith(".woff2")));
    assert.ok(family.files.every((file) => file.sourceUrl.startsWith("https://")));
    const styles = new Set(family.files.map((file) => file.file.includes("-italic-") ? "italic" : "normal"));
    for (const style of styles) {
      const subsets = new Set(family.files
        .filter((file) => (file.file.includes("-italic-") ? "italic" : "normal") === style)
        .map((file) => file.subset));
      assert.deepEqual([...subsets].sort(), ["latin", "latin-ext"]);
    }
  }
  assert.ok(catalog.families.flatMap((family) => family.files)
    .reduce((total, file) => total + file.sizeBytes, 0) <= 5_500 * 1024);

  assert.match(buildValidation, /parse_font_metadata/);
  assert.match(buildValidation, /ROMANIAN_GLYPHS/);
  assert.match(buildValidation, /sha256_hex/);
  assert.match(buildValidation, /collect_relative_files/);
  assert.match(buildValidation, /include_bytes!/);
  assert.match(bundled, /static\/fonturi\/\{\}/);
  assert.match(bundled, /\/fonturi\/\{\}\/\{\}/);
  assert.match(command, /commit_prepared_project_font_install/);
  assert.match(command, /project_workspace\.font_install\.bundled/);
  assert.match(command, /get_bundled_font_preview/);
  assert.match(io, /"get_bundled_font_catalog"/);
  assert.match(io, /"install_bundled_font_family"/);
  assert.match(registry, /get_bundled_font_catalog/);
  assert.match(registry, /install_bundled_font_family/);
  assert.match(workspace, /type FontCreateSource = "google" \| "bundled" \| "local"/);
  assert.match(workspace, /loadBundledFontPreview/);
  assert.match(workspace, /bundledFontCategory/);
  assert.doesNotMatch(tauriConfig, /font-library/);
});

test("rolurile semantice sunt citite și mutate în Rust, separat de instalarea fontului", () => {
  const command = source("../src-tauri/src/commands/fonts.rs");
  const roles = source("../src-tauri/src/fonts/roles.rs");
  const workspace = fontWorkspace();

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
  const workspace = fontWorkspace();
  const typography = source("../src/lib/components/inspector/sections/TypographySection.svelte");
  const io = source("../src/lib/fonts/io.ts");

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
  const workspace = fontWorkspace();
  const io = source("../src/lib/fonts/io.ts");

  assert.match(command, /plan_font_family_removal/);
  assert.match(command, /remove_font_family/);
  assert.match(command, /expected_plan_token/);
  assert.match(command, /family_id/);
  assert.match(workspace, /fontRemovalPlan\.familyId/);
  assert.doesNotMatch(io, /planFontFamilyRemoval\([\s\S]*directory: string/);
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
