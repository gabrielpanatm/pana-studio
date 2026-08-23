import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { stableProjection } from "$lib/application/stable-projection";

test("stable projections retain identity while getters expose the latest authority", () => {
  let revision = 1;
  const projection = stableProjection({
    revision: () => revision,
    label: () => `revision-${revision}`,
  });
  const identity = projection;

  assert.deepEqual({ ...projection }, { revision: 1, label: "revision-1" });
  revision = 7;
  assert.equal(projection, identity);
  assert.deepEqual({ ...projection }, { revision: 7, label: "revision-7" });
});

test("workspace hot boundaries use stable projections and command identities", () => {
  const source = readFileSync(
    new URL("../src/lib/components/application/ApplicationWorkspace.svelte", import.meta.url),
    "utf8",
  );
  assert.match(source, /const projectAreaPane = stableProjection/);
  assert.match(source, /const centerAreaSession = stableProjection/);
  assert.match(source, /pane=\{projectAreaPane\}/);
  assert.match(source, /session=\{centerAreaSession\}/);
  assert.match(source, /commands=\{projectAreaCommands\}/);
  assert.match(source, /workspaceCommands=\{centerAreaWorkspaceCommands\}/);
  assert.doesNotMatch(source, /<WorkspaceProjectArea[\s\S]*?pane=\{\{/);
  assert.doesNotMatch(source, /<WorkspaceCenterArea[\s\S]*?session=\{\{/);
});
