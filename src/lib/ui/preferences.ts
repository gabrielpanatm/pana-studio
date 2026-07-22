export type UiTheme = "dark" | "light";

export type StoredUiPreferences = {
  theme: UiTheme | null;
  leftPaneWidth: number | null;
  rightPaneWidth: number | null;
  terminalPaneHeight: number | null;
};

const uiThemeKey = "pana-studio-ui-theme";
const leftPaneWidthKey = "pana-studio-left-pane-width";
const rightPaneWidthKey = "pana-studio-right-pane-width";
const terminalPaneHeightKey = "pana-studio-terminal-height";
const uiDensityVersionKey = "pana-studio-ui-density-version";
const currentUiDensityVersion = "2";

export function loadStoredUiPreferences(storage: Storage): StoredUiPreferences {
  const storedTheme = storage.getItem(uiThemeKey);
  const densityVersion = storage.getItem(uiDensityVersionKey);
  const shouldResetPaneDimensions = densityVersion !== currentUiDensityVersion;
  if (shouldResetPaneDimensions) {
    storage.setItem(uiDensityVersionKey, currentUiDensityVersion);
  }

  return {
    theme: storedTheme === "dark" || storedTheme === "light" ? storedTheme : null,
    leftPaneWidth: shouldResetPaneDimensions ? null : parseStoredNumber(storage.getItem(leftPaneWidthKey)),
    rightPaneWidth: shouldResetPaneDimensions ? null : parseStoredNumber(storage.getItem(rightPaneWidthKey)),
    terminalPaneHeight: shouldResetPaneDimensions ? null : parseStoredNumber(storage.getItem(terminalPaneHeightKey)),
  };
}

export function saveUiTheme(storage: Storage, theme: UiTheme) {
  storage.setItem(uiThemeKey, theme);
}

export function savePaneDimensions(
  storage: Storage,
  dimensions: {
    leftPaneWidth: number;
    rightPaneWidth: number;
    terminalPaneHeight: number;
  },
) {
  storage.setItem(leftPaneWidthKey, String(dimensions.leftPaneWidth));
  storage.setItem(rightPaneWidthKey, String(dimensions.rightPaneWidth));
  storage.setItem(terminalPaneHeightKey, String(dimensions.terminalPaneHeight));
}

function parseStoredNumber(value: string | null) {
  if (value === null) {
    return null;
  }

  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}
