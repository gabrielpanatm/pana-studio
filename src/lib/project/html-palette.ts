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
  div: { label: "Container", description: "Container neutru", text: "" },
  section: { label: "Secțiune", description: "Bloc semantic", text: "" },
  article: { label: "Articol", description: "Conținut independent", text: "Articol nou" },
  main: { label: "Conținut principal", description: "Zona principală a paginii", text: "" },
  header: { label: "Antet", description: "Antet", text: "" },
  footer: { label: "Subsol", description: "Subsol", text: "" },
  nav: { label: "Navigație", description: "Navigație", text: "" },
  aside: { label: "Conținut lateral", description: "Conținut lateral", text: "" },
  figure: { label: "Figură", description: "Media + descriere", text: "" },
  figcaption: { label: "Legendă figură", description: "Descriere media", text: "Descriere" },
  p: { label: "Paragraf", description: "Text de corp", text: "Paragraf nou." },
  h1: { label: "H1", description: "Titlu principal", text: "Titlu principal" },
  h2: { label: "H2", description: "Titlu secundar", text: "Titlu nou" },
  h3: { label: "H3", description: "Subtitlu", text: "Subtitlu nou" },
  h4: { label: "H4", description: "Titlu nivel 4", text: "Titlu nou" },
  h5: { label: "H5", description: "Titlu nivel 5", text: "Titlu nou" },
  h6: { label: "H6", description: "Titlu nivel 6", text: "Titlu nou" },
  span: { label: "Text în linie", description: "Text în linie", text: "Text" },
  blockquote: { label: "Citat", description: "Citat", text: "Citat nou." },
  pre: { label: "Text preformatat", description: "Text preformatat", text: "Text preformatat" },
  code: { label: "Cod", description: "Cod în linie", text: "cod" },
  strong: { label: "Evidențiere", description: "Accent puternic", text: "Text important" },
  em: { label: "Accent", description: "Accent italic", text: "Text accentuat" },
  small: { label: "Text secundar", description: "Text secundar", text: "Text mic" },
  label: { label: "Etichetă", description: "Etichetă formular", text: "Etichetă" },
  ul: { label: "Listă neordonată", description: "Listă neordonată", text: "Element listă" },
  ol: { label: "Listă ordonată", description: "Listă ordonată", text: "Element listă" },
  li: { label: "Element de listă", description: "Element listă", text: "Element listă" },
  dl: { label: "Listă de descrieri", description: "Listă descrieri", text: "Termen" },
  dt: { label: "Termen", description: "Termen", text: "Termen" },
  dd: { label: "Descriere termen", description: "Descriere termen", text: "Descriere" },
  img: { label: "Imagine", description: "Imagine", text: "Imagine" },
  video: { label: "Video", description: "Video cu comenzi", text: "" },
  audio: { label: "Audio", description: "Audio cu comenzi", text: "" },
  source: { label: "Sursă media", description: "Sursă media", text: "" },
  picture: { label: "Imagine adaptivă", description: "Imagine adaptivă", text: "Imagine" },
  iframe: { label: "Cadru încorporat", description: "Conținut încorporat", text: "Cadru" },
  a: { label: "Legătură", description: "Legătură", text: "Legătură nouă" },
  button: { label: "Buton", description: "Acțiune", text: "Buton nou", className: "btn" },
  form: { label: "Formular", description: "Formular", text: "Trimite" },
  input: { label: "Câmp text", description: "Câmp text", text: "Text" },
  textarea: { label: "Zonă de text", description: "Câmp lung", text: "Text" },
  select: { label: "Listă de opțiuni", description: "Listă opțiuni", text: "Opțiune" },
  option: { label: "Opțiune", description: "Opțiune din listă", text: "Opțiune" },
  fieldset: { label: "Grup de câmpuri", description: "Grup formular", text: "Legendă" },
  legend: { label: "Legendă", description: "Titlul grupului de câmpuri", text: "Legendă" },
  table: { label: "Tabel", description: "Tabel simplu", text: "Celulă" },
  thead: { label: "Antet tabel", description: "Antet tabel", text: "Titlu" },
  tbody: { label: "Corp tabel", description: "Corp tabel", text: "Celulă" },
  tfoot: { label: "Subsol tabel", description: "Subsol tabel", text: "Total" },
  tr: { label: "Rând tabel", description: "Rând tabel", text: "Celulă" },
  th: { label: "Celulă antet", description: "Celulă antet", text: "Titlu" },
  td: { label: "Celulă tabel", description: "Celulă tabel", text: "Celulă" },
  caption: { label: "Descriere tabel", description: "Descriere tabel", text: "Descriere tabel" },
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
