  function clearSelectedElement() {
    if (renderedHtmlSelectionElement && renderedHtmlSelectionElement.classList) {
      renderedHtmlSelectionElement.classList.remove(SELECTED_CLASS);
    }
    renderedHtmlSelectionElement = null;
    hideHtmlSelectionOverlay();
  }

  function ensureHtmlSelectionOverlay() {
    var overlay = document.getElementById(HTML_SELECTION_ID);
    if (!overlay) {
      overlay = document.createElement("div");
      overlay.id = HTML_SELECTION_ID;
      overlay.style.cssText = [
        "position: fixed",
        "z-index: 2147483646",
        "display: none",
        "border: 1px solid #1d7f6a",
        "border-radius: 0",
        "background: transparent",
        "box-shadow: none",
        "pointer-events: none",
        "box-sizing: border-box"
      ].join(";");
      document.body.appendChild(overlay);
    }
    return overlay;
  }

  function hideHtmlSelectionOverlay() {
    var overlay = document.getElementById(HTML_SELECTION_ID);
    if (overlay) overlay.style.display = "none";
  }

  function updateHtmlSelectionOverlay() {
    if (!renderedHtmlSelectionElement || !document.contains(renderedHtmlSelectionElement)) {
      hideHtmlSelectionOverlay();
      return;
    }
    var rect = boundsForElements([renderedHtmlSelectionElement]);
    if (!rect) {
      hideHtmlSelectionOverlay();
      return;
    }
    var overlay = ensureHtmlSelectionOverlay();
    overlay.style.display = "block";
    overlay.style.left = Math.round(rect.left) + "px";
    overlay.style.top = Math.round(rect.top) + "px";
    overlay.style.width = Math.round(rect.width) + "px";
    overlay.style.height = Math.round(rect.height) + "px";
    overlay.style.borderRadius = borderRadiusForElements([renderedHtmlSelectionElement]);
  }

  function markSelectedElement(element) {
    if (!(element instanceof Element)) {
      return;
    }

    clearSelectedElement();
    hideTemplateGate();
    renderedHtmlSelectionElement = element;
    renderedHtmlSelectionElement.classList.add(SELECTED_CLASS);
    updateHtmlSelectionOverlay();
  }

  function renderPreviewSelection(selection) {
    var data = selection || {};
    var kind = String(data.kind || "none");

    if (kind === "html") {
      var element = null;
      try {
        element = data.selector ? document.querySelector(String(data.selector)) : null;
      } catch (error) {
        element = null;
      }
      if (element instanceof Element) {
        markSelectedElement(element);
        return;
      }
      clearSelectedElement();
      hideTemplateGate();
      return;
    }

    if (kind === "tera") {
      showTemplateGate(data.selector || null, data.sourceId || null, {
        origin: data.origin || "local",
        themeName: data.themeName || null,
        canSelectHtml: data.canSelectHtml !== false
      });
      return;
    }

    clearSelectedElement();
    hideTemplateGate();
  }

  function requestElementSelection(element) {
    if (!(element instanceof Element)) {
      return;
    }
    post("selection", { selection: createSelectionInfo(element) });
  }

  function selectElement(element) {
    markSelectedElement(element);
    post("selection", { selection: createSelectionInfo(renderedHtmlSelectionElement) });
  }

  function refreshSelection() {
    if (renderedHtmlSelectionElement && document.contains(renderedHtmlSelectionElement)) {
      updateHtmlSelectionOverlay();
      post("selection", { selection: createSelectionInfo(renderedHtmlSelectionElement) });
    }
  }

  function currentSelectedElement() {
    if (renderedHtmlSelectionElement && document.contains(renderedHtmlSelectionElement)) {
      return renderedHtmlSelectionElement;
    }
    var marked = document.querySelector("." + SELECTED_CLASS);
    if (marked instanceof Element) {
      renderedHtmlSelectionElement = marked;
      return renderedHtmlSelectionElement;
    }
    return null;
  }
