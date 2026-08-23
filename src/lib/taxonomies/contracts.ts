import type { LocalizedDiagnostic } from "$lib/contracts/localized-diagnostic";
import type { WorkspaceEntryMutationReceipt } from "$lib/project/workspace-contract";
import type { SourceOrigin } from "$lib/source-graph/contracts";

type TaxonomyCatalogDiagnosticSeverity = "warning" | "error";

type TaxonomyCatalogDiagnostic = {
  code: string;
  severity: TaxonomyCatalogDiagnosticSeverity;
  diagnostic: LocalizedDiagnostic;
  file: string | null;
  taxonomyName: string | null;
  term: string | null;
};

type TaxonomyCatalogPageUsage = {
  file: string;
  title: string;
  url: string;
};

export type TaxonomyCatalogTemplate = {
  logicalName: string;
  file: string | null;
  origin: SourceOrigin | null;
  themeName: string | null;
  fallback: boolean;
  missing: boolean;
};

export type TaxonomyCatalogTerm = {
  id: string;
  name: string;
  aliases: string[];
  slug: string;
  path: string;
  permalink: string;
  pages: TaxonomyCatalogPageUsage[];
};

type TaxonomyCatalogCapabilities = {
  canEditDefinition: boolean;
  canDeleteDefinition: boolean;
  canAssignTerms: boolean;
};

export type TaxonomyCatalogEntry = {
  id: string;
  name: string;
  slug: string;
  language: string;
  declared: boolean;
  render: boolean;
  feed: boolean;
  paginateBy: number | null;
  paginatePath: string | null;
  path: string;
  permalink: string;
  terms: TaxonomyCatalogTerm[];
  pages: TaxonomyCatalogPageUsage[];
  listTemplate: TaxonomyCatalogTemplate;
  termTemplate: TaxonomyCatalogTemplate;
  capabilities: TaxonomyCatalogCapabilities;
};

export type TaxonomyCatalogSnapshot = {
  schemaVersion: number;
  configPath: string;
  taxonomyRoot: string | null;
  defaultLanguage: string;
  slugifyStrategy: string;
  entries: TaxonomyCatalogEntry[];
  diagnostics: TaxonomyCatalogDiagnostic[];
};

export const TAXONOMY_CATALOG_SCHEMA_VERSION = 2 as const;

export type TaxonomyDefinitionInput = {
  name: string;
  language: string;
  render: boolean;
  feed: boolean;
  paginateBy: number | null;
  paginatePath: string | null;
};

type TaxonomyMutationOperation =
  | { kind: "set_taxonomy_root"; taxonomyRoot: string | null }
  | {
      kind: "upsert_definition";
      originalName: string | null;
      originalLanguage: string | null;
      definition: TaxonomyDefinitionInput;
    }
  | {
      kind: "set_page_terms";
      pageFile: string;
      taxonomyName: string;
      terms: string[];
    }
  | {
      kind: "rename_term";
      taxonomyName: string;
      language: string;
      oldTerm: string;
      newTerm: string;
    }
  | {
      kind: "remove_definition";
      name: string;
      language: string;
      removeAssignments: boolean;
      expectedUsageCount: number;
    };

export type TaxonomyMutationInput = {
  operation: TaxonomyMutationOperation;
};

export type TaxonomyMutationPlan = {
  schemaVersion: number;
  planId: string;
  operation: string;
  label: string;
  configPath: string;
  touchedFiles: string[];
  affectedPages: string[];
  usageCount: number;
  warnings: string[];
};

export type TaxonomyMutationApplyReceipt = {
  plan: TaxonomyMutationPlan;
  workspace: WorkspaceEntryMutationReceipt;
};

export const TAXONOMY_MUTATION_SCHEMA_VERSION = 1 as const;
