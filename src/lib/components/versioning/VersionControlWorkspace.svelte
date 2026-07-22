<script lang="ts">
  import VersionsPanel from "$lib/components/VersionsPanel.svelte";
  import type { AppState } from "$lib/state/app.svelte";

  let { app }: { app: AppState } = $props();

  async function refreshProjectWorkspace() {
    await app.rescanCurrentProject(app.activeScannedPath, { strict: true });
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
    if (receipt.workspace) app.projectWorkspaceSnapshot = receipt.workspace;
    await refreshProjectWorkspace();
  }}
  afterRecovery={async (receipt) => {
    if (receipt.workspace) app.projectWorkspaceSnapshot = receipt.workspace;
    await refreshProjectWorkspace();
  }}
  afterIntegration={async (receipt) => {
    if (receipt.workspace) app.projectWorkspaceSnapshot = receipt.workspace;
    await refreshProjectWorkspace();
  }}
  afterIntegrationRecovery={async (receipt) => {
    if (receipt.workspace) app.projectWorkspaceSnapshot = receipt.workspace;
    await refreshProjectWorkspace();
  }}
/>
