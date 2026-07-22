import type {
  PanaMotionAnimationItem,
  PanaMotionAnimatableItem,
  PanaMotionConfig,
  PanaMotionDraggableItem,
  PanaMotionEasingItem,
  PanaMotionEngineItem,
  PanaMotionExpression,
  PanaMotionFamily,
  PanaMotionItem,
  PanaMotionKeyframe,
  PanaMotionLayoutItem,
  PanaMotionPlayback,
  PanaMotionProperty,
  PanaMotionScopeItem,
  PanaMotionScrollItem,
  PanaMotionStagger,
  PanaMotionSvgItem,
  PanaMotionTimerItem,
  PanaMotionTimelineItem,
  PanaMotionTimelineStep,
  PanaMotionTimelineTrack,
  PanaMotionTarget,
  PanaMotionTextItem,
  PanaMotionTween,
  PanaMotionUtilitiesItem,
  PanaMotionValue,
  PanaMotionWaapiItem,
} from "$lib/types";
import { inferAnimePropertyCategory } from "$lib/js/anime-catalog";

export const ANIME_JS_VERSION = "4.4.1";

export type MotionFamilyDefinition = {
  type: PanaMotionFamily;
  label: string;
  shortLabel: string;
  description: string;
  role: string;
  when: string;
  mentalModel: string;
  advanced: string[];
};

export type MotionFamilyGroup = {
  id: string;
  label: string;
  families: PanaMotionFamily[];
};

export const MOTION_FAMILIES: MotionFamilyDefinition[] = [
  {
    type: "animation",
    label: "Animation",
    shortLabel: "Animation",
    description: "Schimbă proprietăți ale unui element sau grup: opacity, transform, culoare, atribute, keyframes.",
    role: "Materialul de bază. Aici definești ce se mișcă și cum se schimbă.",
    when: "Folosește pentru fade, slide, hover, click, scroll reveal și scroll scrub.",
    mentalModel: "Actorul: un element primește o mișcare.",
    advanced: ["function values", "modifiers", "callbacks"],
  },
  {
    type: "timeline",
    label: "Cronologia paginii",
    shortLabel: "Cronologie",
    description: "Ordonează animații, temporizatoare, callback-uri, setări, repere și sincronizări pe pagina curentă.",
    role: "Regizorul paginii. Nu țintește direct un element, ci orchestrează pași.",
    when: "Folosește când mai multe animații trebuie să pornească într-o ordine controlată.",
    mentalModel: "Scena: așază actorii pe timp.",
    advanced: ["relative positions", "nested timelines", "callbacks"],
  },
  {
    type: "timer",
    label: "Timer",
    shortLabel: "Timer",
    description: "Rulează callback-uri sincronizate cu engine-ul Anime.js, fără să animeze un element.",
    role: "Ceas controlat de Anime.js.",
    when: "Folosește pentru evenimente pe durată: start, update, complete, loop.",
    mentalModel: "Cronometru: declanșează cod la momente precise.",
    advanced: ["loop callbacks", "frame callbacks"],
  },
  {
    type: "animatable",
    label: "Animatable",
    shortLabel: "Live",
    description: "Setters/getters animate pentru valori care se schimbă frecvent.",
    role: "Mișcare fluidă pentru input continuu.",
    when: "Folosește pentru cursor, parallax, drag custom sau valori actualizate des.",
    mentalModel: "Volanul: schimbi mereu direcția, iar Anime.js netezește mișcarea.",
    advanced: ["interactive setters", "parallax expressions"],
  },
  {
    type: "draggable",
    label: "Draggable",
    shortLabel: "Drag",
    description: "Face elemente trăgibile, cu axe, snap, container, viteză și release spring.",
    role: "Interacțiune directă cu mouse/touch.",
    when: "Folosește pentru carduri trăgibile, sliders liberi, panouri sau obiecte interactive.",
    mentalModel: "Mânerul: utilizatorul mută elementul.",
    advanced: ["snap modifiers", "mapTo", "callbacks"],
  },
  {
    type: "layout",
    label: "Layout",
    shortLabel: "Layout",
    description: "Tranziții între stări de layout, enter/exit, reordonare și schimbare de părinte.",
    role: "Animație pentru schimbări structurale.",
    when: "Folosește când elementele își schimbă poziția în grid/flex sau apar/dispar din layout.",
    mentalModel: "Mutarea mobilei: Anime.js animă diferența dintre două stări.",
    advanced: ["record/revert hooks", "DOM order"],
  },
  {
    type: "scope",
    label: "Scope",
    shortLabel: "Scope",
    description: "Root local, defaults, media queries, reduced motion și cleanup.",
    role: "Container de reguli pentru o zonă sau componentă.",
    when: "Folosește pentru defaults, cleanup sigur și comportament diferit pe media queries.",
    mentalModel: "Camera: tot ce creezi înăuntru se poate curăța împreună.",
    advanced: ["scope methods", "keepTime"],
  },
  {
    type: "scroll",
    label: "onScroll",
    shortLabel: "onScroll",
    description: "Observer/controller de scroll: thresholds, sync modes și callbacks.",
    role: "Controlează scroll-ul ca eveniment sau sursă avansată de sincronizare.",
    when: "Folosește pentru cazuri avansate. Pentru un element animat cu scrub simplu, folosește Animation cu Trigger scroll și Scroll mode scrub.",
    mentalModel: "Senzorul de scroll: observă poziția și comandă alte lucruri.",
    advanced: ["threshold expressions", "sync methods"],
  },
  {
    type: "svg",
    label: "SVG",
    shortLabel: "SVG",
    description: "morphTo, createDrawable și createMotionPath.",
    role: "Unelte dedicate pentru forme și trasee SVG.",
    when: "Folosește pentru desenare de linii, morph între forme sau mișcare pe path.",
    mentalModel: "Traseu: elementele pot fi desenate, transformate sau conduse pe o linie.",
    advanced: ["path callbacks", "morph precision"],
  },
  {
    type: "text",
    label: "Text",
    shortLabel: "Text",
    description: "splitText și scrambleText cu accesibilitate și refresh.",
    role: "Pregătește textul pentru efecte pe litere, cuvinte sau scramble.",
    when: "Folosește pentru titluri animate pe caractere, cuvinte sau efecte type/scramble.",
    mentalModel: "Textul este spart în bucăți animate.",
    advanced: ["custom chars", "effects"],
  },
  {
    type: "utilities",
    label: "Utilities",
    shortLabel: "Utils",
    description: "stagger, get/set, cleanInlineStyles, random, clamp, lerp, damp și alte utilitare.",
    role: "Cutia de scule pentru calcule și distribuții.",
    when: "Folosește când ai nevoie de stagger avansat, valori random, clamp, snap sau curățare inline.",
    mentalModel: "Unelte auxiliare, nu animație vizibilă de sine stătătoare.",
    advanced: ["stagger modifier", "seeded random"],
  },
  {
    type: "easing",
    label: "Easings",
    shortLabel: "Ease",
    description: "Built-in, cubicBezier, linear, steps, irregular și spring.",
    role: "Forma mișcării.",
    when: "Folosește când vrei să creezi/alegi felul în care accelerează o animație.",
    mentalModel: "Curba: aceeași distanță, altă senzație.",
    advanced: ["custom ease", "convertEase"],
  },
  {
    type: "waapi",
    label: "WAAPI",
    shortLabel: "WAAPI",
    description: "waapi.animate, hardware acceleration, transforms individuale și finished.",
    role: "Animații prin Web Animations API, eficiente pentru browser.",
    when: "Folosește pentru animații performante, simple, care beneficiază de accelerare nativă.",
    mentalModel: "Anime.js pregătește, browserul rulează.",
    advanced: ["function values", "spring conversion"],
  },
  {
    type: "engine",
    label: "Engine",
    shortLabel: "Engine",
    description: "timeUnit, speed, fps, precision, pauseOnDocumentHidden și control global.",
    role: "Setări globale pentru motorul Anime.js.",
    when: "Folosește rar, când vrei să controlezi viteza, fps-ul sau comportamentul global al runtime-ului.",
    mentalModel: "Motorul mașinii, nu o animație anume.",
    advanced: ["priority", "manual update"],
  },
  {
    type: "interaction",
    label: "Interaction",
    shortLabel: "Event",
    description: "Evenimente DOM și acțiuni declarative compatibile cu contractul curent.",
    role: "Leagă evenimente de acțiuni.",
    when: "Folosește pentru click, hover sau alte evenimente care pornesc comportamente.",
    mentalModel: "Întrerupătorul: un gest declanșează ceva.",
    advanced: ["custom actions"],
  },
  {
    type: "custom",
    label: "Custom JS",
    shortLabel: "Code",
    description: "Cod JS explicit pentru cazuri care nu trebuie ascunse în controale vizuale.",
    role: "Ieșirea controlată către cod manual.",
    when: "Folosește doar când un caz este prea specific pentru UI.",
    mentalModel: "Ușa tehnică: scrii cod când UI-ul nu trebuie să inventeze o abstracție falsă.",
    advanced: ["full JS"],
  },
];

export const MOTION_ELEMENT_FAMILY_GROUPS: MotionFamilyGroup[] = [
  {
    id: "primary",
    label: "Principal",
    families: ["animation"],
  },
  {
    id: "specialized",
    label: "Specializate",
    families: ["text", "svg", "draggable", "layout", "animatable"],
  },
  {
    id: "advanced",
    label: "Avansat",
    families: ["scroll", "timer", "scope", "utilities", "easing", "waapi", "engine", "interaction", "custom"],
  },
];

function id(prefix: string): string {
  return `${prefix}-${Math.random().toString(36).slice(2, 9)}`;
}

export function emptyExpression(label = "Advanced Expression"): PanaMotionExpression {
  return { enabled: false, label, code: "" };
}

export function defaultMotionTarget(dataAnim = ""): PanaMotionTarget {
  return {
    mode: dataAnim ? "dataAnim" : "selected",
    selector: dataAnim ? `[data-anim="${dataAnim}"]` : "",
    dataAnim,
    expression: "",
  };
}

export function defaultMotionValue(value = ""): PanaMotionValue {
  return {
    mode: value ? "literal" : "fromTo",
    value,
    from: "",
    to: "",
    unit: "",
    expression: "",
  };
}

export function defaultMotionTween(): PanaMotionTween {
  return {
    delay: 0,
    duration: 0,
    ease: "",
  };
}

export function defaultMotionProperty(property = "opacity"): PanaMotionProperty {
  return {
    id: id("prop"),
    property,
    category: inferAnimePropertyCategory(property),
    value: {
      ...defaultMotionValue(),
      from: property === "opacity" ? "0" : "",
      to: property === "opacity" ? "1" : "",
    },
    modifier: emptyExpression("Modifier"),
    composition: "replace",
    tween: defaultMotionTween(),
  };
}

export function defaultMotionKeyframe(): PanaMotionKeyframe {
  return {
    id: id("keyframe"),
    label: "Keyframe",
    at: "",
    duration: 300,
    ease: "",
    properties: [defaultMotionProperty("opacity")],
    advanced: [emptyExpression("Keyframe expression")],
  };
}

export function defaultPlayback(duration = 600): PanaMotionPlayback {
  return {
    autoplay: true,
    delay: 0,
    duration,
    loop: 0,
    loopDelay: 0,
    alternate: false,
    reversed: false,
    frameRate: 0,
    playbackRate: 1,
    playbackEase: "",
    persist: false,
  };
}

export function defaultStagger(): PanaMotionStagger {
  return {
    enabled: false,
    each: 50,
    start: 0,
    from: "first",
    reversed: false,
    ease: "",
    grid: "",
    axis: "",
    use: "delay",
    total: 0,
    modifier: emptyExpression("Stagger modifier"),
  };
}

export function defaultTimelineTrack(index = 0): PanaMotionTimelineTrack {
  return {
    id: index === 0 ? "track-main" : id("track"),
    name: index === 0 ? "Principal" : `Track ${index + 1}`,
    collapsed: false,
    height: 38,
    color: "#168a72",
  };
}

function baseItem(type: PanaMotionFamily, target: PanaMotionTarget): Omit<PanaMotionItem, "type"> {
  return {
    id: id(type),
    name: MOTION_FAMILIES.find((family) => family.type === type)?.label ?? type,
    enabled: true,
    target,
    scopeId: "",
    advanced: [emptyExpression("Custom JS")],
  } as Omit<PanaMotionItem, "type">;
}

export function createMotionItem(type: PanaMotionFamily, dataAnim = ""): PanaMotionItem {
  const target = defaultMotionTarget(dataAnim);
  const base = baseItem(type, target);
  switch (type) {
    case "animation":
      return {
        ...base,
        type,
        properties: [defaultMotionProperty("opacity"), defaultMotionProperty("translateY")],
        keyframes: [],
        playback: defaultPlayback(),
        stagger: defaultStagger(),
        callbacks: defaultCallbacks(["begin", "beforeUpdate", "update", "render", "loop", "pause", "complete"]),
        trigger: "load",
        scrollRepeat: false,
        scrollScrub: false,
        textEffect: "",
        targetSelector: "",
      } as PanaMotionAnimationItem;
    case "timeline":
      return {
        ...base,
        type,
        duration: 10_000,
        tracks: [defaultTimelineTrack()],
        labels: [],
        steps: [],
        playback: defaultPlayback(10_000),
      };
    case "timer":
      return { ...base, type, playback: defaultPlayback(1000), callbacks: defaultCallbacks(["begin", "update", "complete", "loop"]) };
    case "animatable":
      return {
        ...base,
        type,
        properties: [defaultMotionProperty("translateX"), defaultMotionProperty("translateY")],
        mode: "setters",
        duration: 500,
        ease: "outQuad",
        unit: "px",
        liveSource: "pointer",
        setterExpression: emptyExpression("Animatable setter"),
      } as PanaMotionAnimatableItem;
    case "draggable":
      return {
        ...base,
        type,
        axes: "both",
        container: "",
        trigger: "",
        snap: "",
        snapX: "",
        snapY: "",
        mapTo: "",
        modifier: emptyExpression("Draggable modifier"),
        containerPadding: 0,
        friction: 0.85,
        releaseContainerFriction: 0,
        velocity: 1,
        minVelocity: 0,
        maxVelocity: 0,
        releaseEase: "",
        dragSpeed: 1,
        dragThreshold: 3,
        scrollThreshold: 50,
        scrollSpeed: 1,
        cursor: true,
        release: { spring: true, mass: 1, stiffness: 80, damping: 12 },
        callbacks: defaultCallbacks(["grab", "drag", "update", "release", "snap", "settle", "resize", "afterResize"]),
      } as PanaMotionDraggableItem;
    case "layout":
      return {
        ...base,
        type,
        mode: "animate",
        includeDisplay: true,
        includeGrid: true,
        includeFlex: true,
        includeOrder: true,
        enterExit: true,
        swapParent: false,
        children: "",
        properties: "",
        enterFrom: "opacity: 0; transform: translateY(24px);",
        leaveTo: "opacity: 0; transform: translateY(-24px);",
        swapAt: "",
        playback: defaultPlayback(),
        callbacks: defaultCallbacks(["begin", "update", "render", "complete"]),
      } as PanaMotionLayoutItem;
    case "scope":
      return {
        ...base,
        type,
        root: target.selector,
        defaults: {},
        mediaQueries: [],
        reducedMotion: "respect",
        keepTime: true,
      } as PanaMotionScopeItem;
    case "scroll":
      return {
        ...base,
        type,
        container: "",
        axis: "y",
        repeat: true,
        debug: false,
        enter: "bottom top",
        leave: "top bottom",
        threshold: "bottom top",
        sync: "play",
        syncMode: "methods",
        syncMethods: "play pause",
        syncEase: "",
        smooth: 0,
        callbacks: defaultCallbacks([
          "enter",
          "enterForward",
          "enterBackward",
          "leave",
          "leaveForward",
          "leaveBackward",
          "update",
          "syncComplete",
          "resize",
        ]),
      } as PanaMotionScrollItem;
    case "svg":
      return {
        ...base,
        type,
        mode: "createDrawable",
        attribute: "d",
        source: "",
        path: "",
        precision: 0.33,
        offset: 0,
        draw: "0 1",
        playback: defaultPlayback(1000),
        callbacks: defaultCallbacks(["begin", "beforeUpdate", "update", "render", "loop", "pause", "complete"]),
      } as PanaMotionSvgItem;
    case "text":
      return {
        ...base,
        type,
        mode: "splitText",
        split: {
          lines: false,
          words: true,
          chars: true,
          debug: false,
          includeSpaces: true,
          accessible: true,
          className: "",
          wrap: "span",
          clone: false,
        },
        scramble: {
          text: "",
          chars: "lowercase",
          override: true,
          ease: "linear",
          cursor: "_",
          revealRate: 60,
          revealDelay: 0,
          settleRate: 30,
          settleDuration: 500,
          delay: 0,
          duration: 900,
          from: "auto",
          reversed: false,
          perturbation: 0.25,
          seed: 0,
        },
        callbacks: defaultCallbacks(["begin", "beforeUpdate", "update", "render", "loop", "pause", "complete"]),
      } as PanaMotionTextItem;
    case "utilities":
      return {
        ...base,
        type,
        utility: "stagger",
        args: "100",
        stagger: defaultStagger(),
        expression: emptyExpression("Utility expression"),
      } as PanaMotionUtilitiesItem;
    case "easing":
      return { ...base, type, mode: "builtIn", value: "outExpo", previewDuration: 600 };
    case "waapi":
      return {
        ...base,
        type,
        properties: [defaultMotionProperty("opacity"), defaultMotionProperty("translateY")],
        playback: defaultPlayback(),
        iterations: 1,
        direction: "normal",
        easing: "outExpo",
        autoplay: true,
        hardwareAcceleration: true,
        convertEase: true,
        finished: emptyExpression("WAAPI finished"),
      } as PanaMotionWaapiItem;
    case "engine":
      return { ...base, type, timeUnit: "ms", speed: 1, fps: 120, precision: 3, pauseOnDocumentHidden: true, priority: 1 } as PanaMotionEngineItem;
    case "interaction":
      return { ...base, type, event: "click", action: "toggleClass", targetSelector: "", value: "" };
    case "custom":
      return { ...base, type, code: "" };
  }
}

function defaultCallbacks(names: string[]): Record<string, PanaMotionExpression> {
  return Object.fromEntries(names.map((name) => [name, emptyExpression(name)]));
}

export function emptyMotionConfig(): PanaMotionConfig {
  return {
    schemaVersion: 1,
    animeVersion: ANIME_JS_VERSION,
    activeItemId: null,
    items: [],
  };
}

export function normalizeMotionConfig(input: Partial<PanaMotionConfig> | null | undefined): PanaMotionConfig {
  const base = emptyMotionConfig();
  const items = Array.isArray(input?.items)
    ? input.items
        .map((item) => normalizeMotionItem(item as Partial<PanaMotionItem>))
        .filter((item): item is PanaMotionItem => Boolean(item))
    : [];
  return {
    schemaVersion: 1,
    animeVersion: typeof input?.animeVersion === "string" && input.animeVersion ? input.animeVersion : base.animeVersion,
    activeItemId: typeof input?.activeItemId === "string" ? input.activeItemId : null,
    items,
  };
}

export function normalizeMotionItem(input: Partial<PanaMotionItem> | null | undefined): PanaMotionItem | null {
  const type = input?.type;
  if (!type || !MOTION_FAMILIES.some((family) => family.type === type)) return null;
  const fallback = createMotionItem(type);
  const item = {
    ...fallback,
    ...input,
    id: typeof input.id === "string" && input.id ? input.id : fallback.id,
    name: typeof input.name === "string" && input.name ? input.name : fallback.name,
    enabled: typeof input.enabled === "boolean" ? input.enabled : fallback.enabled,
    target: { ...fallback.target, ...(input.target ?? {}) },
    advanced: Array.isArray(input.advanced) ? input.advanced.map(normalizeExpression) : fallback.advanced,
  } as PanaMotionItem;

  if (item.type === "animation") {
    const typedInput = input as Partial<PanaMotionAnimationItem>;
    const typedFallback = fallback as PanaMotionAnimationItem;
    return {
      ...item,
      properties: normalizeProperties(typedInput.properties, typedFallback.properties),
      keyframes: normalizeKeyframes(typedInput.keyframes, typedFallback.keyframes),
      playback: { ...typedFallback.playback, ...(typedInput.playback ?? {}) },
      stagger: {
        ...typedFallback.stagger,
        ...(typedInput.stagger ?? {}),
        modifier: normalizeExpression(typedInput.stagger?.modifier ?? typedFallback.stagger.modifier),
      },
      callbacks: normalizeCallbacks(typedFallback.callbacks, typedInput.callbacks),
    } as PanaMotionAnimationItem;
  }

  if (item.type === "timeline") {
    const typedInput = input as Partial<PanaMotionTimelineItem>;
    const typedFallback = fallback as PanaMotionTimelineItem;
    return {
      ...item,
      duration: normalizePositiveNumber(typedInput.duration, typedFallback.duration),
      tracks: normalizeTimelineTracks(typedInput.tracks, typedInput.steps, typedFallback.tracks),
      labels: Array.isArray(typedInput.labels)
        ? typedInput.labels.map((label, index) => ({
            id: typeof label?.id === "string" && label.id ? label.id : id("label"),
            name: typeof label?.name === "string" && label.name ? label.name : `Label ${index + 1}`,
            position: typeof label?.position === "string" ? label.position : "0",
          }))
        : typedFallback.labels,
      steps: normalizeTimelineSteps(typedInput.steps, typedFallback.steps),
      playback: { ...typedFallback.playback, ...(typedInput.playback ?? {}) },
    } as PanaMotionTimelineItem;
  }

  if (item.type === "timer") {
    const typedInput = input as Partial<PanaMotionTimerItem>;
    const typedFallback = fallback as PanaMotionTimerItem;
    return {
      ...item,
      playback: { ...typedFallback.playback, ...(typedInput.playback ?? {}) },
      callbacks: normalizeCallbacks(typedFallback.callbacks, typedInput.callbacks),
    } as PanaMotionTimerItem;
  }

  if (item.type === "animatable") {
    const typedInput = input as Partial<PanaMotionAnimatableItem>;
    const typedFallback = fallback as PanaMotionAnimatableItem;
    return {
      ...item,
      properties: normalizeProperties(typedInput.properties, typedFallback.properties),
      mode: ["setters", "getters", "both"].includes(String(typedInput.mode))
        ? typedInput.mode
        : typedFallback.mode,
      duration: normalizePositiveNumber(typedInput.duration, typedFallback.duration),
      ease: stringOr(typedInput.ease, typedFallback.ease),
      unit: stringOr(typedInput.unit, typedFallback.unit),
      liveSource: ["none", "pointer", "scroll", "expression"].includes(String(typedInput.liveSource))
        ? typedInput.liveSource
        : typedFallback.liveSource,
      setterExpression: normalizeExpression(typedInput.setterExpression ?? typedFallback.setterExpression),
    } as PanaMotionAnimatableItem;
  }

  if (item.type === "waapi") {
    const typedInput = input as Partial<PanaMotionWaapiItem>;
    const typedFallback = fallback as PanaMotionWaapiItem;
    return {
      ...item,
      properties: normalizeProperties(typedInput.properties, typedFallback.properties),
      playback: { ...typedFallback.playback, ...(typedInput.playback ?? {}) },
      iterations: normalizePositiveNumber(typedInput.iterations, typedFallback.iterations),
      direction: stringOr(typedInput.direction, typedFallback.direction),
      easing: stringOr(typedInput.easing, typedFallback.easing),
      autoplay: typeof typedInput.autoplay === "boolean" ? typedInput.autoplay : typedFallback.autoplay,
      hardwareAcceleration: typeof typedInput.hardwareAcceleration === "boolean"
        ? typedInput.hardwareAcceleration
        : typedFallback.hardwareAcceleration,
      convertEase: typeof typedInput.convertEase === "boolean" ? typedInput.convertEase : typedFallback.convertEase,
      finished: normalizeExpression(typedInput.finished ?? typedFallback.finished),
    } as PanaMotionWaapiItem;
  }

  if (item.type === "draggable") {
    const typedInput = input as Partial<PanaMotionDraggableItem>;
    const typedFallback = fallback as PanaMotionDraggableItem;
    return {
      ...item,
      axes: typedInput.axes === "x" || typedInput.axes === "y" || typedInput.axes === "both" ? typedInput.axes : typedFallback.axes,
      container: stringOr(typedInput.container, typedFallback.container),
      trigger: stringOr(typedInput.trigger, typedFallback.trigger),
      snap: stringOr(typedInput.snap, typedFallback.snap),
      snapX: stringOr(typedInput.snapX, typedFallback.snapX),
      snapY: stringOr(typedInput.snapY, typedFallback.snapY),
      mapTo: stringOr(typedInput.mapTo, typedFallback.mapTo),
      modifier: normalizeExpression(typedInput.modifier ?? typedFallback.modifier),
      containerPadding: normalizePositiveNumber(typedInput.containerPadding, typedFallback.containerPadding),
      friction: normalizePositiveNumber(typedInput.friction, typedFallback.friction),
      releaseContainerFriction: normalizePositiveNumber(typedInput.releaseContainerFriction, typedFallback.releaseContainerFriction),
      velocity: normalizePositiveNumber(typedInput.velocity, typedFallback.velocity),
      minVelocity: normalizePositiveNumber(typedInput.minVelocity, typedFallback.minVelocity),
      maxVelocity: normalizePositiveNumber(typedInput.maxVelocity, typedFallback.maxVelocity),
      releaseEase: stringOr(typedInput.releaseEase, typedFallback.releaseEase),
      dragSpeed: normalizePositiveNumber(typedInput.dragSpeed, typedFallback.dragSpeed),
      dragThreshold: normalizePositiveNumber(typedInput.dragThreshold, typedFallback.dragThreshold),
      scrollThreshold: normalizePositiveNumber(typedInput.scrollThreshold, typedFallback.scrollThreshold),
      scrollSpeed: normalizePositiveNumber(typedInput.scrollSpeed, typedFallback.scrollSpeed),
      cursor: typeof typedInput.cursor === "boolean" ? typedInput.cursor : typedFallback.cursor,
      release: {
        ...typedFallback.release,
        ...(typedInput.release ?? {}),
        spring: typeof typedInput.release?.spring === "boolean" ? typedInput.release.spring : typedFallback.release.spring,
        mass: normalizePositiveNumber(typedInput.release?.mass, typedFallback.release.mass),
        stiffness: normalizePositiveNumber(typedInput.release?.stiffness, typedFallback.release.stiffness),
        damping: normalizePositiveNumber(typedInput.release?.damping, typedFallback.release.damping),
      },
      callbacks: normalizeCallbacks(typedFallback.callbacks, typedInput.callbacks),
    } as PanaMotionDraggableItem;
  }

  if (item.type === "layout") {
    const typedInput = input as Partial<PanaMotionLayoutItem>;
    const typedFallback = fallback as PanaMotionLayoutItem;
    return {
      ...item,
      mode: ["record", "animate", "update", "revert"].includes(String(typedInput.mode))
        ? typedInput.mode
        : typedFallback.mode,
      children: stringOr(typedInput.children, typedFallback.children),
      properties: stringOr(typedInput.properties, typedFallback.properties),
      enterFrom: stringOr(typedInput.enterFrom, typedFallback.enterFrom),
      leaveTo: stringOr(typedInput.leaveTo, typedFallback.leaveTo),
      swapAt: stringOr(typedInput.swapAt, typedFallback.swapAt),
      includeDisplay: typeof typedInput.includeDisplay === "boolean" ? typedInput.includeDisplay : typedFallback.includeDisplay,
      includeGrid: typeof typedInput.includeGrid === "boolean" ? typedInput.includeGrid : typedFallback.includeGrid,
      includeFlex: typeof typedInput.includeFlex === "boolean" ? typedInput.includeFlex : typedFallback.includeFlex,
      includeOrder: typeof typedInput.includeOrder === "boolean" ? typedInput.includeOrder : typedFallback.includeOrder,
      enterExit: typeof typedInput.enterExit === "boolean" ? typedInput.enterExit : typedFallback.enterExit,
      swapParent: typeof typedInput.swapParent === "boolean" ? typedInput.swapParent : typedFallback.swapParent,
      playback: { ...typedFallback.playback, ...(typedInput.playback ?? {}) },
      callbacks: normalizeCallbacks(typedFallback.callbacks, typedInput.callbacks),
    } as PanaMotionLayoutItem;
  }

  if (item.type === "scope") {
    const typedInput = input as Partial<PanaMotionScopeItem>;
    const typedFallback = fallback as PanaMotionScopeItem;
    return {
      ...item,
      root: stringOr(typedInput.root, typedFallback.root),
      defaults: normalizeStringRecord(typedInput.defaults, typedFallback.defaults),
      mediaQueries: Array.isArray(typedInput.mediaQueries)
        ? typedInput.mediaQueries.map((query, index) => ({
            id: typeof query?.id === "string" && query.id ? query.id : `mq-${index + 1}`,
            query: typeof query?.query === "string" ? query.query : "",
            enabled: typeof query?.enabled === "boolean" ? query.enabled : true,
          }))
        : typedFallback.mediaQueries,
      reducedMotion: ["respect", "ignore", "disable"].includes(String(typedInput.reducedMotion))
        ? typedInput.reducedMotion
        : typedFallback.reducedMotion,
      keepTime: typeof typedInput.keepTime === "boolean" ? typedInput.keepTime : typedFallback.keepTime,
    } as PanaMotionScopeItem;
  }

  if (item.type === "scroll") {
    const typedInput = input as Partial<PanaMotionScrollItem>;
    const typedFallback = fallback as PanaMotionScrollItem;
    return {
      ...item,
      container: stringOr(typedInput.container, typedFallback.container),
      axis: typedInput.axis === "x" || typedInput.axis === "y" ? typedInput.axis : typedFallback.axis,
      repeat: typeof typedInput.repeat === "boolean" ? typedInput.repeat : typedFallback.repeat,
      debug: typeof typedInput.debug === "boolean" ? typedInput.debug : typedFallback.debug,
      enter: stringOr(typedInput.enter, stringOr(typedInput.threshold, typedFallback.enter)),
      leave: stringOr(typedInput.leave, typedFallback.leave),
      threshold: stringOr(typedInput.threshold, stringOr(typedInput.enter, typedFallback.threshold)),
      sync: validLegacyScrollSync(typedInput.sync) ? typedInput.sync : typedFallback.sync,
      syncMode: validScrollSyncMode(typedInput.syncMode)
        ? typedInput.syncMode
        : legacyScrollSyncMode(typedInput.sync, typedFallback.syncMode),
      syncMethods: stringOr(
        typedInput.syncMethods,
        validMethodScrollSync(typedInput.sync) ? typedInput.sync : typedFallback.syncMethods,
      ),
      syncEase: stringOr(typedInput.syncEase, validEasedScrollSync(typedInput.sync) ? typedInput.sync : typedFallback.syncEase),
      smooth: normalizeSmooth(typedInput.smooth ?? (typedInput.sync === "smooth" ? 0.25 : typedFallback.smooth)),
      callbacks: normalizeCallbacks(typedFallback.callbacks, typedInput.callbacks),
    } as PanaMotionScrollItem;
  }

  if (item.type === "svg") {
    const typedInput = input as Partial<PanaMotionSvgItem>;
    const typedFallback = fallback as PanaMotionSvgItem;
    return {
      ...item,
      mode: ["morphTo", "createDrawable", "createMotionPath"].includes(String(typedInput.mode))
        ? typedInput.mode
        : typedFallback.mode,
      attribute: typedInput.attribute === "points" ? "points" : typedFallback.attribute,
      source: stringOr(typedInput.source, typedFallback.source),
      path: stringOr(typedInput.path, typedFallback.path),
      precision: normalizeRatio(typedInput.precision, typedFallback.precision),
      offset: normalizeRatio(typedInput.offset, typedFallback.offset),
      draw: stringOr(typedInput.draw, typedFallback.draw),
      playback: { ...typedFallback.playback, ...(typedInput.playback ?? {}) },
      callbacks: normalizeCallbacks(typedFallback.callbacks, typedInput.callbacks),
    } as PanaMotionSvgItem;
  }

  if (item.type === "text") {
    const typedInput = input as Partial<PanaMotionTextItem>;
    const typedFallback = fallback as PanaMotionTextItem;
    return {
      ...item,
      mode: typedInput.mode === "scrambleText" ? "scrambleText" : typedFallback.mode,
      split: {
        ...typedFallback.split,
        ...(typedInput.split ?? {}),
        lines: Boolean(typedInput.split?.lines ?? typedFallback.split.lines),
        words: Boolean(typedInput.split?.words ?? typedFallback.split.words),
        chars: Boolean(typedInput.split?.chars ?? typedFallback.split.chars),
        debug: Boolean(typedInput.split?.debug ?? typedFallback.split.debug),
        includeSpaces: Boolean(typedInput.split?.includeSpaces ?? typedFallback.split.includeSpaces),
        accessible: typeof typedInput.split?.accessible === "boolean" ? typedInput.split.accessible : typedFallback.split.accessible,
        clone: Boolean(typedInput.split?.clone ?? typedFallback.split.clone),
      },
      scramble: {
        ...typedFallback.scramble,
        ...(typedInput.scramble ?? {}),
        revealRate: normalizePositiveNumber(typedInput.scramble?.revealRate, typedFallback.scramble.revealRate),
        revealDelay: normalizePositiveNumber(typedInput.scramble?.revealDelay, typedFallback.scramble.revealDelay),
        settleRate: normalizePositiveNumber(typedInput.scramble?.settleRate, typedFallback.scramble.settleRate),
        settleDuration: normalizePositiveNumber(typedInput.scramble?.settleDuration, typedFallback.scramble.settleDuration),
        delay: normalizePositiveNumber(typedInput.scramble?.delay, typedFallback.scramble.delay),
        duration: normalizePositiveNumber(typedInput.scramble?.duration, typedFallback.scramble.duration),
        perturbation: normalizeRatio(typedInput.scramble?.perturbation, typedFallback.scramble.perturbation),
        seed: normalizePositiveNumber(typedInput.scramble?.seed, typedFallback.scramble.seed),
        override: typeof typedInput.scramble?.override === "boolean" ? typedInput.scramble.override : typedFallback.scramble.override,
        reversed: Boolean(typedInput.scramble?.reversed ?? typedFallback.scramble.reversed),
      },
      callbacks: normalizeCallbacks(typedFallback.callbacks, typedInput.callbacks),
    } as PanaMotionTextItem;
  }

  if (item.type === "utilities") {
    const typedInput = input as Partial<PanaMotionUtilitiesItem>;
    const typedFallback = fallback as PanaMotionUtilitiesItem;
    return {
      ...item,
      utility: stringOr(typedInput.utility, typedFallback.utility),
      args: stringOr(typedInput.args, typedFallback.args),
      stagger: {
        ...typedFallback.stagger,
        ...(typedInput.stagger ?? {}),
        modifier: normalizeExpression(typedInput.stagger?.modifier ?? typedFallback.stagger.modifier),
      },
      expression: normalizeExpression(typedInput.expression ?? typedFallback.expression),
    } as PanaMotionUtilitiesItem;
  }

  if (item.type === "easing") {
    const typedInput = input as Partial<PanaMotionEasingItem>;
    const typedFallback = fallback as PanaMotionEasingItem;
    return {
      ...item,
      mode: ["builtIn", "cubicBezier", "linear", "steps", "irregular", "spring", "custom"].includes(String(typedInput.mode))
        ? typedInput.mode
        : typedFallback.mode,
      value: stringOr(typedInput.value, typedFallback.value),
      previewDuration: normalizePositiveNumber(typedInput.previewDuration, typedFallback.previewDuration),
    } as PanaMotionEasingItem;
  }

  if (item.type === "engine") {
    const typedInput = input as Partial<PanaMotionEngineItem>;
    const typedFallback = fallback as PanaMotionEngineItem;
    return {
      ...item,
      timeUnit: typedInput.timeUnit === "s" ? "s" : typedFallback.timeUnit,
      speed: normalizePositiveNumber(typedInput.speed, typedFallback.speed),
      fps: normalizePositiveNumber(typedInput.fps, typedFallback.fps),
      precision: normalizePositiveNumber(typedInput.precision, typedFallback.precision),
      pauseOnDocumentHidden: typeof typedInput.pauseOnDocumentHidden === "boolean"
        ? typedInput.pauseOnDocumentHidden
        : typedFallback.pauseOnDocumentHidden,
      priority: normalizePositiveNumber(typedInput.priority, typedFallback.priority),
    } as PanaMotionEngineItem;
  }

  return item;
}

function stringOr(value: unknown, fallback: string): string {
  return typeof value === "string" ? value : fallback;
}

function validLegacyScrollSync(value: unknown): value is PanaMotionScrollItem["sync"] {
  return ["play", "pause", "restart", "reverse", "progress", "smooth", "eased"].includes(String(value));
}

function validMethodScrollSync(value: unknown): value is "play" | "pause" | "restart" | "reverse" {
  return ["play", "pause", "restart", "reverse"].includes(String(value));
}

function validEasedScrollSync(value: unknown): value is string {
  return typeof value === "string" && value.length > 0 && !validLegacyScrollSync(value);
}

function validScrollSyncMode(value: unknown): value is PanaMotionScrollItem["syncMode"] {
  return ["methods", "progress", "smooth", "eased"].includes(String(value));
}

function legacyScrollSyncMode(value: unknown, fallback: PanaMotionScrollItem["syncMode"]): PanaMotionScrollItem["syncMode"] {
  if (value === "progress") return "progress";
  if (value === "smooth") return "smooth";
  if (value === "eased") return "eased";
  return fallback;
}

function normalizeSmooth(value: unknown): number {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return 0;
  return Math.max(0, Math.min(1, parsed));
}

function normalizeRatio(value: unknown, fallback: number): number {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.max(0, Math.min(1, parsed));
}

function normalizePositiveNumber(value: unknown, fallback: number): number {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.max(0, parsed);
}

function normalizeStringRecord(value: unknown, fallback: Record<string, string>): Record<string, string> {
  if (!value || typeof value !== "object" || Array.isArray(value)) return fallback;
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .filter(([key]) => key.trim().length > 0)
      .map(([key, recordValue]) => [key, typeof recordValue === "string" ? recordValue : String(recordValue)]),
  );
}

function normalizeExpression(input: Partial<PanaMotionExpression> | null | undefined): PanaMotionExpression {
  return {
    enabled: Boolean(input?.enabled),
    label: typeof input?.label === "string" && input.label ? input.label : "Advanced Expression",
    code: typeof input?.code === "string" ? input.code : "",
  };
}

function normalizeMotionValue(input: Partial<PanaMotionValue> | null | undefined): PanaMotionValue {
  return {
    ...defaultMotionValue(),
    ...(input ?? {}),
  };
}

function normalizeMotionTween(input: Partial<PanaMotionTween> | null | undefined): PanaMotionTween {
  return {
    delay: Number.isFinite(Number(input?.delay)) ? Math.max(0, Math.round(Number(input?.delay))) : 0,
    duration: Number.isFinite(Number(input?.duration)) ? Math.max(0, Math.round(Number(input?.duration))) : 0,
    ease: typeof input?.ease === "string" ? input.ease : "",
  };
}

function normalizeProperty(input: Partial<PanaMotionProperty> | null | undefined): PanaMotionProperty {
  const fallback = defaultMotionProperty(typeof input?.property === "string" && input.property ? input.property : "opacity");
  return {
    ...fallback,
    ...(input ?? {}),
    id: typeof input?.id === "string" && input.id ? input.id : fallback.id,
    property: typeof input?.property === "string" && input.property ? input.property : fallback.property,
    value: normalizeMotionValue(input?.value),
    modifier: normalizeExpression(input?.modifier),
    composition: typeof input?.composition === "string" && input.composition ? input.composition : fallback.composition,
    tween: normalizeMotionTween(input?.tween),
  };
}

function normalizeProperties(
  input: Partial<PanaMotionProperty>[] | null | undefined,
  fallback: PanaMotionProperty[],
): PanaMotionProperty[] {
  const source = Array.isArray(input) ? input : fallback;
  return source.map((property) => normalizeProperty(property));
}

function normalizeKeyframes(
  input: Partial<PanaMotionKeyframe>[] | null | undefined,
  fallback: PanaMotionKeyframe[],
): PanaMotionKeyframe[] {
  const source = Array.isArray(input) ? input : fallback;
  return source.map((keyframe) => ({
    ...defaultMotionKeyframe(),
    ...keyframe,
    id: typeof keyframe?.id === "string" && keyframe.id ? keyframe.id : defaultMotionKeyframe().id,
    label: typeof keyframe?.label === "string" && keyframe.label ? keyframe.label : "Keyframe",
    properties: normalizeProperties(keyframe?.properties, [defaultMotionProperty("opacity")]),
    advanced: Array.isArray(keyframe?.advanced) ? keyframe.advanced.map(normalizeExpression) : [emptyExpression("Keyframe expression")],
  }));
}

function normalizeTimelineSteps(
  input: Partial<PanaMotionTimelineStep>[] | null | undefined,
  fallback: PanaMotionTimelineStep[],
): PanaMotionTimelineStep[] {
  const source = Array.isArray(input) ? input : fallback;
  return source.map((step, index) => {
    const type: PanaMotionTimelineStep["type"] = ["animation", "timer", "callback", "set", "sync", "label"].includes(String(step?.type))
      ? step.type as PanaMotionTimelineStep["type"]
      : "animation";
    return {
      id: typeof step?.id === "string" && step.id ? step.id : id("step"),
      type,
      label: typeof step?.label === "string" && step.label ? step.label : `${type} ${index + 1}`,
      position: typeof step?.position === "string" ? step.position : "0",
      duration: normalizePositiveNumber(step?.duration, 0),
      lane: typeof step?.lane === "string" && step.lane ? step.lane : "track-main",
      targetItemId: typeof step?.targetItemId === "string" ? step.targetItemId : "",
      callback: normalizeExpression(step?.callback ?? emptyExpression("Timeline callback")),
    };
  });
}

function normalizeTimelineTracks(
  input: Partial<PanaMotionTimelineTrack>[] | null | undefined,
  steps: Partial<PanaMotionTimelineStep>[] | null | undefined,
  fallback: PanaMotionTimelineTrack[],
): PanaMotionTimelineTrack[] {
  const source = Array.isArray(input) && input.length > 0 ? input : fallback;
  const normalized = source.map((track, index) => ({
    ...defaultTimelineTrack(index),
    ...(track ?? {}),
    id: typeof track?.id === "string" && track.id ? track.id : defaultTimelineTrack(index).id,
    name: typeof track?.name === "string" && track.name ? track.name : defaultTimelineTrack(index).name,
    collapsed: Boolean(track?.collapsed),
    height: normalizePositiveNumber(track?.height, defaultTimelineTrack(index).height),
    color: typeof track?.color === "string" && track.color ? track.color : defaultTimelineTrack(index).color,
  }));
  const seen = new Set(normalized.map((track) => track.id));
  if (Array.isArray(steps)) {
    for (const step of steps) {
      const lane = typeof step?.lane === "string" && step.lane ? step.lane : "";
      if (!lane || seen.has(lane)) continue;
      seen.add(lane);
      normalized.push({
        ...defaultTimelineTrack(normalized.length),
        id: lane,
        name: lane,
      });
    }
  }
  return normalized.length > 0 ? normalized : [defaultTimelineTrack()];
}

function normalizeCallbacks(
  fallback: Record<string, PanaMotionExpression>,
  input: Record<string, Partial<PanaMotionExpression>> | null | undefined,
): Record<string, PanaMotionExpression> {
  const result: Record<string, PanaMotionExpression> = {};
  for (const [name, expression] of Object.entries({ ...fallback, ...(input ?? {}) })) {
    result[name] = normalizeExpression({ ...expression, label: expression?.label || name });
  }
  return result;
}

export function isMotionConfigEmpty(input: Partial<PanaMotionConfig> | null | undefined): boolean {
  return normalizeMotionConfig(input).items.length === 0;
}
