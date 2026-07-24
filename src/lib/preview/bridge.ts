export function escapeHtmlAttribute(value: string) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

const htmlSelectionOverlayId = "pana-studio-html-selection";

export function buildPreviewStatusDocument(title: string, message: string) {
  return `<!doctype html>
    <html lang="en">
      <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>${escapeHtmlAttribute(title)}</title>
        <style>
          :root {
            color-scheme: light dark;
          }
          body {
            margin: 0;
            min-height: 100vh;
            padding: 24px;
            font-family: Inter, system-ui, sans-serif;
            background: #f4f7f5;
            color: #17211d;
          }
          .status-card {
            max-width: 720px;
            padding: 18px 20px;
            border: 1px solid #bfd2c9;
            border-radius: 14px;
            background: #ffffff;
            box-shadow: 0 10px 24px rgba(20, 25, 22, 0.08);
          }
          h1 {
            margin: 0 0 10px;
            font-size: 18px;
          }
          p {
            margin: 0;
            line-height: 1.5;
            white-space: pre-wrap;
          }
        </style>
      </head>
      <body>
        <div class="status-card">
          <h1>${escapeHtmlAttribute(title)}</h1>
          <p>${escapeHtmlAttribute(message)}</p>
        </div>
      </body>
    </html>`;
}

export function ensurePreviewInspectorStyles(previewDocument: Document) {
  if (previewDocument.getElementById("pana-studio-inspector-style")) {
    return;
  }

  const styleElement = previewDocument.createElement("style");
  styleElement.id = "pana-studio-inspector-style";
  // Inspector mode must override cursor declarations from the inspected project,
  // not application chrome; this is an intentional preview-boundary override.
  styleElement.textContent = `
    * {
      cursor: crosshair !important;
    }
  `;

  previewDocument.head.append(styleElement);
}

function ensurePreviewHtmlSelectionOverlay(previewDocument: Document) {
  let overlay = previewDocument.getElementById(htmlSelectionOverlayId) as HTMLDivElement | null;
  if (!overlay) {
    overlay = previewDocument.createElement("div");
    overlay.id = htmlSelectionOverlayId;
    overlay.style.cssText = [
      "position: fixed",
      "z-index: 2147483646",
      "display: none",
      "border: 1px solid #1d7f6a",
      "border-radius: 0",
      "background: transparent",
      "box-shadow: none",
      "pointer-events: none",
      "box-sizing: border-box",
    ].join(";");
    previewDocument.body.append(overlay);
  }
  return overlay;
}

export function hidePreviewHtmlSelectionOverlay(previewDocument: Document | null | undefined) {
  const overlay = previewDocument?.getElementById(htmlSelectionOverlayId);
  if (overlay) overlay.style.display = "none";
}

export function updatePreviewHtmlSelectionOverlay(element: Element | null | undefined) {
  if (!element?.isConnected) {
    hidePreviewHtmlSelectionOverlay(element?.ownerDocument);
    return;
  }

  const previewDocument = element.ownerDocument;
  const rect = element.getBoundingClientRect();
  if (rect.width <= 0 && rect.height <= 0) {
    hidePreviewHtmlSelectionOverlay(previewDocument);
    return;
  }

  const overlay = ensurePreviewHtmlSelectionOverlay(previewDocument);
  const computed = previewDocument.defaultView?.getComputedStyle(element);
  overlay.style.display = "block";
  overlay.style.left = `${Math.round(rect.left)}px`;
  overlay.style.top = `${Math.round(rect.top)}px`;
  overlay.style.width = `${Math.round(rect.width)}px`;
  overlay.style.height = `${Math.round(rect.height)}px`;
  overlay.style.borderRadius = computed?.borderRadius || "0px";
}

export function applyStagedOverrideStylesToDocument(previewDocument: Document, css: string) {
  if (!previewDocument.head) {
    return;
  }

  let styleElement = previewDocument.getElementById("pana-studio-live-overrides") as HTMLStyleElement | null;

  if (!styleElement) {
    styleElement = previewDocument.createElement("style");
    styleElement.id = "pana-studio-live-overrides";
    previewDocument.head.append(styleElement);
  }

  styleElement.textContent = css;
}
