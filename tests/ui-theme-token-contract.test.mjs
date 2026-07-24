import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { test } from "node:test";

function styleSourceUrls(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const entryUrl = new URL(`${entry.name}${entry.isDirectory() ? "/" : ""}`, directory);
    if (entry.isDirectory()) return styleSourceUrls(entryUrl);
    return /\.(?:css|svelte)$/.test(entry.name) ? [entryUrl] : [];
  });
}

test("interfața folosește tokenul canonic --brand pentru accente", () => {
  const designSystemCss = readFileSync(new URL("../src/routes/design-system.css", import.meta.url), "utf8");
  assert.match(designSystemCss, /--brand\s*:/);

  const sourceUrls = [
    ...styleSourceUrls(new URL("../src/", import.meta.url)),
    ...styleSourceUrls(new URL("../static/", import.meta.url)),
  ];
  const undefinedAccentUsers = sourceUrls
    .filter((sourceUrl) => readFileSync(sourceUrl, "utf8").includes("var(--accent)"))
    .map((sourceUrl) => sourceUrl.pathname);

  assert.deepEqual(
    undefinedAccentUsers,
    [],
    "Folosește var(--brand); shell-ul aplicației nu definește --accent.",
  );
});

test("design system-ul oferă o variantă compactă reutilizabilă pentru micro-acțiuni", () => {
  const designSystemCss = readFileSync(new URL("../src/routes/design-system.css", import.meta.url), "utf8");
  const layersSource = readFileSync(
    new URL("../src/lib/components/project/ProjectLayersTab.svelte", import.meta.url),
    "utf8",
  );

  assert.match(designSystemCss, /--control-height-compact:\s*24px/);
  assert.match(designSystemCss, /\.ui-button\.compact,\s*\n\.ui-icon-button\.compact/);
  assert.match(designSystemCss, /\.ui-icon-button\.compact\s*\{/);
  assert.match(layersSource, /class="ui-icon-button compact quiet toggle-btn"/);
  assert.match(layersSource, /class="ui-icon-button compact quiet tree-delete-btn"/);
});
