import type { ApplicationBootProjection } from "$lib/application/contracts";

export const APPLICATION_BOOT_PROJECTION_STORAGE_KEY =
  "pana-studio-boot-projection-v1";

function validInteger(value: unknown) {
  return Number.isSafeInteger(value) && Number(value) >= 0;
}

function boundedText(value: unknown, maximumLength: number) {
  return typeof value === "string"
    && value.length > 0
    && value.length <= maximumLength;
}

export function isApplicationBootProjection(
  value: unknown,
): value is ApplicationBootProjection {
  if (!value || typeof value !== "object") return false;
  const projection = value as Record<string, unknown>;
  return projection.schemaVersion === 1
    && projection.authority === "rust_application_settings"
    && projection.settingsSchemaVersion === 3
    && validInteger(projection.settingsRevision)
    && validInteger(projection.systemGeneration)
    && boundedText(projection.locale, 64)
    && (projection.direction === "ltr" || projection.direction === "rtl")
    && (projection.theme === "light" || projection.theme === "dark")
    && typeof projection.accent === "string"
    && /^#[0-9a-f]{6}$/i.test(projection.accent)
    && (
      projection.contrast === null
      || projection.contrast === "normal"
      || projection.contrast === "high"
    )
    && (
      projection.reducedMotion === null
      || typeof projection.reducedMotion === "boolean"
    )
    && boundedText(projection.loadingLabel, 200)
    && boundedText(projection.loadingSubtitle, 300);
}

export function storeApplicationBootProjection(
  storage: Pick<Storage, "setItem">,
  projection: ApplicationBootProjection,
) {
  if (!isApplicationBootProjection(projection)) return false;
  try {
    storage.setItem(
      APPLICATION_BOOT_PROJECTION_STORAGE_KEY,
      JSON.stringify(projection),
    );
    return true;
  } catch {
    return false;
  }
}

export function applyApplicationBootProjection(
  document_: Document,
  projection: ApplicationBootProjection,
) {
  if (!isApplicationBootProjection(projection)) return false;
  const root = document_.documentElement;
  const background = projection.theme === "light" ? "#edf1ee" : "#111315";
  const strongMixTarget = projection.theme === "dark" ? "white" : "black";

  root.lang = projection.locale;
  root.dir = projection.direction;
  root.dataset.panaLocale = projection.locale;
  root.dataset.panaTheme = projection.theme;
  root.dataset.panaContrast = projection.contrast ?? "normal";
  root.dataset.panaReducedMotion =
    projection.reducedMotion === true ? "true" : "false";
  root.dataset.panaBootAuthority =
    `${projection.settingsRevision}:${projection.systemGeneration}`;
  root.style.colorScheme = projection.theme;
  root.style.background = background;
  root.style.setProperty("--boot-brand", projection.accent);
  root.style.setProperty(
    "--boot-brand-strong",
    `color-mix(in srgb, ${projection.accent} 70%, ${strongMixTarget})`,
  );

  document_
    .querySelector('meta[name="theme-color"]')
    ?.setAttribute("content", background);
  const bootScreen = document_.getElementById("pana-boot-screen");
  if (bootScreen) {
    bootScreen.setAttribute("aria-label", projection.loadingLabel);
    const subtitle = bootScreen.querySelector<HTMLElement>(".boot-subtitle");
    if (subtitle) subtitle.textContent = projection.loadingSubtitle;
  }
  return true;
}
