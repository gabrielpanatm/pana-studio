import assert from "node:assert/strict";
import { test } from "node:test";

import { availableNativeBlockScales } from "../src/lib/blocks/registry.ts";

test("catalogul expune numai scale-urile care au definiții", () => {
  assert.deepEqual(availableNativeBlockScales([]), []);
  assert.deepEqual(
    availableNativeBlockScales([
      { scale: "composition" },
      { scale: "element" },
      { scale: "composition" },
    ]),
    ["element", "composition"],
  );
  assert.deepEqual(
    availableNativeBlockScales([{ scale: "section" }]),
    ["section"],
  );
});
