(function () {
  // Run only when embedded in the Pana Studio iframe, not in external browsers.
  if (window === window.parent) return;

  var SOURCE_APP = "pana-studio-app";
  var SOURCE_PREVIEW = "pana-studio-preview";
  var INTERNAL_BRIDGE_ELEMENT = document.currentScript;
  var SELECTED_CLASS = "pana-studio-selected-element";
  var TEMPLATE_SELECTED_CLASS = "pana-studio-selected-template-source";
  var INSPECTOR_STYLE_ID = "pana-studio-inspector-style";
  var LIVE_OVERRIDES_ID = "pana-studio-live-overrides";
  var SESSION_ID_ATTR = "data-pana-session-id";
  var SOURCE_ID_ATTR = "data-pana-source-id";
  var TEMPLATE_SOURCE_ID_ATTR = "data-pana-template-source-id";
  var TEMPLATE_SOURCE_STACK_ATTR = "data-pana-template-source-stack";
  var PREVIEW_REVISION_ATTR = "data-pana-preview-revision";
  var EMPTY_TERA_SLOT_ATTR = "data-pana-empty-tera-slot";
  var EMPTY_HTML_ATTR = "data-pana-empty-html";
  var EMPTY_EDITABLE_CLASS = "pana-studio-empty-editable";
  var EMPTY_TERA_SLOT_CLASS = "pana-studio-empty-tera-slot";
  var HTML_SELECTION_ID = "pana-studio-html-selection";
  var TEMPLATE_GATE_ID = "pana-studio-template-gate";
  var TEMPLATE_GATE_ACTIONS_ID = "pana-studio-template-gate-actions";
  var PREVIEW_HOVER_ID = "pana-studio-preview-hover";
  var renderedHtmlSelectionElement = null;
  var renderedTemplateGateElements = [];
  var renderedTemplateGate = null;
  var previewHover = null;
  var previewHoverElements = [];
  var previewHoverRequestKey = null;
  var previewDragCandidate = null;
  var previewDragActive = false;
  var previewDragKind = "html";
  var previewDragSourceTeraId = null;
  var previewDragSourceElement = null;
  var previewDragTargetElement = null;
  var previewDragPosition = null;
  var previewDragInvalid = false;
  var previewDragSuppressClick = false;
  var previewInsertDragActive = false;
  var previewTeraInsertDragActive = false;
  var openTeraGateSourceIds = {};
  var nextSessionElementId = 1;
  var activePreviewOperationRevision = null;
  var SOURCE_ORIGIN_PALETTES = {
    local: {
      border: "#3b82f6",
      background: "rgba(59,130,246,0.07)",
      shadow: "rgba(59,130,246,0.18)",
      text: "#1d4ed8",
      label: "LOCAL",
      hint: "rgba(30,64,175,0.94)"
    },
    theme: {
      border: "#d97706",
      background: "rgba(217,119,6,0.07)",
      shadow: "rgba(217,119,6,0.18)",
      text: "#92400e",
      label: "THEME",
      hint: "rgba(146,64,14,0.94)"
    },
    current: {
      border: "#64748b",
      background: "rgba(100,116,139,0.07)",
      shadow: "rgba(100,116,139,0.18)",
      text: "#334155",
      label: "CURRENT",
      hint: "rgba(51,65,85,0.94)"
    },
    unknown: {
      border: "#1d7f6a",
      background: "rgba(29,127,106,0.08)",
      shadow: "rgba(29,127,106,0.18)",
      text: "#0f766e",
      label: "SOURCE",
      hint: "rgba(18,87,74,0.94)"
    }
  };

  // Only user-agent generated events may enter a gesture path. This guard is
  // deliberately evaluated inside the bridge; `isTrusted` received as message
  // data is not provenance and must never be accepted as an equivalent.
  function isTrustedPreviewGesture(event) {
    return Boolean(event && event.isTrusted === true);
  }

  function sourceOriginPalette(origin) {
    return SOURCE_ORIGIN_PALETTES[origin] || SOURCE_ORIGIN_PALETTES.unknown;
  }

  function post(type, payload) {
    var message = Object.assign({ source: SOURCE_PREVIEW, type: type }, payload || {});
    if (activePreviewOperationRevision) {
      message.previewRevision = activePreviewOperationRevision;
    }
    window.parent.postMessage(
      message,
      "*"
    );
  }
