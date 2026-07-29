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
    // The Editare sigură document has a single trusted controller: the mounted
    // Pană Studio parent frame. A matching `source` field is only protocol
    // data and must not let sibling/self windows drive live DOM mutations.
    if (event.source !== window.parent) {
      return;
    }
    var data = event.data;
    if (!data || data.source !== SOURCE_APP) {
      return;
    }

    if (data.type === "set-application-appearance") {
      applyApplicationAppearance(data);
      return;
    }

    if (data.type === "activate-canvas-interaction-agent") {
      activateCanvasAgent(data);
      return;
    }

    if (data.type === "deactivate-canvas-interaction-agent") {
      deactivateCanvasAgent(data);
      return;
    }

    if (data.type === "render-canvas-interaction-overlay") {
      renderCanvasAgentOverlay(data);
      return;
    }

    if (data.type === "inspect-canvas-interaction-target") {
      inspectCanvasAgentTarget(data);
      return;
    }

    if (data.type === "clear-canvas-interaction-overlays") {
      clearCanvasAgentOverlays();
      return;
    }

    if (data.type === "sync-structure") {
      runPreviewOperation(data, function () {
        syncStructure();
      });
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
      setLiveStyleCss(data.id || LIVE_OVERRIDES_ID, data.css || "");
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

    if (data.type === "replace-document") {
      runPreviewOperation(data, function () {
        return replaceDocument(
          data.html || "",
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
    "keydown",
    handlePreviewShortcut,
    true
  );

  window.addEventListener("scroll", function () {
    updateCanvasAgentOverlays();
  }, true);
  window.addEventListener("resize", function () {
    updateCanvasAgentOverlays();
  });
