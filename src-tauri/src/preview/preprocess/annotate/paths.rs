use std::path::Path;

pub use crate::zola_theme::{is_template_relative_path, read_active_theme};

pub(super) fn zola_relative_path(zola_root: &Path, path: &Path) -> String {
    path.strip_prefix(zola_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_string()
}
