use serde::{Deserialize, Serialize};

pub const APPLICATION_SETTINGS_SCHEMA_VERSION: u32 = 2;
pub const DEFAULT_BLOCK_PROPERTIES_HEIGHT: u16 = 220;
pub const MIN_BLOCK_PROPERTIES_HEIGHT: u16 = 140;
pub const MAX_BLOCK_PROPERTIES_HEIGHT: u16 = 520;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationTheme {
    Light,
    #[default]
    Dark,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationSettingsSnapshot {
    pub schema_version: u32,
    pub revision: u64,
    pub initialized: bool,
    pub theme: ApplicationTheme,
    pub block_properties_height: u16,
    pub block_properties_collapsed: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationSettingsInput {
    pub expected_revision: u64,
    pub theme: ApplicationTheme,
    #[serde(default = "default_block_properties_height")]
    pub block_properties_height: u16,
    #[serde(default)]
    pub block_properties_collapsed: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAppConfig {
    pub project_path: String,
    #[serde(default)]
    pub cachebust_assets: bool,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAppConfigInput {
    #[serde(default)]
    pub cachebust_assets: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZolaProjectSettings {
    pub config_path: String,
    pub base_url: String,
    pub title: String,
    pub description: String,
    pub default_language: String,
    pub author: String,
    pub compile_sass: bool,
    pub minify_html: bool,
    pub output_dir: String,
    pub generate_sitemap: bool,
    pub generate_robots_txt: bool,
    pub exclude_paginated_pages_in_sitemap: bool,
    pub generate_feeds: bool,
    pub feed_filenames: Vec<String>,
    pub feed_limit: Option<u32>,
    pub render_emoji: bool,
    pub smart_punctuation: bool,
    pub insert_anchor_links: String,
    pub lazy_async_image: bool,
    pub github_alerts: bool,
    pub bottom_footnotes: bool,
    pub external_links_target_blank: bool,
    pub external_links_no_follow: bool,
    pub external_links_no_referrer: bool,
    pub build_search_index: bool,
    pub search_index_format: String,
    pub search_include_title: bool,
    pub search_include_description: bool,
    pub search_include_date: bool,
    pub search_include_path: bool,
    pub search_include_content: bool,
    pub search_truncate_content_length: Option<u32>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct GlobalAppConfig {
    #[serde(default = "default_global_app_config_version")]
    pub(super) version: u8,
    #[serde(default)]
    pub(super) revision: u64,
    #[serde(default)]
    pub(super) theme: Option<ApplicationTheme>,
    #[serde(default)]
    pub(super) block_properties_height: Option<u16>,
    #[serde(default)]
    pub(super) block_properties_collapsed: Option<bool>,
}

impl Default for GlobalAppConfig {
    fn default() -> Self {
        Self {
            version: default_global_app_config_version(),
            revision: 0,
            theme: None,
            block_properties_height: None,
            block_properties_collapsed: None,
        }
    }
}

fn default_global_app_config_version() -> u8 {
    2
}

fn default_block_properties_height() -> u16 {
    DEFAULT_BLOCK_PROPERTIES_HEIGHT
}

#[cfg(test)]
mod tests {
    use super::{ApplicationTheme, GlobalAppConfig};

    #[test]
    fn legacy_global_config_defaults_new_application_settings_fields() {
        let config: GlobalAppConfig =
            serde_json::from_str(r#"{"version":1}"#).expect("legacy config");

        assert_eq!(config.revision, 0);
        assert_eq!(config.theme, None);
        assert_eq!(config.block_properties_height, None);
        assert_eq!(config.block_properties_collapsed, None);
    }

    #[test]
    fn application_theme_uses_stable_snake_case_values() {
        assert_eq!(
            serde_json::to_string(&ApplicationTheme::Light).expect("light theme"),
            r#""light""#,
        );
        assert_eq!(
            serde_json::to_string(&ApplicationTheme::Dark).expect("dark theme"),
            r#""dark""#,
        );
    }
}
