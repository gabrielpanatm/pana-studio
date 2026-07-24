(function () {
  "use strict";

  if (window.PanaBlockRuntime) return;

  var definitions = Object.create(null);
  var instances = [];
  var activeIds = null;
  var observer = null;
  var reporter = null;
  var lastPageConfigReceipt = null;

  function report(type, payload) {
    if (typeof reporter !== "function") return;
    try { reporter(type, payload || {}); } catch (_) {}
  }

  function blockSelector(id) {
    var escaped = String(id || "").replace(/["\\]/g, "\\$&");
    return '[data-pana-block="' + escaped + '"],[data-pana-component="' + escaped + '"]';
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
      "data-pana-component",
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

  function isRootOptionMutation(record) {
    if (!record || record.type !== "attributes" || !isContractOptionAttribute(record.attributeName || "")) {
      return false;
    }
    var element = record.target;
    if (!element || element.nodeType !== 1) return false;
    var blockId = element.getAttribute("data-pana-block") || element.getAttribute("data-pana-component");
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
          if (existing && existing.optionSignature !== signature) {
            disposeInstance(existing);
            instances = instances.filter(function (instance) { return instance !== existing; });
            existing = null;
            report("lifecycle", { blockId: blockId, phase: "remount-options" });
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
    reconcile(document);
    return true;
  }

  function installPageConfig(config) {
    var entries = config && Array.isArray(config.blocks)
      ? config.blocks
      : config && Array.isArray(config.components)
        ? config.components
        : [];
    activeIds = Object.create(null);
    entries.forEach(function (entry) {
      var id = String(entry && entry.id || "").trim();
      if (id && definitions[id]) activeIds[id] = true;
    });
    window.__panaMotionGraphConfig = config && config.motion ? config.motion : null;
    reconcile(document);
    lastPageConfigReceipt = {
      blockCount: Object.keys(activeIds).length,
      motionItemCount: window.__panaMotionGraphConfig && Array.isArray(window.__panaMotionGraphConfig.items)
        ? window.__panaMotionGraphConfig.items.length
        : 0
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
  // Compatibilitate pentru proiectele create înaintea separării Blocuri/Componente.
  document.addEventListener("pana:components:init", function (event) {
    reconcile(event && event.detail && event.detail.root ? event.detail.root : document);
  });
  document.addEventListener("pana:components:dispose", function (event) {
    dispose(event && event.detail && event.detail.root ? event.detail.root : document);
  });
  window.addEventListener("pagehide", shutdown, { once: true });
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", start, { once: true });
  } else {
    start();
  }

  function instanceToken(root, fallback) {
    return root.getAttribute("data-pana-instance") || fallback;
  }

  register("counter", {
    mount: function (element, makeBag) {
      var bag = makeBag();
      var observerHandle = null;
      var animationFrame = 0;
      var started = false;
      function run() {
        if (started) return;
        started = true;
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
          if (progress < 1) animationFrame = window.requestAnimationFrame(tick);
          else element.textContent = String(target) + suffix;
        }
        animationFrame = window.requestAnimationFrame(tick);
      }
      if ("IntersectionObserver" in window) {
        observerHandle = new IntersectionObserver(function (entries) {
          entries.forEach(function (entry) {
            if (!entry.isIntersecting) return;
            run();
            observerHandle.unobserve(entry.target);
          });
        }, { threshold: 0.3 });
        observerHandle.observe(element);
      } else {
        run();
      }
      bag.add(function () {
        if (observerHandle) observerHandle.disconnect();
        if (animationFrame) window.cancelAnimationFrame(animationFrame);
      });
      return { state: { started: function () { return started; } }, dispose: bag.dispose };
    }
  });

  register("accordion", {
    mount: function (root, makeBag) {
      var bag = makeBag();
      var allowMultiple = root.getAttribute("data-multiple") === "true";
      var token = instanceToken(root, "accordion");
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
        trigger.id = trigger.id || token + "-trigger-" + index;
        panel.id = panel.id || token + "-panel-" + index;
        if (trigger.localName === "button" && !trigger.getAttribute("type")) trigger.setAttribute("type", "button");
        trigger.setAttribute("aria-controls", panel.id);
        panel.setAttribute("role", "region");
        panel.setAttribute("aria-labelledby", trigger.id);
        setOpen(item, trigger, panel, trigger.getAttribute("aria-expanded") === "true" || item.hasAttribute("data-open"));
        bag.listen(trigger, "click", function () {
          var shouldOpen = trigger.getAttribute("aria-expanded") !== "true";
          if (shouldOpen && !allowMultiple) {
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
      return bag.dispose;
    }
  });

  register("tabs", {
    mount: function (root, makeBag) {
      var bag = makeBag();
      var token = instanceToken(root, "tabs");
      var tabs = Array.prototype.slice.call(root.querySelectorAll("[data-pana-tabs-tab]"));
      var panels = Array.prototype.slice.call(root.querySelectorAll("[data-pana-tabs-panel]"));
      if (!tabs.length || !panels.length) return bag.dispose;
      function activate(index, focus) {
        tabs.forEach(function (tab, tabIndex) {
          var active = tabIndex === index;
          tab.setAttribute("aria-selected", active ? "true" : "false");
          tab.setAttribute("tabindex", active ? "0" : "-1");
          if (active && focus && tab.focus) tab.focus();
        });
        panels.forEach(function (panel, panelIndex) { panel.hidden = panelIndex !== index; });
      }
      var selected = parseInt(root.getAttribute("data-default-tab") || "0", 10);
      if (!isFinite(selected) || selected < 0 || selected >= Math.min(tabs.length, panels.length)) {
        selected = 0;
      }
      tabs.forEach(function (tab, index) {
        var panel = panels[index];
        if (!panel) return;
        tab.id = tab.id || token + "-tab-" + index;
        panel.id = panel.id || token + "-panel-" + index;
        if (tab.localName === "button" && !tab.getAttribute("type")) tab.setAttribute("type", "button");
        tab.setAttribute("role", "tab");
        tab.setAttribute("aria-controls", panel.id);
        panel.setAttribute("role", "tabpanel");
        panel.setAttribute("aria-labelledby", tab.id);
        bag.listen(tab, "click", function () { activate(index, false); });
        bag.listen(tab, "keydown", function (event) {
          if (["ArrowRight", "ArrowLeft", "Home", "End"].indexOf(event.key) < 0) return;
          event.preventDefault();
          var next = event.key === "Home" ? 0
            : event.key === "End" ? tabs.length - 1
              : event.key === "ArrowRight" ? (index + 1) % tabs.length
                : (index - 1 + tabs.length) % tabs.length;
          activate(next, true);
        });
      });
      activate(selected, false);
      return bag.dispose;
    }
  });

  function overlayDefinition(kind, delayedClose) {
    var prefix = "data-pana-" + kind;
    return {
      mount: function (root, makeBag) {
        var bag = makeBag();
        var token = instanceToken(root, kind);
        var openers = Array.prototype.slice.call(root.querySelectorAll("[" + prefix + "-open]"));
        var closers = Array.prototype.slice.call(root.querySelectorAll("[" + prefix + "-close]"));
        var overlay = root.querySelector("[" + prefix + "-overlay]");
        var panel = root.querySelector("[" + prefix + "-panel]");
        var title = root.querySelector("[" + prefix + "-title]");
        var previousActive = null;
        var previousOverflow = "";
        var closeTimer = 0;
        var openFrame = 0;
        var closeOnBackdrop = root.getAttribute("data-close-outside") !== "false";
        var closeOnEscape = root.getAttribute("data-close-escape") !== "false";
        if (!overlay || !panel) return bag.dispose;
        panel.id = panel.id || token + "-panel";
        panel.setAttribute("role", "dialog");
        panel.setAttribute("aria-modal", "true");
        if (!panel.getAttribute("tabindex")) panel.setAttribute("tabindex", "-1");
        if (title) {
          title.id = title.id || token + "-title";
          panel.setAttribute("aria-labelledby", title.id);
        }
        openers.forEach(function (opener) {
          if (opener.localName === "button" && !opener.getAttribute("type")) opener.setAttribute("type", "button");
          opener.setAttribute("aria-haspopup", "dialog");
          opener.setAttribute("aria-controls", panel.id);
        });
        closers.forEach(function (closer) {
          if (closer.localName === "button" && !closer.getAttribute("type")) closer.setAttribute("type", "button");
        });
        function expanded(open) {
          openers.forEach(function (opener) { opener.setAttribute("aria-expanded", open ? "true" : "false"); });
        }
        function show(opener) {
          if (closeTimer) window.clearTimeout(closeTimer);
          previousActive = document.activeElement;
          previousOverflow = document.body.style.overflow || "";
          overlay.hidden = false;
          document.body.style.overflow = "hidden";
          expanded(true);
          openFrame = window.requestAnimationFrame(function () {
            root.setAttribute("data-open", "");
            var focus = panel.querySelector("button:not([disabled]),[href],input:not([disabled]),select:not([disabled]),textarea:not([disabled]),[tabindex]:not([tabindex='-1'])") || panel;
            if (focus && focus.focus) focus.focus();
          });
        }
        function hide(restoreFocus) {
          if (openFrame) window.cancelAnimationFrame(openFrame);
          root.removeAttribute("data-open");
          document.body.style.overflow = previousOverflow;
          expanded(false);
          if (delayedClose) {
            closeTimer = window.setTimeout(function () {
              if (!root.hasAttribute("data-open")) overlay.hidden = true;
            }, 240);
          } else {
            overlay.hidden = true;
          }
          if (restoreFocus !== false && previousActive && previousActive.focus && document.contains(previousActive)) {
            previousActive.focus();
          }
        }
        openers.forEach(function (opener) { bag.listen(opener, "click", function () { show(opener); }); });
        closers.forEach(function (closer) { bag.listen(closer, "click", function () { hide(true); }); });
        bag.listen(overlay, "click", function (event) {
          if (closeOnBackdrop && event.target === overlay) hide(true);
        });
        bag.listen(overlay, "keydown", function (event) {
          if (closeOnEscape && event.key === "Escape") hide(true);
        });
        bag.add(function () {
          if (closeTimer) window.clearTimeout(closeTimer);
          if (openFrame) window.cancelAnimationFrame(openFrame);
          if (root.hasAttribute("data-open")) document.body.style.overflow = previousOverflow;
        });
        expanded(!overlay.hidden);
        return bag.dispose;
      }
    };
  }

  register("dialog", overlayDefinition("dialog", false));
  register("offcanvas", overlayDefinition("offcanvas", true));

  register("nav-menu", {
    mount: function (root, makeBag) {
      var bag = makeBag();
      var token = instanceToken(root, "nav-menu");
      var toggle = root.querySelector("[data-pana-nav-menu-toggle]");
      var list = root.querySelector("[data-pana-nav-menu-list]");
      if (!toggle || !list) return bag.dispose;
      var media = window.matchMedia ? window.matchMedia("(max-width: 720px)") : null;
      var closeOnSelect = root.getAttribute("data-close-on-select") !== "false";
      list.id = list.id || token + "-list";
      toggle.setAttribute("aria-controls", list.id);
      if (toggle.localName === "button" && !toggle.getAttribute("type")) toggle.setAttribute("type", "button");
      function compact() { return media ? media.matches : false; }
      function setOpen(open) {
        if (open) root.setAttribute("data-open", "");
        else root.removeAttribute("data-open");
        toggle.setAttribute("aria-expanded", open ? "true" : "false");
        list.hidden = compact() ? !open : false;
      }
      bag.listen(toggle, "click", function () { setOpen(!root.hasAttribute("data-open")); });
      bag.listen(root, "keydown", function (event) {
        if (event.key !== "Escape" || !root.hasAttribute("data-open")) return;
        setOpen(false);
        if (toggle.focus) toggle.focus();
      });
      Array.prototype.forEach.call(list.querySelectorAll("a[href]"), function (link) {
        bag.listen(link, "click", function () {
          if (closeOnSelect && compact()) setOpen(false);
        });
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
  });
})();
