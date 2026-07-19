import type { CssSelectorOption, SelectionInfo } from "$lib/types";

function escapeCssIdentifier(value: string) {
  return value.replace(/[^A-Za-z0-9_-]/g, (character) => `\\${character}`);
}

function addOption(options: CssSelectorOption[], option: CssSelectorOption) {
  if (options.some((item) => item.selector === option.selector)) {
    return;
  }

  options.push(option);
}

function isInlineRule(selector: string) {
  return selector === 'style=""';
}

function addMatchedRuleOptions(options: CssSelectorOption[], selection: SelectionInfo) {
  for (const rule of selection.matchedRules) {
    if (isInlineRule(rule.selector)) {
      continue;
    }

    addOption(options, {
      selector: rule.selector,
      label: rule.selector,
      source: "matched",
      detail: `${rule.source} · regula existenta`,
    });
  }
}

export function selectorOptionsForSelection(selection: SelectionInfo | null): CssSelectorOption[] {
  if (!selection) {
    return [];
  }

  const options: CssSelectorOption[] = [];
  const tag = selection.tag;
  const classes = selection.classes.map(escapeCssIdentifier);
  const hasStableSelector = classes.length > 0 || Boolean(selection.id);

  addMatchedRuleOptions(options, selection);

  if (classes.length > 1) {
    addOption(options, {
      selector: `.${classes.join(".")}`,
      label: `.${classes.join(".")}`,
      source: "compound",
      detail: "toate clasele elementului",
    });
  }

  for (const className of classes) {
    addOption(options, {
      selector: `.${className}`,
      label: `.${className}`,
      source: "class",
      detail: "clasa elementului",
    });
  }

  if (selection.id) {
    const selector = `#${escapeCssIdentifier(selection.id)}`;
    addOption(options, {
      selector,
      label: selector,
      source: "id",
      detail: "id element",
    });
  }

  if (!hasStableSelector && selection.cssSelector && selection.cssSelector !== tag) {
    addOption(options, {
      selector: selection.cssSelector,
      label: selection.cssSelector,
      source: "compound",
      detail: "selector generat fara clasa/id",
    });
  }

  addOption(options, {
    selector: tag,
    label: tag,
    source: "tag",
    detail: "tag fallback",
  });

  return options;
}

export function defaultSelectorForSelection(selection: SelectionInfo | null) {
  return selectorOptionsForSelection(selection)[0]?.selector ?? "";
}
