import type { SelectionInfo, SourceNodeRange } from "$lib/types";

function escapeCssIdentifier(value: string) {
  return value.replace(/[^a-zA-Z0-9_-]/g, "\\$&");
}

export function createCssSelector(tag: string, id: string, classes: string[]) {
  if (id) {
    return `#${escapeCssIdentifier(id)}`;
  }

  if (classes.length > 0) {
    return `${tag}${classes.map((className) => `.${escapeCssIdentifier(className)}`).join("")}`;
  }

  return tag;
}

export function parseHtmlSourceNodes(sourceText: string, htmlVoidTags: ReadonlySet<string>): SourceNodeRange[] {
  const tokenRegex = /<!--[\s\S]*?-->|<!DOCTYPE[\s\S]*?>|<\/?([A-Za-z][A-Za-z0-9:-]*)([^>]*)>/g;
  const nodes: SourceNodeRange[] = [];
  const stack: Array<{
    tag: string;
    selector: string;
    cssSelector: string;
    openStart: number;
    openEnd: number;
    counts: Map<string, number>;
  }> = [];
  const rootCounts = new Map<string, number>();

  for (const match of sourceText.matchAll(tokenRegex)) {
    const token = match[0];
    const matchStart = match.index ?? 0;
    const matchEnd = matchStart + token.length;

    if (token.startsWith("<!--") || /^<!DOCTYPE/i.test(token)) {
      continue;
    }

    const tag = (match[1] ?? "").toLowerCase();
    if (!tag) {
      continue;
    }

    const closing = token.startsWith("</");

    if (closing) {
      for (let index = stack.length - 1; index >= 0; index -= 1) {
        const entry = stack[index];

        if (entry.tag !== tag) {
          continue;
        }

        nodes.push({
          selector: entry.selector,
          cssSelector: entry.cssSelector,
          tag: entry.tag,
          openStart: entry.openStart,
          openEnd: entry.openEnd,
          end: matchEnd,
        });
        stack.splice(index, 1);
        break;
      }

      continue;
    }

    const attrSource = match[2] ?? "";
    const idMatch =
      attrSource.match(/\bid\s*=\s*"([^"]+)"/i) ??
      attrSource.match(/\bid\s*=\s*'([^']+)'/i) ??
      attrSource.match(/\bid\s*=\s*([^\s"'=<>`]+)/i);
    const id = idMatch?.[1] ?? "";
    const classMatch =
      attrSource.match(/\bclass\s*=\s*"([^"]+)"/i) ??
      attrSource.match(/\bclass\s*=\s*'([^']+)'/i) ??
      attrSource.match(/\bclass\s*=\s*([^\s"'=<>`]+)/i);
    const classes = classMatch?.[1]?.split(/\s+/).filter(Boolean) ?? [];

    const parent = stack[stack.length - 1];
    const counts = parent?.counts ?? rootCounts;
    const nextIndex = (counts.get(tag) ?? 0) + 1;
    counts.set(tag, nextIndex);

    const selector = id
      ? `${tag}#${escapeCssIdentifier(id)}`
      : parent
        ? `${parent.selector} > ${tag}:nth-of-type(${nextIndex})`
        : `${tag}:nth-of-type(${nextIndex})`;
    const cssSelector = createCssSelector(tag, id, classes);
    const selfClosing = /\/>$/.test(token) || htmlVoidTags.has(tag);

    if (selfClosing) {
      nodes.push({
        selector,
        cssSelector,
        tag,
        openStart: matchStart,
        openEnd: matchEnd,
        end: matchEnd,
      });
      continue;
    }

    stack.push({
      tag,
      selector,
      cssSelector,
      openStart: matchStart,
      openEnd: matchEnd,
      counts: new Map<string, number>(),
    });
  }

  for (const entry of stack) {
    nodes.push({
      selector: entry.selector,
      cssSelector: entry.cssSelector,
      tag: entry.tag,
      openStart: entry.openStart,
      openEnd: entry.openEnd,
      end: sourceText.length,
    });
  }

  return nodes.sort((left, right) => left.openStart - right.openStart);
}

export function findHtmlNodeAtPosition(nodes: SourceNodeRange[], position: number) {
  let bestMatch: SourceNodeRange | null = null;

  for (const node of nodes) {
    if (position < node.openStart || position > node.end) {
      continue;
    }

    if (!bestMatch || node.openStart >= bestMatch.openStart) {
      bestMatch = node;
    }
  }

  return bestMatch;
}

export function findHtmlNodeForSelection(nodes: SourceNodeRange[], selection: SelectionInfo | null) {
  if (!selection) {
    return null;
  }

  return (
    nodes.find((node) => node.selector === selection.domPath) ??
    nodes.find((node) => node.cssSelector === selection.cssSelector && node.tag === selection.tag) ??
    null
  );
}
