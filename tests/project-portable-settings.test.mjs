import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";
import {
  createDefaultZolaSettings,
  textFieldsFromZolaSettings,
  zolaSettingsWithTextFields,
} from "$lib/project/deploy-settings";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("setările portabile au documente Rust tipizate și revizia rămâne runtime-only", () => {
  const projectSettings = source("../src-tauri/src/commands/config/project_settings.rs");
  const deploySettings = source("../src-tauri/src/deploy/settings.rs");
  const bootstrap = source("../src-tauri/src/commands/project/contracts.rs");
  const lifecycle = source("../src-tauri/src/project/lifecycle.rs");

  assert.match(projectSettings, /\.panastudio\/settings\.toml/);
  assert.match(projectSettings, /deny_unknown_fields/);
  assert.match(deploySettings, /\.panastudio\/deploy\.toml/);
  assert.match(deploySettings, /deny_unknown_fields/);
  assert.doesNotMatch(deploySettings.match(/struct DeploySettingsDocument[\s\S]*?\n\}/)?.[0] ?? "", /revision/);
  assert.match(bootstrap, /project_settings[\s\S]*deploy_settings/);
  assert.match(lifecycle, /PROJECT_OPEN_BOOTSTRAP_SCHEMA_VERSION: u32 = 5/);
});

test("credentialele au o singură autoritate .env și nu sunt proiectate în frontend", () => {
  const credentials = source("../src-tauri/src/deploy/credentials.rs");
  const envStore = source("../src-tauri/src/kernel/project_env_store.rs");
  const commands = source("../src-tauri/src/commands/deploy.rs");
  const types = source("../src/lib/deploy/contracts.ts");

  assert.match(credentials, /ProjectEnvStore::read_namespace/);
  assert.doesNotMatch(credentials, /app_home|deploy-secrets|serde_json::to_vec/);
  assert.match(envStore, /const PROJECT_ENV_PATH: &str = "\.env"/);
  assert.match(envStore, /PANA_DEPLOY_/);
  assert.match(envStore, /ls-files[\s\S]*check-ignore/);
  assert.match(commands, /ProjectEnvStore::write_namespace/);
  assert.doesNotMatch(commands, /read_project_env|write_project_env|delete_project_env/);

  const status = types.match(/export type DeployCredentialStatus = \{[\s\S]*?\n\};/)?.[0] ?? "";
  assert.match(status, /configured: boolean/);
  assert.match(status, /missingFields: string\[\]/);
  assert.doesNotMatch(status, /password|apiToken|privateKey|secretAccessKey|storageKey/);
});

test(".env este exclus din workspace, preview și scanarea publică", () => {
  const workspace = source("../src-tauri/src/kernel/project_workspace/workspace.rs");
  const classifier = source("../src-tauri/src/kernel/file_buffer_store/classify.rs");
  const preview = source("../src-tauri/src/preview/preprocess/workspace.rs");
  const scan = source("../src-tauri/src/project/scan.rs");
  const mcp = source("../src-tauri/src/commands/mcp.rs");
  const wal = source("../src-tauri/src/kernel/write_authority/recovery/model.rs");

  assert.match(workspace, /credentialele sunt administrate exclusiv de ProjectEnvStore/);
  assert.match(classifier, /Some\("\.env"\)[\s\S]*return None/);
  assert.match(preview, /SENSITIVE_SOURCE_FILES: &\[&str\] = &\["\.env"\]/);
  assert.match(scan, /relative_path == "\.env"[\s\S]*continue/);
  assert.match(mcp, /let snapshot = workspace\.documents\.snapshot\(\)/);
  assert.doesNotMatch(mcp, /read_to_string|fs::read/);

  const atomicEvidence = wal.match(/struct WalAtomicFileEvidence \{[\s\S]*?\n\}/)?.[0] ?? "";
  assert.match(atomicEvidence, /new_content_hash/);
  assert.doesNotMatch(atomicEvidence, /payload|contents|bytes/);
});

test("configurația de publicare este salvată printr-o singură mutație workspace", () => {
  const config = source("../src-tauri/src/commands/config.rs");
  const pane = source("../src/lib/components/project-settings/ProjectSettingsWorkspace.svelte");
  const projectIo = source("../src/lib/project/io/configuration.ts");
  const deployIo = source("../src/lib/deploy/io.ts");

  assert.match(config, /save_project_configuration[\s\S]*execute_config_workspace_mutation_at_revision/);
  assert.match(config, /settings\.toml\+zola\.toml\+templates/);
  assert.match(pane, /saveProjectConfiguration\(/);
  assert.match(pane, /registerEditFlushHandler\(/);
  assert.match(pane, /if \(savePromise\) return savePromise/);
  assert.match(pane, /fieldset class="configuration-grid" disabled=\{saving\}/);
  assert.doesNotMatch(pane, /Promise\.all/);
  assert.match(projectIo, /PROJECT_OPEN_BOOTSTRAP_SCHEMA_VERSION/);
  assert.match(deployIo, /validateDeployConfigurationSnapshot/);
});

test("globurile skip_content_templating au round-trip frontend câte unul pe linie", () => {
  const settings = {
    ...createDefaultZolaSettings(),
    skipContentTemplating: ["documentatie/**", "literal/*.md"],
  };
  const fields = textFieldsFromZolaSettings(settings);
  assert.equal(fields.skipContentTemplatingText, "documentatie/**\nliteral/*.md");
  assert.deepEqual(zolaSettingsWithTextFields(settings, {
    ...fields,
    skipContentTemplatingText: " documentatie/** \n\n literal/*.md \n",
  }).skipContentTemplating, ["documentatie/**", "literal/*.md"]);
});

test("Application Home rămâne global/tehnic și căile project-config legacy sunt eliminate", () => {
  const appHome = source("../src-tauri/src/app_home.rs");
  const appConfig = source("../src-tauri/src/commands/config/app_config.rs");
  const startup = source("../src-tauri/src/project/startup.rs");

  assert.doesNotMatch(appHome, /projects_dir|deploy-secrets|project_app_config/);
  assert.match(appHome, /APP_HOME_SCHEMA_VERSION: u32 = 3/);
  assert.match(appConfig, /app_config_path/);
  assert.doesNotMatch(appConfig, /project_root|cachebust|deploy/);
  assert.match(startup, /\.panastudio\/settings\.toml/);
  assert.match(startup, /\.panastudio\/deploy\.toml/);

  for (const removed of [
    "../src-tauri/src/commands/config/env.rs",
    "../src-tauri/src/deploy/env.rs",
    "../src-tauri/permissions/autogenerated/read_project_app_config.toml",
    "../src-tauri/permissions/autogenerated/save_project_app_config.toml",
    "../src-tauri/permissions/autogenerated/read_project_env.toml",
    "../src-tauri/permissions/autogenerated/save_project_env.toml",
  ]) {
    assert.equal(existsSync(new URL(removed, import.meta.url)), false, removed);
  }
});
