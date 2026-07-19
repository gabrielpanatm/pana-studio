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
  const shellCss = readFileSync(new URL("../src/routes/workspace-shell.css", import.meta.url), "utf8");
  assert.match(shellCss, /--brand\s*:/);

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
