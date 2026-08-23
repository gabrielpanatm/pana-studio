import type { NativeBlockSlotMutationContext } from "$lib/blocks/contracts";
import type {
  CanvasPatch,
  ProjectMovePosition,
} from "$lib/preview/contracts";
import type { ProjectWorkspaceMutationReceipt } from "$lib/project/workspace-contract";
import type {
  SourceCapabilityReason,
  SourceNodeKind,
  SourceRange,
} from "$lib/source-graph/contracts";

export const EDITOR_NAVIGATION_SCHEMA_VERSION = 4 as const;

export type EditorNavigationIdentity = {
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number;
  transactionId: string;
  previewRevision: string;
};

type EditorNavigationSurface =
  | "canonicalPreview"
  | "templateWorkbench";

type EditorNavigationNodeKind =
  | "htmlElement"
  | "boundary"
  | "runtimeElement";

export type EditorNavigationBoundaryKind =
  | "template"
  | "component"
  | "markdown";

export type EditorNavigationComponentKind =
  | "partial"
  | "macro"
  | "shortcode"
  | "repeat"
  | "conditional"
  | "transform";

type EditorNavigationViewNodeKind =
  | "htmlElement"
  | "boundary"
  | "relation"
  | "slot"
  | "source";

type EditorNavigationRelationKind =
  | "extends"
  | "include"
  | "import"
  | "blockOverride";

export type EditorNavigationOrigin =
  | "project"
  | "theme"
  | "tera"
  | "panaRuntime"
  | "arbitraryRuntime";

export type EditorNavigationEffectScope =
  | "singleSource"
  | "sharedDefinition"
  | "allRenderedInstances";

type EditorSourceResolution =
  | "direct"
  | "resolved"
  | "fallbackResolved"
  | "ambiguous"
  | "dynamic"
  | "external"
  | "unresolved";

export type EditorSourceReference = {
  sourceNodeId: string | null;
  sourceKind: SourceNodeKind | null;
  file: string;
  range: SourceRange | null;
  label: string;
  origin: EditorNavigationOrigin;
  themeName: string | null;
  canOpenInCode: boolean;
};

export type EditorSourceProvenance = {
  definition: EditorSourceReference | null;
  composition: EditorSourceReference | null;
  resolution: EditorSourceResolution;
};

type EditorNavigationCapabilities = {
  canSelect: boolean;
  canInspect: boolean;
  canOpenInCode: boolean;
  canEnterBoundary: boolean;
  canMoveAtomic: boolean;
  canMove: boolean;
  canEditText: boolean;
  canEditAttributes: boolean;
  readOnly: boolean;
  requiresEditScopeId: string | null;
  reasonCode: SourceCapabilityReason | null;
};

type EditorNavigationBoundary = {
  kind: EditorNavigationBoundaryKind;
  componentKind: EditorNavigationComponentKind | null;
  boundaryInstanceId: string;
  sourceNodeId: string;
  rootRenderInstanceIds: string[];
  atomicWhenClosed: boolean;
  effectScope: EditorNavigationEffectScope;
  renderedInstanceCount: number;
  target: string | null;
  empty: boolean;
};

export type EditorNavigationNode = {
  id: string;
  parentId: string | null;
  children: string[];
  order: number;
  kind: EditorNavigationNodeKind;
  label: string;
  tag: string | null;
  sourceNodeId: string | null;
  renderInstanceId: string | null;
  sourceKind: SourceNodeKind | null;
  file: string | null;
  range: SourceRange | null;
  origin: EditorNavigationOrigin;
  themeName: string | null;
  sourceProvenance: EditorSourceProvenance;
  provenanceStack: string[];
  componentDefinitionIds: string[];
  componentInvocationIds: string[];
  blockDefinitionIds: string[];
  blockSourceInstanceIds: string[];
  dynamicWidgetProviderIds: string[];
  dynamicWidgetSourceInstanceIds: string[];
  bindingKey: string | null;
  bindingPath: string | null;
  boundary: EditorNavigationBoundary | null;
  capabilities: EditorNavigationCapabilities;
};

type EditorNavigationRelation = {
  kind: EditorNavigationRelationKind;
  targetDocumentPath: string | null;
  targetSourceNodeId: string | null;
  targetTemplateName: string | null;
};

export type EditorNavigationViewNode = {
  id: string;
  editorNodeId: string | null;
  parentId: string | null;
  children: string[];
  order: number;
  kind: EditorNavigationViewNodeKind;
  label: string;
  tag: string | null;
  sourceNodeId: string | null;
  sourceKind: SourceNodeKind | null;
  file: string;
  origin: EditorNavigationOrigin;
  themeName: string | null;
  renderInstanceIds: string[];
  boundary: EditorNavigationBoundary | null;
  relation: EditorNavigationRelation | null;
  capabilities: EditorNavigationCapabilities;
};

type EditorNavigationBreadcrumb = {
  documentPath: string;
  templateName: string;
  sourceNodeId: string;
  origin: EditorNavigationOrigin;
  themeName: string | null;
  current: boolean;
};

type EditorNavigationView = {
  activeDocumentPath: string;
  activeTemplateName: string;
  activeSourceNodeId: string;
  breadcrumbs: EditorNavigationBreadcrumb[];
  rootNodeIds: string[];
  nodes: EditorNavigationViewNode[];
  previewContextRenderInstanceId: string | null;
};

type EditorNavigationDiagnostic = {
  code: string;
  message: string;
  sourceNodeId: string | null;
};

export type EditorNavigationSnapshot = {
  schemaVersion: typeof EDITOR_NAVIGATION_SCHEMA_VERSION;
  identity: EditorNavigationIdentity;
  modelRevision: string;
  route: string;
  surface: EditorNavigationSurface;
  rootNodeIds: string[];
  nodes: EditorNavigationNode[];
  focusedView: EditorNavigationView | null;
  diagnostics: EditorNavigationDiagnostic[];
};

export const EDIT_SCOPE_GRANT_SCHEMA_VERSION = 2 as const;

type EditScopeOperation =
  | "moveHtmlInside"
  | "editTextInside"
  | "editAttributesInside"
  | "inspectSharedDefinition";

export type EditScopeGrant = {
  schemaVersion: typeof EDIT_SCOPE_GRANT_SCHEMA_VERSION;
  token: string;
  scopeId: string;
  boundaryInstanceId: string;
  sourceNodeId: string;
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number;
  modelRevision: string;
  previewRevision: string;
  canvasTransactionId: string;
  route: string;
  activeDocumentPath: string;
  operations: EditScopeOperation[];
  issuedAtMs: number;
};

export const EDITOR_MOVE_PLAN_SCHEMA_VERSION = 3 as const;

export const EDITOR_MOVE_EXECUTION_SCHEMA_VERSION = 3 as const;

export const EDITOR_MOVE_LIVE_PROJECTION_SCHEMA_VERSION = 1 as const;

type EditorMoveOperation =
  | "htmlSourceMove"
  | "atomicTeraMove"
  | "componentMove"
  | "blockMove";

type EditorMoveImpact = {
  files: string[];
  editScopeId: string | null;
  effectScope: EditorNavigationEffectScope;
  renderedInstanceCount: number;
  affectsAllRenderedInstances: boolean;
  requiresPreviewReprojection: boolean;
};

type EditorMoveLiveProjectionReason =
  | "ready"
  | "planBlocked"
  | "executionNotHtml"
  | "missingRenderIdentity"
  | "ambiguousSourceIdentity"
  | "multipleRenderedInstances";

type EditorMoveLiveProjection = {
  schemaVersion: typeof EDITOR_MOVE_LIVE_PROJECTION_SCHEMA_VERSION;
  operation: "move";
  scope: "selectedInstance";
  planToken: string | null;
  identity: EditorNavigationIdentity;
  sourceRenderInstanceId: string;
  targetRenderInstanceId: string;
  position: ProjectMovePosition;
  rollback: {
    sourceParentRenderInstanceId: string | null;
    sourceNextSiblingRenderInstanceId: string | null;
  };
};

export type EditorMovePlan = {
  schemaVersion: typeof EDITOR_MOVE_PLAN_SCHEMA_VERSION;
  token: string | null;
  allowed: boolean;
  reasonCode: string | null;
  reason: string | null;
  operation: EditorMoveOperation | null;
  identity: EditorNavigationIdentity;
  modelRevision: string;
  route: string;
  activeDocumentPath: string;
  sourceNodeId: string;
  targetNodeId: string;
  position: ProjectMovePosition;
  impact: EditorMoveImpact;
  liveProjection: EditorMoveLiveProjection | null;
  liveProjectionReason: EditorMoveLiveProjectionReason;
  issuedAtMs: number;
};

export type EditorMovePlanInput = {
  identity: EditorNavigationIdentity;
  route: string;
  activeDocumentPath: string;
  previewContextRenderInstanceId?: string | null;
  sourceNodeId: string;
  targetNodeId: string;
  position: ProjectMovePosition;
  editScopeGrant?: EditScopeGrant | null;
  nativeBlockSlot?: NativeBlockSlotMutationContext | null;
};

export type EditorMoveCommitInput = {
  identity: EditorNavigationIdentity;
  route: string;
  activeDocumentPath: string;
  previewContextRenderInstanceId?: string | null;
  planToken: string;
  inputEmittedAtMs?: number;
  editScopeGrant?: EditScopeGrant | null;
};

type EditorMoveExecutionStatus = "committed" | "blocked";

export type EditorMoveExecutionReceipt = {
  schemaVersion: typeof EDITOR_MOVE_EXECUTION_SCHEMA_VERSION;
  planToken: string;
  projectRoot: string;
  runtimeSessionId: string;
  status: EditorMoveExecutionStatus;
  operation: EditorMoveOperation;
  modelRevision: string | null;
  projectedSourceId: string | null;
  canvasPatch: CanvasPatch | null;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  touchedFiles: string[];
  diagnostic: string | null;
  timings: EditorMoveTimings | null;
};

type EditorMoveTimings = {
  inputEmittedAtMs: number;
  planIssuedAtMs: number;
  rustReceivedAtMs: number;
  rustCompletedAtMs: number;
  inputToReceiptMs: number;
  pointerUpToCommitReceiptMs: number;
  planToReceiptMs: number;
  rustCommandMs: number;
  patchIssuedToReceiptMs: number | null;
  candidateCloneMs: number;
  mutationMs: number;
  recoveryPersistMs: number;
  authorityPublishMs: number;
  authorityTransactionMs: number;
  planRevalidationMs: number;
  nativeBlockContractMs: number;
  workspaceStageMs: number;
  afterProjectModelBuildMs: number;
  projectModelTemplateParseUs: number;
  projectModelComponentGraphUs: number;
  projectModelBlockGraphUs: number;
  projectModelContentModelUs: number;
  projectModelListingItemsUs: number;
  projectModelListingItemsReused: boolean;
  projectModelDynamicWidgetUs: number;
  projectModelMarkdownUs: number;
  projectModelNodeIndexUs: number;
};

export const SELECTION_COORDINATOR_SCHEMA_VERSION = 3 as const;

type SelectionSubjectKind =
  | "htmlElement"
  | "boundary"
  | "runtimeElement"
  | "cssRule";

type SelectionSubject = {
  kind: SelectionSubjectKind;
  boundaryKind: EditorNavigationBoundaryKind | null;
  componentKind: EditorNavigationComponentKind | null;
  tag: string | null;
  label: string;
};

type SelectionFocus =
  | { kind: "element" }
  | {
      kind: "cssRule";
      file: string;
      selector: string;
      viewport: string | null;
      range?: SourceRange | null;
    }
  | {
      kind: "cssProperty";
      file: string;
      selector: string;
      property: string;
      viewport: string | null;
      range?: SourceRange | null;
    }
  | {
      kind: "jsBehavior";
      file: string;
      behaviorId: string | null;
    };

export type SelectionResolution =
  | "cleared"
  | "resolved"
  | "notRendered"
  | "ambiguous";

export type SelectionAnchor = {
  editorNodeId: string | null;
  sourceNodeId: string | null;
  renderInstanceId: string | null;
  renderInstanceIds: string[];
  boundaryInstanceId: string | null;
  file: string | null;
  range: SourceRange | null;
  provenanceStack: string[];
  componentInvocationIds: string[];
  blockSourceInstanceIds: string[];
  dynamicWidgetSourceInstanceIds: string[];
  bindingKey: string | null;
  bindingPath: string | null;
};

export type SelectionEntry = {
  memberId: string;
  resolution: SelectionResolution;
  subject: SelectionSubject;
  anchor: SelectionAnchor;
  provenance: EditorSourceProvenance;
  capabilities: EditorNavigationCapabilities;
  diagnostics: string[];
};

type SelectionAggregateCapabilities = {
  memberCount: number;
  allResolved: boolean;
  allSourceBacked: boolean;
  sameFile: boolean;
  sameParent: boolean;
  hasAncestorDescendant: boolean;
  hasDuplicateSourceTargets: boolean;
  canBatchAttributes: boolean;
  canBatchDuplicate: boolean;
  canBatchDelete: boolean;
  canBatchMove: boolean;
  primaryOnlyEditsAllowed: boolean;
  primaryOnlyReasonCode: string | null;
  reasons: string[];
};

type SelectionAggregateHtmlFacts = {
  complete: boolean;
  commonClasses: string[];
  mixedClasses: string[];
  commonAttributes: Record<string, string | null>;
  mixedAttributeNames: string[];
};

export type SelectionSnapshot = {
  schemaVersion: typeof SELECTION_COORDINATOR_SCHEMA_VERSION;
  selectionRevision: number;
  projectRoot: string;
  runtimeSessionId: string;
  canvasIdentity: EditorNavigationIdentity;
  route: string;
  activeDocumentPath: string | null;
  primaryMemberId: string | null;
  rangeOriginMemberId: string | null;
  members: SelectionEntry[];
  aggregateCapabilities: SelectionAggregateCapabilities;
  aggregateHtmlFacts: SelectionAggregateHtmlFacts;
  focus: SelectionFocus;
  diagnostics: string[];
};

export type HoverSnapshot = {
  schemaVersion: typeof SELECTION_COORDINATOR_SCHEMA_VERSION;
  hoverRevision: number;
  canvasIdentity: EditorNavigationIdentity;
  route: string;
  documentEpoch: number;
  editorNodeId: string;
  subjectKind: SelectionSubjectKind;
  boundaryKind: EditorNavigationBoundaryKind | null;
  componentKind: EditorNavigationComponentKind | null;
  primaryRenderInstanceId: string | null;
  renderInstanceIds: string[];
  boundaryInstanceId: string | null;
};

export type SelectionCoordinatorSnapshot = {
  schemaVersion: typeof SELECTION_COORDINATOR_SCHEMA_VERSION;
  selection: SelectionSnapshot;
  hover: HoverSnapshot | null;
  inspectorSummary: InspectorSelectionSummarySnapshot;
};

export type SelectionIntent =
  | { kind: "selectEditorNode"; editorNodeId: string }
  | { kind: "toggleEditorNode"; editorNodeId: string }
  | { kind: "extendRangeToEditorNode"; editorNodeId: string }
  | { kind: "setPrimaryEditorNode"; editorNodeId: string }
  | {
      kind: "selectSourcePosition";
      file: string;
      offset: number;
      viewport?: string | null;
    }
  | {
      kind: "setFocus";
      focus: SelectionFocus;
      expectedSelectionRevision?: number | null;
    }
  | { kind: "clearSelection" }
  | { kind: "rebase" }
  | { kind: "setHover"; editorNodeId: string; documentEpoch: number }
  | { kind: "clearHover"; documentEpoch: number };

export type SelectionObservationInput = {
  schemaVersion: typeof SELECTION_COORDINATOR_SCHEMA_VERSION;
  selectionRevision: number;
  canvasIdentity: EditorNavigationIdentity;
  documentEpoch: number;
  renderInstanceId: string;
  inspectorFacts: InspectorSelectionPhysicalFacts;
};

export type InspectorSelectionSummaryState =
  | "empty"
  | "resolving"
  | "resolved"
  | "notRendered"
  | "ambiguous"
  | "uninspectable";

type InspectorSelectionSummaryReason =
  | "noSelection"
  | "awaitingPhysicalFacts"
  | "selectionNotRendered"
  | "selectionAmbiguous"
  | "inspectionDisabled"
  | "missingRenderInstance";

type InspectorSelectionBlockContext = {
  providerId: string;
  rootTag: string;
};

type InspectorSelectionPhysicalFacts = {
  observedTag: string;
  elementId: string;
  classes: string[];
  blockContext: InspectorSelectionBlockContext | null;
};

type InspectorSelectionSummaryDiagnostic = {
  code: InspectorSelectionSummaryReason;
  message: string;
};

export type InspectorSelectionSummarySnapshot = {
  schemaVersion: typeof SELECTION_COORDINATOR_SCHEMA_VERSION;
  projectRoot: string;
  runtimeSessionId: string;
  selectionRevision: number;
  canvasIdentity: EditorNavigationIdentity;
  documentEpoch: number | null;
  renderInstanceId: string | null;
  state: InspectorSelectionSummaryState;
  subjectKind: SelectionSubjectKind | null;
  boundaryKind: EditorNavigationBoundaryKind | null;
  componentKind: EditorNavigationComponentKind | null;
  tag: string | null;
  label: string | null;
  selector: string | null;
  elementId: string | null;
  classes: string[];
  blockContext: InspectorSelectionBlockContext | null;
  activeCssClass: string | null;
  canInspect: boolean;
  reason: InspectorSelectionSummaryReason | null;
  diagnostics: InspectorSelectionSummaryDiagnostic[];
};

export type SelectionObservationReceipt = {
  schemaVersion: typeof SELECTION_COORDINATOR_SCHEMA_VERSION;
  selectionRevision: number;
  canvasIdentity: EditorNavigationIdentity;
  documentEpoch: number;
  renderInstanceId: string;
  inspectorSummary: InspectorSelectionSummarySnapshot;
};
