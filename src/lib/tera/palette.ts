import type { TeraPaletteItem } from "$lib/tera/model";

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
  if (item.kind === "macroCall") {
    return `{% import "${item.target || "macros.html"}" as pana_component %}\n{{ pana_component::${item.name || "component"}() }}`;
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
  if (item.kind === "dynamicWidget") {
    return `{# ${item.label || "Widget dinamic"} — sursa este generată de Rust #}`;
  }
  return `{% raw %}\n{% endraw %}`;
}
