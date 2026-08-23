import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { runInNewContext } from "node:vm";
import { handlePreviewInsertDrop } from "$lib/state/preview-insert-controller";
import { handlePreviewTeraInsertDrop } from "$lib/state/preview-tera-insert-controller";
import { dynamicWidgetInstanceIdFromSnippet } from "$lib/state/tera-actions-controller";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

function bridgeFunction(name, nextName) {
  const bridge = source("../src-tauri/src/preview/bridge/07_drag_drop.js");
  const start = bridge.indexOf(`function ${name}(`);
  const end = bridge.indexOf(`\n\n  function ${nextName}(`, start);
  assert.notEqual(start, -1, `${name} lipsește din bridge`);
  assert.notEqual(end, -1, `${nextName} nu delimitează ${name}`);
  return runInNewContext(`(${bridge.slice(start, end).trim()})`);
}

function teraBridgeNormalizer() {
  const bridge = source("../src-tauri/src/preview/bridge/07_drag_drop.js");
  const start = bridge.indexOf("var teraConstructKinds =");
  const end = bridge.indexOf("\n\n  function resetPreviewTeraInsertDragState(", start);
  assert.notEqual(start, -1, "teraConstructKinds lipsește din bridge");
  assert.notEqual(end, -1, "normalizatorul Tera nu poate fi delimitat");
  return runInNewContext(
    `(function () { ${bridge.slice(start, end)}; return normalizedTeraItemPayload; })()`,
  );
}

test("catalogul de inserare este un snapshot Rust versionat și legat de revizie", () => {
  const kernel = source("../src-tauri/src/kernel/insert_catalog.rs");
  const command = source("../src-tauri/src/commands/insert_catalog.rs");
  const registry = source("../src-tauri/src/tauri_command_registry.rs");
  const contracts = source("../src/lib/blocks/contracts.ts");
  const io = source("../src/lib/blocks/io.ts");
  const dragService = source("../src/lib/creation/insert-catalog-drag-service.ts");

  assert.match(kernel, /INSERT_CATALOG_SCHEMA_VERSION:\s*u32\s*=\s*2/);
  assert.match(contracts, /INSERT_CATALOG_SCHEMA_VERSION\s*=\s*2\s+as const/);
  assert.match(io, /snapshot\.schemaVersion !== INSERT_CATALOG_SCHEMA_VERSION/);
  assert.match(dragService, /snapshot\.schemaVersion !== INSERT_CATALOG_SCHEMA_VERSION/);
  for (const field of [
    "project_root",
    "runtime_session_id",
    "workspace_revision",
    "model_revision",
  ]) assert.match(kernel, new RegExp(`pub ${field}:`));
  for (const category of ["Html", "Block", "Component", "Tera", "DynamicWidget"]) {
    assert.match(kernel, new RegExp(`\\b${category},`));
  }
  assert.match(command, /expected_workspace_revision/);
  assert.match(command, /workspace\.revision != request\.expected_workspace_revision/);
  assert.match(command, /workspace\.revision != workspace_revision/);
  assert.match(command, /insert_catalog_stale_revision/);
  assert.match(registry, /read_insert_catalog/);
});

test("snapshotul v2 nu mai construiește sau expune inserarea directField", () => {
  const kernel = source("../src-tauri/src/kernel/insert_catalog.rs");
  const contracts = source("../src/lib/blocks/contracts.ts");
  const adapter = source("../src/lib/state/insert-catalog-drag-controller.ts");
  const panel = source("../src/lib/components/project/InsertCatalogPanel.svelte");

  assert.doesNotMatch(kernel, /\bDirectField\b|InsertCatalogDynamicBinding|InsertCatalogPayload::DynamicField|direct_field_group|direct-fields/);
  assert.doesNotMatch(contracts, /"directField"|kind:\s*"dynamicField"|ProjectDynamicFieldBinding/);
  assert.doesNotMatch(adapter, /payload\.kind\s*===?\s*"dynamicField"|payload\.kind\s*!==?\s*"dynamicField"|dynamicBinding:/);
  assert.doesNotMatch(panel, /"dynamicField"|insert_catalog_dynamic_item_scope_requires_loop/);
});

test("panoul legacy nu mai reconstruiește cataloage locale în Svelte", () => {
  const pane = source("../src/lib/components/ProjectPane.svelte");
  const panel = source("../src/lib/components/project/InsertCatalogPanel.svelte");
  const oldPanelUrl = new URL("../src/lib/components/project/ProjectStructureTab.svelte", import.meta.url);

  assert.equal(existsSync(oldPanelUrl), false);
  assert.match(pane, /<InsertCatalogPanel/);
  assert.doesNotMatch(pane, /ProjectStructureTab|readNativeBlockRegistry|htmlPaletteGroups|teraPaletteGroups/);
  assert.match(panel, /readInsertCatalog/);
  assert.match(panel, /workspaceRevision/);
  assert.match(panel, /snapshot\?\.workspaceRevision !== workspaceRevision/);
  assert.match(panel, /type="search"/);
  assert.match(panel, /onpointerdown=/);
  assert.doesNotMatch(panel, /onclick=\{\(\) => snapshot && startDrag/);
  assert.doesNotMatch(pane, /element-palette-header/);
  assert.match(panel, /class="ui-icon-button compact catalog-close"/);
  assert.match(panel, /onclick=\{close\}|onclick=close/);
});

test("catalogul vizual ocupă panoul și separă categoria HTML de suprafața unică a elementelor", () => {
  const pane = source("../src/lib/components/ProjectPane.svelte");
  const panel = source("../src/lib/components/project/InsertCatalogPanel.svelte");

  assert.match(pane, /\.element-palette-body\s*\{[^}]*display:\s*flex[^}]*overflow:\s*hidden/s);
  assert.match(panel, /\.insert-catalog\s*\{[^}]*height:\s*100%[^}]*overflow:\s*hidden/s);
  assert.match(panel, /\.catalog-scroll\s*\{[^}]*flex:\s*1\s+1\s+auto[^}]*overflow:\s*auto/s);
  for (const section of ["Structură", "Text și titluri", "Text în linie", "Liste", "Media", "Conținut încorporat", "Interactiv", "Formulare", "Tabele", "Avansat"]) {
    assert.match(panel, new RegExp(section));
  }
  assert.match(panel, /buildPresentedSections\(activeCategory/);
  assert.match(panel, /class="html-category-filter"/);
  assert.match(panel, /bind:value=\{activeHtmlSection\}/);
  assert.match(panel, /class="catalog-item ui-entity-selectable"/);
  assert.match(panel, /IconGripVertical/);
  assert.match(panel, /grid-template-columns:\s*minmax\(0,\s*1fr\)/);
  assert.doesNotMatch(panel, /categoryCount|catalog-heading|section-heading|section-count|class="catalog-section"/);
  assert.match(panel, /\.catalog-scroll\s*\{[^}]*background:\s*var\(--material-inset\)[^}]*box-shadow:\s*var\(--shadow-inset\)/s);
  assert.match(panel, /\.catalog-item\s*\{[^}]*background:\s*var\(--material-control\)[^}]*box-shadow:\s*var\(--shadow-control\)/s);
  assert.doesNotMatch(panel, /\.catalog-section\s*\{/);
  for (const icon of ["IconArticle", "IconHeading", "IconQuote", "IconLink", "IconForms", "IconTable"]) {
    assert.match(panel, new RegExp(icon));
  }
});

test("inventarul HTML autoritar rămâne în Rust și acoperă elementele editabile ale documentului", () => {
  const kernel = source("../src-tauri/src/kernel/insert_catalog.rs");

  for (const tag of [
    "h6", "hgroup", "address", "search", "hr", "details", "summary", "dialog",
    "mark", "dfn", "time", "b", "i", "u", "s", "bdi", "bdo", "br", "wbr",
    "ins", "del", "ruby", "rt", "rp", "menu", "img", "picture", "video", "audio",
    "source", "track", "iframe", "canvas", "object", "embed", "map", "area", "optgroup",
    "datalist", "progress", "meter", "colgroup", "col", "template", "slot",
  ]) {
    assert.match(kernel, new RegExp(`\"${tag}\",`));
  }
  for (const internal of ["html", "head", "body", "base", "link", "meta", "title", "style", "script"]) {
    assert.doesNotMatch(kernel.match(/fn html_group[\s\S]*?fn block_group/)?.[0] ?? "", new RegExp(`\"${internal}\",`));
  }
});

test("un adaptor comun convertește itemii discriminați fără autoritate HTML pentru blocuri", () => {
  const adapter = source("../src/lib/state/insert-catalog-drag-controller.ts");
  const service = source("../src/lib/creation/insert-catalog-drag-service.ts");

  assert.match(adapter, /startInsertCatalogDrag/);
  assert.match(adapter, /startElementPaletteDrag\(host, html, event\)/);
  assert.match(adapter, /startTeraPaletteDrag\(host, tera, event\)/);
  assert.match(adapter, /blockId:\s*item\.payload\.blockId/);
  assert.match(adapter, /Rust renders the authoritative block source from blockId/);
  assert.match(adapter, /html:\s*""/);
  assert.match(service, /snapshot\.workspaceRevision !== currentRevision/);
  assert.match(service, /Catalogul de inserare s-a actualizat/);
});

test("bridge-ul iframe păstrează identitatea tuturor blocurilor native până la Rust", async () => {
  const normalize = bridgeFunction(
    "normalizedInsertElementPayload",
    "resetPreviewInsertDragState",
  );
  for (const [blockId, tag, blockKind] of [
    ["icon", "svg", "static"],
    ["counter", "span", "js"],
    ["accordion", "div", "js"],
    ["tabs", "div", "js"],
    ["dialog", "div", "js"],
    ["offcanvas", "div", "js"],
    ["nav-menu", "nav", "js"],
  ]) {
    const bridged = normalize({
      id: `block:${blockId}`,
      kind: "block",
      blockId,
      blockKind,
      tag,
      label: blockId,
      text: "",
      className: blockId,
      html: "",
    });

    assert.equal(bridged.kind, "block");
    assert.equal(bridged.blockId, blockId);
    assert.equal(bridged.blockKind, blockKind);

    let request = null;
    await handlePreviewInsertDrop({
      insertPaletteElementAtTarget: async (next) => {
        request = next;
        return { status: "committed" };
      },
      setGlobalStatus() {},
    }, {
      targetSessionId: "session-1",
      targetSourceId: "source-main",
      targetTag: "main",
      position: "inside",
      element: bridged,
    });

    assert.equal(request?.element.kind, "block");
    assert.equal(request?.element.blockId, blockId);
    assert.equal(request?.element.blockKind, blockKind);
  }
});

test("payloadul HTML component legacy este refuzat, nu convertit sau reinterpretat", async () => {
  const normalize = bridgeFunction(
    "normalizedInsertElementPayload",
    "resetPreviewInsertDragState",
  );
  const bridge = source("../src-tauri/src/preview/bridge/07_drag_drop.js");
  const controller = source("../src/lib/state/preview-insert-controller.ts");
  const palette = source("../src/lib/html/palette.ts");
  const engine = source("../src-tauri/src/project_model/insert_engine.rs");
  const rustNormalizer = engine.match(/fn build_insert_snippet[\s\S]*?fn build_native_block_insert_snippet/)?.[0] ?? "";
  const legacy = {
    id: "component:counter",
    kind: "component",
    componentId: "counter",
    componentKind: "js",
    tag: "span",
    label: "Counter",
  };

  assert.equal(normalize(legacy), null);
  assert.doesNotMatch(bridge, /legacyComponent|componentId|componentKind/);
  assert.doesNotMatch(engine, /alias\s*=\s*"componentId"/);
  assert.doesNotMatch(rustNormalizer, /"block"\s*\|\s*"component"/);
  assert.match(rustNormalizer, /kind != "html"[\s\S]*sunt permise html și block/);
  assert.match(controller, /data\.kind === "html"[\s\S]*data\.kind === "block"[\s\S]*if \(!kind\) return null/);
  assert.match(palette, /kind:\s*"html"\s*\|\s*"block"/);
  assert.doesNotMatch(palette, /kind\?:|htmlPaletteGroups|htmlTagGroups|tagMeta/);

  let calls = 0;
  let statusKind = "";
  await handlePreviewInsertDrop({
    async insertPaletteElementAtTarget() {
      calls += 1;
      return { status: "committed" };
    },
    setGlobalStatus(_message, kind) {
      statusKind = kind;
    },
  }, {
    targetSessionId: "session-1",
    targetSourceId: "source-main",
    targetTag: "main",
    position: "inside",
    element: legacy,
  });
  assert.equal(calls, 0);
  assert.equal(statusKind, "error");
});

test("bridge-ul Tera păstrează macroCall și bindingul dinamic tipizat", () => {
  const normalize = teraBridgeNormalizer();
  const binding = {
    modelId: "serviciu",
    fieldId: "pret",
    path: "pret",
    scope: "page",
    itemPath: null,
    presentation: "text",
    prefix: "",
    suffix: " lei",
    fallback: "0",
    text: "Preț",
  };
  const dynamic = normalize({
    id: "dynamic:serviciu:pret",
    kind: "teraVariable",
    family: "data",
    label: "Preț",
    expression: "page.extra.pret",
    dynamicBinding: binding,
  });
  const macro = normalize({
    id: "component:card",
    kind: "macroCall",
    family: "reuse",
    label: "Card",
    target: "macros/card.html",
    name: "card",
  });
  const widgetProperties = {
    kind: "listing",
    properties: {
      sectionPath: "content/services/_index.md",
      listingItemId: "service-card",
      listingItemTemplate: "listing-items/service-card.html",
      includeSubsections: false,
      sortBy: "none",
      sortOrder: "asc",
      limit: null,
      offset: 0,
      emptyText: "",
      tag: "section",
      className: "services",
    },
  };
  const widget = normalize({
    id: "dynamic-widget:listing",
    kind: "dynamicWidget",
    family: "data",
    label: "Listing",
    dynamicWidget: widgetProperties,
  });

  assert.equal(dynamic.dynamicBinding.modelId, "serviciu");
  assert.equal(dynamic.dynamicBinding.fieldId, "pret");
  assert.equal(macro.kind, "macroCall");
  assert.equal(widget.kind, "dynamicWidget");
  assert.deepEqual(widget.dynamicWidget, widgetProperties);
});

test("același drop Tera aflat în curs produce o singură mutație Rust", async () => {
  let resolveInsert;
  let calls = 0;
  const inserted = new Promise((resolve) => {
    resolveInsert = resolve;
  });
  const host = {
    insertTeraPaletteItemAtTarget: async () => {
      calls += 1;
      return await inserted;
    },
    setGlobalStatus() {},
  };
  const payload = {
    targetSessionId: "session-1",
    targetSourceId: "source-article",
    targetTag: "article",
    position: "inside",
    item: {
      id: "dynamic-widget:field",
      kind: "dynamicWidget",
      family: "data",
      label: "Câmp dinamic",
      dynamicWidget: {
        kind: "dynamicField",
        properties: {
          binding: {
            context: "collectionItem",
            source: { kind: "builtin", field: "title" },
            valueType: "text",
          },
          presentation: "heading",
          tag: "h2",
          format: { dateFormat: "", decimals: null, currency: "" },
          prefix: "",
          suffix: "",
          fallback: "",
          label: "",
          emptyBehavior: "hide",
        },
      },
    },
  };

  const first = handlePreviewTeraInsertDrop(host, payload);
  const second = handlePreviewTeraInsertDrop(host, payload);
  await Promise.resolve();
  assert.equal(calls, 1);
  resolveInsert({ status: "committed" });
  assert.deepEqual(await first, { status: "committed" });
  assert.deepEqual(await second, { status: "committed" });
});

test("identitatea widgetului inserat este extrasă din marker pentru selectarea imediată", () => {
  const actions = source("../src/lib/state/tera-actions-controller.ts");
  assert.equal(dynamicWidgetInstanceIdFromSnippet(
    "{# pana:widget schema=2 provider=dynamic-field instance=dynamic-field-a1b2_c3 props=aa #}",
  ), "dynamic-field-a1b2_c3");
  assert.equal(dynamicWidgetInstanceIdFromSnippet("<h2>Fără marker</h2>"), null);
  assert.match(actions, /await host\.selectDynamicWidgetSourceInstance/);
});

test("catalogul proiectează blocuri, componente Tera și DynamicWidget din grafurile Rust", () => {
  const kernel = source("../src-tauri/src/kernel/insert_catalog.rs");
  const adapter = source("../src/lib/state/insert-catalog-drag-controller.ts");

  assert.match(kernel, /graph\.block_graph\.definitions/);
  assert.match(kernel, /native_block_registry_snapshot/);
  assert.match(kernel, /graph\s*\.component_graph/);
  assert.match(kernel, /ComponentDefinitionKind::Partial/);
  assert.match(kernel, /definition\.active && definition\.shadowed_by\.is_none\(\)/);
  assert.match(kernel, /InsertCatalogPayload::Component/);
  assert.match(kernel, /tera_kind:\s*"include"/);
  assert.match(kernel, /tera_kind:\s*"macroCall"/);
  assert.match(kernel, /dynamic_widget_group/);
  assert.match(kernel, /InsertCatalogPayload::DynamicWidget/);
  assert.match(adapter, /sourceNodeId:\s*payload\.kind === "component" \? payload\.componentId/);
  for (const included of ["img", "video", "audio", "picture", "iframe"]) {
    assert.match(kernel, new RegExp(`html_catalog_covers_authorable_body_elements[\\s\\S]*"${included}"`));
  }
});

test("macrocomenzile fără argumente obligatorii au reprezentare validată de motorul Tera", () => {
  const catalog = source("../src-tauri/src/kernel/insert_catalog.rs");
  const engine = source("../src-tauri/src/project_model/tera_insert_engine.rs");
  const model = source("../src/lib/tera/model.ts");

  assert.match(catalog, /parameters[\s\S]*\.all\(\|parameter\| !parameter\.required\)/);
  assert.match(catalog, /tera_kind:\s*"macroCall"/);
  assert.match(engine, /"macroCall"\s*=>/);
  assert.match(engine, /pana_component::\{name\}\(\)/);
  assert.match(model, /\| "macroCall"/);
});

test("revizia workspace și contextul Canvas sunt trimise reactiv panoului", () => {
  const application = source("../src/lib/components/application/ApplicationWorkspace.svelte");
  const panel = source("../src/lib/components/project/InsertCatalogPanel.svelte");
  const drag = source("../src/lib/creation/insert-catalog-drag-service.ts");
  const io = source("../src/lib/blocks/io.ts");

  assert.match(application, /workspaceRevision:\s*\(\) => projectSession\.workspace\?\.revision \?\? 0/);
  assert.match(application, /activeTemplatePath:\s*\(\) => documents\.activeRenderedTemplatePath/);
  assert.match(application, /activePagePath:\s*\(\) => documents\.templatePreferredPagePath/);
  assert.match(application, /canvasPreviewRevision:\s*\(\) => previewWorkspace\.activeIdentity\?\.previewRevision/);
  assert.match(application, /targetSourceId:\s*\(\) => selectionWorkspace\.coordinatedElement\?\.sourceNodeId/);
  assert.match(panel, /sameContext\(snapshot\?\.context, context\)/);
  assert.match(drag, /context\.canvasPreviewRevision !== \(preview\.activeIdentity\?\.previewRevision \?\? null\)/);
  assert.match(drag, /context\.targetSourceId !== \(selection\.coordinatedElement\?\.sourceNodeId \?\? null\)/);
  assert.match(io, /snapshot\.workspaceRevision !== expectedWorkspaceRevision/);
});
