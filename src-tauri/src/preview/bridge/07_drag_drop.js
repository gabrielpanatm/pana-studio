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
    if (isEmptyTeraSlot(element) || isActiveDocumentRoot(element)) return "inside";
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
    var authoringTarget = activeDocumentAuthoringTargetAtPoint(clientX, clientY);
    if (authoringTarget) return authoringTarget.element;
    var element = document.elementFromPoint(clientX, clientY);
    if (!(element instanceof Element)) return null;
    if (element.id === "pana-studio-preview-drop-line" ||
        element.id === "pana-studio-preview-drop-box" ||
        element.id === "pana-studio-preview-drop-hint" ||
        isStudioOverlayElement(element)) {
      return null;
    }
    if (element === document.body || element === document.documentElement) {
      return null;
    }
    return element;
  }

  function closestPreviewSourceAttribute(element, attributeName) {
    if (!(element instanceof Element)) return null;
    var sourceElement = element.closest("[" + attributeName + "]");
    return sourceElement ? sourceElement.getAttribute(attributeName) : null;
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

    var rect = activeDocumentAuthoringRectForElement(target) || target.getBoundingClientRect();
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

  function normalizedInsertElementPayload(element) {
    var data = element || {};
    var kind = data.kind === "html" ? "html" : data.kind === "block" ? "block" : "";
    if (!kind) return null;
    var tag = String(data.tag || "div").trim().toLowerCase();
    if (!/^[a-z][a-z0-9-]*$/.test(tag)) tag = "div";
    var blockId = typeof data.blockId === "string" ? data.blockId.trim() : "";
    var blockKind = data.blockKind === "js" || data.blockKind === "css" || data.blockKind === "static"
      ? data.blockKind
      : "";
    if (kind === "block" && (!blockId || !blockKind)) return null;
    return {
      id: String(data.id || tag),
      kind: kind,
      blockId: blockId,
      blockKind: blockKind,
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
    macroCall: true,
    "for": true,
    "if": true,
    set: true,
    with: true,
    teraVariable: true,
    teraComment: true,
    raw: true,
    dynamicWidget: true
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
      dynamicWidget: data.dynamicWidget && typeof data.dynamicWidget === "object"
        ? data.dynamicWidget
        : undefined,
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
    var invalid = !element || !drop.target;
    var message = invalid
      ? element ? "Alege o destinație." : "Element de inserare invalid."
      : previewDropLabel(drop.position) + " <" + element.tag + ">";
    updatePreviewDragIndicator(drop.event, drop.target, drop.position, invalid, message);
  }

  function handlePreviewInsertDragDrop(data) {
    var element = normalizedInsertElementPayload(data && data.element);
    var drop = previewInsertTargetFromData(data);
    resetPreviewInsertDragState();
    if (!element || !drop.target || !drop.position) return;
    post("preview-insert-drop", {
      targetRenderInstanceId: closestPreviewSourceAttribute(drop.target, CANVAS_AGENT_RENDER_ATTR),
      targetSessionId: closestPreviewSourceAttribute(drop.target, SESSION_ID_ATTR),
      targetSourceId: closestPreviewSourceAttribute(drop.target, SOURCE_ID_ATTR),
      targetTemplateSourceId: closestPreviewSourceAttribute(drop.target, TEMPLATE_SOURCE_ID_ATTR),
      targetBoundaryInstanceId: closestPreviewSourceAttribute(drop.target, ACTIVE_AUTHORING_ATTR),
      targetTag: drop.target.tagName.toLowerCase(),
      targetKind: isActiveDocumentRoot(drop.target)
        ? "active-document-root"
        : isEmptyTeraSlot(drop.target) ? "empty-tera-slot" : "html",
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
    var message = invalid
      ? "Alege o destinație Tera."
      : previewDropLabel(drop.position) + " " + item.label;
    updatePreviewDragIndicator(drop.event, drop.target, drop.position, invalid, message, "tera");
  }

  function handlePreviewTeraDragDrop(data) {
    var item = normalizedTeraItemPayload(data && data.item);
    var drop = previewInsertTargetFromData(data);
    resetPreviewTeraInsertDragState();
    if (!drop.target || !drop.position) return;
    post("preview-tera-drop", {
      targetRenderInstanceId: closestPreviewSourceAttribute(drop.target, CANVAS_AGENT_RENDER_ATTR),
      targetSessionId: closestPreviewSourceAttribute(drop.target, SESSION_ID_ATTR),
      targetSourceId: closestPreviewSourceAttribute(drop.target, SOURCE_ID_ATTR),
      targetTemplateSourceId: closestPreviewSourceAttribute(drop.target, TEMPLATE_SOURCE_ID_ATTR),
      targetBoundaryInstanceId: closestPreviewSourceAttribute(drop.target, ACTIVE_AUTHORING_ATTR),
      targetTag: drop.target.tagName.toLowerCase(),
      targetKind: isActiveDocumentRoot(drop.target)
        ? "active-document-root"
        : isEmptyTeraSlot(drop.target) ? "empty-tera-slot" : "html",
      position: drop.position,
      item: item
    });
  }
