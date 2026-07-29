import { createCssSelector } from "$lib/html/parser";
import { t } from "$lib/i18n/runtime.svelte";
import type { MessageId } from "$lib/i18n/generated/catalog";
import type {
  DomNodeLink,
  EditableAttributes,
  PageSection,
  CanvasElementObservation,
} from "$lib/types";

export type MarkdownSelectionTarget = {
  kind: "heading" | "link" | "text";
  text: string;
  href?: string;
};

function normalizeText(text: string | null) {
  const compact = text?.replace(/\s+/g, " ").trim();
  return compact && compact.length > 0 ? compact : t("preview-selection-no-text");
}

function escapeCssIdentifier(value: string) {
  return value.replace(/[^A-Za-z0-9_-]/g, (character) => `\\${character}`);
}

const SEMANTIC_TAG_LABEL_IDS: Record<string, MessageId> = {
  main: "project-html-tag-main",
  section: "project-html-tag-section",
  article: "project-html-tag-article",
  header: "project-html-tag-header",
  footer: "project-html-tag-footer",
  nav: "project-html-tag-nav",
  aside: "project-html-tag-aside",
  div: "project-html-tag-div",
  figure: "project-html-tag-figure",
  figcaption: "project-html-tag-figcaption",
  ul: "project-html-tag-ul",
  ol: "project-html-tag-ol",
  li: "project-html-tag-li",
  form: "project-html-tag-form",
  fieldset: "project-html-tag-fieldset",
  table: "project-html-tag-table",
  thead: "project-html-tag-thead",
  tbody: "project-html-tag-tbody",
  tr: "project-html-tag-tr",
  th: "project-html-tag-th",
  td: "project-html-tag-td",
  img: "project-html-tag-img",
  video: "project-html-tag-video",
  audio: "project-html-tag-audio",
  iframe: "project-html-tag-iframe",
  a: "project-html-tag-a",
  button: "project-html-tag-button",
  p: "project-html-tag-p",
  span: "project-html-tag-span",
  small: "project-html-tag-small",
  strong: "project-html-tag-strong",
  em: "project-html-tag-em",
  blockquote: "project-html-tag-blockquote",
  code: "project-html-tag-code",
  pre: "project-html-tag-pre",
  label: "project-html-tag-label",
};

const FULL_TEXT_LABEL_TAGS = new Set([
  "h1", "h2", "h3", "h4", "h5", "h6",
  "p", "a", "button", "span", "small", "strong", "em",
  "blockquote", "figcaption", "label", "code", "pre",
  "li", "th", "td", "caption",
]);
const GENERATED_CLASS_RE = /^(ps|pana)-/;
const UTILITY_CLASS_RE = /^(container|section|row|col|grid|flex|btn|button|active|open|hidden|show|cont-\d+)/;

function semanticTagLabel(tag: string) {
  const id = SEMANTIC_TAG_LABEL_IDS[tag];
  return id ? t(id) : tag;
}

function shortenLabel(text: string | null | undefined) {
  const compact = text?.replace(/\s+/g, " ").trim() ?? "";
  if (!compact) return "";
  return compact.length > 56 ? `${compact.slice(0, 53).trimEnd()}...` : compact;
}

function directTextFor(element: Element) {
  const chunks = Array.from(element.childNodes)
    .filter((node) => node.nodeType === Node.TEXT_NODE)
    .map((node) => shortenLabel(node.nodeValue))
    .filter(Boolean);
  return shortenLabel(chunks.join(" "));
}

function firstDirectHeadingLabelFor(element: Element) {
  const heading = Array.from(element.children).find((child) =>
    /^h[1-6]$/i.test(child.tagName),
  );
  return heading ? shortenLabel(heading.textContent) : "";
}

function firstListItemLabelFor(element: Element) {
  return shortenLabel(element.querySelector(":scope > li")?.textContent);
}

function mediaFileName(value: string | null) {
  if (!value) return "";
  const clean = String(value).split("?")[0].split("#")[0];
  const parts = clean.split("/");
  return decodeURIComponent(parts[parts.length - 1] || clean);
}

function readableClassFor(element: Element) {
  return Array.from(element.classList).find((className) =>
    !RUNTIME_CLASSES.has(className)
      && !GENERATED_CLASS_RE.test(className)
      && !UTILITY_CLASS_RE.test(className)
  ) ?? "";
}

function isDisplayClass(className: string) {
  return !RUNTIME_CLASSES.has(className) && !GENERATED_CLASS_RE.test(className);
}

export function domNodeLabelFor(element: Element) {
  const tag = element.tagName.toLowerCase();

  if (element.hasAttribute("data-pana-empty-tera-slot")) {
    return element.getAttribute("data-pana-empty-label") ?? semanticTagLabel(tag);
  }

  const ariaLabel = shortenLabel(element.getAttribute("aria-label"));
  if (ariaLabel) return ariaLabel;

  const title = shortenLabel(element.getAttribute("title"));
  if (title) return title;

  const ownText = directTextFor(element);

  if (FULL_TEXT_LABEL_TAGS.has(tag)) {
    const fullText = shortenLabel(element.textContent);
    if (fullText) return fullText;
  }

  if (tag === "img") {
    const alt = shortenLabel(element.getAttribute("alt"));
    if (alt) return `Imagine: ${alt}`;
    const src = mediaFileName(element.getAttribute("src"));
    return src ? `Imagine: ${src}` : "Imagine";
  }

  if (tag === "video" || tag === "audio" || tag === "iframe" || tag === "source") {
    const src = mediaFileName(element.getAttribute("src"));
    return src ? `${semanticTagLabel(tag)}: ${src}` : semanticTagLabel(tag);
  }

  if (ownText) return ownText;

  if (tag === "ul" || tag === "ol") {
    const itemLabel = firstListItemLabelFor(element);
    if (itemLabel) return `${semanticTagLabel(tag)}: ${itemLabel}`;
  }

  const headingText = firstDirectHeadingLabelFor(element);
  if (headingText) return `${semanticTagLabel(tag)}: ${headingText}`;

  if (element.id) {
    return `#${element.id}`;
  }

  const firstClass = readableClassFor(element);
  if (firstClass) {
    return `.${firstClass}`;
  }

  return semanticTagLabel(tag);
}

export function createDomPathSelector(element: Element) {
  const segments: string[] = [];
  let current: Element | null = element;

  while (current && current.tagName.toLowerCase() !== "html") {
    const tag = current.tagName.toLowerCase();

    if (current.id) {
      segments.unshift(`${tag}#${escapeCssIdentifier(current.id)}`);
      break;
    }

    const parent: Element | null = current.parentElement;

    if (!parent) {
      segments.unshift(tag);
      break;
    }

    const siblings = Array.from(parent.children).filter(
      (sibling: Element) => sibling.tagName.toLowerCase() === tag,
    );
    const index = siblings.indexOf(current) + 1;
    segments.unshift(`${tag}:nth-of-type(${index})`);
    current = parent;
  }

  return segments.join(" > ");
}

const SESSION_ID_ATTR = "data-pana-session-id";
const SOURCE_ID_ATTR = "data-pana-source-id";
const TEMPLATE_SOURCE_ID_ATTR = "data-pana-template-source-id";
const TEMPLATE_SOURCE_STACK_ATTR = "data-pana-template-source-stack";
const SKIP_ATTRS = new Set([
  "class",
  "style",
  SOURCE_ID_ATTR,
  TEMPLATE_SOURCE_ID_ATTR,
  TEMPLATE_SOURCE_STACK_ATTR,
  "data-pana-preview-revision",
  SESSION_ID_ATTR,
  "data-pana-empty-tera-slot",
  "data-pana-empty-html",
  "data-pana-empty-label",
]);
const RUNTIME_CLASSES = new Set([
  "pana-studio-empty-editable",
  "pana-studio-empty-tera-slot",
]);

function collectElementAttributes(element: Element): Record<string, string> {
  const result: Record<string, string> = {};
  for (const attr of Array.from(element.attributes)) {
    // `data-pana-*` belongs exclusively to the Preview runtime. Filtering the
    // complete namespace prevents newly introduced runtime identities from
    // leaking into an editable ProjectWorkspace draft.
    if (attr.name.startsWith("data-pana-") || SKIP_ATTRS.has(attr.name)) continue;
    result[attr.name] = attr.value;
  }
  return result;
}

export function createDomNodeLink(element: Element): DomNodeLink {
  return {
    selector: createDomPathSelector(element),
    label: domNodeLabelFor(element),
    tag: element.tagName.toLowerCase(),
  };
}

export function formatElementSelector(tag: string, id: string, classes: string[]) {
  const idPart = id ? ` id="${id}"` : "";
  const realClasses = classes.filter(isDisplayClass);
  const classPart = realClasses.length > 0 ? ` class="${realClasses.join(" ")}"` : "";

  return `<${tag}${idPart}${classPart}>`;
}

function inheritedTemplateSourceId(element: Element): string | null {
  let current: Element | null = element;
  while (current && current.tagName.toLowerCase() !== "html") {
    const sourceId = current.getAttribute(TEMPLATE_SOURCE_ID_ATTR);
    if (sourceId) return sourceId;
    current = current.parentElement;
  }
  return null;
}

function assignTemplateSourceStack(element: Element, stack: string[]) {
  if (stack.length === 0) return;
  element.setAttribute(TEMPLATE_SOURCE_ID_ATTR, stack[stack.length - 1]);
  element.setAttribute(TEMPLATE_SOURCE_STACK_ATTR, stack.join(" "));
}

export function summarizeElementText(text: string | null) {
  const normalized = text?.replace(/\s+/g, " ").trim() ?? "";

  if (normalized.length <= 90) {
    return normalized || t("preview-selection-no-text");
  }

  return `${normalized.slice(0, 87)}...`;
}

function sectionDepthFor(element: Element) {
  let depth = 0;
  let current = element.parentElement;

  while (current && current.tagName.toLowerCase() !== "body") {
    if (current.matches("main, section, article, header, footer, nav, aside")) {
      depth += 1;
    }
    current = current.parentElement;
  }

  return depth;
}

export function collectPageSections(document: Document): PageSection[] {
  applyTemplateSourceIdsFromMarkers(document);
  const semanticNodes = Array.from(
    document.querySelectorAll("main, section, article, header, footer, nav, aside"),
  );
  const fallbackNodes =
    semanticNodes.length > 0
      ? semanticNodes
      : Array.from(document.body?.children ?? []).filter((child) => child instanceof Element && !isEmptyTeraSlot(child));

  return fallbackNodes
    .filter((element) => !isEmptyTeraSlot(element))
    .map((element) => ({
      selector: createDomPathSelector(element),
      label: domNodeLabelFor(element),
      tag: element.tagName.toLowerCase(),
      depth: sectionDepthFor(element),
      sourceLocation: null,
      sourceId: element.getAttribute(SOURCE_ID_ATTR),
      templateSourceId: inheritedTemplateSourceId(element),
      sessionId: element.getAttribute(SESSION_ID_ATTR),
    }))
    .filter((section, index, array) => array.findIndex((item) => item.selector === section.selector) === index);
}

const SKIP_TAGS = new Set([
  "script", "style", "noscript", "meta", "link", "head",
  "br", "hr", "wbr", "input", "textarea", "select",
]);
const SVG_TAGS = new Set(["svg", "path", "g", "defs", "use", "circle", "rect", "polygon", "polyline", "line", "text", "tspan"]);
const STUDIO_OVERLAY_IDS = new Set([
  "pana-studio-preview-drop-line",
  "pana-studio-preview-drop-box",
  "pana-studio-preview-drop-hint",
]);
const MAX_TREE_DEPTH = 9;
const MAX_TREE_NODES = 300;

function isEmptyTeraSlot(element: Element) {
  return element.hasAttribute("data-pana-empty-tera-slot");
}

export function collectDomTree(document: Document): PageSection[] {
  applyTemplateSourceIdsFromMarkers(document);
  const result: PageSection[] = [];

  function traverse(element: Element, depth: number) {
    if (result.length >= MAX_TREE_NODES) return;
    if (depth > MAX_TREE_DEPTH) return;
    const tag = element.tagName.toLowerCase();
    if (SKIP_TAGS.has(tag) || SVG_TAGS.has(tag)) return;
    if (STUDIO_OVERLAY_IDS.has(element.id)) return;
    if (isEmptyTeraSlot(element)) return;

    result.push({
      selector: createDomPathSelector(element),
      label: domNodeLabelFor(element),
      tag,
      depth,
      sourceLocation: null,
      sourceId: element.getAttribute(SOURCE_ID_ATTR),
      templateSourceId: inheritedTemplateSourceId(element),
      sessionId: element.getAttribute(SESSION_ID_ATTR),
    });

    for (const child of Array.from(element.children)) {
      traverse(child, depth + 1);
    }
  }

  if (document.body) {
    for (const child of Array.from(document.body.children)) {
      traverse(child, 0);
    }
  }

  return result;
}

function templateSourceMarker(text: string | null) {
  const match = String(text ?? "").match(/^\s*pana-template-source-(start|end):([A-Za-z0-9_-]+)\s*$/);
  return match ? { kind: match[1] as "start" | "end", id: match[2] } : null;
}

function applyTemplateSourceIdsFromMarkers(document: Document) {
  if (!document.body) return;
  const walker = document.createTreeWalker(
    document.body,
    NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_COMMENT,
  );
  const stack: string[] = [];
  let node = walker.nextNode();

  while (node) {
    if (node.nodeType === Node.COMMENT_NODE) {
      const marker = templateSourceMarker(node.nodeValue);
      if (marker?.kind === "start") {
        stack.push(marker.id);
      } else if (marker?.kind === "end") {
        const index = stack.lastIndexOf(marker.id);
        if (index >= 0) stack.splice(index, 1);
      }
    } else if (node instanceof Element && stack.length > 0) {
      assignTemplateSourceStack(node, stack);
    }
    node = walker.nextNode();
  }
}

function normalizeSearchText(text: string | null) {
  return text?.replace(/\s+/g, " ").trim() ?? "";
}

function stripMarkdownInline(value: string) {
  return value
    .replace(/!\[([^\]]*)\]\([^)]+\)/g, "$1")
    .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/(\*\*|__)(.*?)\1/g, "$2")
    .replace(/(\*|_)(.*?)\1/g, "$2")
    .replace(/~~(.*?)~~/g, "$1")
    .replace(/<[^>]+>/g, " ")
    .trim();
}

export function markdownTargetAtPosition(sourceText: string, position: number): MarkdownSelectionTarget | null {
  const before = sourceText.slice(0, position);
  const lineStart = before.lastIndexOf("\n") + 1;
  const lineEndIndex = sourceText.indexOf("\n", position);
  const lineEnd = lineEndIndex === -1 ? sourceText.length : lineEndIndex;
  const line = sourceText.slice(lineStart, lineEnd).trim();

  if (!line || line.startsWith("```")) {
    return null;
  }

  const headingMatch = line.match(/^#{1,6}\s+(.+)$/);
  if (headingMatch) {
    const text = normalizeSearchText(stripMarkdownInline(headingMatch[1]));
    return text ? { kind: "heading", text } : null;
  }

  const linkMatch = line.match(/\[([^\]]+)\]\(([^)]+)\)/);
  if (linkMatch) {
    const text = normalizeSearchText(stripMarkdownInline(linkMatch[1]));
    const href = linkMatch[2].trim();
    if (text) {
      return { kind: "link", text, href };
    }
  }

  const text = normalizeSearchText(
    stripMarkdownInline(
      line
        .replace(/^[-*+]\s+/, "")
        .replace(/^\d+\.\s+/, "")
        .replace(/^>\s+/, ""),
    ),
  );

  if (!text) {
    return null;
  }

  return { kind: "text", text };
}

export function findPreviewElementForMarkdownTarget(document: Document, target: MarkdownSelectionTarget) {
  const normalizedTarget = normalizeSearchText(target.text);

  if (target.kind === "heading") {
    return (
      Array.from(document.querySelectorAll("h1, h2, h3, h4, h5, h6")).find(
        (element) => normalizeSearchText(element.textContent) === normalizedTarget,
      ) ?? null
    );
  }

  if (target.kind === "link") {
    return (
      Array.from(document.querySelectorAll("a")).find((element) => {
        const href = element.getAttribute("href")?.trim() ?? "";
        return normalizeSearchText(element.textContent) === normalizedTarget && (!target.href || href === target.href);
      }) ?? null
    );
  }

  return (
    Array.from(document.querySelectorAll("p, li, blockquote, figcaption, span, div")).find((element) => {
      const content = normalizeSearchText(element.textContent);
      return content.length > 0 && content.includes(normalizedTarget);
    }) ?? null
  );
}

type SelectionEditorStateOptions = {
  variableOverrides: Record<string, string>;
  canEditHtmlSource: boolean;
  canEditSemanticSource: boolean;
  blockedReason: string;
};

export function deriveSelectionEditorState(selection: CanvasElementObservation, options: SelectionEditorStateOptions) {
  const variableValues: Record<string, string> = {};

  for (const variable of selection.variables) {
    variableValues[variable.name] = options.variableOverrides[variable.name] ?? variable.value;
  }

  const attributeValues: EditableAttributes = { ...selection.attributes };

  const canEdit = options.canEditHtmlSource || options.canEditSemanticSource;
  const editViaTemplate = !options.canEditHtmlSource && options.canEditSemanticSource;
  const templateLabel = editViaTemplate ? t("preview-selection-template-suffix") : "";

  const classStatus = canEdit
    ? t("preview-selection-classes-editable", { context: templateLabel })
    : options.blockedReason;
  const imageStatus = selection.imageSrc
    ? canEdit
      ? t("preview-selection-image-editable", { context: templateLabel })
      : options.blockedReason
    : t("preview-selection-no-image-source");
  const attributeStatus = canEdit
    ? t("preview-selection-attributes-editable", { context: templateLabel })
    : options.blockedReason;
  const textStatus = canEdit
    ? selection.hasChildElements
      ? t("preview-selection-text-children-blocked")
      : t("preview-selection-text-editable", { context: templateLabel })
    : options.blockedReason;

  return {
    classEditorValue: selection.classes.join(" "),
    imageSourceValue: selection.imageSrc ?? "",
    attributeValues,
    textContentValue: selection.rawText,
    variableValues,
    classStatus,
    imageStatus,
    attributeStatus,
    textStatus,
  };
}
