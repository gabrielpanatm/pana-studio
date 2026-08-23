import {
  deriveActiveRenderedPreviewPageFile,
  deriveActiveRenderedTemplatePath,
  deriveActiveTemplateFile,
} from "$lib/state/app-derived";
import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type {
  ProjectFile,
  ProjectScan,
} from "$lib/project/lifecycle-contract";
import type { TemplateWorkbenchPlan } from "$lib/project/template-workbench-contract";
import type { TemplateWorkbenchPublicationStatus } from "$lib/preview/io";
import type { SourceGraph } from "$lib/source-graph/graph-contract";

export type ProjectDocumentWorkspaceDependencies = {
  session: ProjectSessionState;
  sourceGraph: () => SourceGraph | null;
};

/** Owns the active document and Template Workbench navigation context. */
export class ProjectDocumentWorkspaceState {
  activeScannedPath = $state<string | null>(null);
  activePreviewPath = $state("about:blank");
  browserPreviewRoute = $state("/");
  templatePlan = $state<TemplateWorkbenchPlan | null>(null);
  templatePreferredPagePath = $state<string | null>(null);
  templatePreferredRoute = $state<string | null>(null);
  templateActive = $state(false);
  templateTarget = $state<string | null>(null);
  templateReturnPreviewPath = $state<string | null>(null);
  templatePublicationStatus = $state<TemplateWorkbenchPublicationStatus | null>(null);
  templateReuseToken = $state<string | null>(null);
  templateRequestSerial = 0;

  private readonly dependencies: ProjectDocumentWorkspaceDependencies;
  private categorizedProject: ProjectScan | null = null;
  private categorizedFiles = new Map<ProjectFile["role"], ProjectFile[]>();

  constructor(dependencies: ProjectDocumentWorkspaceDependencies) {
    this.dependencies = dependencies;
  }

  private filesByRole(role: ProjectFile["role"]) {
    const project = this.dependencies.session.project;
    if (project !== this.categorizedProject) {
      this.categorizedProject = project;
      this.categorizedFiles.clear();
      for (const file of project?.files ?? []) {
        if (file.kind === "DIR") continue;
        const files = this.categorizedFiles.get(file.role) ?? [];
        files.push(file);
        this.categorizedFiles.set(file.role, files);
      }
    }
    return this.categorizedFiles.get(role) ?? [];
  }

  get scannedPages() { return this.filesByRole("page"); }
  get scannedTemplates() { return this.filesByRole("template"); }
  get scannedStyles() { return this.filesByRole("style"); }
  get scannedScripts() { return this.filesByRole("script"); }
  get scannedAssets() { return this.filesByRole("asset"); }

  get currentProjectPath() {
    return this.dependencies.session.project?.root ?? "";
  }

  get activeTemplateFile() {
    return deriveActiveTemplateFile({
      scannedProject: this.dependencies.session.project,
      activeScannedPath: this.activeScannedPath,
    });
  }

  get activeRenderedPreviewPageFile() {
    return deriveActiveRenderedPreviewPageFile({
      scannedProject: this.dependencies.session.project,
      activePreviewPath: this.activePreviewPath,
    });
  }

  get activeRenderedTemplatePath() {
    return deriveActiveRenderedTemplatePath({
      templateWorkbenchActive: this.templateActive,
      templateWorkbenchTarget: this.templateTarget,
      activePreviewPath: this.activePreviewPath,
      sourceGraph: this.dependencies.sourceGraph(),
      activeScannedPath: this.activeScannedPath,
    });
  }

  get isActiveRenderedPreviewPage() {
    return Boolean(this.activeRenderedPreviewPageFile);
  }

  reset() {
    this.activeScannedPath = null;
    this.activePreviewPath = "about:blank";
    this.browserPreviewRoute = "/";
    this.templatePlan = null;
    this.templatePreferredPagePath = null;
    this.templatePreferredRoute = null;
    this.templateActive = false;
    this.templateTarget = null;
    this.templateReturnPreviewPath = null;
    this.templatePublicationStatus = null;
    this.templateReuseToken = null;
    this.templateRequestSerial += 1;
  }
}
