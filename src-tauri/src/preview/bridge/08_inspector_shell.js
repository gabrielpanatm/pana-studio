  function ensureInspectorStyles() {
    var styleElement = document.getElementById(INSPECTOR_STYLE_ID);
    if (!styleElement) {
      styleElement = document.createElement("style");
      styleElement.id = INSPECTOR_STYLE_ID;
      styleElement.setAttribute("data-pana-internal-style", "");
      // Design is a static authoring surface. Authored motion stays available
      // in Motion/Interactive, but must not keep the editor compositor hot.
      styleElement.textContent =
        "* { cursor: crosshair !important; }\n" +
        "*, *::before, *::after { animation-play-state: paused !important; transition-duration: 0s !important; transition-delay: 0s !important; scroll-behavior: auto !important; }\n" +
        "html, body, body * { user-select: none !important; -webkit-user-select: none !important; }\n" +
        "input, textarea, select, [contenteditable='true'], input *, textarea *, select *, [contenteditable='true'] * { user-select: text !important; -webkit-user-select: text !important; }\n" +
        "body.pana-studio-preview-drag-candidate, body.pana-studio-preview-drag-candidate *, body.pana-studio-preview-dragging, body.pana-studio-preview-dragging * { cursor: grabbing !important; user-select: none !important; -webkit-user-select: none !important; }\n" +
        "#pana-studio-preview-drop-line { position: fixed; z-index: 2147483647; height: 0; border-top: 3px solid var(--pana-studio-accent, #1d7f6a); pointer-events: none; display: none; }\n" +
        "#pana-studio-preview-drop-line::before { content: ''; position: absolute; left: -5px; top: -6px; width: 9px; height: 9px; border-radius: 999px; background: var(--pana-studio-accent, #1d7f6a); }\n" +
        "#pana-studio-preview-drop-box { position: fixed; z-index: 2147483647; border: 3px solid var(--pana-studio-accent, #1d7f6a); background: color-mix(in srgb, var(--pana-studio-accent, #1d7f6a) 8%, transparent); pointer-events: none; display: none; box-sizing: border-box; }\n" +
        "#pana-studio-preview-drop-hint { position: fixed; z-index: 2147483647; max-width: 260px; padding: 6px 8px; border-radius: 7px; color: var(--pana-studio-text-on-accent, #ffffff); font: 700 12px/1.25 system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; background: color-mix(in srgb, var(--pana-studio-accent, #1d7f6a) 94%, transparent); pointer-events: none; display: none; box-shadow: 0 8px 24px rgba(0,0,0,0.22); }\n" +
        "#pana-studio-preview-drop-line.tera { border-top-color: var(--pana-studio-entity-template, #2563eb); }\n" +
        "#pana-studio-preview-drop-line.tera::before { background: var(--pana-studio-entity-template, #2563eb); }\n" +
        "#pana-studio-preview-drop-box.tera { border-color: var(--pana-studio-entity-template, #2563eb); background: color-mix(in srgb, var(--pana-studio-entity-template, #2563eb) 8%, transparent); }\n" +
        "#pana-studio-preview-drop-hint.tera { background: color-mix(in srgb, var(--pana-studio-entity-template, #2563eb) 94%, transparent); }\n" +
        "#pana-studio-preview-drop-line.invalid { border-top-color: var(--pana-studio-danger, #d94b4b); }\n" +
        "#pana-studio-preview-drop-line.invalid::before { background: var(--pana-studio-danger, #d94b4b); }\n" +
        "#pana-studio-preview-drop-box.invalid { border-color: var(--pana-studio-danger, #d94b4b); background: color-mix(in srgb, var(--pana-studio-danger, #d94b4b) 8%, transparent); }\n" +
        "#pana-studio-preview-drop-hint.invalid { background: color-mix(in srgb, var(--pana-studio-danger, #d94b4b) 94%, transparent); }\n" +
        "." + EMPTY_EDITABLE_CLASS + " { min-width: min(220px, 100%) !important; min-height: 44px !important; outline: 1px dashed color-mix(in srgb, var(--pana-studio-accent, #1d7f6a) 58%, transparent) !important; outline-offset: -1px !important; background-image: linear-gradient(135deg, color-mix(in srgb, var(--pana-studio-accent, #1d7f6a) 7%, transparent), color-mix(in srgb, var(--pana-studio-accent, #1d7f6a) 2%, transparent)) !important; position: relative !important; box-sizing: border-box !important; }\n" +
        "." + EMPTY_EDITABLE_CLASS + "[data-pana-empty-label]::before { content: attr(data-pana-empty-label); position: absolute; left: 10px; top: 9px; padding: 3px 7px; border-radius: 999px; color: color-mix(in srgb, var(--pana-studio-accent, #1d7f6a) 68%, #111111); background: rgba(255,255,255,0.92); border: 1px solid color-mix(in srgb, var(--pana-studio-accent, #1d7f6a) 22%, transparent); font: 800 11px/1.2 system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; pointer-events: none; white-space: nowrap; }\n" +
        "." + ACTIVE_DOCUMENT_ROOT_CLASS + " { display: block !important; min-height: var(--pana-active-authoring-min-height, 52px) !important; outline-color: color-mix(in srgb, var(--pana-studio-accent, #1d7f6a) 62%, transparent) !important; background-image: linear-gradient(135deg, color-mix(in srgb, var(--pana-studio-accent, #1d7f6a) 8%, transparent), color-mix(in srgb, var(--pana-studio-accent, #1d7f6a) 2%, transparent)) !important; }\n" +
        "." + ACTIVE_DOCUMENT_ROOT_CLASS + "[data-pana-active-document-populated] { height: 0 !important; min-height: 0 !important; margin: 0 !important; padding: 0 !important; border: 0 !important; outline: 0 !important; background: none !important; line-height: 0 !important; }\n" +
        "." + EMPTY_TERA_SLOT_CLASS + "[data-pana-empty-tera-slot-static][hidden] { display: none !important; }\n" +
        "." + ACTIVE_DOCUMENT_ROOT_CLASS + "[data-pana-empty-label]::before { color: var(--pana-studio-accent, #1d7f6a); border-color: color-mix(in srgb, var(--pana-studio-accent, #1d7f6a) 24%, transparent); }";
      document.head.appendChild(styleElement);
    }
  }

  function isTextEditingTarget(element) {
    return element instanceof Element &&
      Boolean(element.closest("input, textarea, select, [contenteditable='true']"));
  }

  function handlePreviewShortcut(event) {
    if (!isTrustedPreviewGesture(event)) return;
    if ((!event.ctrlKey && !event.metaKey) || event.altKey) return;
    var key = String(event.key || "").toLowerCase();
    if (key !== "s" && key !== "z") return;
    if (key === "z" && isTextEditingTarget(event.target)) return;

    event.preventDefault();
    event.stopPropagation();
    post("preview-shortcut", {
      shortcut: key === "s" ? "save" : (event.shiftKey ? "redo" : "undo")
    });
  }
