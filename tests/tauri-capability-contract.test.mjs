import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const capability = JSON.parse(readFileSync(
  new URL("../src-tauri/capabilities/default.json", import.meta.url),
  "utf8",
));

test("webview-ul principal primește numai capabilitățile Tauri folosite", () => {
  assert.deepEqual(capability.webviews, ["main"]);
  assert.deepEqual(new Set(capability.permissions), new Set([
    "core:app:allow-version",
    "core:event:allow-listen",
    "core:event:allow-unlisten",
    "core:path:allow-resolve-directory",
    "core:window:allow-close",
    "core:window:allow-show",
    "dialog:allow-open",
    "opener:allow-open-url",
    "opener:allow-default-urls",
    "pty:default",
    "default",
  ]));
  assert.equal(capability.permissions.some((permission) => (
    permission === "core:default"
    || permission === "dialog:default"
    || permission === "opener:default"
  )), false);
});
