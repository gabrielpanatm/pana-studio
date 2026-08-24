import type { ScssVariable } from "$lib/css/contracts";

const FONT_WEIGHT_KEYWORDS: Readonly<Record<string, string>> = Object.freeze({
  normal: "400",
  bold: "700",
});

const FONT_WEIGHT_PRESETS = new Set([
  "300", "400", "500", "600", "700", "800", "900",
]);

/**
 * Resolves only direct SCSS token chains for presentation. The authored value
 * remains untouched and is still committed as `$token` by the Inspector.
 */
export function resolveFontWeightPreset(
  value: string,
  variables: readonly ScssVariable[] = [],
): string | null {
  let candidate = value.trim();
  const visited = new Set<string>();

  while (candidate.startsWith("$")) {
    const name = candidate.slice(1).trim();
    if (!name || visited.has(name)) return null;
    visited.add(name);
    const variable = variables.find((entry) => entry.name === name);
    if (!variable) return null;
    candidate = variable.value.trim();
  }

  const keyword = FONT_WEIGHT_KEYWORDS[candidate.toLowerCase()];
  if (keyword) return keyword;
  return FONT_WEIGHT_PRESETS.has(candidate) ? candidate : null;
}
