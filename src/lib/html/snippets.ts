export type HtmlSnippetOptions = {
  tag: string;
  className: string;
  dataAnim?: string;
  text: string;
};

const htmlVoidSnippetTags = new Set([
  "area",
  "base",
  "br",
  "col",
  "embed",
  "hr",
  "img",
  "input",
  "link",
  "meta",
  "param",
  "source",
  "track",
  "wbr",
]);

function escapeText(value: string) {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function escapeAttr(value: string) {
  return escapeText(value)
    .replace(/"/g, "&quot;");
}

function rootClassAttr(className: string) {
  const normalized = className.trim().replace(/\s+/g, " ");
  return normalized ? ` class="${escapeAttr(normalized)}"` : "";
}

function rootDataAnimAttr(dataAnim: string | undefined) {
  const normalized = dataAnim?.trim() ?? "";
  return normalized ? ` data-anim="${escapeAttr(normalized)}"` : "";
}

function rootAttrs(options: Pick<HtmlSnippetOptions, "className" | "dataAnim">) {
  return `${rootClassAttr(options.className)}${rootDataAnimAttr(options.dataAnim)}`;
}

function textOr(value: string, fallback: string) {
  const trimmed = value.trim();
  return escapeText(trimmed || fallback);
}

export function buildHtmlSnippet(options: HtmlSnippetOptions) {
  const tag = options.tag.trim().toLowerCase();
  const attrs = rootAttrs(options);
  const text = options.text.trim();

  if (tag === "a") return `<a${attrs} href="#">${textOr(text, "Link nou")}</a>`;
  if (tag === "button") return `<button${attrs} type="button">${textOr(text, "Buton nou")}</button>`;
  if (tag === "img") return `<img${attrs} src="" alt="${escapeAttr(text || "Imagine")}">`;
  if (tag === "input") return `<input${attrs} type="text" placeholder="${escapeAttr(text || "Text")}">`;
  if (tag === "source") return `<source${attrs} src="" type="">`;
  if (tag === "video") return `<video${attrs} controls></video>`;
  if (tag === "audio") return `<audio${attrs} controls></audio>`;
  if (tag === "iframe") return `<iframe${attrs} src="" title="${escapeAttr(text || "Iframe")}"></iframe>`;
  if (tag === "picture") return `<picture${attrs}><img src="" alt="${escapeAttr(text || "Imagine")}"></picture>`;
  if (tag === "ul") return `<ul${attrs}><li>${textOr(text, "Element listă")}</li></ul>`;
  if (tag === "ol") return `<ol${attrs}><li>${textOr(text, "Element listă")}</li></ol>`;
  if (tag === "dl") return `<dl${attrs}><dt>${textOr(text, "Termen")}</dt><dd>Descriere</dd></dl>`;
  if (tag === "form") return `<form${attrs}><button type="submit">${textOr(text, "Trimite")}</button></form>`;
  if (tag === "textarea") return `<textarea${attrs} placeholder="${escapeAttr(text || "Text")}"></textarea>`;
  if (tag === "select") return `<select${attrs}><option>${textOr(text, "Opțiune")}</option></select>`;
  if (tag === "fieldset") return `<fieldset${attrs}><legend>${textOr(text, "Legendă")}</legend></fieldset>`;
  if (tag === "table") return `<table${attrs}><tbody><tr><td>${textOr(text, "Celulă")}</td></tr></tbody></table>`;
  if (tag === "thead") return `<thead${attrs}><tr><th>${textOr(text, "Titlu")}</th></tr></thead>`;
  if (tag === "tbody") return `<tbody${attrs}><tr><td>${textOr(text, "Celulă")}</td></tr></tbody>`;
  if (tag === "tfoot") return `<tfoot${attrs}><tr><td>${textOr(text, "Total")}</td></tr></tfoot>`;
  if (tag === "tr") return `<tr${attrs}><td>${textOr(text, "Celulă")}</td></tr>`;
  if (tag === "th") return `<th${attrs}>${textOr(text, "Titlu")}</th>`;
  if (tag === "td") return `<td${attrs}>${textOr(text, "Celulă")}</td>`;
  if (tag === "caption") return `<caption${attrs}>${textOr(text, "Descriere tabel")}</caption>`;

  if (htmlVoidSnippetTags.has(tag)) return `<${tag}${attrs}>`;
  return `<${tag}${attrs}>${text ? escapeText(text) : ""}</${tag}>`;
}
