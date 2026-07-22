use serde::{Deserialize, Serialize};

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
    pub(super) version: u8,
}

impl Default for GlobalAppConfig {
    fn default() -> Self {
        Self { version: 1 }
    }
}
