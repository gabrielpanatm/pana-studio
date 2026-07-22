import assert from "node:assert/strict";
import { test } from "node:test";
import {
  aiRecoveryAuthorityDisposition,
  shouldAutomaticallyReloadAiReconciliation,
} from "$lib/state/ai-coordination-controller";
import { createExternalDiskState } from "$lib/state/external-disk-controller";

function detail(overrides = {}) {
  return {
    leaseId: "lease-1",
    clientSessionId: "client-1",
    projectSessionId: "project-1",
    basisProjectRevision: 7,
    releasedAtMs: 100,
    expectedChangedFiles: [
      "content/servicii.md",
      "date/meniu.toml",
    ],
    observedChangedFiles: [
      "content/servicii.md",
      "date/meniu.toml",
    ],
    declarationReviewedByUser: false,
    recoveryReloadAuthorized: false,
    summary: null,
    reason: "AI released the lease.",
    ...overrides,
  };
}

function external(overrides = {}) {
  return {
    ...createExternalDiskState(),
    changed: true,
    changedFiles: [
      "date/meniu.toml",
      "content/servicii.md",
    ],
    lastCheckedAt: 110,
    ...overrides,
  };
}

test("AI topology changes auto-reload only when declared, released and observed sets match", () => {
  assert.equal(
    shouldAutomaticallyReloadAiReconciliation(external(), detail()),
    true,
  );
});

test("AI topology auto-reload refuses undeclared or newly changed files", () => {
  assert.equal(
    shouldAutomaticallyReloadAiReconciliation(
      external({ changedFiles: ["content/servicii.md"] }),
      detail(),
    ),
    false,
  );
  assert.equal(
    shouldAutomaticallyReloadAiReconciliation(
      external(),
      detail({ observedChangedFiles: ["content/servicii.md"] }),
    ),
    false,
  );
});

test("AI topology auto-reload waits while disk evidence is unsafe or incomplete", () => {
  for (const unsafe of [
    { checking: true },
    { reconciling: true },
    { blockedByDirtySession: true },
    { workspaceProjectionRecoveryRequired: true },
    { truncated: true },
  ]) {
    assert.equal(
      shouldAutomaticallyReloadAiReconciliation(external(unsafe), detail()),
      false,
    );
  }
});

test("recovery button has a deterministic path for every authority state", () => {
  assert.equal(
    aiRecoveryAuthorityDisposition({ state: "user_active" }),
    "reload",
  );
  assert.equal(
    aiRecoveryAuthorityDisposition({ state: "ai_requested", detail: {} }),
    "reject_active_lease",
  );
  assert.equal(
    aiRecoveryAuthorityDisposition({ state: "ai_active", detail: {} }),
    "reject_active_lease",
  );
  assert.equal(
    aiRecoveryAuthorityDisposition({ state: "ai_orphaned", detail: {} }),
    "authorize_recovery",
  );
  assert.equal(
    aiRecoveryAuthorityDisposition({ state: "conflict", detail: {} }),
    "accept_conflict",
  );
  assert.equal(
    aiRecoveryAuthorityDisposition({
      state: "reconciling",
      detail: { recoveryReloadAuthorized: false },
    }),
    "authorize_recovery",
  );
  assert.equal(
    aiRecoveryAuthorityDisposition({
      state: "reconciling",
      detail: { recoveryReloadAuthorized: true },
    }),
    "reload",
  );
});
