import type {
  FileBufferRequestIdentity,
  WorkspaceEntryMutationReceipt,
} from "$lib/project/workspace-contract";
import type { SourceRange } from "$lib/source-graph/contracts";

export type ContentFieldKind =
  | "text"
  | "textarea"
  | "markdown"
  | "number"
  | "boolean"
  | "date"
  | "select"
  | "url"
  | "color"
  | "image"
  | "group"
  | "repeater";

type ContentFieldChoice = {
  value: string;
  label: string;
};

export type ContentFieldDefinition = {
  id: string;
  key: string;
  label: string;
  kind: ContentFieldKind;
  required: boolean;
  help: string;
  defaultValue?: unknown;
  choices: ContentFieldChoice[];
  minimum?: number;
  maximum?: number;
  pattern?: string;
  fields: ContentFieldDefinition[];
};

export type ContentModelDefinition = {
  schemaVersion: number;
  id: string;
  label: string;
  description: string;
  fields: ContentFieldDefinition[];
  file: string;
};

type ContentModelAssignment = {
  sectionPath: string;
  modelId: string;
};

type CustomFieldTemplateUsage = {
  modelId: string;
  fieldId: string;
  fieldKey: string;
  templateFile: string;
  expression: string;
  offset: number;
};

type ContentModelPageBinding = {
  pageFile: string;
  sectionPath: string;
  modelId: string;
  values: Record<string, unknown>;
  missingRequiredFields: string[];
};

type ContentModelDiagnostic = {
  severity: "warning" | "error";
  code: string;
  message: string;
  file: string | null;
};

export type ContentModelCatalog = {
  schemaVersion: number;
  metadataPresent: boolean;
  models: ContentModelDefinition[];
  assignments: ContentModelAssignment[];
  pageBindings: ContentModelPageBinding[];
  templateUsages: CustomFieldTemplateUsage[];
  diagnostics: ContentModelDiagnostic[];
};

export const CONTENT_MODEL_SCHEMA_VERSION = 1 as const;

type ContentModelMutationOperation =
  | { kind: "create_model"; id: string; label: string; description: string }
  | { kind: "update_model"; modelId: string; label: string; description: string }
  | { kind: "rename_model"; modelId: string; newId: string; label: string; description: string }
  | { kind: "delete_model"; modelId: string }
  | {
      kind: "upsert_field";
      modelId: string;
      parentFieldId: string | null;
      originalFieldId: string | null;
      field: ContentFieldDefinition;
    }
  | { kind: "remove_field"; modelId: string; parentFieldId: string | null; fieldId: string }
  | {
      kind: "reorder_field";
      modelId: string;
      parentFieldId: string | null;
      fieldId: string;
      targetIndex: number;
    }
  | { kind: "attach_model"; modelId: string; sectionPath: string }
  | { kind: "detach_model"; modelId: string; sectionPath: string }
  | {
      kind: "replace_model";
      sectionPath: string;
      fromModelId: string;
      toModelId: string;
      fieldMigrations: Record<string, string>;
    }
  | { kind: "set_page_values"; pageFile: string; values: Record<string, unknown> };

export type ContentModelMutationInput = {
  operation: ContentModelMutationOperation;
};

export type ContentModelMutationPlan = {
  schemaVersion: number;
  planId: string;
  operation: string;
  label: string;
  touchedFiles: string[];
  affectedPages: string[];
  affectedKeys: string[];
  destructive: boolean;
  blocked: boolean;
  blockers: string[];
  templateUsages: CustomFieldTemplateUsage[];
  warnings: string[];
};

export type ContentModelMutationApplyReceipt = {
  plan: ContentModelMutationPlan;
  workspace: WorkspaceEntryMutationReceipt;
};

export type DynamicFieldScope =
  | "page"
  | "collectionItem"
  | "section"
  | "site"
  | "repeaterItem"
  | "taxonomyTerm";

export type DynamicFieldPresentation =
  | "auto"
  | "text"
  | "heading"
  | "paragraph"
  | "badge"
  | "date"
  | "number"
  | "currency"
  | "percent"
  | "image"
  | "link"
  | "button"
  | "trustedContent";

type DynamicFieldEmptyBehavior = "fallback" | "renderEmpty" | "hide";

type DynamicValueType =
  | "text"
  | "richHtml"
  | "date"
  | "number"
  | "boolean"
  | "url"
  | "image"
  | "listObject";

export type DynamicValueSource =
  | { kind: "builtin"; field: string }
  | { kind: "customField"; modelId: string; fieldId: string }
  | { kind: "configExtra"; path: string[] }
  | { kind: "sectionExtra"; path: string[] };

type DynamicValueBinding = {
  context: DynamicFieldScope;
  source: DynamicValueSource;
  valueType: DynamicValueType;
};

type DynamicValueFormat = {
  dateFormat: string;
  decimals: number | null;
  currency: string;
};

type ListingSortBy = "date" | "updated" | "title" | "weight" | "slug" | "none";

type ListingSortOrder = "asc" | "desc";

type DynamicFieldWidgetProperties = {
  binding: DynamicValueBinding;
  presentation: DynamicFieldPresentation;
  tag: string;
  format: DynamicValueFormat;
  prefix: string;
  suffix: string;
  fallback: string;
  label: string;
  emptyBehavior: DynamicFieldEmptyBehavior;
};

type ListingWidgetProperties = {
  sectionPath: string;
  listingItemId: string;
  listingItemTemplate: string;
  includeSubsections: boolean;
  sortBy: ListingSortBy;
  sortOrder: ListingSortOrder;
  limit: number | null;
  offset: number;
  emptyText: string;
  tag: string;
  className: string;
};

export type DynamicWidgetProperties =
  | { kind: "dynamicField"; properties: DynamicFieldWidgetProperties }
  | { kind: "listing"; properties: ListingWidgetProperties };

type DynamicWidgetProviderKind = "dynamic-field" | "listing";

type DynamicWidgetResolutionStatus =
  | "resolved"
  | "unknownProvider"
  | "invalidContract"
  | "incompatible";

type DynamicWidgetDiagnostic = {
  code: string;
  message: string;
  file: string | null;
  instanceId: string | null;
};

type DynamicWidgetProviderDefinition = {
  id: string;
  schemaVersion: number;
  kind: DynamicWidgetProviderKind;
  label: string;
  description: string;
  capabilities: {
    canInsert: boolean;
    canEditProperties: boolean;
    canDuplicate: boolean;
    canDelete: boolean;
    rendersMultipleInstances: boolean;
  };
};

export type DynamicValueDefinition = {
  id: string;
  group: string;
  label: string;
  description: string;
  contexts: DynamicFieldScope[];
  valueType: DynamicValueType;
  source: DynamicValueSource;
  modelId: string | null;
  compatiblePresentations: DynamicFieldPresentation[];
  defaultPresentation: DynamicFieldPresentation;
  defaultTag: string;
};

type DynamicWidgetSourceInstance = {
  id: string;
  instanceId: string;
  providerId: string;
  providerKind: DynamicWidgetProviderKind | null;
  file: string;
  range: SourceRange;
  startMarkerRange: SourceRange;
  endMarkerRange: SourceRange;
  sourceNodeIds: string[];
  rootSourceNodeIds: string[];
  status: DynamicWidgetResolutionStatus;
  properties: DynamicWidgetProperties | null;
  canonicalBindingPath: string | null;
  canonicalBindingExpression: string | null;
  sourceRevision: string;
  diagnostics: DynamicWidgetDiagnostic[];
};

type RenderedDynamicWidgetInstance = {
  id: string;
  sourceInstanceId: string;
  instanceId: string;
  providerId: string;
  renderInstanceId: string;
  route: string;
  sourceNodeId: string | null;
  parentInstanceId: string | null;
  bindingKey: string | null;
  bindingPath: string | null;
};

export type DynamicWidgetGraph = {
  schemaVersion: number;
  definitions: DynamicWidgetProviderDefinition[];
  valueCatalog: DynamicValueDefinition[];
  sourceInstances: DynamicWidgetSourceInstance[];
  diagnostics: DynamicWidgetDiagnostic[];
};

export type DynamicWidgetSnapshotRequest = {
  identity: FileBufferRequestIdentity;
  expectedWorkspaceRevision: number;
  expectedModelRevision: string;
  previewRevision: string;
  sourceInstanceId: string;
};

export type DynamicWidgetSnapshot = {
  schemaVersion: number;
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number;
  modelRevision: string;
  previewRevision: string;
  sourceInstance: DynamicWidgetSourceInstance;
  renderedInstances: RenderedDynamicWidgetInstance[];
};

/** Exact semantic dynamic boundary selected through Canvas navigation. */
export type DynamicWidgetSelectionContext = {
  sourceInstanceId: string;
  sourceInstanceIds: string[];
  providerId: string;
  modelRevision: string;
  previewRevision: string;
  renderInstanceId: string;
};

export type UpdateDynamicWidgetInput = {
  request: DynamicWidgetSnapshotRequest;
  expectedSourceRevision: string;
  properties: DynamicWidgetProperties;
};

export type DeleteDynamicWidgetInput = {
  request: DynamicWidgetSnapshotRequest;
  expectedSourceRevision: string;
};

type ListingItemStatus =
  | "resolved"
  | "missingMetadata"
  | "missingTemplate"
  | "missingModel"
  | "missingPreviewPage"
  | "incompatiblePreviewPage";

type ListingItemDiagnostic = {
  code: string;
  message: string;
  file: string | null;
  itemId: string | null;
};

type ListingItemDefinition = {
  id: string;
  label: string;
  templateName: string;
  file: string;
  modelId: string | null;
  previewPageFile: string | null;
  previewUrl: string | null;
  compatibleSectionPaths: string[];
  usageCount: number;
  status: ListingItemStatus;
  diagnostics: ListingItemDiagnostic[];
};

export type ListingItemCatalog = {
  schemaVersion: number;
  metadataPresent: boolean;
  items: ListingItemDefinition[];
  diagnostics: ListingItemDiagnostic[];
};
