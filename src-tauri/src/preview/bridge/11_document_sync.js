  function normalizeSearchText(text) {
    return (text || "").replace(/\s+/g, " ").trim();
  }

  function findPreviewElementForMarkdownTarget(target) {
    if (!target || !target.kind) return null;
    var normalizedTarget = normalizeSearchText(target.text);
    var candidates;
    if (target.kind === "heading") {
      candidates = document.querySelectorAll("h1, h2, h3, h4, h5, h6");
    } else if (target.kind === "link") {
      candidates = document.querySelectorAll("a");
    } else {
      candidates = document.querySelectorAll("p, li, blockquote, figcaption, span, div");
    }
    for (var i = 0; i < candidates.length; i += 1) {
      if (target.kind === "link") {
        var href = (candidates[i].getAttribute("href") || "").trim();
        if (normalizeSearchText(candidates[i].textContent) === normalizedTarget && (!target.href || href === target.href)) {
          return candidates[i];
        }
      } else {
        var content = normalizeSearchText(candidates[i].textContent);
        if (target.kind === "heading" ? content === normalizedTarget : (content.length > 0 && content.indexOf(normalizedTarget) >= 0)) {
          return candidates[i];
        }
      }
    }
    return null;
  }

  function replaceCanonicalAttributes(target, source, preserveInternal) {
    if (!(target instanceof Element) || !(source instanceof Element)) return;
    var preserved = {};
    if (preserveInternal) {
      [SESSION_ID_ATTR, "data-pana-internal-style"].forEach(function (name) {
        if (target.hasAttribute(name)) preserved[name] = target.getAttribute(name) || "";
      });
    }
    Array.prototype.slice.call(target.attributes || []).forEach(function (attribute) {
      target.removeAttribute(attribute.name);
    });
    Array.prototype.slice.call(source.attributes || []).forEach(function (attribute) {
      target.setAttribute(attribute.name, attribute.value || "");
    });
    Object.keys(preserved).forEach(function (name) {
      target.setAttribute(name, preserved[name]);
    });
  }

  function isInternalCanvasNode(node) {
    if (!(node instanceof Element)) return false;
    if (node === INTERNAL_BRIDGE_ELEMENT) return true;
    if (node.hasAttribute("data-pana-internal-style")) return true;
    return node.tagName === "STYLE" && String(node.id || "").indexOf("pana-") === 0;
  }

  function keyedChildKey(node, occurrenceByBase) {
    if (node.nodeType === 3) return "text";
    if (node.nodeType === 8) return "comment";
    if (!(node instanceof Element)) return "node:" + node.nodeType;
    var explicit = node.getAttribute("data-pana-render-instance-id");
    if (explicit) return "render:" + explicit;
    var source = node.getAttribute(SOURCE_ID_ATTR) || node.getAttribute(TEMPLATE_SOURCE_ID_ATTR);
    var base = source
      ? "source:" + source + ":" + node.localName
      : (node.id ? "id:" + node.id : "tag:" + node.localName);
    var occurrence = occurrenceByBase[base] || 0;
    occurrenceByBase[base] = occurrence + 1;
    return base + ":" + occurrence;
  }

  function keyedChildren(parent) {
    var occurrences = {};
    return Array.prototype.slice.call(parent.childNodes || []).map(function (node) {
      return { node: node, key: keyedChildKey(node, occurrences) };
    });
  }

  function reconcileNode(target, source) {
    if (target.nodeType !== source.nodeType) return document.importNode(source, true);
    if (target.nodeType === 3 || target.nodeType === 8) {
      if (target.nodeValue !== source.nodeValue) target.nodeValue = source.nodeValue;
      return target;
    }
    if (!(target instanceof Element) || !(source instanceof Element) || target.localName !== source.localName) {
      return document.importNode(source, true);
    }
    replaceCanonicalAttributes(target, source, true);
    reconcileChildren(target, source, false);
    return target;
  }

  function reconcileChildren(targetParent, sourceParent, preserveInternal) {
    var existing = keyedChildren(targetParent);
    var available = {};
    existing.forEach(function (entry) {
      if (preserveInternal && isInternalCanvasNode(entry.node)) return;
      if (!available[entry.key]) available[entry.key] = [];
      available[entry.key].push(entry.node);
    });
    var desired = keyedChildren(sourceParent);
    var cursor = targetParent.firstChild;
    desired.forEach(function (entry) {
      while (cursor && preserveInternal && isInternalCanvasNode(cursor)) cursor = cursor.nextSibling;
      var bucket = available[entry.key] || [];
      var candidate = bucket.shift() || null;
      var next = candidate ? reconcileNode(candidate, entry.node) : document.importNode(entry.node, true);
      if (next !== candidate && candidate && candidate.parentNode === targetParent) {
        targetParent.replaceChild(next, candidate);
      }
      if (next !== cursor) targetParent.insertBefore(next, cursor || null);
      cursor = next.nextSibling;
    });
    Object.keys(available).forEach(function (key) {
      available[key].forEach(function (node) {
        if ((!preserveInternal || !isInternalCanvasNode(node)) && node.parentNode === targetParent) {
          node.remove();
        }
      });
    });
  }

  function stylesheetKey(link) {
    if (!(link instanceof Element) || link.localName !== "link") return null;
    var rel = String(link.getAttribute("rel") || "").toLowerCase().split(/\s+/);
    if (rel.indexOf("stylesheet") < 0) return null;
    try {
      return new URL(link.getAttribute("href") || "", document.baseURI).href;
    } catch (_) {
      return link.getAttribute("href") || "";
    }
  }

  function waitForStylesheet(link) {
    return new Promise(function (resolve, reject) {
      if (link.sheet) {
        resolve();
        return;
      }
      var settled = false;
      var timer = window.setTimeout(function () {
        if (settled) return;
        settled = true;
        reject(new Error("Stylesheet-ul Canvas nu a devenit ready în buget."));
      }, 8000);
      link.addEventListener("load", function () {
        if (settled) return;
        settled = true;
        window.clearTimeout(timer);
        resolve();
      }, { once: true });
      link.addEventListener("error", function () {
        if (settled) return;
        settled = true;
        window.clearTimeout(timer);
        reject(new Error("Stylesheet-ul Canvas nu a putut fi încărcat."));
      }, { once: true });
    });
  }

  function prepareStylesheets(nextDocument) {
    var currentByKey = {};
    Array.prototype.forEach.call(document.head.querySelectorAll("link[rel~='stylesheet']"), function (link) {
      var key = stylesheetKey(link);
      if (key) currentByKey[key] = link;
    });
    var desired = [];
    var waits = [];
    Array.prototype.forEach.call(nextDocument.head.querySelectorAll("link[rel~='stylesheet']"), function (source) {
      var key = stylesheetKey(source);
      if (key && currentByKey[key]) {
        desired.push({ key: key, link: currentByKey[key], fresh: false, media: source.getAttribute("media") || "" });
        return;
      }
      var link = document.importNode(source, true);
      var media = link.getAttribute("media") || "";
      link.setAttribute("media", "not all");
      link.setAttribute("data-pana-staged-resource", "");
      document.head.appendChild(link);
      desired.push({ key: key, link: link, fresh: true, media: media });
      waits.push(waitForStylesheet(link));
    });
    return Promise.all(waits).then(function () {
      return { currentByKey: currentByKey, desired: desired };
    }).catch(function (error) {
      desired.forEach(function (entry) {
        if (entry.fresh && entry.link.parentNode) entry.link.remove();
      });
      throw error;
    });
  }

  function reconcileHead(nextDocument, preparedStyles) {
    var desiredKeys = {};
    preparedStyles.desired.forEach(function (entry) {
      if (entry.key) desiredKeys[entry.key] = true;
    });
    Array.prototype.slice.call(document.head.childNodes).forEach(function (node) {
      if (isInternalCanvasNode(node)) return;
      var key = stylesheetKey(node);
      if (key) return;
      node.remove();
    });
    Array.prototype.forEach.call(nextDocument.head.childNodes, function (node) {
      if (stylesheetKey(node)) return;
      document.head.appendChild(document.importNode(node, true));
    });
    preparedStyles.desired.forEach(function (entry) {
      if (entry.media) entry.link.setAttribute("media", entry.media);
      else entry.link.removeAttribute("media");
      entry.link.removeAttribute("data-pana-staged-resource");
    });
    Object.keys(preparedStyles.currentByKey).forEach(function (key) {
      if (!desiredKeys[key]) preparedStyles.currentByKey[key].remove();
    });
  }

  function waitForStyledFrame() {
    var fontsReady = document.fonts && document.fonts.ready
      ? Promise.race([
          document.fonts.ready,
          new Promise(function (resolve) { window.setTimeout(resolve, 4000); })
        ])
      : Promise.resolve();
    return fontsReady.then(function () {
      return new Promise(function (resolve) {
        window.requestAnimationFrame(function () {
          window.requestAnimationFrame(resolve);
        });
      });
    });
  }

  function canvasPhaseReceipt(identity, phase, timings, diagnostic) {
    return {
      schemaVersion: 1,
      identity: identity || null,
      phase: phase,
      phaseTimingsMs: Object.assign({}, timings || {}),
      diagnostic: diagnostic || null
    };
  }

  function replaceDocument(html, selector, liveCss, canvasIdentity) {
    var startedAt = performance.now();
    var phaseTimings = {};
    var phaseReceipts = [];
    var parser = new DOMParser();
    var nextDocument = parser.parseFromString(String(html || ""), "text/html");
    sanitizeDesignSafeTree(nextDocument);
    var scrollX = window.scrollX;
    var scrollY = window.scrollY;
    var active = document.activeElement instanceof Element
      ? (document.activeElement.getAttribute(SESSION_ID_ATTR) || document.activeElement.getAttribute(SOURCE_ID_ATTR))
      : null;

    return prepareStylesheets(nextDocument).then(function (preparedStyles) {
      var resourcesReadyAt = performance.now();
      phaseTimings.resourcesReady = Math.max(0, Math.round(resourcesReadyAt - startedAt));
      phaseReceipts.push(canvasPhaseReceipt(
        canvasIdentity,
        "resourcesReady",
        phaseTimings,
        null
      ));
      replaceCanonicalAttributes(document.documentElement, nextDocument.documentElement, true);
      replaceCanonicalAttributes(document.body, nextDocument.body, true);
      reconcileHead(nextDocument, preparedStyles);
      reconcileChildren(document.body, nextDocument.body, true);
      sanitizeDesignSafeTree(document);
      applyTemplateSourceIdsFromMarkers();
      ensureElementSessionIds();
      refreshEmptyEditableZones();
      clearSelectedElement();
      ensureInspectorStyles();
      setLiveOverridesCss(liveCss || "", false);
      reapplyLiveTextDraft();
      reapplyLiveAttributeDraft();
      syncStructure();
      notifyPanaBlocksInit(document);
      window.scrollTo(scrollX, scrollY);
      if (active) {
        var focusTarget = document.querySelector("[" + SESSION_ID_ATTR + '=\"' + cssEscapeValue(active) + '\"],[' + SOURCE_ID_ATTR + '=\"' + cssEscapeValue(active) + '\"]');
        if (focusTarget && typeof focusTarget.focus === "function") focusTarget.focus({ preventScroll: true });
      }
      var nextSelected = selector ? document.querySelector(selector) : null;
      if (nextSelected) selectElement(nextSelected);
      var committedAt = performance.now();
      phaseTimings.committed = Math.max(0, Math.round(committedAt - startedAt));
      phaseReceipts.push(canvasPhaseReceipt(
        canvasIdentity,
        "committed",
        phaseTimings,
        null
      ));
      return waitForStyledFrame().then(function () {
        var styledReadyAt = performance.now();
        phaseTimings.styledReady = Math.max(0, Math.round(styledReadyAt - startedAt));
        phaseReceipts.push(canvasPhaseReceipt(
          canvasIdentity,
          "styledReady",
          phaseTimings,
          null
        ));
        retireCanvasPatchRollbacks();
        return {
          canvasPhaseReceipts: phaseReceipts
        };
      });
    });
  }
