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
  backgroundFromProperties,
  parseCssGradient,
  serializeBackgroundLonghands,
  serializeCssGradient,
  splitTopLevelCssList,
} from "$lib/inspector/background-model";
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

test("background layers keep top-level commas distinct from functions, URLs and SCSS", () => {
  assert.deepEqual(splitTopLevelCssList(
    'linear-gradient(rgb(1, 2, 3), var(--x)), url("data:image/svg+xml,a,b"), image-set(url(#{asset($name)}) 1x, url("b,c") 2x)',
  ), [
    "linear-gradient(rgb(1, 2, 3), var(--x))",
    'url("data:image/svg+xml,a,b")',
    'image-set(url(#{asset($name)}) 1x, url("b,c") 2x)',
  ]);
  assert.deepEqual(splitTopLevelCssList(
    "url('/a.png') /* keep, comma */, linear-gradient(red, blue)",
  ), ["url('/a.png') /* keep, comma */", "linear-gradient(red, blue)"]);
});

test("layered backgrounds follow CSS list repetition and serialize stable longhands", () => {
  const model = backgroundFromProperties({
    "background-color": "$fundal",
    "background-image": "linear-gradient(45deg, #fff 0%, #000 100%), url('/grain.png'), radial-gradient(circle at center, red 0%, blue 100%)",
    "background-position": "center, top left",
    "background-size": "cover",
    "background-repeat": "no-repeat",
  });
  assert.equal(model.layers.length, 3);
  assert.equal(model.layers[2].position, "center");
  assert.equal(model.layers[1].size, "cover");
  assert.equal(model.color, "$fundal");

  const serialized = serializeBackgroundLonghands(model);
  const reparsed = backgroundFromProperties(serialized);
  assert.equal(reparsed.layers.length, 3);
  assert.equal(reparsed.layers[2].source, model.layers[2].source);
  assert.equal(reparsed.layers[2].position, "center");
});

test("empty background list slots remain editable and serialize to CSS initial values", () => {
  const model = backgroundFromProperties({
    "background-image": "url('/hero.png'), linear-gradient(red, blue)",
    "background-position": ", center",
    "background-size": "cover, ",
  });

  assert.deepEqual(model.opaqueProperties, {});
  assert.deepEqual(model.layers.map((layer) => layer.position), ["0% 0%", "center"]);
  assert.deepEqual(model.layers.map((layer) => layer.size), ["cover", "auto"]);

  model.layers[0].position = "";
  model.layers[1].size = "";
  const serialized = serializeBackgroundLonghands(model);
  assert.equal(serialized["background-position"], "0% 0%, center");
  assert.equal(serialized["background-size"], "cover, auto");
  assert.deepEqual(backgroundFromProperties(serialized).opaqueProperties, {});
});

test("advanced gradients preserve repeating types, hints, units and dynamic colors", () => {
  const source = "repeating-linear-gradient(to right, $start 0 12px, 18px, color-mix(in oklab, red, blue) 24px 36px)";
  const gradient = parseCssGradient(source);
  assert.ok(gradient);
  assert.equal(gradient.kind, "linear");
  assert.equal(gradient.repeating, true);
  assert.equal(gradient.prelude, "to right");
  assert.equal(gradient.structurallyEditable, true);
  assert.equal(gradient.items[1].kind, "hint");
  assert.deepEqual(gradient.items[2].positions, ["24px", "36px"]);
  assert.equal(serializeCssGradient(gradient), source);
});

test("radial, conic and opaque background values retain their authored form", () => {
  for (const source of [
    "radial-gradient(ellipse closest-side at 20% 30%, red 0%, blue 100%)",
    "conic-gradient(from .25turn at center, red 0deg, blue 1turn)",
  ]) {
    const gradient = parseCssGradient(source);
    assert.ok(gradient);
    assert.equal(serializeCssGradient(gradient), source);
  }
  const opaque = backgroundFromProperties({ "background-image": "$fundal-dinamic" });
  assert.equal(opaque.layers[0].kind, "opaque");
  assert.equal(opaque.layers[0].source, "$fundal-dinamic");
  assert.equal(opaque.structurallyEditable, false);
  const shorthand = backgroundFromProperties({ background: "center / cover no-repeat url('/hero.jpg') #111" });
  assert.equal(shorthand.shorthand, "center / cover no-repeat url('/hero.jpg') #111");
  assert.equal(shorthand.structurallyEditable, false);
  const dynamicList = backgroundFromProperties({
    "background-image": "url('/a.png'), url('/b.png')",
    "background-position": "$pozitii-fundal",
  });
  assert.equal(dynamicList.opaqueProperties["background-position"], "$pozitii-fundal");
  assert.equal(serializeBackgroundLonghands(dynamicList)["background-position"], "$pozitii-fundal");
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
  assert.equal(context.background.layers.length, 0);
  assert.equal(context.grid.schemaVersion, 1);
});

test("background projection cascades partial viewport longhands over desktop layers", () => {
  const context = cssRuleContextFromSource(`
.hero {
  background-image: url('/hero.webp'), linear-gradient(red, blue);
  background-size: cover;
  background-repeat: no-repeat;
}
@media (max-width: $bp-mobil) {
  .hero { background-size: contain, 100% 100%; }
}
`, "sass/pagini/index.scss", ".hero", "mobile");

  assert.equal(context.viewportRules.length, 1);
  assert.equal(context.background.layers.length, 2);
  assert.deepEqual(context.background.layers.map((layer) => layer.size), ["contain", "100% 100%"]);
  assert.deepEqual(context.background.layers.map((layer) => layer.repeat), ["no-repeat", "no-repeat"]);
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
