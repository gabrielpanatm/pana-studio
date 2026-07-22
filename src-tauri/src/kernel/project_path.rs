use std::path::{Component, Path};

/// Normalize and validate a path relative to the active project root.
///
/// This is a project boundary primitive, not an editing-command concern. All
/// ProjectWorkspace readers, planners and disk-boundary writers use the same
/// validation so an absolute path or a parent traversal can never enter a
/// workspace mutation.
pub fn normalize_project_relative_path(input: &str) -> Result<String, String> {
    let mut normalized = input.trim().replace('\\', "/");
    while let Some(next) = normalized.strip_prefix("./") {
        normalized = next.to_string();
    }

    if normalized.is_empty() {
        return Err("ProjectWorkspace a blocat un path relativ gol.".to_string());
    }
    if normalized.contains('\0') {
        return Err("ProjectWorkspace a blocat un path relativ invalid.".to_string());
    }

    for component in Path::new(&normalized).components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return Err(format!(
                "ProjectWorkspace a blocat path-ul {normalized}: nu este relativ sigur."
            ));
        }
    }

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::normalize_project_relative_path;

    #[test]
    fn normalizes_project_relative_paths() {
        assert_eq!(
            normalize_project_relative_path(" ./templates\\index.html ").unwrap(),
            "templates/index.html"
        );
    }

    #[test]
    fn rejects_parent_traversal() {
        let error = normalize_project_relative_path("../secret.txt").unwrap_err();
        assert!(error.contains("nu este relativ sigur"));
    }
}
