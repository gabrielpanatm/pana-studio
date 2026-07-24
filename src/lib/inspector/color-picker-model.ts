export type PickerColorSpace =
  | "hex"
  | "srgb"
  | "srgb-linear"
  | "hsl"
  | "hwb"
  | "lab"
  | "lch"
  | "oklab"
  | "oklch"
  | "display-p3"
  | "a98-rgb"
  | "rec2020"
  | "prophoto"
  | "xyz"
  | "xyz-d50"
  | "xyz-d65";

type ColorVariable = Readonly<{ name: string; value: string }>;

const COLOR_FUNCTION = /^(?:rgb|rgba|hsl|hsla|hwb|lab|lch|oklab|oklch)\(/i;
const WIDE_COLOR_FUNCTION = /^color\(\s*(srgb-linear|display-p3|a98-rgb|rec2020|prophoto-rgb|xyz(?:-d50|-d65)?)\b/i;
const NAMED_COLOR = /^(?:[a-z]+)$/i;

/**
 * Concrete CSS colors that can be edited visually. Expressions such as var(),
 * color-mix() and currentColor stay editable as source text, because resolving
 * and normalising them here would destroy author intent.
 */
export function isPickerColorValue(value: string): boolean {
  const candidate = value.trim();
  if (/^#[\da-f]{3,4}(?:[\da-f]{2}){0,2}$/i.test(candidate)) {
    return candidate.length === 4 || candidate.length === 5 || candidate.length === 7 || candidate.length === 9;
  }
  if (COLOR_FUNCTION.test(candidate) || WIDE_COLOR_FUNCTION.test(candidate)) return true;
  return NAMED_COLOR.test(candidate) && candidate.toLowerCase() !== "currentcolor";
}

export function inferPickerColorSpace(value: string): PickerColorSpace {
  const candidate = value.trim().toLowerCase();
  if (candidate.startsWith("#")) return "hex";
  if (candidate.startsWith("hsl")) return "hsl";
  if (candidate.startsWith("hwb")) return "hwb";
  if (candidate.startsWith("oklch")) return "oklch";
  if (candidate.startsWith("oklab")) return "oklab";
  if (candidate.startsWith("lch")) return "lch";
  if (candidate.startsWith("lab")) return "lab";

  const wide = candidate.match(WIDE_COLOR_FUNCTION)?.[1];
  if (wide === "prophoto-rgb") return "prophoto";
  if (wide) return wide as PickerColorSpace;
  return "srgb";
}

/** Resolve a SCSS variable only for the visual picker; the authored token stays intact. */
export function resolvePickerColor(
  value: string,
  variables: readonly ColorVariable[] = [],
): string | null {
  let candidate = value.trim();
  const visited = new Set<string>();

  while (candidate.startsWith("$")) {
    const name = candidate.slice(1);
    if (!name || visited.has(name)) return null;
    visited.add(name);
    const variable = variables.find((entry) => entry.name === name);
    if (!variable) return null;
    candidate = variable.value.trim();
  }

  return isPickerColorValue(candidate) ? candidate : null;
}

/**
 * Keeps one picker opening inside one deterministic edit transaction.
 * Previews remain local until the picker closes; Escape restores the value
 * captured when the transaction started.
 */
export class ColorPickerEditSession {
  readonly initialValue: string;
  currentValue: string;
  private finalized = false;

  constructor(value: string) {
    this.initialValue = value;
    this.currentValue = value;
  }

  preview(value: string): string {
    if (this.finalized) return this.currentValue;
    this.currentValue = value;
    return value;
  }

  commit(): string | null {
    if (this.finalized) return null;
    this.finalized = true;
    if (this.currentValue === this.initialValue) return null;
    return this.currentValue;
  }

  cancel(): string {
    this.finalized = true;
    this.currentValue = this.initialValue;
    return this.currentValue;
  }
}
