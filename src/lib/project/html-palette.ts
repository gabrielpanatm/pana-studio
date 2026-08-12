import { htmlTagGroups, type HtmlTagGroup } from "$lib/html/tag-catalog";
import { buildHtmlSnippet } from "$lib/html/snippets";
import { t } from "$lib/i18n/runtime.svelte";
import type { MessageId } from "$lib/i18n/generated/catalog";

export type HtmlPaletteElement = {
  id: string;
  kind?: "html" | "block";
  blockId?: string;
  blockKind?: "css" | "js" | "static";
  tag: string;
  label: string;
  description: string;
  text: string;
  className: string;
  html: string;
};

export type HtmlPaletteGroup = {
  label: string;
  elements: HtmlPaletteElement[];
};

export type HtmlPaletteInsertOptions = {
  tag: string;
  className: string;
  dataAnim?: string;
  text: string;
  html: string;
};

const tagMeta: Record<string, { labelId: MessageId; text: string; className?: string }> = {
  div: { labelId: "project-html-tag-div", text: "" },
  section: { labelId: "project-html-tag-section", text: "" },
  article: { labelId: "project-html-tag-article", text: "New article" },
  main: { labelId: "project-html-tag-main", text: "" },
  header: { labelId: "project-html-tag-header", text: "" },
  footer: { labelId: "project-html-tag-footer", text: "" },
  nav: { labelId: "project-html-tag-nav", text: "" },
  aside: { labelId: "project-html-tag-aside", text: "" },
  figure: { labelId: "project-html-tag-figure", text: "" },
  figcaption: { labelId: "project-html-tag-figcaption", text: "Description" },
  p: { labelId: "project-html-tag-p", text: "New paragraph." },
  h1: { labelId: "project-html-tag-h1", text: "Main heading" },
  h2: { labelId: "project-html-tag-h2", text: "New heading" },
  h3: { labelId: "project-html-tag-h3", text: "New subheading" },
  h4: { labelId: "project-html-tag-h4", text: "New heading" },
  h5: { labelId: "project-html-tag-h5", text: "New heading" },
  h6: { labelId: "project-html-tag-h6", text: "New heading" },
  span: { labelId: "project-html-tag-span", text: "Text" },
  blockquote: { labelId: "project-html-tag-blockquote", text: "New quote." },
  pre: { labelId: "project-html-tag-pre", text: "Preformatted text" },
  code: { labelId: "project-html-tag-code", text: "code" },
  strong: { labelId: "project-html-tag-strong", text: "Important text" },
  em: { labelId: "project-html-tag-em", text: "Emphasized text" },
  small: { labelId: "project-html-tag-small", text: "Small text" },
  label: { labelId: "project-html-tag-label", text: "Label" },
  ul: { labelId: "project-html-tag-ul", text: "List item" },
  ol: { labelId: "project-html-tag-ol", text: "List item" },
  li: { labelId: "project-html-tag-li", text: "List item" },
  dl: { labelId: "project-html-tag-dl", text: "Term" },
  dt: { labelId: "project-html-tag-dt", text: "Term" },
  dd: { labelId: "project-html-tag-dd", text: "Description" },
  img: { labelId: "project-html-tag-img", text: "Image" },
  video: { labelId: "project-html-tag-video", text: "" },
  audio: { labelId: "project-html-tag-audio", text: "" },
  source: { labelId: "project-html-tag-source", text: "" },
  picture: { labelId: "project-html-tag-picture", text: "Image" },
  iframe: { labelId: "project-html-tag-iframe", text: "Embedded frame" },
  a: { labelId: "project-html-tag-a", text: "New link" },
  button: { labelId: "project-html-tag-button", text: "New button", className: "btn" },
  form: { labelId: "project-html-tag-form", text: "Submit" },
  input: { labelId: "project-html-tag-input", text: "Text" },
  textarea: { labelId: "project-html-tag-textarea", text: "Text" },
  select: { labelId: "project-html-tag-select", text: "Option" },
  option: { labelId: "project-html-tag-option", text: "Option" },
  fieldset: { labelId: "project-html-tag-fieldset", text: "Legend" },
  legend: { labelId: "project-html-tag-legend", text: "Legend" },
  table: { labelId: "project-html-tag-table", text: "Cell" },
  thead: { labelId: "project-html-tag-thead", text: "Heading" },
  tbody: { labelId: "project-html-tag-tbody", text: "Cell" },
  tfoot: { labelId: "project-html-tag-tfoot", text: "Total" },
  tr: { labelId: "project-html-tag-tr", text: "Cell" },
  th: { labelId: "project-html-tag-th", text: "Heading" },
  td: { labelId: "project-html-tag-td", text: "Cell" },
  caption: { labelId: "project-html-tag-caption", text: "Table description" },
};

const groupMessageByFirstTag: Record<string, MessageId> = {
  div: "project-html-group-structure",
  p: "project-html-group-text",
  ul: "project-html-group-lists",
  img: "project-html-group-media",
  a: "project-html-group-actions",
  form: "project-html-group-forms",
  table: "project-html-group-tables",
};

function paletteElementForTag(tag: string): HtmlPaletteElement {
  const meta = tagMeta[tag] ?? {
    labelId: "project-html-tag-generic" as MessageId,
    text: "",
  };
  const className = meta.className ?? "";
  const text = meta.text;
  return {
    id: tag,
    tag,
    label: meta.labelId === "project-html-tag-generic"
      ? t(meta.labelId, { tag: tag.toUpperCase() })
      : t(meta.labelId),
    description: t("project-html-element-description", { tag }),
    text,
    className,
    html: buildHtmlSnippet({ tag, className, text }),
  };
}

function paletteGroupFor(group: HtmlTagGroup): HtmlPaletteGroup {
  return {
    label: t(groupMessageByFirstTag[group.tags[0]] ?? "project-html-group-other"),
    elements: group.tags.map(paletteElementForTag),
  };
}

export function htmlPaletteGroups(): HtmlPaletteGroup[] {
  return htmlTagGroups.map(paletteGroupFor);
}

export function htmlPaletteElements(): HtmlPaletteElement[] {
  return htmlPaletteGroups().flatMap((group) => group.elements);
}

function joinClassNames(...tokens: Array<string | undefined>) {
  return Array.from(new Set(tokens.flatMap((token) => token?.split(/\s+/).map((part) => part.trim()).filter(Boolean) ?? []))).join(" ");
}

export function htmlPaletteInsertOptions(
  element: HtmlPaletteElement,
  identity?: { className?: string; dataAnim?: string; blockInstanceId?: string },
): HtmlPaletteInsertOptions {
  const className = joinClassNames(element.className, identity?.className);
  if (element.kind === "block" && element.html) {
    return {
      tag: element.tag,
      className,
      dataAnim: identity?.dataAnim,
      text: element.text,
      html: element.html
        .replaceAll("__PANA_CLASS__", identity?.className ?? "")
        .replaceAll("__PANA_DATA_ANIM__", identity?.dataAnim ?? "")
        .replaceAll("__PANA_INSTANCE__", identity?.blockInstanceId ?? identity?.dataAnim ?? ""),
    };
  }

  return {
    tag: element.tag,
    className,
    dataAnim: identity?.dataAnim,
    text: element.text,
    html: buildHtmlSnippet({ tag: element.tag, className, dataAnim: identity?.dataAnim, text: element.text }),
  };
}
