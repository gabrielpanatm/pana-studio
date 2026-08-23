import type { ApplicationShellState } from "$lib/application/shell-state.svelte";
import type { MotionWorkspaceState } from "$lib/motion/workspace.svelte";
import type { ProjectDocumentWorkspaceState } from "$lib/project/document-workspace.svelte";
import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { PreviewWorkspaceState } from "$lib/preview/workspace-state.svelte";
import { invalidatePreviewRefreshLease } from "$lib/state/preview-controller";
import type { VersionPreviewReceipt } from "$lib/versioning/contracts";
import { stopVersionPreview } from "$lib/versioning/io";
import type { VersionPreviewState } from "$lib/versioning/preview-state.svelte";
import { t } from "$lib/i18n/runtime.svelte";

export type VersionPreviewServiceDependencies = {
  project: ProjectSessionState;
  documents: ProjectDocumentWorkspaceState;
  preview: PreviewWorkspaceState;
  motion: MotionWorkspaceState;
  shell: ApplicationShellState;
  state: VersionPreviewState;
  flushDrafts: () => Promise<void>;
  projectLatestPreview: () => Promise<unknown>;
};

/** Owns entry into and exit from immutable historical Preview sessions. */
export class VersionPreviewService {
  private readonly dependencies: VersionPreviewServiceDependencies;

  constructor(dependencies: VersionPreviewServiceDependencies) {
    this.dependencies = dependencies;
  }

  async show(receipt: VersionPreviewReceipt) {
    const { project, documents, preview, motion, shell, state } = this.dependencies;
    if (
      receipt.projectRoot !== project.root
      || receipt.sessionId !== project.runtimeSessionId
    ) throw new Error(t("workbench-version-preview-session-stale"));

    await this.dependencies.flushDrafts();
    invalidatePreviewRefreshLease(preview.commands().session);
    preview.interactiveEnabled = false;
    motion.previewMode = "design";
    documents.templateActive = false;
    state.active = receipt;
    shell.centerView = "preview";
    preview.src = receipt.previewUrl;
    preview.documentMarkup = null;
  }

  async returnToLive() {
    const { project, shell, state } = this.dependencies;
    if (!state.active) return;
    await stopVersionPreview({
      expectedProjectRoot: project.root,
      expectedSessionId: project.runtimeSessionId,
    });
    state.active = null;
    shell.centerView = "preview";
    await this.dependencies.projectLatestPreview();
  }
}
