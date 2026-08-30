import type {
  BlockOptionValue,
  NativeBlockSlotMutationContext,
  NativeIconMutationIntent,
  NativeIconState,
} from "$lib/blocks/contracts";
import type {
  ZolaImageFormat,
  ZolaImageFilter,
  ZolaImageOperation,
  ZolaImagePresentation,
} from "$lib/canvas/contracts";
import type { DynamicWidgetProperties } from "$lib/content-models/contracts";
import type { LocalizedDiagnostic } from "$lib/contracts/localized-diagnostic";
import type { ProjectWorkspaceMutationReceipt } from "$lib/project/workspace-contract";
import type { ProjectSourceEditLocation } from "$lib/source-graph/contracts";

export type ProjectMovePosition = "before" | "after" | "inside";

type ProjectHtmlInsertElement = {
  kind?: "html" | "block" | "nativeBlockSlotItem" | null;
  blockId?: string | null;
  tag: string;
  className?: string | null;
  text?: string | null;
  label?: string | null;
};

type ProjectHtmlInsertIntent = {
  targetSourceId: string | null;
  targetTag?: string | null;
  targetKind?: string | null;
  position: ProjectMovePosition;
  element: ProjectHtmlInsertElement;
  nativeBlockSlot?: NativeBlockSlotMutationContext | null;
};

export type ProjectHtmlAttributeMutation =
  | { kind: "setAttribute"; name: string; value: string }
  | { kind: "removeAttribute"; name: string };

export type NativeBlockOptionIntent = {
  providerId: string;
  optionId: string;
  value: BlockOptionValue;
};

export type ProjectGeneratedIdentityKind = "class" | "dataAnim";

export type ProjectGeneratedIdentityIntent = {
  kind: ProjectGeneratedIdentityKind;
};

type ProjectGeneratedIdentityProjection = {
  kind: ProjectGeneratedIdentityKind;
  value: string;
  classes: string[];
  dataAnim: string | null;
  alreadyPresent: boolean;
};

type ProjectHtmlAttributeIntent = {
  targetSourceId: string | null;
  targetTag?: string | null;
  attributes: ProjectHtmlAttributeMutation[];
  zolaImage?: ProjectZolaImageIntent | null;
  nativeBlockOption?: NativeBlockOptionIntent | null;
  nativeIcon?: NativeIconMutationIntent | null;
  generatedIdentity?: ProjectGeneratedIdentityIntent | null;
};

export type ProjectZolaImageIntent = {
  enabled: boolean;
  sourceUrl?: string | null;
  sourcePath?: string | null;
  width?: number | null;
  height?: number | null;
  operation?: ZolaImageOperation | null;
  format?: ZolaImageFormat | null;
  quality?: number | null;
  filter?: ZolaImageFilter | null;
};

type ProjectHtmlTextIntent = {
  targetSourceId: string | null;
  targetTag?: string | null;
  text: string;
};

type ProjectHtmlTagIntent = {
  targetSourceId: string | null;
  targetTag?: string | null;
  newTag: string;
};

type ProjectHtmlDeleteIntent = {
  targetSourceId: string | null;
  targetRenderInstanceId?: string | null;
  targetTag?: string | null;
  nativeBlockSlot?: NativeBlockSlotMutationContext | null;
};

type ProjectHtmlDuplicateIntent = {
  sourceSourceId: string | null;
  sourceTag?: string | null;
  nativeBlockSlot?: NativeBlockSlotMutationContext | null;
};

type ProjectTeraDeleteIntent = {
  targetSourceId: string | null;
  targetKind?: string | null;
  targetLabel?: string | null;
};

type ProjectTeraInsertItem = {
  kind: string;
  label?: string | null;
  target?: string | null;
  name?: string | null;
  expression?: string | null;
  dynamicWidget?: DynamicWidgetProperties | null;
};

type ProjectTeraInsertIntent = {
  targetSourceId: string | null;
  targetKind?: string | null;
  targetTag?: string | null;
  position: ProjectMovePosition;
  item: ProjectTeraInsertItem;
};

type PreviewProjectionIntentKind =
  | "html_insert_drop"
  | "html_attributes"
  | "html_text"
  | "html_tag"
  | "html_duplicate"
  | "tera_insert_drop"
  | "html_delete"
  | "template_delete"
  | "unsupported";

type PreviewProjectionIntentStatus = "accepted" | "blocked" | "unsupported";

type PreviewProjectionEffect =
  | "kernel_mutation_preflight"
  | "unsupported";

type PreviewProjectionDiagnosticSeverity = "info" | "warning" | "error";

export type PreviewProjectionDiagnostic = {
  code: string;
  severity: PreviewProjectionDiagnosticSeverity;
  diagnostic: LocalizedDiagnostic;
  blocking: boolean;
};

export type PreviewProjectionIntentInput = {
  messageType: string;
  previewRevision?: number | null;
  sourceId?: string | null;
  targetSourceId?: string | null;
  sourceTemplateSourceId?: string | null;
  targetTemplateSourceId?: string | null;
  sourceSessionId?: string | null;
  targetSessionId?: string | null;
  sourceTag?: string | null;
  targetTag?: string | null;
  targetKind?: string | null;
  position?: string | null;
  itemKind?: string | null;
  elementTag?: string | null;
};

export type PreviewProjectionIntentReceipt = {
  schemaVersion: number;
  intentId: string;
  kind: PreviewProjectionIntentKind;
  status: PreviewProjectionIntentStatus;
  effect: PreviewProjectionEffect;
  accepted: boolean;
  requiresProjectSession: boolean;
  projectSessionId: string | null;
  projectRoot: string | null;
  runtimeSessionId: string | null;
  previewRevision: number | null;
  messageDiagnostic: LocalizedDiagnostic;
  diagnostics: PreviewProjectionDiagnostic[];
};

export type PreviewStructuralCommandIdentity = {
  expectedProjectRoot: string;
  expectedSessionId: string;
  expectedSelection?: PreviewStructuralSelectionIdentity | null;
};

export type SelectionMutationIdentity = {
  selectionRevision: number;
  workspaceRevision: number;
  primaryMemberId: string | null;
  members: readonly SelectionMutationMemberIdentity[];
};

type SelectionMutationMemberIdentity = {
  memberId: string;
  editorNodeId: string | null;
  sourceNodeId: string | null;
  renderInstanceId: string | null;
};

export type PreviewStructuralSelectionIdentity = SelectionMutationIdentity;

type ProjectHtmlInsertPatch = {
  file: string;
  resolvedTargetId: string;
  insertedLabel: string;
  beforeRevision: string;
  afterRevision: string;
  contents: string;
  targetLocation: ProjectSourceEditLocation;
  insertedLocation: ProjectSourceEditLocation;
  insertedStartLine: number;
  lineShiftStart: number;
  lineShift: number;
  tag: string;
  className: string;
  text: string;
  html: string;
  blockId: string | null;
  dataAnim: string | null;
  blockInstanceId: string | null;
};

type CanvasPatchAnchor = {
  sourceId: string;
  renderInstanceId: string | null;
  expectedTag: string | null;
};

type CanvasPatchOperation =
  | { kind: "batch"; operations: CanvasPatchOperation[] }
  | { kind: "setAttributes"; target: CanvasPatchAnchor; attributes: Record<string, string | null> }
  | {
      kind: "setBlockOption";
      target: CanvasPatchAnchor;
      providerId: string;
      optionId: string;
      attribute: string;
      value: string | null;
    }
  | {
      kind: "setIcon";
      target: CanvasPatchAnchor;
      providerId: "icon";
      iconIdentity: string;
      attributes: Record<string, string | null>;
      childrenHtml: string;
    }
  | { kind: "setText"; target: CanvasPatchAnchor; text: string }
  | { kind: "setTextHtml"; target: CanvasPatchAnchor; escapedText: string }
  | { kind: "replaceTag"; target: CanvasPatchAnchor; newTag: string }
  | {
      kind: "insert";
      target: CanvasPatchAnchor;
      position: ProjectMovePosition;
      html: string;
      inserted: CanvasPatchAnchor | null;
    }
  | { kind: "move"; source: CanvasPatchAnchor; target: CanvasPatchAnchor; position: ProjectMovePosition }
  | {
      kind: "duplicate";
      source: CanvasPatchAnchor;
      html: string;
      inserted: CanvasPatchAnchor | null;
    }
  | { kind: "delete"; target: CanvasPatchAnchor };

export type CanvasPatch = {
  schemaVersion: 1;
  patchId: string;
  issuedAtMs: number;
  projectRoot: string;
  runtimeSessionId: string;
  baseWorkspaceRevision: number;
  workspaceRevision: number;
  workspaceTransactionId: string;
  beforeModelRevision: string;
  afterModelRevision: string;
  operation: CanvasPatchOperation;
};

type PreviewHtmlInsertDropExecutionStatus = "committed" | "blocked";

export type PreviewHtmlInsertDropExecutionInput = {
  intent: PreviewProjectionIntentInput;
  insertIntent: ProjectHtmlInsertIntent;
};

export type PreviewHtmlInsertDropExecutionReceipt = {
  schemaVersion: number;
  intent: PreviewProjectionIntentReceipt;
  status: PreviewHtmlInsertDropExecutionStatus;
  messageDiagnostic: LocalizedDiagnostic;
  modelRevision: string | null;
  patch: ProjectHtmlInsertPatch | null;
  canvasPatch: CanvasPatch | null;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  touchedFiles: string[];
  diagnostics: PreviewProjectionDiagnostic[];
};

export type ProjectHtmlAttributePatch = {
  file: string;
  resolvedTargetId: string;
  beforeRevision: string;
  afterRevision: string;
  contents: string;
  targetLocation: ProjectSourceEditLocation;
  sourceStartLine: number;
  tag: string;
  attributes: Record<string, string | null>;
  zolaImageContract: boolean;
  zolaImage: ZolaImagePresentation | null;
  managedIcon: ProjectManagedIconPatch | null;
  generatedIdentity: ProjectGeneratedIdentityProjection | null;
};

type ProjectManagedIconPatch = {
  state: NativeIconState;
  previousState: NativeIconState;
  previousAttributes: Record<string, string | null>;
  childrenHtml: string;
  previousChildrenHtml: string;
};

type PreviewHtmlAttributesExecutionStatus = "committed" | "blocked";

export type PreviewHtmlAttributesExecutionInput = {
  intent: PreviewProjectionIntentInput;
  attributeIntent: ProjectHtmlAttributeIntent;
};

export type PreviewHtmlAttributesExecutionReceipt = {
  schemaVersion: number;
  intent: PreviewProjectionIntentReceipt;
  status: PreviewHtmlAttributesExecutionStatus;
  messageDiagnostic: LocalizedDiagnostic;
  modelRevision: string | null;
  patch: ProjectHtmlAttributePatch | null;
  canvasPatch: CanvasPatch | null;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  touchedFiles: string[];
  diagnostics: PreviewProjectionDiagnostic[];
};

export type PreviewSelectionBatchAction =
  | { kind: "setAttributes"; attributes: ProjectHtmlAttributeMutation[] }
  | { kind: "mutateClasses"; add: string[]; remove: string[] }
  | { kind: "generateSharedClass" }
  | { kind: "duplicate" }
  | { kind: "delete" }
  | {
      kind: "move";
      targetSourceId: string;
      targetTag?: string | null;
      position: ProjectMovePosition;
    };

export type PreviewSelectionBatchExecutionInput = {
  schemaVersion: 1;
  action: PreviewSelectionBatchAction;
};

export type PreviewSelectionBatchExecutionReceipt = {
  schemaVersion: 1;
  status: "committed" | "blocked";
  modelRevision: string | null;
  affectedSourceIds: string[];
  primaryAffectedSourceId: string | null;
  generatedClass: string | null;
  canvasPatch: CanvasPatch | null;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  diagnostics: string[];
};

export type ProjectHtmlTextPatch = {
  file: string;
  resolvedTargetId: string;
  beforeRevision: string;
  afterRevision: string;
  contents: string;
  targetLocation: ProjectSourceEditLocation;
  sourceStartLine: number;
  lineShiftStart: number;
  lineShift: number;
  tag: string;
  text: string;
  previousEscapedText: string;
};

type PreviewHtmlTextExecutionStatus = "committed" | "blocked";

export type PreviewHtmlTextExecutionInput = {
  intent: PreviewProjectionIntentInput;
  textIntent: ProjectHtmlTextIntent;
  deferCanonicalProjection?: boolean;
  editSessionId?: string | null;
};

export type PreviewHtmlTextExecutionReceipt = {
  schemaVersion: number;
  intent: PreviewProjectionIntentReceipt;
  status: PreviewHtmlTextExecutionStatus;
  messageDiagnostic: LocalizedDiagnostic;
  modelRevision: string | null;
  patch: ProjectHtmlTextPatch | null;
  canvasPatch: CanvasPatch | null;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  touchedFiles: string[];
  diagnostics: PreviewProjectionDiagnostic[];
};

export type ProjectHtmlTagPatch = {
  file: string;
  resolvedTargetId: string;
  beforeRevision: string;
  afterRevision: string;
  contents: string;
  targetLocation: ProjectSourceEditLocation;
  sourceStartLine: number;
  lineShiftStart: number;
  lineShift: number;
  oldTag: string;
  newTag: string;
};

type PreviewHtmlTagExecutionStatus = "committed" | "blocked";

export type PreviewHtmlTagExecutionInput = {
  intent: PreviewProjectionIntentInput;
  tagIntent: ProjectHtmlTagIntent;
};

export type PreviewHtmlTagExecutionReceipt = {
  schemaVersion: number;
  intent: PreviewProjectionIntentReceipt;
  status: PreviewHtmlTagExecutionStatus;
  messageDiagnostic: LocalizedDiagnostic;
  modelRevision: string | null;
  patch: ProjectHtmlTagPatch | null;
  canvasPatch: CanvasPatch | null;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  touchedFiles: string[];
  diagnostics: PreviewProjectionDiagnostic[];
};

type ProjectHtmlDuplicatePatch = {
  file: string;
  resolvedSourceId: string;
  duplicatedLabel: string;
  beforeRevision: string;
  afterRevision: string;
  contents: string;
  sourceLocation: ProjectSourceEditLocation;
  insertedLocation: ProjectSourceEditLocation;
  sourceStartLine: number;
  sourceEndLine: number;
  insertedStartLine: number;
  lineShiftStart: number;
  lineShift: number;
  tag: string;
  html: string;
  blockIds: string[];
  dataAnimCount: number;
  duplicateIdCount: number;
  zolaImageContract: boolean;
  dynamicWidgetContract: boolean;
};

type PreviewHtmlDuplicateExecutionStatus = "committed" | "blocked";

export type PreviewHtmlDuplicateExecutionInput = {
  intent: PreviewProjectionIntentInput;
  duplicateIntent: ProjectHtmlDuplicateIntent;
};

export type PreviewHtmlDuplicateExecutionReceipt = {
  schemaVersion: number;
  intent: PreviewProjectionIntentReceipt;
  status: PreviewHtmlDuplicateExecutionStatus;
  messageDiagnostic: LocalizedDiagnostic;
  modelRevision: string | null;
  patch: ProjectHtmlDuplicatePatch | null;
  canvasPatch: CanvasPatch | null;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  touchedFiles: string[];
  diagnostics: PreviewProjectionDiagnostic[];
};

type ProjectHtmlDeletePatch = {
  file: string;
  resolvedTargetId: string;
  deletedLabel: string;
  beforeRevision: string;
  afterRevision: string;
  contents: string;
  targetLocation: ProjectSourceEditLocation;
  sourceStartLine: number;
  sourceEndLine: number;
  lineShiftStart: number;
  lineShift: number;
};

type PreviewHtmlDeleteExecutionStatus = "committed" | "blocked";

export type PreviewHtmlDeleteExecutionInput = {
  intent: PreviewProjectionIntentInput;
  deleteIntent: ProjectHtmlDeleteIntent;
};

export type PreviewHtmlDeleteExecutionReceipt = {
  schemaVersion: number;
  intent: PreviewProjectionIntentReceipt;
  status: PreviewHtmlDeleteExecutionStatus;
  messageDiagnostic: LocalizedDiagnostic;
  modelRevision: string | null;
  patch: ProjectHtmlDeletePatch | null;
  canvasPatch: CanvasPatch | null;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  touchedFiles: string[];
  diagnostics: PreviewProjectionDiagnostic[];
};

type ProjectTeraDeletePatch = {
  file: string;
  resolvedTargetId: string;
  deletedLabel: string;
  deletedKind: string;
  beforeRevision: string;
  afterRevision: string;
  contents: string;
  targetLocation: ProjectSourceEditLocation;
  sourceStartLine: number;
  sourceEndLine: number;
  lineShiftStart: number;
  lineShift: number;
};

type PreviewTeraDeleteExecutionStatus = "committed" | "blocked";

export type PreviewTeraDeleteExecutionInput = {
  intent: PreviewProjectionIntentInput;
  deleteIntent: ProjectTeraDeleteIntent;
};

export type PreviewTeraDeleteExecutionReceipt = {
  schemaVersion: number;
  intent: PreviewProjectionIntentReceipt;
  status: PreviewTeraDeleteExecutionStatus;
  messageDiagnostic: LocalizedDiagnostic;
  modelRevision: string | null;
  patch: ProjectTeraDeletePatch | null;
  canvasPatch: null;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  touchedFiles: string[];
  diagnostics: PreviewProjectionDiagnostic[];
};

type ProjectTeraInsertPatch = {
  file: string;
  resolvedTargetId: string;
  insertedLabel: string;
  insertedKind: string;
  beforeRevision: string;
  afterRevision: string;
  contents: string;
  targetLocation: ProjectSourceEditLocation;
  insertedLocation: ProjectSourceEditLocation;
  insertedStartLine: number;
  lineShiftStart: number;
  lineShift: number;
  snippet: string;
};

type PreviewTeraInsertDropExecutionStatus = "committed" | "blocked";

export type PreviewTeraInsertDropExecutionInput = {
  intent: PreviewProjectionIntentInput;
  insertIntent: ProjectTeraInsertIntent;
};

export type PreviewTeraInsertDropExecutionReceipt = {
  schemaVersion: number;
  intent: PreviewProjectionIntentReceipt;
  status: PreviewTeraInsertDropExecutionStatus;
  messageDiagnostic: LocalizedDiagnostic;
  modelRevision: string | null;
  patch: ProjectTeraInsertPatch | null;
  canvasPatch: null;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  touchedFiles: string[];
  diagnostics: PreviewProjectionDiagnostic[];
};
