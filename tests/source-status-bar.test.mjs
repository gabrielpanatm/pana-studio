import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { measureSourceText } from "$lib/editor/source-metrics";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("metricile sursei numără liniile și dimensiunea UTF-8 exact", () => {
  assert.deepEqual(measureSourceText(""), {
    characterCount: 0,
    lineCount: 1,
    utf8Bytes: 0,
  });
  assert.deepEqual(measureSourceText("a\r\nb\rc\n🙂"), {
    characterCount: 9,
    lineCount: 4,
    utf8Bytes: 11,
  });
});

test("modul Code folosește un status bar inferior, fără header superior", () => {
  const editor = source("../src/lib/components/EditorShell.svelte");
  const editorHost = editor.indexOf('class="code-editor-host"');
  const statusBar = editor.indexOf('<footer class="source-status-bar"');

  assert.doesNotMatch(editor, /class="source-header"/);
  assert.ok(statusBar > editorHost, "status bar-ul trebuie să fie după editor");
  assert.match(editor, /grid-template-rows:\s*minmax\(0, 1fr\) auto/);
  assert.match(editor, /class="source-path"[\s\S]*currentSourcePath/);
  assert.match(editor, /workbench-character-count/);
  assert.match(editor, /workbench-line-count/);
  assert.match(editor, /formattedSourceSize/);
  assert.match(editor, /\.source-status-bar\s*\{[\s\S]*min-height:\s*36px;[\s\S]*border-top:/);
});
