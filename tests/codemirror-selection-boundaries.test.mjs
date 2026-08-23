import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { css } from "@codemirror/lang-css";
import { html } from "@codemirror/lang-html";
import { sass } from "@codemirror/lang-sass";
import { EditorState } from "@codemirror/state";
import { codeSelectionDecorationRanges } from "../src/lib/editor/codemirror.ts";

function highlightedSlices(source, extension, range, presentation) {
  const state = EditorState.create({ doc: source, extensions: [extension] });
  return codeSelectionDecorationRanges(state, {
    ranges: range,
    presentation,
  }).map(({ from, to }) => source.slice(from, to));
}

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("selecția HTML evidențiază numai tagurile pereche, inclusiv într-un template Tera", () => {
  const source = [
    '<section class="{{ section.class }}">',
    "  {% if visible %}",
    "    <p>{{ text }}</p>",
    "  {% endif %}",
    "</section>",
  ].join("\n");

  assert.deepEqual(
    highlightedSlices(source, html(), { from: 0, to: source.length }, "htmlElement"),
    ['<section class="{{ section.class }}">', "</section>"],
  );
});

test("elementele HTML fără tag de închidere evidențiază numai tagul disponibil", () => {
  const voidElement = '<img src="/images/card.png">';
  const selfClosing = "<custom-widget />";

  assert.deepEqual(
    highlightedSlices(voidElement, html(), { from: 0, to: voidElement.length }, "htmlElement"),
    [voidElement],
  );
  assert.deepEqual(
    highlightedSlices(selfClosing, html(), { from: 0, to: selfClosing.length }, "htmlElement"),
    [selfClosing],
  );
});

test("selecția SCSS evidențiază selectorul și acoladele regulii imbricate", () => {
  const source = [
    ".parent {",
    "  & .child {",
    "    color: red;",
    "  }",
    "}",
  ].join("\n");
  const from = source.indexOf("& .child");

  assert.deepEqual(
    highlightedSlices(source, sass(), { from, to: from + "& .child".length }, "cssRule"),
    ["& .child", "{", "}"],
  );
});

test("selecția CSS evidențiază selectorul și ambele acolade", () => {
  const source = ".card { display: grid; }";
  assert.deepEqual(
    highlightedSlices(source, css(), { from: 0, to: ".card".length }, "cssRule"),
    [".card", "{", "}"],
  );
});

test("un selector dintr-o listă folosește acoladele blocului comun", () => {
  const source = [
    ".card,",
    ".hero:hover {",
    "  color: red;",
    "}",
  ].join("\n");
  const from = source.indexOf(".hero:hover");

  assert.deepEqual(
    highlightedSlices(source, sass(), { from, to: from + ".hero:hover".length }, "cssRule"),
    [".hero:hover", "{", "}"],
  );
});

test("navigarea generică păstrează intervalul integral", () => {
  const source = "alpha beta gamma";
  assert.deepEqual(
    highlightedSlices(source, [], { from: 6, to: 10 }, "range"),
    ["beta"],
  );
});

test("proiecția semantică diferențiază selecțiile HTML și CSS fără a schimba range-ul Rust", () => {
  const controller = source("../src/lib/editor/source-workspace.svelte.ts");
  const editor = source("../src/lib/editor/controller.ts");

  assert.match(controller, /selection\?\.focus\.kind === "cssRule"[\s\S]*\? "cssRule"/);
  assert.match(controller, /primary\?\.subject\.kind === "htmlElement"[\s\S]*\? "htmlElement"/);
  assert.match(editor, /setSelectedSourceProjection\.of\(range \? \{ ranges: range, presentation \} : null\)/);
});
