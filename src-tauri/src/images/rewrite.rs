use std::path::{Component, Path};

pub fn rewrite_asset_references(
    content: &str,
    text_file_path: &Path,
    output_dir: &Path,
    replacements: &[(String, String)],
) -> String {
    let mut next = content.to_string();
    let text_dir = text_file_path.parent().unwrap_or(output_dir);

    for (old_rel, new_rel) in replacements {
        let old_abs = format!("/{}", old_rel);
        let new_abs = format!("/{}", new_rel);
        next = next.replace(&old_abs, &new_abs);
        next = next.replace(old_rel, new_rel);

        let old_path = output_dir.join(old_rel);
        let new_path = output_dir.join(new_rel);
        if let (Some(old_from_text), Some(new_from_text)) = (
            relative_path_between(text_dir, &old_path),
            relative_path_between(text_dir, &new_path),
        ) {
            next = next.replace(&old_from_text, &new_from_text);
        }
    }

    next
}

fn relative_path_between(from_dir: &Path, to_path: &Path) -> Option<String> {
    let from = normalized_components(from_dir);
    let to = normalized_components(to_path);
    if from.is_empty() || to.is_empty() {
        return None;
    }

    let mut common = 0usize;
    while common < from.len() && common < to.len() && from[common] == to[common] {
        common += 1;
    }

    let mut parts = Vec::new();
    for _ in common..from.len() {
        parts.push("..".to_string());
    }
    parts.extend(to[common..].iter().cloned());

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

fn normalized_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            Component::RootDir => Some(String::new()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_absolute_and_relative_asset_references() {
        let output = Path::new("/site/public");
        let file = output.join("blog/page/index.html");
        let source = r#"<img src="/imagini/hero.jpg"><img src="../../../imagini/hero.jpg">"#;

        let result = rewrite_asset_references(
            source,
            &file,
            output,
            &[(
                "imagini/hero.jpg".to_string(),
                "imagini/hero.webp".to_string(),
            )],
        );

        assert!(result.contains(r#"src="/imagini/hero.webp""#));
        assert!(result.contains(r#"src="../../../imagini/hero.webp""#));
    }
}
