import type { LocalizedDiagnostic } from "$lib/contracts/localized-diagnostic";
import type { WorkspaceEntryMutationReceipt } from "$lib/project/workspace-contract";

type ComponentMutationOperation =
  | "create"
  | "update"
  | "duplicate"
  | "move"
  | "rename"
  | "extract"
  | "delete"
  | "override_theme";

export type ComponentDraftKind =
  | "partial"
  | "macro_library"
  | "shortcode_html"
  | "shortcode_markdown";

type ComponentCompanionKind = "style" | "script" | "data";

export type ComponentCompanionDraft = {
  kind: ComponentCompanionKind;
  relativePath: string;
  contents: string;
  createOnly: boolean;
};

type ComponentExtractionRange = {
  start: number;
  end: number;
};

export type ComponentMutationInput = {
  operation: ComponentMutationOperation;
  definitionId: string | null;
  kind: ComponentDraftKind | null;
  name: string | null;
  destinationName: string | null;
  contents: string | null;
  sourceFile: string | null;
  sourceRange: ComponentExtractionRange | null;
  companions: ComponentCompanionDraft[];
};

type ComponentPlannedWrite = {
  relativePath: string;
  contents: string;
  createOnly: boolean;
};

type ComponentMutationDiagnostic = {
  diagnostic: LocalizedDiagnostic;
  relativePath: string | null;
};

type ComponentMutationPlan = {
  schemaVersion: 2;
  operation: ComponentMutationOperation;
  definitionId: string | null;
  sourceRelativePath: string | null;
  destinationRelativePath: string | null;
  writes: ComponentPlannedWrite[];
  deletes: string[];
  touchedFiles: string[];
  diagnostics: ComponentMutationDiagnostic[];
};

export type ComponentMutationApplyReceipt = {
  plan: ComponentMutationPlan;
  workspace: WorkspaceEntryMutationReceipt;
};

type IconCatalogNode = {
  tag: "path";
  attributes: Record<string, string>;
};

type IconCatalogItem = {
  id: string;
  label: string;
  category: string;
  tags: string[];
  nodes: IconCatalogNode[];
};

export type IconCatalogSummary = {
  schemaVersion: 1;
  packId: "tabler-outline";
  packVersion: string;
  license: string;
  total: number;
  categories: string[];
};

export type IconCatalogSearchInput = {
  query: string;
  category?: string | null;
  offset?: number | null;
  limit?: number | null;
};

export type IconCatalogPage = {
  schemaVersion: 1;
  packId: "tabler-outline";
  packVersion: string;
  query: string;
  category: string | null;
  offset: number;
  limit: number;
  total: number;
  hasMore: boolean;
  items: IconCatalogItem[];
};
