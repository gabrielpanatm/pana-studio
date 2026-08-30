export type SourceLanguage = "html" | "css" | "scss" | "js" | "markdown" | "plain";

export type CenterView = "preview" | "code" | "kernel";

export type ApplicationSurface = "workbench" | "settings";

export type ApplicationTheme = "light" | "dark";

type ApplicationLanguagePreference =
  | { mode: "system" }
  | { mode: "fixed"; value: string };

export type ApplicationThemePreference =
  | { mode: "system" }
  | { mode: "fixed"; value: ApplicationTheme };

type ApplicationAccentPreference =
  | { mode: "system" }
  | { mode: "brand" }
  | { mode: "fixed"; value: string };

type ApplicationPreferenceSelections = {
  language: ApplicationLanguagePreference;
  theme: ApplicationThemePreference;
  accent: ApplicationAccentPreference;
};

type SystemPreferenceSource =
  | "xdg_portal"
  | "tauri_window"
  | "posix_locale"
  | "fallback"
  | "unavailable";

type ApplicationPreferenceResolutionSource =
  | "fixed"
  | "xdg_portal"
  | "tauri_window"
  | "posix_locale"
  | "fallback";

type SystemAccentColor = {
  red: number;
  green: number;
  blue: number;
};

export type SystemPreferencesSnapshot = {
  schemaVersion: 1;
  generation: number;
  localeCandidates: string[];
  localeSource: SystemPreferenceSource;
  colorScheme: ApplicationTheme | null;
  colorSchemeSource: SystemPreferenceSource;
  accent: SystemAccentColor | null;
  accentSource: SystemPreferenceSource;
  contrast: "normal" | "high" | null;
  contrastSource: SystemPreferenceSource;
  reducedMotion: boolean | null;
  reducedMotionSource: SystemPreferenceSource;
  portalAvailable: boolean;
};

type EffectiveApplicationPreferences = {
  locale: string;
  direction: "ltr" | "rtl";
  theme: ApplicationTheme;
  accent: string;
  languageSource: ApplicationPreferenceResolutionSource;
  themeSource: ApplicationPreferenceResolutionSource;
  accentSource: ApplicationPreferenceResolutionSource;
};

export type ApplicationBootProjection = {
  schemaVersion: 1;
  authority: "rust_application_settings";
  settingsSchemaVersion: 3;
  settingsRevision: number;
  systemGeneration: number;
  locale: string;
  direction: "ltr" | "rtl";
  theme: ApplicationTheme;
  accent: string;
  contrast: "normal" | "high" | null;
  reducedMotion: boolean | null;
  loadingLabel: string;
  loadingSubtitle: string;
};

export type ApplicationSettingsSnapshot = {
  schemaVersion: 3;
  revision: number;
  brandAccent: string;
  preferences: ApplicationPreferenceSelections;
  effective: EffectiveApplicationPreferences;
  system: SystemPreferencesSnapshot;
  boot: ApplicationBootProjection;
  blockPropertiesHeight: number;
  blockPropertiesCollapsed: boolean;
};

export type ApplicationSettingsPatch = {
  language?: ApplicationLanguagePreference;
  theme?: ApplicationThemePreference;
  accent?: ApplicationAccentPreference;
  blockPropertiesHeight?: number;
  blockPropertiesCollapsed?: boolean;
};

export type AppHomeSnapshot = {
  schemaVersion: 3;
  identifier: string;
  embeddedZolaVersion: string;
  configDir: string;
  dataDir: string;
  cacheDir: string;
  logDir: string;
  tempDir: string;
  mcpDir: string;
  sessionsDir: string;
  kernelDir: string;
  writeAuthorityWalDir: string;
  scratchDir: string;
  previewCacheDir: string;
  appLogsDir: string;
};

type StorageAreaSnapshot = {
  path: string;
  bytes: number;
  entries: number;
};

type StorageCacheSnapshot = {
  webkit: StorageAreaSnapshot;
  preview: StorageAreaSnapshot;
  totalBytes: number;
  reclaimableBytes: number;
  protectedPreviewBytes: number;
  webkitCleanupSupported: boolean;
};

type StorageLogsSnapshot = {
  area: StorageAreaSnapshot;
  activeBytes: number;
  archiveCount: number;
};

export type StorageSessionSnapshot = {
  id: string;
  projectName: string;
  projectRoot: string;
  bytes: number;
  entries: number;
  lastSeenAtMs: number;
  projectExists: boolean;
  hasRecovery: boolean;
  recoverySignals: string[];
  manifestStatus: string;
  active: boolean;
  deletable: boolean;
  defaultSelected: boolean;
};

type StorageSessionsSnapshot = {
  path: string;
  revision: string;
  totalBytes: number;
  reclaimableBytes: number;
  count: number;
  orphanCount: number;
  recoveryCount: number;
  activeCount: number;
  items: StorageSessionSnapshot[];
};

export type ApplicationStorageSnapshot = {
  schemaVersion: 1;
  scannedAtMs: number;
  totalBytes: number;
  reclaimableBytes: number;
  cache: StorageCacheSnapshot;
  logs: StorageLogsSnapshot;
  sessions: StorageSessionsSnapshot;
};

export type DeleteStorageSessionsRequest = {
  expectedRevision: string;
  sessionIds: string[];
  confirmedRecoverySessionIds: string[];
};

export type StorageCleanupReceipt = {
  schemaVersion: 1;
  operation: "cache" | "logs" | "sessions";
  removedItems: number;
  bytesBefore: number;
  bytesAfter: number;
  freedBytes: number;
  protectedBytes: number;
  failures: string[];
  snapshot: ApplicationStorageSnapshot;
};

export type ProjectPaneTab = "layers" | "files";

export type InspectorTab = "html" | "css" | "js";
