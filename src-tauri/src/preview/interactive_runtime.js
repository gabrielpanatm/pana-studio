(function () {
  "use strict";

  var SOURCE = "pana-studio-interactive";
  var SCHEMA_VERSION = 1;
  var MAX_NODES = 5000;
  var MAX_DEPTH = 64;
  var MAX_TEXT = 160;
  var mutationTimer = 0;
  var lastPublishedAt = 0;
  var lifecycleDefinitions = Object.create(null);
  var lifecycleInstances = [];

  function previewRevision() {
    return document.documentElement.getAttribute("data-pana-preview-revision") || "";
  }

  function post(type, payload) {
    var message = Object.assign({
      source: SOURCE,
      schemaVersion: SCHEMA_VERSION,
      type: type,
      previewRevision: previewRevision()
    }, payload || {});
    window.parent.postMessage(message, "*");
  }

  function safeText(value) {
    var normalized = String(value || "").replace(/\s+/g, " ").trim();
    return normalized.length > MAX_TEXT ? normalized.slice(0, MAX_TEXT) : normalized;
  }

  function inspectDocument() {
    var nodes = [];
    if (!document.body) return nodes;
    var walker = document.createTreeWalker(document.body, NodeFilter.SHOW_ELEMENT);
    var node = walker.currentNode;
    while (node && nodes.length < MAX_NODES) {
      var depth = 0;
      var parent = node.parentElement;
      while (parent && parent !== document.body && depth < MAX_DEPTH) {
        depth += 1;
        parent = parent.parentElement;
      }
      nodes.push({
        tag: node.localName || "",
        id: node.id || null,
        classes: Array.prototype.slice.call(node.classList || [], 0, 32),
        sourceId: node.getAttribute("data-pana-source-id") || null,
        renderInstanceId: node.getAttribute("data-pana-render-instance-id") || null,
        depth: depth,
        text: safeText(node.childElementCount === 0 ? node.textContent : "")
      });
      node = walker.nextNode();
    }
    return nodes;
  }

  function publishInspection(reason) {
    lastPublishedAt = performance.now();
    post("dom-snapshot", {
      reason: reason || "mutation",
      truncated: document.body ? document.body.querySelectorAll("*").length > MAX_NODES : false,
      nodes: inspectDocument()
    });
  }

  function scheduleInspection(reason) {
    if (mutationTimer) window.clearTimeout(mutationTimer);
    var elapsed = performance.now() - lastPublishedAt;
    mutationTimer = window.setTimeout(function () {
      mutationTimer = 0;
      publishInspection(reason);
    }, Math.max(40, 120 - elapsed));
  }

  function elementInside(root, element) {
    return root === document || root === element || (root && root.contains && root.contains(element));
  }

  function disposeInstance(instance) {
    if (!instance || instance.disposed) return;
    instance.disposed = true;
    try {
      if (typeof instance.cleanup === "function") instance.cleanup();
    } catch (error) {
      post("lifecycle-error", {
        componentId: instance.componentId,
        phase: "dispose",
        message: error && error.message ? String(error.message) : String(error)
      });
    }
  }

  function dispose(root) {
    var retained = [];
    lifecycleInstances.forEach(function (instance) {
      if (elementInside(root || document, instance.element)) disposeInstance(instance);
      else retained.push(instance);
    });
    lifecycleInstances = retained;
  }

  function reconcile(root) {
    var scope = root && root.querySelectorAll ? root : document;
    lifecycleInstances = lifecycleInstances.filter(function (instance) {
      if (!document.contains(instance.element)) {
        disposeInstance(instance);
        return false;
      }
      return true;
    });
    Object.keys(lifecycleDefinitions).forEach(function (componentId) {
      var definition = lifecycleDefinitions[componentId];
      var selector = '[data-pana-component="' + componentId.replace(/["\\]/g, "\\$&") + '"]';
      Array.prototype.forEach.call(scope.querySelectorAll(selector), function (element) {
        var existing = lifecycleInstances.find(function (instance) {
          return instance.componentId === componentId && instance.element === element && !instance.disposed;
        });
        try {
          if (existing) {
            if (typeof definition.update === "function") definition.update(element, existing.state);
            return;
          }
          var mounted = typeof definition.mount === "function" ? definition.mount(element) : null;
          lifecycleInstances.push({
            componentId: componentId,
            element: element,
            cleanup: mounted && typeof mounted.dispose === "function" ? mounted.dispose : mounted,
            state: mounted && mounted.state ? mounted.state : null,
            disposed: false
          });
        } catch (error) {
          post("lifecycle-error", {
            componentId: componentId,
            phase: existing ? "update" : "mount",
            message: error && error.message ? String(error.message) : String(error)
          });
        }
      });
    });
  }

  function cleanupBag() {
    var cleanups = [];
    return {
      listen: function (target, type, listener, options) {
        if (!target || !target.addEventListener) return;
        target.addEventListener(type, listener, options);
        cleanups.push(function () { target.removeEventListener(type, listener, options); });
      },
      add: function (cleanup) {
        if (typeof cleanup === "function") cleanups.push(cleanup);
      },
      dispose: function () {
        cleanups.splice(0).reverse().forEach(function (cleanup) {
          try { cleanup(); } catch (_) {}
        });
      }
    };
  }

  function counterDefinition() {
    return {
      __panaBuiltIn: true,
      mount: function (element) {
        var bag = cleanupBag();
        var frame = 0;
        var observer = null;
        var state = { started: false };
        function run() {
          if (state.started) return;
          state.started = true;
          var target = parseInt(element.getAttribute("data-tinta") || "0", 10);
          var duration = parseInt(element.getAttribute("data-durata") || "1800", 10);
          var suffix = element.getAttribute("data-sufix") || "";
          if (!isFinite(target)) target = 0;
          if (!isFinite(duration) || duration < 1) duration = 1800;
          var start = null;
          function tick(timestamp) {
            if (start === null) start = timestamp;
            var progress = Math.min((timestamp - start) / duration, 1);
            element.textContent = String(Math.floor(target * progress)) + suffix;
            if (progress < 1) frame = window.requestAnimationFrame(tick);
          }
          frame = window.requestAnimationFrame(tick);
        }
        if ("IntersectionObserver" in window) {
          observer = new IntersectionObserver(function (entries) {
            entries.forEach(function (entry) {
              if (!entry.isIntersecting) return;
              run();
              observer.unobserve(entry.target);
            });
          }, { threshold: 0.3 });
          observer.observe(element);
        } else run();
        bag.add(function () {
          if (frame) window.cancelAnimationFrame(frame);
          if (observer) observer.disconnect();
        });
        return { state: state, dispose: bag.dispose };
      }
    };
  }

  function accordionDefinition() {
    return {
      __panaBuiltIn: true,
      mount: function (root) {
        var bag = cleanupBag();
        var state = { allowMultiple: root.getAttribute("data-multiple") === "true" };
        var items = Array.prototype.slice.call(root.querySelectorAll("[data-pana-accordion-item]"));
        function setOpen(item, trigger, panel, open) {
          trigger.setAttribute("aria-expanded", open ? "true" : "false");
          panel.hidden = !open;
          if (open) item.setAttribute("data-open", "");
          else item.removeAttribute("data-open");
        }
        items.forEach(function (item, index) {
          var trigger = item.querySelector("[data-pana-accordion-trigger]");
          var panel = item.querySelector("[data-pana-accordion-panel]");
          if (!trigger || !panel) return;
          var instance = root.getAttribute("data-pana-instance") || "accordion";
          trigger.id = trigger.id || instance + "-trigger-" + index;
          panel.id = panel.id || instance + "-panel-" + index;
          trigger.setAttribute("aria-controls", panel.id);
          panel.setAttribute("role", "region");
          panel.setAttribute("aria-labelledby", trigger.id);
          setOpen(item, trigger, panel, trigger.getAttribute("aria-expanded") === "true" || item.hasAttribute("data-open"));
          bag.listen(trigger, "click", function () {
            var shouldOpen = trigger.getAttribute("aria-expanded") !== "true";
            if (shouldOpen && !state.allowMultiple) {
              items.forEach(function (other) {
                if (other === item) return;
                var otherTrigger = other.querySelector("[data-pana-accordion-trigger]");
                var otherPanel = other.querySelector("[data-pana-accordion-panel]");
                if (otherTrigger && otherPanel) setOpen(other, otherTrigger, otherPanel, false);
              });
            }
            setOpen(item, trigger, panel, shouldOpen);
          });
        });
        return { state: state, dispose: bag.dispose };
      },
      update: function (root, state) {
        if (state) state.allowMultiple = root.getAttribute("data-multiple") === "true";
      }
    };
  }

  function tabsDefinition() {
    return {
      __panaBuiltIn: true,
      mount: function (root) {
        var bag = cleanupBag();
        var tabs = Array.prototype.slice.call(root.querySelectorAll("[data-pana-tabs-tab]"));
        var panels = Array.prototype.slice.call(root.querySelectorAll("[data-pana-tabs-panel]"));
        function activate(index, focus) {
          tabs.forEach(function (tab, tabIndex) {
            var active = tabIndex === index;
            tab.setAttribute("aria-selected", active ? "true" : "false");
            tab.setAttribute("tabindex", active ? "0" : "-1");
            if (active && focus && tab.focus) tab.focus();
          });
          panels.forEach(function (panel, panelIndex) { panel.hidden = panelIndex !== index; });
        }
        tabs.forEach(function (tab, index) {
          var panel = panels[index];
          if (!panel) return;
          tab.setAttribute("role", "tab");
          panel.setAttribute("role", "tabpanel");
          bag.listen(tab, "click", function () { activate(index, false); });
          bag.listen(tab, "keydown", function (event) {
            if (["ArrowRight", "ArrowLeft", "Home", "End"].indexOf(event.key) < 0) return;
            event.preventDefault();
            var next = event.key === "Home" ? 0 : event.key === "End" ? tabs.length - 1
              : event.key === "ArrowRight" ? (index + 1) % tabs.length
              : (index - 1 + tabs.length) % tabs.length;
            activate(next, true);
          });
        });
        var selected = tabs.findIndex(function (tab) { return tab.getAttribute("aria-selected") === "true"; });
        activate(selected >= 0 ? selected : 0, false);
        return bag.dispose;
      }
    };
  }

  function overlayDefinition(kind) {
    var prefix = "data-pana-" + kind;
    return {
      __panaBuiltIn: true,
      mount: function (root) {
        var bag = cleanupBag();
        var openers = Array.prototype.slice.call(root.querySelectorAll("[" + prefix + "-open]"));
        var closers = Array.prototype.slice.call(root.querySelectorAll("[" + prefix + "-close]"));
        var overlay = root.querySelector("[" + prefix + "-overlay]");
        var panel = root.querySelector("[" + prefix + "-panel]");
        if (!overlay || !panel) return bag.dispose;
        var previousActive = null;
        var previousOverflow = "";
        function setOpen(open) {
          overlay.hidden = !open;
          if (open) root.setAttribute("data-open", "");
          else root.removeAttribute("data-open");
          openers.forEach(function (opener) { opener.setAttribute("aria-expanded", open ? "true" : "false"); });
        }
        function open(opener) {
          previousActive = document.activeElement;
          previousOverflow = document.body.style.overflow || "";
          document.body.style.overflow = "hidden";
          setOpen(true);
          var focus = panel.querySelector("button:not([disabled]),[href],[tabindex]:not([tabindex='-1'])") || panel;
          if (focus && focus.focus) focus.focus();
        }
        function close() {
          setOpen(false);
          document.body.style.overflow = previousOverflow;
          if (previousActive && previousActive.focus && document.contains(previousActive)) previousActive.focus();
        }
        openers.forEach(function (opener) { bag.listen(opener, "click", function () { open(opener); }); });
        closers.forEach(function (closer) { bag.listen(closer, "click", close); });
        bag.listen(overlay, "click", function (event) { if (event.target === overlay) close(); });
        bag.listen(overlay, "keydown", function (event) { if (event.key === "Escape") close(); });
        bag.add(function () { document.body.style.overflow = previousOverflow; });
        return bag.dispose;
      }
    };
  }

  function navMenuDefinition() {
    return {
      __panaBuiltIn: true,
      mount: function (root) {
        var bag = cleanupBag();
        var toggle = root.querySelector("[data-pana-nav-menu-toggle]");
        var list = root.querySelector("[data-pana-nav-menu-list]");
        if (!toggle || !list) return bag.dispose;
        var media = window.matchMedia ? window.matchMedia("(max-width: 720px)") : null;
        function compact() { return media ? media.matches : false; }
        function setOpen(open) {
          if (open) root.setAttribute("data-open", "");
          else root.removeAttribute("data-open");
          toggle.setAttribute("aria-expanded", open ? "true" : "false");
          list.hidden = compact() ? !open : false;
        }
        bag.listen(toggle, "click", function () { setOpen(!root.hasAttribute("data-open")); });
        bag.listen(root, "keydown", function (event) {
          if (event.key === "Escape") setOpen(false);
        });
        Array.prototype.forEach.call(list.querySelectorAll("a[href]"), function (link) {
          bag.listen(link, "click", function () { if (compact()) setOpen(false); });
        });
        if (media) {
          var sync = function () { setOpen(root.hasAttribute("data-open")); };
          if (media.addEventListener) bag.listen(media, "change", sync);
          else if (media.addListener) {
            media.addListener(sync);
            bag.add(function () { media.removeListener(sync); });
          }
        }
        setOpen(root.hasAttribute("data-open"));
        return bag.dispose;
      }
    };
  }

  function builtInDefinition(componentId) {
    if (componentId === "counter") return counterDefinition();
    if (componentId === "accordion") return accordionDefinition();
    if (componentId === "tabs") return tabsDefinition();
    if (componentId === "dialog") return overlayDefinition("dialog");
    if (componentId === "offcanvas") return overlayDefinition("offcanvas");
    if (componentId === "nav-menu") return navMenuDefinition();
    return null;
  }

  function installPageConfig(config) {
    var desired = Object.create(null);
    var components = config && Array.isArray(config.components) ? config.components : [];
    components.forEach(function (component) {
      var id = String(component && component.id || "").trim();
      var definition = builtInDefinition(id);
      if (!id || !definition) return;
      desired[id] = true;
      lifecycleDefinitions[id] = definition;
    });
    Object.keys(lifecycleDefinitions).forEach(function (id) {
      if (!lifecycleDefinitions[id].__panaBuiltIn || desired[id]) return;
      lifecycleInstances.filter(function (instance) { return instance.componentId === id; }).forEach(disposeInstance);
      lifecycleInstances = lifecycleInstances.filter(function (instance) { return instance.componentId !== id; });
      delete lifecycleDefinitions[id];
    });
    window.__panaMotionGraphConfig = config && config.motion ? config.motion : null;
    reconcile(document);
    post("page-config-installed", {
      componentCount: Object.keys(desired).length,
      motionItemCount: window.__panaMotionGraphConfig && Array.isArray(window.__panaMotionGraphConfig.items)
        ? window.__panaMotionGraphConfig.items.length
        : 0
    });
  }

  Object.defineProperty(window, "PanaInteractiveRuntime", {
    configurable: false,
    enumerable: false,
    writable: false,
    value: Object.freeze({
      register: function (componentId, definition) {
        var id = String(componentId || "").trim();
        if (!id || !definition || typeof definition !== "object") return false;
        lifecycleDefinitions[id] = definition;
        reconcile(document);
        return true;
      },
      installPageConfig: installPageConfig,
      reconcile: reconcile,
      dispose: dispose
    })
  });

  document.addEventListener("pana:components:init", function (event) {
    reconcile(event && event.detail && event.detail.root ? event.detail.root : document);
  });
  document.addEventListener("pana:components:dispose", function (event) {
    dispose(event && event.detail && event.detail.root ? event.detail.root : document);
  });
  window.addEventListener("pagehide", function () { dispose(document); }, { once: true });

  var observer = new MutationObserver(function () {
    reconcile(document);
    scheduleInspection("mutation");
  });

  function ready() {
    if (document.body) {
      observer.observe(document.body, { childList: true, subtree: true, attributes: true, characterData: true });
    }
    reconcile(document);
    var fonts = document.fonts && document.fonts.ready ? document.fonts.ready : Promise.resolve();
    Promise.race([
      fonts,
      new Promise(function (resolve) { window.setTimeout(resolve, 4000); })
    ]).then(function () {
      window.requestAnimationFrame(function () {
        window.requestAnimationFrame(function () {
          post("ready", { nodeCount: document.querySelectorAll("*").length });
          publishInspection("ready");
        });
      });
    });
  }

  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", ready, { once: true });
  else ready();
})();
