import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(`../${relativePath}`, import.meta.url), "utf8");
}

test("UI-ul Versiuni expune remote, progres, preview și integrare explicită", () => {
  const panel = source("src/lib/components/VersionsPanel.svelte");
  const io = source("src/lib/project/io.ts");
  const types = source("src/lib/types.ts");

  for (const command of [
    "configure_version_remote",
    "remove_version_remote",
    "fetch_version_remote",
    "push_version_branch",
    "cancel_version_network_operation",
    "read_version_integration_plan",
    "integrate_version_target",
    "switch_version_branch",
    "read_version_integration_recovery",
    "resolve_version_integration_recovery",
  ]) {
    assert.match(io, new RegExp(`"${command}"`), command);
  }
  assert.match(types, /"diverged"/);
  assert.match(types, /"conflict_resolution_required"/);
  assert.match(types, /"integration"/);
  assert.match(panel, /Pană Studio nu rulează <code>git pull<\/code>/);
  assert.match(panel, /Preview patch din țintă/);
  assert.match(panel, /Commit-uri care intră din țintă/);
  assert.match(panel, /pana-versioning-network-progress/);
  assert.match(panel, /Fast-forward/);
  assert.match(panel, /Merge explicit/);
});

test("backendul remote folosește refspec-uri explicite și nu oferă force/pull", () => {
  const remote = source("src-tauri/src/versioning/remote.rs");
  const git = source("src-tauri/src/versioning/git.rs");
  const commands = source("src-tauri/src/commands/versioning.rs");

  assert.match(remote, /\+refs\/heads\/\*:refs\/remotes\/\{remote\}\/\*/);
  assert.match(remote, /refs\/heads\/\{local_branch\}:refs\/heads\/\{remote_branch\}/);
  assert.match(remote, /OsString::from\("--no-tags"\)/);
  assert.match(remote, /OsString::from\("--atomic"\)/);
  assert.doesNotMatch(remote, /run_network\([^)]*\["pull"/s);
  assert.doesNotMatch(remote, /--force|--force-with-lease/);
  assert.match(git, /GIT_CONFIG_KEY_0/);
  assert.match(git, /credential\.helper/);
  assert.match(git, /GIT_TERMINAL_PROMPT/);
  assert.match(git, /NETWORK_TIMEOUT/);
  assert.match(commands, /VersionNetworkOperationStatus::Cancelled/);
});

test("integrarea păstrează marker durabil, CAS și commit merge cu doi părinți", () => {
  const integration = source("src-tauri/src/versioning/integration.rs");
  const commands = source("src-tauri/src/commands/versioning.rs");

  assert.match(integration, /refs\/pana-studio\/integrations/);
  assert.match(integration, /"commit-tree"[\s\S]*"-p"[\s\S]*"-p"/);
  assert.match(integration, /"update-ref"/);
  assert.match(integration, /VersionIntegrationKind::MergeConflict/);
  assert.match(integration, /promote_conflict_resolution/);
  assert.match(integration, /abort_integration_metadata/);
  assert.match(commands, /publish_integration_tree/);
  assert.match(commands, /ProjectWorkspace/);
  assert.match(commands, /VersionIntegrationRecoveryState::ManualReview/);
});
