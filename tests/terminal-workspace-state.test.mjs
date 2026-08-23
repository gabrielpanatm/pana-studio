import assert from "node:assert/strict";
import { test } from "node:test";
import { TerminalWorkspaceState } from "$lib/terminal/workspace.svelte";

function terminalHarness({ allowPane = true, projectPath = "/tmp/project" } = {}) {
  const paneRequests = [];
  const statuses = [];
  const workspace = new TerminalWorkspaceState({
    setPaneOpen: async (open) => {
      paneRequests.push(open);
      if (allowPane) workspace.synchronizePaneOpen(open);
      return allowPane;
    },
    currentProjectPath: () => projectPath,
    runZolaValidation: async () => true,
    openCurrentProjectInBrowser: async () => {},
    setGlobalStatus: (text, kind) => statuses.push({ text, kind }),
  });
  return { workspace, paneRequests, statuses };
}

test("Terminal Workspace deține taburile și sincronizează panoul autoritativ", async () => {
  const { workspace, paneRequests } = terminalHarness();

  assert.equal(workspace.terminalTabs.length, 1);
  assert.equal(workspace.activeTerminalTab?.id, "terminal-shell-1");
  await workspace.openTab();
  assert.equal(workspace.terminalTabs.length, 2);
  assert.equal(workspace.activeTerminalTab?.id, "terminal-shell-2");
  await workspace.selectTab("terminal-shell-1");
  assert.equal(workspace.activeTerminalTabId, "terminal-shell-1");
  workspace.closeTab("terminal-shell-2");
  assert.equal(workspace.terminalTabs.length, 1);
  await workspace.togglePane();

  assert.deepEqual(paneRequests, [true, true, false]);
  assert.equal(workspace.terminalPaneOpen, false);
  workspace.reset();
  assert.equal(workspace.activeTerminalTabId, "terminal-shell-1");
  assert.equal(workspace.terminalTabs.length, 1);
});

test("Terminal Workspace nu mută taburile dacă Workbench refuză deschiderea", async () => {
  const { workspace } = terminalHarness({ allowPane: false });

  await workspace.openTab();
  await workspace.selectTab("terminal-shell-unknown");

  assert.equal(workspace.terminalTabs.length, 1);
  assert.equal(workspace.activeTerminalTabId, "terminal-shell-1");
});

test("quick task fără proiect publică eroarea prin contractul îngust de status", async () => {
  const { workspace, statuses } = terminalHarness({ projectPath: "" });

  await workspace.runQuickTask(workspace.terminalQuickTasks[0]);

  assert.equal(statuses.length, 1);
  assert.equal(statuses[0].kind, "error");
});
