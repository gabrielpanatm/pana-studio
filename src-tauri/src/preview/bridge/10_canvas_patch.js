  var CANVAS_RENDER_INSTANCE_ATTR = "data-pana-render-instance-id";
  var CANVAS_PROJECT_ROOT_ATTR = "data-pana-canvas-project-root";
  var CANVAS_RUNTIME_SESSION_ATTR = "data-pana-canvas-runtime-session-id";
  var CANVAS_WORKSPACE_REVISION_ATTR = "data-pana-canvas-workspace-revision";
  var CANVAS_WORKSPACE_TRANSACTION_ATTR = "data-pana-canvas-workspace-transaction-id";
  var appliedCanvasPatchIds = [];
  var pendingCanvasPatchRollbacks = [];
  var MAX_PENDING_CANVAS_PATCH_ROLLBACKS = 8;

  function canvasCssEscape(value) {
    if (window.CSS && typeof window.CSS.escape === "function") {
      return window.CSS.escape(String(value || ""));
    }
    return String(value || "").replace(/["\\]/g, "\\$&");
  }

  function canvasPatchIdentityMatches(patch) {
    var root = document.documentElement;
    if (!root || !patch || patch.schemaVersion !== 1) return false;
    return root.getAttribute(CANVAS_PROJECT_ROOT_ATTR) === String(patch.projectRoot || "") &&
      root.getAttribute(CANVAS_RUNTIME_SESSION_ATTR) === String(patch.runtimeSessionId || "") &&
      Number(root.getAttribute(CANVAS_WORKSPACE_REVISION_ATTR)) === patch.baseWorkspaceRevision &&
      typeof patch.workspaceTransactionId === "string" && patch.workspaceTransactionId.length > 0 &&
      typeof patch.patchId === "string" && /^canvas_patch_[0-9a-f]{64}$/.test(patch.patchId) &&
      patch.workspaceRevision > patch.baseWorkspaceRevision;
  }

  function canvasPatchElementsForAnchor(anchor) {
    if (!anchor || typeof anchor.sourceId !== "string" || !anchor.sourceId) return [];
    if (anchor.renderInstanceId) {
      return Array.prototype.slice.call(document.querySelectorAll(
        "[" + CANVAS_RENDER_INSTANCE_ATTR + '=\"' + canvasCssEscape(anchor.renderInstanceId) + '\"]'
      ));
    }
    var sourceMatches = Array.prototype.slice.call(document.querySelectorAll(
      "[" + SOURCE_ID_ATTR + '=\"' + canvasCssEscape(anchor.sourceId) + '\"]'
    ));
    if (sourceMatches.length > 0) return sourceMatches;
    if (!anchor.selectorFallback) return [];
    try {
      var fallback = document.querySelector(anchor.selectorFallback);
      return fallback ? [fallback] : [];
    } catch (_) {
      return [];
    }
  }

  function requireCanvasPatchElement(anchor, label) {
    var elements = canvasPatchElementsForAnchor(anchor);
    if (elements.length !== 1) {
      throw new Error("CanvasPatch " + label + " cere exact o instanță randată; găsite " + elements.length + ".");
    }
    var element = elements[0];
    var expectedTag = String(anchor.expectedTag || "").trim().toLowerCase();
    if (expectedTag && element.tagName.toLowerCase() !== expectedTag) {
      throw new Error("CanvasPatch " + label + " nu corespunde tag-ului așteptat.");
    }
    return element;
  }

  function canvasPatchElementFromHtml(html) {
    var template = document.createElement("template");
    template.innerHTML = String(html || "").trim();
    sanitizeDesignSafeTree(template.content);
    if (template.content.children.length !== 1) {
      throw new Error("CanvasPatch insert cere exact un element HTML sigur.");
    }
    return template.content.firstElementChild;
  }

  var activeLiveTextDraft = null;
  var MAX_LIVE_TEXT_DRAFT_LENGTH = 1024 * 1024;

  function liveTextDraftCandidates(attribute, value) {
    if (!value) return [];
    return Array.prototype.slice.call(document.querySelectorAll(
      "[" + attribute + '=\"' + canvasCssEscape(value) + '\"]'
    ));
  }

  function resolveLiveTextDraftTarget(target) {
    if (!target || typeof target !== "object") {
      throw new Error("Draftul live de text nu are țintă.");
    }
    var candidates = liveTextDraftCandidates(SESSION_ID_ATTR, String(target.sessionId || ""));
    if (candidates.length !== 1) {
      candidates = liveTextDraftCandidates(SOURCE_ID_ATTR, String(target.sourceId || ""));
    }
    if (candidates.length !== 1 && target.selector) {
      try {
        candidates = Array.prototype.slice.call(document.querySelectorAll(String(target.selector)));
      } catch (_) {
        candidates = [];
      }
    }
    if (candidates.length !== 1) {
      throw new Error("Draftul live de text cere o singură țintă randată.");
    }
    var element = candidates[0];
    var expectedTag = String(target.expectedTag || "").trim().toLowerCase();
    if (expectedTag && element.tagName.toLowerCase() !== expectedTag) {
      throw new Error("Draftul live de text nu corespunde tag-ului așteptat.");
    }
    if (element.children.length > 0) {
      throw new Error("Draftul live de text a refuzat un element cu copii HTML.");
    }
    return element;
  }

  function applyStoredLiveTextDraft(draft) {
    var element = resolveLiveTextDraftTarget(draft.target);
    element.textContent = draft.text;
    if (element === currentSelectedElement()) updateHtmlSelectionOverlay();
    return element;
  }

  function applyLiveTextDraft(data) {
    var editSessionId = String(data.editSessionId || "");
    var text = String(data.text == null ? "" : data.text);
    if (!/^[A-Za-z0-9_-]{1,128}$/.test(editSessionId)) {
      throw new Error("Draftul live de text are o identitate invalidă.");
    }
    if (text.length > MAX_LIVE_TEXT_DRAFT_LENGTH) {
      throw new Error("Draftul live de text depășește limita sigură.");
    }
    activeLiveTextDraft = {
      editSessionId: editSessionId,
      target: {
        selector: String(data.target && data.target.selector || ""),
        sourceId: String(data.target && data.target.sourceId || ""),
        sessionId: String(data.target && data.target.sessionId || ""),
        expectedTag: String(data.target && data.target.expectedTag || "")
      },
      text: text
    };
    applyStoredLiveTextDraft(activeLiveTextDraft);
  }

  function clearLiveTextDraft(data) {
    if (!activeLiveTextDraft) return;
    var editSessionId = String(data.editSessionId || "");
    if (editSessionId && activeLiveTextDraft.editSessionId !== editSessionId) return;
    activeLiveTextDraft = null;
  }

  function reapplyLiveTextDraft() {
    if (!activeLiveTextDraft) return false;
    try {
      applyStoredLiveTextDraft(activeLiveTextDraft);
      return true;
    } catch (_) {
      return false;
    }
  }

  var activeLiveAttributeDraft = null;

  function resolveLiveAttributeDraftTarget(target) {
    if (!target || typeof target !== "object") {
      throw new Error("Draftul live de atribute nu are țintă.");
    }
    var candidates = liveTextDraftCandidates(SESSION_ID_ATTR, String(target.sessionId || ""));
    if (candidates.length !== 1) {
      candidates = liveTextDraftCandidates(SOURCE_ID_ATTR, String(target.sourceId || ""));
    }
    if (candidates.length !== 1 && target.selector) {
      try {
        candidates = Array.prototype.slice.call(document.querySelectorAll(String(target.selector)));
      } catch (_) {
        candidates = [];
      }
    }
    if (candidates.length !== 1) {
      throw new Error("Draftul live de atribute cere o singură țintă randată.");
    }
    var element = candidates[0];
    var expectedTag = String(target.expectedTag || "").trim().toLowerCase();
    if (expectedTag && element.tagName.toLowerCase() !== expectedTag) {
      throw new Error("Draftul live de atribute nu corespunde tag-ului așteptat.");
    }
    return element;
  }

  function editableLiveAttributeName(name) {
    var normalized = String(name || "").trim().toLowerCase();
    if (
      !normalized
      || normalized === "class"
      || normalized === "style"
      || normalized.indexOf("data-pana-") === 0
      || normalized.indexOf("on") === 0
    ) return null;
    return normalized;
  }

  function applyStoredLiveAttributeDraft(draft) {
    var element = resolveLiveAttributeDraftTarget(draft.target);
    var attributes = draft.attributes || {};
    (draft.baselineNames || []).forEach(function (name) {
      var normalized = editableLiveAttributeName(name);
      if (normalized && !Object.prototype.hasOwnProperty.call(attributes, normalized)) {
        element.removeAttribute(normalized);
      }
    });
    Object.keys(attributes).forEach(function (name) {
      var normalized = editableLiveAttributeName(name);
      if (!normalized) {
        throw new Error("Draftul live a refuzat un atribut intern sau activ.");
      }
      var value = String(attributes[name] == null ? "" : attributes[name]);
      if (!designSafeAttributeAllowed(element, normalized, value)) {
        throw new Error("Draftul live a refuzat valoarea unui atribut nesigur.");
      }
      element.setAttribute(normalized, value);
    });
    if (element === currentSelectedElement()) updateHtmlSelectionOverlay();
    return element;
  }

  function applyLiveAttributeDraft(data) {
    var editSessionId = String(data.editSessionId || "");
    var draftEpoch = Number(data.draftEpoch);
    if (!/^[A-Za-z0-9_-]{1,128}$/.test(editSessionId)) {
      throw new Error("Draftul live de atribute are o identitate invalidă.");
    }
    if (!Number.isSafeInteger(draftEpoch) || draftEpoch <= 0) {
      throw new Error("Draftul live de atribute are un epoch invalid.");
    }
    if (
      activeLiveAttributeDraft
      && activeLiveAttributeDraft.editSessionId === editSessionId
      && activeLiveAttributeDraft.draftEpoch >= draftEpoch
    ) {
      return { editSessionId: editSessionId, draftEpoch: draftEpoch, stale: true };
    }
    var attributes = {};
    Object.keys(data.attributes || {}).forEach(function (name) {
      var normalized = editableLiveAttributeName(name);
      if (!normalized) {
        throw new Error("Draftul live a refuzat un atribut intern sau activ.");
      }
      attributes[normalized] = String(data.attributes[name] == null ? "" : data.attributes[name]);
    });
    var baselineNames = (Array.isArray(data.baselineNames) ? data.baselineNames : [])
      .map(editableLiveAttributeName)
      .filter(Boolean);
    activeLiveAttributeDraft = {
      editSessionId: editSessionId,
      draftEpoch: draftEpoch,
      target: {
        selector: String(data.target && data.target.selector || ""),
        sourceId: String(data.target && data.target.sourceId || ""),
        sessionId: String(data.target && data.target.sessionId || ""),
        expectedTag: String(data.target && data.target.expectedTag || "")
      },
      attributes: attributes,
      baselineNames: baselineNames
    };
    applyStoredLiveAttributeDraft(activeLiveAttributeDraft);
    return { editSessionId: editSessionId, draftEpoch: draftEpoch, stale: false };
  }

  function clearLiveAttributeDraft(data) {
    if (!activeLiveAttributeDraft) return;
    var editSessionId = String(data.editSessionId || "");
    var draftEpoch = Number(data.draftEpoch);
    if (editSessionId && activeLiveAttributeDraft.editSessionId !== editSessionId) return;
    if (Number.isSafeInteger(draftEpoch) && draftEpoch < activeLiveAttributeDraft.draftEpoch) return;
    activeLiveAttributeDraft = null;
  }

  function reapplyLiveAttributeDraft() {
    if (!activeLiveAttributeDraft) return false;
    try {
      applyStoredLiveAttributeDraft(activeLiveAttributeDraft);
      return true;
    } catch (_) {
      return false;
    }
  }

  function canvasPatchReplaceTag(element, newTag) {
    var normalizedTag = String(newTag || "").trim().toLowerCase();
    if (!/^[a-z][a-z0-9-]*$/.test(normalizedTag) || !designSafeElementAllowedName(normalizedTag)) {
      throw new Error("CanvasPatch a refuzat tag-ul nesigur.");
    }
    if (element.tagName.toLowerCase() === normalizedTag) return element;
    var replacement = document.createElement(normalizedTag);
    Array.prototype.forEach.call(element.attributes, function (attribute) {
      if (designSafeAttributeAllowed(replacement, attribute.localName || attribute.name, attribute.value)) {
        replacement.setAttribute(attribute.name, attribute.value);
      }
    });
    while (element.firstChild) replacement.appendChild(element.firstChild);
    element.parentNode.replaceChild(replacement, element);
    return replacement;
  }

  function canvasPatchInsertAt(target, element, position) {
    if (position === "before") target.before(element);
    else if (position === "after") target.after(element);
    else if (position === "inside") target.append(element);
    else throw new Error("CanvasPatch a refuzat poziția structurală.");
  }

  function restoreCanvasAttribute(element, name, value) {
    if (value === null) element.removeAttribute(name);
    else element.setAttribute(name, value);
  }

  function runCanvasPatchRollbacks(rollbacks) {
    for (var index = rollbacks.length - 1; index >= 0; index -= 1) {
      try {
        rollbacks[index]();
      } catch (_) {
        // Continue restoring the remaining local mutations. The caller still
        // reports the original typed failure to the parent runtime.
      }
    }
  }

  function removePendingCanvasPatchRollback(patchId) {
    pendingCanvasPatchRollbacks = pendingCanvasPatchRollbacks.filter(function (entry) {
      return entry.patchId !== patchId;
    });
  }

  function retireCanvasPatchRollbacks() {
    pendingCanvasPatchRollbacks = [];
  }

  function rememberCanvasPatchRollback(entry) {
    removePendingCanvasPatchRollback(entry.patchId);
    pendingCanvasPatchRollbacks.push(entry);
    while (pendingCanvasPatchRollbacks.length > MAX_PENDING_CANVAS_PATCH_ROLLBACKS) {
      pendingCanvasPatchRollbacks.shift();
    }
  }

  function restoreCanvasPatchSelection(previousSelection) {
    refreshEmptyEditableZones();
    if (previousSelection && previousSelection.isConnected) selectElement(previousSelection);
    else clearSelectedElement();
    post("structure", { sections: collectPageSections() });
  }

  function applyCanvasPatch(patch) {
    var patchStartedAt = performance.now();
    if (!canvasPatchIdentityMatches(patch)) {
      throw new Error("CanvasPatch nu corespunde documentului Canvas montat.");
    }
    if (appliedCanvasPatchIds.indexOf(patch.patchId) >= 0) {
      throw new Error("CanvasPatch duplicat refuzat.");
    }
    var operation = patch.operation || {};
    var selected = null;
    var previousSelection = currentSelectedElement();
    var rollbacks = [];
    var root = document.documentElement;
    var basePreviewRevision = root.getAttribute(PREVIEW_REVISION_ATTR) || "";
    var baseCanvasTransactionId = root.getAttribute("data-pana-canvas-transaction-id") || "";

    try {
      if (operation.kind === "setBlockOption") {
        selected = requireCanvasPatchElement(operation.target, "target");
        var providerId = String(operation.providerId || "").trim();
        var optionId = String(operation.optionId || "").trim();
        var optionAttribute = String(operation.attribute || "").trim().toLowerCase();
        if (
          !providerId
          || !optionId
          || !optionAttribute
          || selected.getAttribute("data-pana-block") !== providerId
          || ["data-pana-block", "data-pana-component", "data-pana-instance"].indexOf(optionAttribute) >= 0
          || optionAttribute.indexOf("on") === 0
        ) {
          throw new Error("CanvasPatch a refuzat contractul proprietății de bloc.");
        }
        var previousOptionValue = selected.hasAttribute(optionAttribute)
          ? selected.getAttribute(optionAttribute)
          : null;
        rollbacks.push(function () {
          restoreCanvasAttribute(selected, optionAttribute, previousOptionValue);
        });
        if (operation.value === null) selected.removeAttribute(optionAttribute);
        else selected.setAttribute(optionAttribute, String(operation.value));
      } else if (operation.kind === "setAttributes") {
        selected = requireCanvasPatchElement(operation.target, "target");
        var attributeChanges = Object.keys(operation.attributes || {}).map(function (name) {
          var normalized = String(name || "").trim();
          if (!normalized || normalized.indexOf("data-pana-") === 0 || normalized.indexOf("on") === 0) {
            throw new Error("CanvasPatch a refuzat un atribut intern sau activ.");
          }
          var value = operation.attributes[name];
          if (
            value !== null
            && !designSafeAttributeAllowed(selected, normalized, value)
          ) {
            throw new Error("CanvasPatch a refuzat valoarea unui atribut nesigur.");
          }
          return {
            name: normalized,
            value: value,
            previous: selected.hasAttribute(normalized) ? selected.getAttribute(normalized) : null
          };
        });
        rollbacks.push(function () {
          attributeChanges.forEach(function (change) {
            restoreCanvasAttribute(selected, change.name, change.previous);
          });
        });
        attributeChanges.forEach(function (change) {
          if (change.value === null) {
            selected.removeAttribute(change.name);
          } else {
            selected.setAttribute(change.name, String(change.value));
          }
        });
      } else if (operation.kind === "setText") {
        selected = requireCanvasPatchElement(operation.target, "target");
        var originalChildren = Array.prototype.slice.call(selected.childNodes);
        rollbacks.push(function () {
          while (selected.firstChild) selected.removeChild(selected.firstChild);
          originalChildren.forEach(function (child) { selected.appendChild(child); });
        });
        selected.textContent = String(operation.text || "");
      } else if (operation.kind === "replaceTag") {
        var originalTagElement = requireCanvasPatchElement(operation.target, "target");
        var replacementTagElement = null;
        rollbacks.push(function () {
          if (
            !replacementTagElement
            || replacementTagElement === originalTagElement
            || !replacementTagElement.parentNode
          ) return;
          while (replacementTagElement.firstChild) {
            originalTagElement.appendChild(replacementTagElement.firstChild);
          }
          replacementTagElement.parentNode.replaceChild(originalTagElement, replacementTagElement);
        });
        replacementTagElement = canvasPatchReplaceTag(originalTagElement, operation.newTag);
        selected = replacementTagElement;
      } else if (operation.kind === "insert") {
        var insertTarget = requireCanvasPatchElement(operation.target, "target");
        selected = canvasPatchElementFromHtml(operation.html);
        rollbacks.push(function () { if (selected && selected.parentNode) selected.remove(); });
        canvasPatchInsertAt(insertTarget, selected, operation.position);
      } else if (operation.kind === "duplicate") {
        var duplicateSource = requireCanvasPatchElement(operation.source, "source");
        selected = canvasPatchElementFromHtml(operation.html);
        rollbacks.push(function () { if (selected && selected.parentNode) selected.remove(); });
        duplicateSource.after(selected);
      } else if (operation.kind === "move") {
        var moveSource = requireCanvasPatchElement(operation.source, "source");
        var moveTarget = requireCanvasPatchElement(operation.target, "target");
        if (moveSource === moveTarget || moveSource.contains(moveTarget)) {
          throw new Error("CanvasPatch a refuzat destinația ciclică.");
        }
        var moveParent = moveSource.parentNode;
        var moveNextSibling = moveSource.nextSibling;
        rollbacks.push(function () {
          if (!moveParent) return;
          moveParent.insertBefore(
            moveSource,
            moveNextSibling && moveNextSibling.parentNode === moveParent ? moveNextSibling : null
          );
        });
        canvasPatchInsertAt(moveTarget, moveSource, operation.position);
        selected = moveSource;
      } else if (operation.kind === "delete") {
        var deleteTarget = requireCanvasPatchElement(operation.target, "target");
        var parent = deleteTarget.parentElement;
        var deleteNextSibling = deleteTarget.nextSibling;
        rollbacks.push(function () {
          if (!parent) return;
          parent.insertBefore(
            deleteTarget,
            deleteNextSibling && deleteNextSibling.parentNode === parent ? deleteNextSibling : null
          );
        });
        deleteTarget.remove();
        selected = parent && parent !== document.documentElement && parent !== document.body ? parent : null;
      } else {
        throw new Error("CanvasPatch a refuzat un tip de operație necunoscut.");
      }

      var previousWorkspaceRevision = root.getAttribute(CANVAS_WORKSPACE_REVISION_ATTR);
      var previousWorkspaceTransaction = root.getAttribute(CANVAS_WORKSPACE_TRANSACTION_ATTR);
      rollbacks.push(function () {
        restoreCanvasAttribute(root, CANVAS_WORKSPACE_REVISION_ATTR, previousWorkspaceRevision);
        restoreCanvasAttribute(root, CANVAS_WORKSPACE_TRANSACTION_ATTR, previousWorkspaceTransaction);
      });
      root.setAttribute(CANVAS_WORKSPACE_REVISION_ATTR, String(patch.workspaceRevision));
      root.setAttribute(CANVAS_WORKSPACE_TRANSACTION_ATTR, patch.workspaceTransactionId);

      refreshEmptyEditableZones();
      notifyPanaBlocksInit(document);
      if (selected && selected.isConnected) selectElement(selected);
      else clearSelectedElement();
      post("structure", { sections: collectPageSections() });

      appliedCanvasPatchIds.push(patch.patchId);
      if (appliedCanvasPatchIds.length > 128) appliedCanvasPatchIds.shift();
      rememberCanvasPatchRollback({
        patchId: patch.patchId,
        workspaceRevision: patch.workspaceRevision,
        workspaceTransactionId: patch.workspaceTransactionId,
        basePreviewRevision: basePreviewRevision,
        baseCanvasTransactionId: baseCanvasTransactionId,
        previousSelection: previousSelection,
        rollbacks: rollbacks
      });
      return {
        canvasPatchReceipt: {
          schemaVersion: 1,
          patchId: patch.patchId,
          workspaceRevision: patch.workspaceRevision,
          workspaceTransactionId: patch.workspaceTransactionId,
          bridgeCommitDurationMs: Math.max(0, performance.now() - patchStartedAt)
        }
      };
    } catch (error) {
      runCanvasPatchRollbacks(rollbacks);
      restoreCanvasPatchSelection(previousSelection);
      throw error;
    }
  }

  function rollbackCanvasPatch(patch) {
    if (!patch || typeof patch.patchId !== "string") {
      throw new Error("Rollback CanvasPatch a primit o identitate invalidă.");
    }
    var entry = null;
    for (var index = pendingCanvasPatchRollbacks.length - 1; index >= 0; index -= 1) {
      if (pendingCanvasPatchRollbacks[index].patchId === patch.patchId) {
        entry = pendingCanvasPatchRollbacks[index];
        break;
      }
    }
    var root = document.documentElement;
    if (
      !entry
      || entry.workspaceRevision !== patch.workspaceRevision
      || entry.workspaceTransactionId !== patch.workspaceTransactionId
      || root.getAttribute(PREVIEW_REVISION_ATTR) !== entry.basePreviewRevision
      || root.getAttribute("data-pana-canvas-transaction-id") !== entry.baseCanvasTransactionId
      || Number(root.getAttribute(CANVAS_WORKSPACE_REVISION_ATTR)) !== entry.workspaceRevision
      || root.getAttribute(CANVAS_WORKSPACE_TRANSACTION_ATTR) !== entry.workspaceTransactionId
    ) {
      throw new Error("Rollback CanvasPatch a refuzat un document care nu mai este provizoriu.");
    }
    runCanvasPatchRollbacks(entry.rollbacks);
    removePendingCanvasPatchRollback(entry.patchId);
    appliedCanvasPatchIds = appliedCanvasPatchIds.filter(function (patchId) {
      return patchId !== entry.patchId;
    });
    restoreCanvasPatchSelection(entry.previousSelection);
    return {
      canvasPatchRollbackReceipt: {
        schemaVersion: 1,
        patchId: entry.patchId,
        workspaceRevision: patch.baseWorkspaceRevision,
        workspaceTransactionId: entry.workspaceTransactionId
      }
    };
  }
