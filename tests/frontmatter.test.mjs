import assert from "node:assert/strict";
import { test } from "node:test";
import {
  parsePageFrontmatter,
  updatePageFrontmatter,
} from "$lib/markdown/frontmatter";

test("page frontmatter reads Zola taxonomy arrays as editable comma-separated values", () => {
  const source = `+++
title = "Articol"
taxonomies.tags = ["design", "zola"]
taxonomies.categories = ["Noutăți"]
+++

Conținut`;
  const parsed = parsePageFrontmatter(source);
  assert.equal(parsed.kind, "toml");
  assert.equal(parsed.values.tags, "design, zola");
  assert.equal(parsed.values.categories, "Noutăți");
});

test("page frontmatter writes taxonomy arrays without converting Markdown into a parallel model", () => {
  const source = `+++
title = "Articol"
+++

Corpul paginii`;
  const parsed = parsePageFrontmatter(source);
  const updated = updatePageFrontmatter(source, {
    ...parsed.values,
    tags: "design, zola",
    categories: "Ghiduri",
  });
  assert.match(updated, /^\+\+\+[\s\S]*taxonomies\.tags = \["design", "zola"\]/);
  assert.match(updated, /taxonomies\.categories = \["Ghiduri"\]/);
  assert.match(updated, /Corpul paginii$/);
});
