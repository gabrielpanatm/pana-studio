import type { CssSelectorOption } from "$lib/css/contracts";
import type { CanvasElementObservation } from "$lib/canvas/contracts";

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

function addMatchedRuleOptions(options: CssSelectorOption[], selection: CanvasElementObservation) {
  for (const rule of selection.matchedRules) {
    if (isInlineRule(rule.selector)) {
      continue;
    }

    addOption(options, {
      selector: rule.selector,
      label: rule.selector,
      source: "matched",
      detailKind: "matched_rule",
      detailSource: rule.source,
    });
  }
}

export function selectorOptionsForObservation(selection: CanvasElementObservation | null): CssSelectorOption[] {
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
      detailKind: "all_element_classes",
    });
  }

  for (const className of classes) {
    addOption(options, {
      selector: `.${className}`,
      label: `.${className}`,
      source: "class",
      detailKind: "element_class",
    });
  }

  if (selection.id) {
    const selector = `#${escapeCssIdentifier(selection.id)}`;
    addOption(options, {
      selector,
      label: selector,
      source: "id",
      detailKind: "element_id",
    });
  }

  if (!hasStableSelector && selection.cssSelector && selection.cssSelector !== tag) {
    addOption(options, {
      selector: selection.cssSelector,
      label: selection.cssSelector,
      source: "compound",
      detailKind: "generated_without_class_or_id",
    });
  }

  addOption(options, {
    selector: tag,
    label: tag,
    source: "tag",
    detailKind: "tag_fallback",
  });

  return options;
}

export function defaultSelectorForObservation(selection: CanvasElementObservation | null) {
  return selectorOptionsForObservation(selection)[0]?.selector ?? "";
}
