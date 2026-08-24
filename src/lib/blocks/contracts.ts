import type { DynamicWidgetProperties } from "$lib/content-models/contracts";
import type { LocalizedDiagnostic } from "$lib/contracts/localized-diagnostic";
import type { ProjectMovePosition } from "$lib/preview/contracts";
import type {
  ProjectSourceEditLocation,
  SourceDiagnosticSeverity,
} from "$lib/source-graph/contracts";

type BlockOrigin = "native" | "application" | "theme" | "project";

type BlockScale = "element" | "section" | "composition";

type BlockResolutionStatus = "resolved" | "unknownProvider" | "invalidContract";

type BlockRequirementKind = "runtime" | "stylesheet" | "markup";

type BlockOptionControl = "toggle" | "number" | "text" | "select";

export type BlockOptionValue =
  | { kind: "boolean"; value: boolean }
  | { kind: "integer"; value: number }
  | { kind: "text"; value: string };

type BlockCapabilities = {
  canInsert: boolean;
  canEditProperties: boolean;
  supportsVariants: boolean;
  supportsSlots: boolean;
};

type BlockRequirement = {
  id: string;
  kind: BlockRequirementKind;
  minimumVersion: number;
  required: boolean;
};

export type BlockOptionDefinition = {
  id: string;
  label: string;
  description: string;
  control: BlockOptionControl;
  attribute: string;
  defaultValue: BlockOptionValue;
  omitWhenDefault: boolean;
  constraints: {
    minimum: number | null;
    maximum: number | null;
    step: number | null;
    maximumLength: number | null;
  };
  choices: Array<{ value: string; label: string }>;
};

type BlockSlotDefinition = {
  id: string;
  label: string;
  required: boolean;
  multiple: boolean;
  itemKind: string;
  minimumItems: number;
  maximumItems: number | null;
};

type BlockDefinition = {
  id: string;
  schemaVersion: number;
  providerId: string;
  familyId: string;
  variantId: string;
  displayName: string;
  description: string;
  origin: BlockOrigin;
  scale: BlockScale;
  capabilities: BlockCapabilities;
  requirements: BlockRequirement[];
  options: BlockOptionDefinition[];
  slots: BlockSlotDefinition[];
};

type BlockDiagnostic = {
  code: string;
  diagnostic: LocalizedDiagnostic;
  severity: SourceDiagnosticSeverity;
  file: string | null;
  sourceNodeId: string | null;
};

type BlockSourceInstance = {
  id: string;
  definitionId: string | null;
  providerId: string;
  file: string;
  sourceNodeId: string;
  status: BlockResolutionStatus;
  diagnostics: BlockDiagnostic[];
};

type RenderedBlockInstance = {
  id: string;
  definitionId: string | null;
  sourceInstanceId: string | null;
  renderInstanceId: string;
  route: string;
  sourceNodeId: string | null;
  parentInstanceId: string | null;
  bindingKey: string | null;
  bindingPath: string | null;
};

export type BlockGraph = {
  schemaVersion: number;
  definitions: BlockDefinition[];
  sourceInstances: BlockSourceInstance[];
  diagnostics: BlockDiagnostic[];
};

export type NativeBlockOptionState = {
  id: string;
  value: BlockOptionValue;
  isDefault: boolean;
};

export type UiBlockSourceInstance = {
  id: string;
  definitionId: string | null;
  providerId: string;
  file: string;
  markerSourceNodeId: string;
  rootSourceNodeId: string | null;
  rootLocation: ProjectSourceEditLocation | null;
  status: BlockResolutionStatus;
  editable: boolean;
  diagnostic: LocalizedDiagnostic | null;
  options: NativeBlockOptionState[];
  slots: NativeBlockSlotState[];
  icon: NativeIconState | null;
};

type NativeBlockSlotItemState = {
  sourceNodeId: string;
  tag: string;
  label: string;
  index: number;
  editable: boolean;
};

type NativeBlockSlotState = {
  id: string;
  itemKind: string;
  containerSourceNodeId: string | null;
  minimumItems: number;
  maximumItems: number | null;
  editable: boolean;
  diagnostic: string | null;
  items: NativeBlockSlotItemState[];
};

export type NativeBlockSlotMutationContext = {
  providerId: string;
  slotId: string;
  rootSourceId: string;
  expectedModelRevision: string;
};

export type NativeBlockSlotMutationRequest = {
  operation: "insert" | "duplicate" | "move" | "delete";
  context: NativeBlockSlotMutationContext;
  slot: NativeBlockSlotState;
  item?: NativeBlockSlotItemState | null;
  targetItem?: NativeBlockSlotItemState | null;
  position?: "before" | "after";
};

export type NativeIconState = {
  iconIdentity: string;
  packId: string;
  iconId: string;
  size: number;
  strokeWidth: string;
  decorative: boolean;
  accessibleLabel: string | null;
};

export type NativeIconMutationIntent = {
  iconIdentity: string;
  size: number;
  strokeWidth: string;
  decorative: boolean;
  accessibleLabel: string | null;
};

export type UiBlockGraphSnapshot = {
  schemaVersion: 4;
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number;
  modelRevision: string;
  previewRevision: string | null;
  canvasAvailable: boolean;
  definitions: BlockDefinition[];
  sourceInstances: UiBlockSourceInstance[];
  renderedInstances: RenderedBlockInstance[];
  diagnostics: LocalizedDiagnostic[];
};

export const INSERT_CATALOG_SCHEMA_VERSION = 2 as const;

export type InsertCatalogCategory =
  | "html"
  | "block"
  | "component"
  | "tera"
  | "dynamicWidget";

type InsertCatalogOrigin =
  | "application"
  | "native"
  | "project"
  | "theme";

export type InsertCatalogContext = {
  activeDocumentPath: string | null;
  activeTemplatePath: string | null;
  activePagePath: string | null;
  canvasPreviewRevision: string | null;
  canvasAvailable: boolean;
  targetSourceId: string | null;
  targetTag: string | null;
};

type InsertCatalogCapabilities = {
  canDrag: boolean;
  allowedPositions: ProjectMovePosition[];
  reasonCode: string | null;
  reasonArguments: Record<string, string>;
};

type InsertCatalogPayload =
  | {
      kind: "html";
      tag: string;
      className: string;
      text: string;
    }
  | {
      kind: "block";
      blockId: string;
      blockKind: "js" | "static";
      tag: string;
      className: string;
      text: string;
    }
  | {
      kind: "component";
      componentId: string;
      teraKind: string;
      family: string;
      target: string;
      name: string | null;
      expression: string | null;
    }
  | {
      kind: "tera";
      teraKind: string;
      family: string;
      target: string | null;
      name: string | null;
      expression: string | null;
    }
  | {
      kind: "dynamicWidget";
      providerId: string;
      properties: DynamicWidgetProperties;
    };

export type InsertCatalogItem = {
  id: string;
  category: InsertCatalogCategory;
  origin: InsertCatalogOrigin;
  label: string;
  description: string;
  capabilities: InsertCatalogCapabilities;
  payload: InsertCatalogPayload;
};

type InsertCatalogGroup = {
  id: string;
  category: InsertCatalogCategory;
  label: string;
  description: string;
  items: InsertCatalogItem[];
};

export type InsertCatalogSnapshot = {
  schemaVersion: typeof INSERT_CATALOG_SCHEMA_VERSION;
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number;
  modelRevision: string;
  context: InsertCatalogContext;
  groups: InsertCatalogGroup[];
};
