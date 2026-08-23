import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import {
  auditProviderStatusCounts,
  auditReceiptIsCurrent,
} from "../src/lib/audit/model.ts";
import { codeSelectionRangeForSourceRange } from "../src/lib/editor/source-ranges.ts";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

function finding(overrides = {}) {
  return {
    id: "audit:one",
    fingerprint: "sha256:one",
    providerId: "content_semantics",
    ruleCode: "content_missing_title",
    category: "seo",
    outcome: "violation",
    impact: "moderate",
    policy: "advisory",
    titleDiagnostic: { schemaVersion: 1, code: "title", arguments: {} },
    messageDiagnostic: { schemaVersion: 1, code: "message", arguments: {} },
    primaryLocation: {
      file: "content/page.md",
      range: { start: 0, end: 3, line: 1, column: 1, endLine: 1, endColumn: 4 },
      origin: "project",
      sourceNodeId: null,
    },
    relatedLocations: [],
    evidence: [],
    fixes: [],
    suppression: null,
    ...overrides,
  };
}

function receipt(overrides = {}) {
  return {
    schemaVersion: 4,
    rulesetVersion: 1,
    projectRoot: "/project",
    runtimeSessionId: "session",
    workspaceRevision: 7,
    projectModelRevision: "pm_test",
    mode: "quick",
    scope: { kind: "project" },
    completeness: "complete",
    summary: {
      total: 1,
      violations: 1,
      needsReview: 0,
      engineErrors: 0,
      passed: 0,
      notApplicable: 0,
      skipped: 0,
      suppressed: 0,
      blocking: 0,
      affectedFiles: 1,
    },
    providers: [],
    findings: [finding()],
    ...overrides,
  };
}

test("receipt-ul Audit este curent numai pentru identitatea și revizia exactă", () => {
  const value = receipt();
  assert.equal(auditReceiptIsCurrent(value, {
    projectRoot: "/project",
    runtimeSessionId: "session",
    workspaceRevision: 7,
  }), true);
  assert.equal(auditReceiptIsCurrent(value, {
    projectRoot: "/project",
    runtimeSessionId: "session",
    workspaceRevision: 8,
  }), false);
  assert.equal(auditReceiptIsCurrent(value, {
    projectRoot: "/project",
    runtimeSessionId: "other",
    workspaceRevision: 7,
  }), false);
});

test("stările complete, partial, failed și skipped rămân distincte în UI", () => {
  const provider = (status) => ({
    id: `provider_${status}`,
    kind: "source",
    status,
    publishCoverageRequirement: "required",
    coverage: { eligible: 1, analyzed: status === "complete" ? 1 : 0, limitations: [] },
    findingCount: 0,
    errorDiagnostic: null,
  });
  assert.deepEqual(
    auditProviderStatusCounts([
      provider("complete"),
      provider("partial"),
      provider("failed"),
      provider("skipped"),
      provider("partial"),
    ]),
    { complete: 1, partial: 2, failed: 1, skipped: 1 },
  );
});

test("navigarea exactă convertește offset-urile UTF-8 Rust în offset-uri UTF-16 editor", () => {
  assert.deepEqual(
    codeSelectionRangeForSourceRange("🙂<img>", {
      start: 4,
      end: 9,
      line: 1,
      column: 2,
      endLine: 1,
      endColumn: 7,
    }),
    { from: 2, to: 7 },
  );
});

test("UI-ul consumă receipt-ul Rust, propagă Result și navighează la range exact", () => {
  const audit = source("../src/lib/components/audit/AuditWorkspace.svelte");
  const state = source("../src/lib/audit/workspace-state.svelte.ts");
  const rust = source("../src-tauri/src/kernel/audit/model.rs");

  assert.match(rust, /pub struct AuditRunReceipt/);
  assert.match(rust, /pub struct AuditProviderReceipt/);
  assert.match(rust, /Violation[\s\S]*NeedsReview[\s\S]*EngineError/);
  assert.match(state, /Promise<AuditRefreshResult>/);
  assert.match(state, /current\(receipt: AuditRunReceipt \| null = this\.snapshot\)/);
  assert.match(audit, /if \(!result\.ok\) throw new Error/);
  assert.doesNotMatch(audit, /if \(!valid\) throw new Error/);
  assert.match(audit, /buildError[\s\S]*refreshProjectAudit\(true, "full"\)/);
  assert.match(audit, /revealSourceRange\(location\.file, location\.range\)/);
  assert.doesNotMatch(audit, /projectAuditSnapshot\?\.diagnostics/);
});

test("fixul safe trimite numai identități și este reconstruit atomic în Rust", () => {
  const state = source("../src/lib/audit/workspace-state.svelte.ts");
  const io = source("../src/lib/audit/io.ts");
  const types = source("../src/lib/audit/contracts.ts");
  const command = source("../src-tauri/src/commands/audit.rs");

  const inputContract = types.slice(
    types.indexOf("export type AuditFixApplyInput"),
    types.indexOf("export type AuditFixApplyReceipt"),
  );
  assert.match(inputContract, /expectedProjectRoot/);
  assert.match(inputContract, /expectedSessionId/);
  assert.match(inputContract, /expectedWorkspaceRevision/);
  assert.match(inputContract, /expectedProjectModelRevision/);
  assert.match(inputContract, /findingFingerprint/);
  assert.match(inputContract, /fixId/);
  assert.doesNotMatch(inputContract, /replacement|edits|contents/);
  assert.match(io, /invoke<AuditFixApplyReceipt>\("apply_audit_fix", \{ input \}\)/);
  assert.match(state, /this\.commands\.runStructural[\s\S]*this\.gateway\.applyFix/);
  assert.match(state, /this\.commands\.settleMutation/);
  assert.match(command, /require_authoritative_audit_request/);
  assert.match(command, /materialize_audit_fix\(&candidate, fix\)/);
  assert.match(command, /publish_prepared_project_workspace_candidate/);
  assert.match(command, /ProjectWorkspacePreviewProjection::Required/);
});
