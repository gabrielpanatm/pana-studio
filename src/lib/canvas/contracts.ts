import type {
  CssRuleMatch,
  CssVariableRow,
  StyleRow,
} from "$lib/css/contracts";
import type {
  EditorMovePlan,
  EditorNavigationEffectScope,
  EditorNavigationBoundaryKind,
  EditorNavigationComponentKind,
  EditorNavigationIdentity,
  EditorNavigationOrigin,
  EditorSourceProvenance,
  EditScopeGrant,
  HoverSnapshot,
  SelectionSnapshot,
} from "$lib/editor/contracts";
import type {
  SourceCapabilityReason,
  SourceEditLocation,
  SourceRange,
} from "$lib/source-graph/contracts";

export const CANVAS_INTERACTION_SCHEMA_VERSION = 3 as const;

export type CanvasInteractionIdentity = {
  canvas: EditorNavigationIdentity;
  route: string;
  documentEpoch: number;
  agentInstanceId: string;
};

export type CanvasInteractionGesture =
  | "pointerMove"
  | "pointerDown"
  | "click"
  | "contextMenu"
  | "dragStart"
  | "dragOver"
  | "drop";

export type CanvasPointerButton =
  | "none"
  | "primary"
  | "auxiliary"
  | "secondary"
  | "back"
  | "forward";

export type CanvasPointerModifiers = {
  alt: boolean;
  control: boolean;
  meta: boolean;
  shift: boolean;
};

export type CanvasPointerSample = {
  clientX: number;
  clientY: number;
  button: CanvasPointerButton;
  buttons: number;
  modifiers: CanvasPointerModifiers;
};

export type CanvasHitCandidateKind =
  | "renderInstance"
  | "boundaryInstance";

export type CanvasHitCandidate = {
  kind: CanvasHitCandidateKind;
  id: string;
};

export type CanvasDragPosition = "before" | "after" | "inside";

export type CanvasDragSample = {
  sessionId: string;
  position: CanvasDragPosition | null;
};

export type CanvasInteractionRequest = {
  schemaVersion: typeof CANVAS_INTERACTION_SCHEMA_VERSION;
  identity: CanvasInteractionIdentity;
  emittedAtMs: number;
  gestureSequence: number;
  gesture: CanvasInteractionGesture;
  pointer: CanvasPointerSample;
  hitPath: CanvasHitCandidate[];
  drag: CanvasDragSample | null;
};

type CanvasInteractionStatus =
  | "resolved"
  | "noTarget"
  | "stale"
  | "rejected";

type CanvasInteractionTargetKind =
  | "htmlElement"
  | "boundary"
  | "runtimeElement";

type CanvasInteractionScopeState =
  | "unscoped"
  | "locked"
  | "authorized";

type CanvasInteractionActions = {
  canSelect: boolean;
  canInspect: boolean;
  canOpenInCode: boolean;
  canEnterBoundary: boolean;
  canMoveAtomic: boolean;
  canMove: boolean;
  canEditText: boolean;
  canEditAttributes: boolean;
  readOnly: boolean;
  reasonCode: SourceCapabilityReason | null;
};

export type CanvasInteractionTarget = {
  editorNodeId: string;
  kind: CanvasInteractionTargetKind;
  boundaryKind: EditorNavigationBoundaryKind | null;
  componentKind: EditorNavigationComponentKind | null;
  label: string;
  tag: string | null;
  sourceNodeId: string | null;
  file: string | null;
  range: SourceRange | null;
  renderInstanceId: string | null;
  boundaryInstanceId: string | null;
  origin: EditorNavigationOrigin;
  themeName: string | null;
  sourceProvenance: EditorSourceProvenance;
  requiredEditScopeId: string | null;
  scopeState: CanvasInteractionScopeState;
  effectScope: EditorNavigationEffectScope;
  renderedInstanceCount: number;
  actions: CanvasInteractionActions;
};

export type CanvasOverlayProjection = {
  primaryRenderInstanceId: string | null;
  renderInstanceIds: string[];
  boundaryInstanceId: string | null;
};

type CanvasInteractionDiagnosticCode =
  | "protocol_version_mismatch"
  | "snapshot_binding_mismatch"
  | "canvas_identity_mismatch"
  | "route_mismatch"
  | "document_epoch_mismatch"
  | "agent_instance_mismatch"
  | "agent_binding_missing"
  | "gesture_sequence_stale"
  | "invalid_pointer"
  | "invalid_identity"
  | "hit_path_too_large"
  | "invalid_hit_candidate"
  | "duplicate_hit_candidate"
  | "invalid_drag_sample"
  | "unknown_hit_candidate"
  | "candidate_not_selectable";

type CanvasInteractionDiagnostic = {
  code: CanvasInteractionDiagnosticCode;
  message: string;
  candidateId: string | null;
};

export type CanvasInteractionReceipt = {
  schemaVersion: typeof CANVAS_INTERACTION_SCHEMA_VERSION;
  identity: CanvasInteractionIdentity;
  gestureSequence: number;
  gesture: CanvasInteractionGesture;
  status: CanvasInteractionStatus;
  target: CanvasInteractionTarget | null;
  overlay: CanvasOverlayProjection | null;
  dragPosition: CanvasDragPosition | null;
  diagnostics: CanvasInteractionDiagnostic[];
};

export type CanvasInteractionBindingReceipt = {
  schemaVersion: typeof CANVAS_INTERACTION_SCHEMA_VERSION;
  identity: CanvasInteractionIdentity;
  lastAcceptedSequence: number;
  activeDocumentPath: string | null;
  authoringSurfaces: CanvasInteractionAuthoringSurface[];
};

type CanvasInteractionAuthoringSurface = {
  sourceNodeId: string;
  boundaryInstanceId: string;
  renderInstanceId: string | null;
};

export type CanvasInteractionResolveInput = {
  request: CanvasInteractionRequest;
  editScopeGrant: EditScopeGrant | null;
};

type CanvasDragOverTimings = {
  emittedAtMs: number;
  rustReceivedAtMs: number;
  rustCompletedAtMs: number;
  inputToPlanDurationMs: number;
  inputToFirstAllowedPlanMs: number | null;
  rustDurationMs: number;
};

export type CanvasDragOverResolveInput = {
  request: CanvasInteractionRequest;
  sourceNodeId: string;
  editScopeGrant: EditScopeGrant | null;
};

export type CanvasDragOverReceipt = {
  schemaVersion: typeof CANVAS_INTERACTION_SCHEMA_VERSION;
  interaction: CanvasInteractionReceipt;
  plan: EditorMovePlan | null;
  timings: CanvasDragOverTimings;
};

export type CanvasHoverReceipt = {
  schemaVersion: typeof CANVAS_INTERACTION_SCHEMA_VERSION;
  interaction: CanvasInteractionReceipt;
  projection: {
    changed: boolean;
    hover: HoverSnapshot | null;
  } | null;
  timings: {
    emittedAtMs: number;
    rustReceivedAtMs: number;
    rustCompletedAtMs: number;
    inputToProjectionDurationMs: number;
    rustDurationMs: number;
  };
};

export type PageSection = {
  selector: string;
  label: string;
  tag: string;
  depth: number;
  sourceLocation?: SourceEditLocation | null;
  sourceId?: string | null;
  templateSourceId?: string | null;
  sessionId?: string | null;
};

type DomNodeLink = {
  selector: string;
  label: string;
  tag: string;
};

export type BlockSelectionContext = {
  providerId: string;
  rootSelector: string;
  rootTag: string;
  sourceInstanceIds: string[];
  rootSourceId: string | null;
  rootTemplateSourceId: string | null;
  rootSessionId: string | null;
};

type CanvasBlockObservation = {
  providerId: string;
  rootSelector: string;
  rootTag: string;
};

/**
 * Bounded facts read from the currently rendered DOM element.
 *
 * This object is never selection authority. It intentionally contains no
 * project/session/source identity; those fields live in SelectionSnapshot.
 */
export type CanvasElementObservation = {
  selector: string;
  cssSelector: string;
  domPath: string;
  tag: string;
  id: string;
  href: string;
  title: string;
  alt: string;
  classes: string[];
  text: string;
  rawText: string;
  hasChildElements: boolean;
  rect: {
    width: string;
    height: string;
    top: string;
    left: string;
  };
  styles: StyleRow[];
  variables: CssVariableRow[];
  matchedRules: CssRuleMatch[];
  imageSrc: string | null;
  zolaImage: ZolaImagePresentation | null;
  attributes: Record<string, string>;
  parentNode: DomNodeLink | null;
  childNodes: DomNodeLink[];
  blockContext: CanvasBlockObservation | null;
};

export type AcceptedCanvasElementObservation = {
  selectionRevision: number;
  canvasIdentity: EditorNavigationIdentity;
  documentEpoch: number;
  renderInstanceId: string;
  observation: CanvasElementObservation;
};

export type CoordinatedElementSelection = {
  snapshot: SelectionSnapshot;
  documentEpoch: number;
  renderInstanceId: string;
  sourceNodeId: string | null;
  sourceLocation: SourceEditLocation | null;
  observation: CanvasElementObservation;
};

export type InspectorHtmlPhysicalFacts = {
  selectionRevision: number;
  renderInstanceId: string;
  rect: {
    width: string;
    height: string;
    top: string;
    left: string;
  };
  hasChildElements: boolean;
  childElementCount: number;
  zolaImage: ZolaImagePresentation | null;
};

export type ZolaImageOperation = "fit_width" | "fit" | "fill";

export type ZolaImageFormat = "auto" | "webp" | "avif" | "jpg" | "png";

export type ZolaImageFilter = "nearest" | "triangle" | "catmull_rom" | "gaussian" | "lanczos3";

export type ZolaImagePresentation = {
  sourceUrl: string;
  sourcePath: string;
  width: number;
  height: number | null;
  operation: ZolaImageOperation;
  format: ZolaImageFormat;
  quality: number;
  filter: ZolaImageFilter | null;
};

export type EditableAttributes = Record<string, string>;

export type InspectorPendingArea = "html" | "css" | "js";

export type HtmlPendingArea = "tag" | "attributes" | "text" | "image" | "classes" | "structure";
