import { tick } from "svelte";
import type { ApplicationShellState } from "$lib/application/shell-state.svelte";
import type { SourceWorkspaceState } from "$lib/editor/source-workspace.svelte";
import type { ProjectDocumentWorkspaceState } from "$lib/project/document-workspace.svelte";
import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { EditFlushReason } from "$lib/session/edit-flush-registry";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import type { CenterView } from "$lib/application/contracts";
import type { WorkbenchActivity } from "$lib/workbench/contracts";
import type { WorkbenchWorkspaceState } from "$lib/workbench/workspace-state.svelte";
import { t } from "$lib/i18n/runtime.svelte";
import { errorMessage } from "$lib/util";

export type WorkbenchNavigationServiceDependencies = {
  shell: ApplicationShellState;
  workbench: WorkbenchWorkspaceState;
  project: ProjectSessionState;
  documents: ProjectDocumentWorkspaceState;
  source: Pick<SourceWorkspaceState, "requestSelectionReveal">;
  status: GlobalStatusState;
  flushDrafts: (reason: EditFlushReason) => Promise<void>;
  projectLatestPreview: () => Promise<unknown>;
};

/** Serializes Workbench surface changes and their Rust navigation projection. */
export class WorkbenchNavigationService {
  private readonly dependencies: WorkbenchNavigationServiceDependencies;

  constructor(dependencies: WorkbenchNavigationServiceDependencies) {
    this.dependencies = dependencies;
  }

  async setCenterView(view: CenterView) {
    const { shell, workbench, project, documents, source, status } = this.dependencies;
    if (view === "preview" && workbench.activeDocumentPresentation === "code_only") {
      view = "code";
    }
    const targetActivity: WorkbenchActivity = view === "kernel" ? "audit" : "editor";
    if (centerViewAlreadyCanonical({
      view,
      targetActivity,
      currentView: shell.centerView,
      activePath: documents.activeScannedPath,
      snapshot: workbench.snapshot,
      workbenchHydrated: workbench.isHydrated(project.runtimeSessionId),
    })) return true;
    if (view !== shell.centerView && shell.centerView === "preview") {
      try {
        await this.dependencies.flushDrafts("template-switch");
      } catch (error) {
        status.set(t("workbench-activity-switch-blocked", {
          message: errorMessage(error),
        }), "error");
        return false;
      }
    }
    const enteringCode = view === "code" && shell.centerView !== "code";
    const enteringPreview = view === "preview" && shell.centerView !== "preview";
    if (enteringCode) source.requestSelectionReveal();

    if (
      workbench.isHydrated(project.runtimeSessionId)
      && workbench.snapshot
      && workbench.snapshot.activeActivity !== targetActivity
    ) {
      try {
        await workbench.apply({ kind: "set_activity", activity: targetActivity });
        status.clear("workbench.activity-sync");
      } catch (error) {
        status.escalate({
          id: "workbench.activity-sync",
          level: "warning",
          title: t("workbench-activity-switch-failed"),
          message: errorMessage(error),
        });
        return false;
      }
    }

    shell.centerView = view;
    if (
      documents.activeScannedPath
      && (view === "preview" || view === "code")
      && workbench.snapshot?.split === "none"
    ) {
      try {
        await workbench.setActiveDocumentSurface(documents.activeScannedPath, view);
        status.clear("workbench.surface-sync");
      } catch (error) {
        status.escalate({
          id: "workbench.surface-sync",
          level: "warning",
          title: t("workbench-document-surface-save-failed"),
          message: errorMessage(error),
        });
      }
    }

    if (enteringPreview && project.project) {
      const projectRoot = project.root;
      const sessionId = project.runtimeSessionId;
      const sessionEpoch = project.epoch;
      await tick();
      if (
        shell.centerView === "preview"
        && project.root === projectRoot
        && project.runtimeSessionId === sessionId
        && project.epoch === sessionEpoch
      ) {
        try {
          await this.dependencies.projectLatestPreview();
        } catch (error) {
          if (
            shell.centerView === "preview"
            && project.root === projectRoot
            && project.runtimeSessionId === sessionId
            && project.epoch === sessionEpoch
          ) {
            status.set(
              t("workbench-preview-project-failed", { message: errorMessage(error) }),
              "error",
            );
          }
        }
      }
    }
    return true;
  }
}

function centerViewAlreadyCanonical(input: {
  view: CenterView;
  targetActivity: WorkbenchActivity;
  currentView: CenterView;
  activePath: string | null;
  snapshot: WorkbenchWorkspaceState["snapshot"];
  workbenchHydrated: boolean;
}) {
  if (input.view !== input.currentView) return false;
  const snapshot = input.snapshot;
  if (
    input.workbenchHydrated
    && snapshot
    && snapshot.activeActivity !== input.targetActivity
  ) return false;
  if (
    !input.activePath
    || input.view === "kernel"
    || !snapshot
    || snapshot.split !== "none"
  ) return true;
  const group = snapshot.groups.find(
    (candidate) => candidate.groupId === snapshot.activeGroupId,
  );
  const document = group?.documents.find(
    (candidate) => candidate.documentId === group.activeDocumentId,
  );
  const expectedSurface = input.view === "code" ? "code" : "visual";
  return document?.relativePath === input.activePath
    && document.surface === expectedSurface;
}
