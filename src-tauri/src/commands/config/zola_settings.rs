use std::path::{Path, PathBuf};

use crate::commands::config::{
    model::{HighlightDataAttrPosition, ZolaProjectSettings},
    toml_edit::{
        remove_toml_key, toml_array, toml_bool, toml_paginated_sitemap, toml_quote,
        toml_section_exists, toml_string, toml_string_array, toml_u32, upsert_or_remove_u32,
        upsert_toml_value,
    },
};

pub(super) fn write_zola_settings_to_source(
    source: &str,
    settings: &ZolaProjectSettings,
) -> Result<String, String> {
    write_zola_settings_source(source, settings)
}

pub(super) fn parse_zola_project_settings_source(
    source: &str,
    config_path: &str,
) -> ZolaProjectSettings {
    parse_zola_project_settings(source, config_path)
}

pub(super) fn zola_config_relative_path(root: &Path, prefer_create: bool) -> String {
    let path = zola_config_path(root, prefer_create);
    path.strip_prefix(root)
        .unwrap_or(&path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn zola_config_path(root: &Path, prefer_create: bool) -> PathBuf {
    let zola = root.join("zola.toml");
    if zola.exists() || prefer_create && !root.join("config.toml").exists() {
        return zola;
    }
    root.join("config.toml")
}

fn parse_zola_project_settings(source: &str, config_path: &str) -> ZolaProjectSettings {
    ZolaProjectSettings {
        config_path: config_path.to_string(),
        base_url: toml_string(source, None, "base_url").unwrap_or_default(),
        title: toml_string(source, None, "title").unwrap_or_default(),
        description: toml_string(source, None, "description").unwrap_or_default(),
        default_language: toml_string(source, None, "default_language")
            .unwrap_or_else(|| "en".to_string()),
        author: toml_string(source, None, "author").unwrap_or_default(),
        compile_sass: toml_bool(source, None, "compile_sass").unwrap_or(false),
        minify_html: toml_bool(source, None, "minify_html").unwrap_or(false),
        output_dir: toml_string(source, None, "output_dir").unwrap_or_else(|| "public".to_string()),
        generate_sitemap: toml_bool(source, None, "generate_sitemap").unwrap_or(true),
        generate_robots_txt: toml_bool(source, None, "generate_robots_txt").unwrap_or(true),
        exclude_paginated_pages_in_sitemap: toml_paginated_sitemap(source),
        generate_feeds: toml_bool(source, None, "generate_feeds").unwrap_or(false),
        feed_filenames: {
            let values = toml_string_array(source, None, "feed_filenames");
            if values.is_empty() {
                vec!["atom.xml".to_string()]
            } else {
                values
            }
        },
        feed_limit: toml_u32(source, None, "feed_limit"),
        skip_content_templating: toml_string_array(source, None, "skip_content_templating"),
        highlight_data_attr_position: toml_section_exists(source, "markdown.highlighting").then(
            || {
                toml_string(source, Some("markdown.highlighting"), "data_attr_position")
                    .as_deref()
                    .and_then(HighlightDataAttrPosition::from_zola_str)
                    .unwrap_or(HighlightDataAttrPosition::Code)
            },
        ),
        render_emoji: toml_bool(source, Some("markdown"), "render_emoji").unwrap_or(false),
        smart_punctuation: toml_bool(source, Some("markdown"), "smart_punctuation")
            .unwrap_or(false),
        insert_anchor_links: toml_string(source, Some("markdown"), "insert_anchor_links")
            .unwrap_or_else(|| "none".to_string()),
        lazy_async_image: toml_bool(source, Some("markdown"), "lazy_async_image").unwrap_or(false),
        github_alerts: toml_bool(source, Some("markdown"), "github_alerts").unwrap_or(false),
        bottom_footnotes: toml_bool(source, Some("markdown"), "bottom_footnotes").unwrap_or(false),
        external_links_target_blank: toml_bool(
            source,
            Some("markdown"),
            "external_links_target_blank",
        )
        .unwrap_or(false),
        external_links_no_follow: toml_bool(source, Some("markdown"), "external_links_no_follow")
            .unwrap_or(false),
        external_links_no_referrer: toml_bool(
            source,
            Some("markdown"),
            "external_links_no_referrer",
        )
        .unwrap_or(false),
        build_search_index: toml_bool(source, None, "build_search_index")
            .or_else(|| toml_bool(source, Some("search"), "build_search_index"))
            .unwrap_or(false),
        search_index_format: toml_string(source, Some("search"), "index_format")
            .unwrap_or_else(|| "elasticlunr_javascript".to_string()),
        search_include_title: toml_bool(source, Some("search"), "include_title").unwrap_or(true),
        search_include_description: toml_bool(source, Some("search"), "include_description")
            .unwrap_or(false),
        search_include_date: toml_bool(source, Some("search"), "include_date").unwrap_or(false),
        search_include_path: toml_bool(source, Some("search"), "include_path").unwrap_or(false),
        search_include_content: toml_bool(source, Some("search"), "include_content")
            .unwrap_or(true),
        search_truncate_content_length: toml_u32(source, Some("search"), "truncate_content_length"),
    }
}

fn write_zola_settings_source(
    source: &str,
    settings: &ZolaProjectSettings,
) -> Result<String, String> {
    let mut next = source.to_string();
    next = upsert_toml_value(&next, None, "base_url", toml_quote(&settings.base_url));
    next = upsert_toml_value(&next, None, "title", toml_quote(&settings.title));
    next = upsert_toml_value(
        &next,
        None,
        "description",
        toml_quote(&settings.description),
    );
    next = upsert_toml_value(
        &next,
        None,
        "default_language",
        toml_quote(&settings.default_language),
    );
    next = upsert_toml_value(&next, None, "author", toml_quote(&settings.author));
    next = upsert_toml_value(
        &next,
        None,
        "compile_sass",
        settings.compile_sass.to_string(),
    );
    next = upsert_toml_value(&next, None, "minify_html", settings.minify_html.to_string());
    next = upsert_toml_value(&next, None, "output_dir", toml_quote(&settings.output_dir));
    next = upsert_toml_value(
        &next,
        None,
        "generate_sitemap",
        settings.generate_sitemap.to_string(),
    );
    next = upsert_toml_value(
        &next,
        None,
        "generate_robots_txt",
        settings.generate_robots_txt.to_string(),
    );
    next = upsert_toml_value(
        &next,
        None,
        "exclude_paginated_pages_in_sitemap",
        toml_quote(if settings.exclude_paginated_pages_in_sitemap {
            "all"
        } else {
            "none"
        }),
    );
    next = upsert_toml_value(
        &next,
        None,
        "generate_feeds",
        settings.generate_feeds.to_string(),
    );
    next = upsert_toml_value(
        &next,
        None,
        "feed_filenames",
        toml_array(&settings.feed_filenames),
    );
    next = upsert_or_remove_u32(&next, None, "feed_limit", settings.feed_limit);
    next = if settings.skip_content_templating.is_empty() {
        remove_toml_key(&next, None, "skip_content_templating")
    } else {
        upsert_toml_value(
            &next,
            None,
            "skip_content_templating",
            toml_array(&settings.skip_content_templating),
        )
    };
    next = match settings.highlight_data_attr_position {
        Some(position) => {
            if !toml_section_exists(source, "markdown.highlighting") {
                return Err(
                    "data_attr_position poate fi configurat numai după activarea secțiunii [markdown.highlighting]."
                        .to_string(),
                );
            }
            upsert_toml_value(
                &next,
                Some("markdown.highlighting"),
                "data_attr_position",
                toml_quote(position.as_zola_str()),
            )
        }
        None => remove_toml_key(&next, Some("markdown.highlighting"), "data_attr_position"),
    };
    next = upsert_toml_value(
        &next,
        Some("markdown"),
        "render_emoji",
        settings.render_emoji.to_string(),
    );
    next = upsert_toml_value(
        &next,
        Some("markdown"),
        "smart_punctuation",
        settings.smart_punctuation.to_string(),
    );
    next = upsert_toml_value(
        &next,
        Some("markdown"),
        "insert_anchor_links",
        toml_quote(&settings.insert_anchor_links),
    );
    next = upsert_toml_value(
        &next,
        Some("markdown"),
        "lazy_async_image",
        settings.lazy_async_image.to_string(),
    );
    next = upsert_toml_value(
        &next,
        Some("markdown"),
        "github_alerts",
        settings.github_alerts.to_string(),
    );
    next = upsert_toml_value(
        &next,
        Some("markdown"),
        "bottom_footnotes",
        settings.bottom_footnotes.to_string(),
    );
    next = upsert_toml_value(
        &next,
        Some("markdown"),
        "external_links_target_blank",
        settings.external_links_target_blank.to_string(),
    );
    next = upsert_toml_value(
        &next,
        Some("markdown"),
        "external_links_no_follow",
        settings.external_links_no_follow.to_string(),
    );
    next = upsert_toml_value(
        &next,
        Some("markdown"),
        "external_links_no_referrer",
        settings.external_links_no_referrer.to_string(),
    );
    next = upsert_toml_value(
        &next,
        None,
        "build_search_index",
        settings.build_search_index.to_string(),
    );
    next = remove_toml_key(&next, Some("search"), "build_search_index");
    next = upsert_toml_value(
        &next,
        Some("search"),
        "index_format",
        toml_quote(&settings.search_index_format),
    );
    next = upsert_toml_value(
        &next,
        Some("search"),
        "include_title",
        settings.search_include_title.to_string(),
    );
    next = upsert_toml_value(
        &next,
        Some("search"),
        "include_description",
        settings.search_include_description.to_string(),
    );
    next = upsert_toml_value(
        &next,
        Some("search"),
        "include_date",
        settings.search_include_date.to_string(),
    );
    next = upsert_toml_value(
        &next,
        Some("search"),
        "include_path",
        settings.search_include_path.to_string(),
    );
    next = upsert_toml_value(
        &next,
        Some("search"),
        "include_content",
        settings.search_include_content.to_string(),
    );
    next = upsert_or_remove_u32(
        &next,
        Some("search"),
        "truncate_content_length",
        settings.search_truncate_content_length,
    );
    zola_config::Config::parse(&next)
        .map_err(|error| format!("Configurația Zola rezultată este invalidă: {error:#}"))?;
    Ok(next)
}

#[cfg(test)]
mod tests {
    use crate::commands::config::toml_edit::toml_raw_value;

    use super::*;

    fn sample_settings() -> ZolaProjectSettings {
        ZolaProjectSettings {
            config_path: "zola.toml".to_string(),
            base_url: "https://example.com".to_string(),
            title: "Example".to_string(),
            description: "Site description".to_string(),
            default_language: "ro".to_string(),
            author: "Autor".to_string(),
            compile_sass: true,
            minify_html: false,
            output_dir: "public".to_string(),
            generate_sitemap: true,
            generate_robots_txt: true,
            exclude_paginated_pages_in_sitemap: true,
            generate_feeds: true,
            feed_filenames: vec!["atom.xml".to_string(), "rss.xml".to_string()],
            feed_limit: Some(10),
            skip_content_templating: vec!["documentatie/**".to_string()],
            highlight_data_attr_position: Some(HighlightDataAttrPosition::Pre),
            render_emoji: true,
            smart_punctuation: true,
            insert_anchor_links: "left".to_string(),
            lazy_async_image: true,
            github_alerts: true,
            bottom_footnotes: true,
            external_links_target_blank: true,
            external_links_no_follow: false,
            external_links_no_referrer: true,
            build_search_index: true,
            search_index_format: "fuse_json".to_string(),
            search_include_title: true,
            search_include_description: true,
            search_include_date: false,
            search_include_path: true,
            search_include_content: true,
            search_truncate_content_length: Some(120),
        }
    }

    #[test]
    fn zola_settings_defaults_match_zola_config_defaults() {
        let settings = parse_zola_project_settings("", "zola.toml");

        assert!(!settings.compile_sass);
        assert!(settings.generate_sitemap);
        assert!(settings.generate_robots_txt);
        assert_eq!(settings.feed_filenames, vec!["atom.xml"]);
        assert!(!settings.exclude_paginated_pages_in_sitemap);
        assert!(!settings.build_search_index);
    }

    #[test]
    fn zola_settings_parse_paginated_sitemap_values() {
        let settings = parse_zola_project_settings(
            r#"exclude_paginated_pages_in_sitemap = "all""#,
            "zola.toml",
        );
        assert!(settings.exclude_paginated_pages_in_sitemap);

        let settings = parse_zola_project_settings(
            r#"exclude_paginated_pages_in_sitemap = "none""#,
            "zola.toml",
        );
        assert!(!settings.exclude_paginated_pages_in_sitemap);

        let settings = parse_zola_project_settings(
            r#"exclude_paginated_pages_in_sitemap = true"#,
            "zola.toml",
        );
        assert!(settings.exclude_paginated_pages_in_sitemap);
    }

    #[test]
    fn zola_settings_write_search_index_in_root_section() {
        let mut settings = sample_settings();
        settings.highlight_data_attr_position = None;
        let source = r#"base_url = "https://old.test"

[search]
build_search_index = false
include_title = false
"#;
        let updated = write_zola_settings_to_source(source, &settings).unwrap();

        assert_eq!(
            toml_raw_value(&updated, None, "build_search_index"),
            Some("true".to_string())
        );
        assert_eq!(
            toml_raw_value(&updated, Some("search"), "build_search_index"),
            None
        );
        assert_eq!(
            toml_raw_value(&updated, Some("search"), "index_format"),
            Some("\"fuse_json\"".to_string())
        );
    }

    #[test]
    fn zola_settings_write_paginated_sitemap_as_zola_string() {
        let mut settings = sample_settings();
        settings.exclude_paginated_pages_in_sitemap = true;
        settings.highlight_data_attr_position = None;
        let updated = write_zola_settings_to_source("", &settings).unwrap();
        assert_eq!(
            toml_raw_value(&updated, None, "exclude_paginated_pages_in_sitemap"),
            Some("\"all\"".to_string())
        );

        settings.exclude_paginated_pages_in_sitemap = false;
        let updated = write_zola_settings_to_source("", &settings).unwrap();
        assert_eq!(
            toml_raw_value(&updated, None, "exclude_paginated_pages_in_sitemap"),
            Some("\"none\"".to_string())
        );
    }

    #[test]
    fn zola_023_editor_settings_round_trip_without_touching_unrelated_toml() {
        let source = r#"# comentariu păstrat
base_url = "https://old.test"
skip_content_templating = ["vechi/**"]

[markdown.highlighting]
style = "class"
theme = "github-dark"
data_attr_position = "code"

[extra]
custom = "păstrat"
"#;
        let mut settings = sample_settings();
        settings.skip_content_templating =
            vec!["documentatie/**".to_string(), "literal/*.md".to_string()];
        settings.highlight_data_attr_position = Some(HighlightDataAttrPosition::Both);

        let updated = write_zola_settings_to_source(source, &settings).unwrap();
        let parsed = parse_zola_project_settings(&updated, "zola.toml");

        assert_eq!(
            parsed.skip_content_templating,
            settings.skip_content_templating
        );
        assert_eq!(
            parsed.highlight_data_attr_position,
            Some(HighlightDataAttrPosition::Both)
        );
        assert!(updated.contains("# comentariu păstrat"));
        assert!(updated.contains("custom = \"păstrat\""));
        zola_config::Config::parse(&updated).expect("configurație acceptată de Zola");
    }

    #[test]
    fn highlighting_position_requires_an_existing_valid_highlighting_section() {
        let error = write_zola_settings_to_source(
            "base_url = \"https://example.test\"\n",
            &sample_settings(),
        )
        .expect_err("secțiunea highlighting nu trebuie creată incomplet");

        assert!(error.contains("[markdown.highlighting]"));
    }

    #[test]
    fn invalid_skip_content_glob_is_rejected_by_zola_config() {
        let mut settings = sample_settings();
        settings.highlight_data_attr_position = None;
        settings.skip_content_templating = vec!["[invalid".to_string()];

        let error =
            write_zola_settings_to_source("base_url = \"https://example.test\"\n", &settings)
                .expect_err("globul invalid trebuie respins");

        assert!(error.contains("skip_content_templating"));
    }
}
