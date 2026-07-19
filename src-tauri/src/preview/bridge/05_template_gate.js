  function clearSelectedTemplateSource() {
    renderedTemplateGateElements.forEach(function (element) {
      if (element && element.classList) {
        element.classList.remove(TEMPLATE_SELECTED_CLASS);
      }
    });
    renderedTemplateGateElements = [];
  }

  function ensureTemplateGateOverlay() {
    var overlay = document.getElementById(TEMPLATE_GATE_ID);
    if (!overlay) {
      overlay = document.createElement("div");
      overlay.id = TEMPLATE_GATE_ID;
      overlay.style.cssText = [
        "position: fixed",
        "z-index: 2147483646",
        "display: none",
        "border: 1px solid #3b82f6",
        "border-radius: 0",
        "background: rgba(59,130,246,0.07)",
        "box-shadow: none",
        "pointer-events: none",
        "box-sizing: border-box"
      ].join(";");
      document.body.appendChild(overlay);
    }
    return overlay;
  }

  function templateGatePalette(origin) {
    return sourceOriginPalette(origin || "local");
  }

  function applyTemplateGatePalette(overlay, actions, gate) {
    var palette = templateGatePalette(gate && gate.origin);
    overlay.style.borderColor = palette.border;
    overlay.style.background = palette.background;
    overlay.style.boxShadow = "none";
    actions.style.borderColor = palette.shadow;
    actions.style.boxShadow = "0 10px 26px " + palette.shadow;
    var badge = actions.querySelector("[data-pana-template-gate-badge]");
    if (badge) {
      badge.textContent = palette.label + (gate && gate.themeName ? ": " + gate.themeName : "");
      badge.style.color = palette.text;
      badge.style.background = palette.background;
      badge.style.borderColor = palette.shadow;
    }
    var editButton = actions.querySelector("[data-pana-template-gate-edit]");
    if (editButton) {
      editButton.style.display = !gate || gate.canSelectHtml !== false ? "inline-flex" : "none";
    }
  }

  function templateGateButtonStyle(color, background) {
    return [
      "min-width: 72px",
      "height: 28px",
      "padding: 0 10px",
      "border: 1px solid " + color,
      "border-radius: 7px",
      "color: " + color,
      "background: " + background,
      "font: inherit",
      "cursor: pointer"
    ].join(";");
  }

  function ensureTemplateGateActions() {
    var actions = document.getElementById(TEMPLATE_GATE_ACTIONS_ID);
    if (!actions) {
      actions = document.createElement("div");
      actions.id = TEMPLATE_GATE_ACTIONS_ID;
      actions.setAttribute("role", "toolbar");
      actions.style.cssText = [
        "position: fixed",
        "z-index: 2147483647",
        "display: none",
        "gap: 6px",
        "padding: 5px",
        "border: 1px solid rgba(59,130,246,0.24)",
        "border-radius: 9px",
        "background: rgba(255,255,255,0.96)",
        "box-shadow: 0 10px 26px rgba(30,64,175,0.16)",
        "font: 800 12px/1.2 system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif"
      ].join(";");

      var editButton = document.createElement("button");
      editButton.type = "button";
      editButton.textContent = "Editează";
      editButton.setAttribute("data-pana-template-gate-edit", "true");
      editButton.title = "Deblochează HTML-ul randat pentru editare vizuală";
      editButton.style.cssText = templateGateButtonStyle("#1d4ed8", "rgba(239,246,255,0.98)");
      editButton.addEventListener("click", function (event) {
        if (!isTrustedPreviewGesture(event)) return;
        event.preventDefault();
        event.stopPropagation();
        if (!renderedTemplateGate) return;
        post("preview-template-edit-selected", renderedTemplateGate);
      });

      var badge = document.createElement("span");
      badge.setAttribute("data-pana-template-gate-badge", "true");
      badge.style.cssText = [
        "display: inline-flex",
        "align-items: center",
        "height: 28px",
        "padding: 0 8px",
        "border: 1px solid rgba(59,130,246,0.22)",
        "border-radius: 7px",
        "font: 900 10px/1 system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
        "letter-spacing: 0.06em",
        "text-transform: uppercase",
        "white-space: nowrap"
      ].join(";");

      actions.appendChild(badge);
      actions.appendChild(editButton);
      document.body.appendChild(actions);
    }
    return actions;
  }

  function hideTemplateGate() {
    var overlay = document.getElementById(TEMPLATE_GATE_ID);
    var actions = document.getElementById(TEMPLATE_GATE_ACTIONS_ID);
    if (overlay) overlay.style.display = "none";
    if (actions) actions.style.display = "none";
    clearSelectedTemplateSource();
    renderedTemplateGate = null;
  }

  function semanticTemplateGateRoot(element) {
    var fallback = null;
    var current = element;

    while (current && current !== document.body && current !== document.documentElement) {
      var tag = current.tagName.toLowerCase();
      if (tag === "header" || tag === "footer" || tag === "section" || tag === "article" || tag === "aside") {
        return current;
      }
      if (!fallback && (tag === "main" || tag === "nav")) {
        fallback = current;
      }
      current = current.parentElement;
    }

    return fallback || element;
  }

  function templateGateTopElementsForSource(sourceId) {
    if (!sourceId || !document.body) return [];
    var selector = "[" + TEMPLATE_SOURCE_STACK_ATTR + "~=\"" + escapeCssIdentifier(String(sourceId)) + "\"],"
      + "[" + TEMPLATE_SOURCE_ID_ATTR + "=\"" + escapeCssIdentifier(String(sourceId)) + "\"]";
    var elements = Array.prototype.slice.call(document.querySelectorAll(selector));
    return elements.filter(function (element) {
      var parent = element.parentElement;
      while (parent && parent !== document.body && parent !== document.documentElement) {
        if (elementHasTemplateSource(parent, sourceId)) {
          return false;
        }
        parent = parent.parentElement;
      }
      return true;
    });
  }

  function elementHasTemplateSource(element, sourceId) {
    if (!element || !sourceId) return false;
    if (element.getAttribute(TEMPLATE_SOURCE_ID_ATTR) === String(sourceId)) return true;
    var stack = String(element.getAttribute(TEMPLATE_SOURCE_STACK_ATTR) || "").split(/\s+/);
    return stack.indexOf(String(sourceId)) >= 0;
  }

  function templateGateElements(selector, sourceId) {
    var sourceElements = templateGateTopElementsForSource(sourceId);
    if (sourceElements.length > 0) return sourceElements;

    var element = selector ? document.querySelector(selector) : null;
    return element ? [semanticTemplateGateRoot(element)] : [];
  }

  function elementBelongsToRenderedTemplateGate(element) {
    if (!(element instanceof Element) || !renderedTemplateGate) return false;
    return renderedTemplateGateElements.some(function (gateElement) {
      return gateElement === element || gateElement.contains(element);
    });
  }

  function updateTemplateGatePosition() {
    if (!renderedTemplateGate) return;
    var elements = renderedTemplateGateElements.length > 0
      ? renderedTemplateGateElements
      : templateGateElements(renderedTemplateGate.selector, renderedTemplateGate.sourceId);
    var rect = boundsForElements(elements);
    if (!rect) {
      hideTemplateGate();
      return;
    }
    var overlay = ensureTemplateGateOverlay();
    var actions = ensureTemplateGateActions();
    applyTemplateGatePalette(overlay, actions, renderedTemplateGate);
    overlay.style.display = "block";
    overlay.style.left = Math.round(rect.left) + "px";
    overlay.style.top = Math.round(rect.top) + "px";
    overlay.style.width = Math.round(rect.width) + "px";
    overlay.style.height = Math.round(rect.height) + "px";
    overlay.style.borderRadius = borderRadiusForElements(elements);
    actions.style.display = "flex";
    var actionsWidth = actions.offsetWidth || 122;
    actions.style.left = Math.min(Math.max(8, window.innerWidth - actionsWidth - 8), Math.max(8, Math.round(rect.left))) + "px";
    actions.style.top = Math.max(8, Math.round(rect.top - 42)) + "px";
  }

  function showTemplateGate(selector, sourceId, options) {
    var elements = templateGateElements(selector, sourceId);
    if (elements.length === 0) return;
    var rootSelector = createDomPathSelector(elements[0]);
    clearSelectedElement();
    hideTemplateGate();
    renderedTemplateGate = {
      selector: rootSelector,
      sourceId: sourceId || null,
      origin: options && options.origin ? String(options.origin) : "local",
      themeName: options && options.themeName ? String(options.themeName) : null,
      canSelectHtml: !options || options.canSelectHtml !== false
    };
    renderedTemplateGateElements = elements;
    updateTemplateGatePosition();
  }
