import type { LocalizedDiagnostic } from "$lib/contracts/localized-diagnostic";

type PageJsDraftEntry = {
  templatePath: string;
  base: PageJsConfig;
  current: PageJsConfig;
  cachebustAssets: boolean;
  source: string;
  coalesceKey: string | null;
  transactionId: string | null;
  updatedAtMs: number;
  revision: number;
  baseConfigBytes: number;
  currentConfigBytes: number;
  retainedConfigBytes: number;
};

type PageJsDraftStoreLimits = {
  maxDrafts: number;
  maxConfigBytes: number;
  maxTotalConfigBytes: number;
};

export type PageJsDraftStoreSnapshot = {
  schemaVersion: 2;
  sessionId: string;
  runtimeSessionId: string;
  projectRoot: string;
  revision: number;
  dirtyCount: number;
  retainedConfigBytes: number;
  limits: PageJsDraftStoreLimits;
  drafts: PageJsDraftEntry[];
};

export type PageJsDraftStageInput = {
  templatePath: string;
  baseConfig: PageJsConfig;
  currentConfig: PageJsConfig;
  source?: string | null;
  coalesceKey?: string | null;
  transactionId?: string | null;
};

export type PageJsRequestIdentity = {
  expectedProjectRoot: string;
  expectedSessionId: string;
};

export type PageJsDraftSessionIdentity = PageJsRequestIdentity;

export type PageJsCommandReceipt<T> = {
  projectRoot: string;
  runtimeSessionId: string;
  payload: T;
};

export type PageJsWorkspaceState = {
  templatePath: string;
  motionRuntime: MotionRuntimeContract;
  accepted: PageJsConfig;
  current: PageJsConfig;
  dirty: boolean;
  entryRevision: number | null;
};

export type PageJsDraftStageReceipt = {
  schemaVersion: 2;
  status: "staged" | "cleared" | "unchanged";
  changed: boolean;
  dirty: boolean;
  templatePath: string;
  revision: number;
  entryRevision: number | null;
  dirtyCount: number;
  retainedConfigBytes: number;
  projectRoot: string;
  runtimeSessionId: string;
};

// ── JS / Motion tab types ─────────────────────────────────────────────────────

export type MotionTargetKind = "element" | "selector" | "trigger" | "relative" | "viewport" | "document";

export type MotionTargetRelation =
  | "selfElement"
  | "children"
  | "descendants"
  | "parent"
  | "ancestors"
  | "siblings"
  | "nextSibling"
  | "previousSibling";

type MotionTargetScope = "all" | "each" | "first";

export type MotionTarget = {
  kind: MotionTargetKind;
  dataAnim: string;
  selector: string;
  relation: MotionTargetRelation;
  scope: MotionTargetScope;
};

export type MotionTriggerCommand = "restart" | "play" | "pause" | "reverse" | "toggle" | "reset" | "none";

export type MotionTrigger =
  | { type: "load"; phase: "domReady" | "windowLoad" }
  | { type: "inView"; threshold: number; once: boolean }
  | {
      type: "click";
      firstClick: MotionTriggerCommand;
      secondClick: MotionTriggerCommand;
      preventDefault: boolean;
    }
  | { type: "hover"; enter: MotionTriggerCommand; leave: MotionTriggerCommand }
  | {
      type: "scroll";
      mode: "trigger" | "scrub";
      start: string;
      end: string;
      smoothMs: number;
      once: boolean;
    }
  | { type: "pointer"; axis: "x" | "y" | "both"; smoothMs: number; rest: number }
  | { type: "custom"; event: string; preventDefault: boolean };

type MotionConditions = {
  mediaQueries: Array<{ id: string; query: string; enabled: boolean }>;
  reducedMotion: "reduce" | "skipToEnd" | "disable";
};

type MotionPlayback = {
  delayMs: number;
  repeat: number;
  infinite: boolean;
  loopDelayMs: number;
  alternate: boolean;
  reversed: boolean;
  playbackRate: number;
  playbackEase: string;
};

export type MotionValue = {
  kind: "number" | "text" | "color" | "cssVariable" | "relative";
  value: string;
  unit: string;
};

export type MotionProperty = {
  id: string;
  name: string;
  category: "transform" | "style" | "cssVariable" | "htmlAttribute" | "svgAttribute" | "object";
  from?: MotionValue | null;
  to: MotionValue;
};

export type MotionKeyframe = {
  id: string;
  offset: number;
  ease: string;
  properties: MotionProperty[];
};

type MotionStagger = {
  amount: number;
  mode: "each" | "total";
  from: string;
  reversed: boolean;
  ease: string;
};

type MotionActionRepeat = {
  count: number;
  infinite: boolean;
  alternate: boolean;
  delayMs: number;
};

type MotionSpecialization =
  | { type: "splitText"; mode: "lines" | "words" | "chars" }
  | { type: "svgPath"; path: string; autoRotate: boolean }
  | { type: "svgMorph"; source: string; precision: number }
  | { type: "svgDraw" };

export type MotionAnimateAction = {
  type: "animate";
  id: string;
  name: string;
  enabled: boolean;
  target: MotionTarget;
  start: number;
  duration: number;
  mode: "from" | "to" | "fromTo";
  ease: string;
  properties: MotionProperty[];
  keyframes: MotionKeyframe[];
  stagger?: MotionStagger | null;
  repeat: MotionActionRepeat;
  specialization?: MotionSpecialization | null;
};

type MotionSetValue =
  | { type: "property"; name: string; value: MotionValue }
  | { type: "attribute"; name: string; value: string }
  | { type: "addClass"; name: string }
  | { type: "removeClass"; name: string }
  | { type: "toggleClass"; name: string };

export type MotionSetAction = {
  type: "set";
  id: string;
  name: string;
  enabled: boolean;
  target: MotionTarget;
  start: number;
  values: MotionSetValue[];
};

type MotionMediaAction = {
  type: "media";
  id: string;
  name: string;
  enabled: boolean;
  target: MotionTarget;
  start: number;
  command: "play" | "pause" | "toggle" | "reset";
};

type MotionCallAction = {
  type: "call";
  id: string;
  name: string;
  enabled: boolean;
  start: number;
  code: string;
};

type MotionNestedAction = {
  type: "nested";
  id: string;
  name: string;
  enabled: boolean;
  start: number;
  duration: number;
  interactionId: string;
};

export type MotionAction =
  | MotionAnimateAction
  | MotionSetAction
  | MotionMediaAction
  | MotionCallAction
  | MotionNestedAction;

export type MotionInteraction = {
  id: string;
  name: string;
  enabled: boolean;
  trigger: MotionTrigger;
  triggerTarget: MotionTarget;
  conditions: MotionConditions;
  playback: MotionPlayback;
  domain: "time" | "progress";
  actions: MotionAction[];
  markers: Array<{ id: string; name: string; at: number }>;
};

export type MotionBehavior =
  | {
      type: "draggable";
      id: string;
      name: string;
      enabled: boolean;
      target: MotionTarget;
      axis: "x" | "y" | "both";
      container: string;
      snap: number;
      friction: number;
      cursor: boolean;
    }
  | {
      type: "layout";
      id: string;
      name: string;
      enabled: boolean;
      target: MotionTarget;
      childrenSelector: string;
      properties: string[];
      durationMs: number;
      ease: string;
    };

type MotionCustomCode = {
  id: string;
  name: string;
  enabled: boolean;
  code: string;
};

export type MotionRuntimeContract = {
  schemaVersion: number;
  animeVersion: string;
};

export type MotionDocument = {
  schemaVersion: 2;
  animeVersion: string;
  interactions: MotionInteraction[];
  behaviors: MotionBehavior[];
  customCode: MotionCustomCode[];
};

export type PageJsConfig = {
  motion?: MotionDocument | null;
};

export type MotionMutation =
  | { command: "createInteraction"; interaction: MotionInteraction }
  | { command: "updateInteraction"; interaction: MotionInteraction }
  | { command: "deleteInteraction"; interactionId: string }
  | { command: "insertAction"; interactionId: string; index: number; action: MotionAction }
  | { command: "updateAction"; interactionId: string; action: MotionAction }
  | { command: "deleteAction"; interactionId: string; actionId: string }
  | { command: "setActionTiming"; interactionId: string; actionId: string; start?: number; duration?: number }
  | { command: "reorderAction"; interactionId: string; actionId: string; index: number }
  | { command: "upsertBehavior"; behavior: MotionBehavior }
  | { command: "deleteBehavior"; behaviorId: string }
  | { command: "upsertCustomCode"; customCode: MotionCustomCode }
  | { command: "deleteCustomCode"; customCodeId: string }
  | { command: "replaceDocument"; document: MotionDocument };

type MotionMutationTransaction = {
  schemaVersion: 3;
  id: string;
  command: string;
  beforeConfigHash: string;
  afterConfigHash: string;
  forward: MotionMutation;
  inverse: MotionMutation;
};

type MotionDiagnostic = {
  severity: "error" | "warning";
  path: string;
  diagnostic: LocalizedDiagnostic;
};

type MotionMutationReceipt = {
  schemaVersion: 3;
  command: string;
  changed: boolean;
  config: PageJsConfig;
  diagnostics: MotionDiagnostic[];
  transaction: MotionMutationTransaction | null;
};

export type MotionPageMutationInput = {
  templatePath: string;
  expectedProjectRoot: string;
  expectedSessionId: string;
  expectedEntryRevision: number | null;
  mutation: MotionMutation;
};

export type MotionPageMutationReceipt = {
  mutation: MotionMutationReceipt;
  pageJs: PageJsDraftStageReceipt;
  workspaceRevision: number;
};
