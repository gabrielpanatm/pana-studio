import assert from "node:assert/strict";
import { test } from "node:test";
import { projectPreviewResourceUrl } from "$lib/project/assets";

test("asset Preview URL targets the exact Rust generation and preserves unicode paths", () => {
  const result = projectPreviewResourceUrl(
    "http://127.0.0.1:43210",
    "images/Captură de ecran #1?.png",
    "workspace-5-1786563476-111285469",
  );
  const url = new URL(result);

  assert.equal(decodeURIComponent(url.pathname), "/images/Captură de ecran #1?.png");
  assert.equal(
    url.searchParams.get("__pana_preview_revision"),
    "workspace-5-1786563476-111285469",
  );
  assert.equal(url.hash, "");
});

test("asset Preview URL falls back to the active generation when no revision is pending", () => {
  const result = projectPreviewResourceUrl(
    "http://127.0.0.1:43210",
    "/images/existent.png",
    null,
  );
  const url = new URL(result);

  assert.equal(url.pathname, "/images/existent.png");
  assert.equal(url.search, "");
});

test("asset Preview URL rejects missing inputs", () => {
  assert.equal(projectPreviewResourceUrl(null, "images/photo.png", "preview-1"), "");
  assert.equal(projectPreviewResourceUrl("http://127.0.0.1:43210", "", "preview-1"), "");
});
