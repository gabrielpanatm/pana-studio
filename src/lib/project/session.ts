import type { CenterView, ProjectFile, ProjectScan } from "$lib/types";
import { currentContentSection, projectStatusFor, scannedCacheKey, slugifyPageTitle } from "$lib/project/files";
import {
  isRenderedPreviewPageFile,
  isTemplateProjectFile,
  preferredCenterViewForProjectFile,
} from "$lib/project/workflow";

export type ProjectOpenPlan = {
  projectStatus: string;
  targetCssFile: string | null;
  fileToOpen: ProjectFile | null;
};

export function planOpenedProject(project: ProjectScan): ProjectOpenPlan {
  return {
    projectStatus: projectStatusFor(project),
    targetCssFile: project.files.find((file) => (file.kind === "CSS" || file.kind === "SCSS") && file.role === "style")?.relativePath ?? null,
    fileToOpen: null,
  };
}

export function preservePreviewBaseUrl(project: ProjectScan, previousProject: ProjectScan | null): ProjectScan {
  const sameProject = previousProject?.root === project.root;
  const preserved = {
    ...project,
    kernelSessionId: project.kernelSessionId
      ?? (sameProject ? previousProject?.kernelSessionId : undefined),
    acceptedDiskGeneration: project.acceptedDiskGeneration
      ?? (sameProject ? previousProject?.acceptedDiskGeneration : undefined),
    acceptedDiskManifest: project.acceptedDiskManifest
      ?? (sameProject ? previousProject?.acceptedDiskManifest : undefined),
  };
  if (!previousProject?.previewBaseUrl) return preserved;
  return { ...preserved, previewBaseUrl: previousProject.previewBaseUrl };
}

export function selectProjectFileAfterScan(project: ProjectScan, preferredRelativePath: string | null): ProjectFile | null {
  const preferredFile = preferredRelativePath
    ? (project.files.find((file) => file.relativePath === preferredRelativePath) ?? null)
    : null;
  const firstPreviewPage = project.files.find((file) => file.role === "page" && file.previewPath) ?? null;
  const fallbackFile = project.files[0] ?? null;

  return preferredFile ?? firstPreviewPage ?? fallbackFile;
}

export type ContentPagePlan =
  | {
      ok: true;
      section: string;
      slug: string;
      title: string;
      creatingStatus: string;
    }
  | {
      ok: false;
      status: string;
    };

export function planContentPageCreation(rawTitle: string, activeScannedPath: string | null): ContentPagePlan {
  const title = rawTitle.trim();

  if (!title) {
    return {
      ok: false,
      status: "Titlul paginii nu poate fi gol.",
    };
  }

  const slug = slugifyPageTitle(title);

  if (!slug) {
    return {
      ok: false,
      status: "Nu am putut genera un slug valid din titlul dat.",
    };
  }

  return {
    ok: true,
    section: currentContentSection(activeScannedPath),
    slug,
    title,
    creatingStatus: `Se creeaza pagina ${slug}.md...`,
  };
}

export type ScannedProjectFileLoadPlan = {
  cacheKey: string;
  centerView: CenterView;
  isPreviewPage: boolean;
  isTemplateFile: boolean;
  isMarkdownPage: boolean;
  shouldResetHistoryAfterLoad: boolean;
};

export function planScannedProjectFileLoad(file: ProjectFile): ScannedProjectFileLoadPlan {
  const isPreviewPage = isRenderedPreviewPageFile(file);
  const isTemplateFile = isTemplateProjectFile(file);
  const isMarkdownPage = file.role === "page" && file.kind === "MD";

  return {
    cacheKey: scannedCacheKey(file),
    centerView: preferredCenterViewForProjectFile(file),
    isPreviewPage,
    isTemplateFile,
    isMarkdownPage,
    shouldResetHistoryAfterLoad: isPreviewPage || file.kind === "HTML" || isMarkdownPage,
  };
}
