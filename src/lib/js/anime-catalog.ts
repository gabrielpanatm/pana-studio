export const ANIME_EASING_OPTIONS = [
  "linear",
  "inQuad", "outQuad", "inOutQuad",
  "inCubic", "outCubic", "inOutCubic",
  "inExpo", "outExpo", "inOutExpo",
  "inBack", "outBack", "inOutBack",
  "inElastic", "outElastic", "inOutElastic",
  "spring({mass:1,stiffness:80,damping:10})",
];

export const ANIME_ANIMATION_TRIGGERS = ["load", "scroll", "click", "hover"] as const;
export const ANIME_DIRECTIONS = ["normal", "reverse", "alternate", "alternateReverse"] as const;
export const ANIME_INTERACTION_EVENTS = ["click", "mouseenter", "mouseleave"] as const;
export const ANIME_INTERACTION_ACTIONS = ["toggleClass", "addClass", "removeClass", "show", "hide", "scrollTo"] as const;

export const ANIME_PROP_GROUPS: Array<{ label: string; props: string[] }> = [
  { label: "Transform", props: ["translateX", "translateY", "translateZ", "rotate", "rotateX", "rotateY", "rotateZ", "scale", "scaleX", "scaleY", "skewX", "skewY", "perspective"] },
  { label: "Visibility", props: ["opacity"] },
  { label: "Dimensions", props: ["width", "height", "maxWidth", "maxHeight", "minWidth", "minHeight"] },
  { label: "Position", props: ["top", "left", "right", "bottom"] },
  { label: "Spacing", props: ["margin", "marginTop", "marginBottom", "marginLeft", "marginRight", "padding", "paddingTop", "paddingBottom", "paddingLeft", "paddingRight"] },
  { label: "Border", props: ["borderRadius", "borderWidth", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomLeftRadius", "borderBottomRightRadius"] },
  { label: "Color", props: ["color", "backgroundColor", "borderColor", "fill", "stroke", "outlineColor"] },
  { label: "Typography", props: ["fontSize", "letterSpacing", "lineHeight", "wordSpacing"] },
  { label: "Effects", props: ["boxShadow", "textShadow", "filter"] },
  { label: "SVG", props: ["strokeDashoffset", "strokeDasharray", "strokeWidth", "strokeOpacity", "fillOpacity", "r", "cx", "cy"] },
];

export const ANIME_PROP_DEFAULTS: Record<string, { from: string; to: string }> = {
  translateX: { from: "-40px", to: "0px" },
  translateY: { from: "40px", to: "0px" },
  translateZ: { from: "-100px", to: "0px" },
  rotate: { from: "0deg", to: "360deg" },
  rotateX: { from: "90deg", to: "0deg" },
  rotateY: { from: "90deg", to: "0deg" },
  rotateZ: { from: "45deg", to: "0deg" },
  scale: { from: "0.9", to: "1" },
  scaleX: { from: "0", to: "1" },
  scaleY: { from: "0", to: "1" },
  skewX: { from: "20deg", to: "0deg" },
  skewY: { from: "20deg", to: "0deg" },
  perspective: { from: "1000", to: "1000" },
  opacity: { from: "0", to: "1" },
  width: { from: "0px", to: "100%" },
  height: { from: "0px", to: "200px" },
  maxWidth: { from: "0px", to: "100%" },
  maxHeight: { from: "0px", to: "500px" },
  minWidth: { from: "0px", to: "100px" },
  minHeight: { from: "0px", to: "100px" },
  top: { from: "-20px", to: "0px" },
  left: { from: "-20px", to: "0px" },
  right: { from: "-20px", to: "0px" },
  bottom: { from: "-20px", to: "0px" },
  margin: { from: "0px", to: "16px" },
  marginTop: { from: "0px", to: "16px" },
  marginBottom: { from: "0px", to: "16px" },
  marginLeft: { from: "0px", to: "16px" },
  marginRight: { from: "0px", to: "16px" },
  padding: { from: "0px", to: "16px" },
  paddingTop: { from: "0px", to: "16px" },
  paddingBottom: { from: "0px", to: "16px" },
  paddingLeft: { from: "0px", to: "16px" },
  paddingRight: { from: "0px", to: "16px" },
  borderRadius: { from: "0px", to: "12px" },
  borderWidth: { from: "0px", to: "2px" },
  borderTopLeftRadius: { from: "0px", to: "12px" },
  borderTopRightRadius: { from: "0px", to: "12px" },
  borderBottomLeftRadius: { from: "0px", to: "12px" },
  borderBottomRightRadius: { from: "0px", to: "12px" },
  color: { from: "#000000", to: "#ffffff" },
  backgroundColor: { from: "rgba(0,0,0,0)", to: "#ffffff" },
  borderColor: { from: "rgba(0,0,0,0)", to: "#000000" },
  fill: { from: "#000000", to: "#ffffff" },
  stroke: { from: "#000000", to: "#ffffff" },
  outlineColor: { from: "rgba(0,0,0,0)", to: "#000000" },
  fontSize: { from: "12px", to: "24px" },
  letterSpacing: { from: "0px", to: "4px" },
  lineHeight: { from: "1", to: "1.6" },
  wordSpacing: { from: "0px", to: "4px" },
  boxShadow: { from: "none", to: "0 4px 20px rgba(0,0,0,0.15)" },
  textShadow: { from: "none", to: "0 2px 8px rgba(0,0,0,0.3)" },
  filter: { from: "blur(0px)", to: "blur(10px)" },
  strokeDashoffset: { from: "100", to: "0" },
  strokeDasharray: { from: "0", to: "100" },
  strokeWidth: { from: "0", to: "2" },
  strokeOpacity: { from: "0", to: "1" },
  fillOpacity: { from: "0", to: "1" },
  r: { from: "0", to: "50" },
  cx: { from: "0", to: "50" },
  cy: { from: "0", to: "50" },
};

export type AnimePreset = {
  name: string;
  easing: string;
  props: Array<{ prop: string; from: string; to: string }>;
  textEffect?: string;
  stagger?: number;
  duration?: number;
};

export const ANIME_PRESETS: AnimePreset[] = [
  { name: "Fade", easing: "outQuad", props: [{ prop: "opacity", from: "0", to: "1" }] },
  { name: "Slide ↑", easing: "outExpo", props: [{ prop: "translateY", from: "40px", to: "0px" }, { prop: "opacity", from: "0", to: "1" }] },
  { name: "Slide ↓", easing: "outExpo", props: [{ prop: "translateY", from: "-40px", to: "0px" }, { prop: "opacity", from: "0", to: "1" }] },
  { name: "Slide →", easing: "outExpo", props: [{ prop: "translateX", from: "-40px", to: "0px" }, { prop: "opacity", from: "0", to: "1" }] },
  { name: "Slide ←", easing: "outExpo", props: [{ prop: "translateX", from: "40px", to: "0px" }, { prop: "opacity", from: "0", to: "1" }] },
  { name: "Zoom In", easing: "outExpo", props: [{ prop: "scale", from: "0.85", to: "1" }, { prop: "opacity", from: "0", to: "1" }] },
  { name: "Zoom Out", easing: "outExpo", props: [{ prop: "scale", from: "1.15", to: "1" }, { prop: "opacity", from: "0", to: "1" }] },
  { name: "Bounce", easing: "outElastic", props: [{ prop: "translateY", from: "30px", to: "0px" }, { prop: "opacity", from: "0", to: "1" }] },
  { name: "Rotate", easing: "outBack", props: [{ prop: "rotate", from: "-10deg", to: "0deg" }, { prop: "opacity", from: "0", to: "1" }] },
  { name: "Flip X", easing: "outExpo", props: [{ prop: "rotateX", from: "90deg", to: "0deg" }, { prop: "opacity", from: "0", to: "1" }] },
  { name: "Flip Y", easing: "outExpo", props: [{ prop: "rotateY", from: "90deg", to: "0deg" }, { prop: "opacity", from: "0", to: "1" }] },
  { name: "Text Chars", easing: "outExpo", props: [{ prop: "translateY", from: "20px", to: "0px" }, { prop: "opacity", from: "0", to: "1" }], textEffect: "chars", stagger: 30 },
  { name: "Text Words", easing: "outExpo", props: [{ prop: "translateY", from: "20px", to: "0px" }, { prop: "opacity", from: "0", to: "1" }], textEffect: "words", stagger: 60 },
  { name: "Typewriter", easing: "linear", props: [{ prop: "opacity", from: "0", to: "1" }], textEffect: "typewriter", stagger: 40, duration: 80 },
];

export const ANIME_ALL_KNOWN_PROPS = new Set(ANIME_PROP_GROUPS.flatMap((group) => group.props));

const ANIME_TRANSFORM_PROPS = new Set(ANIME_PROP_GROUPS.find((group) => group.label === "Transform")?.props ?? []);
const ANIME_SVG_PROPS = new Set(ANIME_PROP_GROUPS.find((group) => group.label === "SVG")?.props ?? []);

export function inferAnimePropertyCategory(
  property: string,
): "css" | "transform" | "cssVariable" | "object" | "htmlAttribute" | "svgAttribute" | "utility" {
  if (property.startsWith("--")) return "cssVariable";
  if (ANIME_TRANSFORM_PROPS.has(property)) return "transform";
  if (ANIME_SVG_PROPS.has(property)) return "svgAttribute";
  return "css";
}

export function toAnimeV4Ease(value: string): string {
  if (value.startsWith("ease") && value.length > 4) return value[4].toLowerCase() + value.slice(5);
  return value;
}
