import assert from "node:assert/strict";
import test from "node:test";

import {
  probeBatchTimeoutMs,
  probeBatches,
  probeExpression,
} from "../scripts/performance-webkit-adapter.mjs";

test("WebKit probe batches preserve every iteration exactly once", () => {
  assert.deepEqual(probeBatches(23), [
    { start: 0, end: 10 },
    { start: 10, end: 20 },
    { start: 20, end: 23 },
  ]);
  assert.throws(() => probeBatches(0), /positive safe integer/);
  assert.throws(() => probeBatches(10, 0), /positive safe integer/);
  assert.equal(probeBatchTimeoutMs(7), 270_000);
  assert.equal(probeBatchTimeoutMs(10), 360_000);
  assert.throws(() => probeBatchTimeoutMs(0), /positive safe integer/);
});

test("WebKit probe expression carries phase and batch checkpoint boundaries", () => {
  const expression = probeExpression({
    samples: 100,
    warmups: 10,
    frameSamples: 600,
  }, "reload", 20, 30);

  assert.match(expression, /const phase = "reload";/);
  assert.match(expression, /const batchStart = 20;/);
  assert.match(expression, /const batchEnd = 30;/);
  assert.match(expression, /phase === "reload" \? batchStart : batchEnd/);
  assert.match(expression, /iteration >= 10/);
});

test("document switching separates tab activation from authoritative document settlement", () => {
  const expression = probeExpression({
    samples: 5,
    warmups: 1,
    frameSamples: 30,
  }, "document-code", 0, 6);

  assert.match(expression, /dataset\.documentActivationPhase === "ready"/);
  assert.match(expression, /dataset\.documentActivationPath === path/);
  assert.match(expression, /dataset\.activeDocumentPath === path/);
  assert.match(expression, /dataset\.sourceLoading === "false"/);
  assert.match(expression, /input_to_document_ready|readyMs/);
  assert.match(expression, /tabActivationMs/);
  assert.match(expression, /const measureTabSelection = async/);
  assert.match(expression, /new MutationObserver/);
  assert.match(expression, /attributeFilter: \["aria-selected"\]/);
  assert.match(expression, /reportedIntentMs: Number\(editor\.dataset\.documentActivationIntentMs/);
  assert.match(expression, /reportedResolveMs: Number\(editor\.dataset\.documentActivationResolveMs/);
  assert.match(expression, /reportedLoadMs: Number\(editor\.dataset\.documentActivationLoadMs/);
  assert.match(expression, /reportedSurfaceMs: Number\(editor\.dataset\.documentActivationSurfaceMs/);
  const documentProbe = expression.slice(
    expression.indexOf("const documentPhase"),
    expression.indexOf("const inspectorSamples"),
  );
  assert.doesNotMatch(documentProbe, /await twice\(\)/);
  assert.doesNotMatch(
    documentProbe,
    /await waitFor\(\(\) => tabForPath\(path\)\?\.getAttribute\("aria-selected"\)/,
  );
});

test("cold document setup has a separate timeout from measured settlement", () => {
  const expression = probeExpression({
    samples: 5,
    warmups: 1,
    frameSamples: 30,
    timeoutMs: 45_000,
  }, "document-code", 0, 6);

  assert.match(expression, /const coldSetupTimeoutMs = 120000;/);
  assert.match(expression, /some\(\(tab\) => tab\.title\?\.includes\(path\)\), coldSetupTimeoutMs\)/);
  assert.match(expression, /dataset\.sourceLoading === "false"[\s\S]*coldSetupTimeoutMs/);
  assert.match(expression, /const waitForDocumentReady = async \(path, minimumSerial\) => await waitFor\(\(\) =>/);
  assert.doesNotMatch(expression, /waitForDocumentReady[\s\S]{0,180}coldSetupTimeoutMs/);
});

test("document probe defines code↔code, canonical reuse and rapid latest-wins scenarios", () => {
  const expression = probeExpression({
    samples: 5,
    warmups: 1,
    frameSamples: 30,
  }, "document-rapid", 0, 6);

  assert.match(expression, /scenario: "code_to_code"/);
  assert.match(expression, /scenario: "canonical_template_reactivation"/);
  assert.match(expression, /const rapidDocumentSamples = \[\]/);
  assert.match(expression, /cacheOutcome: settled\.dataset\.documentActivationCacheOutcome/);
  assert.match(expression, /burstSize: burst\.length/);
  assert.match(expression, /"config\.toml"/);
  assert.match(expression, /"static\/js\/site\.js"/);
  assert.match(expression, /"templates\/index\.html"/);
});
