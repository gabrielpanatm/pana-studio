import { collectMatchedCssRules, collectRelevantCssVariables, formatBox } from "$lib/css/matcher";
import {
  createCssSelector,
  findHtmlNodeAtPosition,
  findHtmlNodeForSelection,
  parseHtmlSourceNodes,
} from "$lib/html/parser";
import { zolaImagePresentationFromElement } from "$lib/html/zola-image";
import type {
  DomNodeLink,
  EditableAttributes,
  PageSection,
  SelectionInfo,
  SourceLanguage,
  SourceNodeRange,
} from "$lib/types";

export type MarkdownSelectionTarget = {
  kind: "heading" | "link" | "text";
  text: string;
  href?: string;
};

function normalizeText(text: string | null) {
  const compact = text?.replace(/\s+/g, " ").trim();
  return compact && compact.length > 0 ? compact : "Fara text";
}

function escapeCssIdentifier(value: string) {
  return value.replace(/[^A-Za-z0-9_-]/g, (character) => `\\${character}`);
}

const SEMANTIC_TAG_LABELS: Record<string, string> = {
  main: "Conținut principal",
  section: "Secțiune",
  article: "Articol",
  header: "Antet",
  footer: "Subsol",
  nav: "Navigație",
  aside: "Conținut lateral",
  div: "Container",
  figure: "Figură",
  figcaption: "Legendă",
  ul: "Listă",
  ol: "Listă ordonată",
  li: "Element listă",
  form: "Formular",
  fieldset: "Grup formular",
  table: "Tabel",
  thead: "Antet tabel",
  tbody: "Corp tabel",
  tr: "Rând tabel",
  th: "Celulă antet",
  td: "Celulă tabel",
  img: "Imagine",
  video: "Video",
  audio: "Audio",
  iframe: "Iframe",
  a: "Link",
  button: "Buton",
  p: "Paragraf",
  span: "Text",
  small: "Text mic",
  strong: "Text important",
  em: "Text accentuat",
  blockquote: "Citat",
  code: "Cod",
  pre: "Text preformatat",
  label: "Etichetă",
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
  return SEMANTIC_TAG_LABELS[tag] ?? tag;
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
  "pana-studio-selected-element",
  "pana-studio-selected-template-source",
  "pana-studio-empty-editable",
  "pana-studio-empty-tera-slot",
]);

function collectElementAttributes(element: Element, selectedClass: string): Record<string, string> {
  const result: Record<string, string> = {};
  for (const attr of Array.from(element.attributes)) {
    // `data-pana-*` belongs exclusively to the Preview runtime. Filtering the
    // complete namespace prevents newly introduced runtime identities from
    // leaking into an editable ProjectWorkspace draft.
    if (attr.name.startsWith("data-pana-") || SKIP_ATTRS.has(attr.name)) continue;
    // Remove the selection marker class from id if it matches
    if (attr.name === "id" && attr.value === selectedClass) continue;
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
    return normalized || "Fara text";
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
  "pana-studio-html-selection",
  "pana-studio-preview-hover",
  "pana-studio-template-gate",
  "pana-studio-template-gate-actions",
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

export function findSourceElementForSelection(
  document: Document,
  selection: SelectionInfo | null,
  preferredSelector: string | null = null,
) {
  return (
    (preferredSelector ? document.querySelector(preferredSelector) : null) ??
    (selection
      ? document.querySelector(selection.domPath) ?? document.querySelector(selection.cssSelector)
      : null) ??
    document.body
  );
}

export type CodeCursorSelectionAction =
  | {
      type: "none";
    }
  | {
      type: "select-html-node";
      selector: string;
    }
  | {
      type: "select-markdown-target";
      target: MarkdownSelectionTarget;
    };

export function resolveCodeCursorSelectionAction(options: {
  sourceLanguage: SourceLanguage;
  sourceText: string;
  position: number;
  selectedElement: SelectionInfo | null;
  activeScannedPath: string | null;
  htmlVoidTags: ReadonlySet<string>;
}): CodeCursorSelectionAction {
  if (options.sourceLanguage === "html") {
    const node = findHtmlNodeAtPosition(
      parseHtmlSourceNodes(options.sourceText, options.htmlVoidTags),
      options.position,
    );

    if (!node || options.selectedElement?.domPath === node.selector) {
      return { type: "none" };
    }

    return { type: "select-html-node", selector: node.selector };
  }

  if (!options.activeScannedPath?.endsWith(".md")) {
    return { type: "none" };
  }

  const target = markdownTargetAtPosition(options.sourceText, options.position);

  return target ? { type: "select-markdown-target", target } : { type: "none" };
}

export function codeSelectionRangeForSelection(
  sourceLanguage: SourceLanguage,
  nodes: SourceNodeRange[],
  selection: SelectionInfo | null,
) {
  const node = sourceLanguage === "html" ? findHtmlNodeForSelection(nodes, selection) : null;
  return node ? { from: node.openStart, to: node.openEnd } : null;
}

export function resolveHtmlSourceSelectionContext(options: {
  sourceText: string;
  cursorPosition: number;
  selectedElement: SelectionInfo | null;
  htmlVoidTags: ReadonlySet<string>;
}) {
  const parsedDocument = new DOMParser().parseFromString(options.sourceText, "text/html");
  const parsedNodes = parseHtmlSourceNodes(options.sourceText, options.htmlVoidTags);
  const activeNode =
    findHtmlNodeAtPosition(parsedNodes, options.cursorPosition) ??
    findHtmlNodeForSelection(parsedNodes, options.selectedElement);
  const activeSelector = activeNode?.selector ?? null;

  return {
    parsedDocument,
    pageSections: collectDomTree(parsedDocument),
    activeSelector,
    pendingSelector:
      activeSelector ??
      options.selectedElement?.domPath ??
      options.selectedElement?.cssSelector ??
      "body",
  };
}

export function createSelectionInfoFromSourceElement(
  element: Element,
  fallback: SelectionInfo | null,
  selectedClass: string,
): SelectionInfo {
  const tag = element.tagName.toLowerCase();
  const id = element.id;
  const classes = Array.from(element.classList).filter((className) =>
    className !== selectedClass && !RUNTIME_CLASSES.has(className)
  );
  const parentElement =
    element.parentElement && element.parentElement.tagName.toLowerCase() !== "html"
      ? element.parentElement
      : null;
  const childNodes = Array.from(element.children)
    .filter((child): child is Element => child instanceof Element && !isEmptyTeraSlot(child))
    .slice(0, 24)
    .map((child) => createDomNodeLink(child));
  const hasChildElements = Array.from(element.children).some((child) =>
    child instanceof Element && !isEmptyTeraSlot(child)
  );

  return {
    selector: formatElementSelector(tag, id, classes),
    cssSelector: createCssSelector(tag, id, classes),
    domPath: createDomPathSelector(element),
    tag,
    id,
    href: element.getAttribute("href") ?? "",
    title: element.getAttribute("title") ?? "",
    alt: element.getAttribute("alt") ?? "",
    classes,
    text: summarizeElementText(element.textContent),
    rawText: element.textContent ?? "",
    hasChildElements,
    rect:
      fallback?.rect ?? {
        width: "-",
        height: "-",
        top: "-",
        left: "-",
      },
    styles: fallback?.styles ?? [],
    variables: fallback?.variables ?? [],
    matchedRules: fallback?.matchedRules ?? [],
    imageSrc: tag === "img" ? element.getAttribute("src") : null,
    zolaImage: tag === "img" ? zolaImagePresentationFromElement(element) : null,
    attributes: collectElementAttributes(element, selectedClass),
    parentNode: parentElement ? createDomNodeLink(parentElement) : null,
    childNodes,
    sourceLocation: null,
    sourceId: element.getAttribute(SOURCE_ID_ATTR) ?? null,
    templateSourceId: inheritedTemplateSourceId(element),
    sessionId: element.getAttribute(SESSION_ID_ATTR) ?? null,
  };
}

export function createSelectionInfo(element: Element, previewWindow: Window, selectedClass: string): SelectionInfo {
  const computed = previewWindow.getComputedStyle(element);
  const rect = element.getBoundingClientRect();
  const tag = element.tagName.toLowerCase();
  const id = element.id;
  const classes = Array.from(element.classList).filter((className) =>
    className !== selectedClass && !RUNTIME_CLASSES.has(className)
  );
  const variables = collectRelevantCssVariables(element, previewWindow);
  const matchedRules = collectMatchedCssRules(element, previewWindow);
  const parentElement =
    element.parentElement && element.parentElement.tagName.toLowerCase() !== "html"
      ? element.parentElement
      : null;
  const childNodes = Array.from(element.children)
    .filter((child): child is Element => child instanceof Element && !isEmptyTeraSlot(child))
    .slice(0, 24)
    .map((child) => createDomNodeLink(child));
  const hasChildElements = Array.from(element.children).some((child) =>
    child instanceof Element && !isEmptyTeraSlot(child)
  );

  return {
    selector: formatElementSelector(tag, id, classes),
    cssSelector: createCssSelector(tag, id, classes),
    domPath: createDomPathSelector(element),
    tag,
    id,
    href: element.getAttribute("href") ?? "",
    title: element.getAttribute("title") ?? "",
    alt: element.getAttribute("alt") ?? "",
    classes,
    text: summarizeElementText(element.textContent),
    rawText: element.textContent ?? "",
    hasChildElements,
    rect: {
      width: `${Math.round(rect.width)}px`,
      height: `${Math.round(rect.height)}px`,
      top: `${Math.round(rect.top)}px`,
      left: `${Math.round(rect.left)}px`,
    },
    styles: [
      { label: "color", value: computed.color },
      { label: "background", value: computed.backgroundColor },
      { label: "font-size", value: computed.fontSize },
      { label: "line-height", value: computed.lineHeight },
      { label: "text-align", value: computed.textAlign },
      { label: "font-weight", value: computed.fontWeight },
      { label: "display", value: computed.display },
      { label: "flex-direction", value: computed.flexDirection },
      { label: "justify-content", value: computed.justifyContent },
      { label: "align-items", value: computed.alignItems },
      { label: "gap", value: computed.gap },
      { label: "margin", value: formatBox(computed, "margin") },
      { label: "padding", value: formatBox(computed, "padding") },
      { label: "border-radius", value: computed.borderRadius },
    ],
    variables,
    matchedRules,
    imageSrc: tag === "img" ? element.getAttribute("src") : null,
    zolaImage: tag === "img" ? zolaImagePresentationFromElement(element) : null,
    attributes: collectElementAttributes(element, selectedClass),
    parentNode: parentElement ? createDomNodeLink(parentElement) : null,
    childNodes,
    sourceLocation: null,
    sourceId: element.getAttribute(SOURCE_ID_ATTR) ?? null,
    templateSourceId: inheritedTemplateSourceId(element),
    sessionId: element.getAttribute(SESSION_ID_ATTR) ?? null,
  };
}

type SelectionEditorStateOptions = {
  variableOverrides: Record<string, string>;
  canEditHtmlSource: boolean;
  blockedReason: string;
};

export function deriveSelectionEditorState(selection: SelectionInfo, options: SelectionEditorStateOptions) {
  const variableValues: Record<string, string> = {};

  for (const variable of selection.variables) {
    variableValues[variable.name] = options.variableOverrides[variable.name] ?? variable.value;
  }

  const attributeValues: EditableAttributes = { ...selection.attributes };

  const hasSourceAnchor = Boolean(selection.sourceId || selection.sourceLocation || selection.sessionId);
  const canEdit = options.canEditHtmlSource || hasSourceAnchor;
  const editViaTemplate = !options.canEditHtmlSource && hasSourceAnchor;
  const templateLabel = editViaTemplate ? " (template Tera)" : "";

  const classStatus = canEdit
    ? `Clasele elementului pot fi editate direct in HTML${templateLabel}.`
    : options.blockedReason;
  const imageStatus = selection.imageSrc
    ? canEdit
      ? `Sursa imaginii poate fi editata direct in HTML${templateLabel}.`
      : options.blockedReason
    : "Elementul selectat nu foloseste atributul src.";
  const attributeStatus = canEdit
    ? `Atributele HTML pot fi editate direct${templateLabel}.`
    : options.blockedReason;
  const textStatus = canEdit
    ? selection.hasChildElements
      ? "Elementul selectat contine alti noduri HTML. Editarea textului este blocata."
      : `Textul poate fi editat pentru elemente simple, fara copii HTML${templateLabel}.`
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
