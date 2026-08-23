import { createDiskState, type DiskState } from "$lib/session/disk-state";
import {
  resetFileBufferDraftSyncState,
} from "$lib/session/file-buffer-draft-sync";
import { resetPageJsDraftSyncState } from "$lib/session/page-js-draft-sync";
import { resetProjectWorkspacePreviewCoordinator } from "$lib/kernel/project-workspace-preview-coordinator";
import { t } from "$lib/i18n/runtime.svelte";
import type { ScssVariable } from "$lib/css/contracts";
import type { WorkspaceDerivedProjectionStatus } from "$lib/session/workspace-mutation-coordinator";
import type { GlobalStatusKind } from "$lib/status/global-status";
import type {
  HtmlPendingArea,
  InspectorPendingArea,
} from "$lib/canvas/contracts";
import type { FileExplorerSnapshot } from "$lib/project/file-explorer-contract";
import type { TemplateWorkbenchPlan } from "$lib/project/template-workbench-contract";
import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";
import type { SourceGraph } from "$lib/source-graph/graph-contract";
import type { SourceEditLocation } from "$lib/source-graph/contracts";
import type { WorkbenchSnapshot } from "$lib/workbench/contracts";
import type { PublishWorkspaceState } from "$lib/deploy/publish-state.svelte";

type ProjectSourceResetHost = {
  source: string;
  sourceCache: Record<string, string>;
  activeScannedPath: string | null;
  sourceGraph: SourceGraph | null;
  sourceGraphProjectionStatus: WorkspaceDerivedProjectionStatus;
  sourceGraphWorkspaceRevision: number | null;
  scssVariables: ScssVariable[];
  targetCssFile: string;
};

type ProjectPreviewResetHost = {
  previewSrc: string;
  activePreviewPath: string;
  browserPreviewRoute: string;
  previewDocumentMarkup: string | null;
  previewWorkspaceRevision: string | null;
  activeVersionPreview: unknown | null;
  clearPreviewSelection: (options?: { clearCanvasOverlay?: boolean }) => void;
  resetControlledPreviewState: () => void;
  resetPageSections: () => void;
};

type ProjectTemplateResetHost = {
  templateWorkbenchPlan: TemplateWorkbenchPlan | null;
  templateWorkbenchPreferredPagePath: string | null;
  templateWorkbenchPreferredRoute: string | null;
  templateWorkbenchReuseToken: string | null;
  templateWorkbenchActive: boolean;
  templateWorkbenchTarget: string | null;
  templateWorkbenchReturnPreviewPath: string | null;
  templateWorkbenchRequestSerial: number;
};

type ProjectEditorResetHost = {
  overrideRules: Record<string, unknown>;
  variableOverrides: Record<string, string>;
  htmlPending: Record<HtmlPendingArea, boolean>;
  inspectorPending: Record<InspectorPendingArea, boolean>;
  pendingTag: string | null;
  pendingTagOriginal: string | null;
  pendingTagSourceLocation: SourceEditLocation | null;
  tagStatus: string;
  editorSelection: { reset: () => void };
  resetInspectorPendingSources: () => void;
  cancelPendingHtmlMutations: () => void;
};

type ProjectWorkspaceResetHost = {
  projectWorkspaceSnapshot: ProjectWorkspaceSnapshot | null;
  workbenchSnapshot: WorkbenchSnapshot | null;
  fileExplorerSnapshot: FileExplorerSnapshot | null;
  fileExplorerLoading: boolean;
  fileExplorerError: string;
  publishWorkspace: Pick<PublishWorkspaceState, "cachebustAssets" | "invalidate">;
  diskState: DiskState;
  kernelProjectSessionId: string;
  refreshToken: number;
  setSessionProjectRoot: (projectRoot?: string) => void;
  resetExternalDiskState: () => void;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
};

export type ProjectSessionResetOptions = {
  preserveExternalReconcileBarrier?: boolean;
  invalidateHistory?: boolean;
};

function createEmptyInspectorPending(): Record<InspectorPendingArea, boolean> {
  return { html: false, css: false, js: false };
}

function createEmptyHtmlPending(): Record<HtmlPendingArea, boolean> {
  return {
    tag: false,
    attributes: false,
    text: false,
    image: false,
    classes: false,
    structure: false,
  };
}

export function resetProjectScopedState(
  host: ProjectSourceResetHost
    & ProjectPreviewResetHost
    & ProjectTemplateResetHost
    & ProjectEditorResetHost
    & ProjectWorkspaceResetHost,
  options: ProjectSessionResetOptions = {},
) {
  resetProjectWorkspacePreviewCoordinator();
  resetFileBufferDraftSyncState();
  resetPageJsDraftSyncState();
  if (options.invalidateHistory) {
    host.cancelPendingHtmlMutations();
    host.refreshToken += 1;
  }
  if (!options.preserveExternalReconcileBarrier) host.resetExternalDiskState();
  host.resetControlledPreviewState();
  host.resetPageSections();
  host.sourceGraph = null;
  host.sourceGraphProjectionStatus = "deferred";
  host.sourceGraphWorkspaceRevision = null;
  host.sourceCache = {};
  host.source = "";
  host.activeScannedPath = null;
  host.previewSrc = "about:blank";
  host.activePreviewPath = "about:blank";
  host.browserPreviewRoute = "/";
  host.previewDocumentMarkup = null;
  host.previewWorkspaceRevision = null;
  host.projectWorkspaceSnapshot = null;
  host.workbenchSnapshot = null;
  host.fileExplorerSnapshot = null;
  host.fileExplorerLoading = false;
  host.fileExplorerError = "";
  host.scssVariables = [];
  host.targetCssFile = "styles.css";
  host.templateWorkbenchPlan = null;
  host.templateWorkbenchPreferredPagePath = null;
  host.templateWorkbenchPreferredRoute = null;
  host.templateWorkbenchReuseToken = null;
  host.templateWorkbenchActive = false;
  host.templateWorkbenchTarget = null;
  host.templateWorkbenchReturnPreviewPath = null;
  host.templateWorkbenchRequestSerial += 1;
  host.clearPreviewSelection({ clearCanvasOverlay: true });
  host.editorSelection.reset();
  host.overrideRules = {};
  host.variableOverrides = {};
  host.htmlPending = createEmptyHtmlPending();
  host.resetInspectorPendingSources();
  host.inspectorPending = createEmptyInspectorPending();
  host.pendingTag = null;
  host.pendingTagOriginal = null;
  host.pendingTagSourceLocation = null;
  host.tagStatus = "";
  host.setGlobalStatus(t("project-controller-no-session-save"), "idle");
  host.publishWorkspace.cachebustAssets = false;
  host.publishWorkspace.invalidate();
  host.diskState = createDiskState();
  host.activeVersionPreview = null;
  host.setSessionProjectRoot();
  host.kernelProjectSessionId = "";
}
