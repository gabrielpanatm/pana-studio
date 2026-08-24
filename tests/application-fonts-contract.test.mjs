import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("fonturile interfeței sunt incluse local ca familii variabile", () => {
  const packageJson = JSON.parse(source("../package.json"));
  const fontImports = source("../src/routes/app-fonts.css");

  for (const family of ["inter", "urbanist", "jetbrains-mono"]) {
    assert.equal(
      typeof packageJson.dependencies[`@fontsource-variable/${family}`],
      "string",
      family,
    );
    assert.match(
      fontImports,
      new RegExp(`@fontsource-variable/${family}/wght\\.css`),
      family,
    );
  }
  assert.doesNotMatch(fontImports, /local\s*\(/);
});

test("rolurile tipografice ale aplicației au o singură sursă de adevăr", () => {
  const designSystem = source("../src/routes/design-system.css");
  const workspaceShell = source("../src/routes/workspace-shell.css");
  const codeMirror = source("../src/lib/editor/codemirror.ts");
  const terminal = source("../src/lib/terminal/controller.ts");
  const elementDrag = source("../src/lib/state/element-palette-drag-controller.ts");
  const teraDrag = source("../src/lib/state/tera-palette-drag-controller.ts");

  assert.match(workspaceShell, /@import "\.\/app-fonts\.css";/);
  assert.match(designSystem, /--font-ui:\s*"Inter Variable"/);
  assert.match(designSystem, /--font-heading:\s*"Urbanist Variable"/);
  assert.match(designSystem, /--font-mono:\s*"JetBrains Mono Variable"/);
  assert.match(designSystem, /font:\s*400 var\(--font-body\)\/1\.4 var\(--font-ui\)/);
  assert.match(designSystem, /:where\(h1, h2, h3\)[\s\S]*font-family:\s*var\(--font-heading\)/);
  assert.match(codeMirror, /fontFamily:\s*"var\(--font-mono\)"/);
  assert.match(terminal, /fontFamily:\s*"var\(--font-mono\)"/);
  for (const dragController of [elementDrag, teraDrag]) {
    assert.match(dragController, /font\s*=\s*"700 12px\/1\.2 var\(--font-ui\)"/);
    assert.match(dragController, /fontFamily\s*=\s*"var\(--font-mono\)"/);
  }
});

test("notificarea juridică inventariază fonturile aplicației", () => {
  const notices = source("../THIRD_PARTY_NOTICES.md");

  assert.match(notices, /## Fonturile interfeței/);
  assert.match(notices, /Inter, Urbanist și JetBrains Mono/);
  assert.match(notices, /SIL Open Font License 1\.1/);
});
