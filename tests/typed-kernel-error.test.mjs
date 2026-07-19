import assert from "node:assert/strict";
import { test } from "node:test";

import { errorMessage, isRecoveryRequiredError } from "$lib/util";

test("structured editor recovery stays visible and non-retryable in UI copy", () => {
  const message = errorMessage({
    kind: "recovery_required",
    detail: {
      commandId: "command-save-42",
      phase: "domain_effect",
      diagnostic: "Commitul este vizibil, dar fsync-ul directorului a eșuat.",
      retryForbidden: true,
    },
  });

  assert.match(message, /^RECOVERY_REQUIRED \[command-save-42, domain_effect\]:/);
  assert.match(message, /Commitul este vizibil/);
  assert.match(message, /Nu repeta operația automat\.$/);
  assert.doesNotMatch(message, /\[object Object\]/);
});

test("structured editor rejection exposes its diagnostic", () => {
  assert.equal(
    errorMessage({
      kind: "rejected",
      detail: { diagnostic: "Disk baseline stale." },
    }),
    "Disk baseline stale.",
  );
});

test("external-config owner can disable retry only for typed recovery", () => {
  assert.equal(
    isRecoveryRequiredError({
      kind: "recovery_required",
      detail: {
        diagnostic: "Backup-ul este vizibil, iar targetul cere reconciliere.",
        retryForbidden: true,
      },
    }),
    true,
  );
  assert.equal(
    isRecoveryRequiredError({
      kind: "rejected",
      detail: { diagnostic: "Config invalid." },
    }),
    false,
  );
});

test("ordinary Error behavior remains unchanged", () => {
  assert.equal(errorMessage(new Error("ordinary failure")), "ordinary failure");
});
