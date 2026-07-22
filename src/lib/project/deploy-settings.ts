import type { ZolaProjectSettings } from "$lib/types";

export const BUNNY_ENV_KEYS = [
  { key: "BUNNY_STORAGE_ZONE", label: "Storage Zone", secret: false },
  { key: "BUNNY_STORAGE_KEY", label: "Storage Key", secret: true },
  { key: "BUNNY_STORAGE_REGION", label: "Region", secret: false },
  { key: "BUNNY_PULL_ZONE_ID", label: "Pull Zone ID", secret: false },
  { key: "BUNNY_CDN_API_KEY", label: "CDN API Key", secret: true },
] as const;

export type ProjectAppConfig = {
  cachebustAssets: boolean;
};

export type ProjectAppConfigDraft = {
  cachebustAssetsDraft: boolean;
};

export type ZolaSettingsTextFields = {
  feedFilenamesText: string;
  feedLimitText: string;
  searchTruncateText: string;
};

export function createDefaultZolaSettings(): ZolaProjectSettings {
  return {
    configPath: "zola.toml",
    baseUrl: "",
    title: "",
    description: "",
    defaultLanguage: "en",
    author: "",
    compileSass: false,
    minifyHtml: false,
    outputDir: "public",
    generateSitemap: true,
    generateRobotsTxt: true,
    excludePaginatedPagesInSitemap: false,
    generateFeeds: false,
    feedFilenames: ["atom.xml"],
    feedLimit: null,
    renderEmoji: false,
    smartPunctuation: false,
    insertAnchorLinks: "none",
    lazyAsyncImage: false,
    githubAlerts: false,
    bottomFootnotes: false,
    externalLinksTargetBlank: false,
    externalLinksNoFollow: false,
    externalLinksNoReferrer: false,
    buildSearchIndex: false,
    searchIndexFormat: "elasticlunr_javascript",
    searchIncludeTitle: true,
    searchIncludeDescription: false,
    searchIncludeDate: false,
    searchIncludePath: false,
    searchIncludeContent: true,
    searchTruncateContentLength: null,
  };
}

export function textFieldsFromZolaSettings(settings: ZolaProjectSettings): ZolaSettingsTextFields {
  return {
    feedFilenamesText: settings.feedFilenames.join(", "),
    feedLimitText: settings.feedLimit?.toString() ?? "",
    searchTruncateText: settings.searchTruncateContentLength?.toString() ?? "",
  };
}

export function zolaSettingsWithTextFields(
  settings: ZolaProjectSettings,
  textFields: ZolaSettingsTextFields,
): ZolaProjectSettings {
  return {
    ...settings,
    feedFilenames: parseList(textFields.feedFilenamesText),
    feedLimit: parseOptionalNumber(textFields.feedLimitText),
    searchTruncateContentLength: parseOptionalNumber(textFields.searchTruncateText),
  };
}

export function appConfigDraftFromConfig(config: ProjectAppConfig): ProjectAppConfigDraft {
  return {
    cachebustAssetsDraft: config.cachebustAssets,
  };
}

export function appConfigFromDraft(draft: ProjectAppConfigDraft): ProjectAppConfig {
  return {
    cachebustAssets: draft.cachebustAssetsDraft,
  };
}

export function bunnyEnvVarsFromDraft(envVars: Record<string, string>): Record<string, string> {
  const bunny: Record<string, string> = {};
  for (const { key } of BUNNY_ENV_KEYS) {
    if (envVars[key] !== undefined) bunny[key] = envVars[key];
  }
  return bunny;
}

function parseList(value: string) {
  return value
    .split(",")
    .map((entry) => entry.trim())
    .filter(Boolean);
}

function parseOptionalNumber(value: string) {
  const trimmed = value.trim();
  if (!trimmed) return null;
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) && parsed >= 0 ? Math.floor(parsed) : null;
}
