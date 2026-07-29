import assert from "node:assert/strict";
import { test } from "node:test";
import {
  parsePageFrontmatter,
  updatePageFrontmatter,
} from "$lib/markdown/frontmatter";

test("page frontmatter leaves Zola taxonomies outside the frontend metadata model", () => {
  const source = `+++
title = "Articol"
taxonomies.tags = ["design", "zola"]
taxonomies.categories = ["Noutăți"]
+++

Conținut`;
  const parsed = parsePageFrontmatter(source);
  assert.equal(parsed.kind, "toml");
  assert.equal("tags" in parsed.values, false);
  assert.equal("categories" in parsed.values, false);
});

test("page metadata edits preserve taxonomy arrays for the Rust semantic mutation lane", () => {
  const source = `+++
title = "Articol"
taxonomies.tags = ["design", "zola"]
+++

Corpul paginii`;
  const parsed = parsePageFrontmatter(source);
  const updated = updatePageFrontmatter(source, {
    ...parsed.values,
    title: "Articol actualizat",
  });
  assert.match(updated, /title = "Articol actualizat"/);
  assert.match(updated, /^\+\+\+[\s\S]*taxonomies\.tags = \["design", "zola"\]/);
  assert.match(updated, /Corpul paginii$/);
});
