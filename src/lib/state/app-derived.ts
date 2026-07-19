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
import type { AppState } from "$lib/state/app.svelte";
import type { ProjectFile } from "$lib/types";

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
  return app.resolveSourceEditTargetForSourceId(app.selectedElement?.sourceId);
}

export function deriveSelectedTemplateSourceNode(app: AppState) {
  return app.selectedTemplateSourceId
    ? (app.sourceGraph?.nodes.find((node) => node.id === app.selectedTemplateSourceId) ?? null)
    : null;
}

export function deriveSelectedSessionSourceLocation(app: AppState) {
  return Boolean(app.selectedElement?.sessionId && app.selectedElement?.sourceLocation);
}

export function deriveCanEditHtml(app: AppState) {
  return app.isActivePreviewHtmlSource || Boolean(app.selectedSourceEditTarget) || app.selectedSessionSourceLocation;
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
  return Boolean(app.selectedElement && canElementAcceptChildren(app.selectedElement.tag, htmlVoidTags));
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
    return "Pagina activa vine din Markdown Zola. Editarea HTML directa pentru content-ul randat nu este disponibila inca.";
  }
  if (app.activeScannedPath && isZolaTemplatePath(zolaRelativePath(app.activeScannedPath))) {
    return "Template-urile Tera sunt doar pentru preview si inspectie acum. Editarea structurala directa vine mai tarziu.";
  }
  return "Comuta pe preview-ul unei pagini HTML editabile sau pe sursa HTML activa.";
}

function normalizedProjectPath(path: string | null | undefined) {
  if (!path || path === "about:blank" || path.startsWith("Template Workbench:")) return "";
  return projectRelativeZolaPath(path)
    .replaceAll("\\", "/")
    .replace(/\/+/g, "/")
    .replace(/^\.\//, "");
}

export function deriveCanUndoMoodBoard(app: AppState) {
  return app.moodBoardPast.length > 0;
}

export function deriveCanRedoMoodBoard(app: AppState) {
  return app.moodBoardFuture.length > 0;
}

export function deriveActiveTerminalTab(app: AppState) {
  return app.terminalTabs.find((tab) => tab.id === app.activeTerminalTabId) ?? app.terminalTabs[0] ?? null;
}
