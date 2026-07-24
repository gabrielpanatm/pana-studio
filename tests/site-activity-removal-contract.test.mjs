import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("dashboardul și utilitarele dedicate activității Site sunt eliminate", () => {
  assert.equal(
    existsSync(new URL("../src/lib/components/site/SiteOverviewWorkspace.svelte", import.meta.url)),
    false,
  );
  assert.equal(
    existsSync(new URL("../src/lib/site/overview.ts", import.meta.url)),
    false,
  );

  const center = source("../src/lib/components/workspace/WorkspaceCenterArea.svelte");
  const rail = source("../src/lib/components/workbench/ActivityRail.svelte");
  const terms = source("../src/lib/i18n/ui-terms.ts");

  assert.doesNotMatch(center, /SiteOverviewWorkspace|centerView === "site"/);
  assert.doesNotMatch(rail, /UI_TERMS\.site|id:\s*"site"|IconSitemap/);
  assert.doesNotMatch(terms, /^\s*site:\s*"Site",?$/m);
});

test("Site nu mai este activitate sau center view, dar valorile vechi au migrare Rust", () => {
  const types = source("../src/lib/types.ts");
  const appState = source("../src/lib/state/app.svelte.ts");
  const workbenchModel = source("../src-tauri/src/kernel/workbench/model.rs");
  const contextModel = source("../src-tauri/src/kernel/context_hub/model.rs");
  const commandSearch = source("../src-tauri/src/kernel/command_center/search.rs");

  const centerView = types.slice(
    types.indexOf("export type CenterView"),
    types.indexOf("export type ApplicationSurface"),
  );
  const workbenchActivity = types.slice(
    types.indexOf("export type WorkbenchActivity"),
    types.indexOf("export type WorkbenchSurface"),
  );

  assert.doesNotMatch(centerView, /"site"/);
  assert.doesNotMatch(workbenchActivity, /"site"/);
  assert.doesNotMatch(appState, /activity === "site"|centerView = "site"|view === "site"/);
  assert.doesNotMatch(commandSearch, /WorkbenchActivity::Site/);
  assert.doesNotMatch(workbenchModel, /^\s*Site,\s*$/m);
  assert.doesNotMatch(contextModel, /^\s*Site,\s*$/m);
  assert.match(workbenchModel, /#\[serde\(alias = "site"\)\]\s*Templates/);
  assert.match(contextModel, /#\[serde\(alias = "site"\)\]\s*Preview/);
});
