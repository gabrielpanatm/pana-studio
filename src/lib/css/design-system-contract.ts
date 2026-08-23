import type { WorkspaceEntryMutationReceipt } from "$lib/project/workspace-contract";
import type { SourceRange } from "$lib/source-graph/contracts";

export const DESIGN_CLASS_INVENTORY_SCHEMA_VERSION = 1;
export const DESIGN_CLASS_RENAME_SCHEMA_VERSION = 1;

export type DesignClassOccurrenceKind = "markup" | "style";

export type DesignClassOccurrence = {
  file: string;
  kind: DesignClassOccurrenceKind;
  range: SourceRange;
};

export type DesignClassEntry = {
  name: string;
  markupOccurrences: number;
  selectorOccurrences: number;
  files: string[];
  occurrences: DesignClassOccurrence[];
};

export type DesignClassInventorySnapshot = {
  schemaVersion: typeof DESIGN_CLASS_INVENTORY_SCHEMA_VERSION;
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number;
  projectModelRevision: string;
  classes: DesignClassEntry[];
};

export type DesignClassRenameReceipt = {
  schemaVersion: typeof DESIGN_CLASS_RENAME_SCHEMA_VERSION;
  oldName: string;
  newName: string;
  changedFiles: string[];
  replacementCount: number;
  workspace: WorkspaceEntryMutationReceipt;
};

export const DESIGN_TOKEN_CATALOG_SCHEMA_VERSION = 1;

export type DesignTokenVisualKind =
  | "color"
  | "font_family"
  | "font_size"
  | "font_weight"
  | "line_height"
  | "letter_spacing"
  | "spacing"
  | "radius"
  | "shadow"
  | "transition"
  | "breakpoint"
  | "layout"
  | "layer"
  | "other";

export type DesignTokenSnapshot = {
  id: string;
  name: string;
  categoryId: string;
  groupLabel: string;
  visualKind: DesignTokenVisualKind;
  rawValue: string;
  resolvedValue: string | null;
  dependencies: string[];
  sourcePath: string;
  sourceLine: number;
  editable: boolean;
  diagnostic: string | null;
};

export type DesignTokenCategorySnapshot = {
  id: string;
  label: string;
  tokenCount: number;
};

export type DesignTokenCatalogSnapshot = {
  schemaVersion: typeof DESIGN_TOKEN_CATALOG_SCHEMA_VERSION;
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number;
  categories: DesignTokenCategorySnapshot[];
  tokens: DesignTokenSnapshot[];
  warnings: string[];
};

export const THEME_STYLE_CATALOG_SCHEMA_VERSION = 1;

export type ThemeStyleControlKind = "text" | "color" | "choice";

export type ThemeStyleControlOption = {
  value: string;
  label: string;
};

export type ThemeStylePropertySnapshot = {
  id: string;
  label: string;
  control: ThemeStyleControlKind;
  options: ThemeStyleControlOption[];
  value: string | null;
  effectiveValue: string | null;
  inheritedFrom: string | null;
  tokenName: string | null;
  canClear: boolean;
};

export type ThemeStyleTargetSnapshot = {
  id: string;
  categoryId: string;
  label: string;
  description: string;
  selector: string;
  parentId: string | null;
  previewKind: string;
  sampleText: string;
  sourcePath: string;
  editable: boolean;
  diagnostic: string | null;
  hasOverrides: boolean;
  properties: ThemeStylePropertySnapshot[];
};

export type ThemeStyleCategorySnapshot = {
  id: string;
  label: string;
  targetCount: number;
};

export type ThemeStyleCatalogSnapshot = {
  schemaVersion: typeof THEME_STYLE_CATALOG_SCHEMA_VERSION;
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number;
  sourcePath: string;
  sourceOrigin: string;
  categories: ThemeStyleCategorySnapshot[];
  targets: ThemeStyleTargetSnapshot[];
  warnings: string[];
};

export type ThemeStylePropertyInput = {
  id: string;
  value: string;
};

export type ThemeStylePreviewProperty = {
  id: string;
  value: string;
  inherited: boolean;
};

export type ThemeStyleDraftPreview = {
  schemaVersion: typeof THEME_STYLE_CATALOG_SCHEMA_VERSION;
  targetId: string;
  selector: string;
  sourcePath: string;
  css: string;
  properties: ThemeStylePreviewProperty[];
};
