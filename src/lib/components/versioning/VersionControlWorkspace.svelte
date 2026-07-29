<script lang="ts">
  import VersionsPanel from "$lib/components/VersionsPanel.svelte";
  import { projectLatestProjectWorkspacePreview } from "$lib/kernel/project-workspace-preview-coordinator";
  import type { AppState } from "$lib/state/app.svelte";
  import type { ProjectWorkspaceSnapshot } from "$lib/types";
  import { errorMessage } from "$lib/util";
  import { t } from "$lib/i18n/runtime.svelte";

  let { app }: { app: AppState } = $props();

  async function reconcilePublishedGitWorkspace(
    workspace: ProjectWorkspaceSnapshot | null,
    changedPaths: string[],
  ) {
    if (!workspace) return;
    if (
      workspace.projectRoot !== app.sessionProjectRoot
      || workspace.runtimeSessionId !== app.kernelProjectSessionId
    ) return;
    app.projectWorkspaceSnapshot = workspace;
    const warnings: string[] = [];
    try {
      const derived = await app.reconcileWorkspaceDerivedState({
        expectedProjectRoot: workspace.projectRoot,
        expectedSessionId: workspace.runtimeSessionId,
        expectedWorkspaceRevision: workspace.revision,
        topologyChanged: true,
        preferredRelativePath: app.activeScannedPath,
        refreshSourceGraph: true,
        refreshScss: true,
      });
      warnings.push(...derived.warnings);
    } catch (error) {
      warnings.push(errorMessage(error));
    }
    try {
      await projectLatestProjectWorkspacePreview(app, {
        reason: "workspace-mutation",
        minimumWorkspaceRevision: workspace.revision,
        requestedPaths: changedPaths,
        force: true,
      });
    } catch (error) {
      warnings.push(`Preview: ${errorMessage(error)}`);
    }
    if (warnings.length > 0) {
      app.escalateGlobalStatus({
        id: "versioning.derived-projection",
        level: "warning",
        title: t("versions-projection-resync-title"),
        message: [...new Set(warnings)].join(" "),
      });
    } else {
      app.clearNotification("versioning.derived-projection");
    }
  }
</script>

<VersionsPanel
  projectRoot={app.sessionProjectRoot}
  sessionId={app.kernelProjectSessionId}
  workspace={app.projectWorkspaceSnapshot}
  onStatusUpdate={(text, kind) => app.setGlobalStatus(text, kind)}
  activePreviewCommitOid={app.activeVersionPreview?.commitOid ?? null}
  showPreview={async (receipt) => { await app.showVersionPreview(receipt); }}
  returnToLivePreview={async () => { await app.returnToLivePreview(); }}
  afterRestore={async (receipt) => {
    await reconcilePublishedGitWorkspace(receipt.workspace, receipt.changedPaths);
  }}
  afterRecovery={async (receipt) => {
    await reconcilePublishedGitWorkspace(receipt.workspace, []);
  }}
  afterIntegration={async (receipt) => {
    await reconcilePublishedGitWorkspace(receipt.workspace, receipt.changedPaths);
  }}
  afterIntegrationRecovery={async (receipt) => {
    await reconcilePublishedGitWorkspace(receipt.workspace, []);
  }}
/>
