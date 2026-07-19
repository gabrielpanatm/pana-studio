  function ensurePreviewHoverOverlay() {
    var overlay = document.getElementById(PREVIEW_HOVER_ID);
    if (!overlay) {
      overlay = document.createElement("div");
      overlay.id = PREVIEW_HOVER_ID;
      overlay.style.cssText = [
        "position: fixed",
        "z-index: 2147483645",
        "display: none",
        "border: 1px dashed #0f766e",
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

  function previewHoverPalette(variant, origin) {
    if (variant === "tera") {
      var sourcePalette = sourceOriginPalette(origin || "local");
      return {
        border: sourcePalette.border,
        background: "transparent",
        shadow: "transparent"
      };
    }
    return {
      border: "#0f766e",
      background: "transparent",
      shadow: "transparent"
    };
  }

  function hidePreviewHover() {
    var overlay = document.getElementById(PREVIEW_HOVER_ID);
    if (overlay) overlay.style.display = "none";
    previewHover = null;
    previewHoverElements = [];
  }

  function previewHoverElementsFor(selector, sourceId, variant) {
    if (variant === "tera") {
      var teraElements = templateGateElements(selector, sourceId);
      if (teraElements.length > 0) return teraElements;
    }
    try {
      var element = selector ? document.querySelector(selector) : null;
      return element ? [element] : [];
    } catch (error) {
      return [];
    }
  }

  function updatePreviewHoverPosition() {
    if (!previewHover) return;
    var elements = previewHoverElements.length > 0
      ? previewHoverElements
      : previewHoverElementsFor(previewHover.selector, previewHover.sourceId, previewHover.variant);
    var rect = boundsForElements(elements);
    if (!rect) {
      hidePreviewHover();
      return;
    }
    var palette = previewHoverPalette(previewHover.variant, previewHover.origin);
    var overlay = ensurePreviewHoverOverlay();
    overlay.style.display = "block";
    overlay.style.borderColor = palette.border;
    overlay.style.background = palette.background;
    overlay.style.boxShadow = "none";
    overlay.style.left = Math.round(rect.left) + "px";
    overlay.style.top = Math.round(rect.top) + "px";
    overlay.style.width = Math.round(rect.width) + "px";
    overlay.style.height = Math.round(rect.height) + "px";
    overlay.style.borderRadius = borderRadiusForElements(elements);
  }

  function showPreviewHover(selector, sourceId, options) {
    var variant = options && options.variant ? String(options.variant) : "html";
    var elements = previewHoverElementsFor(selector, sourceId, variant);
    if (elements.length === 0) {
      hidePreviewHover();
      return;
    }
    previewHover = {
      selector: selector || null,
      sourceId: sourceId || null,
      variant: variant,
      origin: options && options.origin ? String(options.origin) : null
    };
    previewHoverElements = elements;
    updatePreviewHoverPosition();
  }
