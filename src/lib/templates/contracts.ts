import type { LocalizedDiagnostic } from "$lib/contracts/localized-diagnostic";
import type { SourceOrigin } from "$lib/source-graph/contracts";

type TemplateCatalogRole = "page" | "layout" | "partial" | "listing_item" | "component_library";

export type TemplateSemanticCategory =
  | "layout"
  | "page"
  | "archive"
  | "element"
  | "partial"
  | "listing_item"
  | "taxonomy"
  | "system";

export type TemplateSemanticRole =
  | "layout"
  | "homepage"
  | "default_page"
  | "specific_page"
  | "section_archive"
  | "section_element"
  | "partial"
  | "listing_item"
  | "taxonomy_list"
  | "taxonomy_term"
  | "not_found"
  | "custom";

type TemplateSemanticTargetKind =
  | "resource"
  | "site"
  | "page"
  | "section"
  | "taxonomy"
  | "system"
  | "custom";

type TemplateCatalogContext = "page" | "section" | "system";

export type TemplateAssignmentSource = "explicit" | "inherited" | "default" | "convention";

type TemplateCatalogReferenceKind = "extends" | "includes";

type TemplateCatalogTemplateUsage = {
  file: string;
  name: string;
  kind: TemplateCatalogReferenceKind;
};

type TemplateCatalogPageUsage = {
  file: string;
  title: string;
  url: string;
};

export type TemplateResource = {
  id: string;
  file: string;
  name: string;
  origin: SourceOrigin;
  themeName: string | null;
  roles: TemplateCatalogRole[];
  editable: boolean;
  effective: boolean;
  localOverridePath: string;
  extends: string | null;
  includes: string[];
  blocks: string[];
  components: string[];
  usedByTemplates: TemplateCatalogTemplateUsage[];
  affectedPages: TemplateCatalogPageUsage[];
  canDelete: boolean;
  deleteBlockedDiagnostic: LocalizedDiagnostic | null;
  nodeId: string;
};

type TemplateSemanticTarget = {
  id: string;
  kind: TemplateSemanticTargetKind;
  label: string | null;
  labelDiagnostic: LocalizedDiagnostic | null;
  file: string | null;
  url: string | null;
};

type TemplateAssignment = {
  key: string | null;
  source: TemplateAssignmentSource;
  declaredIn: string | null;
  resourceId: string | null;
  resourceName: string;
  fallbackName: string | null;
};

type TemplatePreviewContext = {
  kind: TemplateCatalogContext;
  pageFile: string | null;
  title: string | null;
  titleDiagnostic: LocalizedDiagnostic | null;
  url: string;
  exact: boolean;
  available: boolean;
  unavailableDiagnostic: LocalizedDiagnostic | null;
};

export type TemplateSemanticEntry = {
  id: string;
  category: TemplateSemanticCategory;
  role: TemplateSemanticRole;
  label: string | null;
  labelDiagnostic: LocalizedDiagnostic | null;
  target: TemplateSemanticTarget;
  assignment: TemplateAssignment;
  previewContext: TemplatePreviewContext | null;
  affectedPages: TemplateCatalogPageUsage[];
};

export type TemplateCatalogSnapshot = {
  schemaVersion: number;
  activeTheme: string | null;
  resources: TemplateResource[];
  semanticEntries: TemplateSemanticEntry[];
};

export const TEMPLATE_CATALOG_SCHEMA_VERSION = 6 as const;

export type CreateListingItemInput = {
  label: string;
  slug: string;
  modelId: string;
  previewPageFile: string;
};

export type DeleteListingItemInput = {
  id: string;
};

export type TemplateSemanticCreateRole = TemplateSemanticRole;

export type CreateSemanticTemplateInput = {
  role: TemplateSemanticCreateRole;
  name: string;
  targetId?: string | null;
  newSection?: {
    title: string;
    slug: string;
    sortBy?: "none" | "date" | "title" | "weight" | null;
  } | null;
  parentTemplateName?: string | null;
  includePageContent: boolean;
};

export type SetTemplateParentInput = {
  relativePath: string;
  parentTemplateName?: string | null;
};

type TemplateAssignmentKey = "template" | "page_template";

export type SetTemplateAssignmentInput = {
  contentRelativePath: string;
  key: TemplateAssignmentKey;
  templateName?: string | null;
};

export type DuplicateTemplateInput = {
  sourceRelativePath: string;
  destinationName: string;
};

export type OverrideThemeTemplateInput = {
  sourceRelativePath: string;
};

export type RenameTemplateInput = {
  sourceRelativePath: string;
  destinationName: string;
};

export type DeleteTemplateInput = {
  relativePath: string;
};
