import assert from "node:assert/strict";
import { test } from "node:test";
import {
  pageFrontmatterMutationValue,
  parsePageFrontmatter,
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

test("page metadata reads SEO values from the TOML extra table emitted by Rust", () => {
  const source = `+++
title = "Articol"
taxonomies.tags = ["design", "zola"]

[extra]
seo_title = "Titlu SEO"
custom_field = "păstrat"
+++

Corpul paginii`;
  const parsed = parsePageFrontmatter(source);
  assert.equal(parsed.values.seoTitle, "Titlu SEO");
  assert.equal("custom_field" in parsed.values, false);
});

test("page metadata converts weight to a typed Rust scalar", () => {
  assert.deepEqual(pageFrontmatterMutationValue("weight", "2"), {
    kind: "integer",
    value: 2,
  });
  assert.deepEqual(pageFrontmatterMutationValue("weight", ""), { kind: "empty" });
  assert.deepEqual(pageFrontmatterMutationValue("draft", true), {
    kind: "boolean",
    value: true,
  });
  assert.throws(
    () => pageFrontmatterMutationValue("weight", "1.5"),
    /număr întreg/,
  );
});

test("section metadata reads and validates mandatory pagination", () => {
  const source = `+++
title = "Servicii"
paginate_by = 6
+++`;
  assert.equal(parsePageFrontmatter(source).values.paginateBy, "6");
  assert.deepEqual(pageFrontmatterMutationValue("paginateBy", "12"), {
    kind: "integer",
    value: 12,
  });
  assert.throws(() => pageFrontmatterMutationValue("paginateBy", ""), /cel puțin un articol/);
  assert.throws(() => pageFrontmatterMutationValue("paginateBy", "0"), /cel puțin un articol/);
});

test("hidden keeps the inherited section state distinct from explicit visibility", () => {
  assert.equal(parsePageFrontmatter("+++\ntitle = 'Moștenit'\n+++").values.hidden, "inherit");
  assert.equal(parsePageFrontmatter("+++\ntitle = 'Ascuns'\nhidden = true\n+++").values.hidden, "hidden");
  assert.equal(parsePageFrontmatter("+++\ntitle = 'Vizibil'\nhidden = false\n+++").values.hidden, "visible");
  assert.deepEqual(pageFrontmatterMutationValue("hidden", "inherit"), { kind: "empty" });
  assert.deepEqual(pageFrontmatterMutationValue("hidden", "hidden"), { kind: "boolean", value: true });
  assert.deepEqual(pageFrontmatterMutationValue("hidden", "visible"), { kind: "boolean", value: false });
});

test("include_in_feeds defaults to true without writing the default key", () => {
  assert.equal(parsePageFrontmatter("+++\ntitle = 'Implicit'\n+++").values.includeInFeeds, true);
  assert.equal(parsePageFrontmatter("+++\ntitle = 'Exclus'\ninclude_in_feeds = false\n+++").values.includeInFeeds, false);
  assert.deepEqual(pageFrontmatterMutationValue("includeInFeeds", false), { kind: "boolean", value: false });
  assert.deepEqual(pageFrontmatterMutationValue("includeInFeeds", true), { kind: "empty" });
});
