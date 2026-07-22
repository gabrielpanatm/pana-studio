import { templateNameForPath } from "$lib/project/files";
import type { SourceGraph, SourceGraphNode } from "$lib/types";
import type { TeraPaletteGroup, TeraPaletteItem } from "$lib/tera/model";

function templateReferenceForFile(file: string) {
  return templateNameForPath(file);
}

function partialLabel(node: SourceGraphNode) {
  const reference = templateReferenceForFile(node.file);
  return reference.replace(/^partials\//, "").replace(/\.html$/, "") || node.label;
}

export function teraSnippetForItem(item: TeraPaletteItem) {
  if (item.snippet?.trim()) {
    return item.snippet.trim();
  }
  if (item.kind === "extends") {
    return `{% extends "${item.target || "base.html"}" %}`;
  }
  if (item.kind === "block") {
    const name = item.name || "content";
    return `{% block ${name} %}\n{% endblock %}`;
  }
  if (item.kind === "include") {
    return `{% include "${item.target || "partials/cta.html"}" %}`;
  }
  if (item.kind === "import") {
    return `{% import "${item.target || "macros.html"}" as ${item.name || "macros"} %}`;
  }
  if (item.kind === "macro") {
    return `{% macro ${item.name || "componenta"}() %}\n{% endmacro %}`;
  }
  if (item.kind === "for") {
    return `{% for ${item.expression || "item in items"} %}\n{% endfor %}`;
  }
  if (item.kind === "if") {
    return `{% if ${item.expression || "condition"} %}\n{% endif %}`;
  }
  if (item.kind === "set") {
    return `{% set ${item.expression || "name = value"} %}`;
  }
  if (item.kind === "with") {
    return `{% with ${item.expression || "value = value"} %}\n{% endwith %}`;
  }
  if (item.kind === "teraVariable") {
    return `{{ ${item.expression || "value"} }}`;
  }
  if (item.kind === "teraComment") {
    return `{# ${item.expression || "comentariu"} #}`;
  }
  return `{% raw %}\n{% endraw %}`;
}

function item(data: Omit<TeraPaletteItem, "snippet"> & { snippet?: string }): TeraPaletteItem {
  const itemData = { ...data, snippet: data.snippet ?? "" };
  return {
    ...itemData,
    snippet: itemData.snippet || teraSnippetForItem(itemData),
  };
}

function partialItems(graph: SourceGraph | null): TeraPaletteItem[] {
  const partials = graph?.nodes
    .filter((node) => node.kind === "partial")
    .sort((a, b) => templateReferenceForFile(a.file).localeCompare(templateReferenceForFile(b.file))) ?? [];

  return partials.map((node) => {
    const target = templateReferenceForFile(node.file);
    return item({
      id: `include:${target}`,
      kind: "include",
      family: "composition",
      label: `Include ${partialLabel(node)}`,
      description: target,
      target,
      sourceNodeId: node.id,
    });
  });
}

export function teraPaletteGroups(graph: SourceGraph | null, loopItems: TeraPaletteItem[] = []): TeraPaletteGroup[] {
  const partials = partialItems(graph);
  return [
    {
      label: "Compoziție Tera",
      description: "Șabloane de bază, blocuri și fragmente.",
      items: [
        item({
          id: "extends:base",
          kind: "extends",
          family: "composition",
          label: "Extinde șablonul de bază",
          description: "Leagă template-ul la un șablon de bază.",
          target: "base.html",
        }),
        item({
          id: "block:content",
          kind: "block",
          family: "composition",
          label: "Bloc de conținut",
          description: "Zonă Tera cu conținut suprascris.",
          name: "content",
        }),
        item({
          id: "include:generic",
          kind: "include",
          family: "composition",
          label: "Include fragment",
          description: "Inserează un fragment reutilizabil.",
          target: partials[0]?.target || "partials/cta.html",
        }),
        item({
          id: "import:macros",
          kind: "import",
          family: "composition",
          label: "Importă macrocomenzi",
          description: "Importă macrocomenzi Tera.",
          target: "macros.html",
          name: "macros",
        }),
      ],
    },
    ...(partials.length > 0
      ? [{
          label: "Fragmente",
          description: "Includeri din harta surselor.",
          items: partials,
        }]
      : []),
    ...(loopItems.length > 0
      ? [{
          label: "Bucle create",
          description: "Blocuri configurate în Editor.",
          items: loopItems,
        }]
      : []),
    {
      label: "Logică Tera",
      description: "Condiții, bucle și context.",
      items: [
        item({
          id: "if:condition",
          kind: "if",
          family: "logic",
          label: "Condiție",
          description: "Condiție Tera.",
          expression: "condition",
        }),
        item({
          id: "with:value",
          kind: "with",
          family: "logic",
          label: "Context local",
          description: "Context local pentru variabile.",
          expression: "value = value",
        }),
      ],
    },
    {
      label: "Date și reutilizare",
      description: "Variabile, setări, macro-uri și zone sigure.",
      items: [
        item({
          id: "set:name",
          kind: "set",
          family: "data",
          label: "Definește variabilă",
          description: "Definește o variabilă Tera.",
          expression: "name = value",
        }),
        item({
          id: "variable:value",
          kind: "teraVariable",
          family: "data",
          label: "Variabilă",
          description: "Afișează o expresie.",
          expression: "value",
        }),
        item({
          id: "macro:componenta",
          kind: "macro",
          family: "reuse",
          label: "Macrocomandă",
          description: "Definește o funcție reutilizabilă.",
          name: "componenta",
        }),
        item({
          id: "comment:tera",
          kind: "teraComment",
          family: "safe",
          label: "Comentariu",
          description: "Comentariu Tera.",
          expression: "comentariu",
        }),
        item({
          id: "raw:tera",
          kind: "raw",
          family: "safe",
          label: "Conținut brut",
          description: "Zonă neinterpretată de Tera.",
        }),
      ],
    },
  ];
}
