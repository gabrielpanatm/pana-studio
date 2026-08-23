import assert from "node:assert/strict";
import { test } from "node:test";
import { WorkspaceLayoutState } from "$lib/ui/workspace-layout.svelte";

test("Workspace Layout deține dimensiunile și starea sidebarelor", () => {
  const layout = new WorkspaceLayoutState();

  assert.deepEqual(
    [layout.leftPaneWidth, layout.rightPaneWidth, layout.terminalPaneHeight],
    [260, 320, 240],
  );
  layout.toggleLeftPane();
  layout.toggleRightPane();
  assert.equal(layout.leftPaneCollapsed, true);
  assert.equal(layout.rightPaneCollapsed, true);

  layout.expandSidebars();
  assert.equal(layout.leftPaneCollapsed, false);
  assert.equal(layout.rightPaneCollapsed, false);
});
