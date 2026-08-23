import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(`../${relativePath}`, import.meta.url), "utf8");
}

test("Git este o activitate Workbench canonică, nu un overlay local", () => {
  const types = source("src/lib/workbench/contracts.ts");
  const rustModel = source("src-tauri/src/kernel/workbench/model.rs");
  const rustSearch = source("src-tauri/src/kernel/command_center/search.rs");
  const rail = source("src/lib/components/workbench/ActivityRail.svelte");
  const center = source("src/lib/components/workspace/WorkspaceCenterArea.svelte");
  const workspace = source("src/lib/components/versioning/VersionControlWorkspace.svelte");
  const chrome = source("src/lib/components/workspace/AppChrome.svelte");
  const state = source("src/lib/components/application/ApplicationWorkspace.svelte");

  assert.match(types, /\| "versioning"/);
  assert.match(rustModel, /\bVersioning,/);
  assert.match(rustSearch, /WorkbenchActivity::Versioning/);
  assert.match(rustSearch, /"Control versiuni"/);
  assert.match(rail, /id: "versioning"/);
  assert.match(rail, /IconGitBranch/);
  assert.match(center, /retainedAuxiliarySurface === "versioning"/);
  assert.match(center, /retainedAuxiliarySurface === "versioning"[\s\S]*<VersionControlWorkspace/);
  assert.match(workspace, /<VersionsPanel/);
  assert.doesNotMatch(chrome, /<VersionsPanel/);
  assert.doesNotMatch(state, /versionsPanelOpen/);
  const panel = source("src/lib/components/VersionsPanel.svelte");
  assert.doesNotMatch(panel, /versions-backdrop|position:\s*fixed/);
  assert.match(panel, /\.versions-panel\s*\{[^}]*width:\s*100%/);
  assert.doesNotMatch(panel, /\.versions-panel\s*\{[^}]*(?:max-width|width:\s*min\(|margin:\s*0 auto|border-(?:left|right))/);
});

test("UI-ul Versiuni expune remote, progres, preview și integrare explicită", () => {
  const panel = source("src/lib/components/VersionsPanel.svelte");
  const io = source("src/lib/versioning/io.ts");
  const networkController = source(
    "src/lib/versioning/network-controller.svelte.ts",
  );
  const types = source("src/lib/versioning/contracts.ts");

  assert.doesNotMatch(panel, /from "\$lib\/versioning\/io"/);
  assert.match(panel, /NetworkController/);
  assert.match(panel, /IntegrationController/);
  assert.match(panel, /RecoveryController/);
  assert.match(panel, /SnapshotController/);
  assert.doesNotMatch(panel, /from "\$lib\/project\/io"/);

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
  assert.match(panel, /t\("versions-no-pull-hint"\)/);
  assert.match(panel, /t\("versions-target-patch-preview"\)/);
  assert.match(panel, /t\("versions-target-commits"/);
  assert.match(networkController, /pana-versioning-network-progress/);
  assert.match(panel, /t\("versions-fast-forward"\)/);
  assert.match(panel, /t\("versions-explicit-merge"\)/);
});

test("activitatea Git folosește shell-ul și tema vizuală standard a aplicației", () => {
  const panel = source("src/lib/components/VersionsPanel.svelte");
  const designSystem = source("src/routes/design-system.css");

  assert.match(panel, /aria-labelledby="version-control-title"/);
  assert.match(panel, /class="activity-workspace [^"]*versioning-workspace"/);
  assert.match(panel, /class="workspace-header panel-header"/);
  assert.match(panel, /class="eyebrow"><IconGitBranch/);
  assert.match(panel, /class="header-metrics"/);
  assert.match(panel, /class="head-reference"/);
  assert.match(panel, /class="setup-icon"/);
  assert.match(designSystem, /\.activity-workspace > :is\(\.workspace-header, \.audit-header\)\s*\{[\s\S]*padding:\s*17px 20px/);
  assert.match(designSystem, /\.activity-workspace > \.workspace-header \.header-metrics > div\s*\{[\s\S]*var\(--material-control\)/);
  assert.match(panel, /\.setup-card\s*\{[^}]*grid-template-columns:\s*auto minmax\(0,\s*1fr\) auto/);
  assert.doesNotMatch(panel, /class="title-icon"/);
});

test("backendul remote folosește refspec-uri explicite și nu oferă force/pull", () => {
  const remote = source("src-tauri/src/versioning/remote.rs");
  const git = source("src-tauri/src/versioning/git.rs");
  const networkCommands = source("src-tauri/src/commands/versioning/network.rs");

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
  assert.match(networkCommands, /VersionNetworkOperationStatus::Cancelled/);
});

test("fetch și push separă capturarea, așteptarea fără lock și publicarea revalidată", () => {
  const networkCommands = source("src-tauri/src/commands/versioning/network.rs");
  const remote = source("src-tauri/src/versioning/remote.rs");
  const git = source("src-tauri/src/versioning/git.rs");
  const networkRuntime = source(
    "src-tauri/src/versioning/network_operation.rs",
  );
  const workspaceRecovery = source(
    "src-tauri/src/kernel/project_workspace/recovery.rs",
  );
  const workspaceSave = source(
    "src-tauri/src/kernel/project_workspace/save.rs",
  );
  const projectTransition = source(
    "src-tauri/src/commands/project/transition_decisions.rs",
  );
  const networkCommand = networkCommands.slice(
    networkCommands.indexOf("async fn network_mutate_with_repository"),
    networkCommands.indexOf("fn capture_network_preflight"),
  );
  const networkCapture = networkCommands.slice(
    networkCommands.indexOf("fn capture_network_preflight"),
    networkCommands.indexOf("fn publish_network_result"),
  );

  assert.match(networkCommand, /execute_version_network_phases/);
  assert.match(networkCommand, /spawn_prepared_network/);
  assert.doesNotMatch(networkCommand, /with_mutation_preflight/);
  assert.doesNotMatch(networkCommand, /project_workspace\s*\.\s*lock/);
  assert.ok(
    networkCapture.indexOf("drop(workspace_guard)") <
      networkCapture.indexOf("captured.with_repository"),
    "ProjectWorkspace trebuie eliberat înainte de preflight-ul Git local",
  );
  assert.match(networkCommands, /fn validate_network_publication_state/);
  assert.match(networkCommands, /finish_success\(operation_lease\)/);
  assert.match(remote, /fn prepare_fetch_remote/);
  assert.match(remote, /fn prepare_push_branch/);
  assert.match(remote, /fn finalize_prepared_network/);
  assert.doesNotMatch(remote, /fn fetch_remote\s*\(/);
  assert.doesNotMatch(remote, /fn push_branch\s*\(/);
  assert.match(git, /fn spawn_network/);
  assert.match(git, /struct RunningGitCommand/);
  assert.doesNotMatch(git, /fn run_network\s*\(/);
  assert.match(networkRuntime, /require_source_mutation_allowed/);
  assert.match(networkRuntime, /require_git_mutation_allowed/);
  assert.match(networkRuntime, /require_project_transition_allowed/);
  assert.match(workspaceRecovery, /require_source_mutation_allowed/);
  assert.match(workspaceSave, /require_source_mutation_allowed/);
  assert.match(projectTransition, /require_project_transition_allowed/);
});

test("integrarea păstrează marker durabil, CAS și commit merge cu doi părinți", () => {
  const integration = source("src-tauri/src/versioning/integration.rs");
  const integrationCommands = source("src-tauri/src/commands/versioning/integration.rs");

  assert.match(integration, /refs\/pana-studio\/integrations/);
  assert.match(integration, /"commit-tree"[\s\S]*"-p"[\s\S]*"-p"/);
  assert.match(integration, /"update-ref"/);
  assert.match(integration, /VersionIntegrationKind::MergeConflict/);
  assert.match(integration, /promote_conflict_resolution/);
  assert.match(integration, /abort_integration_metadata/);
  assert.match(integrationCommands, /publish_integration_tree/);
  assert.match(integrationCommands, /ProjectWorkspace/);
  assert.match(integrationCommands, /VersionIntegrationRecoveryState::ManualReview/);
});
