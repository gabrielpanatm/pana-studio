import type {
  MotionAction,
  MotionAnimateAction,
  MotionBehavior,
  MotionDocument,
  MotionInteraction,
  MotionProperty,
  MotionTarget,
  MotionTrigger,
  MotionValue,
} from "$lib/types";

export const MOTION_SCHEMA_VERSION = 2 as const;
export const MOTION_ANIME_VERSION = "4.4.1" as const;

export function motionId(prefix: string): string {
  const value = globalThis.crypto?.randomUUID?.()
    ?? `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
  return `${prefix}-${value}`;
}

export function emptyMotionDocument(): MotionDocument {
  return {
    schemaVersion: MOTION_SCHEMA_VERSION,
    animeVersion: MOTION_ANIME_VERSION,
    interactions: [],
    behaviors: [],
    customCode: [],
  };
}

export function normalizeMotionDocument(
  value: MotionDocument | null | undefined,
): MotionDocument {
  if (!value) return emptyMotionDocument();
  return structuredClone(value);
}

export function isMotionDocumentEmpty(value: MotionDocument | null | undefined): boolean {
  return !value
    || (
      value.interactions.length === 0
      && value.behaviors.length === 0
      && value.customCode.length === 0
    );
}

export function targetForDataAnim(dataAnim: string): MotionTarget {
  return {
    kind: "element",
    dataAnim,
    selector: "",
    relation: "selfElement",
    scope: "all",
  };
}

export function triggerTarget(): MotionTarget {
  return {
    kind: "trigger",
    dataAnim: "",
    selector: "",
    relation: "selfElement",
    scope: "all",
  };
}

export function defaultMotionTrigger(type: MotionTrigger["type"]): MotionTrigger {
  switch (type) {
    case "inView":
      return { type, threshold: 0.15, once: true };
    case "click":
      return {
        type,
        firstClick: "restart",
        secondClick: "reverse",
        preventDefault: false,
      };
    case "hover":
      return { type, enter: "restart", leave: "reverse" };
    case "scroll":
      return {
        type,
        mode: "scrub",
        start: "bottom top",
        end: "top bottom",
        smoothMs: 120,
        once: false,
      };
    case "pointer":
      return { type, axis: "x", smoothMs: 50, rest: 0.5 };
    case "custom":
      return { type, event: "pana-motion", preventDefault: false };
    default:
      return { type: "load", phase: "domReady" };
  }
}

export function motionValue(value: string, unit = "", kind: MotionValue["kind"] = "number"): MotionValue {
  return { kind, value, unit };
}

function property(
  name: string,
  from: MotionValue | null,
  to: MotionValue,
  category: MotionProperty["category"] = "transform",
): MotionProperty {
  return {
    id: motionId("property"),
    name,
    category,
    from,
    to,
  };
}

export type MotionPreset = "fade" | "slideUp" | "scale" | "custom";

export function createAnimateAction(
  preset: MotionPreset,
  target: MotionTarget = triggerTarget(),
): MotionAnimateAction {
  const properties = preset === "slideUp"
    ? [
        property("translateY", motionValue("24", "px"), motionValue("0", "px")),
        property("opacity", motionValue("0"), motionValue("1"), "style"),
      ]
    : preset === "scale"
      ? [
          property("scale", motionValue("0.92"), motionValue("1")),
          property("opacity", motionValue("0"), motionValue("1"), "style"),
        ]
      : preset === "custom"
        ? [property("translateX", motionValue("0", "px"), motionValue("80", "px"))]
        : [property("opacity", motionValue("0"), motionValue("1"), "style")];
  return {
    type: "animate",
    id: motionId("action"),
    name: preset === "slideUp"
      ? "Slide up"
      : preset === "scale"
        ? "Scale in"
        : preset === "custom"
          ? "Animation"
          : "Fade in",
    enabled: true,
    target,
    start: 0,
    duration: 600,
    mode: "fromTo",
    ease: "out(3)",
    properties,
    keyframes: [],
    stagger: null,
    repeat: {
      count: 0,
      infinite: false,
      alternate: false,
      delayMs: 0,
    },
    specialization: null,
  };
}

export function createMotionInteraction(
  dataAnim: string,
  triggerType: MotionTrigger["type"],
  preset: MotionPreset,
): MotionInteraction {
  const trigger = defaultMotionTrigger(triggerType);
  const domain = (
    trigger.type === "pointer"
    || (trigger.type === "scroll" && trigger.mode === "scrub")
  ) ? "progress" : "time";
  const action = createAnimateAction(preset);
  if (domain === "progress") {
    action.duration = 100;
  }
  return {
    id: motionId("interaction"),
    name: `${triggerLabel(trigger)} · ${dataAnim}`,
    enabled: true,
    trigger,
    triggerTarget: targetForDataAnim(dataAnim),
    conditions: {
      mediaQueries: [],
      reducedMotion: "reduce",
    },
    playback: {
      delayMs: 0,
      repeat: 0,
      infinite: false,
      loopDelayMs: 0,
      alternate: false,
      reversed: false,
      playbackRate: 1,
      playbackEase: "",
    },
    domain,
    actions: [action],
    markers: [],
  };
}

export function triggerLabel(trigger: MotionTrigger): string {
  switch (trigger.type) {
    case "load": return "On load";
    case "inView": return "On entering viewport";
    case "click": return "On click";
    case "hover": return "On hover";
    case "scroll": return trigger.mode === "scrub" ? "Scroll-linked progress" : "On scroll";
    case "pointer": return "On pointer move";
    case "custom": return `Event ${trigger.event || "custom"}`;
  }
}

export function targetLabel(target: MotionTarget): string {
  if (target.kind === "trigger") return "Trigger element";
  if (target.kind === "element") return target.dataAnim || "Element";
  if (target.kind === "selector") return target.selector || "Selector";
  if (target.kind === "relative") {
    return `${target.relation}${target.selector ? ` · ${target.selector}` : ""}`;
  }
  return target.kind === "viewport" ? "Viewport" : "Document";
}

export function actionDuration(action: MotionAction): number {
  return action.type === "animate" || action.type === "nested" ? action.duration : 0;
}

export function actionSpan(action: MotionAction): number {
  if (action.type !== "animate" || action.repeat.infinite) return actionDuration(action);
  return action.duration * (action.repeat.count + 1)
    + action.repeat.delayMs * action.repeat.count;
}

export function interactionDuration(interaction: MotionInteraction): number {
  const max = interaction.actions.reduce(
    (value, action) => Math.max(value, action.start + actionSpan(action)),
    interaction.domain === "progress" ? 100 : 0,
  );
  return Math.max(interaction.domain === "progress" ? 100 : 1, max);
}

export function targetMatchesDataAnim(target: MotionTarget, dataAnim: string): boolean {
  return target.kind === "element" && target.dataAnim === dataAnim;
}

export function interactionTriggeredByDataAnim(
  interaction: MotionInteraction,
  dataAnim: string,
): boolean {
  return targetMatchesDataAnim(interaction.triggerTarget, dataAnim);
}

export function actionTargetsDataAnim(
  action: MotionAction,
  dataAnim: string,
  interactionTriggerTarget?: MotionTarget,
): boolean {
  if (!("target" in action)) return false;
  if (targetMatchesDataAnim(action.target, dataAnim)) return true;
  return action.target.kind === "trigger"
    && interactionTriggerTarget !== undefined
    && targetMatchesDataAnim(interactionTriggerTarget, dataAnim);
}

export function interactionTargetsDataAnim(
  interaction: MotionInteraction,
  dataAnim: string,
): boolean {
  return interaction.actions.some((action) =>
    actionTargetsDataAnim(action, dataAnim, interaction.triggerTarget)
  );
}

export function interactionTouchesDataAnim(
  interaction: MotionInteraction,
  dataAnim: string,
): boolean {
  return interactionTriggeredByDataAnim(interaction, dataAnim)
    || interactionTargetsDataAnim(interaction, dataAnim);
}

export function behaviorTouchesDataAnim(
  behavior: MotionBehavior,
  dataAnim: string,
): boolean {
  return behavior.target.kind === "element" && behavior.target.dataAnim === dataAnim;
}
