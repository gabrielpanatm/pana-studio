use crate::{
    kernel::file_buffer_store::FileBufferStore, zola_links::rewrite_template_asset_cachebust,
    zola_theme::active_theme_from_source,
};

pub(super) fn project_template_asset_link_targets(store: &FileBufferStore) -> Vec<String> {
    let active_theme = store
        .text_for("zola.toml")
        .or_else(|| store.text_for("config.toml"))
        .as_deref()
        .and_then(active_theme_from_source);
    store
        .files
        .keys()
        .filter(|path| template_belongs_to_active_project(path, active_theme.as_deref()))
        .cloned()
        .collect()
}

pub(super) fn rewrite_template_asset_links_source(source: &str, cachebust_assets: bool) -> String {
    rewrite_template_asset_cachebust(source, cachebust_assets)
}

fn template_belongs_to_active_project(path: &str, active_theme: Option<&str>) -> bool {
    if !path.ends_with(".html") {
        return false;
    }
    if path.starts_with("templates/") {
        return true;
    }
    active_theme.is_some_and(|theme| path.starts_with(&format!("themes/{theme}/templates/")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_scope_includes_local_and_only_active_theme_templates() {
        assert!(template_belongs_to_active_project(
            "templates/index.html",
            None
        ));
        assert!(template_belongs_to_active_project(
            "themes/test-theme/templates/base.html",
            Some("test-theme")
        ));
        assert!(!template_belongs_to_active_project(
            "themes/other/templates/base.html",
            Some("test-theme")
        ));
        assert!(!template_belongs_to_active_project(
            "templates/readme.txt",
            None
        ));

        let source = r#"<link rel="stylesheet" href="/css/site.css">"#;
        let updated = rewrite_template_asset_links_source(source, true);
        assert!(updated.contains("{{ get_url(path='css/site.css', cachebust=true) }}"));
    }
}
