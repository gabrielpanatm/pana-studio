import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

function source(path) {
  return readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
}

test("startup surfaces WAL recovery actions before a project is mounted", () => {
  const startup = source("src/lib/components/startup/StartupView.svelte");
  const recovery = source("src/lib/components/kernel/WriteAuthorityRecoveryControl.svelte");
  const controller = source("src/lib/state/project-controller.ts");

  assert.match(startup, /WRITE_AUTHORITY_RECOVERY_BLOCKED/);
  assert.match(startup, /<WriteAuthorityRecoveryControl/);
  assert.match(startup, /startupRecoveryScan\?\.blocked !== false/);
  assert.match(startup, /app\.retryStartupProjectOpen\(\)/);
  assert.match(startup, /Proiectul nu este pierdut/);
  assert.match(recovery, /item\.diagnostic \|\| t\("wal-item-diagnostic"\)/);
  assert.match(recovery, /onScanUpdate\?\.\(scan\)/);
  assert.match(recovery, /discard_staged_write: t\("wal-resolution-discard-staged"\)/);
  assert.match(recovery, /t\("wal-confirm-discard-staged"\)/);
  assert.match(controller, /export async function retryStartupProjectOpen/);
  assert.match(controller, /openProjectRoot\(host, candidate\.root, \{ startupCandidate: candidate \}\)/);
});
