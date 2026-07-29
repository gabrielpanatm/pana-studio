import { canElementAcceptChildren, htmlVoidTags } from "$lib/html/mutations";
import { parseHtmlSourceNodes } from "$lib/html/parser";
import {
  currentHtmlRelativePath,
  currentSourceRelativePath,
  detectSourceLanguage,
  isZolaTemplatePath,
  projectRelativeZolaPath,
  zolaRelativePath,
} from "$lib/project/files";
import {
  canPreviewCurrentSource as canPreviewCurrentSourceForWorkflow,
} from "$lib/project/workflow";
import { deriveGlobalDirtyState } from "$lib/session/dirty-state";
import { workbenchSourceStatusFromSelection } from "$lib/source-provenance";
import type { AppState } from "$lib/state/app.svelte";
import type { ProjectFile } from "$lib/types";
import { t } from "$lib/i18n/runtime.svelte";

export function deriveCurrentSourcePath(app: AppState) {
  return app.activeScannedPath ?? "";
}

export function deriveSourceLanguage(app: AppState) {
  return detectSourceLanguage(app.currentSourcePath);
}

export function deriveCurrentSourceCacheKey(app: AppState) {
  return app.activeScannedPath ? `scanned:${app.activeScannedPath}` : "no-project";
}

export function deriveCurrentHtmlRelativePath(app: AppState) {
  return currentHtmlRelativePath(app.activePreviewPath);
}

export function deriveCurrentSourceRelativePath(app: AppState) {
  return currentSourceRelativePath(app.activeScannedPath);
}

export function deriveHtmlSourceNodes(app: AppState) {
  return app.sourceLanguage === "html" ? parseHtmlSourceNodes(app.source, htmlVoidTags) : [];
}

export function deriveScannedFilesByRole(app: AppState, role: ProjectFile["role"]) {
  return app.scannedProject?.files.filter((file) => file.kind !== "DIR" && file.role === role) ?? [];
}

export function deriveCurrentProjectPath(app: AppState) {
  return app.scannedProject?.root ?? "";
}

export function deriveActiveTemplateFile(app: AppState) {
  return app.scannedProject?.files.find(
    (file) => file.relativePath === app.activeScannedPath && file.role === "template",
  ) ?? null;
}

export function deriveActiveRenderedPreviewPageFile(app: AppState) {
  return app.scannedProject?.previewBaseUrl
    ? (app.scannedProject.files.find(
        (file) => file.relativePath === app.activePreviewPath && file.role === "page" && Boolean(file.previewPath),
      ) ?? null)
    : null;
}

export function deriveActiveRenderedTemplatePath(app: AppState) {
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

export function deriveIsActivePreviewHtmlSource(app: AppState) {
  return app.sourceLanguage === "html" && app.currentSourceRelativePath === app.currentHtmlRelativePath;
}

export function deriveIsActiveRenderedPreviewPage(app: AppState) {
  return Boolean(app.activeRenderedPreviewPageFile);
}

export function deriveSelectedSourceEditTarget(app: AppState) {
  return app.resolveSourceEditTargetForSourceId(
    app.selectionSnapshot?.anchor?.sourceNodeId,
  );
}

export function deriveSelectedTemplateSourceNode(app: AppState) {
  const sourceNodeId = app.selectionSnapshot?.subject?.kind === "teraBoundary"
    ? app.selectionSnapshot.anchor?.sourceNodeId
    : null;
  return sourceNodeId
    ? (app.sourceGraph?.nodes.find((node) => node.id === sourceNodeId) ?? null)
    : null;
}

export function deriveSelectedEditorNavigationNode(app: AppState) {
  const editorNodeId = app.selectionSnapshot?.projections.layers.editorNodeId ?? null;
  return editorNodeId
    ? (app.editorNavigationSnapshot?.nodes.find(
        (node) => node.id === editorNodeId,
      ) ?? null)
    : null;
}

export function deriveSelectedSemanticSourceLocation(app: AppState) {
  const selection = app.selectionSnapshot;
  return Boolean(
    selection
    && selection.resolution === "resolved"
    && selection.anchor?.file
    && selection.anchor.range,
  );
}

export function deriveWorkbenchSourceStatus(app: AppState) {
  return workbenchSourceStatusFromSelection(app.selectionSnapshot);
}

export function deriveCanEditHtml(app: AppState) {
  return app.isActivePreviewHtmlSource
    || Boolean(app.selectedSourceEditTarget)
    || app.selectedSemanticSourceLocation;
}

export function deriveAppDirtyState(app: AppState) {
  return deriveGlobalDirtyState({
    workspaceDirty: app.projectWorkspaceSnapshot?.dirty ?? false,
    htmlPending: app.htmlPending,
    inspectorPending: app.inspectorPending,
  });
}

export function deriveSessionHasPending(app: AppState) {
  return app.globalDirtyState.dirty;
}

export function deriveCanAddChildToSelectedElement(app: AppState) {
  const summary = app.inspectorSelectionSummary;
  return Boolean(
    summary?.state === "resolved"
    && summary.tag
    && canElementAcceptChildren(summary.tag, htmlVoidTags),
  );
}

export function deriveCanPreviewCurrentSource(app: AppState) {
  return canPreviewCurrentSourceForWorkflow({
    activeScannedPath: app.activeScannedPath,
    sourceLanguage: app.sourceLanguage,
    hasActiveTemplateFile: app.activeTemplateFile !== null,
  });
}

export function deriveHtmlSourceMutationBlockedReason(app: AppState) {
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

export function deriveActiveTerminalTab(app: AppState) {
  return app.terminalTabs.find((tab) => tab.id === app.activeTerminalTabId) ?? app.terminalTabs[0] ?? null;
}
