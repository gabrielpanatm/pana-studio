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
    if (!(target instanceof Element) || !(source instanceof Element)) return 0;
    var preserved = {};
    if (preserveInternal) {
      [SESSION_ID_ATTR, "data-pana-internal-style"].forEach(function (name) {
        if (target.hasAttribute(name)) preserved[name] = target.getAttribute(name) || "";
      });
    }
    var desired = {};
    Array.prototype.slice.call(source.attributes || []).forEach(function (attribute) {
      desired[attribute.name] = attribute.value || "";
    });
    Object.keys(preserved).forEach(function (name) {
      desired[name] = preserved[name];
    });
    var mutations = 0;
    Array.prototype.slice.call(target.attributes || []).forEach(function (attribute) {
      if (!Object.prototype.hasOwnProperty.call(desired, attribute.name)) {
        target.removeAttribute(attribute.name);
        mutations += 1;
      }
    });
    Object.keys(desired).forEach(function (name) {
      if (!target.hasAttribute(name) || target.getAttribute(name) !== desired[name]) {
        target.setAttribute(name, desired[name]);
        mutations += 1;
      }
    });
    return mutations;
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

  function preloadKey(link) {
    if (!(link instanceof Element) || link.localName !== "link") return null;
    var rel = String(link.getAttribute("rel") || "").toLowerCase().split(/\s+/);
    if (rel.indexOf("preload") < 0) return null;
    var href;
    try {
      href = new URL(link.getAttribute("href") || "", document.baseURI).href;
    } catch (_) {
      href = link.getAttribute("href") || "";
    }
    return [
      href,
      String(link.getAttribute("as") || "").toLowerCase(),
      String(link.getAttribute("type") || "").toLowerCase(),
      String(link.getAttribute("crossorigin") || "").toLowerCase()
    ].join("|");
  }

  function normalizedTokenAttribute(element, name) {
    return String(element.getAttribute(name) || "")
      .toLowerCase()
      .split(/\s+/)
      .filter(Boolean)
      .sort()
      .join(" ");
  }

  function canonicalHeadUrl(element, name) {
    var value = element.getAttribute(name) || "";
    try {
      return new URL(value, document.baseURI).href;
    } catch (_) {
      return value;
    }
  }

  function headSemanticBaseKey(node) {
    if (node.nodeType === 3) return "text";
    if (node.nodeType === 8) return "comment";
    if (!(node instanceof Element)) return "node:" + node.nodeType;
    if (node.id) return "id:" + node.id;
    if (node.localName === "title") return "title";
    if (node.localName === "base") return "base";
    if (node.localName === "meta") {
      if (node.hasAttribute("charset")) return "meta:charset";
      var metaIdentity = ["name", "property", "http-equiv", "itemprop"].map(function (name) {
        return name + "=" + String(node.getAttribute(name) || "").toLowerCase();
      }).join("|");
      return "meta:" + metaIdentity;
    }
    if (node.localName === "link") {
      return [
        "link",
        normalizedTokenAttribute(node, "rel"),
        canonicalHeadUrl(node, "href"),
        String(node.getAttribute("as") || "").toLowerCase()
      ].join(":");
    }
    if (node.localName === "script" && node.hasAttribute("src")) {
      return "script:" + canonicalHeadUrl(node, "src");
    }
    return "tag:" + node.localName;
  }

  function keyedHeadChildren(parent) {
    var occurrences = {};
    return Array.prototype.slice.call(parent.childNodes || []).map(function (node) {
      var base = headSemanticBaseKey(node);
      var occurrence = occurrences[base] || 0;
      occurrences[base] = occurrence + 1;
      return { node: node, key: base + ":" + occurrence };
    });
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

  function captureElementAttributes(element) {
    return Array.prototype.slice.call(element.attributes || []).map(function (attribute) {
      return { name: attribute.name, value: attribute.value || "" };
    });
  }

  function restoreElementAttributes(element, attributes) {
    var desired = {};
    (attributes || []).forEach(function (attribute) {
      desired[attribute.name] = attribute.value;
    });
    Array.prototype.slice.call(element.attributes || []).forEach(function (attribute) {
      if (!Object.prototype.hasOwnProperty.call(desired, attribute.name)) {
        element.removeAttribute(attribute.name);
      }
    });
    Object.keys(desired).forEach(function (name) {
      if (!element.hasAttribute(name) || element.getAttribute(name) !== desired[name]) {
        element.setAttribute(name, desired[name]);
      }
    });
  }

  function takePreparedNode(byKey, key) {
    var bucket = key && byKey[key] ? byKey[key] : [];
    return bucket.length > 0 ? bucket.shift() : null;
  }

  function indexPreparedLinks(selector, keyFor) {
    var byKey = {};
    var all = [];
    Array.prototype.forEach.call(document.head.querySelectorAll(selector), function (link) {
      var key = keyFor(link);
      if (!key) return;
      if (!byKey[key]) byKey[key] = [];
      byKey[key].push(link);
      all.push(link);
    });
    return { byKey: byKey, all: all };
  }

  function waitForPreload(link) {
    return new Promise(function (resolve, reject) {
      var settled = false;
      var timer = window.setTimeout(function () {
        if (settled) return;
        settled = true;
        reject(new Error("Preload-ul Canvas nu a devenit ready în buget."));
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
        reject(new Error("Preload-ul Canvas nu a putut fi încărcat."));
      }, { once: true });
    });
  }

  function prepareStylesheets(nextDocument) {
    var currentStyles = indexPreparedLinks("link[rel~='stylesheet']", stylesheetKey);
    var currentPreloads = indexPreparedLinks("link[rel~='preload']", preloadKey);
    var desired = [];
    var desiredPreloads = [];
    var waits = [];
    Array.prototype.forEach.call(nextDocument.head.querySelectorAll("link[rel~='stylesheet']"), function (source) {
      var key = stylesheetKey(source);
      var current = takePreparedNode(currentStyles.byKey, key);
      if (current) {
        desired.push({
          key: key,
          link: current,
          source: source,
          fresh: false,
          originalAttributes: captureElementAttributes(current)
        });
        return;
      }
      var link = document.importNode(source, true);
      link.setAttribute("media", "not all");
      link.setAttribute("data-pana-staged-resource", "");
      document.head.appendChild(link);
      desired.push({
        key: key,
        link: link,
        source: source,
        fresh: true,
        originalAttributes: []
      });
      waits.push(waitForStylesheet(link));
    });
    Array.prototype.forEach.call(nextDocument.head.querySelectorAll("link[rel~='preload']"), function (source) {
      var key = preloadKey(source);
      var current = takePreparedNode(currentPreloads.byKey, key);
      if (current) {
        desiredPreloads.push({
          key: key,
          link: current,
          source: source,
          fresh: false,
          originalAttributes: captureElementAttributes(current)
        });
        return;
      }
      var link = document.importNode(source, true);
      link.setAttribute("data-pana-staged-resource", "");
      var wait = waitForPreload(link);
      document.head.appendChild(link);
      desiredPreloads.push({
        key: key,
        link: link,
        source: source,
        fresh: true,
        originalAttributes: []
      });
      waits.push(wait);
    });
    return Promise.all(waits).then(function () {
      return {
        currentStyles: currentStyles,
        currentPreloads: currentPreloads,
        desired: desired,
        desiredPreloads: desiredPreloads,
        obsolete: [],
        obsoleteStyleSnapshots: [],
        obsoletePreloads: [],
        activatedAt: null,
        stats: {
          reused: desired.filter(function (entry) { return !entry.fresh; }).length,
          staged: desired.filter(function (entry) { return entry.fresh; }).length,
          retired: 0,
          preloadsReused: desiredPreloads.filter(function (entry) { return !entry.fresh; }).length,
          preloadsStaged: desiredPreloads.filter(function (entry) { return entry.fresh; }).length,
          preloadsRetired: 0,
          headNodesReused: 0,
          headNodesCreated: 0,
          headNodesRetired: 0,
          headNodesReordered: 0,
          stylesheetAttributeMutations: 0,
          preloadAttributeMutations: 0
        }
      };
    }).catch(function (error) {
      desired.concat(desiredPreloads).forEach(function (entry) {
        if (entry.fresh && entry.link.parentNode) entry.link.remove();
      });
      throw error;
    });
  }

  function reconcileHead(nextDocument, preparedStyles) {
    var desiredStyleNodes = preparedStyles.desired.map(function (entry) { return entry.link; });
    var desiredPreloadNodes = preparedStyles.desiredPreloads.map(function (entry) { return entry.link; });
    preparedStyles.obsolete = preparedStyles.currentStyles.all.filter(function (node) {
      return desiredStyleNodes.indexOf(node) < 0;
    });
    preparedStyles.obsoleteStyleSnapshots = preparedStyles.obsolete.map(function (link) {
      return {
        link: link,
        attributes: captureElementAttributes(link)
      };
    });
    preparedStyles.obsoletePreloads = preparedStyles.currentPreloads.all.filter(function (node) {
      return desiredPreloadNodes.indexOf(node) < 0;
    });
    preparedStyles.obsolete.forEach(function (link) {
      if (link.getAttribute("media") !== "not all") {
        link.setAttribute("media", "not all");
        preparedStyles.stats.stylesheetAttributeMutations += 1;
      }
    });

    var available = {};
    keyedHeadChildren(document.head).forEach(function (entry) {
      if (
        isInternalCanvasNode(entry.node)
        || stylesheetKey(entry.node)
        || preloadKey(entry.node)
      ) return;
      if (!available[entry.key]) available[entry.key] = [];
      available[entry.key].push(entry.node);
    });

    var styleIndex = 0;
    var preloadIndex = 0;
    var desiredNodes = [];
    var pinnedResourceNodes = [];
    keyedHeadChildren(nextDocument.head).forEach(function (entry) {
      var styleKey = stylesheetKey(entry.node);
      if (styleKey) {
        var preparedStyle = preparedStyles.desired[styleIndex];
        styleIndex += 1;
        if (!preparedStyle || preparedStyle.key !== styleKey) {
          throw new Error("Planul stylesheet Canvas nu corespunde head-ului canonic.");
        }
        preparedStyles.stats.stylesheetAttributeMutations += replaceCanonicalAttributes(
          preparedStyle.link,
          preparedStyle.source,
          false
        );
        if (!preparedStyle.fresh && preparedStyle.link.parentNode === document.head) {
          pinnedResourceNodes.push(preparedStyle.link);
        }
        desiredNodes.push(preparedStyle.link);
        return;
      }
      var resourceKey = preloadKey(entry.node);
      if (resourceKey) {
        var preparedPreload = preparedStyles.desiredPreloads[preloadIndex];
        preloadIndex += 1;
        if (!preparedPreload || preparedPreload.key !== resourceKey) {
          throw new Error("Planul preload Canvas nu corespunde head-ului canonic.");
        }
        preparedStyles.stats.preloadAttributeMutations += replaceCanonicalAttributes(
          preparedPreload.link,
          preparedPreload.source,
          false
        );
        if (!preparedPreload.fresh && preparedPreload.link.parentNode === document.head) {
          pinnedResourceNodes.push(preparedPreload.link);
        }
        desiredNodes.push(preparedPreload.link);
        return;
      }
      var bucket = available[entry.key] || [];
      var candidate = bucket.shift() || null;
      if (candidate) {
        preparedStyles.stats.headNodesReused += 1;
        desiredNodes.push(reconcileNode(candidate, entry.node));
      } else {
        preparedStyles.stats.headNodesCreated += 1;
        desiredNodes.push(document.importNode(entry.node, true));
      }
    });

    var desiredSet = desiredNodes.slice();
    var desiredReusableResources = desiredNodes.filter(function (node) {
      return pinnedResourceNodes.indexOf(node) >= 0;
    });
    var currentReusableResources = Array.prototype.slice.call(document.head.childNodes).filter(function (node) {
      return pinnedResourceNodes.indexOf(node) >= 0;
    });
    var reusableResourceOrderChanged = desiredReusableResources.length !== currentReusableResources.length
      || desiredReusableResources.some(function (node, index) {
        return currentReusableResources[index] !== node;
      });
    if (reusableResourceOrderChanged) {
      // O schimbare semantică reală a ordinii resurselor trebuie aplicată. Doar
      // mutațiile fără schimbarea ordinii sunt suprimate pentru a evita
      // invalidarea CSSOM/FontFace produsă de WebKit.
      pinnedResourceNodes = [];
    }
    var cursor = document.head.firstChild;
    desiredNodes.forEach(function (node) {
      while (cursor && isInternalCanvasNode(cursor)) cursor = cursor.nextSibling;
      if (pinnedResourceNodes.indexOf(node) >= 0) {
        // WebKit invalidează CSSOM/FontFace chiar și când aceeași instanță
        // <link> este doar mutată în același head. Resursele reutilizate sunt
        // ancore imobile; nodurile canonice non-resursă se așază în jurul lor.
        cursor = node.nextSibling;
        return;
      }
      if (node !== cursor) {
        if (node.parentNode === document.head) {
          preparedStyles.stats.headNodesReordered += 1;
        }
        document.head.insertBefore(node, cursor || null);
      }
      cursor = node.nextSibling;
    });
    Array.prototype.slice.call(document.head.childNodes).forEach(function (node) {
      if (
        isInternalCanvasNode(node)
        || desiredSet.indexOf(node) >= 0
        || preparedStyles.obsolete.indexOf(node) >= 0
        || preparedStyles.obsoletePreloads.indexOf(node) >= 0
      ) return;
      node.remove();
      preparedStyles.stats.headNodesRetired += 1;
    });
    preparedStyles.activatedAt = performance.now();
  }

  function retireObsoleteStylesheets(preparedStyles) {
    preparedStyles.obsolete.forEach(function (link) {
      if (link.parentNode) link.remove();
    });
    preparedStyles.stats.retired = preparedStyles.obsolete.length;
    preparedStyles.obsoletePreloads.forEach(function (link) {
      if (link.parentNode) link.remove();
    });
    preparedStyles.stats.preloadsRetired = preparedStyles.obsoletePreloads.length;
  }

  function rollbackPreparedStylesheets(preparedStyles) {
    if (!preparedStyles) return;
    preparedStyles.desired.concat(preparedStyles.desiredPreloads).forEach(function (entry) {
      if (entry.fresh) {
        if (entry.link.parentNode) entry.link.remove();
        return;
      }
      restoreElementAttributes(entry.link, entry.originalAttributes);
    });
    preparedStyles.obsoleteStyleSnapshots.forEach(function (entry) {
      restoreElementAttributes(entry.link, entry.attributes);
    });
  }

  function typographyFontValue(style) {
    if (style.font) return style.font;
    return [
      style.fontStyle || "normal",
      style.fontWeight || "400",
      style.fontSize || "16px",
      style.fontFamily || "sans-serif"
    ].join(" ");
  }

  function createTypographyProbe(includeInitiallyErrored) {
    var canvas = document.createElement("canvas");
    var context = canvas.getContext && canvas.getContext("2d");
    var descriptors = [];
    if (context) {
      Array.prototype.slice.call(
        document.querySelectorAll("h1, h2, h3, p, a, button, li")
      ).slice(0, 12).forEach(function (element) {
        var text = normalizeSearchText(element.textContent).slice(0, 96);
        if (!text) return;
        var font = typographyFontValue(window.getComputedStyle(element));
        context.font = font;
        descriptors.push({
          font: font,
          text: text,
          width: context.measureText(text).width
        });
      });
    }
    var faces = [];
    if (document.fonts && typeof document.fonts.forEach === "function") {
      document.fonts.forEach(function (face) {
        faces.push({
          face: face,
          initiallyLoaded: face.status === "loaded",
          initiallyErrored: face.status === "error",
          invalidationObserved: false
        });
      });
    }
    return {
      context: context,
      descriptors: descriptors,
      faces: faces,
      documentInitiallyLoaded: !document.fonts || document.fonts.status === "loaded",
      documentInvalidationObserved: false,
      includeInitiallyErrored: includeInitiallyErrored === true,
      fontInvalidationCount: 0,
      fontFallbackFrames: 0,
      maxTextMetricDelta: 0,
      fontActivationErrors: []
    };
  }

  function sampleTypographyProbe(probe) {
    if (!probe) return;
    var fallback = false;
    if (document.fonts && document.fonts.status !== "loaded") {
      if (probe.documentInitiallyLoaded && !probe.documentInvalidationObserved) {
        probe.documentInvalidationObserved = true;
        probe.fontInvalidationCount += 1;
      }
    }
    probe.faces.forEach(function (entry) {
      if (entry.initiallyLoaded && entry.face.status !== "loaded") {
        fallback = true;
        if (!entry.invalidationObserved) {
          entry.invalidationObserved = true;
          probe.fontInvalidationCount += 1;
        }
      }
    });
    var frameMetricDelta = 0;
    if (!probe.context) {
      if (fallback) probe.fontFallbackFrames += 1;
      return;
    }
    probe.descriptors.forEach(function (entry) {
      probe.context.font = entry.font;
      var delta = Math.abs(probe.context.measureText(entry.text).width - entry.width);
      frameMetricDelta = Math.max(frameMetricDelta, delta);
      probe.maxTextMetricDelta = Math.max(
        probe.maxTextMetricDelta,
        delta
      );
    });
    if (fallback || frameMetricDelta > 0.25) probe.fontFallbackFrames += 1;
  }

  function normalizedFontFamily(value) {
    var family = String(value || "").trim();
    if (
      family.length >= 2
      && ((family[0] === '"' && family[family.length - 1] === '"')
        || (family[0] === "'" && family[family.length - 1] === "'"))
    ) {
      family = family.slice(1, -1);
    }
    return family.trim().toLowerCase();
  }

  function fontFaceSources(face) {
    var family = normalizedFontFamily(face && face.family);
    if (!family) return [];
    var sources = [];
    function visitRules(rules) {
      Array.prototype.forEach.call(rules || [], function (rule) {
        if (rule && rule.style && typeof rule.style.getPropertyValue === "function") {
          var declaredFamily = normalizedFontFamily(rule.style.getPropertyValue("font-family"));
          var source = String(rule.style.getPropertyValue("src") || "").trim();
          if (declaredFamily === family && source && sources.indexOf(source) < 0) {
            sources.push(source.slice(0, 280));
          }
        }
        if (rule && rule.cssRules) {
          try { visitRules(rule.cssRules); } catch (_) {}
        }
      });
    }
    Array.prototype.forEach.call(document.styleSheets || [], function (sheet) {
      try { visitRules(sheet.cssRules); } catch (_) {}
    });
    return sources.slice(0, 3);
  }

  function typographyFontErrors(probe) {
    if (!document.fonts || typeof document.fonts.forEach !== "function") return [];
    var errors = [];
    document.fonts.forEach(function (face) {
      if (face.status !== "error") return;
      var baseline = probe.faces.find(function (entry) { return entry.face === face; });
      if (baseline && baseline.initiallyErrored && !probe.includeInitiallyErrored) return;
      errors.push({
        family: String(face.family || "font necunoscut"),
        weight: String(face.weight || "normal"),
        style: String(face.style || "normal"),
        sources: fontFaceSources(face)
      });
    });
    return errors;
  }

  function fontActivationDiagnostic(errors) {
    if (!errors || errors.length === 0) return null;
    var details = errors.slice(0, 8).map(function (error) {
      var descriptor = error.family + " (" + error.weight + ", " + error.style + ")";
      if (error.sources && error.sources.length > 0) {
        descriptor += " ← " + error.sources.join(" | ");
      }
      return descriptor;
    });
    if (errors.length > details.length) details.push("+" + (errors.length - details.length) + " fonturi");
    return (
      "Fonturi indisponibile în Canvas; browserul folosește fallback-ul stabil: "
      + details.join("; ")
    ).slice(0, 4000);
  }

  function waitForStyledFrame(timingOrigin, typographyProbe, includeInitiallyErrored) {
    var origin = typeof timingOrigin === "number" ? timingOrigin : performance.now();
    typographyProbe = typographyProbe || createTypographyProbe(includeInitiallyErrored);
    var sampling = true;
    function sampleFrame() {
      if (!sampling) return;
      sampleTypographyProbe(typographyProbe);
      window.requestAnimationFrame(sampleFrame);
    }
    sampleTypographyProbe(typographyProbe);
    window.requestAnimationFrame(sampleFrame);
    var fontsReady = document.fonts && document.fonts.ready
      ? new Promise(function (resolve, reject) {
          var timer = window.setTimeout(function () {
            reject(new Error("Fonturile Canvas nu au devenit ready în buget."));
          }, 4000);
          document.fonts.ready.then(function () {
            window.clearTimeout(timer);
            if (document.fonts.status !== "loaded") {
              reject(new Error("FontFaceSet Canvas nu este în starea loaded."));
              return;
            }
            typographyProbe.fontActivationErrors = typographyFontErrors(typographyProbe);
            resolve();
          }, function (error) {
            window.clearTimeout(timer);
            reject(error);
          });
        })
      : Promise.resolve();
    return fontsReady.then(function () {
      var fontsReadyAt = Math.max(0, Math.round(performance.now() - origin));
      return new Promise(function (resolve) {
        window.requestAnimationFrame(function () {
          sampleTypographyProbe(typographyProbe);
          window.requestAnimationFrame(function () {
            sampling = false;
            sampleTypographyProbe(typographyProbe);
            resolve({
              fontsReady: fontsReadyAt,
              styledReady: Math.max(0, Math.round(performance.now() - origin)),
              fontInvalidationCount: typographyProbe.fontInvalidationCount,
              fontFallbackFrames: typographyProbe.fontFallbackFrames,
              maxTextMetricDelta: Math.round(typographyProbe.maxTextMetricDelta * 1000) / 1000,
              fontActivationErrorCount: typographyProbe.fontActivationErrors.length,
              fontActivationDiagnostic: fontActivationDiagnostic(typographyProbe.fontActivationErrors)
            });
          });
        });
      });
    }).catch(function (error) {
      sampling = false;
      throw error;
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

  function validateCanonicalDocumentStructure(nextDocument) {
    if (!nextDocument || !nextDocument.head || !nextDocument.body) {
      throw new Error("Documentul Canvas nu conține structura canonică head/body.");
    }
    var visualHeadNode = nextDocument.head.querySelector(
      "[data-pana-empty-tera-slot], [data-pana-active-document-root], .pana-studio-empty-editable, div, main, section, article"
    );
    if (visualHeadNode) {
      throw new Error("Documentul Canvas conține un element vizual invalid în head.");
    }
    var bodyOwnedHeadNode = nextDocument.body.querySelector(
      "link[rel~='stylesheet'], link[rel~='preload'], base, title, meta[charset], meta[name], meta[property], meta[http-equiv]"
    );
    if (bodyOwnedHeadNode) {
      throw new Error("Documentul Canvas a mutat o resursă head în body și a fost refuzat.");
    }
  }

  function replaceDocument(html, liveCss, canvasIdentity) {
    var startedAt = performance.now();
    var phaseTimings = {};
    var phaseReceipts = [];
    var parser = new DOMParser();
    var nextDocument = parser.parseFromString(String(html || ""), "text/html");
    validateCanonicalDocumentStructure(nextDocument);
    sanitizeDesignSafeTree(nextDocument);
    var scrollX = window.scrollX;
    var scrollY = window.scrollY;
    var active = document.activeElement instanceof Element
      ? (document.activeElement.getAttribute(SESSION_ID_ATTR) || document.activeElement.getAttribute(SOURCE_ID_ATTR))
      : null;

    var preparedStyles = null;
    return prepareStylesheets(nextDocument).then(function (prepared) {
      preparedStyles = prepared;
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
      var typographyProbe = createTypographyProbe();
      reconcileHead(nextDocument, preparedStyles);
      reconcileChildren(document.body, nextDocument.body, true);
      sanitizeDesignSafeTree(document);
      restoreApplicationAppearance();
      applyTemplateSourceIdsFromMarkers();
      ensureElementSessionIds();
      refreshEmptyEditableZones();
      clearCanvasAgentOverlays();
      ensureInspectorStyles();
      setLiveOverridesCss(liveCss || "");
      reapplyLiveTextDraft();
      reapplyLiveAttributeDraft();
      syncStructure();
      notifyPanaBlocksInit(document);
      window.scrollTo(scrollX, scrollY);
      if (active) {
        var focusTarget = document.querySelector("[" + SESSION_ID_ATTR + '=\"' + cssEscapeValue(active) + '\"],[' + SOURCE_ID_ATTR + '=\"' + cssEscapeValue(active) + '\"]');
        if (focusTarget && typeof focusTarget.focus === "function") focusTarget.focus({ preventScroll: true });
      }
      var committedAt = performance.now();
      phaseTimings.committed = Math.max(0, Math.round(committedAt - startedAt));
      phaseReceipts.push(canvasPhaseReceipt(
        canvasIdentity,
        "committed",
        phaseTimings,
        null
      ));
      return waitForStyledFrame(startedAt, typographyProbe).then(function (styledMetrics) {
        var styledReadyAt = performance.now();
        retireObsoleteStylesheets(preparedStyles);
        phaseTimings.fontsReady = styledMetrics.fontsReady;
        phaseTimings.styledReady = Math.max(0, Math.round(styledReadyAt - startedAt));
        phaseReceipts.push(canvasPhaseReceipt(
          canvasIdentity,
          "styledReady",
          phaseTimings,
          styledMetrics.fontActivationDiagnostic
        ));
        retireCanvasPatchRollbacks();
        return {
          canvasPhaseReceipts: phaseReceipts,
          stylesheetPromotion: {
            schemaVersion: 1,
            mode: "in_place",
            reused: preparedStyles.stats.reused,
            staged: preparedStyles.stats.staged,
            retired: preparedStyles.stats.retired,
            preloadsReused: preparedStyles.stats.preloadsReused,
            preloadsStaged: preparedStyles.stats.preloadsStaged,
            preloadsRetired: preparedStyles.stats.preloadsRetired,
            headNodesReused: preparedStyles.stats.headNodesReused,
            headNodesCreated: preparedStyles.stats.headNodesCreated,
            headNodesRetired: preparedStyles.stats.headNodesRetired,
            headNodesReordered: preparedStyles.stats.headNodesReordered,
            stylesheetAttributeMutations: preparedStyles.stats.stylesheetAttributeMutations,
            preloadAttributeMutations: preparedStyles.stats.preloadAttributeMutations,
            fontInvalidationCount: styledMetrics.fontInvalidationCount,
            fontFallbackFrames: styledMetrics.fontFallbackFrames,
            maxTextMetricDelta: styledMetrics.maxTextMetricDelta,
            fontActivationErrorCount: styledMetrics.fontActivationErrorCount,
            fontActivationDiagnostic: styledMetrics.fontActivationDiagnostic,
            fontsReadyMs: styledMetrics.fontsReady,
            activationToStyledMs: preparedStyles.activatedAt === null
              ? 0
              : Math.max(0, Math.round(styledReadyAt - preparedStyles.activatedAt))
          }
        };
      });
    }).catch(function (error) {
      rollbackPreparedStylesheets(preparedStyles);
      throw error;
    });
  }
