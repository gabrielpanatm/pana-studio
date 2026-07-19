use std::path::{Path, PathBuf};

use tauri::{AppHandle, Runtime};

use crate::app_home::app_home_snapshot;

pub(crate) fn preview_project_dir<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &Path,
) -> Result<PathBuf, String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    project_root.hash(&mut hasher);
    let hash = hasher.finish();
    Ok(PathBuf::from(app_home_snapshot(app)?.preview_cache_dir).join(format!("project-{hash:x}")))
}
