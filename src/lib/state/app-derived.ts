import { canElementAcceptChildren, htmlVoidTags } from "$lib/html/mutations";
import {
  detectSourceLanguage,
  isZolaTemplatePath,
  projectRelativeZolaPath,
  zolaRelativePath,
} from "$lib/project/files";
import {
  canPreviewCurrentSource as canPreviewCurrentSourceForWorkflow,
} from "$lib/project/workflow";
import { deriveGlobalDirtyState } from "$lib/session/dirty-state";
import type { GlobalDirtyState } from "$lib/session/dirty-state";
import type {
  HtmlPendingArea,
  InspectorPendingArea,
} from "$lib/canvas/contracts";
import type {
  ProjectFile,
  ProjectScan,
} from "$lib/project/lifecycle-contract";
import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";
import type { SourceGraph } from "$lib/source-graph/graph-contract";
import type { SourceEditTarget } from "$lib/source-graph/contracts";
import type { EditorSelectionSessionController } from "$lib/state/editor-selection-session.svelte";
import { t } from "$lib/i18n/runtime.svelte";

export type AppDerivedSource = {
  activeScannedPath: string | null;
  activePreviewPath: string;
  scannedProject: ProjectScan | null;
  templateWorkbenchActive: boolean;
  templateWorkbenchTarget: string | null;
  sourceGraph: SourceGraph | null;
  currentSourcePath: string;
  sourceLanguage: ReturnType<typeof detectSourceLanguage>;
  currentSourceRelativePath: string;
  currentHtmlRelativePath: string;
  activeTemplateFile: ProjectFile | null;
  activeRenderedPreviewPageFile: ProjectFile | null;
  editorSelection: Pick<
    EditorSelectionSessionController,
    "inspectorSummary" | "navigationSnapshot" | "selectionSnapshot"
  >;
  projectWorkspaceSnapshot: ProjectWorkspaceSnapshot | null;
  htmlPending: Record<HtmlPendingArea, boolean>;
  inspectorPending: Record<InspectorPendingArea, boolean>;
  globalDirtyState: GlobalDirtyState;
  selectedSourceEditTarget: SourceEditTarget | null;
  selectedSemanticSourceLocation: boolean;
  isActivePreviewHtmlSource: boolean;
  resolveSourceEditTargetForSourceId: (
    sourceId: string | null | undefined,
  ) => SourceEditTarget | null;
};

type DerivedInput<Key extends keyof AppDerivedSource> = Pick<AppDerivedSource, Key>;

export function deriveActiveTemplateFile(
  app: DerivedInput<"scannedProject" | "activeScannedPath">,
) {
  return app.scannedProject?.files.find(
    (file) => file.relativePath === app.activeScannedPath && file.role === "template",
  ) ?? null;
}

export function deriveActiveRenderedPreviewPageFile(
  app: DerivedInput<"scannedProject" | "activePreviewPath">,
) {
  return app.scannedProject?.previewBaseUrl
    ? (app.scannedProject.files.find(
        (file) => file.relativePath === app.activePreviewPath && file.role === "page" && Boolean(file.previewPath),
      ) ?? null)
    : null;
}

export function deriveActiveRenderedTemplatePath(
  app: DerivedInput<
    | "templateWorkbenchActive"
    | "templateWorkbenchTarget"
    | "activePreviewPath"
    | "sourceGraph"
    | "activeScannedPath"
  >,
) {
  if (app.templateWorkbenchActive && app.templateWorkbenchTarget) {
    return normalizedProjectPath(app.templateWorkbenchTarget);
  }

  const activePreviewPath = normalizedProjectPath(app.activePreviewPath);
  const page = app.sourceGraph?.pages.find(
    (candidate) => normalizedProjectPath(candidate.file) === activePreviewPath,
  ) ?? null;
  const template = app.sourceGraph?.templates.find(
    (candidate) => candidate.nodeId === page?.templateNodeId,
  ) ?? null;
  if (template) return normalizedProjectPath(template.file);

  if (!activePreviewPath) {
    const activeSource = normalizedProjectPath(app.activeScannedPath);
    return activeSource && isZolaTemplatePath(activeSource) ? activeSource : null;
  }

  return null;
}

export function deriveCanEditHtml(
  app: DerivedInput<
    "isActivePreviewHtmlSource" | "selectedSourceEditTarget" | "selectedSemanticSourceLocation"
  >,
) {
  return app.isActivePreviewHtmlSource
    || Boolean(app.selectedSourceEditTarget)
    || app.selectedSemanticSourceLocation;
}

export function deriveAppDirtyState(
  app: DerivedInput<"projectWorkspaceSnapshot" | "htmlPending" | "inspectorPending">,
) {
  return deriveGlobalDirtyState({
    workspaceDirty: app.projectWorkspaceSnapshot?.dirty ?? false,
    htmlPending: app.htmlPending,
    inspectorPending: app.inspectorPending,
  });
}

export function deriveCanAddChildToSelectedElement(
  app: DerivedInput<"editorSelection">,
) {
  const summary = app.editorSelection.inspectorSummary;
  return Boolean(
    summary?.state === "resolved"
    && summary.tag
    && canElementAcceptChildren(summary.tag, htmlVoidTags),
  );
}

export function deriveCanPreviewCurrentSource(
  app: DerivedInput<"activeScannedPath" | "sourceLanguage" | "activeTemplateFile">,
) {
  return canPreviewCurrentSourceForWorkflow({
    activeScannedPath: app.activeScannedPath,
    sourceLanguage: app.sourceLanguage,
    hasActiveTemplateFile: app.activeTemplateFile !== null,
  });
}

export function deriveHtmlSourceMutationBlockedReason(app: DerivedInput<"activeScannedPath">) {
  if (app.activeScannedPath?.endsWith(".md")) {
    return t("workbench-html-mutation-markdown-blocked");
  }
  if (app.activeScannedPath && isZolaTemplatePath(zolaRelativePath(app.activeScannedPath))) {
    return t("workbench-html-mutation-tera-blocked");
  }
  return t("workbench-html-mutation-preview-required");
}

function normalizedProjectPath(path: string | null | undefined) {
  if (!path || path === "about:blank") return "";
  return projectRelativeZolaPath(path)
    .replaceAll("\\", "/")
    .replace(/\/+/g, "/")
    .replace(/^\.\//, "");
}
