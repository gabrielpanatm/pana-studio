import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  ColorPickerEditSession,
  inferPickerColorSpace,
  isPickerColorValue,
  resolvePickerColor,
} from "$lib/inspector/color-picker-model";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("pickerul este o componentă Pană Studio, nu o integrare Webstudio", () => {
  const component = source("../src/lib/components/ui/PanaColorPicker.svelte");
  const packageJson = JSON.parse(source("../package.json"));

  assert.equal(packageJson.dependencies["hdr-color-input"], undefined);
  assert.equal(packageJson.dependencies["colorjs.io"], "^0.6.1");
  assert.doesNotMatch(component, /hdr-color-input|<color-input|<canvas/);
  assert.match(component, /class="color-area"/);
  assert.match(component, /linear-gradient\(to top, #000, transparent\)/);
  assert.match(component, /use:portal/);
});

test("pickerul acceptă CSS Color 4 fără a trata expresiile dinamice drept culori normalizabile", () => {
  for (const value of [
    "#0f08",
    "rgb(20 40 60 / .5)",
    "oklch(68% .18 245 / 80%)",
    "color(display-p3 .2 .7 .9)",
  ]) {
    assert.equal(isPickerColorValue(value), true, value);
  }

  for (const value of ["var(--brand)", "color-mix(in oklab, red, blue)", "currentColor", "$brand"]) {
    assert.equal(isPickerColorValue(value), false, value);
  }
});

test("spațiul de editare este dedus fără rescrierea valorii sursă", () => {
  assert.equal(inferPickerColorSpace("#336699cc"), "hex");
  assert.equal(inferPickerColorSpace("hsl(220 50% 40%)"), "hsl");
  assert.equal(inferPickerColorSpace("oklch(60% .2 240)"), "oklch");
  assert.equal(inferPickerColorSpace("color(display-p3 .2 .3 .4)"), "display-p3");
  assert.equal(inferPickerColorSpace("color(prophoto-rgb .2 .3 .4)"), "prophoto");
});

test("variabilele SCSS sunt rezolvate doar pentru preview și ciclurile sunt respinse", () => {
  const variables = [
    { name: "brand", value: "$brand-raw" },
    { name: "brand-raw", value: "#336699" },
    { name: "a", value: "$b" },
    { name: "b", value: "$a" },
  ];

  assert.equal(resolvePickerColor("$brand", variables), "#336699");
  assert.equal(resolvePickerColor("$a", variables), null);
  assert.equal(resolvePickerColor("$missing", variables), null);
});

test("sesiunea separă preview, commit și cancel pentru autoritatea Rust", () => {
  const session = new ColorPickerEditSession("#112233");
  assert.equal(session.preview("#445566"), "#445566");
  session.preview("#778899");
  assert.equal(session.commit(), "#778899", "toate preview-urile produc un singur commit final");
  assert.equal(session.commit(), null, "o sesiune finalizată nu poate produce încă un commit");

  const cancelled = new ColorPickerEditSession("#112233");
  cancelled.preview("#445566");
  cancelled.preview("#778899");
  assert.equal(cancelled.cancel(), "#112233", "Escape revine la valoarea de la deschidere");
  assert.equal(cancelled.commit(), null, "anularea nu lasă o mutație comisibilă");
});

test("componenta nu comite interacțiunile interne ale pickerului", () => {
  const component = source("../src/lib/components/ui/PanaColorPicker.svelte");

  assert.equal(
    component.match(/commitLatest\(\)/g)?.length,
    2,
    "commitLatest există numai ca funcție și ca graniță de închidere",
  );
  assert.doesNotMatch(component, /onchange=\{commitLatest\}/);
  assert.match(component, /registerEditFlushHandler\([\s\S]*closePicker\(true\)/);
});
