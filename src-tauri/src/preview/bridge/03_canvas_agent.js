  var CANVAS_AGENT_SOURCE = "pana-studio-canvas-agent";
  var CANVAS_AGENT_SCHEMA_VERSION = 2;
  var CANVAS_AGENT_RENDER_ATTR = "data-pana-render-instance-id";
  var CANVAS_AGENT_HOVER_ID = "pana-studio-canvas-agent-hover";
  var CANVAS_AGENT_SELECTION_ID = "pana-studio-canvas-agent-selection";
  var CANVAS_AGENT_SELECTION_MEMBER_ATTR = "data-pana-selection-member";
  var CANVAS_AGENT_DRAG_ID = "pana-studio-canvas-agent-drag";
  var CANVAS_AGENT_GRID_ID = "pana-studio-canvas-agent-grid";
  var CANVAS_AGENT_ACTION_ATTR = "data-pana-canvas-agent-action";
  var CANVAS_AGENT_HOVER_ATTR = "data-pana-canvas-agent-hover";
  var CANVAS_AGENT_HOVER_STYLE_ID = "pana-studio-canvas-agent-hover-style";
  var CANVAS_AGENT_HOVER_DWELL_MS = 120;
  var canvasAgentInstanceId = createCanvasAgentInstanceId();
  var canvasAgentDocumentEpoch = 0;
  var canvasAgentGestureSequence = 0;
  var canvasAgentSelectionEnabled = false;
  var canvasAgentPendingPointerMove = null;
  var canvasAgentPointerFrame = null;
  var canvasAgentLastPointerHitKey = null;
  var canvasAgentHitElements = new Map();
  var canvasAgentHoverElements = [];
  var canvasAgentHoverPendingEvent = null;
  var canvasAgentHoverFrame = null;
  var canvasAgentHoverTimer = null;
  var canvasAgentDragCandidate = null;
  var canvasAgentDragActive = false;
  var canvasAgentDragSerial = 0;
  var canvasAgentSuppressClick = false;
  var canvasAgentPendingDrop = null;
  var activeCanvasAgentDragPreview = null;
  var canvasAgentGridOverlayEnabled = false;
  var canvasAgentOverlayRequests = {
    hover: null,
    selection: null,
    drag: null
  };

  function createCanvasAgentInstanceId() {
    var bytes = new Uint8Array(16);
    if (!window.crypto || typeof window.crypto.getRandomValues !== "function") {
      return "agent-unavailable-" + String(Date.now());
    }
    window.crypto.getRandomValues(bytes);
    return "agent-" + Array.prototype.map.call(bytes, function (byte) {
      return byte.toString(16).padStart(2, "0");
    }).join("");
  }

  function postCanvasAgent(type, payload) {
    window.parent.postMessage(Object.assign({
      source: CANVAS_AGENT_SOURCE,
      schemaVersion: CANVAS_AGENT_SCHEMA_VERSION,
      type: type,
      agentInstanceId: canvasAgentInstanceId
    }, payload || {}), "*");
  }

  function announceCanvasAgentReady() {
    postCanvasAgent("agentReady", {});
  }

  function canvasAgentSelectionActive() {
    return canvasAgentSelectionEnabled && canvasAgentDocumentEpoch > 0;
  }

  function activateCanvasAgent(data) {
    if (!data || data.schemaVersion !== CANVAS_AGENT_SCHEMA_VERSION) return false;
    if (data.agentInstanceId !== canvasAgentInstanceId) return false;
    var epoch = Number(data.documentEpoch);
    var lastAcceptedSequence = Number(data.lastAcceptedSequence);
    if (!Number.isSafeInteger(epoch) || epoch <= 0) return false;
    if (!Number.isSafeInteger(lastAcceptedSequence) || lastAcceptedSequence < 0) return false;
    if (canvasAgentDocumentEpoch > 0 && epoch < canvasAgentDocumentEpoch) return false;
    if (epoch > canvasAgentDocumentEpoch) {
      canvasAgentGestureSequence = lastAcceptedSequence;
    } else {
      canvasAgentGestureSequence = Math.max(
        canvasAgentGestureSequence,
        lastAcceptedSequence
      );
    }
    canvasAgentDocumentEpoch = epoch;
    canvasAgentSelectionEnabled = Boolean(data.selection);
    configureActiveDocumentAuthoringSurfaces(data.authoringSurfaces);
    clearCanvasAgentOverlays();
    postCanvasAgent("agentActivated", {
      documentEpoch: canvasAgentDocumentEpoch
    });
    return true;
  }

  function deactivateCanvasAgent(data) {
    if (data && data.agentInstanceId && data.agentInstanceId !== canvasAgentInstanceId) {
      return;
    }
    canvasAgentSelectionEnabled = false;
    clearActiveDocumentAuthoringSurfaces();
    clearCanvasAgentHoverDwell();
    canvasAgentPendingPointerMove = null;
    canvasAgentLastPointerHitKey = null;
    if (canvasAgentPointerFrame !== null) {
      window.cancelAnimationFrame(canvasAgentPointerFrame);
      canvasAgentPointerFrame = null;
    }
    clearCanvasAgentDrag();
    clearCanvasAgentOverlays();
  }

  function canvasAgentButton(event) {
    if (!event || typeof event.button !== "number" || event.button < 0) return "none";
    if (event.button === 0) return "primary";
    if (event.button === 1) return "auxiliary";
    if (event.button === 2) return "secondary";
    if (event.button === 3) return "back";
    if (event.button === 4) return "forward";
    return "none";
  }

  function canvasAgentHitPath(event) {
    var rawPath = event && typeof event.composedPath === "function"
      ? event.composedPath()
      : [];
    if (rawPath.length === 0 && event && event.target instanceof Element) {
      var cursor = event.target;
      while (cursor) {
        rawPath.push(cursor);
        cursor = cursor.parentElement;
      }
    }
    var result = [];
    var seen = {};
    var hitElements = new Map();
    var authoringTarget = activeDocumentAuthoringTargetAtPoint(
      Number(event && event.clientX),
      Number(event && event.clientY)
    );
    if (authoringTarget) {
      result.push({
        kind: "boundaryInstance",
        id: authoringTarget.surface.boundaryInstanceId
      });
      if (authoringTarget.surface.renderInstanceId) {
        hitElements.set(authoringTarget.surface.renderInstanceId, authoringTarget.element);
      }
    }
    for (var index = 0; index < rawPath.length && result.length < 64; index += 1) {
      var node = rawPath[index];
      if (!(node instanceof Element) || isStudioOverlayElement(node)) continue;
      var renderInstanceId = node.getAttribute(CANVAS_AGENT_RENDER_ATTR);
      if (!renderInstanceId || renderInstanceId.length > 512 || seen[renderInstanceId]) continue;
      seen[renderInstanceId] = true;
      hitElements.set(renderInstanceId, node);
      result.push({
        kind: "renderInstance",
        id: renderInstanceId
      });
    }
    canvasAgentHitElements = hitElements;
    return result;
  }

  function emitCanvasAgentGesture(event, gesture, emptyHitPath, overrideHitPath, drag) {
    if (!canvasAgentSelectionActive() || !isTrustedPreviewGesture(event)) return;
    if (emptyHitPath) canvasAgentHitElements.clear();
    var hitPath = emptyHitPath ? [] : (overrideHitPath || canvasAgentHitPath(event));
    if (gesture === "pointerMove") {
      var hitKey = hitPath.map(function (candidate) {
        return candidate.kind + ":" + candidate.id;
      }).join("|");
      if (hitKey === canvasAgentLastPointerHitKey) return;
      canvasAgentLastPointerHitKey = hitKey;
    }
    canvasAgentGestureSequence += 1;
    postCanvasAgent("gesture", {
      documentEpoch: canvasAgentDocumentEpoch,
      emittedAtMs: Math.max(1, Math.trunc(Date.now())),
      gestureSequence: canvasAgentGestureSequence,
      gesture: gesture,
      pointer: {
        clientX: Number(event.clientX) || 0,
        clientY: Number(event.clientY) || 0,
        button: canvasAgentButton(event),
        buttons: Number.isSafeInteger(event.buttons) && event.buttons >= 0
          ? Math.min(65535, event.buttons)
          : 0,
        modifiers: {
          alt: event.altKey === true,
          control: event.ctrlKey === true,
          meta: event.metaKey === true,
          shift: event.shiftKey === true
        }
      },
      hitPath: hitPath,
      drag: drag || null
    });
    return canvasAgentGestureSequence;
  }

  function scheduleCanvasAgentPointerGesture(event, gesture, drag) {
    if (!canvasAgentSelectionActive() || !isTrustedPreviewGesture(event)) return;
    canvasAgentPendingPointerMove = {
      event: event,
      gesture: gesture,
      drag: drag || null
    };
    if (canvasAgentPointerFrame !== null) return;
    canvasAgentPointerFrame = window.requestAnimationFrame(function () {
      var pending = canvasAgentPendingPointerMove;
      canvasAgentPendingPointerMove = null;
      canvasAgentPointerFrame = null;
      if (pending) {
        emitCanvasAgentGesture(
          pending.event,
          pending.gesture,
          false,
          null,
          pending.drag
        );
      }
    });
  }

  function canvasAgentDropAxis(target) {
    var parentStyle = target.parentElement
      ? window.getComputedStyle(target.parentElement)
      : null;
    return parentStyle
      && parentStyle.display.indexOf("flex") >= 0
      && parentStyle.flexDirection.indexOf("row") === 0
      ? "horizontal"
      : "vertical";
  }

  function canvasAgentDropPosition(event) {
    var target = event.target instanceof Element
      ? event.target.closest("[" + CANVAS_AGENT_RENDER_ATTR + "]")
      : null;
    if (!(target instanceof Element)) return "inside";
    var rect = target.getBoundingClientRect();
    var horizontal = canvasAgentDropAxis(target) === "horizontal";
    var extent = horizontal ? rect.width : rect.height;
    if (!Number.isFinite(extent) || extent <= 0) return "inside";
    var offset = horizontal ? event.clientX - rect.left : event.clientY - rect.top;
    var ratio = offset / extent;
    if (ratio < 0.25) return "before";
    if (ratio > 0.75) return "after";
    return "inside";
  }

  function handleCanvasAgentPointerMove(event) {
    if (!canvasAgentSelectionActive() || !isTrustedPreviewGesture(event)) return;
    var candidate = canvasAgentDragCandidate;
    if (!candidate || event.pointerId !== candidate.pointerId) return;
    var distance = Math.hypot(
      event.clientX - candidate.startX,
      event.clientY - candidate.startY
    );
    if (!canvasAgentDragActive && distance < 6) return;
    event.preventDefault();
    event.stopPropagation();
    if (!canvasAgentDragActive) {
      canvasAgentDragActive = true;
      emitCanvasAgentGesture(event, "dragStart", false, candidate.hitPath, {
        sessionId: candidate.sessionId,
        position: null
      });
    }
    scheduleCanvasAgentPointerGesture(event, "dragOver", {
      sessionId: candidate.sessionId,
      position: canvasAgentDropPosition(event)
    });
  }

  function handleCanvasAgentPointerOver(event) {
    if (!canvasAgentSelectionActive() || !isTrustedPreviewGesture(event)) return;
    if (canvasAgentDragActive) return;
    scheduleCanvasAgentHoverDwell(event);
  }

  function handleCanvasAgentHoverPointerMove(event) {
    if (
      !canvasAgentSelectionActive()
      || !isTrustedPreviewGesture(event)
      || canvasAgentDragCandidate
      || canvasAgentDragActive
    ) return;
    scheduleCanvasAgentHoverDwell(event);
  }

  function scheduleCanvasAgentHoverDwell(event) {
    canvasAgentHoverPendingEvent = event;
    if (canvasAgentHoverFrame !== null) return;
    canvasAgentHoverFrame = window.requestAnimationFrame(function () {
      canvasAgentHoverFrame = null;
      if (canvasAgentHoverTimer !== null) {
        window.clearTimeout(canvasAgentHoverTimer);
      }
      canvasAgentHoverTimer = window.setTimeout(function () {
        var pending = canvasAgentHoverPendingEvent;
        canvasAgentHoverPendingEvent = null;
        canvasAgentHoverTimer = null;
        if (pending) emitCanvasAgentGesture(pending, "pointerMove", false, null, null);
      }, CANVAS_AGENT_HOVER_DWELL_MS);
    });
  }

  function clearCanvasAgentHoverDwell() {
    canvasAgentHoverPendingEvent = null;
    if (canvasAgentHoverFrame !== null) {
      window.cancelAnimationFrame(canvasAgentHoverFrame);
      canvasAgentHoverFrame = null;
    }
    if (canvasAgentHoverTimer !== null) {
      window.clearTimeout(canvasAgentHoverTimer);
      canvasAgentHoverTimer = null;
    }
  }

  function restoreCanvasAgentDragPreview(retire) {
    var preview = activeCanvasAgentDragPreview;
    if (!preview) return false;
    var source = preview.source;
    var parent = preview.originalParent;
    var nextSibling = preview.originalNextSibling;
    if (source instanceof Element) {
      source.style.pointerEvents = preview.originalPointerEvents;
    }
    if (source instanceof Element && parent instanceof Node) {
      parent.insertBefore(
        source,
        nextSibling && nextSibling.parentNode === parent ? nextSibling : null
      );
    }
    if (retire !== false) activeCanvasAgentDragPreview = null;
    updateCanvasAgentOverlays();
    return true;
  }

  function exactCanvasAgentRenderElement(renderInstanceId) {
    if (
      typeof renderInstanceId !== "string"
      || !renderInstanceId
      || renderInstanceId.length > 512
    ) return null;
    var escaped = window.CSS && typeof window.CSS.escape === "function"
      ? window.CSS.escape(renderInstanceId)
      : renderInstanceId.replace(/["\\]/g, "\\$&");
    var matches = document.querySelectorAll(
      "[" + CANVAS_AGENT_RENDER_ATTR + '="' + escaped + '"]'
    );
    return matches.length === 1 ? matches[0] : null;
  }

  function projectCanvasAgentDragPreview(data) {
    var projection = data && data.projection;
    var pendingDropMatches = canvasAgentPendingDrop
      && data
      && data.dragSessionId === canvasAgentPendingDrop.sessionId
      && data.gestureSequence === canvasAgentPendingDrop.gestureSequence;
    if (
      !data
      || data.agentInstanceId !== canvasAgentInstanceId
      || data.documentEpoch !== canvasAgentDocumentEpoch
      || !pendingDropMatches
      || !Number.isSafeInteger(data.gestureSequence)
      || data.gestureSequence <= 0
      || !Number.isSafeInteger(data.inputEmittedAtMs)
      || data.inputEmittedAtMs <= 0
      || !projection
      || projection.schemaVersion !== 1
      || projection.operation !== "move"
      || projection.scope !== "selectedInstance"
      || !projection.planToken
      || (
        projection.position !== "before"
        && projection.position !== "after"
        && projection.position !== "inside"
      )
    ) return false;
    var source = exactCanvasAgentRenderElement(projection.sourceRenderInstanceId);
    var target = exactCanvasAgentRenderElement(projection.targetRenderInstanceId);
    if (
      !(source instanceof Element)
      || !(target instanceof Element)
      || source === target
      || source.contains(target)
    ) return false;

    var current = activeCanvasAgentDragPreview;
    if (
      current
      && (
        current.dragSessionId !== data.dragSessionId
        || current.source !== source
      )
    ) {
      restoreCanvasAgentDragPreview(true);
      current = null;
    }
    if (!current) {
      var rollback = projection.rollback || {};
      var expectedParent = rollback.sourceParentRenderInstanceId
        ? exactCanvasAgentRenderElement(rollback.sourceParentRenderInstanceId)
        : null;
      var expectedNextSibling = rollback.sourceNextSiblingRenderInstanceId
        ? exactCanvasAgentRenderElement(rollback.sourceNextSiblingRenderInstanceId)
        : null;
      if (
        (rollback.sourceParentRenderInstanceId && source.parentElement !== expectedParent)
        || (
          rollback.sourceNextSiblingRenderInstanceId
          && source.nextElementSibling !== expectedNextSibling
        )
      ) return false;
      current = {
        dragSessionId: data.dragSessionId,
        planToken: projection.planToken,
        gestureSequence: 0,
        source: source,
        originalParent: source.parentNode,
        originalNextSibling: source.nextSibling,
        originalPointerEvents: source.style.pointerEvents
      };
      activeCanvasAgentDragPreview = current;
    }
    if (data.gestureSequence <= current.gestureSequence) return false;
    restoreCanvasAgentDragPreview(false);
    if (projection.position === "before") target.before(source);
    else if (projection.position === "after") target.after(source);
    else target.appendChild(source);
    source.style.pointerEvents = "none";
    current.planToken = projection.planToken;
    current.gestureSequence = data.gestureSequence;
    canvasAgentPendingDrop = null;
    updateCanvasAgentOverlays();
    postCanvasAgent("dragPreviewApplied", {
      documentEpoch: canvasAgentDocumentEpoch,
      dragSessionId: data.dragSessionId,
      gestureSequence: data.gestureSequence,
      planToken: projection.planToken,
      dragPreviewAppliedMs: Math.max(0, Date.now() - data.inputEmittedAtMs)
    });
    return true;
  }

  function cancelCanvasAgentDragPreview(data) {
    if (
      data
      && data.agentInstanceId
      && data.agentInstanceId !== canvasAgentInstanceId
    ) return false;
    if (
      data
      && data.documentEpoch
      && data.documentEpoch !== canvasAgentDocumentEpoch
    ) return false;
    if (
      data
      && data.dragSessionId
      && activeCanvasAgentDragPreview
      && data.dragSessionId !== activeCanvasAgentDragPreview.dragSessionId
    ) return false;
    if (
      data
      && data.dragSessionId
      && canvasAgentPendingDrop
      && data.dragSessionId !== canvasAgentPendingDrop.sessionId
    ) return false;
    canvasAgentPendingDrop = null;
    return restoreCanvasAgentDragPreview(true);
  }

  function clearCanvasAgentDrag(preservePreview) {
    document.removeEventListener("pointermove", handleCanvasAgentPointerMove, true);
    canvasAgentDragCandidate = null;
    canvasAgentDragActive = false;
    hideCanvasAgentDragIndicator();
    if (preservePreview !== true) {
      canvasAgentPendingDrop = null;
      restoreCanvasAgentDragPreview(true);
    }
  }

  function handleCanvasAgentPointerUp(event) {
    var candidate = canvasAgentDragCandidate;
    if (
      !canvasAgentSelectionActive()
      || !isTrustedPreviewGesture(event)
      || !candidate
      || event.pointerId !== candidate.pointerId
    ) return;
    if (!canvasAgentDragActive) {
      clearCanvasAgentDrag();
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    if (canvasAgentPointerFrame !== null) {
      window.cancelAnimationFrame(canvasAgentPointerFrame);
      canvasAgentPointerFrame = null;
      canvasAgentPendingPointerMove = null;
    }
    var dropGestureSequence = emitCanvasAgentGesture(event, "drop", false, null, {
      sessionId: candidate.sessionId,
      position: canvasAgentDropPosition(event)
    });
    if (Number.isSafeInteger(dropGestureSequence) && dropGestureSequence > 0) {
      canvasAgentPendingDrop = {
        sessionId: candidate.sessionId,
        gestureSequence: dropGestureSequence
      };
    }
    canvasAgentSuppressClick = true;
    window.setTimeout(function () {
      canvasAgentSuppressClick = false;
    }, 160);
    clearCanvasAgentDrag(true);
  }

  function boundedCanvasAgentString(value, maximumLength) {
    return typeof value === "string" ? value.slice(0, maximumLength) : "";
  }

  function boundedCanvasAgentRows(value, maximumRows, keyName) {
    if (!Array.isArray(value)) return [];
    return value.slice(0, maximumRows).map(function (entry) {
      var row = {};
      row[keyName] = boundedCanvasAgentString(
        entry && entry[keyName],
        keyName === "selector" ? 2048 : 256
      );
      row.value = boundedCanvasAgentString(entry && entry.value, 2048);
      if (keyName === "selector") {
        row.source = boundedCanvasAgentString(entry && entry.source, 2048);
        row.media = entry && typeof entry.media === "string"
          ? boundedCanvasAgentString(entry.media, 1024)
          : null;
        row.declarations = entry && Number.isSafeInteger(entry.declarations)
          ? Math.max(0, Math.min(100000, entry.declarations))
          : 0;
        row.kind = boundedCanvasAgentString(entry && entry.kind, 128);
        row.score = entry && Number.isFinite(entry.score)
          ? Math.max(-1000000, Math.min(1000000, entry.score))
          : 0;
        delete row.value;
      }
      return row;
    }).filter(function (entry) {
      return Boolean(entry[keyName]);
    });
  }

  function boundedCanvasAgentNodeLink(value) {
    if (!value || typeof value !== "object") return null;
    var tag = boundedCanvasAgentString(value.tag, 64).toLowerCase();
    var selector = boundedCanvasAgentString(value.selector, 4096);
    if (!tag || !selector) return null;
    return {
      tag: tag,
      selector: selector,
      label: boundedCanvasAgentString(value.label, 512)
    };
  }

  function boundedCanvasAgentAttributes(value) {
    if (!value || typeof value !== "object") return {};
    var result = {};
    Object.keys(value).slice(0, 128).forEach(function (name) {
      if (!name || name.toLowerCase().indexOf("data-pana-") === 0) return;
      result[boundedCanvasAgentString(name, 256)] =
        boundedCanvasAgentString(value[name], 4096);
    });
    return result;
  }

  function physicalCanvasAgentObservation(element, renderInstanceId) {
    var observation = createElementObservation(element);
    var blockContext = observation.blockContext;
    var physicalBlockContext = blockContext ? {
      providerId: boundedCanvasAgentString(blockContext.providerId, 256),
      markerKind: blockContext.markerKind === "canonical" ? "canonical" : "legacy",
      rootSelector: boundedCanvasAgentString(blockContext.rootSelector, 4096),
      rootTag: boundedCanvasAgentString(blockContext.rootTag, 64).toLowerCase()
    } : null;
    return {
      selector: boundedCanvasAgentString(observation.selector, 2048),
      cssSelector: boundedCanvasAgentString(observation.cssSelector, 2048),
      domPath: boundedCanvasAgentString(observation.domPath, 4096),
      tag: boundedCanvasAgentString(observation.tag, 64).toLowerCase(),
      id: boundedCanvasAgentString(observation.id, 512),
      href: boundedCanvasAgentString(observation.href, 4096),
      title: boundedCanvasAgentString(observation.title, 2048),
      alt: boundedCanvasAgentString(observation.alt, 2048),
      classes: Array.isArray(observation.classes)
        ? observation.classes.slice(0, 64).map(function (className) {
            return boundedCanvasAgentString(className, 256);
          }).filter(Boolean)
        : [],
      text: boundedCanvasAgentString(observation.text, 512),
      rawText: boundedCanvasAgentString(observation.rawText, 65536),
      hasChildElements: observation.hasChildElements === true,
      rect: {
        width: boundedCanvasAgentString(observation.rect && observation.rect.width, 64),
        height: boundedCanvasAgentString(observation.rect && observation.rect.height, 64),
        top: boundedCanvasAgentString(observation.rect && observation.rect.top, 64),
        left: boundedCanvasAgentString(observation.rect && observation.rect.left, 64)
      },
      styles: boundedCanvasAgentRows(observation.styles, 32, "label"),
      variables: boundedCanvasAgentRows(observation.variables, 256, "name"),
      matchedRules: boundedCanvasAgentRows(observation.matchedRules, 256, "selector"),
      imageSrc: typeof observation.imageSrc === "string"
        ? boundedCanvasAgentString(observation.imageSrc, 4096)
        : null,
      zolaImage: observation.zolaImage || null,
      attributes: boundedCanvasAgentAttributes(observation.attributes),
      parentNode: boundedCanvasAgentNodeLink(observation.parentNode),
      childNodes: Array.isArray(observation.childNodes)
        ? observation.childNodes.slice(0, 24).map(boundedCanvasAgentNodeLink).filter(Boolean)
        : [],
      renderInstanceId: renderInstanceId,
      blockContext: physicalBlockContext
    };
  }

  function inspectCanvasAgentTarget(data) {
    if (!canvasAgentSelectionActive()) return;
    if (!data || data.agentInstanceId !== canvasAgentInstanceId) return;
    if (Number(data.documentEpoch) !== canvasAgentDocumentEpoch) return;
    var inspectionRequestId = data.inspectionRequestId;
    var renderInstanceId = data.renderInstanceId;
    if (
      typeof inspectionRequestId !== "string"
      || !inspectionRequestId
      || inspectionRequestId.length > 128
      || typeof renderInstanceId !== "string"
      || !renderInstanceId
      || renderInstanceId.length > 512
    ) return;
    var selector = "[" + CANVAS_AGENT_RENDER_ATTR + "=\"" +
      escapeCssIdentifier(renderInstanceId) + "\"]";
    var element = document.querySelector(selector);
    if (!(element instanceof Element)) return;
    postCanvasAgent("domInspection", {
      documentEpoch: canvasAgentDocumentEpoch,
      inspectionRequestId: inspectionRequestId,
      renderInstanceId: renderInstanceId,
      observation: physicalCanvasAgentObservation(element, renderInstanceId)
    });
  }

  function canvasAgentProjectionElements(projection) {
    var renderIds = projection && Array.isArray(projection.renderInstanceIds)
      ? projection.renderInstanceIds.slice(0, 4096)
      : [];
    var elements = [];
    var seen = [];
    renderIds.forEach(function (renderInstanceId) {
      if (typeof renderInstanceId !== "string" || !renderInstanceId || renderInstanceId.length > 512) {
        return;
      }
      var element = canvasAgentHitElements.get(renderInstanceId);
      if (
        !(element instanceof Element)
        || !element.isConnected
        || element.getAttribute(CANVAS_AGENT_RENDER_ATTR) !== renderInstanceId
      ) {
        var selector = "[" + CANVAS_AGENT_RENDER_ATTR + "=\"" +
          escapeCssIdentifier(renderInstanceId) + "\"]";
        element = document.querySelector(selector);
      }
      if (!(element instanceof Element) || seen.indexOf(element) >= 0) return;
      seen.push(element);
      elements.push(element);
    });
    if (
      elements.length === 0
      && projection
      && typeof projection.boundaryInstanceId === "string"
    ) {
      var authoringElement = activeDocumentAuthoringElementForBoundary(
        projection.boundaryInstanceId
      );
      if (authoringElement instanceof Element) elements.push(authoringElement);
    }
    return elements;
  }

  function ensureCanvasAgentOverlay(channel) {
    var overlayId = channel === "selection"
      ? CANVAS_AGENT_SELECTION_ID
      : CANVAS_AGENT_HOVER_ID;
    var overlay = document.getElementById(overlayId);
    if (!overlay) {
      overlay = document.createElement("div");
      overlay.id = overlayId;
      overlay.setAttribute("data-pana-canvas-agent-overlay", channel);
      overlay.style.cssText = [
        "position: fixed",
        "z-index: " + (channel === "selection" ? "2147483646" : "2147483645"),
        "display: none",
        "border: 1px " + (channel === "selection" ? "solid" : "dashed") +
          " var(--pana-studio-accent, #1d7f6a)",
        "background: transparent",
        "box-shadow: none",
        "pointer-events: none",
        "box-sizing: border-box"
      ].join(";");
      document.body.appendChild(overlay);
    }
    return overlay;
  }

  function ensureCanvasAgentHoverStyle() {
    var style = document.getElementById(CANVAS_AGENT_HOVER_STYLE_ID);
    if (style) return;
    style = document.createElement("style");
    style.id = CANVAS_AGENT_HOVER_STYLE_ID;
    style.textContent = [
      "[" + CANVAS_AGENT_HOVER_ATTR + "] {",
      "outline: 1px dashed var(--pana-studio-accent, #1d7f6a) !important;",
      "outline-offset: -1px !important;",
      "}",
      "[" + CANVAS_AGENT_HOVER_ATTR + "=\"tera\"] {",
      "outline-color: #3b82f6 !important;",
      "}",
      "[" + CANVAS_AGENT_HOVER_ATTR + "=\"markdown\"] {",
      "outline-color: var(--pana-studio-markdown, #f59e0b) !important;",
      "}"
    ].join("");
    document.head.appendChild(style);
  }

  function clearCanvasAgentHoverTargets() {
    canvasAgentHoverElements.forEach(function (element) {
      if (element instanceof Element) {
        element.removeAttribute(CANVAS_AGENT_HOVER_ATTR);
      }
    });
    canvasAgentHoverElements = [];
  }

  function renderCanvasAgentHover(data) {
    canvasAgentOverlayRequests.hover = data;
    var elements = canvasAgentProjectionElements(data.projection);
    var hoverKind = data.targetKind === "teraBoundary"
      ? "tera"
      : data.targetKind === "markdownBoundary"
        ? "markdown"
        : "html";
    clearCanvasAgentHoverTargets();
    if (elements.length === 0) return;
    ensureCanvasAgentHoverStyle();
    elements.forEach(function (element) {
      element.setAttribute(CANVAS_AGENT_HOVER_ATTR, hoverKind);
    });
    canvasAgentHoverElements = elements;
  }

  function ensureCanvasAgentDragIndicator() {
    var overlay = document.getElementById(CANVAS_AGENT_DRAG_ID);
    if (!overlay) {
      overlay = document.createElement("div");
      overlay.id = CANVAS_AGENT_DRAG_ID;
      overlay.setAttribute("data-pana-canvas-agent-overlay", "drag");
      overlay.style.cssText = [
        "position: fixed",
        "z-index: 2147483647",
        "display: none",
        "pointer-events: none",
        "box-sizing: border-box",
        "border-radius: 3px"
      ].join(";");
      document.body.appendChild(overlay);
    }
    return overlay;
  }

  function hideCanvasAgentDragIndicator() {
    canvasAgentOverlayRequests.drag = null;
    var overlay = document.getElementById(CANVAS_AGENT_DRAG_ID);
    if (overlay) overlay.style.display = "none";
  }

  function canvasAgentPrimaryProjectionElement(projection, elements) {
    var primaryId = projection && projection.primaryRenderInstanceId;
    if (typeof primaryId === "string" && primaryId) {
      for (var index = 0; index < elements.length; index += 1) {
        if (elements[index].getAttribute(CANVAS_AGENT_RENDER_ATTR) === primaryId) {
          return elements[index];
        }
      }
    }
    return elements[0] || null;
  }

  function renderCanvasAgentDragIndicator(data) {
    var position = data && data.dragPosition;
    var activeSessionId = canvasAgentDragCandidate
      ? canvasAgentDragCandidate.sessionId
      : null;
    if (
      !canvasAgentDragActive
      || !activeSessionId
      || data.dragSessionId !== activeSessionId
      || (position !== "before" && position !== "after" && position !== "inside")
    ) {
      hideCanvasAgentDragIndicator();
      return;
    }

    var elements = canvasAgentProjectionElements(data.projection);
    var rect = boundsForElements(elements);
    var primary = canvasAgentPrimaryProjectionElement(data.projection, elements);
    if (!rect || !(primary instanceof Element)) {
      hideCanvasAgentDragIndicator();
      return;
    }

    canvasAgentOverlayRequests.drag = data;
    var overlay = ensureCanvasAgentDragIndicator();
    var isTera = data.targetKind === "teraBoundary";
    var permission = data.dragPermission && typeof data.dragPermission === "object"
      ? data.dragPermission
      : null;
    var permissionState = permission
      && (permission.state === "pending"
        || permission.state === "allowed"
        || permission.state === "blocked")
      ? permission.state
      : "pending";
    var pendingAccent = isTera
      ? "#3b82f6"
      : "var(--pana-studio-accent, #1d7f6a)";
    var accent = permissionState === "allowed"
      ? "#15803d"
      : permissionState === "blocked"
        ? "#dc2626"
        : pendingAccent;
    var fill = permissionState === "allowed"
      ? "rgba(21,128,61,0.10)"
      : permissionState === "blocked"
        ? "rgba(220,38,38,0.10)"
        : isTera
          ? "rgba(59,130,246,0.10)"
          : "color-mix(in srgb, var(--pana-studio-accent, #1d7f6a) 10%, transparent)";
    var axis = canvasAgentDropAxis(primary);
    var edge = position === "before" ? "before" : "after";

    overlay.setAttribute("data-pana-drag-position", position);
    overlay.setAttribute("data-pana-drag-axis", axis);
    overlay.setAttribute("data-pana-drag-permission", permissionState);
    overlay.style.display = "block";
    overlay.style.border = "0";
    overlay.style.background = accent;
    overlay.style.borderRadius = "999px";
    overlay.style.boxShadow = "0 0 0 1px rgba(255,255,255,0.78)";

    if (position === "inside") {
      overlay.style.left = Math.round(rect.left) + "px";
      overlay.style.top = Math.round(rect.top) + "px";
      overlay.style.width = Math.max(12, Math.round(rect.width)) + "px";
      overlay.style.height = Math.max(12, Math.round(rect.height)) + "px";
      overlay.style.border = "2px solid " + accent;
      overlay.style.borderRadius = borderRadiusForElements(elements);
      overlay.style.background = fill;
      overlay.style.boxShadow = "inset 0 0 0 1px rgba(255,255,255,0.55)";
    } else if (axis === "horizontal") {
      overlay.style.left = Math.round(
        edge === "before" ? rect.left - 1 : rect.left + rect.width - 1
      ) + "px";
      overlay.style.top = Math.round(rect.top) + "px";
      overlay.style.width = "3px";
      overlay.style.height = Math.max(12, Math.round(rect.height)) + "px";
    } else {
      overlay.style.left = Math.round(rect.left) + "px";
      overlay.style.top = Math.round(
        edge === "before" ? rect.top - 1 : rect.top + rect.height - 1
      ) + "px";
      overlay.style.width = Math.max(24, Math.round(rect.width)) + "px";
      overlay.style.height = "3px";
    }
  }

  function renderCanvasAgentAction(overlay, data, rect) {
    var action = overlay.querySelector("[" + CANVAS_AGENT_ACTION_ATTR + "]");
    var canEnterBoundary = data
      && data.targetKind === "teraBoundary"
      && data.actions
      && data.actions.canEnterBoundary === true
      && typeof data.editorNodeId === "string"
      && data.editorNodeId
      && Number.isSafeInteger(data.selectionRevision)
      && data.selectionRevision > 0;
    if (!canEnterBoundary) {
      if (action) action.style.display = "none";
      return;
    }
    if (!action) {
      action = document.createElement("button");
      action.type = "button";
      action.setAttribute(CANVAS_AGENT_ACTION_ATTR, "enterBoundary");
      action.textContent = "Editează conținutul";
      action.style.cssText = [
        "position: absolute",
        "left: 0",
        "top: -30px",
        "display: none",
        "height: 26px",
        "padding: 0 9px",
        "border: 1px solid #2563eb",
        "border-radius: 6px",
        "background: #2563eb",
        "color: #fff",
        "font: 12px/24px system-ui, sans-serif",
        "white-space: nowrap",
        "pointer-events: auto",
        "cursor: pointer"
      ].join(";");
      action.addEventListener("click", function (event) {
        if (!isTrustedPreviewGesture(event)) return;
        event.preventDefault();
        event.stopPropagation();
        var request = canvasAgentPrimarySelectionRequest();
        if (
          !request
          || request.targetKind !== "teraBoundary"
          || !request.actions
          || request.actions.canEnterBoundary !== true
        ) return;
        canvasAgentGestureSequence += 1;
        postCanvasAgent("action", {
          documentEpoch: canvasAgentDocumentEpoch,
          actionSequence: canvasAgentGestureSequence,
          selectionRevision: Number(request.selectionRevision),
          editorNodeId: request.editorNodeId,
          action: "enterBoundary"
        });
      });
      overlay.appendChild(action);
    }
    action.style.top = rect && rect.top < 34 ? "2px" : "-30px";
    action.style.display = "block";
  }

  function emitCanvasAgentSelectionAction(action) {
    if (!canvasAgentSelectionActive()) return false;
    var request = canvasAgentPrimarySelectionRequest();
    if (
      !request
      || typeof request.editorNodeId !== "string"
      || !request.editorNodeId
      || !Number.isSafeInteger(request.selectionRevision)
      || request.selectionRevision <= 0
    ) return false;
    canvasAgentGestureSequence += 1;
    postCanvasAgent("action", {
      documentEpoch: canvasAgentDocumentEpoch,
      actionSequence: canvasAgentGestureSequence,
      selectionRevision: Number(request.selectionRevision),
      editorNodeId: request.editorNodeId,
      action: action
    });
    return true;
  }

  function canvasAgentPrimarySelectionRequest() {
    var request = canvasAgentOverlayRequests.selection;
    if (!request || !Array.isArray(request.members)) return request;
    var primaryMemberId = typeof request.primaryMemberId === "string"
      ? request.primaryMemberId
      : "";
    return request.members.find(function (member) {
      return member && member.memberId === primaryMemberId;
    }) || null;
  }

  function ensureCanvasAgentSelectionMemberOverlay(memberId, primary) {
    if (primary) {
      var primaryOverlay = ensureCanvasAgentOverlay("selection");
      primaryOverlay.setAttribute(CANVAS_AGENT_SELECTION_MEMBER_ATTR, memberId);
      return primaryOverlay;
    }
    var overlays = document.querySelectorAll("[" + CANVAS_AGENT_SELECTION_MEMBER_ATTR + "]");
    for (var index = 0; index < overlays.length; index += 1) {
      if (
        overlays[index].id !== CANVAS_AGENT_SELECTION_ID
        && overlays[index].getAttribute(CANVAS_AGENT_SELECTION_MEMBER_ATTR) === memberId
      ) {
        return overlays[index];
      }
    }
    var overlay = document.createElement("div");
    overlay.setAttribute("data-pana-canvas-agent-overlay", "selection-member");
    overlay.setAttribute(CANVAS_AGENT_SELECTION_MEMBER_ATTR, memberId);
    overlay.style.cssText = [
      "position: fixed",
      "z-index: 2147483645",
      "display: none",
      "border: 1px solid var(--pana-studio-accent, #1d7f6a)",
      "background: color-mix(in srgb, var(--pana-studio-accent, #1d7f6a) 4%, transparent)",
      "box-shadow: none",
      "pointer-events: none",
      "box-sizing: border-box"
    ].join(";");
    document.body.appendChild(overlay);
    return overlay;
  }

  function positionCanvasAgentSelectionMember(overlay, member, primary) {
    var elements = canvasAgentProjectionElements(member.projection);
    var rect = boundsForElements(elements);
    if (!rect) {
      overlay.style.display = "none";
      return;
    }
    var isTera = member.targetKind === "teraBoundary";
    var isMarkdown = member.targetKind === "markdownBoundary";
    overlay.style.display = "block";
    overlay.style.borderStyle = primary ? "solid" : "dashed";
    overlay.style.borderWidth = primary ? "2px" : "1px";
    overlay.style.borderColor = isTera
      ? "#3b82f6"
      : isMarkdown
        ? "var(--pana-studio-markdown, #f59e0b)"
        : "var(--pana-studio-accent, #1d7f6a)";
    overlay.style.left = Math.round(rect.left) + "px";
    overlay.style.top = Math.round(rect.top) + "px";
    overlay.style.width = Math.round(rect.width) + "px";
    overlay.style.height = Math.round(rect.height) + "px";
    overlay.style.borderRadius = borderRadiusForElements(elements);
    if (primary) renderCanvasAgentAction(overlay, member, rect);
  }

  function renderCanvasAgentSelectionSet(data) {
    var renderStartedAt = performance.now();
    canvasAgentOverlayRequests.selection = data;
    var activeMemberIds = [];
    var primaryMemberId = typeof data.primaryMemberId === "string"
      ? data.primaryMemberId
      : "";
    data.members.slice(0, 256).forEach(function (member) {
      if (!member || typeof member.memberId !== "string" || !member.memberId) return;
      var primary = member.memberId === primaryMemberId;
      activeMemberIds.push(member.memberId);
      var overlay = ensureCanvasAgentSelectionMemberOverlay(member.memberId, primary);
      positionCanvasAgentSelectionMember(overlay, member, primary);
    });
    document.querySelectorAll("[" + CANVAS_AGENT_SELECTION_MEMBER_ATTR + "]").forEach(function (overlay) {
      var memberId = overlay.getAttribute(CANVAS_AGENT_SELECTION_MEMBER_ATTR) || "";
      if (activeMemberIds.indexOf(memberId) >= 0) return;
      if (overlay.id === CANVAS_AGENT_SELECTION_ID) {
        overlay.removeAttribute(CANVAS_AGENT_SELECTION_MEMBER_ATTR);
        overlay.style.display = "none";
      } else {
        overlay.remove();
      }
    });
    updateCanvasAgentGridOverlay();
    if (typeof data.measurementId === "string" && data.measurementId) {
      postCanvasAgent("selectionOverlayRendered", {
        documentEpoch: canvasAgentDocumentEpoch,
        measurementId: data.measurementId,
        memberCount: activeMemberIds.length,
        renderDurationMs: Math.max(0, performance.now() - renderStartedAt)
      });
    }
  }

  function renderCanvasAgentOverlay(data) {
    if (!canvasAgentSelectionActive()) return;
    if (!data || data.agentInstanceId !== canvasAgentInstanceId) return;
    if (Number(data.documentEpoch) !== canvasAgentDocumentEpoch) return;
    if (data.channel === "drag") {
      renderCanvasAgentDragIndicator(data);
      return;
    }
    var channel = data.channel === "selection" ? "selection" : "hover";
    if (channel === "hover") {
      renderCanvasAgentHover(data);
      return;
    }
    if (Array.isArray(data.members)) {
      renderCanvasAgentSelectionSet(data);
      return;
    }
    document.querySelectorAll("[" + CANVAS_AGENT_SELECTION_MEMBER_ATTR + "]").forEach(function (overlay) {
      if (overlay.id === CANVAS_AGENT_SELECTION_ID) {
        overlay.removeAttribute(CANVAS_AGENT_SELECTION_MEMBER_ATTR);
      } else {
        overlay.remove();
      }
    });
    canvasAgentOverlayRequests[channel] = data;
    var overlay = ensureCanvasAgentOverlay(channel);
    var elements = canvasAgentProjectionElements(data.projection);
    var rect = boundsForElements(elements);
    if (!rect) {
      overlay.style.display = "none";
      return;
    }
    var isTera = data.targetKind === "teraBoundary";
    var isMarkdown = data.targetKind === "markdownBoundary";
    overlay.style.display = "block";
    overlay.style.borderColor = isTera
      ? "#3b82f6"
      : isMarkdown
        ? "var(--pana-studio-markdown, #f59e0b)"
        : "var(--pana-studio-accent, #1d7f6a)";
    overlay.style.left = Math.round(rect.left) + "px";
    overlay.style.top = Math.round(rect.top) + "px";
    overlay.style.width = Math.round(rect.width) + "px";
    overlay.style.height = Math.round(rect.height) + "px";
    overlay.style.borderRadius = borderRadiusForElements(elements);
    if (channel === "selection") renderCanvasAgentAction(overlay, data, rect);
    if (channel === "selection") updateCanvasAgentGridOverlay();
  }

  function canvasGridTrackPixels(value) {
    var tracks = [];
    var pattern = /(-?\d+(?:\.\d+)?)px/g;
    var match;
    while ((match = pattern.exec(String(value || ""))) !== null) {
      var pixels = Number(match[1]);
      if (Number.isFinite(pixels) && pixels >= 0) tracks.push(pixels);
    }
    return tracks;
  }

  function canvasGridPixels(value) {
    var parsed = Number.parseFloat(String(value || "0"));
    return Number.isFinite(parsed) ? parsed : 0;
  }

  function ensureCanvasAgentGridOverlay() {
    var overlay = document.getElementById(CANVAS_AGENT_GRID_ID);
    if (overlay) return overlay;
    overlay = document.createElement("div");
    overlay.id = CANVAS_AGENT_GRID_ID;
    overlay.setAttribute("data-pana-canvas-agent-overlay", "grid");
    overlay.style.cssText = [
      "position: fixed",
      "z-index: 2147483644",
      "display: none",
      "overflow: visible",
      "border: 1px solid #7c3aed",
      "background: transparent",
      "box-shadow: inset 0 0 0 1px rgba(255,255,255,0.42)",
      "pointer-events: none",
      "box-sizing: border-box"
    ].join(";");
    document.body.appendChild(overlay);
    return overlay;
  }

  function appendCanvasGridLine(overlay, axis, offset, crossOffset, length, number) {
    var line = document.createElement("span");
    line.setAttribute("data-pana-grid-line", axis);
    line.style.cssText = [
      "position: absolute",
      axis === "column" ? "left:" + Math.round(offset) + "px" : "top:" + Math.round(offset) + "px",
      axis === "column" ? "top:" + Math.round(crossOffset) + "px" : "left:" + Math.round(crossOffset) + "px",
      axis === "column" ? "width:1px" : "height:1px",
      axis === "column" ? "height:" + Math.max(0, Math.round(length)) + "px" : "width:" + Math.max(0, Math.round(length)) + "px",
      "background:#7c3aed",
      "opacity:.82",
      "pointer-events:none"
    ].join(";");
    var label = document.createElement("small");
    label.textContent = String(number);
    label.style.cssText = [
      "position:absolute",
      axis === "column" ? "left:-8px" : "top:-8px",
      axis === "column" ? "top:-17px" : "left:-17px",
      "display:grid",
      "width:15px",
      "height:15px",
      "place-items:center",
      "border-radius:4px",
      "background:#7c3aed",
      "color:#fff",
      "font:9px/1 system-ui,sans-serif"
    ].join(";");
    line.appendChild(label);
    overlay.appendChild(line);
  }

  function appendCanvasGridGap(overlay, axis, offset, crossOffset, breadth, length) {
    if (!(breadth > 0)) return;
    var gap = document.createElement("span");
    gap.setAttribute("data-pana-grid-gap", axis);
    gap.style.cssText = [
      "position:absolute",
      axis === "column" ? "left:" + Math.round(offset) + "px" : "top:" + Math.round(offset) + "px",
      axis === "column" ? "top:" + Math.round(crossOffset) + "px" : "left:" + Math.round(crossOffset) + "px",
      axis === "column" ? "width:" + Math.round(breadth) + "px" : "height:" + Math.round(breadth) + "px",
      axis === "column" ? "height:" + Math.round(length) + "px" : "width:" + Math.round(length) + "px",
      "background:rgba(124,58,237,.10)",
      "pointer-events:none"
    ].join(";");
    overlay.appendChild(gap);
  }

  function appendCanvasGridAreaLabels(overlay, element, containerRect) {
    Array.prototype.forEach.call(element.children, function (child) {
      if (!(child instanceof Element) || isStudioOverlayElement(child)) return;
      var area = window.getComputedStyle(child).gridArea;
      if (!area || area === "auto" || area.indexOf("auto / auto") === 0 || /^\d/.test(area)) return;
      var rect = child.getBoundingClientRect();
      if (!rect.width || !rect.height) return;
      var label = document.createElement("span");
      label.textContent = area;
      label.setAttribute("data-pana-grid-area-label", area);
      label.style.cssText = [
        "position:absolute",
        "left:" + Math.round(rect.left - containerRect.left + 4) + "px",
        "top:" + Math.round(rect.top - containerRect.top + 4) + "px",
        "max-width:" + Math.max(20, Math.round(rect.width - 8)) + "px",
        "overflow:hidden",
        "padding:2px 5px",
        "border-radius:4px",
        "background:rgba(124,58,237,.88)",
        "color:#fff",
        "font:10px/1.2 system-ui,sans-serif",
        "text-overflow:ellipsis",
        "white-space:nowrap",
        "pointer-events:none"
      ].join(";");
      overlay.appendChild(label);
    });
  }

  function updateCanvasAgentGridOverlay() {
    var overlay = ensureCanvasAgentGridOverlay();
    overlay.replaceChildren();
    if (!canvasAgentGridOverlayEnabled || !canvasAgentSelectionActive()) {
      overlay.style.display = "none";
      return;
    }
    var request = canvasAgentPrimarySelectionRequest();
    var elements = request ? canvasAgentProjectionElements(request.projection) : [];
    var element = canvasAgentPrimaryProjectionElement(request && request.projection, elements);
    if (!(element instanceof Element)) { overlay.style.display = "none"; return; }
    var computed = window.getComputedStyle(element);
    if (computed.display !== "grid" && computed.display !== "inline-grid") {
      overlay.style.display = "none";
      return;
    }
    var rect = element.getBoundingClientRect();
    var columns = canvasGridTrackPixels(computed.gridTemplateColumns);
    var rows = canvasGridTrackPixels(computed.gridTemplateRows);
    var columnGap = canvasGridPixels(computed.columnGap);
    var rowGap = canvasGridPixels(computed.rowGap);
    var startX = canvasGridPixels(computed.borderLeftWidth) + canvasGridPixels(computed.paddingLeft);
    var startY = canvasGridPixels(computed.borderTopWidth) + canvasGridPixels(computed.paddingTop);
    var contentWidth = columns.reduce(function (sum, value) { return sum + value; }, 0) + Math.max(0, columns.length - 1) * columnGap;
    var contentHeight = rows.reduce(function (sum, value) { return sum + value; }, 0) + Math.max(0, rows.length - 1) * rowGap;
    overlay.style.display = "block";
    overlay.style.left = Math.round(rect.left) + "px";
    overlay.style.top = Math.round(rect.top) + "px";
    overlay.style.width = Math.round(rect.width) + "px";
    overlay.style.height = Math.round(rect.height) + "px";
    var x = startX;
    appendCanvasGridLine(overlay, "column", x, startY, contentHeight, 1);
    columns.forEach(function (track, index) {
      x += track;
      appendCanvasGridLine(overlay, "column", x, startY, contentHeight, index + 2);
      if (index < columns.length - 1) {
        appendCanvasGridGap(overlay, "column", x, startY, columnGap, contentHeight);
        x += columnGap;
      }
    });
    var y = startY;
    appendCanvasGridLine(overlay, "row", y, startX, contentWidth, 1);
    rows.forEach(function (track, index) {
      y += track;
      appendCanvasGridLine(overlay, "row", y, startX, contentWidth, index + 2);
      if (index < rows.length - 1) {
        appendCanvasGridGap(overlay, "row", y, startX, rowGap, contentWidth);
        y += rowGap;
      }
    });
    appendCanvasGridAreaLabels(overlay, element, rect);
  }

  function setCanvasAgentGridOverlay(data) {
    canvasAgentGridOverlayEnabled = Boolean(data && data.enabled);
    updateCanvasAgentGridOverlay();
  }

  function clearCanvasAgentOverlays() {
    canvasAgentOverlayRequests.hover = null;
    canvasAgentOverlayRequests.selection = null;
    canvasAgentOverlayRequests.drag = null;
    clearCanvasAgentHoverTargets();
    [CANVAS_AGENT_HOVER_ID, CANVAS_AGENT_SELECTION_ID, CANVAS_AGENT_DRAG_ID, CANVAS_AGENT_GRID_ID].forEach(function (overlayId) {
      var overlay = document.getElementById(overlayId);
      if (overlay) overlay.style.display = "none";
    });
    document.querySelectorAll("[" + CANVAS_AGENT_SELECTION_MEMBER_ATTR + "]").forEach(function (overlay) {
      if (overlay.id === CANVAS_AGENT_SELECTION_ID) {
        overlay.removeAttribute(CANVAS_AGENT_SELECTION_MEMBER_ATTR);
      } else {
        overlay.remove();
      }
    });
  }

  function updateCanvasAgentOverlays() {
    if (!canvasAgentSelectionActive()) return;
    if (canvasAgentOverlayRequests.hover) {
      renderCanvasAgentOverlay(canvasAgentOverlayRequests.hover);
    }
    if (canvasAgentOverlayRequests.selection) {
      renderCanvasAgentOverlay(canvasAgentOverlayRequests.selection);
    }
    if (canvasAgentOverlayRequests.drag) {
      renderCanvasAgentOverlay(canvasAgentOverlayRequests.drag);
    }
    updateCanvasAgentGridOverlay();
  }

  document.addEventListener("pointerover", handleCanvasAgentPointerOver, true);
  document.addEventListener("pointermove", handleCanvasAgentHoverPointerMove, {
    capture: true,
    passive: true
  });
  document.addEventListener("pointerdown", function (event) {
    if (event.target instanceof Element &&
        event.target.closest("[" + CANVAS_AGENT_ACTION_ATTR + "]")) return;
    emitCanvasAgentGesture(event, "pointerDown");
    if (event.button !== 0) return;
    restoreCanvasAgentDragPreview(true);
    canvasAgentPendingDrop = null;
    clearCanvasAgentHoverDwell();
    var hitPath = canvasAgentHitPath(event);
    if (hitPath.length === 0) return;
    if (hitPath[0].kind === "boundaryInstance") return;
    canvasAgentDragSerial += 1;
    canvasAgentDragCandidate = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      hitPath: hitPath,
      sessionId: canvasAgentInstanceId + "-drag-" + String(canvasAgentDragSerial)
    };
    canvasAgentDragActive = false;
    document.removeEventListener("pointermove", handleCanvasAgentPointerMove, true);
    document.addEventListener("pointermove", handleCanvasAgentPointerMove, true);
  }, true);
  document.addEventListener("pointerup", handleCanvasAgentPointerUp, true);
  document.addEventListener("pointercancel", function (event) {
    if (
      canvasAgentDragCandidate
      && event.pointerId === canvasAgentDragCandidate.pointerId
    ) clearCanvasAgentDrag();
  }, true);
  document.addEventListener("click", function (event) {
    if (!canvasAgentSelectionActive() || !isTrustedPreviewGesture(event)) return;
    if (canvasAgentSuppressClick) {
      event.preventDefault();
      event.stopPropagation();
      canvasAgentSuppressClick = false;
      return;
    }
    if (event.target instanceof Element &&
        event.target.closest("[" + CANVAS_AGENT_ACTION_ATTR + "]")) return;
    event.preventDefault();
    event.stopPropagation();
    emitCanvasAgentGesture(event, "click");
  }, true);
  document.addEventListener("contextmenu", function (event) {
    if (!canvasAgentSelectionActive() || !isTrustedPreviewGesture(event)) return;
    if (event.target instanceof Element &&
        event.target.closest("[" + CANVAS_AGENT_ACTION_ATTR + "]")) return;
    event.preventDefault();
    event.stopPropagation();
    emitCanvasAgentGesture(event, "contextMenu");
  }, true);
  document.addEventListener("pointerleave", function (event) {
    if (!canvasAgentSelectionActive() || !isTrustedPreviewGesture(event)) return;
    clearCanvasAgentHoverDwell();
    emitCanvasAgentGesture(event, "pointerMove", true);
  }, true);
  document.addEventListener("keydown", function (event) {
    if (!canvasAgentSelectionActive() || !isTrustedPreviewGesture(event)) return;
    if (event.key !== "Delete" && event.key !== "Backspace") return;
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    var active = document.activeElement;
    if (active instanceof Element &&
        active.closest("input, textarea, select, [contenteditable='true']")) return;
    if (!emitCanvasAgentSelectionAction("deleteSelection")) return;
    event.preventDefault();
    event.stopPropagation();
  }, true);
