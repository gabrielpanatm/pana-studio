import { buildHtmlSnippet } from "$lib/html/snippets";

export type HtmlPaletteElement = {
  id: string;
  kind: "html" | "block";
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
