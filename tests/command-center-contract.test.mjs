import assert from "node:assert/strict";
import { test } from "node:test";

import { appShortcutIntent } from "../src/lib/ui/app-shortcuts.ts";
import { commandCenterQuery } from "../src/lib/workbench/command-center.ts";

test("Ctrl+K deschide Command Center înaintea shortcut-urilor editorului", () => {
  assert.equal(appShortcutIntent({
    key: "k",
    ctrlKey: true,
    metaKey: false,
    altKey: false,
    shiftKey: false,
    target: null,
  }), "commandCenter");
});

test("shortcut-urile shell-ului folosesc convențiile IDE", () => {
  const shortcut = (key) => appShortcutIntent({
    key,
    ctrlKey: true,
    metaKey: false,
    altKey: false,
    shiftKey: false,
    target: null,
  });
  assert.equal(shortcut("`"), "toggleTerminal");
  assert.equal(shortcut("b"), "togglePrimarySidebar");
  assert.equal(shortcut("\\"), "toggleEditorSplit");
  assert.equal(appShortcutIntent({
    key: "m",
    ctrlKey: true,
    metaKey: false,
    altKey: false,
    shiftKey: true,
    target: null,
  }), "showProblems");
});

test("prefixele Command Center selectează scope-ul fără a ajunge în query", () => {
  assert.deepEqual(commandCenterQuery("> validare"), {
    query: "validare",
    scope: "commands",
    scopeLabel: "Comenzi",
  });
  assert.deepEqual(commandCenterQuery("# index.html"), {
    query: "index.html",
    scope: "files",
    scopeLabel: "Fișiere",
  });
  assert.deepEqual(commandCenterQuery("@ macro card"), {
    query: "macro card",
    scope: "symbols",
    scopeLabel: "Simboluri",
  });
});
