  function compactText(text) {
    return (text || "").replace(/\s+/g, " ").trim();
  }

  function shortenLabel(text) {
    var compact = compactText(text);
    if (!compact) return "";
    return compact.length > 56 ? compact.slice(0, 53).trimEnd() + "..." : compact;
  }

  function directTextFor(element) {
    var chunks = [];
    Array.prototype.forEach.call(element.childNodes, function (node) {
      if (node.nodeType === Node.TEXT_NODE) {
        var text = compactText(node.nodeValue);
        if (text) chunks.push(text);
      }
    });
    return shortenLabel(chunks.join(" "));
  }

  function textContentLabelFor(element) {
    return shortenLabel(element.textContent);
  }

  function firstDirectHeadingLabelFor(element) {
    var headingTags = { h1: true, h2: true, h3: true, h4: true, h5: true, h6: true };
    for (var index = 0; index < element.children.length; index += 1) {
      var child = element.children[index];
      var tag = child.tagName ? child.tagName.toLowerCase() : "";
      if (headingTags[tag]) {
        var label = textContentLabelFor(child);
        if (label) return label;
      }
    }
    return "";
  }

  function firstListItemLabelFor(element) {
    var item = element.querySelector(":scope > li");
    return item ? textContentLabelFor(item) : "";
  }

  function mediaFileName(value) {
    if (!value) return "";
    var clean = String(value).split("?")[0].split("#")[0];
    var parts = clean.split("/");
    return decodeURIComponent(parts[parts.length - 1] || clean);
  }

  function semanticTagLabel(tag) {
    var labels = {
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
    return labels[tag] || tag;
  }

  function readableClassFor(element) {
    var generatedPrefix = /^(ps|pana)-/;
    var utilityClass = /^(container|section|row|col|grid|flex|btn|button|active|open|hidden|show|cont-\d+)/;
    return Array.prototype.find.call(element.classList, function (className) {
      return className !== EMPTY_EDITABLE_CLASS &&
        className !== EMPTY_TERA_SLOT_CLASS &&
        !generatedPrefix.test(className) &&
        !utilityClass.test(className);
    }) || "";
  }

  function escapeCssIdentifier(value) {
    return String(value).replace(/[^A-Za-z0-9_-]/g, function (character) {
      return "\\" + character;
    });
  }

  function createCssSelector(tag, id, classes) {
    if (id) {
      return tag + "#" + escapeCssIdentifier(id);
    }

    if (classes.length > 0) {
      return tag + "." + classes.map(escapeCssIdentifier).join(".");
    }

    return tag;
  }

  function createDomPathSelector(element) {
    var segments = [];
    var current = element;

    while (current && current.tagName && current.tagName.toLowerCase() !== "html") {
      var tag = current.tagName.toLowerCase();

      if (current.id) {
        segments.unshift(tag + "#" + escapeCssIdentifier(current.id));
        break;
      }

      var parent = current.parentElement;
      if (!parent) {
        segments.unshift(tag);
        break;
      }

      var siblings = Array.prototype.filter.call(parent.children, function (sibling) {
        return sibling.tagName.toLowerCase() === tag;
      });
      var index = siblings.indexOf(current) + 1;
      segments.unshift(tag + ":nth-of-type(" + index + ")");
      current = parent;
    }

    return segments.join(" > ");
  }

  function domNodeLabelFor(element) {
    var tag = element.tagName.toLowerCase();
    if (element.hasAttribute(EMPTY_TERA_SLOT_ATTR)) {
      return element.getAttribute("data-pana-empty-label") || semanticTagLabel(tag);
    }

    var ariaLabel = shortenLabel(element.getAttribute("aria-label"));
    if (ariaLabel) return ariaLabel;

    var title = shortenLabel(element.getAttribute("title"));
    if (title) return title;

    var ownText = directTextFor(element);
    var fullTextTags = {
      h1: true, h2: true, h3: true, h4: true, h5: true, h6: true,
      p: true, a: true, button: true, span: true, small: true, strong: true,
      em: true, blockquote: true, figcaption: true, label: true, code: true, pre: true,
      li: true, th: true, td: true, caption: true
    };
    if (fullTextTags[tag]) {
      var fullText = textContentLabelFor(element);
      if (fullText) return fullText;
    }

    if (tag === "img") {
      var alt = shortenLabel(element.getAttribute("alt"));
      if (alt) return "Imagine: " + alt;
      var src = mediaFileName(element.getAttribute("src"));
      return src ? "Imagine: " + src : "Imagine";
    }

    if (tag === "video" || tag === "audio" || tag === "iframe" || tag === "source") {
      var mediaSrc = mediaFileName(element.getAttribute("src"));
      return mediaSrc ? semanticTagLabel(tag) + ": " + mediaSrc : semanticTagLabel(tag);
    }

    if (ownText) return ownText;

    if (tag === "ul" || tag === "ol") {
      var itemLabel = firstListItemLabelFor(element);
      if (itemLabel) return semanticTagLabel(tag) + ": " + itemLabel;
    }

    var directHeading = firstDirectHeadingLabelFor(element);
    if (directHeading) return semanticTagLabel(tag) + ": " + directHeading;

    if (element.id) {
      return "#" + element.id;
    }

    var firstClass = readableClassFor(element);
    if (firstClass) {
      return "." + firstClass;
    }

    return semanticTagLabel(tag);
  }

  function createDomNodeLink(element) {
    return {
      selector: createDomPathSelector(element),
      label: domNodeLabelFor(element),
      tag: element.tagName.toLowerCase(),
    };
  }

  function inheritedTemplateSourceId(element) {
    var current = element;
    while (current && current instanceof Element && current.tagName.toLowerCase() !== "html") {
      var sourceId = current.getAttribute(TEMPLATE_SOURCE_ID_ATTR);
      if (sourceId) return sourceId;
      current = current.parentElement;
    }
    return null;
  }

  function formatElementSelector(tag, id, classes) {
    var idPart = id ? ' id="' + id + '"' : "";
    var classPart = classes.length > 0 ? ' class="' + classes.join(" ") + '"' : "";
    return "<" + tag + idPart + classPart + ">";
  }

  var SKIP_ATTRS = { "class": true, "style": true };
  SKIP_ATTRS[SOURCE_ID_ATTR] = true;
  SKIP_ATTRS[TEMPLATE_SOURCE_ID_ATTR] = true;
  SKIP_ATTRS[TEMPLATE_SOURCE_STACK_ATTR] = true;
  SKIP_ATTRS[PREVIEW_REVISION_ATTR] = true;
  SKIP_ATTRS[SESSION_ID_ATTR] = true;
  SKIP_ATTRS[EMPTY_TERA_SLOT_ATTR] = true;
  SKIP_ATTRS[EMPTY_HTML_ATTR] = true;
  SKIP_ATTRS["data-pana-empty-label"] = true;

  function templateSourceMarker(text) {
    var match = String(text || "").match(/^\s*pana-template-source-(start|end):([A-Za-z0-9_-]+)\s*$/);
    return match ? { kind: match[1], id: match[2] } : null;
  }

  function assignTemplateSourceStack(element, stack) {
    if (!element || stack.length === 0) return;
    element.setAttribute(TEMPLATE_SOURCE_ID_ATTR, stack[stack.length - 1]);
    element.setAttribute(TEMPLATE_SOURCE_STACK_ATTR, stack.join(" "));
  }

  function applyTemplateSourceIdsFromMarkers() {
    if (!document.body) return;
    var walker = document.createTreeWalker(
      document.body,
      NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_COMMENT
    );
    var stack = [];
    var node = walker.nextNode();

    while (node) {
      if (node.nodeType === Node.COMMENT_NODE) {
        var marker = templateSourceMarker(node.nodeValue);
        if (marker && marker.kind === "start") {
          stack.push(marker.id);
        } else if (marker && marker.kind === "end") {
          var index = stack.lastIndexOf(marker.id);
          if (index >= 0) stack.splice(index, 1);
        }
      } else if (node instanceof Element && stack.length > 0) {
        assignTemplateSourceStack(node, stack);
      }
      node = walker.nextNode();
    }
  }

  function ensureElementSessionIds() {
    if (!document.body) return;
    var elements = document.body.querySelectorAll("*");
    for (var i = 0; i < elements.length; i += 1) {
      if (!elements[i].getAttribute(SESSION_ID_ATTR)) {
        elements[i].setAttribute(SESSION_ID_ATTR, "ps-" + String(nextSessionElementId++));
      }
    }
  }

  function collectElementAttributes(element) {
    var result = {};
    for (var i = 0; i < element.attributes.length; i++) {
      var attr = element.attributes[i];
      if (attr.name.indexOf("data-pana-") !== 0 && !SKIP_ATTRS[attr.name]) {
        result[attr.name] = attr.value;
      }
    }
    return result;
  }

  function cssEscapeValue(value) {
    if (window.CSS && typeof window.CSS.escape === "function") {
      return window.CSS.escape(String(value || ""));
    }
    return String(value || "").replace(/["\\]/g, "\\$&");
  }

  function notifyPanaBlocksInit(root) {
    var detail = { root: root || document };
    try {
      document.dispatchEvent(new CustomEvent("pana:blocks:init", { detail: detail }));
    } catch (_) {
      var event = document.createEvent("CustomEvent");
      event.initCustomEvent("pana:blocks:init", false, false, detail);
      document.dispatchEvent(event);
    }
  }

  function summarizeElementText(text) {
    var normalized = (text || "").replace(/\s+/g, " ").trim();
    if (normalized.length <= 90) {
      return normalized || "Fara text";
    }

    return normalized.slice(0, 87) + "...";
  }

  function sectionDepthFor(element) {
    var depth = 0;
    var current = element.parentElement;

    while (current && current.tagName.toLowerCase() !== "body") {
      if (current.matches("main, section, article, header, footer, nav, aside")) {
        depth += 1;
      }
      current = current.parentElement;
    }

    return depth;
  }

  function isStudioOverlayElement(element) {
    if (!(element instanceof Element)) return false;
    return element.id === HTML_SELECTION_ID ||
      element.id === PREVIEW_HOVER_ID ||
      element.id === TEMPLATE_GATE_ID ||
      element.id === TEMPLATE_GATE_ACTIONS_ID ||
      element.id === "pana-studio-preview-drop-line" ||
      element.id === "pana-studio-preview-drop-box" ||
      element.id === "pana-studio-preview-drop-hint";
  }

  function collectPageSections() {
    var result = [];
    var skipTags = {
      script: true, style: true, noscript: true, meta: true, link: true, head: true,
      br: true, hr: true, wbr: true, input: true, textarea: true, select: true
    };
    var svgTags = {
      svg: true, path: true, g: true, defs: true, use: true, circle: true, rect: true,
      polygon: true, polyline: true, line: true, text: true, tspan: true
    };
    var maxDepth = 9;
    var maxNodes = 300;

    function traverse(element, depth) {
      if (result.length >= maxNodes || depth > maxDepth) return;
      var tag = element.tagName.toLowerCase();
      if (skipTags[tag] || svgTags[tag]) return;
      if (isStudioOverlayElement(element)) return;
      if (isEmptyTeraSlot(element)) return;
      result.push({
        selector: createDomPathSelector(element),
	        label: domNodeLabelFor(element),
	        tag: tag,
	        depth: depth,
	        sourceLocation: null,
	        sourceId: element.getAttribute(SOURCE_ID_ATTR) || null,
        templateSourceId: inheritedTemplateSourceId(element),
        sessionId: element.getAttribute(SESSION_ID_ATTR) || null
      });
      Array.prototype.forEach.call(element.children, function (child) {
        traverse(child, depth + 1);
      });
    }

    if (document.body) {
      Array.prototype.forEach.call(document.body.children, function (child) {
        traverse(child, 0);
      });
    }

    return result;
  }

  function syncStructure() {
    post("structure", { sections: collectPageSections() });
  }

  function formatBox(style, property) {
    var top = style.getPropertyValue(property + "-top");
    var right = style.getPropertyValue(property + "-right");
    var bottom = style.getPropertyValue(property + "-bottom");
    var left = style.getPropertyValue(property + "-left");

    if (top === right && right === bottom && bottom === left) {
      return top;
    }

    if (top === bottom && right === left) {
      return top + " " + right;
    }

    return top + " " + right + " " + bottom + " " + left;
  }
