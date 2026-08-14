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
    return sourceMatches;
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
    var rawIconRoots = template.content.querySelectorAll('[data-pana-block="icon"]');
    if (rawIconRoots.length > 0 && (
      rawIconRoots.length !== 1
      || template.content.children.length !== 1
      || template.content.firstElementChild !== rawIconRoots[0]
    )) {
      throw new Error("CanvasPatch Icon insert cere un singur block rădăcină.");
    }
    if (rawIconRoots.length === 1) {
      canvasPatchValidateIconElement(template.content.firstElementChild);
    }
    sanitizeDesignSafeTree(template.content);
    if (template.content.children.length !== 1) {
      throw new Error("CanvasPatch insert cere exact un element HTML sigur.");
    }
    var element = template.content.firstElementChild;
    if (element.getAttribute("data-pana-block") === "icon") {
      canvasPatchValidateIconElement(element);
    }
    return element;
  }

  function applyCanvasPatchInsertedIdentity(element, anchor) {
    if (!element || !anchor) return;
    if (typeof anchor.sourceId === "string" && anchor.sourceId) {
      element.setAttribute(SOURCE_ID_ATTR, anchor.sourceId);
    }
    if (typeof anchor.renderInstanceId === "string" && anchor.renderInstanceId) {
      element.setAttribute(CANVAS_RENDER_INSTANCE_ATTR, anchor.renderInstanceId);
    }
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
    var renderInstanceId = String(target.renderInstanceId || "");
    var sourceId = String(target.sourceId || "");
    if (!renderInstanceId || !sourceId) {
      throw new Error("Draftul live de text cere SelectionAnchor complet.");
    }
    var candidates = liveTextDraftCandidates(CANVAS_RENDER_INSTANCE_ATTR, renderInstanceId);
    if (candidates.length !== 1) {
      throw new Error("Draftul live de text cere o singură țintă randată.");
    }
    var element = candidates[0];
    if (element.getAttribute(SOURCE_ID_ATTR) !== sourceId) {
      throw new Error("Draftul live de text nu corespunde SourceNodeId-ului așteptat.");
    }
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
    updateCanvasAgentOverlays();
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
        sourceId: String(data.target && data.target.sourceId || ""),
        renderInstanceId: String(data.target && data.target.renderInstanceId || ""),
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
    var renderInstanceId = String(target.renderInstanceId || "");
    var sourceId = String(target.sourceId || "");
    if (!renderInstanceId || !sourceId) {
      throw new Error("Draftul live de atribute cere SelectionAnchor complet.");
    }
    var candidates = liveTextDraftCandidates(CANVAS_RENDER_INSTANCE_ATTR, renderInstanceId);
    if (candidates.length !== 1) {
      throw new Error("Draftul live de atribute cere o singură țintă randată.");
    }
    var element = candidates[0];
    if (element.getAttribute(SOURCE_ID_ATTR) !== sourceId) {
      throw new Error("Draftul live de atribute nu corespunde SourceNodeId-ului așteptat.");
    }
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
    updateCanvasAgentOverlays();
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
        sourceId: String(data.target && data.target.sourceId || ""),
        renderInstanceId: String(data.target && data.target.renderInstanceId || ""),
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

  var MANAGED_ICON_ATTRIBUTES = {
    "data-pana-icon": true,
    xmlns: true,
    viewBox: true,
    width: true,
    height: true,
    fill: true,
    stroke: true,
    "stroke-width": true,
    "stroke-linecap": true,
    "stroke-linejoin": true,
    "aria-hidden": true,
    focusable: true,
    role: true,
    "aria-label": true
  };

  function canvasPatchIconAttributes(attributes, iconIdentity) {
    var changes = Object.keys(attributes || {}).map(function (name) {
      if (!MANAGED_ICON_ATTRIBUTES[name]) {
        throw new Error("CanvasPatch Icon a refuzat un atribut neadministrat.");
      }
      var value = attributes[name];
      if (value !== null) value = String(value);
      var valid = value === null;
      if (name === "data-pana-icon") valid = value === iconIdentity;
      else if (name === "xmlns") valid = value === "http://www.w3.org/2000/svg";
      else if (name === "viewBox") valid = value === "0 0 24 24";
      else if (name === "width" || name === "height") {
        valid = value === null || (/^[0-9]{1,3}$/.test(value) && Number(value) >= 8 && Number(value) <= 512);
      } else if (name === "fill") valid = value === null || value === "none";
      else if (name === "stroke") valid = value === null || value === "currentColor";
      else if (name === "stroke-width") {
        valid = value === null || (/^[0-9](?:\.[0-9]{1,2})?$/.test(value) && Number(value) >= 0.5 && Number(value) <= 4);
      } else if (name === "stroke-linecap" || name === "stroke-linejoin") valid = value === null || value === "round";
      else if (name === "focusable") valid = value === null || value === "false";
      else if (name === "aria-hidden") valid = value === null || value === "true";
      else if (name === "role") valid = value === null || value === "img";
      else if (name === "aria-label") valid = value === null || (
        value.trim().length > 0
        && value.length <= 160
        && !/[\u0000-\u001f\u007f]/.test(value)
      );
      if (!valid) throw new Error("CanvasPatch Icon a refuzat valoarea atributului " + name + ".");
      return { name: name, value: value };
    });
    var byName = {};
    changes.forEach(function (change) { byName[change.name] = change.value; });
    if (
      changes.length !== Object.keys(MANAGED_ICON_ATTRIBUTES).length
      || byName["data-pana-icon"] !== iconIdentity
    ) {
      throw new Error("CanvasPatch Icon nu confirmă identitatea în atribute.");
    }
    var decorative = byName["aria-hidden"] === "true";
    var semantic = byName.role === "img" && typeof byName["aria-label"] === "string" && byName["aria-label"].trim();
    if (
      (decorative && (byName.role !== null || byName["aria-label"] !== null))
      || (!decorative && (byName["aria-hidden"] !== null || !semantic))
    ) {
      throw new Error("CanvasPatch Icon a refuzat contractul de accesibilitate.");
    }
    return changes;
  }

  function canvasPatchIconChildren(childrenHtml) {
    var source = String(childrenHtml || "");
    if (!source || source.length > 65536) {
      throw new Error("CanvasPatch Icon a refuzat geometria goală sau supradimensionată.");
    }
    var parsed = new DOMParser().parseFromString(
      '<svg xmlns="http://www.w3.org/2000/svg">' + source + "</svg>",
      "image/svg+xml"
    );
    if (parsed.querySelector("parsererror")) {
      throw new Error("CanvasPatch Icon a refuzat geometria SVG invalidă.");
    }
    var root = parsed.documentElement;
    var nodes = Array.prototype.slice.call(root.children);
    var unsafeChild = Array.prototype.some.call(root.childNodes, function (node) {
      return node.nodeType !== 1 && !(node.nodeType === 3 && !String(node.nodeValue || "").trim());
    });
    if (nodes.length === 0 || nodes.length > 32 || unsafeChild) {
      throw new Error("CanvasPatch Icon cere exclusiv noduri SVG path.");
    }
    nodes.forEach(function (node) {
      if (node.localName !== "path" || node.children.length > 0) {
        throw new Error("CanvasPatch Icon a refuzat un nod SVG nepermis.");
      }
      Array.prototype.forEach.call(node.attributes, function (attribute) {
        var name = attribute.localName || attribute.name;
        var value = attribute.value;
        var allowed = name === "d"
          ? Boolean(value) && /^[ MmAaCcHhLlQqSsTtVvZz0-9+.,-]+$/.test(value)
          : name === "fill" ? value === "currentColor"
          : name === "stroke" ? value === "none"
          : name === "opacity" ? value === ".5"
          : false;
        if (!allowed) throw new Error("CanvasPatch Icon a refuzat un atribut SVG nepermis.");
      });
    });
    return nodes;
  }

  function canvasPatchValidateIconElement(element) {
    if (!element || element.tagName.toLowerCase() !== "svg") {
      throw new Error("CanvasPatch Icon insert cere o rădăcină SVG.");
    }
    Array.prototype.forEach.call(element.attributes, function (attribute) {
      var name = String(attribute.localName || attribute.name || "").toLowerCase();
      if (
        name.indexOf("on") === 0
        || ["href", "xlink:href", "src", "filter", "mask", "clip-path"].indexOf(name) >= 0
      ) {
        throw new Error("CanvasPatch Icon insert a refuzat un atribut activ sau URL.");
      }
    });
    var identity = String(element.getAttribute("data-pana-icon") || "");
    if (
      element.getAttribute("data-pana-block") !== "icon"
      || !element.getAttribute("data-pana-instance")
      || !element.getAttribute("data-anim")
      || !/^tabler-outline:[a-z0-9]+(?:-[a-z0-9]+)*$/.test(identity)
    ) {
      throw new Error("CanvasPatch Icon insert a refuzat identitatea rădăcinii.");
    }
    var attributes = {};
    Object.keys(MANAGED_ICON_ATTRIBUTES).forEach(function (name) {
      attributes[name] = element.hasAttribute(name) ? element.getAttribute(name) : null;
    });
    canvasPatchIconAttributes(attributes, identity);
    canvasPatchIconChildren(element.innerHTML);
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

  function refreshCanvasPatchProjection() {
    refreshEmptyEditableZones();
    updateCanvasAgentOverlays();
    post("structure", { sections: collectPageSections() });
  }

  function applyCanvasPatch(patch) {
    var patchStartedAt = performance.now();
    // CanvasPatch trebuie să-și înregistreze rollback-ul față de DOM-ul
    // canonic de bază, nu față de repoziționarea vizuală din timpul dragului.
    // Restaurarea și reaplicarea patch-ului sunt sincrone, deci browserul nu
    // publică un frame intermediar.
    restoreCanvasAgentDragPreview(true);
    if (!canvasPatchIdentityMatches(patch)) {
      throw new Error("CanvasPatch nu corespunde documentului Canvas montat.");
    }
    if (appliedCanvasPatchIds.indexOf(patch.patchId) >= 0) {
      throw new Error("CanvasPatch duplicat refuzat.");
    }
    var operation = patch.operation || {};
    var selected = null;
    var rollbacks = [];
    var root = document.documentElement;
    var basePreviewRevision = root.getAttribute(PREVIEW_REVISION_ATTR) || "";
    var baseCanvasTransactionId = root.getAttribute("data-pana-canvas-transaction-id") || "";

    try {
      function applyCanvasPatchOperation(operation) {
        var selected = null;
      if (operation.kind === "setIcon") {
        selected = requireCanvasPatchElement(operation.target, "target");
        var iconProviderId = String(operation.providerId || "");
        var iconIdentity = String(operation.iconIdentity || "");
        if (
          iconProviderId !== "icon"
          || selected.tagName.toLowerCase() !== "svg"
          || selected.getAttribute("data-pana-block") !== "icon"
          || !/^tabler-outline:[a-z0-9]+(?:-[a-z0-9]+)*$/.test(iconIdentity)
        ) {
          throw new Error("CanvasPatch Icon a refuzat identitatea țintei.");
        }
        var iconAttributeChanges = canvasPatchIconAttributes(operation.attributes, iconIdentity);
        var replacementIconNodes = canvasPatchIconChildren(operation.childrenHtml);
        var previousIconChildren = Array.prototype.slice.call(selected.childNodes);
        iconAttributeChanges.forEach(function (change) {
          change.previous = selected.hasAttribute(change.name) ? selected.getAttribute(change.name) : null;
        });
        rollbacks.push(function () {
          iconAttributeChanges.forEach(function (change) {
            restoreCanvasAttribute(selected, change.name, change.previous);
          });
          while (selected.firstChild) selected.removeChild(selected.firstChild);
          previousIconChildren.forEach(function (child) { selected.appendChild(child); });
        });
        iconAttributeChanges.forEach(function (change) {
          if (change.value === null) selected.removeAttribute(change.name);
          else selected.setAttribute(change.name, change.value);
        });
        while (selected.firstChild) selected.removeChild(selected.firstChild);
        replacementIconNodes.forEach(function (node) {
          selected.appendChild(document.importNode(node, true));
        });
      } else if (operation.kind === "setBlockOption") {
        selected = requireCanvasPatchElement(operation.target, "target");
        var providerId = String(operation.providerId || "").trim();
        var optionId = String(operation.optionId || "").trim();
        var optionAttribute = String(operation.attribute || "").trim().toLowerCase();
        if (
          !providerId
          || !optionId
          || !optionAttribute
          || selected.getAttribute("data-pana-block") !== providerId
          || ["data-pana-block", "data-pana-instance"].indexOf(optionAttribute) >= 0
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
      } else if (operation.kind === "setTextHtml") {
        selected = requireCanvasPatchElement(operation.target, "target");
        var originalEscapedTextChildren = Array.prototype.slice.call(selected.childNodes);
        rollbacks.push(function () {
          while (selected.firstChild) selected.removeChild(selected.firstChild);
          originalEscapedTextChildren.forEach(function (child) { selected.appendChild(child); });
        });
        var textDecoder = document.createElement("textarea");
        textDecoder.innerHTML = String(operation.escapedText || "");
        selected.textContent = textDecoder.value;
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
        applyCanvasPatchInsertedIdentity(selected, operation.inserted);
        rollbacks.push(function () { if (selected && selected.parentNode) selected.remove(); });
        canvasPatchInsertAt(insertTarget, selected, operation.position);
      } else if (operation.kind === "duplicate") {
        var duplicateSource = requireCanvasPatchElement(operation.source, "source");
        selected = canvasPatchElementFromHtml(operation.html);
        applyCanvasPatchInsertedIdentity(selected, operation.inserted);
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

        return selected;
      }

      if (operation.kind === "batch") {
        if (
          !Array.isArray(operation.operations)
          || operation.operations.length < 1
          || operation.operations.length > 256
          || operation.operations.some(function (item) {
            return !item || typeof item !== "object" || item.kind === "batch";
          })
        ) {
          throw new Error("CanvasPatch batch a refuzat lista de operații.");
        }
        operation.operations.forEach(function (item) {
          selected = applyCanvasPatchOperation(item);
        });
      } else {
        selected = applyCanvasPatchOperation(operation);
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
      updateCanvasAgentOverlays();
      post("structure", { sections: collectPageSections() });

      appliedCanvasPatchIds.push(patch.patchId);
      if (appliedCanvasPatchIds.length > 128) appliedCanvasPatchIds.shift();
      rememberCanvasPatchRollback({
        patchId: patch.patchId,
        workspaceRevision: patch.workspaceRevision,
        workspaceTransactionId: patch.workspaceTransactionId,
        basePreviewRevision: basePreviewRevision,
        baseCanvasTransactionId: baseCanvasTransactionId,
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
      refreshCanvasPatchProjection();
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
    refreshCanvasPatchProjection();
    return {
      canvasPatchRollbackReceipt: {
        schemaVersion: 1,
        patchId: entry.patchId,
        workspaceRevision: patch.baseWorkspaceRevision,
        workspaceTransactionId: entry.workspaceTransactionId
      }
    };
  }
