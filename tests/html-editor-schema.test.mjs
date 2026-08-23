import assert from "node:assert/strict";
import test from "node:test";

import {
  htmlAttributeDefinition,
  htmlAttributePreviewMode,
  htmlAttributeValueError,
  htmlTagTransitionOptions,
  htmlEditorSchema,
} from "$lib/html/editor-schema.ts";
import {
  isLatestHtmlAttributeDraftSettlement,
  liveProjectableHtmlAttributeDraft,
} from "$lib/html/live-attribute-draft.ts";
import {
  assetEditLeaseMatches,
  captureAssetEditLease,
  cancelledAssetEditValue,
} from "$lib/html/asset-edit-session.ts";

test("schema separates presence booleans, ARIA booleans and meaningful empty strings", () => {
  assert.equal(htmlAttributeDefinition("hidden")?.semantic, "booleanPresence");
  assert.equal(htmlAttributeDefinition("aria-hidden")?.semantic, "ariaBoolean");
  assert.equal(htmlAttributeDefinition("alt")?.emptyPolicy, "preserve");
  assert.equal(htmlAttributeValueError("aria-hidden", ""), null);
  assert.match(htmlAttributeValueError("aria-hidden", "yes") ?? "", /true.*false/);
});

test("tag options retain only structurally compatible live destinations", () => {
  const sectionOptions = htmlTagTransitionOptions("section").map((option) => option.value);
  assert.ok(sectionOptions.includes("article"));
  assert.ok(!sectionOptions.includes("ul"));
  assert.ok(!sectionOptions.includes("img"));
  assert.ok(!sectionOptions.includes("iframe"));
  assert.deepEqual(htmlTagTransitionOptions("img"), []);
  assert.deepEqual(htmlTagTransitionOptions("a").map((option) => option.value), ["a"]);
});

test("live attribute drafts omit source-only and blocked attributes without losing empty values", () => {
  const projection = liveProjectableHtmlAttributeDraft(
    "a",
    {
      href: "",
      target: "_blank",
      download: "",
      "aria-label": "",
      "data-state": "",
      onclick: "alert(1)",
    },
    ["href", "target", "download", "aria-label", "data-state", "onclick"],
  );

  assert.deepEqual(projection.attributes, {
    href: "",
    "aria-label": "",
    "data-state": "",
  });
  assert.deepEqual(projection.baselineNames, ["href", "aria-label", "data-state"]);
  assert.equal(htmlAttributePreviewMode("target", "a"), "sourceOnly");
  assert.equal(htmlAttributePreviewMode("onclick", "a"), "blocked");
});

test("only the most recent epoch of the active attribute session may settle UI state", () => {
  assert.equal(isLatestHtmlAttributeDraftSettlement("attr_a", 4, "attr_a", 4), true);
  assert.equal(isLatestHtmlAttributeDraftSettlement("attr_a", 4, "attr_a", 3), false);
  assert.equal(isLatestHtmlAttributeDraftSettlement("attr_b", 1, "attr_a", 4), false);
});

test("a delayed media picker commit cannot cross selection context and cancel restores baseline", () => {
  const lease = captureAssetEditLease("project-a::image-a", "/old.webp");
  assert.equal(assetEditLeaseMatches(lease, "project-a::image-a"), true);
  assert.equal(assetEditLeaseMatches(lease, "project-a::image-b"), false);
  assert.equal(assetEditLeaseMatches(lease, "project-b::image-a"), false);
  assert.equal(cancelledAssetEditValue(lease), "/old.webp");
});

test("schema records implicit browser values without serializing fake attributes", () => {
  assert.equal(htmlAttributeDefinition("loading")?.implicitValue, "eager");
  assert.equal(htmlAttributeDefinition("decoding")?.implicitValue, "auto");
  assert.equal(htmlAttributeDefinition("fetchpriority")?.implicitValue, "auto");
  assert.equal(htmlAttributeDefinition("method")?.implicitValue, "get");
  assert.equal(htmlAttributeDefinition("preload")?.implicitValue, "metadata");
});

test("every palette tag and element-specific attribute references a declared tag", () => {
  for (const group of htmlEditorSchema.paletteGroups) {
    for (const tag of group.tags) assert.ok(htmlEditorSchema.tags[tag], `tag lipsă: ${tag}`);
  }
  for (const [name, definition] of Object.entries(htmlEditorSchema.attributes)) {
    for (const tag of definition.elements ?? []) {
      assert.ok(htmlEditorSchema.tags[tag], `${name} referă tagul nedeclarat ${tag}`);
    }
  }
});
