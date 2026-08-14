import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { sass } from "@codemirror/lang-sass";
import { foldable, syntaxTree } from "@codemirror/language";
import { EditorState } from "@codemirror/state";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("configurația CodeMirror activează gutterul și comenzile native de folding", () => {
  const controller = source("../src/lib/editor/controller.ts");

  assert.match(controller, /foldGutter\(\)/);
  assert.match(controller, /keymap\.of\(\[\.\.\.defaultKeymap, \.\.\.foldKeymap\]\)/);
});

test("SCSS folosește parserul Sass oficial încărcat separat de CSS", () => {
  const editor = source("../src/lib/editor/codemirror.ts");
  const scssBranch = editor.match(/if \(language === "scss"\) \{[\s\S]*?\n  \}/)?.[0] ?? "";

  assert.match(scssBranch, /@codemirror\/lang-sass/);
  assert.match(scssBranch, /return sass\(\)/);
  assert.doesNotMatch(scssBranch, /lang-css|return css\(\)/);
});

test("parserul SCSS recunoaște variabilele, nestingul și intervalele pliabile", () => {
  const document = [
    "$accent: #0a8;",
    ".card {",
    "  color: $accent;",
    "  &:hover {",
    "    color: white;",
    "  }",
    "}",
  ].join("\n");
  const state = EditorState.create({ doc: document, extensions: [sass()] });
  const tree = syntaxTree(state).toString();
  const cardLine = state.doc.line(2);
  const cardFold = foldable(state, cardLine.from, cardLine.to);

  assert.match(tree, /SassVariableName/);
  assert.match(tree, /NestingSelector/);
  assert.ok(cardFold, "regula SCSS trebuie să ofere un interval pliabil");
  assert.match(document.slice(cardFold.from, cardFold.to), /color: \$accent;[\s\S]*&:hover/);
  assert.ok(cardFold.from >= cardLine.to);
  assert.ok(cardFold.to < document.length);
});
