import assert from "node:assert/strict";
import { test } from "node:test";
import {
  createProjectOpenRecoveryDecisionRequest,
  projectOpenRecoveryAbandonDecision,
  projectOpenRecoveryReasonLabel,
} from "$lib/project/open-recovery";

function assessment(overrides = {}) {
  return {
    schemaVersion: 1,
    status: "decision_required",
    projectRoot: "/project",
    assessmentToken: "a".repeat(64),
    conflictReason: "project_root_replaced",
    rootIdentityChanged: true,
    recoveryRevision: 51,
    dirtyDocumentCount: 1,
    stagedBinaryResourceCount: 0,
    deletedBinaryResourceCount: 0,
    pageJsDraftCount: 0,
    undoCount: 6,
    redoCount: 0,
    acceptedFileCount: 22,
    currentFileCount: 0,
    diagnostic: "Calea desemnează alt dosar fizic.",
    ...overrides,
  };
}

test("cererea UI este creată numai pentru recovery care cere decizie", () => {
  assert.throws(
    () => createProjectOpenRecoveryDecisionRequest(
      "/project",
      assessment({ status: "restorable" }),
      null,
    ),
    /nu cere o decizie explicită/,
  );
  assert.throws(
    () => createProjectOpenRecoveryDecisionRequest(
      "/project",
      assessment({ assessmentToken: null }),
      null,
    ),
    /nu cere o decizie explicită/,
  );
});

test("abandonarea poartă exact tokenul preflight și decizia de tranziție curentă", () => {
  const request = createProjectOpenRecoveryDecisionRequest(
    "/project",
    assessment(),
    "operator-decision-1",
  );
  assert.equal(request.targetRoot, "/project");
  assert.equal(request.operatorDecisionId, "operator-decision-1");
  assert.deepEqual(projectOpenRecoveryAbandonDecision(request), {
    action: "abandon",
    assessmentToken: "a".repeat(64),
  });
});

test("motivul înlocuirii rădăcinii este prezentat separat de driftul de conținut", () => {
  assert.equal(projectOpenRecoveryReasonLabel(assessment()), "dosar fizic înlocuit");
  assert.equal(
    projectOpenRecoveryReasonLabel(assessment({ conflictReason: "disk_baseline_changed" })),
    "conținut schimbat pe disk",
  );
});
