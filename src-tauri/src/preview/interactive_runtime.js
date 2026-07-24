(function () {
  "use strict";

  var SOURCE = "pana-studio-interactive";
  var SCHEMA_VERSION = 1;
  var MAX_NODES = 5000;
  var MAX_DEPTH = 64;
  var MAX_TEXT = 160;
  var mutationTimer = 0;
  var lastPublishedAt = 0;
  var observer = null;

  function previewRevision() {
    return document.documentElement.getAttribute("data-pana-preview-revision") || "";
  }

  function post(type, payload) {
    window.parent.postMessage(Object.assign({
      source: SOURCE,
      schemaVersion: SCHEMA_VERSION,
      type: type,
      previewRevision: previewRevision()
    }, payload || {}), "*");
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

  function ready() {
    var runtime = window.PanaBlockRuntime;
    if (!runtime) {
      post("lifecycle-error", {
        blockId: null,
        phase: "bootstrap",
        message: "Runtime-ul canonic pentru blocuri nu a fost încărcat."
      });
      return;
    }
    runtime.setReporter(post);
    runtime.start();
    if (document.body) {
      observer = new MutationObserver(function () { scheduleInspection("mutation"); });
      observer.observe(document.body, {
        childList: true,
        subtree: true,
        attributes: true,
        characterData: true
      });
    }
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

  window.addEventListener("pagehide", function () {
    if (observer) observer.disconnect();
    if (mutationTimer) window.clearTimeout(mutationTimer);
    var runtime = window.PanaBlockRuntime;
    if (runtime) runtime.setReporter(null);
  }, { once: true });

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", ready, { once: true });
  } else {
    ready();
  }
})();
