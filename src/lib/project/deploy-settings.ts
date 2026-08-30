import type {
  ProjectSettingsSnapshot,
  ZolaProjectSettings,
} from "$lib/project/lifecycle-contract";

export type ProjectSettingsDraft = {
  cachebustAssetsDraft: boolean;
};

export type ZolaSettingsTextFields = {
  feedFilenamesText: string;
  feedLimitText: string;
  skipContentTemplatingText: string;
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
    skipContentTemplating: [],
    highlightDataAttrPosition: null,
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
    skipContentTemplatingText: settings.skipContentTemplating.join("\n"),
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
    skipContentTemplating: parseLines(textFields.skipContentTemplatingText),
    searchTruncateContentLength: parseOptionalNumber(textFields.searchTruncateText),
  };
}

export function projectSettingsDraftFromSnapshot(
  settings: ProjectSettingsSnapshot,
): ProjectSettingsDraft {
  return {
    cachebustAssetsDraft: settings.cachebustAssets,
  };
}

export function projectSettingsFromDraft(
  draft: ProjectSettingsDraft,
  expectedWorkspaceRevision: number,
): { expectedWorkspaceRevision: number; cachebustAssets: boolean } {
  return {
    expectedWorkspaceRevision,
    cachebustAssets: draft.cachebustAssetsDraft,
  };
}

function parseList(value: string) {
  return value
    .split(",")
    .map((entry) => entry.trim())
    .filter(Boolean);
}

function parseLines(value: string) {
  return value
    .split(/\r?\n/)
    .map((entry) => entry.trim())
    .filter(Boolean);
}

function parseOptionalNumber(value: string) {
  const trimmed = value.trim();
  if (!trimmed) return null;
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) && parsed >= 0 ? Math.floor(parsed) : null;
}
