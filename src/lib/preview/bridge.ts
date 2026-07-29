export function escapeHtmlAttribute(value: string) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

const applicationAccentProperty = "--pana-studio-accent";
const applicationTextOnAccentProperty = "--pana-studio-text-on-accent";

function normalizedApplicationColor(value: string, fallback: string) {
  return /^#[0-9a-f]{6}$/i.test(value) ? value.toLowerCase() : fallback;
}

export function applyApplicationAppearanceToPreviewDocument(
  previewDocument: Document,
  accent: string,
  textOnAccent: string,
) {
  previewDocument.documentElement.style.setProperty(
    applicationAccentProperty,
    normalizedApplicationColor(accent, "#1d7f6a"),
  );
  previewDocument.documentElement.style.setProperty(
    applicationTextOnAccentProperty,
    normalizedApplicationColor(textOnAccent, "#ffffff"),
  );
}

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
