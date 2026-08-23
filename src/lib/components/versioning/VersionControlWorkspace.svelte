<script lang="ts">
  import VersionsPanel from "$lib/components/VersionsPanel.svelte";
  import type { ProjectWorkspaceMutationService } from "$lib/session/workspace-mutation-service";
  import type { GlobalStatusState } from "$lib/status/state.svelte";
  import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";
  import type { VersionPreviewReceipt } from "$lib/versioning/contracts";
  import { errorMessage } from "$lib/util";
  import { t } from "$lib/i18n/runtime.svelte";

  let {
    globalStatus,
    workspaceMutations,
    activeScannedPath,
    activeVersionPreview,
    showVersionPreview,
    returnToLivePreview,
  }: {
    globalStatus: GlobalStatusState;
    workspaceMutations: ProjectWorkspaceMutationService;
    activeScannedPath: string | null;
    activeVersionPreview: VersionPreviewReceipt | null;
    showVersionPreview: (receipt: VersionPreviewReceipt) => Promise<void>;
    returnToLivePreview: () => Promise<void>;
  } = $props();

  async function reconcilePublishedGitWorkspace(
    workspace: ProjectWorkspaceSnapshot | null,
    changedPaths: string[],
  ) {
    if (!workspace) return;
    if (
      workspace.projectRoot !== workspaceMutations.snapshot?.projectRoot
      || workspace.runtimeSessionId !== workspaceMutations.snapshot?.runtimeSessionId
    ) return;
    if (!workspaceMutations.publishSnapshot(workspace)) return;
    const warnings: string[] = [];
    try {
      const derived = await workspaceMutations.reconcile({
        expectedProjectRoot: workspace.projectRoot,
        expectedSessionId: workspace.runtimeSessionId,
        expectedWorkspaceRevision: workspace.revision,
        topologyChanged: true,
        preferredRelativePath: activeScannedPath,
        refreshSourceGraph: true,
        refreshScss: true,
      });
      warnings.push(...derived.warnings);
    } catch (error) {
      warnings.push(errorMessage(error));
    }
    try {
      await workspaceMutations.projectPreview({
        reason: "workspace-mutation",
        minimumWorkspaceRevision: workspace.revision,
        requestedPaths: changedPaths,
        force: true,
      });
    } catch (error) {
      warnings.push(`Preview: ${errorMessage(error)}`);
    }
    if (warnings.length > 0) {
      globalStatus.escalate({
        id: "versioning.derived-projection",
        level: "warning",
        title: t("versions-projection-resync-title"),
        message: [...new Set(warnings)].join(" "),
      });
    } else {
      globalStatus.clear("versioning.derived-projection");
    }
  }
</script>

<VersionsPanel
  projectRoot={workspaceMutations.snapshot?.projectRoot ?? ""}
  sessionId={workspaceMutations.snapshot?.runtimeSessionId ?? ""}
  workspace={workspaceMutations.snapshot}
  onStatusUpdate={(text, kind) => globalStatus.set(text, kind)}
  activePreviewCommitOid={activeVersionPreview?.commitOid ?? null}
  showPreview={showVersionPreview}
  {returnToLivePreview}
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
