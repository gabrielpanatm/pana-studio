  var previewDragContainerTags = {
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

  function canPreviewDragReceiveChildren(element) {
    return Boolean(element && previewDragContainerTags[element.tagName.toLowerCase()]);
  }

  function dropPositionFromPreviewPointer(event, element) {
    if (isEmptyTeraSlot(element)) return "inside";
    var rect = element.getBoundingClientRect();
    var relativeY = rect.height > 0 ? (event.clientY - rect.top) / rect.height : 0.5;

    if (!canPreviewDragReceiveChildren(element)) {
      return relativeY < 0.5 ? "before" : "after";
    }

    if (relativeY < 0.25) return "before";
    if (relativeY > 0.75) return "after";
    return "inside";
  }

  function previewDropLabel(position) {
    if (position === "before") return "Înainte";
    if (position === "after") return "După";
    return "Copil";
  }

  function previewDragTargetFromPoint(clientX, clientY) {
    var element = document.elementFromPoint(clientX, clientY);
    if (!(element instanceof Element)) return null;
    if (element.id === "pana-studio-preview-drop-line" ||
        element.id === "pana-studio-preview-drop-box" ||
        element.id === "pana-studio-preview-drop-hint" ||
        element.id === HTML_SELECTION_ID ||
        element.id === PREVIEW_HOVER_ID ||
        element.id === TEMPLATE_GATE_ID ||
        element.id === TEMPLATE_GATE_ACTIONS_ID ||
        (element.closest && element.closest("#" + HTML_SELECTION_ID)) ||
        (element.closest && element.closest("#" + PREVIEW_HOVER_ID)) ||
        (element.closest && element.closest("#" + TEMPLATE_GATE_ACTIONS_ID))) {
      return null;
    }
    if (element === document.body || element === document.documentElement) {
      return null;
    }
    return element;
  }

  function previewDragTargetFromPointer(event) {
    return previewDragTargetFromPoint(event.clientX, event.clientY);
  }

  function previewDraggableElementFromTarget(target) {
    if (!(target instanceof Element)) return null;
    if (target.closest("input, textarea, select, [contenteditable='true']")) return null;
    if (target.closest("#" + HTML_SELECTION_ID + ", #" + PREVIEW_HOVER_ID + ", #" + TEMPLATE_GATE_ID + ", #" + TEMPLATE_GATE_ACTIONS_ID)) return null;
    var element = target.closest("body *");
    return element instanceof Element ? element : null;
  }

  function closestPreviewSourceAttribute(element, attributeName) {
    if (!(element instanceof Element)) return null;
    var sourceElement = element.closest("[" + attributeName + "]");
    return sourceElement ? sourceElement.getAttribute(attributeName) : null;
  }

  function previewDropGateStatus(target) {
    var templateSourceId = closestPreviewSourceAttribute(target, TEMPLATE_SOURCE_ID_ATTR);
    if (!templateSourceId || openTeraGateSourceIds[templateSourceId]) {
      return { invalid: false, sourceId: templateSourceId };
    }
    return {
      invalid: true,
      sourceId: templateSourceId,
      message: "Deschide gate-ul Tera înainte de drop."
    };
  }

  function ensurePreviewDragOverlay() {
    var line = document.getElementById("pana-studio-preview-drop-line");
    if (!line) {
      line = document.createElement("div");
      line.id = "pana-studio-preview-drop-line";
      document.body.appendChild(line);
    }

    var box = document.getElementById("pana-studio-preview-drop-box");
    if (!box) {
      box = document.createElement("div");
      box.id = "pana-studio-preview-drop-box";
      document.body.appendChild(box);
    }

    var hint = document.getElementById("pana-studio-preview-drop-hint");
    if (!hint) {
      hint = document.createElement("div");
      hint.id = "pana-studio-preview-drop-hint";
      document.body.appendChild(hint);
    }

    return { line: line, box: box, hint: hint };
  }

  function clearPreviewDragIndicator() {
    var overlay = ensurePreviewDragOverlay();
    overlay.line.style.display = "none";
    overlay.box.style.display = "none";
    overlay.hint.style.display = "none";
    overlay.line.classList.remove("invalid");
    overlay.box.classList.remove("invalid");
    overlay.hint.classList.remove("invalid");
    overlay.line.classList.remove("tera");
    overlay.box.classList.remove("tera");
    overlay.hint.classList.remove("tera");
  }

  function updatePreviewDragIndicator(event, target, position, invalid, message, variant) {
    var overlay = ensurePreviewDragOverlay();
    overlay.line.style.display = "none";
    overlay.box.style.display = "none";

    overlay.hint.style.display = "block";
    overlay.hint.style.left = Math.round(event.clientX + 14) + "px";
    overlay.hint.style.top = Math.round(event.clientY + 14) + "px";
    overlay.hint.textContent = message || (invalid ? "Drop invalid" : previewDropLabel(position));

    overlay.line.classList.toggle("invalid", Boolean(invalid));
    overlay.box.classList.toggle("invalid", Boolean(invalid));
    overlay.hint.classList.toggle("invalid", Boolean(invalid));
    overlay.line.classList.toggle("tera", variant === "tera");
    overlay.box.classList.toggle("tera", variant === "tera");
    overlay.hint.classList.toggle("tera", variant === "tera");

    if (!target) return;

    var rect = target.getBoundingClientRect();
    if (invalid || position === "inside") {
      overlay.box.style.display = "block";
      overlay.box.style.left = Math.round(rect.left) + "px";
      overlay.box.style.top = Math.round(rect.top) + "px";
      overlay.box.style.width = Math.max(12, Math.round(rect.width)) + "px";
      overlay.box.style.height = Math.max(12, Math.round(rect.height)) + "px";
      return;
    }

    overlay.line.style.display = "block";
    overlay.line.style.left = Math.round(rect.left) + "px";
    overlay.line.style.top = Math.round(position === "before" ? rect.top : rect.bottom) + "px";
    overlay.line.style.width = Math.max(24, Math.round(rect.width)) + "px";
  }

  function resetPreviewDragState() {
    previewDragCandidate = null;
    previewDragActive = false;
    previewDragKind = "html";
    previewDragSourceTeraId = null;
    previewDragSourceElement = null;
    previewDragTargetElement = null;
    previewDragPosition = null;
    previewDragInvalid = false;
    document.body.classList.remove("pana-studio-preview-drag-candidate");
    document.body.classList.remove("pana-studio-preview-dragging");
    clearPreviewDragIndicator();
  }

  function normalizedInsertElementPayload(element) {
    var data = element || {};
    var tag = String(data.tag || "div").trim().toLowerCase();
    if (!/^[a-z][a-z0-9-]*$/.test(tag)) tag = "div";
    return {
      id: String(data.id || tag),
      kind: data.kind === "component" ? "component" : "html",
      componentId: typeof data.componentId === "string" ? data.componentId : "",
      componentKind: data.componentKind === "js" ? "js" : data.componentKind === "css" ? "css" : "",
      tag: tag,
      label: String(data.label || tag),
      description: typeof data.description === "string" ? data.description : "",
      text: typeof data.text === "string" ? data.text : "",
      className: typeof data.className === "string" ? data.className : "",
      html: typeof data.html === "string" ? data.html : ""
    };
  }

  function resetPreviewInsertDragState() {
    previewInsertDragActive = false;
    document.body.classList.remove("pana-studio-preview-drag-candidate");
    document.body.classList.remove("pana-studio-preview-dragging");
    clearPreviewDragIndicator();
  }

  var teraConstructKinds = {
    extends: true,
    block: true,
    include: true,
    import: true,
    macro: true,
    "for": true,
    "if": true,
    set: true,
    with: true,
    teraVariable: true,
    teraComment: true,
    raw: true
  };

  function normalizedTeraItemPayload(item) {
    var data = item || {};
    var kind = String(data.kind || "").trim();
    if (!teraConstructKinds[kind]) kind = "block";
    return {
      id: String(data.id || kind),
      kind: kind,
      family: String(data.family || "composition"),
      label: String(data.label || kind),
      description: String(data.description || ""),
      snippet: typeof data.snippet === "string" ? data.snippet : "",
      target: typeof data.target === "string" ? data.target : undefined,
      name: typeof data.name === "string" ? data.name : undefined,
      expression: typeof data.expression === "string" ? data.expression : undefined,
      sourceNodeId: typeof data.sourceNodeId === "string" ? data.sourceNodeId : undefined
    };
  }

  function resetPreviewTeraInsertDragState() {
    previewTeraInsertDragActive = false;
    document.body.classList.remove("pana-studio-preview-drag-candidate");
    document.body.classList.remove("pana-studio-preview-dragging");
    clearPreviewDragIndicator();
  }

  function previewInsertTargetFromData(data) {
    var x = Number(data && data.x);
    var y = Number(data && data.y);
    if (!Number.isFinite(x) || !Number.isFinite(y)) {
      return { target: null, position: null, event: { clientX: 0, clientY: 0 } };
    }
    var target = previewDragTargetFromPoint(x, y);
    var event = { clientX: x, clientY: y };
    var position = target ? dropPositionFromPreviewPointer(event, target) : null;
    return { target: target, position: position, event: event };
  }

  function handlePreviewInsertDragUpdate(data) {
    ensureInspectorStyles();
    previewInsertDragActive = true;
    document.body.classList.add("pana-studio-preview-dragging");

    var element = normalizedInsertElementPayload(data && data.element);
    var drop = previewInsertTargetFromData(data);
    var invalid = !drop.target;
    var gate = invalid ? null : previewDropGateStatus(drop.target);
    if (gate && gate.invalid) invalid = true;
    var message = invalid
      ? (gate && gate.message ? gate.message : "Alege o destinație.")
      : previewDropLabel(drop.position) + " <" + element.tag + ">";
    updatePreviewDragIndicator(drop.event, drop.target, drop.position, invalid, message);
  }

  function handlePreviewInsertDragDrop(data) {
    var element = normalizedInsertElementPayload(data && data.element);
    var drop = previewInsertTargetFromData(data);
    resetPreviewInsertDragState();
    if (!drop.target || !drop.position) return;
    if (previewDropGateStatus(drop.target).invalid) return;

    post("preview-insert-drop", {
      targetSelector: createDomPathSelector(drop.target),
      targetSessionId: closestPreviewSourceAttribute(drop.target, SESSION_ID_ATTR),
      targetSourceId: closestPreviewSourceAttribute(drop.target, SOURCE_ID_ATTR),
      targetTemplateSourceId: closestPreviewSourceAttribute(drop.target, TEMPLATE_SOURCE_ID_ATTR),
      targetSourceLocation: null,
      targetTag: drop.target.tagName.toLowerCase(),
      targetKind: isEmptyTeraSlot(drop.target) ? "empty-tera-slot" : "html",
      position: drop.position,
      element: element
    });
  }

  function handlePreviewTeraDragUpdate(data) {
    ensureInspectorStyles();
    previewTeraInsertDragActive = true;
    document.body.classList.add("pana-studio-preview-dragging");

    var item = normalizedTeraItemPayload(data && data.item);
    var drop = previewInsertTargetFromData(data);
    var invalid = !drop.target;
    var gate = invalid ? null : previewDropGateStatus(drop.target);
    if (gate && gate.invalid) invalid = true;
    var message = invalid
      ? (gate && gate.message ? gate.message : "Alege o destinație Tera.")
      : previewDropLabel(drop.position) + " " + item.label;
    updatePreviewDragIndicator(drop.event, drop.target, drop.position, invalid, message, "tera");
  }

  function handlePreviewTeraDragDrop(data) {
    var item = normalizedTeraItemPayload(data && data.item);
    var drop = previewInsertTargetFromData(data);
    resetPreviewTeraInsertDragState();
    if (!drop.target || !drop.position) return;
    if (previewDropGateStatus(drop.target).invalid) return;

    post("preview-tera-drop", {
      targetSelector: createDomPathSelector(drop.target),
      targetSessionId: closestPreviewSourceAttribute(drop.target, SESSION_ID_ATTR),
      targetSourceId: closestPreviewSourceAttribute(drop.target, SOURCE_ID_ATTR),
      targetTemplateSourceId: closestPreviewSourceAttribute(drop.target, TEMPLATE_SOURCE_ID_ATTR),
      targetTag: drop.target.tagName.toLowerCase(),
      targetKind: isEmptyTeraSlot(drop.target) ? "empty-tera-slot" : "html",
      position: drop.position,
      item: item
    });
  }

  function handlePreviewPointerDown(event) {
    if (!isTrustedPreviewGesture(event)) return;
    if (event.button !== 0) return;
    var current = currentSelectedElement();
    var target = event.target;
    if (!(target instanceof Element)) return;
    var draggable = previewDraggableElementFromTarget(target);
    if (!draggable) return;
    var teraSourceId = renderedTemplateGate && elementBelongsToRenderedTemplateGate(target)
      ? renderedTemplateGate.sourceId
      : null;
    var source = teraSourceId
      ? (current && current.contains(target) ? current : draggable)
      : (current && current.contains(target) ? current : draggable);

    event.preventDefault();
    event.stopPropagation();
    clearTextSelection();
    document.body.classList.add("pana-studio-preview-drag-candidate");
    try {
      target.setPointerCapture(event.pointerId);
    } catch (_error) {}

    if (!teraSourceId && source !== current) {
      selectElement(source);
    }

    previewDragCandidate = {
      kind: teraSourceId ? "tera" : "html",
      sourceId: teraSourceId,
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      source: source
    };
  }

  function handlePreviewPointerMove(event) {
    if (!isTrustedPreviewGesture(event)) return;
    if (!previewDragCandidate || event.pointerId !== previewDragCandidate.pointerId) return;
    var distance = Math.hypot(event.clientX - previewDragCandidate.startX, event.clientY - previewDragCandidate.startY);
    if (!previewDragActive && distance < 6) return;

    event.preventDefault();
    event.stopPropagation();

    if (!previewDragActive) {
      previewDragActive = true;
      previewDragKind = previewDragCandidate.kind || "html";
      previewDragSourceTeraId = previewDragCandidate.sourceId || null;
      previewDragSourceElement = previewDragCandidate.source;
      clearTextSelection();
      document.body.classList.remove("pana-studio-preview-drag-candidate");
      document.body.classList.add("pana-studio-preview-dragging");
    }

    var target = previewDragTargetFromPointer(event);
    var invalid = !target;
    var message = "";
    var position = null;

    if (!invalid && previewDragKind === "html" && (target === previewDragSourceElement || previewDragSourceElement.contains(target))) {
      invalid = true;
      message = "Nu poate fi mutat în propriul copil.";
    }

    if (!invalid && previewDragKind === "tera" && closestPreviewSourceAttribute(target, TEMPLATE_SOURCE_ID_ATTR) === previewDragSourceTeraId) {
      invalid = true;
      message = "Nu poate fi mutat în propriul gate Tera.";
    }

    if (!invalid) {
      var gate = previewDropGateStatus(target);
      if (gate.invalid) {
        invalid = true;
        message = gate.message;
      }
    }

    if (!invalid) {
      position = dropPositionFromPreviewPointer(event, target);
    } else if (!message) {
      message = "Alege alt element.";
    }

    previewDragTargetElement = target;
    previewDragPosition = position;
    previewDragInvalid = invalid;
    updatePreviewDragIndicator(event, target, position, invalid, message, previewDragKind === "tera" ? "tera" : undefined);
  }

  function previewHoverKeyForSelection(selection) {
    return [
      selection.sessionId || "",
      selection.sourceId || "",
      selection.templateSourceId || "",
      selection.domPath || selection.cssSelector || ""
    ].join("|");
  }

  function clearPreviewHoverRequest() {
    if (previewHoverRequestKey === null) return;
    previewHoverRequestKey = null;
    post("preview-hover-clear", {});
  }

  function requestPreviewHoverForElement(element) {
    if (!(element instanceof Element)) {
      clearPreviewHoverRequest();
      return;
    }
    var selection = createSelectionInfo(element);
    var key = previewHoverKeyForSelection(selection);
    if (key === previewHoverRequestKey) return;
    previewHoverRequestKey = key;
    post("preview-hover", { selection: selection });
  }

  function handlePreviewHoverPointerMove(event) {
    if (!isTrustedPreviewGesture(event)) return;
    if (previewDragCandidate || previewDragActive || previewInsertDragActive || previewTeraInsertDragActive) {
      return;
    }
    var eventTarget = event.target;
    if (eventTarget instanceof Element &&
        eventTarget.closest("#" + TEMPLATE_GATE_ACTIONS_ID)) {
      clearPreviewHoverRequest();
      return;
    }
    requestPreviewHoverForElement(previewDragTargetFromPointer(event));
  }

  function handlePreviewHoverPointerLeave() {
    clearPreviewHoverRequest();
  }

  function handlePreviewPointerUp(event) {
    if (!isTrustedPreviewGesture(event)) return;
    if (!previewDragCandidate || event.pointerId !== previewDragCandidate.pointerId) return;
    var wasActive = previewDragActive;
    var source = previewDragSourceElement;
    var sourceKind = previewDragKind;
    var sourceTeraId = previewDragSourceTeraId;
    var target = previewDragTargetElement;
    var position = previewDragPosition;
    var invalid = previewDragInvalid;

    if (wasActive) {
      event.preventDefault();
      event.stopPropagation();
      previewDragSuppressClick = true;
      window.setTimeout(function () {
        previewDragSuppressClick = false;
      }, 120);
    }

    resetPreviewDragState();

    if (!wasActive || invalid || !source || !target || !position) return;

    if (sourceKind === "tera") {
      if (!sourceTeraId) return;
      post("preview-tera-move-drop", {
        sourceId: sourceTeraId,
        targetSelector: createDomPathSelector(target),
        targetSourceId: closestPreviewSourceAttribute(target, SOURCE_ID_ATTR),
        targetTemplateSourceId: closestPreviewSourceAttribute(target, TEMPLATE_SOURCE_ID_ATTR),
        targetTag: target.tagName.toLowerCase(),
        targetKind: "preview",
        targetSlotKind: isEmptyTeraSlot(target) ? "empty-tera-slot" : "html",
        position: position
      });
      return;
    }

    post("preview-layer-drop", {
      sourceSelector: createDomPathSelector(source),
      targetSelector: createDomPathSelector(target),
      sourceSessionId: closestPreviewSourceAttribute(source, SESSION_ID_ATTR),
      sourceSourceId: closestPreviewSourceAttribute(source, SOURCE_ID_ATTR),
      sourceTemplateSourceId: closestPreviewSourceAttribute(source, TEMPLATE_SOURCE_ID_ATTR),
      targetSessionId: closestPreviewSourceAttribute(target, SESSION_ID_ATTR),
      targetSourceId: closestPreviewSourceAttribute(target, SOURCE_ID_ATTR),
      targetTemplateSourceId: closestPreviewSourceAttribute(target, TEMPLATE_SOURCE_ID_ATTR),
      targetKind: isEmptyTeraSlot(target) ? "empty-tera-slot" : "html",
      position: position
    });
  }

  function handlePreviewPointerCancel(event) {
    if (!previewDragCandidate || event.pointerId !== previewDragCandidate.pointerId) return;
    resetPreviewDragState();
  }

  function clearTextSelection() {
    var selection = window.getSelection && window.getSelection();
    if (selection && typeof selection.removeAllRanges === "function") {
      selection.removeAllRanges();
    }
  }

  function handlePreviewSelectStart(event) {
    if (event.target instanceof Element && event.target.closest("input, textarea, select, [contenteditable='true']")) return;
    clearTextSelection();
    event.preventDefault();
    event.stopPropagation();
  }
