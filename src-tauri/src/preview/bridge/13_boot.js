  function mountedCanvasIdentity() {
    var root = document.documentElement;
    var workspaceRevision = Number(root.getAttribute("data-pana-canvas-workspace-revision"));
    var identity = {
      projectRoot: root.getAttribute("data-pana-canvas-project-root") || "",
      runtimeSessionId: root.getAttribute("data-pana-canvas-runtime-session-id") || "",
      workspaceRevision: workspaceRevision,
      transactionId: root.getAttribute("data-pana-canvas-transaction-id") || "",
      previewRevision: root.getAttribute(PREVIEW_REVISION_ATTR) || ""
    };
    if (!identity.projectRoot || !identity.runtimeSessionId || !Number.isSafeInteger(workspaceRevision) || workspaceRevision < 0 || !identity.transactionId || !identity.previewRevision) {
      return null;
    }
    return identity;
  }

  function boot() {
    var startedAt = performance.now();
    ensureInspectorStyles();
    applyTemplateSourceIdsFromMarkers();
    ensureElementSessionIds();
    refreshEmptyEditableZones();
    syncStructure();
    var identity = mountedCanvasIdentity();
    var committedAt = Math.max(0, Math.round(performance.now() - startedAt));
    waitForStyledFrame().then(function () {
      var styledReadyAt = Math.max(0, Math.round(performance.now() - startedAt));
      post("ready", {
        canvasIdentity: identity,
        canvasPhaseReceipts: [
          {
            schemaVersion: 1,
            identity: identity,
            phase: "resourcesReady",
            phaseTimingsMs: { resourcesReady: 0 },
            diagnostic: null
          },
          {
            schemaVersion: 1,
            identity: identity,
            phase: "committed",
            phaseTimingsMs: { resourcesReady: 0, committed: committedAt },
            diagnostic: null
          },
          {
            schemaVersion: 1,
            identity: identity,
            phase: "styledReady",
            phaseTimingsMs: {
              resourcesReady: 0,
              committed: committedAt,
              styledReady: styledReadyAt
            },
            diagnostic: null
          }
        ]
      });
    }).catch(function (error) {
      var message = error && error.message ? String(error.message) : String(error || "Canvas boot failed");
      post("ready", {
        canvasIdentity: identity,
        canvasPhaseReceipts: [{
          schemaVersion: 1,
          identity: identity,
          phase: "failed",
          phaseTimingsMs: { failed: Math.max(0, Math.round(performance.now() - startedAt)) },
          diagnostic: message
        }]
      });
    });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", boot, { once: true });
  } else {
    boot();
  }
})();
