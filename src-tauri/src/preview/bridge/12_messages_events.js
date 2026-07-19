  function previewOperationRevision(data) {
    return data && typeof data.previewRevision === "number" && data.previewRevision > 0
      ? data.previewRevision
      : null;
  }

  function canvasFailurePhaseDetails(data, message) {
    if (!data || !data.canvasIdentity) return null;
    return {
      canvasPhaseReceipts: [{
        schemaVersion: 1,
        identity: data.canvasIdentity,
        phase: "failed",
        phaseTimingsMs: { failed: 0 },
        diagnostic: String(message || "Canvas commit failed")
      }]
    };
  }

  function completePreviewOperation(data, ok, error, details) {
    var revision = previewOperationRevision(data);
    if (!revision) return;
    var payload = {
      previewRevision: revision,
      operation: data.type || "",
      ok: ok !== false,
      error: error || null
    };
    if (data.canvasIdentity) payload.canvasIdentity = data.canvasIdentity;
    if (details && details.canvasPhaseReceipts) {
      payload.canvasPhaseReceipts = details.canvasPhaseReceipts;
    }
    if (details && details.canvasPatchReceipt) {
      payload.canvasPatchReceipt = details.canvasPatchReceipt;
    }
    if (details && details.canvasPatchRollbackReceipt) {
      payload.canvasPatchRollbackReceipt = details.canvasPatchRollbackReceipt;
    }
    post("preview-operation-complete", payload);
  }

  function runPreviewOperation(data, callback) {
    var previousRevision = activePreviewOperationRevision;
    activePreviewOperationRevision = previewOperationRevision(data);
    try {
      var result = callback();
      activePreviewOperationRevision = previousRevision;
      if (result && typeof result.then === "function") {
        return result.then(function (details) {
          completePreviewOperation(data, true, null, details || null);
          return details;
        }).catch(function (error) {
          var message = error && error.message ? String(error.message) : String(error || "Eroare preview");
          if (window.console && typeof window.console.error === "function") window.console.error(error);
          completePreviewOperation(data, false, message, canvasFailurePhaseDetails(data, message));
          return null;
        });
      }
      completePreviewOperation(data, true, null, result || null);
      return result;
    } catch (error) {
      activePreviewOperationRevision = previousRevision;
      var message = error && error.message ? String(error.message) : String(error || "Eroare preview");
      if (window.console && typeof window.console.error === "function") {
        window.console.error(error);
      }
      completePreviewOperation(data, false, message, canvasFailurePhaseDetails(data, message));
      return null;
    }
  }

  window.addEventListener("message", function (event) {
    // The Design Safe document has a single trusted controller: the mounted
    // Pană Studio parent frame. A matching `source` field is only protocol
    // data and must not let sibling/self windows drive live DOM mutations.
    if (event.source !== window.parent) {
      return;
    }
    var data = event.data;
    if (!data || data.source !== SOURCE_APP) {
      return;
    }

    if (data.type === "sync-structure") {
      runPreviewOperation(data, function () {
        syncStructure();
      });
      return;
    }

    if (data.type === "set-tera-gate-state") {
      openTeraGateSourceIds = {};
      (data.openGateSourceIds || []).forEach(function (sourceId) {
        if (typeof sourceId === "string" && sourceId) {
          openTeraGateSourceIds[sourceId] = true;
        }
      });
      refreshEmptyEditableZones();
      return;
    }

    if (data.type === "preview-insert-drag-update") {
      handlePreviewInsertDragUpdate(data);
      return;
    }

    if (data.type === "preview-insert-drag-drop") {
      handlePreviewInsertDragDrop(data);
      return;
    }

    if (data.type === "preview-insert-drag-clear") {
      resetPreviewInsertDragState();
      return;
    }

    if (data.type === "preview-tera-drag-update") {
      handlePreviewTeraDragUpdate(data);
      return;
    }

    if (data.type === "preview-tera-drag-drop") {
      handlePreviewTeraDragDrop(data);
      return;
    }

    if (data.type === "preview-tera-drag-clear") {
      resetPreviewTeraInsertDragState();
      return;
    }

    if (data.type === "set-live-overrides-css") {
      setLiveOverridesCss(data.css || "");
      return;
    }

    if (data.type === "set-live-style-css") {
      setLiveStyleCss(data.id || LIVE_OVERRIDES_ID, data.css || "", Boolean(data.refreshSelection));
      return;
    }

    if (data.type === "render-preview-selection") {
      renderPreviewSelection(data.selection);
      return;
    }

    if (data.type === "apply-live-text-draft") {
      runPreviewOperation(data, function () {
        applyLiveTextDraft(data);
      });
      return;
    }

    if (data.type === "clear-live-text-draft") {
      runPreviewOperation(data, function () {
        clearLiveTextDraft(data);
      });
      return;
    }

    if (data.type === "apply-live-attribute-draft") {
      runPreviewOperation(data, function () {
        applyLiveAttributeDraft(data);
      });
      return;
    }

    if (data.type === "clear-live-attribute-draft") {
      runPreviewOperation(data, function () {
        clearLiveAttributeDraft(data);
      });
      return;
    }

    if (data.type === "select-by-selector") {
      var element = data.selector ? document.querySelector(data.selector) : null;
      if (element) {
        selectElement(element);
      }
      return;
    }

    if (data.type === "show-preview-hover") {
      showPreviewHover(data.selector || null, data.sourceId || null, {
        variant: data.variant || "html",
        origin: data.origin || null
      });
      return;
    }

    if (data.type === "clear-preview-hover") {
      previewHoverRequestKey = null;
      hidePreviewHover();
      return;
    }

    if (data.type === "select-markdown-target") {
      var markdownElement = findPreviewElementForMarkdownTarget(data.target);
      if (markdownElement) {
        selectElement(markdownElement);
      }
      return;
    }

    if (data.type === "replace-document") {
      runPreviewOperation(data, function () {
        return replaceDocument(
          data.html || "",
          data.selector || null,
          data.liveCss || "",
          data.canvasIdentity || null
        );
      });
      return;
    }

    if (data.type === "apply-canvas-patch") {
      runPreviewOperation(data, function () {
        return applyCanvasPatch(data.patch);
      });
      return;
    }

    if (data.type === "rollback-canvas-patch") {
      runPreviewOperation(data, function () {
        return rollbackCanvasPatch(data.patch);
      });
      return;
    }

  });

  document.addEventListener(
    "pointerdown",
    handlePreviewPointerDown,
    true
  );

  document.addEventListener(
    "pointermove",
    handlePreviewPointerMove,
    true
  );

  document.addEventListener(
    "pointermove",
    handlePreviewHoverPointerMove,
    true
  );

  document.addEventListener(
    "pointerup",
    handlePreviewPointerUp,
    true
  );

  document.addEventListener(
    "pointercancel",
    handlePreviewPointerCancel,
    true
  );

  document.addEventListener(
    "selectstart",
    handlePreviewSelectStart,
    true
  );

  document.addEventListener(
    "pointerdown",
    function (event) {
      if (!isTrustedPreviewGesture(event)) return;
      if (event.button !== 0) return;
      post("preview-pointerdown", {
        clientX: event.clientX,
        clientY: event.clientY
      });
    },
    true
  );

  var lastPreviewContextMenuAt = 0;
  function openPreviewContextMenuFromEvent(event) {
    if (!isTrustedPreviewGesture(event)) return;
    var now = Date.now();
    if (now - lastPreviewContextMenuAt < (event.type === "contextmenu" ? 350 : 80)) {
      event.preventDefault();
      event.stopPropagation();
      return;
    }

    var target = event.target instanceof Element
      ? event.target
      : (event.target && event.target.parentElement ? event.target.parentElement : null);
    if (!(target instanceof Element)) return;
    if (isStudioOverlayElement(target) || target.closest("#" + HTML_SELECTION_ID + ", #" + PREVIEW_HOVER_ID + ", #" + TEMPLATE_GATE_ID + ", #" + TEMPLATE_GATE_ACTIONS_ID)) return;
    if (target.closest("input, textarea, select, [contenteditable='true']")) return;
    var element = target.closest("body *");
    if (!(element instanceof Element) && target === document.body) element = document.body;
    if (!(element instanceof Element)) return;

    event.preventDefault();
    event.stopPropagation();
    lastPreviewContextMenuAt = now;
    selectElement(element);
    post("preview-context-menu", {
      clientX: event.clientX,
      clientY: event.clientY,
      viewportWidth: window.innerWidth || document.documentElement.clientWidth || 1,
      viewportHeight: window.innerHeight || document.documentElement.clientHeight || 1,
      selection: createSelectionInfo(element)
    });
  }

  document.addEventListener(
    "pointerdown",
    function (event) {
      if (event.button !== 2) return;
      openPreviewContextMenuFromEvent(event);
    },
    true
  );

  document.addEventListener(
    "mousedown",
    function (event) {
      if (event.button !== 2) return;
      openPreviewContextMenuFromEvent(event);
    },
    true
  );

  document.addEventListener(
    "contextmenu",
    function (event) {
      openPreviewContextMenuFromEvent(event);
    },
    true
  );

  document.addEventListener(
    "pointerleave",
    handlePreviewHoverPointerLeave,
    true
  );

  document.addEventListener(
    "keydown",
    function (event) {
      if (!isTrustedPreviewGesture(event)) return;
      if (event.key !== "Delete" && event.key !== "Backspace") return;
      if (event.ctrlKey || event.metaKey || event.altKey) return;
      var active = document.activeElement;
      if (active && active instanceof Element && active.closest("input, textarea, select, [contenteditable='true']")) return;
      var current = currentSelectedElement();
      if (!current || current === document.body || current === document.documentElement) return;
      event.preventDefault();
      event.stopPropagation();
      postDeleteSelected();
    },
    true
  );

  document.addEventListener(
    "keydown",
    handlePreviewShortcut,
    true
  );

  window.addEventListener("scroll", function () {
    updateHtmlSelectionOverlay();
    updateTemplateGatePosition();
    updatePreviewHoverPosition();
  }, true);
  window.addEventListener("resize", function () {
    updateHtmlSelectionOverlay();
    updateTemplateGatePosition();
    updatePreviewHoverPosition();
  });
  window.addEventListener("blur", handlePreviewHoverPointerLeave);

  document.addEventListener(
    "click",
    function (event) {
      if (!isTrustedPreviewGesture(event)) return;
      if (previewDragSuppressClick) {
        event.preventDefault();
        event.stopPropagation();
        if (typeof event.stopImmediatePropagation === "function") {
          event.stopImmediatePropagation();
        }
        previewDragSuppressClick = false;
        return;
      }

      var target = event.target;
      if (!(target instanceof Element)) {
        return;
      }

      if (target.id === TEMPLATE_GATE_ACTIONS_ID || target.closest("#" + TEMPLATE_GATE_ACTIONS_ID)) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();
      hidePreviewHover();
      previewHoverRequestKey = null;
      requestElementSelection(target);
    },
    true
  );
