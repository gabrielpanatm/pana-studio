import type { TeraPaletteItem } from "$lib/tera/model";

export type LoopSourceKind = "sectionPages" | "sectionExtra" | "configExtra" | "dataFile" | "custom";
export type LoopLayoutKind = "cardGrid" | "linkList" | "plainList";

export type LoopDefinition = {
  id: string;
  label: string;
  sourceKind: LoopSourceKind;
  alias: string;
  layout: LoopLayoutKind;
  sectionPath?: string;
  extraKey?: string;
  dataPath?: string;
  collectionKey?: string;
  customCollection?: string;
  titleExpression: string;
  descriptionExpression?: string;
  urlExpression?: string;
  createdAt: number;
};

export const loopSourceOptions: Array<{
  kind: LoopSourceKind;
  label: string;
  description: string;
}> = [
  {
    kind: "sectionPages",
    label: "Pagini din secțiune",
    description: "Folosește get_section(...).pages, sigur în orice template.",
  },
  {
    kind: "sectionExtra",
    label: "Date din section.extra",
    description: "Folosește un array definit în front matter-ul unei secțiuni.",
  },
  {
    kind: "configExtra",
    label: "Date din config.extra",
    description: "Folosește liste globale din zola.toml.",
  },
  {
    kind: "dataFile",
    label: "Fișier data",
    description: "Folosește load_data(path=...).",
  },
  {
    kind: "custom",
    label: "Expresie avansată",
    description: "Colecție Tera scrisă manual.",
  },
];

export const loopLayoutOptions: Array<{
  kind: LoopLayoutKind;
  label: string;
  description: string;
}> = [
  {
    kind: "cardGrid",
    label: "Grid de carduri",
    description: "Secțiune cu carduri repetate.",
  },
  {
    kind: "linkList",
    label: "Listă de linkuri",
    description: "Listă compactă pentru navigație/arhivă.",
  },
  {
    kind: "plainList",
    label: "Listă simplă",
    description: "Text repetat, fără card.",
  },
];

export function defaultLoopDefinition(): LoopDefinition {
  const createdAt = Date.now();
  return normalizeLoopDefinition({
    id: `loop-${createdAt}`,
    label: "Pagini secțiune",
    sourceKind: "sectionPages",
    alias: "page",
    layout: "cardGrid",
    sectionPath: "_index.md",
    titleExpression: "page.title",
    descriptionExpression: "page.description",
    urlExpression: "page.permalink",
    createdAt,
  });
}

export function loopDefaultsForSource(kind: LoopSourceKind): Partial<LoopDefinition> {
  if (kind === "sectionPages") {
    return {
      label: "Pagini secțiune",
      alias: "page",
      sectionPath: "_index.md",
      titleExpression: "page.title",
      descriptionExpression: "page.description",
      urlExpression: "page.permalink",
    };
  }
  if (kind === "sectionExtra") {
    return {
      label: "Servicii",
      alias: "service",
      sectionPath: "_index.md",
      extraKey: "services",
      titleExpression: "service.title",
      descriptionExpression: "service.description",
      urlExpression: "service.url",
    };
  }
  if (kind === "configExtra") {
    return {
      label: "Navigație",
      alias: "item",
      extraKey: "nav",
      titleExpression: "item.label",
      descriptionExpression: "item.description",
      urlExpression: "item.url",
    };
  }
  if (kind === "dataFile") {
    return {
      label: "Date servicii",
      alias: "service",
      dataPath: "data/services.toml",
      collectionKey: "services",
      titleExpression: "service.title",
      descriptionExpression: "service.description",
      urlExpression: "service.url",
    };
  }
  return {
    label: "Loop custom",
    alias: "item",
    customCollection: "items",
    titleExpression: "item.title",
    descriptionExpression: "item.description",
    urlExpression: "item.url",
  };
}

export function normalizeLoopDefinition(definition: LoopDefinition): LoopDefinition {
  const alias = safeIdentifier(definition.alias || "item");
  const label = cleanLabel(definition.label || "Loop");
  return {
    ...definition,
    id: definition.id || `loop-${Date.now()}`,
    label,
    alias,
    layout: definition.layout || "cardGrid",
    sectionPath: cleanPath(definition.sectionPath || "_index.md"),
    extraKey: safeProperty(definition.extraKey || "items"),
    dataPath: cleanPath(definition.dataPath || "data/items.toml"),
    collectionKey: safeProperty(definition.collectionKey || "items"),
    customCollection: cleanExpression(definition.customCollection || `${alias}s`),
    titleExpression: cleanExpression(definition.titleExpression || `${alias}.title`),
    descriptionExpression: cleanExpression(definition.descriptionExpression || ""),
    urlExpression: cleanExpression(definition.urlExpression || ""),
    createdAt: definition.createdAt || Date.now(),
  };
}

export function loopPaletteItemForDefinition(definition: LoopDefinition): TeraPaletteItem {
  const loop = normalizeLoopDefinition(definition);
  return {
    id: `loop:${loop.id}`,
    kind: "for",
    family: "logic",
    label: loop.label,
    description: loopDescription(loop),
    snippet: loopSnippetForDefinition(loop),
    expression: loopExpressionLabel(loop),
  };
}

export function loopSnippetForDefinition(definition: LoopDefinition) {
  const loop = normalizeLoopDefinition(definition);
  const setup = loopSetupLines(loop);
  const collection = loopCollectionExpression(loop);
  const blockClassName = `pana-loop-${slugify(loop.label)}`;
  const body = loopItemBody(loop);
  const setupText = setup.length ? `${setup.join("\n")}\n` : "";

  return `${setupText}<section class="pana-loop ${blockClassName}">\n  <div class="${blockClassName}__items">\n    {% for ${loop.alias} in ${collection} %}\n${indent(body, "      ")}\n    {% endfor %}\n  </div>\n</section>`;
}

export function loopExpressionLabel(definition: LoopDefinition) {
  const loop = normalizeLoopDefinition(definition);
  return `${loop.alias} in ${loopCollectionExpression(loop).replace(/\s+/g, " ")}`;
}

export function loopDescription(definition: LoopDefinition) {
  const loop = normalizeLoopDefinition(definition);
  const source = loopSourceOptions.find((option) => option.kind === loop.sourceKind)?.label ?? "Sursă";
  const layout = loopLayoutOptions.find((option) => option.kind === loop.layout)?.label ?? "Layout";
  return `${source} · ${layout} · ${loopExpressionLabel(loop)}`;
}

export function loopSandboxItems(definition: LoopDefinition) {
  const loop = normalizeLoopDefinition(definition);
  const title = loop.alias === "page" ? "Pagină" : capitalize(loop.alias);
  return [1, 2, 3].map((index) => ({
    title: `${title} ${index}`,
    description: `Descriere generată din ${loopDescription(loop).split(" · ")[0].toLowerCase()}.`,
  }));
}

function loopSetupLines(loop: LoopDefinition) {
  if (loop.sourceKind === "sectionPages" || loop.sourceKind === "sectionExtra") {
    return [`{% set loop_section = get_section(path="${escapeTeraString(loop.sectionPath || "_index.md")}") %}`];
  }
  if (loop.sourceKind === "dataFile") {
    return [`{% set loop_data = load_data(path="${escapeTeraString(loop.dataPath || "data/items.toml")}") %}`];
  }
  return [];
}

function loopCollectionExpression(loop: LoopDefinition) {
  if (loop.sourceKind === "sectionPages") return "loop_section.pages";
  if (loop.sourceKind === "sectionExtra") return `loop_section.extra.${safeProperty(loop.extraKey || "items")} | default(value=[])`;
  if (loop.sourceKind === "configExtra") return `config.extra.${safeProperty(loop.extraKey || "items")} | default(value=[])`;
  if (loop.sourceKind === "dataFile") return `loop_data.${safeProperty(loop.collectionKey || "items")} | default(value=[])`;
  return cleanExpression(loop.customCollection || `${loop.alias}s`);
}

function loopItemBody(loop: LoopDefinition) {
  const titleVar = "loop_item_title";
  const descriptionVar = "loop_item_description";
  const urlVar = "loop_item_url";
  const titleExpression = outputExpression(loop.titleExpression, loop.label);
  const descriptionExpression = outputExpression(loop.descriptionExpression || "", "");
  const urlExpression = outputExpression(loop.urlExpression || "", "");
  const setup = [
    `{% set ${titleVar} = ${titleExpression} %}`,
    `{% set ${descriptionVar} = ${descriptionExpression} %}`,
    `{% set ${urlVar} = ${urlExpression} %}`,
  ];

  if (loop.layout === "linkList") {
    return [
      ...setup,
      `<a class="pana-loop-link" href="{% if ${urlVar} %}{{ ${urlVar} | safe }}{% else %}#{% endif %}">{{ ${titleVar} }}</a>`,
    ].join("\n");
  }

  if (loop.layout === "plainList") {
    return [
      ...setup,
      `<p class="pana-loop-item">{{ ${titleVar} }}</p>`,
    ].join("\n");
  }

  return [
    ...setup,
    `<article class="pana-loop-card">`,
    `  <h3>{% if ${urlVar} %}<a href="{{ ${urlVar} | safe }}">{{ ${titleVar} }}</a>{% else %}{{ ${titleVar} }}{% endif %}</h3>`,
    `  {% if ${descriptionVar} %}<p>{{ ${descriptionVar} }}</p>{% endif %}`,
    `</article>`,
  ].join("\n");
}

function outputExpression(expression: string, fallback: string) {
  const clean = cleanExpression(expression);
  if (!clean) return `"${escapeTeraString(fallback)}"`;
  return `${clean} | default(value="${escapeTeraString(fallback)}")`;
}

function indent(value: string, prefix: string) {
  return value.split("\n").map((line) => `${prefix}${line}`).join("\n");
}

function cleanLabel(value: string) {
  return value.trim().replace(/\s+/g, " ").slice(0, 80) || "Loop";
}

function cleanPath(value: string) {
  return value.trim().replace(/^\/+/, "").slice(0, 160);
}

function cleanExpression(value: string) {
  return value.trim().replace(/[{};%]/g, "").slice(0, 180);
}

function safeIdentifier(value: string) {
  const identifier = value.trim().replace(/[^A-Za-z0-9_]/g, "_").replace(/^[^A-Za-z_]+/, "");
  return identifier || "item";
}

function safeProperty(value: string) {
  return safeIdentifier(value || "items");
}

function slugify(value: string) {
  return value
    .toLowerCase()
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 48) || "loop";
}

function capitalize(value: string) {
  return value ? `${value.charAt(0).toUpperCase()}${value.slice(1)}` : "Item";
}

function escapeTeraString(value: string) {
  return value.replace(/\\/g, "\\\\").replace(/"/g, "\\\"");
}
