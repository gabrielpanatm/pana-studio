import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
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

test("panoul History nu mai este expus, dar Undo și Redo rămân disponibile", () => {
  const historyPanel = new URL(
    "../src/lib/components/HistoryPanel.svelte",
    import.meta.url,
  );
  const toolbar = readFileSync(
    new URL(
      "../src/lib/components/topbar/HistoryActionButtons.svelte",
      import.meta.url,
    ),
    "utf8",
  );
  const appChrome = readFileSync(
    new URL("../src/lib/components/workspace/AppChrome.svelte", import.meta.url),
    "utf8",
  );
  const commandTypes = readFileSync(
    new URL("../src/lib/types.ts", import.meta.url),
    "utf8",
  );
  const rustModel = readFileSync(
    new URL(
      "../src-tauri/src/kernel/command_center/model.rs",
      import.meta.url,
    ),
    "utf8",
  );
  const rustSearch = readFileSync(
    new URL(
      "../src-tauri/src/kernel/command_center/search.rs",
      import.meta.url,
    ),
    "utf8",
  );

  assert.equal(existsSync(historyPanel), false);
  assert.doesNotMatch(toolbar, /IconClock|historyPanelOpen|toggleHistoryPanel|historySnapshots/);
  assert.match(toolbar, /IconArrowBackUp/);
  assert.match(toolbar, /IconArrowForwardUp/);
  assert.doesNotMatch(appChrome, /HistoryPanel|historyPanelOpen|toggleHistoryPanel/);
  assert.doesNotMatch(commandTypes, /"open_history"/);
  assert.doesNotMatch(rustModel, /OpenHistory/);
  assert.doesNotMatch(rustSearch, /command\.open_history|OpenHistory/);
});
