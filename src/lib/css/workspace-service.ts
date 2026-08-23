import type { CssAuthoringState } from "$lib/css/authoring-state.svelte";
import type { DesignClassInventoryState } from "$lib/css/class-inventory-state.svelte";
import type { CssMutationAuthorityReceipt } from "$lib/css/mutation-contract";
import type { ScssVariable } from "$lib/css/contracts";
import type { SourceWorkspaceState } from "$lib/editor/source-workspace.svelte";
import type { ProjectAnalysisState } from "$lib/project/analysis-state.svelte";
import type { ProjectDocumentWorkspaceState } from "$lib/project/document-workspace.svelte";
import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { PreviewWorkspaceState } from "$lib/preview/workspace-state.svelte";
import type {
  PreviewStructuralSessionLease,
} from "$lib/kernel/preview-structural-lane";
import type {
  WorkspaceMutationAuthorityReceipt,
  WorkspaceMutationSettlement,
  WorkspaceMutationSettlementOptions,
} from "$lib/session/workspace-mutation-coordinator";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import type { WorkbenchWorkspaceState } from "$lib/workbench/workspace-state.svelte";
import {
  createDesignSystemClass,
  createDesignSystemVariable,
  projectCommittedInspectorCssMutation,
  renameDesignSystemClass,
  updateDesignSystemVariable,
  type InspectorCssControllerHost,
} from "$lib/state/inspector-css-controller";
import {
  applyInspectorLiveProperties,
  applyInspectorLivePropertyDrafts,
  breakpointValue,
  captureInspectorLiveCssIdentity,
  clearInspectorLiveProperties,
  injectRawCss,
  restoreLiveCssLayersToPreview,
  type InspectorCssDraft,
  type InspectorLiveCssIdentity,
  type PreviewLiveControllerHost,
} from "$lib/state/preview-live-controller";

export type CssWorkspaceServiceDependencies = {
  project: ProjectSessionState;
  documents: ProjectDocumentWorkspaceState;
  analysis: ProjectAnalysisState;
  authoring: CssAuthoringState;
  source: SourceWorkspaceState;
  workbench: WorkbenchWorkspaceState;
  preview: PreviewWorkspaceState;
  inventory: DesignClassInventoryState;
  status: GlobalStatusState;
  structural: {
    run: <T>(operation: (lease: PreviewStructuralSessionLease) => Promise<T>) => Promise<T>;
    require: (lease: PreviewStructuralSessionLease) => void;
    settle: (
      receipt: WorkspaceMutationAuthorityReceipt,
      options?: WorkspaceMutationSettlementOptions,
    ) => Promise<WorkspaceMutationSettlement>;
  };
};

/** Owns Inspector live CSS and durable Design System mutations. */
export class CssWorkspaceService {
  private readonly previewLive: PreviewLiveControllerHost;
  private readonly inspector: InspectorCssControllerHost;
  private readonly currentPreviewDevice: () => "desktop" | "tablet" | "mobile";

  constructor(dependencies: CssWorkspaceServiceDependencies) {
    const { project, documents, analysis, authoring, source, workbench, preview, inventory, status } = dependencies;
    this.currentPreviewDevice = () => workbench.previewDevice;
    this.previewLive = {
      get scssVariables() { return analysis.scssVariables; },
      get previewDevice() { return workbench.previewDevice; },
      get liveCssById() { return authoring.liveLayers; },
      set liveCssById(layers) { authoring.liveLayers = layers; },
      get inspectorLiveCssEpoch() { return authoring.liveEpoch; },
      set inspectorLiveCssEpoch(epoch) { authoring.liveEpoch = epoch; },
      get inspectorLiveCssIdentity() { return authoring.liveIdentity; },
      set inspectorLiveCssIdentity(identity) { authoring.liveIdentity = identity; },
      get sessionProjectRoot() { return project.root; },
      get kernelProjectSessionId() { return project.runtimeSessionId; },
      getPreviewDocument: () => preview.getDocument(),
      postPreviewMessage: (payload) => preview.postMessage(payload),
      markPreviewLive: (message) => preview.markLive(message),
    };
    this.inspector = {
      context: () => ({
        projectRoot: project.root,
        runtimeSessionId: project.runtimeSessionId,
        workspace: project.workspace,
        activeScannedPath: documents.activeScannedPath,
      }),
      acceptWorkspace: (workspace) => { project.workspace = workspace; },
      source,
      scssVariables: () => analysis.scssVariables,
      acceptScssVariables: (variables) => { analysis.scssVariables = variables; },
      previewLive: this.previewLive,
      runStructural: dependencies.structural.run,
      requireStructural: dependencies.structural.require,
      settleMutation: dependencies.structural.settle,
      notifyCssSourceChanged: () => source.notifyCssSourceChanged(),
      refreshDesignClassInventory: (force) => inventory.refresh(force),
      setGlobalStatus: (text, kind) => status.set(text, kind),
    };
  }

  applyLiveProperties(
    selector: string | null,
    properties: Record<string, string>,
    viewport?: "desktop" | "tablet" | "mobile",
  ) {
    return applyInspectorLiveProperties(
      this.previewLive,
      selector,
      properties,
      viewport ?? this.currentPreviewDevice(),
    );
  }

  breakpointValue(name: string, fallback: string) {
    return breakpointValue(this.previewLive, name, fallback);
  }

  applyLiveDrafts(entries: InspectorCssDraft[]) {
    return applyInspectorLivePropertyDrafts(this.previewLive, entries);
  }

  clearLiveProperties(expectedEpoch?: number) {
    let expectedIdentity: InspectorLiveCssIdentity | undefined;
    if (expectedEpoch !== undefined) {
      const captured = captureInspectorLiveCssIdentity(this.previewLive, expectedEpoch);
      if (!captured) return false;
      expectedIdentity = captured;
    }
    return clearInspectorLiveProperties(this.previewLive, expectedIdentity);
  }

  projectCommittedMutation(authority: CssMutationAuthorityReceipt, liveEpoch: number | null) {
    return projectCommittedInspectorCssMutation(this.inspector, authority, liveEpoch);
  }

  updateVariable(variable: ScssVariable, value: string) {
    return updateDesignSystemVariable(this.inspector, variable, value);
  }

  createVariable(relativePath: string, name: string, value: string) {
    return createDesignSystemVariable(this.inspector, relativePath, name, value);
  }

  createClass(name: string, relativePath: string) {
    return createDesignSystemClass(this.inspector, name, relativePath);
  }

  renameClass(oldName: string, newName: string) {
    return renameDesignSystemClass(this.inspector, oldName, newName);
  }

  injectRaw(id: string, css: string) {
    injectRawCss(this.previewLive, id, css);
  }

  restoreLiveLayers() {
    restoreLiveCssLayersToPreview(this.previewLive);
  }
}
