import type { EditableStyles } from "$lib/css/contracts";
import type { CanvasElementObservation } from "$lib/canvas/contracts";

export function formatBox(style: CSSStyleDeclaration, property: "margin" | "padding") {
  const top = style.getPropertyValue(`${property}-top`);
  const right = style.getPropertyValue(`${property}-right`);
  const bottom = style.getPropertyValue(`${property}-bottom`);
  const left = style.getPropertyValue(`${property}-left`);

  if (top === right && right === bottom && bottom === left) {
    return top;
  }

  if (top === bottom && right === left) {
    return `${top} ${right}`;
  }

  return `${top} ${right} ${bottom} ${left}`;
}

export function toHexColor(value: string, fallback: string) {
  const match = value.match(/^rgba?\((\d+),\s*(\d+),\s*(\d+)(?:,\s*([0-9.]+))?\)$/);

  if (!match) {
    return fallback;
  }

  const alpha = match[4] ? Number(match[4]) : 1;

  if (alpha === 0) {
    return fallback;
  }

  return `#${[match[1], match[2], match[3]]
    .map((channel) => Number(channel).toString(16).padStart(2, "0"))
    .join("")}`;
}

export function getEditableStylesFromObservation(selection: CanvasElementObservation): EditableStyles {
  const color = selection.styles.find((style) => style.label === "color")?.value ?? "#17211d";
  const background = selection.styles.find((style) => style.label === "background")?.value ?? "#ffffff";
  const fontSize = selection.styles.find((style) => style.label === "font-size")?.value ?? "16px";
  const lineHeight = selection.styles.find((style) => style.label === "line-height")?.value ?? "normal";
  const textAlign = selection.styles.find((style) => style.label === "text-align")?.value ?? "left";
  const margin = selection.styles.find((style) => style.label === "margin")?.value ?? "0px";
  const padding = selection.styles.find((style) => style.label === "padding")?.value ?? "0px";
  const borderRadius = selection.styles.find((style) => style.label === "border-radius")?.value ?? "0px";
  const display = selection.styles.find((style) => style.label === "display")?.value ?? "block";
  const flexDirection = selection.styles.find((style) => style.label === "flex-direction")?.value ?? "row";
  const gap = selection.styles.find((style) => style.label === "gap")?.value ?? "0px";
  const justifyContent = selection.styles.find((style) => style.label === "justify-content")?.value ?? "normal";
  const alignItems = selection.styles.find((style) => style.label === "align-items")?.value ?? "normal";

  return {
    color: toHexColor(color, "#17211d"),
    backgroundColor: toHexColor(background, "#ffffff"),
    fontSize,
    lineHeight,
    textAlign,
    margin,
    padding,
    borderRadius,
    display,
    flexDirection,
    gap,
    justifyContent,
    alignItems,
  };
}
