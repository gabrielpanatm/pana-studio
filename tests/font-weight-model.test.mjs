import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { resolveFontWeightPreset } from "$lib/inspector/font-weight-model";

test("font-weight păstrează tokenul sursă și rezolvă numai presetul vizual", () => {
  const variables = [
    { name: "font-normal", value: "400", file: "sass/_variables.scss" },
    { name: "font-strong", value: "$font-bold", file: "sass/_variables.scss" },
    { name: "font-bold", value: "700", file: "sass/_variables.scss" },
  ];

  assert.equal(resolveFontWeightPreset("$font-normal", variables), "400");
  assert.equal(resolveFontWeightPreset("$font-strong", variables), "700");
  assert.equal(resolveFontWeightPreset("bold", variables), "700");
  assert.equal(resolveFontWeightPreset("600", variables), "600");
});

test("font-weight nu pretinde că poate evalua expresii sau tokenuri invalide", () => {
  const variables = [
    { name: "a", value: "$b", file: "sass/_variables.scss" },
    { name: "b", value: "$a", file: "sass/_variables.scss" },
    { name: "dynamic", value: "calc(400 + 100)", file: "sass/_variables.scss" },
  ];

  assert.equal(resolveFontWeightPreset("$a", variables), null);
  assert.equal(resolveFontWeightPreset("$dynamic", variables), null);
  assert.equal(resolveFontWeightPreset("$missing", variables), null);
  assert.equal(resolveFontWeightPreset("950", variables), null);
});

test("controlul de grosime folosește aceeași proprietate pentru token și preseturi", () => {
  const component = readFileSync(
    new URL("../src/lib/components/inspector/sections/TypographySection.svelte", import.meta.url),
    "utf8",
  );

  assert.match(component, /variablesForProperty\("font-weight", scssVariables\)/);
  assert.match(component, /resolveFontWeightPreset\(fontWeightValue, scssVariables\)/);
  assert.match(component, /edit\.continuous\("font-weight"\)/);
  assert.doesNotMatch(component, /toggleable=\{false\}/);
  assert.doesNotMatch(component, /if \(value === resolvedFontWeight\) return/);
  assert.match(
    component,
    /function selectFontWeightPreset\(value: string\) \{\s*edit\.commit\("font-weight", value\);\s*\}/,
  );
});
