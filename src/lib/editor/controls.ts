import type { ScssVariable } from "$lib/types";

const VARIABLE_PREFIX_MAP: Record<string, string> = {
  "color": "color-", "background-color": "color-", "border-color": "color-",
  "outline-color": "color-", "fill": "color-", "stroke": "color-",
  "text-decoration-color": "color-", "caret-color": "color-",
  "gap": "space-", "row-gap": "space-", "column-gap": "space-",
  "padding": "space-", "padding-top": "space-", "padding-right": "space-",
  "padding-bottom": "space-", "padding-left": "space-",
  "padding-block": "space-", "padding-inline": "space-",
  "margin": "space-", "margin-top": "space-", "margin-right": "space-",
  "margin-bottom": "space-", "margin-left": "space-",
  "font-size": "text-", "font-weight": "font-", "font-family": "font-",
  "line-height": "leading-", "letter-spacing": "tracking-",
  "border-radius": "radius-",
  "border-top-left-radius": "radius-", "border-top-right-radius": "radius-",
  "border-bottom-left-radius": "radius-", "border-bottom-right-radius": "radius-",
  "width": "size-", "height": "size-",
  "min-width": "size-", "min-height": "size-",
  "max-width": "size-", "max-height": "size-",
};

export function variablesForProperty(property: string, variables: ScssVariable[]): ScssVariable[] {
  const prefix = VARIABLE_PREFIX_MAP[property];
  if (!prefix) return [];
  return variables.filter(v => v.name.startsWith(prefix));
}

export const SELECT_OPTIONS: Record<string, string[]> = {
  display: ["block", "inline", "inline-block", "flex", "inline-flex", "grid", "inline-grid", "none"],
  "flex-direction": ["row", "column", "row-reverse", "column-reverse"],
  "flex-wrap": ["nowrap", "wrap", "wrap-reverse"],
  "justify-content": ["normal", "flex-start", "center", "flex-end", "space-between", "space-around", "space-evenly"],
  "align-items": ["normal", "stretch", "flex-start", "center", "flex-end", "baseline"],
  "align-self": ["auto", "stretch", "flex-start", "center", "flex-end", "baseline"],
  "text-align": ["left", "center", "right", "justify", "start", "end"],
  "text-transform": ["none", "uppercase", "lowercase", "capitalize"],
  "font-style": ["normal", "italic"],
  "text-decoration": ["none", "underline", "line-through"],
  overflow: ["visible", "hidden", "clip", "scroll", "auto"],
  "overflow-x": ["visible", "hidden", "clip", "scroll", "auto"],
  "overflow-y": ["visible", "hidden", "clip", "scroll", "auto"],
  position: ["static", "relative", "absolute", "fixed", "sticky"],
  cursor: ["auto", "default", "pointer", "grab", "not-allowed", "none"],
  "white-space": ["normal", "nowrap", "pre", "pre-wrap"],
  "pointer-events": ["auto", "none"],
  "box-sizing": ["content-box", "border-box"],
  "object-fit": ["fill", "contain", "cover", "none", "scale-down"],
};

const COLOR_PROPS = [
  "color", "background-color", "border-color", "outline-color",
  "border-top-color", "border-right-color", "border-bottom-color", "border-left-color",
  "fill", "stroke", "caret-color", "text-decoration-color",
];

export type ControlType = "color" | "select" | "text";

export function isHexColor(value: string): boolean {
  const t = value.trim();
  return /^#[0-9a-fA-F]{3}$/.test(t) || /^#[0-9a-fA-F]{6}$/.test(t) || /^#[0-9a-fA-F]{8}$/.test(t);
}

export function controlTypeFor(property: string, value?: string): ControlType {
  if (COLOR_PROPS.includes(property)) {
    if (value !== undefined && isHexColor(value)) return "color";
    return "text";
  }
  if (property in SELECT_OPTIONS) return "select";
  return "text";
}

export function resolvedColorValue(value: string): string {
  const trimmed = value.trim();
  if (/^#[0-9a-fA-F]{3}$/.test(trimmed)) return trimmed;
  if (/^#[0-9a-fA-F]{6}$/.test(trimmed)) return trimmed;
  if (/^#[0-9a-fA-F]{8}$/.test(trimmed)) return trimmed.slice(0, 7);
  return "#000000";
}
