import { htmlTagGroups, type HtmlTagGroup } from "$lib/html/tag-catalog";
import { buildHtmlSnippet } from "$lib/html/snippets";

export type HtmlPaletteElement = {
  id: string;
  kind?: "html" | "component";
  componentId?: string;
  componentKind?: "css" | "js";
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

const tagMeta: Record<string, { label: string; description: string; text: string; className?: string }> = {
  div: { label: "Div", description: "Container neutru", text: "" },
  section: { label: "Section", description: "Bloc semantic", text: "" },
  article: { label: "Article", description: "Conținut independent", text: "Articol nou" },
  main: { label: "Main", description: "Conținut principal", text: "" },
  header: { label: "Header", description: "Antet", text: "" },
  footer: { label: "Footer", description: "Subsol", text: "" },
  nav: { label: "Nav", description: "Navigație", text: "" },
  aside: { label: "Aside", description: "Conținut lateral", text: "" },
  figure: { label: "Figure", description: "Media + descriere", text: "" },
  figcaption: { label: "Figcaption", description: "Descriere media", text: "Descriere" },
  p: { label: "Paragraph", description: "Text de corp", text: "Paragraf nou." },
  h1: { label: "H1", description: "Titlu principal", text: "Titlu principal" },
  h2: { label: "H2", description: "Titlu secundar", text: "Titlu nou" },
  h3: { label: "H3", description: "Subtitlu", text: "Subtitlu nou" },
  h4: { label: "H4", description: "Heading nivel 4", text: "Heading nou" },
  h5: { label: "H5", description: "Heading nivel 5", text: "Heading nou" },
  h6: { label: "H6", description: "Heading nivel 6", text: "Heading nou" },
  span: { label: "Span", description: "Text inline", text: "Text" },
  blockquote: { label: "Quote", description: "Citat", text: "Citat nou." },
  pre: { label: "Pre", description: "Text preformatat", text: "Text preformatat" },
  code: { label: "Code", description: "Cod inline", text: "cod" },
  strong: { label: "Strong", description: "Accent puternic", text: "Text important" },
  em: { label: "Em", description: "Accent italic", text: "Text accentuat" },
  small: { label: "Small", description: "Text secundar", text: "Text mic" },
  label: { label: "Label", description: "Etichetă formular", text: "Etichetă" },
  ul: { label: "UL", description: "Listă neordonată", text: "Element listă" },
  ol: { label: "OL", description: "Listă ordonată", text: "Element listă" },
  li: { label: "LI", description: "Element listă", text: "Element listă" },
  dl: { label: "DL", description: "Listă descrieri", text: "Termen" },
  dt: { label: "DT", description: "Termen", text: "Termen" },
  dd: { label: "DD", description: "Descriere termen", text: "Descriere" },
  img: { label: "Image", description: "Imagine", text: "Imagine" },
  video: { label: "Video", description: "Video cu controls", text: "" },
  audio: { label: "Audio", description: "Audio cu controls", text: "" },
  source: { label: "Source", description: "Sursă media", text: "" },
  picture: { label: "Picture", description: "Imagine responsive", text: "Imagine" },
  iframe: { label: "Iframe", description: "Conținut embedded", text: "Iframe" },
  a: { label: "Link", description: "Legătură", text: "Link nou" },
  button: { label: "Button", description: "Acțiune", text: "Buton nou", className: "btn" },
  form: { label: "Form", description: "Formular", text: "Trimite" },
  input: { label: "Input", description: "Câmp text", text: "Text" },
  textarea: { label: "Textarea", description: "Câmp lung", text: "Text" },
  select: { label: "Select", description: "Listă opțiuni", text: "Opțiune" },
  option: { label: "Option", description: "Opțiune select", text: "Opțiune" },
  fieldset: { label: "Fieldset", description: "Grup formular", text: "Legendă" },
  legend: { label: "Legend", description: "Titlu fieldset", text: "Legendă" },
  table: { label: "Table", description: "Tabel simplu", text: "Celulă" },
  thead: { label: "Thead", description: "Antet tabel", text: "Titlu" },
  tbody: { label: "Tbody", description: "Corp tabel", text: "Celulă" },
  tfoot: { label: "Tfoot", description: "Subsol tabel", text: "Total" },
  tr: { label: "TR", description: "Rând tabel", text: "Celulă" },
  th: { label: "TH", description: "Celulă antet", text: "Titlu" },
  td: { label: "TD", description: "Celulă tabel", text: "Celulă" },
  caption: { label: "Caption", description: "Descriere tabel", text: "Descriere tabel" },
};

function paletteElementForTag(tag: string): HtmlPaletteElement {
  const meta = tagMeta[tag] ?? {
    label: tag.toUpperCase(),
    description: "Element HTML",
    text: "",
  };
  const className = meta.className ?? "";
  const text = meta.text;
  return {
    id: tag,
    tag,
    label: meta.label,
    description: meta.description,
    text,
    className,
    html: buildHtmlSnippet({ tag, className, text }),
  };
}

function paletteGroupFor(group: HtmlTagGroup): HtmlPaletteGroup {
  return {
    label: group.label,
    elements: group.tags.map(paletteElementForTag),
  };
}

export const htmlPaletteGroups: HtmlPaletteGroup[] = htmlTagGroups.map(paletteGroupFor);
export const htmlPaletteElements: HtmlPaletteElement[] = htmlPaletteGroups.flatMap((group) => group.elements);

function joinClassNames(...tokens: Array<string | undefined>) {
  return Array.from(new Set(tokens.flatMap((token) => token?.split(/\s+/).map((part) => part.trim()).filter(Boolean) ?? []))).join(" ");
}

export function htmlPaletteInsertOptions(
  element: HtmlPaletteElement,
  identity?: { className?: string; dataAnim?: string; componentInstanceId?: string },
): HtmlPaletteInsertOptions {
  const className = joinClassNames(element.className, identity?.className);
  if (element.kind === "component" && element.html) {
    return {
      tag: element.tag,
      className,
      dataAnim: identity?.dataAnim,
      text: element.text,
      html: element.html
        .replaceAll("__PANA_CLASS__", identity?.className ?? "")
        .replaceAll("__PANA_DATA_ANIM__", identity?.dataAnim ?? "")
        .replaceAll("__PANA_INSTANCE__", identity?.componentInstanceId ?? identity?.dataAnim ?? ""),
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
