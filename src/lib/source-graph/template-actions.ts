import {
  createSiteArchiveStructure,
  createSitePageStructure,
  createSitePartialStructure,
  createSiteSingleStructure,
  includeSitePartial,
} from "$lib/project/io";
import {
  projectLatestProjectWorkspacePreview,
  type ProjectWorkspacePreviewHost,
} from "$lib/kernel/project-workspace-preview-coordinator";
import {
  previewStructuralCommandIdentity,
  requireCurrentPreviewStructuralSession,
  type PreviewStructuralSessionHost,
  type PreviewStructuralSessionLease,
} from "$lib/kernel/preview-structural-lane";
import { logicalTemplateName } from "$lib/project/files";
import type {
  SiteStructureAuthorityReceipt,
  SourceGraphNode,
  SourceGraphTemplate,
} from "$lib/types";

export type PartialPreset = "cta" | "header" | "footer" | "generic";

export type PageCreationOptions = {
  title: string;
  slug: string;
  pageTemplateName: string;
  draft: boolean;
  targetTemplate?: SourceGraphTemplate | null;
  activeTheme?: string | null;
};

export type ArchiveCreationOptions = {
  title: string;
  slug: string;
  archiveTemplateName: string;
  targetTemplate?: SourceGraphTemplate | null;
  activeTheme?: string | null;
};

export type SingleCreationOptions = {
  sectionSlug: string;
  title: string;
  slug: string;
  singleTemplateName: string;
  targetTemplate?: SourceGraphTemplate | null;
  activeTheme?: string | null;
};

export type TemplateWriteBase = {
  origin: "local" | "theme";
  themeName: string | null;
};

export type SiteStructureSessionHost = PreviewStructuralSessionHost & {
  projectCommittedSiteStructure: (
    lease: PreviewStructuralSessionLease,
    touchedFiles: string[],
    workspaceRevision: number,
    preferredRelativePath?: string | null,
  ) => Promise<void>;
};

export type SiteStructurePreviewSyncHost = PreviewStructuralSessionHost
  & ProjectWorkspacePreviewHost;

type AuthoritativeSiteStructureReceipt = {
  authority: SiteStructureAuthorityReceipt;
};

/**
 * Synchronizes the Preview workspace with an already committed Site Structure
 * receipt after the caller has refreshed the authoritative SourceGraph. The
 * coordinator defers this visual projection when no Design Safe surface is
 * mounted; ProjectWorkspace remains the source of truth in either case.
 */
export async function syncCommittedSiteStructurePreview(
  host: SiteStructurePreviewSyncHost,
  lease: PreviewStructuralSessionLease,
  touchedFiles: string[],
  workspaceRevision: number,
) {
  requireCurrentPreviewStructuralSession(host, lease);
  await projectLatestProjectWorkspacePreview(host, {
    reason: "site-workspace",
    minimumWorkspaceRevision: workspaceRevision,
    requestedPaths: touchedFiles,
  });
  requireCurrentPreviewStructuralSession(host, lease);
}

export function partialNameFromLabel(label: string) {
  return label
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

export function partialTemplateName(name: string) {
  const clean = partialNameFromLabel(name);
  if (!clean) return null;
  return `partials/${clean}.html`;
}

export function partialProjectPath(name: string) {
  const templateName = partialTemplateName(name);
  return templateName ? `sursa/templates/${templateName}` : null;
}

export function templateWriteBaseForTarget(
  target?: SourceGraphTemplate | SourceGraphNode | null,
  _activeTheme?: string | null,
): TemplateWriteBase {
  if (target?.origin === "theme") {
    return { origin: "theme", themeName: target.themeName };
  }
  return { origin: "local", themeName: null };
}

export function templateProjectPath(templateName: string, base: TemplateWriteBase) {
  const clean = normalizeTemplateFileName(templateName);
  if (!clean) return null;
  if (base.origin === "theme") {
    if (!base.themeName) throw new Error("Tema țintă lipsește pentru scrierea template-ului.");
    return `sursa/themes/${base.themeName}/templates/${clean}`;
  }
  return `sursa/templates/${clean}`;
}

export async function createReusablePartial(
  host: SiteStructureSessionHost,
  lease: PreviewStructuralSessionLease,
  name: string,
  preset: PartialPreset = "generic",
  target?: SourceGraphTemplate | SourceGraphNode | null,
  activeTheme?: string | null,
) {
  const base = templateWriteBaseForTarget(target, activeTheme);
  const templateName = partialTemplateName(name);
  if (!templateName) {
    throw new Error("Numele partialului este invalid.");
  }

  const result = await createSitePartialStructure({
    name,
    preset,
    targetOrigin: base.origin,
    targetThemeName: base.themeName,
  }, previewStructuralCommandIdentity(lease));
  await projectAuthoritativeSiteStructureReceipt(host, lease, result, result.path);

  return {
    path: result.path,
    templateName: result.templateName,
    created: result.created,
    origin: result.origin,
    themeName: result.themeName,
    workspaceMutation: result.workspaceMutation,
  };
}

export async function createPageStructure(
  host: SiteStructureSessionHost,
  lease: PreviewStructuralSessionLease,
  options: PageCreationOptions,
) {
  const slug = partialNameFromLabel(options.slug || options.title);
  const title = options.title.trim() || slug;
  const pageTemplate = normalizeTemplateFileName(options.pageTemplateName || "page.html");
  const base = templateWriteBaseForTarget(options.targetTemplate, options.activeTheme);
  if (!slug || !title || !pageTemplate) {
    throw new Error("Pagina are nevoie de titlu, slug și template valid.");
  }

  const templatePath = templateProjectPath(pageTemplate, base);
  if (!templatePath) throw new Error("Template-ul paginii este invalid.");
  const result = await createSitePageStructure({
    title,
    slug,
    pageTemplateName: pageTemplate,
    draft: options.draft,
    targetOrigin: base.origin,
    targetThemeName: base.themeName,
  }, previewStructuralCommandIdentity(lease));
  await projectAuthoritativeSiteStructureReceipt(host, lease, result, result.contentPath);

  return {
    slug: result.slug,
    contentPath: result.contentPath,
    templatePath: result.templatePath,
    pageTemplate: result.pageTemplate,
    created: result.created,
    origin: result.origin,
    themeName: result.themeName,
    workspaceMutation: result.workspaceMutation,
  };
}

export async function createArchiveStructure(
  host: SiteStructureSessionHost,
  lease: PreviewStructuralSessionLease,
  options: ArchiveCreationOptions,
) {
  const slug = partialNameFromLabel(options.slug || options.title);
  const title = options.title.trim() || slug;
  const archiveTemplate = normalizeTemplateFileName(options.archiveTemplateName || `${slug}.html`);
  const base = templateWriteBaseForTarget(options.targetTemplate, options.activeTheme);
  if (!slug || !archiveTemplate) {
    throw new Error("Arhiva are nevoie de nume, slug și template valid.");
  }

  const templatePath = templateProjectPath(archiveTemplate, base);
  if (!templatePath) throw new Error("Template-ul arhivei este invalid.");
  const result = await createSiteArchiveStructure({
    title,
    slug,
    archiveTemplateName: archiveTemplate,
    targetOrigin: base.origin,
    targetThemeName: base.themeName,
  }, previewStructuralCommandIdentity(lease));
  await projectAuthoritativeSiteStructureReceipt(host, lease, result, result.contentPath);

  return {
    slug: result.slug,
    contentPath: result.contentPath,
    templatePath: result.templatePath,
    archiveTemplate: result.archiveTemplate,
    created: result.created,
    origin: result.origin,
    themeName: result.themeName,
    workspaceMutation: result.workspaceMutation,
  };
}

export async function createSingleStructure(
  host: SiteStructureSessionHost,
  lease: PreviewStructuralSessionLease,
  options: SingleCreationOptions,
) {
  const sectionSlug = partialNameFromLabel(options.sectionSlug);
  const itemSlug = partialNameFromLabel(options.slug || options.title);
  const title = options.title.trim() || itemSlug;
  const singleTemplate = normalizeTemplateFileName(options.singleTemplateName || `${sectionSlug}-single.html`);
  const base = templateWriteBaseForTarget(options.targetTemplate, options.activeTheme);
  if (!sectionSlug || !itemSlug || !singleTemplate) {
    throw new Error("Single-ul are nevoie de secțiune, slug și template valid.");
  }

  const templatePath = templateProjectPath(singleTemplate, base);
  if (!templatePath) throw new Error("Template-ul single este invalid.");
  const result = await createSiteSingleStructure({
    sectionSlug,
    title,
    slug: itemSlug,
    singleTemplateName: singleTemplate,
    targetOrigin: base.origin,
    targetThemeName: base.themeName,
  }, previewStructuralCommandIdentity(lease));
  await projectAuthoritativeSiteStructureReceipt(host, lease, result, result.itemPath);

  return {
    sectionSlug: result.sectionSlug,
    itemSlug: result.itemSlug,
    itemPath: result.itemPath,
    templatePath: result.templatePath,
    singleTemplate: result.singleTemplate,
    created: result.created,
    origin: result.origin,
    themeName: result.themeName,
    workspaceMutation: result.workspaceMutation,
  };
}

export async function includePartialInTemplate(
  host: SiteStructureSessionHost,
  lease: PreviewStructuralSessionLease,
  template: SourceGraphTemplate,
  partialTemplateNameValue: string,
  ensurePartial: {
    name: string;
    preset: PartialPreset;
    targetOrigin: "local" | "theme";
    targetThemeName: string | null;
  } | null = null,
) {
  const result = await includeSitePartial({
    targetFile: template.file,
    partialTemplateName: partialTemplateNameValue,
    ensurePartial,
  }, previewStructuralCommandIdentity(lease));
  await projectAuthoritativeSiteStructureReceipt(host, lease, result, result.targetFile);
  return {
    changed: result.changed,
    includeChanged: result.includeChanged,
    partialCreated: result.partialCreated,
    partialPath: result.partialPath,
    reason: result.reason,
    templateFile: result.targetFile,
    workspaceMutation: result.workspaceMutation,
  };
}

async function projectAuthoritativeSiteStructureReceipt(
  host: SiteStructureSessionHost,
  lease: PreviewStructuralSessionLease,
  receipt: AuthoritativeSiteStructureReceipt,
  preferredRelativePath: string | null,
) {
  requireCurrentPreviewStructuralSession(host, lease);
  const authority = receipt.authority;
  if (
    authority.projectRoot !== lease.projectRoot
    || authority.sessionId !== lease.sessionId
  ) {
    throw new Error(
      "Receipt-ul Site Structure aparține altei instanțe ProjectSession.",
    );
  }
  if (authority.status === "noop") return;

  if (authority.status !== "staged") {
    throw new Error("Site Structure a returnat un status ProjectWorkspace necunoscut.");
  }
  await host.projectCommittedSiteStructure(
    lease,
    authority.touchedFiles,
    authority.revisionAfter,
    preferredRelativePath,
  );
  requireCurrentPreviewStructuralSession(host, lease);
}

function normalizeTemplateFileName(value: string) {
  const clean = logicalTemplateName(value);
  if (!clean) return "";
  return clean.endsWith(".html") ? clean : `${clean}.html`;
}
