(function () {
  "use strict";
  /* PANA BLOCK RUNTIME CORE */

  if (window.PanaBlockRuntime) return;

  var definitions = Object.create(null);
  var instances = [];
  var activeIds = null;
  var observer = null;
  var reporter = null;
  var lastPageConfigReceipt = null;
  var structureIds = typeof WeakMap === "function" ? new WeakMap() : null;
  var nextStructureId = 1;

  function report(type, payload) {
    if (typeof reporter !== "function") return;
    try { reporter(type, payload || {}); } catch (_) {}
  }

  function blockSelector(id) {
    var escaped = String(id || "").replace(/["\\]/g, "\\$&");
    return '[data-pana-block="' + escaped + '"]';
  }

  function rootsInside(scope, id) {
    var selector = blockSelector(id);
    var roots = [];
    if (scope && scope.nodeType === 1 && scope.matches && scope.matches(selector)) roots.push(scope);
    if (scope && scope.querySelectorAll) {
      Array.prototype.push.apply(roots, scope.querySelectorAll(selector));
    }
    return roots;
  }

  function elementInside(scope, element) {
    return scope === document || scope === element || Boolean(scope && scope.contains && scope.contains(element));
  }

  function isContractOptionAttribute(name) {
    return [
      "class",
      "style",
      "id",
      "data-anim",
      "data-open",
      "data-pana-block",
      "data-pana-instance",
      "data-pana-source-id",
      "data-pana-template-source-id",
      "data-pana-template-source-stack",
      "data-pana-session-id",
      "data-pana-preview-revision"
    ].indexOf(name) < 0;
  }

  function optionSignature(element) {
    return Array.prototype.slice.call(element.attributes)
      .filter(function (attribute) { return isContractOptionAttribute(attribute.name); })
      .map(function (attribute) { return attribute.name + "=" + attribute.value; })
      .sort()
      .join("\u0000");
  }

  function structureNodeId(element) {
    if (!structureIds) return element.getAttribute("data-pana-source-id") || element.localName;
    var id = structureIds.get(element);
    if (!id) {
      id = nextStructureId++;
      structureIds.set(element, id);
    }
    return String(id);
  }

  function structureSignature(element) {
    var selector = [
      "[data-pana-slot]",
      "[data-pana-slider-slide]",
      "[data-pana-accordion-item]",
      "[data-pana-tabs-tab]",
      "[data-pana-tabs-panel]"
    ].join(",");
    return Array.prototype.slice.call(element.querySelectorAll(selector))
      .map(function (node) {
        return node.localName + ":" + structureNodeId(node) + ":" +
          (node.getAttribute("data-pana-slot") || "");
      })
      .join("\u0000");
  }

  function isRootOptionMutation(record) {
    if (!record || record.type !== "attributes" || !isContractOptionAttribute(record.attributeName || "")) {
      return false;
    }
    var element = record.target;
    if (!element || element.nodeType !== 1) return false;
    var blockId = element.getAttribute("data-pana-block");
    return Boolean(blockId && definitions[blockId]);
  }

  function cleanupBag() {
    var cleanups = [];
    var disposed = false;
    return {
      listen: function (target, type, listener, options) {
        if (!target || !target.addEventListener) return;
        target.addEventListener(type, listener, options);
        cleanups.push(function () { target.removeEventListener(type, listener, options); });
      },
      add: function (cleanup) {
        if (typeof cleanup === "function") cleanups.push(cleanup);
      },
      frame: function (callback) {
        var id = window.requestAnimationFrame(callback);
        cleanups.push(function () { window.cancelAnimationFrame(id); });
        return id;
      },
      timer: function (callback, delay) {
        var id = window.setTimeout(callback, delay);
        cleanups.push(function () { window.clearTimeout(id); });
        return id;
      },
      dispose: function () {
        if (disposed) return;
        disposed = true;
        cleanups.splice(0).reverse().forEach(function (cleanup) {
          try { cleanup(); } catch (_) {}
        });
      }
    };
  }

  function disposeInstance(instance) {
    if (!instance || instance.disposed) return;
    instance.disposed = true;
    try {
      if (typeof instance.dispose === "function") instance.dispose();
    } catch (error) {
      report("lifecycle-error", {
        blockId: instance.blockId,
        phase: "dispose",
        message: error && error.message ? String(error.message) : String(error)
      });
    }
  }

  function dispose(scope) {
    var retained = [];
    instances.forEach(function (instance) {
      if (elementInside(scope || document, instance.element)) disposeInstance(instance);
      else retained.push(instance);
    });
    instances = retained;
  }

  function reconcile(scope) {
    var root = scope && scope.querySelectorAll ? scope : document;
    instances = instances.filter(function (instance) {
      if (!document.contains(instance.element)) {
        disposeInstance(instance);
        return false;
      }
      if (activeIds && !activeIds[instance.blockId]) {
        disposeInstance(instance);
        return false;
      }
      return true;
    });

    Object.keys(definitions).forEach(function (blockId) {
      if (activeIds && !activeIds[blockId]) return;
      var definition = definitions[blockId];
      rootsInside(root, blockId).forEach(function (element) {
        var existing = instances.find(function (instance) {
          return instance.blockId === blockId && instance.element === element && !instance.disposed;
        });
        try {
          var signature = optionSignature(element);
          var structuralSignature = structureSignature(element);
          if (existing && existing.optionSignature !== signature) {
            disposeInstance(existing);
            instances = instances.filter(function (instance) { return instance !== existing; });
            existing = null;
            report("lifecycle", { blockId: blockId, phase: "remount-options" });
          }
          if (existing && existing.structureSignature !== structuralSignature) {
            disposeInstance(existing);
            instances = instances.filter(function (instance) { return instance !== existing; });
            existing = null;
            report("lifecycle", { blockId: blockId, phase: "remount-structure" });
          }
          if (existing) {
            if (typeof definition.update === "function") {
              definition.update(element, existing.state);
            }
            return;
          }
          var mounted = typeof definition.mount === "function"
            ? definition.mount(element, cleanupBag)
            : null;
          instances.push({
            blockId: blockId,
            element: element,
            state: mounted && mounted.state ? mounted.state : null,
            dispose: mounted && typeof mounted.dispose === "function" ? mounted.dispose : mounted,
            optionSignature: signature,
            structureSignature: structuralSignature,
            disposed: false
          });
          report("lifecycle", { blockId: blockId, phase: "mount" });
        } catch (error) {
          report("lifecycle-error", {
            blockId: blockId,
            phase: existing ? "update" : "mount",
            message: error && error.message ? String(error.message) : String(error)
          });
        }
      });
    });
  }

  function register(blockId, definition) {
    var id = String(blockId || "").trim();
    if (!id || !definition || typeof definition !== "object") return false;
    definitions[id] = definition;
    if (activeIds) reconcile(document);
    return true;
  }

  function installPageConfig(config) {
    var entries = config && Array.isArray(config.blocks) ? config.blocks : [];
    activeIds = Object.create(null);
    entries.forEach(function (entry) {
      var id = String(entry && entry.id || "").trim();
      if (id && definitions[id]) activeIds[id] = true;
    });
    reconcile(document);
    lastPageConfigReceipt = {
      blockCount: Object.keys(activeIds).length
    };
    report("page-config-installed", lastPageConfigReceipt);
  }

  function start() {
    if (observer || !document.body) return;
    observer = new MutationObserver(function (records) {
      if (records.some(function (record) {
        return record.type === "childList" || isRootOptionMutation(record);
      })) reconcile(document);
    });
    observer.observe(document.body, { childList: true, subtree: true, attributes: true });
    reconcile(document);
  }

  function shutdown() {
    if (observer) observer.disconnect();
    observer = null;
    dispose(document);
  }

  var api = Object.freeze({
    register: register,
    installPageConfig: installPageConfig,
    reconcile: reconcile,
    dispose: dispose,
    start: start,
    shutdown: shutdown,
    setReporter: function (nextReporter) {
      reporter = typeof nextReporter === "function" ? nextReporter : null;
      if (reporter && lastPageConfigReceipt) {
        report("page-config-installed", lastPageConfigReceipt);
      }
    }
  });

  Object.defineProperty(window, "PanaBlockRuntime", {
    configurable: false,
    enumerable: false,
    writable: false,
    value: api
  });

  document.addEventListener("pana:blocks:init", function (event) {
    reconcile(event && event.detail && event.detail.root ? event.detail.root : document);
  });
  document.addEventListener("pana:blocks:dispose", function (event) {
    dispose(event && event.detail && event.detail.root ? event.detail.root : document);
  });
  window.addEventListener("pagehide", shutdown, { once: true });
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", start, { once: true });
  } else {
    start();
  }

})();
