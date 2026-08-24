import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("contractul public deploy este generic, tipizat și nu mai expune intrarea Bunny-only", () => {
  const model = source("../src-tauri/src/deploy/model.rs");
  const engine = source("../src-tauri/src/deploy/engine.rs");
  const commands = source("../src-tauri/src/commands/deploy.rs");
  const registry = source("../src-tauri/src/tauri_command_registry.rs");
  const io = source("../src/lib/deploy/io.ts");

  for (const provider of ["Bunny", "Ftp", "Sftp", "S3", "CloudflarePages"]) {
    assert.match(model, new RegExp(`DeployTargetProvider[\\s\\S]*${provider}\\(`));
    assert.match(engine, new RegExp(`DeployTargetProvider::${provider}\\(`));
  }
  for (const command of [
    "read_deploy_configuration",
    "save_deploy_settings",
    "save_deploy_credential",
    "delete_deploy_credential",
    "test_deploy_connection",
    "plan_deploy",
    "execute_deploy",
  ]) {
    assert.match(commands, new RegExp(command));
    assert.match(registry, new RegExp(command));
    assert.match(io, new RegExp(`"${command}"`));
  }
  assert.doesNotMatch(registry, /deploy_to_bunny/);
  assert.doesNotMatch(io, /deploy_to_bunny|deployToBunny/);
});

test("workspace-ul Publicare oferă ținte, test, plan, progres și receipt fără setările Zola", () => {
  const pane = source("../src/lib/components/publish/PublishOperationsPane.svelte");
  const targets = source("../src/lib/components/deploy/DeployTargetsPanel.svelte");

  assert.match(pane, /<DeployTargetsPanel/);
  assert.doesNotMatch(pane, /readProjectConfiguration|saveProjectConfiguration|ZolaProjectSettings/);
  for (const provider of ["bunny", "s3", "sftp", "ftp", "cloudflare_pages"]) {
    assert.match(targets, new RegExp(`value: "${provider}"|provider === "${provider}"`));
  }
  assert.match(targets, /testDeployConnection\(/);
  assert.match(targets, /planDeploy\(/);
  assert.match(targets, /executeDeploy\(/);
  assert.match(targets, /listen<DeployProgressEvent>\("deploy-progress"/);
  assert.match(targets, /DeployReceipt/);
  assert.match(targets, /expectedSettingsRevision: plan\.settingsRevision/);
  assert.match(targets, /expectedPlanToken: plan\.planToken/);
  assert.doesNotMatch(pane, /readProjectEnv|saveProjectEnv|BUNNY_ENV_KEYS/);
});

test("credentialele sunt referențiate public și materialul secret rămâne backend-only", () => {
  const credentials = source("../src-tauri/src/deploy/credentials.rs");
  const model = source("../src-tauri/src/deploy/model.rs");
  const types = source("../src/lib/deploy/contracts.ts");

  assert.match(model, /pub credential_env_prefix: String/);
  assert.match(credentials, /enum StoredDeployCredential/);
  assert.doesNotMatch(credentials, /pub\(crate\) enum StoredDeployCredential[\s\S]*derive\([^)]*Debug/);
  assert.match(credentials, /DeployCredentialStatus/);
  assert.match(types, /type DeployCredentialStatus = \{[\s\S]*configured: boolean/);
  assert.doesNotMatch(types.match(/type DeployCredentialStatus = \{[\s\S]*?\n\};/)?.[0] ?? "", /password|apiToken|privateKey|secretAccessKey/);
});

test("sincronizarea filesystem publică manifestul după mutații, iar Pages creează versiuni", () => {
  const manifest = source("../src-tauri/src/deploy/remote_manifest.rs");
  const pages = source("../src-tauri/src/deploy/cloudflare_pages.rs");
  const retry = source("../src-tauri/src/deploy/retry.rs");

  assert.match(manifest, /for file in &previous\.files/);
  assert.match(manifest, /kind: DeployActionKind::Delete/);
  assert.match(manifest, /REMOTE_MANIFEST_FILE_NAME/);
  assert.match(pages, /create_deployment/);
  assert.match(pages, /deployment_id/);
  assert.match(pages, /deployment_url/);
  assert.match(retry, /content-addressed writes/);
  assert.match(retry, /Deployment[\s\S]*deliberately excluded/);
});

test("modul mirror este explicit, sigur implicit și separă ștergerile neadministrate", () => {
  const model = source("../src-tauri/src/deploy/model.rs");
  const manifest = source("../src-tauri/src/deploy/remote_manifest.rs");
  const targets = source("../src/lib/components/deploy/DeployTargetsPanel.svelte");

  assert.match(model, /enum DeployCleanupPolicy[\s\S]*ManagedOnly[\s\S]*MirrorDestination/);
  assert.match(model, /impl Default|derive\([^)]*Default/);
  assert.match(manifest, /remote_inventory/);
  assert.match(manifest, /DeployDeleteOrigin::Unmanaged/);
  assert.match(targets, /Elimină din destinație fișierele care nu există în build/);
  assert.match(targets, /mirror_destination/);
  assert.match(targets, /deletedUnmanagedFiles/);
});
