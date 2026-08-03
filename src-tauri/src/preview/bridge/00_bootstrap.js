(function () {
  // Run only when embedded in the Pana Studio iframe, not in external browsers.
  if (window === window.parent) return;

  var SOURCE_APP = "pana-studio-app";
  var SOURCE_PREVIEW = "pana-studio-preview";
  var INTERNAL_BRIDGE_ELEMENT = document.currentScript;
  var INSPECTOR_STYLE_ID = "pana-studio-inspector-style";
  var LIVE_OVERRIDES_ID = "pana-studio-live-overrides";
  var SESSION_ID_ATTR = "data-pana-session-id";
  var SOURCE_ID_ATTR = "data-pana-source-id";
  var TEMPLATE_SOURCE_ID_ATTR = "data-pana-template-source-id";
  var TEMPLATE_SOURCE_STACK_ATTR = "data-pana-template-source-stack";
  var PREVIEW_REVISION_ATTR = "data-pana-preview-revision";
  var EMPTY_TERA_SLOT_ATTR = "data-pana-empty-tera-slot";
  var ACTIVE_DOCUMENT_ROOT_ATTR = "data-pana-active-document-root";
  var EMPTY_HTML_ATTR = "data-pana-empty-html";
  var EMPTY_EDITABLE_CLASS = "pana-studio-empty-editable";
  var EMPTY_TERA_SLOT_CLASS = "pana-studio-empty-tera-slot";
  var ACTIVE_DOCUMENT_ROOT_CLASS = "pana-studio-active-document-root";
  var previewInsertDragActive = false;
  var previewTeraInsertDragActive = false;
  var nextSessionElementId = 1;
  var activePreviewOperationRevision = null;
  var APPLICATION_ACCENT_PROPERTY = "--pana-studio-accent";
  var APPLICATION_TEXT_ON_ACCENT_PROPERTY = "--pana-studio-text-on-accent";
  var applicationAppearance = {
    accent: "#1d7f6a",
    textOnAccent: "#ffffff"
  };

  function normalizedApplicationColor(value, fallback) {
    return typeof value === "string" && /^#[0-9a-f]{6}$/i.test(value)
      ? value.toLowerCase()
      : fallback;
  }

  function applyApplicationAppearance(data) {
    applicationAppearance = {
      accent: normalizedApplicationColor(
        data && data.accent,
        applicationAppearance.accent
      ),
      textOnAccent: normalizedApplicationColor(
        data && data.textOnAccent,
        applicationAppearance.textOnAccent
      )
    };
    restoreApplicationAppearance();
  }

  function restoreApplicationAppearance() {
    document.documentElement.style.setProperty(
      APPLICATION_ACCENT_PROPERTY,
      applicationAppearance.accent
    );
    document.documentElement.style.setProperty(
      APPLICATION_TEXT_ON_ACCENT_PROPERTY,
      applicationAppearance.textOnAccent
    );
  }

  // Only user-agent generated events may enter a gesture path. This guard is
  // deliberately evaluated inside the bridge; `isTrusted` received as message
  // data is not provenance and must never be accepted as an equivalent.
  function isTrustedPreviewGesture(event) {
    return Boolean(event && event.isTrusted === true);
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
