use std::{fs, path::Path};

use crate::{zola_links::rewrite_template_asset_cachebust, zola_theme::ZolaThemeResolver};

pub(super) fn project_template_asset_link_targets(root: &Path) -> Result<Vec<String>, String> {
    let mut targets = Vec::new();
    let templates_root = root.join("templates");
    if templates_root.exists() {
        collect_template_asset_link_targets(root, &templates_root, &mut targets)?;
    }

    let resolver = ZolaThemeResolver::for_root(root);
    if let Some(theme) = resolver.active_theme() {
        let theme_templates_root = root.join("themes").join(theme).join("templates");
        if theme_templates_root.exists() {
            collect_template_asset_link_targets(root, &theme_templates_root, &mut targets)?;
        }
    }

    Ok(targets)
}

pub(super) fn rewrite_template_asset_links_source(source: &str, cachebust_assets: bool) -> String {
    rewrite_template_asset_cachebust(source, cachebust_assets)
}

fn collect_template_asset_link_targets(
    root: &Path,
    dir: &Path,
    targets: &mut Vec<String>,
) -> Result<(), String> {
    for entry in
        fs::read_dir(dir).map_err(|e| format!("Nu am putut citi folderul templates: {}", e))?
    {
        let entry =
            entry.map_err(|e| format!("Nu am putut citi o intrare din templates: {}", e))?;
        let path = entry.path();
        if path.is_dir() {
            collect_template_asset_link_targets(root, &path, targets)?;
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("html") {
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .map_err(|e| {
                format!(
                    "Nu am putut relativiza template-ul {}: {}",
                    path.display(),
                    e
                )
            })?
            .to_string_lossy()
            .replace('\\', "/");
        targets.push(relative);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    #[test]
    fn rewrites_active_theme_templates_when_no_local_templates_exist() {
        let root = unique_test_dir("asset-links-theme");
        fs::create_dir_all(root.join("themes/test-theme/templates")).unwrap();
        fs::write(
            root.join("zola.toml"),
            "base_url = \"http://example.test\"\ntheme = \"test-theme\"\n",
        )
        .unwrap();
        fs::write(
            root.join("themes/test-theme/templates/base.html"),
            r#"<link rel="stylesheet" href="/css/site.css">"#,
        )
        .unwrap();

        let targets = project_template_asset_link_targets(&root).unwrap();
        assert_eq!(targets, vec!["themes/test-theme/templates/base.html"]);

        let source =
            fs::read_to_string(root.join("themes/test-theme/templates/base.html")).unwrap();
        let updated = rewrite_template_asset_links_source(&source, true);
        assert!(updated.contains("{{ get_url(path='css/site.css', cachebust=true) }}"));
        cleanup(&root);
    }

    fn cleanup(root: &Path) {
        let _ = fs::remove_dir_all(root);
    }

    fn unique_test_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("panastudio-asset-links-{label}-{nanos}"))
    }
}
