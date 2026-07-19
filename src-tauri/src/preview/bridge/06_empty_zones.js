  var emptyZoneContainerTags = {
    main: true,
    section: true,
    article: true,
    header: true,
    footer: true,
    nav: true,
    aside: true,
    div: true,
    ul: true,
    ol: true,
    li: true,
    form: true,
    fieldset: true
  };

  function isEmptyTeraSlot(element) {
    return element instanceof Element && element.hasAttribute(EMPTY_TERA_SLOT_ATTR);
  }

  function closestTemplateGateSourceId(element) {
    if (!(element instanceof Element)) return null;
    var sourceElement = element.closest("[" + TEMPLATE_SOURCE_ID_ATTR + "]");
    return sourceElement ? sourceElement.getAttribute(TEMPLATE_SOURCE_ID_ATTR) : null;
  }

  function belongsToClosedTeraGate(element) {
    var sourceId = closestTemplateGateSourceId(element);
    return Boolean(sourceId && !openTeraGateSourceIds[sourceId]);
  }

  function isEmptyZoneContainer(element) {
    if (!(element instanceof Element)) return false;
    if (isEmptyTeraSlot(element)) return false;
    if (isStudioOverlayElement(element)) return false;
    if (element === document.body || element === document.documentElement) return false;
    return Boolean(emptyZoneContainerTags[element.tagName.toLowerCase()]);
  }

  function hasMeaningfulElementChild(element) {
    return Array.prototype.some.call(element.children, function (child) {
      return child instanceof Element && !isStudioOverlayElement(child);
    });
  }

  function isEmptyEditableElement(element) {
    if (!isEmptyZoneContainer(element)) return false;
    if (belongsToClosedTeraGate(element)) return false;
    if (hasMeaningfulElementChild(element)) return false;
    return String(element.textContent || "").trim().length === 0;
  }

  function clearEmptyHtmlAffordances() {
    Array.prototype.forEach.call(document.querySelectorAll("[" + EMPTY_HTML_ATTR + "]"), function (element) {
      element.classList.remove(EMPTY_EDITABLE_CLASS);
      element.removeAttribute(EMPTY_HTML_ATTR);
      element.removeAttribute("data-pana-empty-label");
    });
  }

  function refreshEmptyHtmlAffordances() {
    clearEmptyHtmlAffordances();
    Array.prototype.forEach.call(document.body ? document.body.querySelectorAll("*") : [], function (element) {
      if (!isEmptyEditableElement(element)) return;
      element.classList.add(EMPTY_EDITABLE_CLASS);
      element.setAttribute(EMPTY_HTML_ATTR, "true");
      element.setAttribute("data-pana-empty-label", "Element HTML gol");
    });
  }

  function removeEmptyTeraSlots() {
    Array.prototype.forEach.call(document.querySelectorAll("[" + EMPTY_TERA_SLOT_ATTR + "]"), function (element) {
      if (element.getAttribute("data-pana-empty-tera-slot-static") === "true") return;
      element.remove();
    });
  }

  function meaningfulContentBetween(startNode, endNode) {
    if (!startNode || !endNode || startNode.parentNode !== endNode.parentNode) return true;
    var node = startNode.nextSibling;
    while (node && node !== endNode) {
      if (node.nodeType === Node.ELEMENT_NODE) {
        var element = node;
        if (!isStudioOverlayElement(element) && !isEmptyTeraSlot(element)) return true;
      } else if (node.nodeType === Node.TEXT_NODE && String(node.nodeValue || "").trim().length > 0) {
        return true;
      }
      node = node.nextSibling;
    }
    return false;
  }

  function templateMarkerPairs() {
    if (!document.body) return [];
    var walker = document.createTreeWalker(document.body, NodeFilter.SHOW_COMMENT);
    var stack = [];
    var pairs = [];
    var node = walker.nextNode();

    while (node) {
      var marker = templateSourceMarker(node.nodeValue);
      if (marker && marker.kind === "start") {
        stack.push({ id: marker.id, node: node });
      } else if (marker && marker.kind === "end") {
        for (var index = stack.length - 1; index >= 0; index -= 1) {
          if (stack[index].id === marker.id) {
            pairs.push({ id: marker.id, start: stack[index].node, end: node });
            stack.splice(index, 1);
            break;
          }
        }
      }
      node = walker.nextNode();
    }

    return pairs;
  }

  function refreshEmptyTeraSlots() {
    removeEmptyTeraSlots();
    templateMarkerPairs().forEach(function (pair) {
      if (!pair.start.parentNode || pair.start.parentNode !== pair.end.parentNode) return;
      if (meaningfulContentBetween(pair.start, pair.end)) return;
      if (emptyTeraSlotBetween(pair.start, pair.end, pair.id)) return;
      var slot = document.createElement("div");
      slot.className = EMPTY_EDITABLE_CLASS + " " + EMPTY_TERA_SLOT_CLASS;
      slot.setAttribute(EMPTY_TERA_SLOT_ATTR, pair.id);
      slot.setAttribute(SOURCE_ID_ATTR, pair.id);
      slot.setAttribute(TEMPLATE_SOURCE_ID_ATTR, pair.id);
      slot.setAttribute("data-pana-empty-label", "Block Tera gol");
      pair.end.parentNode.insertBefore(slot, pair.end);
    });
  }

  function emptyTeraSlotBetween(startNode, endNode, sourceId) {
    var node = startNode.nextSibling;
    while (node && node !== endNode) {
      if (
        node.nodeType === Node.ELEMENT_NODE &&
        isEmptyTeraSlot(node) &&
        node.getAttribute(EMPTY_TERA_SLOT_ATTR) === sourceId
      ) {
        return true;
      }
      node = node.nextSibling;
    }
    return false;
  }

  function refreshEmptyEditableZones() {
    if (!document.body) return;
    refreshEmptyTeraSlots();
    refreshEmptyHtmlAffordances();
    ensureElementSessionIds();
  }
