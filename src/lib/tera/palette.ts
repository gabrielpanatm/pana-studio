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
  if (item.kind === "componentDefinition") {
    const name = item.name || "component";
    return `{% component ${name}() %}\n{% endcomponent ${name} %}`;
  }
  if (item.kind === "componentCall") {
    return `{{<${item.name || item.target || "component"} />}}`;
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
