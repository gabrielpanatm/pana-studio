import assert from "node:assert/strict";
import test from "node:test";
import {
  parseBoxShadowList,
  parseTextShadowList,
  serializeBoxShadowList,
  serializeTextShadowList,
  splitShadowList,
} from "$lib/inspector/shadow-value";
import {
  parseBackgroundGradient,
  serializeBackgroundGradient,
  isBackgroundGradientStructurallyEditable,
} from "$lib/inspector/background-gradient";
import { cssRuleContextFromSource } from "$lib/css/source-sync";
import {
  captureCssPendingValueBaseline,
  restoreCssPendingValueBaseline,
} from "$lib/inspector/css-property-edit";

test("shadow list keeps commas inside color functions", () => {
  assert.deepEqual(splitShadowList(
    "0 2px 4px rgba(0, 0, 0, .2), inset 0 0 1px #fff",
  ), [
    "0 2px 4px rgba(0, 0, 0, .2)",
    "inset 0 0 1px #fff",
  ]);
});

test("structured box and text shadows round-trip supported values", () => {
  const box = parseBoxShadowList("inset 0 4px 8px 0 rgba(0, 0, 0, 0.15)");
  assert.ok(box);
  assert.equal(serializeBoxShadowList(box), "inset 0 4px 8px 0 rgba(0, 0, 0, 0.15)");

  const text = parseTextShadowList("0 2px 4px currentColor");
  assert.ok(text);
  assert.equal(serializeTextShadowList(text), "0 2px 4px currentColor");
});

test("unsupported shadow syntax stays in raw mode instead of being rewritten", () => {
  assert.equal(parseBoxShadowList("$shadow-card"), null);
  assert.equal(parseBoxShadowList("var(--shadow-card)"), null);
  assert.equal(parseTextShadowList("paint(my-shadow)"), null);
});

test("basic generated gradients round-trip through the structured editor", () => {
  const source = "linear-gradient(135deg, #ffffff 0%, rgba(0, 0, 0, 0.50) 100%)";
  assert.equal(serializeBackgroundGradient(parseBackgroundGradient(source)), source);
  assert.equal(isBackgroundGradientStructurallyEditable(source), true);
});

test("complex gradients are kept in raw mode instead of normalized destructively", () => {
  assert.equal(isBackgroundGradientStructurallyEditable(
    "repeating-linear-gradient(45deg, red, blue 10px)",
  ), false);
  assert.equal(isBackgroundGradientStructurallyEditable(
    "linear-gradient(45deg, $color-start 0%, $color-end 100%)",
  ), false);
});

test("open-source projection reads grouped desktop and exact viewport rules", () => {
  const source = `
.card, .hero { color: red; content: "}"; }
@media (max-width: $bp-mobil) {
  .card, .hero { color: blue; }
}
`;
  const context = cssRuleContextFromSource(
    source,
    "sass/pagini/index.scss",
    ".hero",
    "mobile",
  );
  assert.deepEqual(context.baseRules, [
    { property: "color", value: "red" },
    { property: "content", value: "\"}\"" },
  ]);
  assert.deepEqual(context.viewportRules, [{ property: "color", value: "blue" }]);
  assert.equal(context.hasBaseRule, true);
  assert.equal(context.hasViewportRule, true);
});

test("CSS edit cancel removes a new draft instead of serializing an empty declaration", () => {
  const pending = { color: "red" };
  const baseline = captureCssPendingValueBaseline(pending, "text-align");
  const withDraft = { ...pending, "text-align": "left" };

  assert.deepEqual(restoreCssPendingValueBaseline(withDraft, "text-align", baseline), pending);
});

test("CSS edit cancel restores the previous optimistic value during queued commits", () => {
  const pending = { "text-align": "right", color: "red" };
  const baseline = captureCssPendingValueBaseline(pending, "text-align");
  const withNewDraft = { ...pending, "text-align": "center" };

  assert.deepEqual(
    restoreCssPendingValueBaseline(withNewDraft, "text-align", baseline),
    pending,
  );
});
