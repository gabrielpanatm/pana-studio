import { templateNameForPath } from "$lib/project/files";
import { t } from "$lib/i18n/runtime.svelte";
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
    return `{% macro ${item.name || "component"}() %}\n{% endmacro %}`;
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
  if (item.kind === "teraVariable") {
    return `{{ ${item.expression || "value"} }}`;
  }
  if (item.kind === "teraComment") {
    return `{# ${item.expression || "comment"} #}`;
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
      label: t("project-tera-include-partial", { name: partialLabel(node) }),
      description: target,
      target,
      sourceNodeId: node.id,
    });
  });
}

export function teraPaletteGroups(graph: SourceGraph | null): TeraPaletteGroup[] {
  const partials = partialItems(graph);
  return [
    {
      label: t("project-tera-group-composition"),
      description: t("project-tera-group-composition-description"),
      items: [
        item({
          id: "extends:base",
          kind: "extends",
          family: "composition",
          label: t("project-tera-extends"),
          description: t("project-tera-extends-description"),
          target: "base.html",
        }),
        item({
          id: "block:content",
          kind: "block",
          family: "composition",
          label: t("project-tera-block"),
          description: t("project-tera-block-description"),
          name: "content",
        }),
        item({
          id: "include:generic",
          kind: "include",
          family: "composition",
          label: t("project-tera-include"),
          description: t("project-tera-include-description"),
          target: partials[0]?.target || "partials/cta.html",
        }),
        item({
          id: "import:macros",
          kind: "import",
          family: "composition",
          label: t("project-tera-import"),
          description: t("project-tera-import-description"),
          target: "macros.html",
          name: "macros",
        }),
      ],
    },
    ...(partials.length > 0
      ? [{
          label: t("project-tera-group-partials"),
          description: t("project-tera-group-partials-description"),
          items: partials,
        }]
      : []),
    {
      label: t("project-tera-group-logic"),
      description: t("project-tera-group-logic-description"),
      items: [
        item({
          id: "for:items",
          kind: "for",
          family: "logic",
          label: t("project-tera-loop"),
          description: t("project-tera-loop-description"),
          expression: "item in items",
        }),
        item({
          id: "if:condition",
          kind: "if",
          family: "logic",
          label: t("project-tera-condition"),
          description: t("project-tera-condition-description"),
          expression: "condition",
        }),
      ],
    },
    {
      label: t("project-tera-group-data"),
      description: t("project-tera-group-data-description"),
      items: [
        item({
          id: "set:name",
          kind: "set",
          family: "data",
          label: t("project-tera-set"),
          description: t("project-tera-set-description"),
          expression: "name = value",
        }),
        item({
          id: "variable:value",
          kind: "teraVariable",
          family: "data",
          label: t("project-tera-variable"),
          description: t("project-tera-variable-description"),
          expression: "value",
        }),
        item({
          id: "macro:componenta",
          kind: "macro",
          family: "reuse",
          label: t("project-tera-macro"),
          description: t("project-tera-macro-description"),
          name: "component",
        }),
        item({
          id: "comment:tera",
          kind: "teraComment",
          family: "safe",
          label: t("project-tera-comment"),
          description: t("project-tera-comment-description"),
          expression: "comment",
        }),
        item({
          id: "raw:tera",
          kind: "raw",
          family: "safe",
          label: t("project-tera-raw"),
          description: t("project-tera-raw-description"),
        }),
      ],
    },
  ];
}
