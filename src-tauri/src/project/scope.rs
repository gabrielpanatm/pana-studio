/// Directories owned by build tools or Pană Studio itself. They are derived
/// state, not editable project sources, so every project traversal that feeds
/// an authority surface must ignore them consistently.
const DERIVED_OR_INTERNAL_DIRS: &[&str] = &[
    ".git",
    ".svelte-kit",
    "build",
    "dist",
    "node_modules",
    "target",
    "export",
    ".panastudio",
    ".panastudio_preview",
];

pub(crate) fn is_derived_or_internal_dir(name: &str) -> bool {
    DERIVED_OR_INTERNAL_DIRS.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::is_derived_or_internal_dir;

    #[test]
    fn generated_export_is_outside_the_project_source_authority() {
        assert!(is_derived_or_internal_dir("export"));
        assert!(!is_derived_or_internal_dir("sursa"));
    }
}
