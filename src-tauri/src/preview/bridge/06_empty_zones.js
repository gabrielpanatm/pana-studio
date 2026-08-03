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
  var ACTIVE_AUTHORING_ATTR = "data-pana-active-authoring-surface";
  var ACTIVE_AUTHORING_OVERLAY_ATTR = "data-pana-active-authoring-overlay";
  var ACTIVE_AUTHORING_POPULATED_ATTR = "data-pana-active-document-populated";
  var ACTIVE_AUTHORING_MIN_HEIGHT_PROPERTY = "--pana-active-authoring-min-height";
  var ACTIVE_AUTHORING_APPEND_HIT_HEIGHT = 24;
  var activeDocumentAuthoringSurfaces = [];
  var activeDocumentAuthoringFrame = null;
  var activeDocumentAuthoringLayoutDirty = true;
  var activeDocumentAuthoringViewportWidth = 0;
  var activeDocumentAuthoringViewportHeight = 0;

  function isEmptyTeraSlot(element) {
    return element instanceof Element && element.hasAttribute(EMPTY_TERA_SLOT_ATTR);
  }

  function isActiveDocumentRoot(element) {
    return element instanceof Element && element.hasAttribute(ACTIVE_DOCUMENT_ROOT_ATTR);
  }

  function normalizedAuthoringSurfaces(value) {
    if (!Array.isArray(value)) return [];
    var seen = {};
    return value.slice(0, 32).filter(function (surface) {
      if (!surface || typeof surface !== "object") return false;
      var sourceNodeId = String(surface.sourceNodeId || "");
      var boundaryInstanceId = String(surface.boundaryInstanceId || "");
      var renderInstanceId = typeof surface.renderInstanceId === "string"
        ? surface.renderInstanceId
        : "";
      if (!sourceNodeId || !boundaryInstanceId) return false;
      if (sourceNodeId.length > 512 || boundaryInstanceId.length > 512 || renderInstanceId.length > 512) {
        return false;
      }
      var key = sourceNodeId + "\u0000" + boundaryInstanceId + "\u0000" + renderInstanceId;
      if (seen[key]) return false;
      seen[key] = true;
      surface.sourceNodeId = sourceNodeId;
      surface.boundaryInstanceId = boundaryInstanceId;
      surface.renderInstanceId = renderInstanceId || null;
      return true;
    });
  }

  function authoringRootForSurface(surface) {
    if (!surface) return null;
    var sourceSelector = "[" + ACTIVE_DOCUMENT_ROOT_ATTR + "=\"" +
      cssEscapeValue(surface.sourceNodeId) + "\"][" + SOURCE_ID_ATTR + "=\"" +
      cssEscapeValue(surface.sourceNodeId) + "\"]";
    var dynamicElement = document.querySelector(sourceSelector);
    return dynamicElement instanceof Element && isActiveDocumentRoot(dynamicElement)
      ? dynamicElement
      : null;
  }

  function activeDocumentAuthoringElementForBoundary(boundaryInstanceId) {
    if (typeof boundaryInstanceId !== "string" || !boundaryInstanceId) return null;
    for (var index = 0; index < activeDocumentAuthoringSurfaces.length; index += 1) {
      var surface = activeDocumentAuthoringSurfaces[index];
      if (surface.boundaryInstanceId !== boundaryInstanceId) continue;
      var element = authoringRootForSurface(surface);
      if (element instanceof Element) return element;
    }
    return null;
  }

  function nextMeaningfulAuthoringSibling(element) {
    var sibling = element ? element.nextElementSibling : null;
    while (sibling) {
      if (!isStudioOverlayElement(sibling)) return sibling;
      sibling = sibling.nextElementSibling;
    }
    return null;
  }

  function activeAuthoringViewportSize() {
    var root = document.documentElement;
    return {
      width: Math.max(
        root ? root.clientWidth : 0,
        Number.isFinite(window.innerWidth) ? window.innerWidth : 0
      ),
      height: Math.max(
        root ? root.clientHeight : 0,
        Number.isFinite(window.innerHeight) ? window.innerHeight : 0
      )
    };
  }

  function clearActiveDocumentAuthoringFlowSizing() {
    Array.prototype.forEach.call(
      document.querySelectorAll("[" + ACTIVE_DOCUMENT_ROOT_ATTR + "]"),
      function (element) {
        if (element instanceof HTMLElement) {
          element.style.removeProperty(ACTIVE_AUTHORING_MIN_HEIGHT_PROPERTY);
        }
      }
    );
  }

  function isActiveAuthoringFlowGeometryCandidate(element) {
    if (!(element instanceof Element)) return false;
    if (isEmptyTeraSlot(element) || isActiveDocumentRoot(element) || isStudioOverlayElement(element)) return false;
    if (element.closest("[data-pana-canvas-agent-overlay], [data-pana-canvas-agent-action]")) {
      return false;
    }
    var tag = element.tagName.toLowerCase();
    if (
      tag === "script" || tag === "style" || tag === "link" || tag === "meta" ||
      tag === "noscript" || tag === "template"
    ) return false;
    var style = window.getComputedStyle(element);
    if (style.display === "none" || style.visibility === "hidden") return false;
    if (style.position === "absolute" || style.position === "fixed") return false;
    var rect = element.getBoundingClientRect();
    return rect.width > 0 || rect.height > 0;
  }

  function authoredFlowBottom() {
    if (!document.body) return 0;
    var scrollTop = window.scrollY || document.documentElement.scrollTop || 0;
    var bottom = 0;
    Array.prototype.forEach.call(document.body.querySelectorAll("*"), function (element) {
      if (!isActiveAuthoringFlowGeometryCandidate(element)) return;
      var rect = element.getBoundingClientRect();
      if (!Number.isFinite(rect.bottom)) return;
      bottom = Math.max(bottom, rect.bottom + scrollTop);
    });
    return bottom;
  }

  function activeAuthoringParentContentBottom(element) {
    var parent = element ? element.parentElement : null;
    if (!(parent instanceof Element)) return null;
    var parentRect = parent.getBoundingClientRect();
    var style = window.getComputedStyle(parent);
    return parentRect.bottom - (parseFloat(style.borderBottomWidth) || 0) -
      (parseFloat(style.paddingBottom) || 0);
  }

  function fitActiveDocumentAuthoringFlow() {
    clearActiveDocumentAuthoringFlowSizing();
    var primary = null;
    for (var index = 0; index < activeDocumentAuthoringSurfaces.length; index += 1) {
      var candidate = authoringRootForSurface(activeDocumentAuthoringSurfaces[index]);
      if (candidate instanceof HTMLElement) {
        primary = candidate;
        break;
      }
    }
    var viewport = activeAuthoringViewportSize();
    activeDocumentAuthoringViewportWidth = viewport.width;
    activeDocumentAuthoringViewportHeight = viewport.height;
    activeDocumentAuthoringLayoutDirty = false;
    if (!(primary instanceof HTMLElement)) return;
    // Once the active source has authored content, its synthetic root is only
    // an identity anchor. It must not consume flow space or move the site's
    // footer. Consecutive drops use a separately computed hit band instead.
    if (primary.hasAttribute(ACTIVE_AUTHORING_POPULATED_ATTR)) return;

    var naturalRect = primary.getBoundingClientRect();
    var parentContentBottom = activeAuthoringParentContentBottom(primary);
    var availableParentHeight = Number.isFinite(parentContentBottom)
      ? Math.max(0, parentContentBottom - naturalRect.top)
      : 0;
    var naturalHeight = Math.max(52, naturalRect.height, availableParentHeight);
    var flowBottom = authoredFlowBottom();
    var residualViewportHeight = Math.max(0, viewport.height - flowBottom);
    var fittedHeight = Math.max(52, naturalHeight + residualViewportHeight);
    primary.style.setProperty(ACTIVE_AUTHORING_MIN_HEIGHT_PROPERTY, fittedHeight + "px");
  }

  function activeDocumentAuthoringLayoutIsStale() {
    var viewport = activeAuthoringViewportSize();
    return activeDocumentAuthoringLayoutDirty ||
      viewport.width !== activeDocumentAuthoringViewportWidth ||
      viewport.height !== activeDocumentAuthoringViewportHeight;
  }

  function invalidateActiveDocumentAuthoringLayout() {
    activeDocumentAuthoringLayoutDirty = true;
    scheduleActiveDocumentAuthoringRefresh();
  }

  function activeDocumentAuthoringRectForElement(element) {
    if (!(element instanceof Element) || !isActiveDocumentRoot(element)) return null;
    var active = activeDocumentAuthoringSurfaces.some(function (surface) {
      return authoringRootForSurface(surface) === element;
    });
    if (!active) return null;
    var slotRect = element.getBoundingClientRect();
    var parent = element.parentElement;
    if (!(parent instanceof Element)) return slotRect;
    var parentRect = parent.getBoundingClientRect();
    var style = window.getComputedStyle(parent);
    var contentLeft = parentRect.left + (parseFloat(style.borderLeftWidth) || 0) +
      (parseFloat(style.paddingLeft) || 0);
    var contentRight = parentRect.right - (parseFloat(style.borderRightWidth) || 0) -
      (parseFloat(style.paddingRight) || 0);
    var contentBottom = parentRect.bottom - (parseFloat(style.borderBottomWidth) || 0) -
      (parseFloat(style.paddingBottom) || 0);
    if (element.hasAttribute(ACTIVE_AUTHORING_POPULATED_ATTR)) {
      // The DOM anchor is collapsed for populated documents. Keep an
      // out-of-flow target across the real residual space of its parent so a
      // later drop anywhere in the active page still addresses the exact Rust
      // document root. This geometry never participates in document flow.
      var parentContentTop = parentRect.top + (parseFloat(style.borderTopWidth) || 0) +
        (parseFloat(style.paddingTop) || 0);
      var appendTop = Math.max(parentContentTop, slotRect.top);
      var appendBottom = Math.max(appendTop, contentBottom);
      var populatedNextSibling = nextMeaningfulAuthoringSibling(element);
      if (populatedNextSibling) {
        var populatedNextRect = populatedNextSibling.getBoundingClientRect();
        if (populatedNextRect.top >= appendTop) {
          appendBottom = Math.min(appendBottom, populatedNextRect.top);
        }
      }
      if (appendBottom - appendTop < 1) {
        appendBottom = appendTop;
        appendTop = Math.max(
          parentContentTop,
          appendBottom - ACTIVE_AUTHORING_APPEND_HIT_HEIGHT
        );
      }
      return {
        left: contentLeft,
        top: appendTop,
        right: contentRight,
        bottom: appendBottom,
        width: Math.max(0, contentRight - contentLeft),
        height: Math.max(0, appendBottom - appendTop)
      };
    }
    var nextSibling = nextMeaningfulAuthoringSibling(element);
    if (nextSibling) {
      var nextRect = nextSibling.getBoundingClientRect();
      if (nextRect.top > slotRect.top) contentBottom = Math.min(contentBottom, nextRect.top);
    }
    var left = Math.min(slotRect.left, contentLeft);
    var right = Math.max(slotRect.right, contentRight);
    var top = slotRect.top;
    var bottom = Math.max(slotRect.bottom, contentBottom);
    return {
      left: left,
      top: top,
      right: right,
      bottom: bottom,
      width: Math.max(0, right - left),
      height: Math.max(0, bottom - top)
    };
  }

  function activeDocumentAuthoringTargetAtPoint(clientX, clientY) {
    if (!Number.isFinite(clientX) || !Number.isFinite(clientY)) return null;
    var matches = [];
    activeDocumentAuthoringSurfaces.forEach(function (surface) {
      var element = authoringRootForSurface(surface);
      var rect = activeDocumentAuthoringRectForElement(element);
      if (!rect || rect.width <= 0 || rect.height <= 0) return;
      if (clientX < rect.left || clientX > rect.right || clientY < rect.top || clientY > rect.bottom) {
        return;
      }
      matches.push({ surface: surface, element: element, rect: rect });
    });
    matches.sort(function (left, right) {
      return left.rect.width * left.rect.height - right.rect.width * right.rect.height;
    });
    return matches[0] || null;
  }

  function ensureActiveAuthoringOverlay(index) {
    var selector = "[" + ACTIVE_AUTHORING_OVERLAY_ATTR + "=\"" + String(index) + "\"]";
    var overlay = document.querySelector(selector);
    if (overlay) return overlay;
    overlay = document.createElement("div");
    overlay.setAttribute(ACTIVE_AUTHORING_OVERLAY_ATTR, String(index));
    overlay.setAttribute("data-pana-canvas-agent-overlay", "authoring");
    overlay.style.cssText = [
      "position:fixed",
      "z-index:2147483643",
      "display:none",
      "border:1px dashed rgba(59,130,246,.62)",
      "background:linear-gradient(135deg,rgba(59,130,246,.055),rgba(59,130,246,.015))",
      "pointer-events:none",
      "box-sizing:border-box"
    ].join(";");
    document.body.appendChild(overlay);
    return overlay;
  }

  function refreshActiveDocumentAuthoringSurfaces() {
    activeDocumentAuthoringFrame = null;
    if (activeDocumentAuthoringLayoutIsStale()) {
      fitActiveDocumentAuthoringFlow();
    }
    Array.prototype.forEach.call(document.querySelectorAll("[" + ACTIVE_AUTHORING_ATTR + "]"), function (element) {
      element.removeAttribute(ACTIVE_AUTHORING_ATTR);
    });
    activeDocumentAuthoringSurfaces.forEach(function (surface, index) {
      var element = authoringRootForSurface(surface);
      var overlay = ensureActiveAuthoringOverlay(index);
      var rect = activeDocumentAuthoringRectForElement(element);
      if (!(element instanceof Element) || !rect || rect.width <= 0 || rect.height <= 0) {
        overlay.style.display = "none";
        return;
      }
      element.setAttribute(ACTIVE_AUTHORING_ATTR, surface.boundaryInstanceId);
      if (element.hasAttribute(ACTIVE_AUTHORING_POPULATED_ATTR)) {
        // A populated document keeps an invisible append hit band, but no
        // permanent Canvas rectangle over the rendered website.
        overlay.style.display = "none";
        return;
      }
      overlay.style.display = "block";
      overlay.style.left = Math.round(rect.left) + "px";
      overlay.style.top = Math.round(rect.top) + "px";
      overlay.style.width = Math.round(rect.width) + "px";
      overlay.style.height = Math.round(rect.height) + "px";
    });
    Array.prototype.forEach.call(document.querySelectorAll("[" + ACTIVE_AUTHORING_OVERLAY_ATTR + "]"), function (overlay) {
      var index = Number(overlay.getAttribute(ACTIVE_AUTHORING_OVERLAY_ATTR));
      if (!Number.isInteger(index) || index < 0 || index >= activeDocumentAuthoringSurfaces.length) {
        overlay.remove();
      }
    });
  }

  function scheduleActiveDocumentAuthoringRefresh() {
    if (activeDocumentAuthoringFrame !== null) return;
    activeDocumentAuthoringFrame = window.requestAnimationFrame(refreshActiveDocumentAuthoringSurfaces);
  }

  function configureActiveDocumentAuthoringSurfaces(value) {
    activeDocumentAuthoringSurfaces = normalizedAuthoringSurfaces(value);
    refreshActiveDocumentRoots();
    // The Rust binding is the first authority allowed to present an empty
    // state. Reconcile ordinary empty HTML only after materializing its exact
    // document boundary so Canvas can never expose competing labels.
    refreshEmptyHtmlAffordances();
    invalidateActiveDocumentAuthoringLayout();
  }

  function clearActiveDocumentAuthoringSurfaces() {
    clearActiveDocumentAuthoringFlowSizing();
    activeDocumentAuthoringSurfaces = [];
    activeDocumentAuthoringLayoutDirty = true;
    activeDocumentAuthoringViewportWidth = 0;
    activeDocumentAuthoringViewportHeight = 0;
    if (activeDocumentAuthoringFrame !== null) {
      window.cancelAnimationFrame(activeDocumentAuthoringFrame);
      activeDocumentAuthoringFrame = null;
    }
    Array.prototype.forEach.call(document.querySelectorAll("[" + ACTIVE_AUTHORING_ATTR + "]"), function (element) {
      element.removeAttribute(ACTIVE_AUTHORING_ATTR);
    });
    Array.prototype.forEach.call(document.querySelectorAll("[" + ACTIVE_AUTHORING_OVERLAY_ATTR + "]"), function (overlay) {
      overlay.remove();
    });
    removeActiveDocumentRoots();
    removeEmptyTeraSlots();
  }

  function isEmptyZoneContainer(element) {
    if (!(element instanceof Element)) return false;
    if (isEmptyTeraSlot(element) || isActiveDocumentRoot(element)) return false;
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
      element.remove();
    });
  }

  function removeActiveDocumentRoots() {
    Array.prototype.forEach.call(
      document.querySelectorAll("[" + ACTIVE_DOCUMENT_ROOT_ATTR + "]"),
      function (element) {
        var sourceId = element.getAttribute(ACTIVE_DOCUMENT_ROOT_ATTR) || "";
        var active = activeDocumentAuthoringSurfaces.some(function (surface) {
          return surface.sourceNodeId === sourceId;
        });
        if (!active) element.remove();
      }
    );
  }

  function authoringSurfaceForSourceId(sourceId) {
    for (var index = 0; index < activeDocumentAuthoringSurfaces.length; index += 1) {
      if (activeDocumentAuthoringSurfaces[index].sourceNodeId === sourceId) {
        return activeDocumentAuthoringSurfaces[index];
      }
    }
    return null;
  }

  function meaningfulContentBetween(startNode, endNode) {
    if (!startNode || !endNode || startNode.parentNode !== endNode.parentNode) return true;
    var node = startNode.nextSibling;
    while (node && node !== endNode) {
      if (node.nodeType === Node.ELEMENT_NODE) {
        var element = node;
        if (
          !isStudioOverlayElement(element) &&
          !isEmptyTeraSlot(element) &&
          !isActiveDocumentRoot(element)
        ) return true;
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

  function refreshActiveDocumentRoots() {
    removeActiveDocumentRoots();
    removeEmptyTeraSlots();
    templateMarkerPairs().forEach(function (pair) {
      if (!pair.start.parentNode || pair.start.parentNode !== pair.end.parentNode) return;
      var authoringSurface = authoringSurfaceForSourceId(pair.id);
      if (!authoringSurface) {
        var inactive = pair.start.nextSibling;
        while (inactive && inactive !== pair.end) {
          var inactiveNext = inactive.nextSibling;
          if (
            inactive.nodeType === Node.ELEMENT_NODE &&
            (isEmptyTeraSlot(inactive) || isActiveDocumentRoot(inactive))
          ) {
            inactive.remove();
          }
          inactive = inactiveNext;
        }
        return;
      }
      var hasContent = meaningfulContentBetween(pair.start, pair.end);
      var root = activeDocumentRootBetween(pair.start, pair.end, pair.id);
      if (!root) {
        root = document.createElement("div");
        root.className = EMPTY_EDITABLE_CLASS + " " + ACTIVE_DOCUMENT_ROOT_CLASS;
        root.setAttribute(ACTIVE_DOCUMENT_ROOT_ATTR, pair.id);
        pair.end.parentNode.insertBefore(root, pair.end);
      }
      removeDuplicateActiveDocumentRoots(pair.start, pair.end, pair.id, root);
      root.setAttribute(SOURCE_ID_ATTR, pair.id);
      root.setAttribute(TEMPLATE_SOURCE_ID_ATTR, pair.id);
      root.setAttribute(ACTIVE_AUTHORING_ATTR, authoringSurface.boundaryInstanceId);
      root.removeAttribute(CANVAS_AGENT_RENDER_ATTR);
      if (hasContent) {
        root.setAttribute(ACTIVE_AUTHORING_POPULATED_ATTR, "true");
        root.removeAttribute("data-pana-empty-label");
      } else {
        root.removeAttribute(ACTIVE_AUTHORING_POPULATED_ATTR);
        root.setAttribute("data-pana-empty-label", "Document gol");
      }
    });
  }

  function activeDocumentRootBetween(startNode, endNode, sourceId) {
    var node = startNode.nextSibling;
    while (node && node !== endNode) {
      if (
        node.nodeType === Node.ELEMENT_NODE &&
        isActiveDocumentRoot(node) &&
        node.getAttribute(ACTIVE_DOCUMENT_ROOT_ATTR) === sourceId
      ) {
        return node;
      }
      node = node.nextSibling;
    }
    return null;
  }

  function removeDuplicateActiveDocumentRoots(startNode, endNode, sourceId, retainedRoot) {
    var node = startNode.nextSibling;
    while (node && node !== endNode) {
      var next = node.nextSibling;
      if (
        node !== retainedRoot &&
        node.nodeType === Node.ELEMENT_NODE &&
        isActiveDocumentRoot(node) &&
        node.getAttribute(ACTIVE_DOCUMENT_ROOT_ATTR) === sourceId
      ) {
        node.remove();
      }
      node = next;
    }
  }

  function refreshEmptyEditableZones() {
    if (!document.body) return;
    refreshActiveDocumentRoots();
    refreshEmptyHtmlAffordances();
    ensureElementSessionIds();
    invalidateActiveDocumentAuthoringLayout();
  }

  window.addEventListener("resize", invalidateActiveDocumentAuthoringLayout);
  window.addEventListener("scroll", scheduleActiveDocumentAuthoringRefresh, true);
  document.addEventListener("load", function (event) {
    var target = event.target;
    if (target instanceof Element && target.matches("img, video, iframe, link[rel='stylesheet']")) {
      invalidateActiveDocumentAuthoringLayout();
    }
  }, true);
  if (document.fonts && typeof document.fonts.addEventListener === "function") {
    document.fonts.addEventListener("loadingdone", invalidateActiveDocumentAuthoringLayout);
  }
