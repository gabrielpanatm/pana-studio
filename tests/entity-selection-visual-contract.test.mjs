import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

const designSystem = source("../src/routes/design-system.css");

const qualifiedSurfaces = [
  "../src/lib/components/project/EditorNavigationTree.svelte",
  "../src/lib/components/project/ProjectFilesTab.svelte",
  "../src/lib/components/creation/BlocksWorkspace.svelte",
  "../src/lib/components/creation/ComponentsWorkspace.svelte",
  "../src/lib/components/creation/AssetsWorkspace.svelte",
  "../src/lib/components/creation/DesignSystemWorkspace.svelte",
  "../src/lib/components/creation/DesignTokenCatalog.svelte",
  "../src/lib/components/creation/ThemeStylesWorkspace.svelte",
  "../src/lib/components/themes/ThemesWorkspace.svelte",
  "../src/lib/components/templates/TemplatesWorkspace.svelte",
  "../src/lib/components/content/ContentWorkspace.svelte",
  "../src/lib/components/data/DataWorkspace.svelte",
  "../src/lib/components/taxonomies/TaxonomiesWorkspace.svelte",
  "../src/lib/components/inspector/SelectionSummaryCard.svelte",
  "../src/lib/components/inspector/js/MotionStudioPanel.svelte",
  "../src/lib/components/workspace/MotionTimelinePanel.svelte",
  "../src/lib/components/workbench/CommandCenter.svelte",
];

test("semantic entities share one theme-aware outline contract", () => {
  assert.match(
    designSystem,
    /--entity-selection-outline:\s*var\(--brand\)/,
  );
  assert.match(
    designSystem,
    /\.ui-entity-selectable:hover:not\(:disabled\):not\(\[aria-disabled="true"\]\):not\(\[data-ui-selected="true"\]\),[\s\S]*?outline-style:\s*dashed;/,
  );
  assert.match(
    designSystem,
    /\.ui-entity-selectable\[data-ui-selected="true"\]\s*\{[\s\S]*?outline-style:\s*solid;/,
  );
  assert.match(
    designSystem,
    /\.ui-entity-selectable:focus-visible\s*\{[\s\S]*?outline:\s*2px solid var\(--focus-ring\);[\s\S]*?outline-offset:\s*1px;/,
  );

  const selectedRule = designSystem.match(
    /\.ui-entity-selectable\[data-ui-selected="true"\]\s*\{([^}]*)\}/,
  )?.[1] ?? "";
  assert.doesNotMatch(
    selectedRule,
    /\b(?:background|box-shadow|border(?:-left)?|transform)\s*:/,
  );
  assert.match(
    designSystem,
    /button\.ui-entity-trigger:hover:not\(:disabled\)\s*\{[\s\S]*?background:\s*var\(--ui-entity-trigger-background,\s*transparent\);[\s\S]*?box-shadow:\s*var\(--ui-entity-trigger-shadow,\s*none\);/,
  );
});

test("all qualified entity surfaces opt in explicitly and publish persistent state", () => {
  for (const relativePath of qualifiedSurfaces) {
    const component = source(relativePath);
    assert.match(
      component,
      /ui-entity-selectable/,
      `${relativePath} must opt into the semantic entity contract`,
    );
    assert.match(
      component,
      /data-ui-selected=/,
      `${relativePath} must publish its persistent selection state`,
    );
    assert.match(
      component,
      /aria-(?:selected|pressed)=/,
      `${relativePath} must expose selection to assistive technology`,
    );
  }
});

test("legacy resource selection decoration is absent from migrated surfaces", () => {
  const migrated = qualifiedSurfaces.map(source).join("\n");

  for (const legacySelector of [
    /\.resource-card\.selected/,
    /\.asset-card\.selected/,
    /\.theme-row\.selected/,
    /\.template-card\.selected/,
    /\.style-target-row\.selected/,
    /\.token-card\.selected/,
    /\.taxonomy-card\.selected/,
    /\.selected-term/,
    /\.node-tree\s*>\s*button\.selected/,
    /\.class-row\.selected/,
    /\.style-row\.selected/,
    /\.font-row\.selected/,
    /\.action-clip\.selected/,
    /\.results button\.selected/,
  ]) {
    assert.doesNotMatch(migrated, legacySelector);
  }

  assert.doesNotMatch(
    designSystem,
    /\.resource-card\.selected|\.resource-card:hover/,
  );
});

test("action controls, tabs, fields and resize handles stay outside the entity contract", () => {
  for (const relativePath of [
    "../src/lib/components/workbench/DocumentBar.svelte",
    "../src/lib/components/workbench/ResponsiveCanvasToolbar.svelte",
    "../src/lib/components/topbar/ToolbarButton.svelte",
    "../src/lib/components/ui/SelectControl.svelte",
    "../src/lib/components/workspace/WorkspaceResizeHandle.svelte",
    "../src/lib/components/workbench/WorkbenchSplitHandle.svelte",
  ]) {
    assert.doesNotMatch(
      source(relativePath),
      /ui-entity-selectable/,
      `${relativePath} is an excluded control surface`,
    );
  }
});

test("contextual actions remain available for hovered and selected tree entities", () => {
  const layers = source(
    "../src/lib/components/project/EditorNavigationTree.svelte",
  );
  const files = source(
    "../src/lib/components/project/ProjectFilesTab.svelte",
  );

  assert.match(
    layers,
    /\.navigation-row:hover \.delete-action,[\s\S]*?\.navigation-row\.selected \.delete-action/,
  );
  assert.match(
    files,
    /hoveredPath === node\.path \|\| node\.entry\?\.id === snapshot\?\.selectedEntry\?\.entryId/,
  );
});

test("wrapped entity triggers cannot reintroduce the generic button hover fill", () => {
  assert.match(
    source("../src/lib/components/creation/DesignTokenCatalog.svelte"),
    /class="token-select ui-entity-trigger"/,
  );
  assert.match(
    source("../src/lib/components/taxonomies/TaxonomiesWorkspace.svelte"),
    /class="taxonomy-row ui-entity-trigger"/,
  );
});
