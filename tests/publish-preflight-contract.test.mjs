import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

function functionBody(text, startMarker, endMarker) {
  const start = text.indexOf(startMarker);
  const end = text.indexOf(endMarker, start + startMarker.length);
  assert.notEqual(start, -1, `lipsește ${startMarker}`);
  assert.notEqual(end, -1, `lipsește limita ${endMarker}`);
  return text.slice(start, end);
}

test("PublishPreflightReceipt este contract Rust versionat și leagă toate identitățile", () => {
  const contract = source("../src-tauri/src/kernel/publish_preflight.rs");
  for (const field of [
    "project_root",
    "runtime_session_id",
    "workspace_revision",
    "disk_generation",
    "workspace_dirty",
    "disk_coherent",
    "observed_disk_fingerprint",
    "project_model_revision",
    "deploy_settings_revision",
    "deploy_settings_fingerprint",
    "active_target",
    "audit_identity",
    "audit_receipt",
    "status",
    "gates",
    "preflight_token",
  ]) {
    assert.match(contract, new RegExp(`pub ${field}:`));
  }
  assert.match(contract, /PUBLISH_PREFLIGHT_SCHEMA_VERSION: u32 = 1/);
  assert.match(contract, /PUBLISH_BUILD_RECEIPT_SCHEMA_VERSION: u32 = 1/);
  assert.match(contract, /REQUIRED_PUBLISH_GATES:[\s\S]*"zola_check"[\s\S]*"deploy_credentials"/);
  assert.match(contract, /audit_policy"[\s\S]*PublishPreflightGateOutcome::Advisory/);
  assert.match(contract, /pana-publish-preflight-v1/);
  assert.match(contract, /pana-publish-build-v1/);
});

test("Preflight capturează o proiecție, rămâne read-only și nu face rețea deploy", () => {
  const commands = source("../src-tauri/src/commands/publish.rs");
  const run = functionBody(
    commands,
    "pub fn run_publish_preflight",
    "#[tauri::command]\npub fn current_publish_preflight_receipt",
  );
  assert.match(run, /capture_publish_authority_context/);
  assert.match(run, /build_project_model_for_audit_from_workspace_projection/);
  assert.match(run, /run_zola_editor_check/);
  assert.match(run, /publish_preflight_receipt_if_current/);
  assert.doesNotMatch(run, /save_project_workspace|run_zola_build|resolve_credential|test_deploy_connection|plan_deploy_with_artifact|execute_deploy_with_artifact/);
  assert.match(commands, /context\.dirty[\s\S]*AuditBuildEvidence::Skipped/);
  assert.match(commands, /observed_disk == context\.accepted_disk\.manifest/);
});

test("plan și execute refuză dovada locală înainte de credentiale sau inventar remote", () => {
  const commands = source("../src-tauri/src/commands/deploy.rs");
  const plan = functionBody(commands, "pub async fn plan_deploy", "#[tauri::command]\npub async fn execute_deploy");
  const execute = functionBody(commands, "pub async fn execute_deploy", "#[tauri::command]\npub fn cancel_publish_operation");

  for (const body of [plan, execute]) {
    const requireBuild = body.indexOf("require_current_publish_build");
    const artifact = body.indexOf("artifact_identity_matches");
    const credential = body.indexOf("resolve_credential");
    assert.ok(requireBuild >= 0 && artifact > requireBuild && credential > artifact);
  }
  const authorizePlan = execute.indexOf("require_authorized_deploy_plan_token");
  const executeCredential = execute.indexOf("resolve_credential");
  assert.ok(authorizePlan >= 0 && executeCredential > authorizePlan);
  assert.match(plan, /expected_build_token[\s\S]*expected_artifact_id/);
  assert.match(execute, /expected_preflight_token[\s\S]*expected_build_token[\s\S]*expected_artifact_id/);
  assert.match(commands, /pana-publish-deploy-plan-v1/);
});

test("frontendul consumă decizia Rust, verifică currency și propagă proof chain", () => {
  const publishController = source("../src/lib/deploy/publish-state.svelte.ts");
  const workspace = source("../src/lib/components/publish/PublishWorkspace.svelte");
  const targets = source("../src/lib/components/deploy/DeployTargetsPanel.svelte");

  assert.match(workspace, /publishWorkspace\.currentPreflight\(\)/);
  assert.match(workspace, /preflight\?\.status === "ready"/);
  assert.doesNotMatch(workspace, /sourceSaved && auditCurrent|controlledPreview\.validation === "valid"/);
  assert.match(workspace, /preflight\.gates as gate/);
  assert.match(workspace, /auditFingerprints/);
  assert.match(workspace, /revealSourceRange/);

  assert.match(publishController, /receipt\.workspaceRevision === authority\.workspace\.revision/);
  assert.match(publishController, /receipt\.diskGeneration === authority\.workspace\.diskGeneration/);
  assert.match(publishController, /build\.preflightToken === preflight\.preflightToken/);
  assert.match(publishController, /build\.deploySettingsFingerprint === preflight\.deploySettingsFingerprint/);

  assert.match(targets, /expectedBuildToken: build\.buildToken/);
  assert.match(targets, /expectedArtifactId: build\.artifactId/);
  assert.match(targets, /expectedPreflightToken: plan\.preflightToken/);
  assert.match(targets, /expectedBuildToken: plan\.buildToken/);
  assert.match(targets, /credentialKindSupportsProvider/);
  assert.match(targets, /testDeployConnection\(/);
});

test("mutațiile workspace și schimbările deploy invalidează autorizația", () => {
  const recovery = source("../src-tauri/src/kernel/project_workspace/recovery.rs");
  const deploy = source("../src-tauri/src/commands/deploy.rs");
  const frontend = source("../src/lib/components/deploy/DeployTargetsPanel.svelte");

  assert.match(recovery, /emit_project_workspace_mutated[\s\S]*clear_publish_authorization/);
  for (const command of ["save_deploy_settings", "save_deploy_credential", "delete_deploy_credential"]) {
    const index = deploy.indexOf(`pub fn ${command}`);
    assert.notEqual(index, -1);
    const nextCommand = deploy.indexOf("#[tauri::command]", index);
    assert.match(deploy.slice(index, nextCommand === -1 ? undefined : nextCommand), /invalidate_publish_authorization/);
  }
  assert.match(frontend, /invalidatePublishAuthorization\(\)/);
  assert.match(frontend, /plan\.preflightToken !== publishBuild\.preflightToken/);
  assert.match(frontend, /plan\.artifactId !== publishBuild\.artifactId/);
});
