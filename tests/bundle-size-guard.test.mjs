import assert from "node:assert/strict";
import { test } from "node:test";
import {
  collectStaticGraph,
  initialGraphEntryNames,
  localeCatalogEntries,
  manifestKeysForNames,
} from "../scripts/check-bundle-size.mjs";

const manifest = {
  start: { name: "entry/start", file: "start.js", imports: ["shared"] },
  app: { name: "entry/app", file: "app.js", imports: ["shared", "runtime"] },
  layout: { name: "nodes/0", file: "layout.js", imports: ["runtime"] },
  error: { name: "nodes/1", file: "error.js", imports: ["shared"] },
  page: { name: "nodes/2", file: "page.js", imports: ["feature"] },
  shared: { file: "shared.js" },
  runtime: { file: "runtime.js", imports: ["shared"] },
  feature: { file: "feature.js", imports: ["runtime"] },
};

test("graful inițial include closure-ul static și deduplică dependențele", () => {
  const roots = manifestKeysForNames(manifest, initialGraphEntryNames);
  const graph = collectStaticGraph(manifest, roots);

  assert.deepEqual(
    new Set(graph),
    new Set(Object.keys(manifest).filter((key) => key !== "error")),
  );
  assert.equal(graph.includes("error"), false);
  assert.equal(graph.filter((key) => key === "shared").length, 1);
});

test("garda refuză un entry inițial absent", () => {
  assert.throws(
    () => manifestKeysForNames(manifest, ["nodes/3"]),
    /manifest is missing initial entry nodes\/3/,
  );
});

test("garda refuză referințele statice rupte", () => {
  assert.throws(
    () => collectStaticGraph({ root: { file: "root.js", imports: ["missing"] } }, ["root"]),
    /manifest references missing entry missing/,
  );
});

test("garda descoperă toate cataloagele dinamice fără registru duplicat", () => {
  const entries = localeCatalogEntries({
    "src/lib/i18n/generated/catalog.ro.ts": { file: "ro.js" },
    "src/lib/i18n/generated/catalog.en-US.ts": { file: "en.js" },
    "src/lib/i18n/generated/catalog.ts": { file: "runtime.js" },
  });

  assert.deepEqual(entries, [
    { key: "src/lib/i18n/generated/catalog.en-US.ts", locale: "en-US" },
    { key: "src/lib/i18n/generated/catalog.ro.ts", locale: "ro" },
  ]);
});
