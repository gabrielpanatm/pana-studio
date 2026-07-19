import type { CssRuleMatch, CssVariableRow, EditableStyles, SelectionInfo } from "$lib/types";

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

export function getEditableStyles(element: Element, previewWindow: Window): EditableStyles {
  const computed = previewWindow.getComputedStyle(element);

  return {
    color: toHexColor(computed.color, "#17211d"),
    backgroundColor: toHexColor(computed.backgroundColor, "#ffffff"),
    fontSize: computed.fontSize,
    lineHeight: computed.lineHeight,
    textAlign: computed.textAlign,
    margin: formatBox(computed, "margin"),
    padding: formatBox(computed, "padding"),
    borderRadius: computed.borderRadius,
    display: computed.display,
    flexDirection: computed.flexDirection,
    gap: computed.gap,
    justifyContent: computed.justifyContent,
    alignItems: computed.alignItems,
  };
}

export function getEditableStylesFromSelection(selection: SelectionInfo): EditableStyles {
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

export function collectMatchedCssRules(element: Element, previewWindow: Window): CssRuleMatch[] {
  const matches: CssRuleMatch[] = [];
  const seen = new Set<string>();
  const document = previewWindow.document;
  const inlineStyle = element.getAttribute("style");

  if (inlineStyle?.trim()) {
    const inlineDeclarationCount = inlineStyle
      .split(";")
      .map((entry) => entry.trim())
      .filter(Boolean).length;

    matches.push({
      selector: 'style=""',
      source: "inline",
      media: null,
      declarations: inlineDeclarationCount,
      kind: "inline",
      score: 1000,
    });
  }

  for (const sheet of Array.from(document.styleSheets)) {
    collectMatchedCssRulesFromStyleSheet(sheet, element, matches, seen);
  }

  return matches.sort((left, right) => right.score - left.score || right.declarations - left.declarations);
}

function collectMatchedCssRulesFromStyleSheet(
  sheet: StyleSheet,
  element: Element,
  matches: CssRuleMatch[],
  seen: Set<string>,
) {
  if (!("cssRules" in sheet)) {
    return;
  }

  const cssSheet = sheet as CSSStyleSheet;
  let rules: CSSRuleList | undefined;

  try {
    rules = cssSheet.cssRules;
  } catch {
    return;
  }

  if (rules === undefined) {
    return;
  }

  const source = stylesheetSourceLabel(cssSheet.href);
  collectMatchedCssRulesFromRuleList(rules, element, matches, seen, source, null);
}

function collectMatchedCssRulesFromRuleList(
  rules: CSSRuleList,
  element: Element,
  matches: CssRuleMatch[],
  seen: Set<string>,
  source: string,
  media: string | null,
) {
  for (const rule of Array.from(rules)) {
    if (rule instanceof CSSStyleRule) {
      try {
        if (!element.matches(rule.selectorText)) {
          continue;
        }
      } catch {
        continue;
      }

      const key = `${source}|${media ?? ""}|${rule.selectorText}`;

      if (seen.has(key)) {
        continue;
      }

      seen.add(key);
      const matchMeta = describeRuleMatch(rule.selectorText, element);
      matches.push({
        selector: rule.selectorText,
        source,
        media,
        declarations: Array.from(rule.style).length,
        kind: matchMeta.kind,
        score: matchMeta.score,
      });
      continue;
    }

    if (rule instanceof CSSMediaRule) {
      collectMatchedCssRulesFromRuleList(
        rule.cssRules,
        element,
        matches,
        seen,
        source,
        rule.conditionText,
      );
      continue;
    }

    if (rule instanceof CSSSupportsRule) {
      const nextMedia = media ? `${media} | supports ${rule.conditionText}` : `supports ${rule.conditionText}`;
      collectMatchedCssRulesFromRuleList(rule.cssRules, element, matches, seen, source, nextMedia);
    }
  }
}

function stylesheetSourceLabel(href: string | null) {
  if (!href) {
    return "<" + "style>";
  }

  try {
    const url = new URL(href);
    return url.pathname || href;
  } catch {
    return href;
  }
}

function describeRuleMatch(selectorText: string, element: Element) {
  const selectors = selectorText
    .split(",")
    .map((selector) => selector.trim())
    .filter(Boolean);

  let best = { kind: "descendant", score: 0 };

  for (const selector of selectors) {
    let score = 0;

    try {
      if (!element.matches(selector)) {
        continue;
      }
    } catch {
      continue;
    }

    const ids = (selector.match(/#[A-Za-z0-9_-]+/g) ?? []).length;
    const classes = (selector.match(/\.[A-Za-z0-9_-]+/g) ?? []).length;
    const attributes = (selector.match(/\[[^\]]+\]/g) ?? []).length;
    const pseudos = (selector.match(/:(?!:)[A-Za-z0-9_-]+/g) ?? []).length;
    const combinators = (selector.match(/[\s>+~]+/g) ?? []).length;
    const tagMatch =
      selector === element.tagName.toLowerCase() ||
      selector.startsWith(`${element.tagName.toLowerCase()}.`) ||
      selector.startsWith(`${element.tagName.toLowerCase()}#`);

    score += ids * 100;
    score += classes * 10;
    score += attributes * 8;
    score += pseudos * 4;
    score += tagMatch ? 6 : 0;
    score -= combinators * 3;

    let kind = "descendant";

    if (selector.startsWith("#")) {
      kind = "id";
    } else if (selector === element.tagName.toLowerCase()) {
      kind = "tag";
    } else if (combinators === 0 && (classes > 0 || attributes > 0 || pseudos > 0)) {
      kind = "direct";
    } else if (combinators > 0) {
      kind = "nested";
    }

    if (score > best.score) {
      best = { kind, score };
    }
  }

  return best;
}

export function collectRelevantCssVariables(element: Element, previewWindow: Window): CssVariableRow[] {
  const variableNames = new Set<string>();
  const document = previewWindow.document;
  const computed = previewWindow.getComputedStyle(element);

  collectVariableNamesFromValue(element.getAttribute("style") ?? "", variableNames);

  for (const sheet of Array.from(document.styleSheets)) {
    collectVariableNamesFromStyleSheet(sheet, element, variableNames);
  }

  return Array.from(variableNames)
    .map((name) => ({
      name,
      value:
        computed.getPropertyValue(name).trim() ||
        previewWindow.getComputedStyle(document.documentElement).getPropertyValue(name).trim(),
    }))
    .filter((variable) => variable.value.length > 0)
    .sort((left, right) => left.name.localeCompare(right.name));
}

function collectVariableNamesFromStyleSheet(sheet: StyleSheet, element: Element, variableNames: Set<string>) {
  if (!("cssRules" in sheet)) {
    return;
  }

  const cssSheet = sheet as CSSStyleSheet;
  let rules: CSSRuleList | undefined;

  try {
    rules = cssSheet.cssRules;
  } catch {
    return;
  }

  if (rules === undefined) {
    return;
  }

  collectVariableNamesFromRuleList(rules, element, variableNames);
}

function collectVariableNamesFromRuleList(rules: CSSRuleList, element: Element, variableNames: Set<string>) {
  for (const rule of Array.from(rules)) {
    if (rule instanceof CSSStyleRule) {
      try {
        if (!element.matches(rule.selectorText)) {
          continue;
        }
      } catch {
        continue;
      }

      for (const propertyName of Array.from(rule.style)) {
        collectVariableNamesFromValue(rule.style.getPropertyValue(propertyName), variableNames);
      }
      continue;
    }

    if (rule instanceof CSSMediaRule || rule instanceof CSSSupportsRule) {
      collectVariableNamesFromRuleList(rule.cssRules, element, variableNames);
    }
  }
}

function collectVariableNamesFromValue(value: string, variableNames: Set<string>) {
  for (const name of extractVariableNames(value)) {
    variableNames.add(name);
  }
}

function extractVariableNames(value: string) {
  const matches = value.matchAll(/var\(\s*(--[A-Za-z0-9_-]+)/g);
  return Array.from(matches, (match) => match[1]);
}
