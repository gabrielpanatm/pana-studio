import assert from "node:assert/strict";
import { test } from "node:test";
import {
  applyInspectorLiveProperties,
  bindInspectorLiveCssTransaction,
  captureInspectorLiveCssIdentity,
  clearInspectorLiveProperties,
  INSPECTOR_LIVE_STYLE_ID,
  restoreLiveCssLayersToPreview,
} from "$lib/state/preview-live-controller";

function host() {
  const messages = [];
  return {
    scssVariables: [],
    previewDevice: "desktop",
    liveCssById: { "pana-animation-preview": ".pulse { opacity: .5; }" },
    inspectorLiveCssEpoch: 0,
    inspectorLiveCssIdentity: null,
    sessionProjectRoot: "/tmp/project",
    kernelProjectSessionId: "runtime-1",
    getPreviewDocument: () => undefined,
    postPreviewMessage: (message) => messages.push(message),
    messages,
  };
}

test("proiecția canonică elimină numai generația CSS live pe care a confirmat-o", () => {
  const target = host();
  const firstEpoch = applyInspectorLiveProperties(
    target,
    ".title",
    { "text-align": "left" },
  );
  const firstIdentity = captureInspectorLiveCssIdentity(target, firstEpoch);
  const secondEpoch = applyInspectorLiveProperties(
    target,
    ".title",
    { "text-align": "right" },
  );
  const secondDraft = captureInspectorLiveCssIdentity(target, secondEpoch);

  assert.notEqual(firstEpoch, secondEpoch);
  assert.ok(firstIdentity);
  assert.ok(secondDraft);
  assert.match(
    target.liveCssById[INSPECTOR_LIVE_STYLE_ID],
    /text-align: right !important/,
  );
  assert.equal(clearInspectorLiveProperties(target, firstIdentity), false);
  assert.match(
    target.liveCssById[INSPECTOR_LIVE_STYLE_ID],
    /text-align: right !important/,
  );

  const bound = bindInspectorLiveCssTransaction(target, secondDraft, {
    workspaceRevision: 7,
    workspaceTransactionId: "workspace-tx-7",
    canvasTransactionId: "canvas-tx-7",
    previewRevision: "preview-7",
  });
  assert.deepEqual(bound, {
    projectRoot: "/tmp/project",
    runtimeSessionId: "runtime-1",
    epoch: secondEpoch,
    workspaceRevision: 7,
    workspaceTransactionId: "workspace-tx-7",
    canvasTransactionId: "canvas-tx-7",
    previewRevision: "preview-7",
  });
  assert.equal(clearInspectorLiveProperties(target, secondDraft), false);
  assert.equal(clearInspectorLiveProperties(target, bound), true);
  assert.equal(INSPECTOR_LIVE_STYLE_ID in target.liveCssById, false);
  assert.equal(target.liveCssById["pana-animation-preview"], ".pulse { opacity: .5; }");
  assert.deepEqual(target.messages.at(-1), {
    type: "set-live-style-css",
    id: INSPECTOR_LIVE_STYLE_ID,
    css: "",
    refreshSelection: false,
  });
});

test("un override invalidat nu mai este reaplicat după reload-ul Preview", () => {
  const target = host();
  const epoch = applyInspectorLiveProperties(target, ".title", { "text-align": "left" });
  const identity = captureInspectorLiveCssIdentity(target, epoch);
  assert.ok(identity);
  clearInspectorLiveProperties(target, identity);
  target.messages.length = 0;

  restoreLiveCssLayersToPreview(target);

  assert.deepEqual(target.messages, [{
    type: "set-live-style-css",
    id: "pana-animation-preview",
    css: ".pulse { opacity: .5; }",
    refreshSelection: false,
  }]);
});
