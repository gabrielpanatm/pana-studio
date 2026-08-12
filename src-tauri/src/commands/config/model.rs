use serde::{Deserialize, Serialize};

use crate::deploy::DeploySettings;
use crate::system_preferences::{SystemContrast, SystemPreferencesSnapshot};

pub const APPLICATION_SETTINGS_SCHEMA_VERSION: u32 = 3;
pub const APPLICATION_BOOT_PROJECTION_SCHEMA_VERSION: u32 = 1;
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

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ApplicationLanguagePreference {
    #[default]
    System,
    Fixed {
        value: String,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ApplicationThemePreference {
    #[default]
    System,
    Fixed {
        value: ApplicationTheme,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ApplicationAccentPreference {
    #[default]
    System,
    Brand,
    Fixed {
        value: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationPreferenceSelections {
    pub language: ApplicationLanguagePreference,
    pub theme: ApplicationThemePreference,
    pub accent: ApplicationAccentPreference,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationPreferenceResolutionSource {
    Fixed,
    XdgPortal,
    TauriWindow,
    PosixLocale,
    Fallback,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveApplicationPreferences {
    pub locale: String,
    pub direction: String,
    pub theme: ApplicationTheme,
    pub accent: String,
    pub language_source: ApplicationPreferenceResolutionSource,
    pub theme_source: ApplicationPreferenceResolutionSource,
    pub accent_source: ApplicationPreferenceResolutionSource,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationBootProjection {
    pub schema_version: u32,
    pub authority: &'static str,
    pub settings_schema_version: u32,
    pub settings_revision: u64,
    pub system_generation: u64,
    pub locale: String,
    pub direction: String,
    pub theme: ApplicationTheme,
    pub accent: String,
    pub contrast: Option<SystemContrast>,
    pub reduced_motion: Option<bool>,
    pub loading_label: String,
    pub loading_subtitle: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationSettingsSnapshot {
    pub schema_version: u32,
    pub revision: u64,
    pub brand_accent: String,
    pub preferences: ApplicationPreferenceSelections,
    pub effective: EffectiveApplicationPreferences,
    pub system: SystemPreferencesSnapshot,
    pub boot: ApplicationBootProjection,
    pub block_properties_height: u16,
    pub block_properties_collapsed: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationSettingsPatch {
    #[serde(default)]
    pub language: Option<ApplicationLanguagePreference>,
    #[serde(default)]
    pub theme: Option<ApplicationThemePreference>,
    #[serde(default)]
    pub accent: Option<ApplicationAccentPreference>,
    #[serde(default)]
    pub block_properties_height: Option<u16>,
    #[serde(default)]
    pub block_properties_collapsed: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationSettingsPatchInput {
    pub expected_revision: u64,
    #[serde(default)]
    pub patch: ApplicationSettingsPatch,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAppConfig {
    pub project_path: String,
    #[serde(default)]
    pub cachebust_assets: bool,
    #[serde(default)]
    pub deploy: DeploySettings,
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
    #[serde(default, rename = "theme", skip_serializing_if = "Option::is_none")]
    pub(super) legacy_theme: Option<ApplicationTheme>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) language_preference: Option<ApplicationLanguagePreference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) theme_preference: Option<ApplicationThemePreference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) accent_preference: Option<ApplicationAccentPreference>,
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
            legacy_theme: None,
            language_preference: None,
            theme_preference: None,
            accent_preference: None,
            block_properties_height: None,
            block_properties_collapsed: None,
        }
    }
}

fn default_global_app_config_version() -> u8 {
    3
}

#[cfg(test)]
mod tests {
    use super::{
        ApplicationLanguagePreference, ApplicationTheme, ApplicationThemePreference,
        GlobalAppConfig,
    };

    #[test]
    fn legacy_global_config_defaults_new_application_settings_fields() {
        let config: GlobalAppConfig =
            serde_json::from_str(r#"{"version":1}"#).expect("legacy config");

        assert_eq!(config.revision, 0);
        assert_eq!(config.legacy_theme, None);
        assert_eq!(config.language_preference, None);
        assert_eq!(config.theme_preference, None);
        assert_eq!(config.accent_preference, None);
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

    #[test]
    fn system_and_fixed_preferences_have_stable_tagged_shapes() {
        assert_eq!(
            serde_json::to_string(&ApplicationLanguagePreference::System).expect("system language"),
            r#"{"mode":"system"}"#,
        );
        assert_eq!(
            serde_json::to_string(&ApplicationThemePreference::Fixed {
                value: ApplicationTheme::Dark,
            })
            .expect("fixed theme"),
            r#"{"mode":"fixed","value":"dark"}"#,
        );
    }

    #[test]
    fn legacy_theme_is_still_deserialized_for_migration() {
        let config: GlobalAppConfig =
            serde_json::from_str(r#"{"version":2,"theme":"light"}"#).expect("legacy theme");
        assert_eq!(config.legacy_theme, Some(ApplicationTheme::Light));
        assert_eq!(config.theme_preference, None);
    }
}
