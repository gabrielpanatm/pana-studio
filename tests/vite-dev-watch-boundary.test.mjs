import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  createViteDevWatchIgnored,
  viteDevIgnoredRoots,
} from "../scripts/vite-dev-watch-boundary.mjs";

const projectRoot = new URL("../", import.meta.url).pathname;

test("Vite exclude numai arborii generați care nu aparțin frontendului", () => {
  assert.deepEqual(viteDevIgnoredRoots, [
    "src-tauri",
    "benchmark-results",
    "tools/performance-benchmark/target",
  ]);

  const ignored = createViteDevWatchIgnored(projectRoot);
  for (const path of viteDevIgnoredRoots) {
    assert.equal(ignored(path), true, `${path} root must be ignored`);
    assert.equal(ignored(`${path}/nested/cache/file.bin`), true, `${path} descendants must be ignored`);
  }

  for (const path of [
    "src",
    "tests/fixtures/projects/index-zero/sursa",
    "src-tauri-copy",
    "benchmark-results-archive",
    "tools/performance-benchmark/targeted",
  ]) {
    assert.equal(ignored(path), false, `${path} must remain visible to Vite`);
  }
});

test("configurația Vite folosește frontiera canonică de watch", () => {
  const source = readFileSync(new URL("../vite.config.js", import.meta.url), "utf8");
  assert.match(source, /import \{ createViteDevWatchIgnored \}/);
  assert.match(source, /ignored:\s*createViteDevWatchIgnored\(projectRoot\)/);
  assert.doesNotMatch(source, /ignored:\s*\[\s*"\*\*\/src-tauri\/\*\*"\s*\]/);
});
