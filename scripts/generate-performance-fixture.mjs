import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";

const markerName = ".pana-performance-fixture.json";

function encodeDynamicFieldProperties(context) {
  const properties = {
    kind: "dynamicField",
    properties: {
      binding: {
        context,
        source: {
          kind: "customField",
          modelId: "service",
          fieldId: "field-title",
        },
        valueType: "text",
      },
      presentation: "heading",
      tag: "h2",
      format: {
        dateFormat: "",
        decimals: null,
        currency: "",
      },
      prefix: "",
      suffix: "",
      fallback: "Fără titlu",
      label: "",
      emptyBehavior: "fallback",
    },
  };
  return Buffer.from(JSON.stringify(properties), "utf8").toString("hex");
}

function dynamicFieldWidget(instanceId, context, expression) {
  const encoded = encodeDynamicFieldProperties(context);
  return [
    `{# pana:widget schema=2 provider=dynamic-field instance=${instanceId} props=${encoded} #}`,
    `<h2 data-pana-widget-instance="${instanceId}">${expression}</h2>`,
    `{# /pana:widget instance=${instanceId} #}`,
  ];
}

function write(root, relativePath, source) {
  const path = join(root, relativePath);
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, source);
}

function resetOwnedTarget(root) {
  if (!existsSync(root)) return;
  const entries = readdirSync(root);
  if (entries.length === 0) return;
  const markerPath = join(root, markerName);
  if (!existsSync(markerPath)) {
    throw new Error(`Refusing to replace unowned directory: ${root}`);
  }
  const marker = JSON.parse(readFileSync(markerPath, "utf8"));
  if (marker.schemaVersion !== 1 || marker.owner !== "pana-studio-performance") {
    throw new Error(`Invalid performance fixture marker: ${markerPath}`);
  }
  rmSync(root, { recursive: true, force: false });
}

export function generatePerformanceFixture(options = {}) {
  const root = resolve(options.root ?? join(tmpdir(), "pana-studio-performance-fixture"));
  // Keep the deterministic source set below the kernel's 1,000-entry accepted
  // disk manifest ceiling. A truncated manifest would benchmark an incomplete
  // project and could silently omit the templates exercised by edit probes.
  const pageCount = options.pageCount ?? 350;
  const componentCount = options.componentCount ?? 120;
  const nodeCount = options.nodeCount ?? 1_000;
  for (const [name, value] of Object.entries({ pageCount, componentCount, nodeCount })) {
    if (!Number.isSafeInteger(value) || value <= 0) {
      throw new Error(`${name} must be a positive integer`);
    }
  }
  resetOwnedTarget(root);
  mkdirSync(root, { recursive: true });

  write(root, "zola.toml", [
    'base_url = "https://performance.invalid"',
    'title = "Pană Studio Performance Fixture"',
    'default_language = "ro"',
    "compile_sass = true",
    "minify_html = false",
    "build_search_index = false",
    "",
  ].join("\n"));
  write(root, "content/_index.md", [
    "+++",
    'title = "Acasă"',
    'template = "index.html"',
    "+++",
    "",
  ].join("\n"));
  write(root, ".panastudio/project.toml", "schema_version = 1\n");
  write(root, ".panastudio/assignments.toml", [
    "schema_version = 1",
    "",
    "[[assignments]]",
    'sectionPath = "content/_index.md"',
    'modelId = "service"',
    "",
  ].join("\n"));
  write(root, ".panastudio/content-models/service.toml", [
    "schemaVersion = 1",
    'id = "service"',
    'label = "Serviciu"',
    "",
    "[[fields]]",
    'id = "field-title"',
    'key = "title"',
    'label = "Titlu"',
    'kind = "text"',
    "",
  ].join("\n"));
  write(root, ".panastudio/listing-items.toml", [
    "schema_version = 1",
    "",
    "[[items]]",
    'id = "service-card"',
    'label = "Card serviciu"',
    'templateName = "listing-items/service-card.html"',
    'modelId = "service"',
    'previewPageFile = "content/pages/page-0000.md"',
    "",
  ].join("\n"));
  write(root, "templates/base.html", [
    '<!doctype html><html lang="ro"><head><meta charset="utf-8">',
    '<link rel="stylesheet" href="/site.css"></head><body>',
    "{% block content %}{% endblock content %}</body></html>",
    "",
  ].join("\n"));
  write(root, "templates/macros/performance.html", [
    "{% macro badge(text) %}",
    '<strong class="performance-badge">{{ text }}</strong>',
    "{% endmacro badge %}",
    "",
  ].join("\n"));
  write(root, "templates/listing-items/service-card.html", [
    '<article class="service-card">',
    ...dynamicFieldWidget(
      "dynamic-field-performance01",
      "collectionItem",
      "{{ item.extra.title }}",
    ),
    "</article>",
    "",
  ].join("\n"));

  const indexNodes = Array.from({ length: nodeCount }, (_, index) => (
    `  <article class="card card-${index % componentCount}" data-index="${index}"><h2>Nod ${index}</h2><p>Conținut determinist ${index}</p></article>`
  ));
  write(root, "templates/index.html", [
    "{% extends 'base.html' %}",
    "{% import 'macros/performance.html' as performance %}",
    "{% block content %}",
    '<main id="performance-root">',
    "  {% include 'components/card-000.html' %}",
    "  {{ performance::badge(text=page.title) }}",
    "  {% for item in [1, 2, 3] %}<span>{{ item }}</span>{% endfor %}",
    '  <span data-pana-block="counter" data-pana-instance="performance-counter">0</span>',
    "  {{ page.content | safe }}",
    "  {{ page.extra.title }}",
    ...dynamicFieldWidget(
      "dynamic-field-performance01",
      "page",
      "{{ page.extra.title }}",
    ).map((line) => `  ${line}`),
    ...indexNodes,
    "</main>",
    "{% endblock content %}",
    "",
  ].join("\n"));

  for (let index = 0; index < componentCount; index += 1) {
    write(root, `templates/components/card-${String(index).padStart(3, "0")}.html`, (
      `<article class="card card-${index}"><h2>{{ title }}</h2></article>\n`
    ));
  }
  for (let index = 0; index < pageCount; index += 1) {
    const id = String(index).padStart(4, "0");
    write(root, `content/pages/page-${id}.md`, [
      "+++",
      `title = "Pagina ${id}"`,
      `template = "pages/page-${id}.html"`,
      "+++",
      "",
      `Conținut pagina ${id}.`,
      "",
    ].join("\n"));
    write(root, `templates/pages/page-${id}.html`, [
      "{% extends 'base.html' %}",
      "{% block content %}",
      `<main><h1>Pagina ${id}</h1>{% include 'components/card-${String(index % componentCount).padStart(3, "0")}.html' %}</main>`,
      "{% endblock content %}",
      "",
    ].join("\n"));
  }
  const styles = Array.from({ length: componentCount }, (_, index) => (
    `.card-${index} { color: rgb(${index % 255} 40 80); padding: ${index % 24}px; }`
  ));
  write(root, "sass/site.scss", [
    "$performance-accent: #16836f;",
    ".card { border: 1px solid $performance-accent; }",
    ...styles,
    "",
  ].join("\n"));

  const manifest = {
    schemaVersion: 1,
    owner: "pana-studio-performance",
    pageCount,
    componentCount,
    nodeCount,
    expectedSourceFileCount: 11 + pageCount * 2 + componentCount,
  };
  write(root, markerName, `${JSON.stringify(manifest, null, 2)}\n`);
  return { root, ...manifest };
}

function parseArguments(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--root") options.root = argv[++index];
    else if (argument === "--pages") options.pageCount = Number(argv[++index]);
    else if (argument === "--components") options.componentCount = Number(argv[++index]);
    else if (argument === "--nodes") options.nodeCount = Number(argv[++index]);
    else throw new Error(`Unknown argument: ${argument}`);
  }
  return options;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    console.log(JSON.stringify(generatePerformanceFixture(parseArguments(process.argv.slice(2))), null, 2));
  } catch (error) {
    console.error(`[performance-fixture] ${error.message}`);
    process.exitCode = 1;
  }
}
