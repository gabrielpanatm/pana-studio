import assert from "node:assert/strict";
import { test } from "node:test";

import { resolveUiBlockSourceInstanceForSelection } from "../src/lib/blocks/registry.ts";

function sourceInstance(id, providerId, rootSourceNodeId, markerKind = "canonical") {
  return {
    id,
    providerId,
    rootSourceNodeId,
    markerKind,
  };
}

function selection(overrides = {}) {
  return {
    providerId: "counter",
    markerKind: "canonical",
    rootSelector: "main > span.counter",
    rootTag: "span",
    sourceInstanceIds: [],
    rootSourceId: null,
    rootTemplateSourceId: null,
    rootSessionId: "session",
    ...overrides,
  };
}

test("rezolvă instanța selectată prin identitatea BlockGraph validată de Rust", () => {
  const counter = sourceInstance("block-counter", "counter", "source-counter");
  const graph = { sourceInstances: [counter] };

  assert.equal(
    resolveUiBlockSourceInstanceForSelection(
      graph,
      selection({ sourceInstanceIds: [counter.id] }),
    ),
    counter,
  );
});

test("pentru blocuri imbricate alege ultima instanță compatibilă", () => {
  const outer = sourceInstance("block-outer", "counter", "source-outer");
  const unrelated = sourceInstance("block-tabs", "tabs", "source-tabs");
  const inner = sourceInstance("block-inner", "counter", "source-inner");
  const graph = { sourceInstances: [outer, unrelated, inner] };

  assert.equal(
    resolveUiBlockSourceInstanceForSelection(
      graph,
      selection({ sourceInstanceIds: [outer.id, unrelated.id, inner.id] }),
    ),
    inner,
  );
});

test("nu leagă selecția de alt provider sau alt tip de marcaj", () => {
  const tabs = sourceInstance("block-tabs", "tabs", "source-tabs");
  const legacyCounter = sourceInstance(
    "block-counter-legacy",
    "counter",
    "source-counter",
    "legacy",
  );
  const graph = { sourceInstances: [tabs, legacyCounter] };

  assert.equal(
    resolveUiBlockSourceInstanceForSelection(
      graph,
      selection({ sourceInstanceIds: [tabs.id, legacyCounter.id] }),
    ),
    null,
  );
});

test("o selecție tranzitorie fără identități nu poate prăbuși panoul", () => {
  const counter = sourceInstance("block-counter", "counter", "source-counter");
  const transientSelection = selection();
  delete transientSelection.sourceInstanceIds;

  assert.doesNotThrow(() => {
    assert.equal(
      resolveUiBlockSourceInstanceForSelection(
        { sourceInstances: [counter] },
        transientSelection,
      ),
      null,
    );
  });
});
