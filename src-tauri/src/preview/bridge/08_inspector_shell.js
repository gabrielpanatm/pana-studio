  function ensureInspectorStyles() {
    var styleElement = document.getElementById(INSPECTOR_STYLE_ID);
    if (!styleElement) {
      styleElement = document.createElement("style");
      styleElement.id = INSPECTOR_STYLE_ID;
      styleElement.setAttribute("data-pana-internal-style", "");
      styleElement.textContent =
        "* { cursor: crosshair !important; }\n" +
        "html, body, body * { user-select: none !important; -webkit-user-select: none !important; }\n" +
        "input, textarea, select, [contenteditable='true'], input *, textarea *, select *, [contenteditable='true'] * { user-select: text !important; -webkit-user-select: text !important; }\n" +
        "body.pana-studio-preview-drag-candidate, body.pana-studio-preview-drag-candidate *, body.pana-studio-preview-dragging, body.pana-studio-preview-dragging * { cursor: grabbing !important; user-select: none !important; -webkit-user-select: none !important; }\n" +
        "#pana-studio-preview-drop-line { position: fixed; z-index: 2147483647; height: 0; border-top: 3px solid #1d7f6a; pointer-events: none; display: none; }\n" +
        "#pana-studio-preview-drop-line::before { content: ''; position: absolute; left: -5px; top: -6px; width: 9px; height: 9px; border-radius: 999px; background: #1d7f6a; }\n" +
        "#pana-studio-preview-drop-box { position: fixed; z-index: 2147483647; border: 3px solid #1d7f6a; background: rgba(29,127,106,0.08); pointer-events: none; display: none; box-sizing: border-box; }\n" +
        "#pana-studio-preview-drop-hint { position: fixed; z-index: 2147483647; max-width: 260px; padding: 6px 8px; border-radius: 7px; color: #ffffff; font: 700 12px/1.25 system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; background: rgba(18, 87, 74, 0.94); pointer-events: none; display: none; box-shadow: 0 8px 24px rgba(0,0,0,0.22); }\n" +
        "#pana-studio-preview-drop-line.tera { border-top-color: #3b82f6; }\n" +
        "#pana-studio-preview-drop-line.tera::before { background: #3b82f6; }\n" +
        "#pana-studio-preview-drop-box.tera { border-color: #3b82f6; background: rgba(59,130,246,0.08); }\n" +
        "#pana-studio-preview-drop-hint.tera { background: rgba(30,64,175,0.94); }\n" +
        "#pana-studio-preview-drop-line.invalid { border-top-color: #dc2626; }\n" +
        "#pana-studio-preview-drop-line.invalid::before { background: #dc2626; }\n" +
        "#pana-studio-preview-drop-box.invalid { border-color: #dc2626; background: rgba(220,38,38,0.08); }\n" +
        "#pana-studio-preview-drop-hint.invalid { background: rgba(185,28,28,0.94); }\n" +
        "." + EMPTY_EDITABLE_CLASS + " { min-width: min(220px, 100%) !important; min-height: 44px !important; outline: 1px dashed rgba(29,127,106,0.58) !important; outline-offset: -1px !important; background-image: linear-gradient(135deg, rgba(29,127,106,0.07), rgba(29,127,106,0.02)) !important; position: relative !important; box-sizing: border-box !important; }\n" +
        "." + EMPTY_EDITABLE_CLASS + "::before { content: attr(data-pana-empty-label); position: absolute; left: 10px; top: 9px; padding: 3px 7px; border-radius: 999px; color: #0f766e; background: rgba(255,255,255,0.92); border: 1px solid rgba(29,127,106,0.22); font: 800 11px/1.2 system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; pointer-events: none; white-space: nowrap; }\n" +
        "." + EMPTY_TERA_SLOT_CLASS + " { display: block !important; min-height: 52px !important; outline-color: rgba(59,130,246,0.62) !important; background-image: linear-gradient(135deg, rgba(59,130,246,0.08), rgba(59,130,246,0.02)) !important; }\n" +
        "." + EMPTY_TERA_SLOT_CLASS + "::before { color: #1d4ed8; border-color: rgba(59,130,246,0.24); }";
      document.head.appendChild(styleElement);
    }
  }

  function postDeleteSelected() {
    var current = currentSelectedElement();
    if (!current || current === document.body || current === document.documentElement) return;
    var target = createSelectionInfo(current);
    post("preview-delete-selected", {
      selector: target.domPath,
      sourceId: target.sourceId,
      templateSourceId: target.templateSourceId,
      sessionId: target.sessionId,
      sourceSessionId: target.sessionId,
      sourceTag: target.tag,
      target: target
    });
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
