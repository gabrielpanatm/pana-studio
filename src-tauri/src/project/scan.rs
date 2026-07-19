use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use crate::kernel::project_workspace::WorkspaceProjectionLease;
use crate::project::{
    model::{ProjectFile, ProjectFileKind, ProjectFileRole, ProjectScan},
    strip_zola_root_prefix, zola_project_root,
};
use crate::zola_theme::{
    active_theme_from_source, is_template_relative_path, read_active_theme,
    zola_path_without_theme_root,
};

use super::scope::is_derived_or_internal_dir;

pub(crate) const MAX_SCAN_FILES: usize = 500;

pub fn scan_project_root(root: &Path) -> Result<ProjectScan, String> {
    if !root.exists() {
        return Err(format!("Folderul nu exista: {}", root.to_string_lossy()));
    }

    if !root.is_dir() {
        return Err(format!(
            "Path-ul nu este folder: {}",
            root.to_string_lossy()
        ));
    }

    let root = root
        .canonicalize()
        .map_err(|error| format!("Nu am putut rezolva folderul: {}", error))?;
    let zola_mode = is_zola_project(&root);
    let zola_root = zola_project_root(&root);
    let active_theme = zola_mode.then(|| read_active_theme(&zola_root)).flatten();
    let mut files = Vec::new();

    collect_project_files(&root, &root, &mut files, zola_mode)?;
    files.sort_by(compare_project_files);

    let is_empty = fs::read_dir(&root)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false);

    Ok(ProjectScan {
        root: root.to_string_lossy().to_string(),
        preview_base_url: None,
        preview_warning: None,
        active_theme,
        files,
        is_zola: zola_mode,
        is_empty,
        kernel_session_id: None,
        accepted_disk_manifest: None,
        accepted_disk_generation: None,
    })
}

/// Builds the Files-panel projection from exactly one immutable
/// ProjectWorkspace revision. Accepted disk membership supplies non-text
/// resources; the complete workspace text namespace replaces disk text
/// membership. Live disk enumeration is deliberately forbidden here.
pub fn scan_project_workspace_projection(
    projection: &WorkspaceProjectionLease,
) -> Result<ProjectScan, String> {
    projection
        .accepted_disk
        .require_identity(&projection.runtime_session_id, &projection.project_root)?;
    projection.accepted_disk.require_complete()?;
    let root = PathBuf::from(&projection.project_root);
    let mut paths = projection
        .accepted_disk
        .manifest
        .files
        .iter()
        .map(|entry| entry.relative_path.clone())
        .collect::<BTreeSet<_>>();
    for deleted in &projection.deleted_sources {
        paths.remove(deleted);
    }
    paths.extend(projection.source_texts.keys().cloned());
    paths.extend(projection.resource_bytes.keys().cloned());

    let config_source = ["sursa/zola.toml", "sursa/config.toml"]
        .iter()
        .find_map(|path| projection.source_texts.get(*path));
    let is_zola = config_source.is_some();
    let active_theme = config_source.and_then(|source| active_theme_from_source(source));
    let mut files = Vec::new();
    let mut directories = BTreeSet::new();

    for relative_path in paths {
        if files.len() >= MAX_SCAN_FILES {
            break;
        }
        let path = root.join(&relative_path);
        let Some(kind) = project_file_kind(&path) else {
            continue;
        };
        let Some(role) = project_file_role_for_path(&relative_path, &kind, is_zola) else {
            continue;
        };
        let Some(name) = Path::new(&relative_path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
        else {
            continue;
        };
        let preview_path = project_preview_path(&relative_path, &kind, &role, is_zola);
        files.push(ProjectFile {
            name,
            relative_path: relative_path.clone(),
            absolute_path: path.to_string_lossy().into_owned(),
            role,
            kind,
            preview_path,
        });

        let mut parent = Path::new(&relative_path).parent();
        while let Some(directory) = parent {
            let relative_directory = directory.to_string_lossy().replace('\\', "/");
            if relative_directory.is_empty() {
                break;
            }
            directories.insert(relative_directory);
            parent = directory.parent();
        }
    }

    for relative_path in directories {
        if files.len() >= MAX_SCAN_FILES {
            break;
        }
        let Some(name) = Path::new(&relative_path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
        else {
            continue;
        };
        files.push(ProjectFile {
            name,
            absolute_path: root.join(&relative_path).to_string_lossy().into_owned(),
            relative_path,
            role: ProjectFileRole::Asset,
            kind: ProjectFileKind::Dir,
            preview_path: None,
        });
    }
    files.sort_by(compare_project_files);
    let is_empty = files.is_empty();

    Ok(ProjectScan {
        root: projection.project_root.clone(),
        preview_base_url: None,
        preview_warning: None,
        active_theme,
        files,
        is_zola,
        is_empty,
        kernel_session_id: Some(projection.runtime_session_id.clone()),
        accepted_disk_manifest: Some(projection.accepted_disk.manifest.clone()),
        accepted_disk_generation: Some(projection.accepted_disk.generation),
    })
}

pub fn is_zola_project(root: &Path) -> bool {
    let zola_root = zola_project_root(root);
    is_real_directory(&zola_root)
        && has_zola_config(&zola_root)
        && is_real_directory(&zola_root.join("content"))
}

fn collect_project_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<ProjectFile>,
    zola_mode: bool,
) -> Result<(), String> {
    if files.len() >= MAX_SCAN_FILES {
        return Ok(());
    }

    let entries = fs::read_dir(current).map_err(|error| {
        format!(
            "Nu am putut citi folderul {}: {}",
            current.to_string_lossy(),
            error
        )
    })?;

    for entry in entries {
        if files.len() >= MAX_SCAN_FILES {
            break;
        }

        let entry = entry.map_err(|error| format!("Nu am putut citi o intrare: {}", error))?;
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "Nu am putut citi tipul intrării {}: {}",
                path.display(),
                error
            )
        })?;

        if file_type.is_symlink() {
            continue;
        }

        if file_type.is_dir() {
            if should_skip_dir(&file_name, zola_mode) {
                continue;
            }

            let relative_path = relative_project_path(root, &path)?;
            files.push(ProjectFile {
                name: file_name,
                relative_path,
                absolute_path: path.to_string_lossy().to_string(),
                role: ProjectFileRole::Asset,
                kind: ProjectFileKind::Dir,
                preview_path: None,
            });

            collect_project_files(root, &path, files, zola_mode)?;
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let Some(kind) = project_file_kind(&path) else {
            continue;
        };

        let relative_path = relative_project_path(root, &path)?;
        let Some(role) = project_file_role_for_path(&relative_path, &kind, zola_mode) else {
            continue;
        };
        let preview_path = project_preview_path(&relative_path, &kind, &role, zola_mode);

        files.push(ProjectFile {
            name: file_name,
            relative_path,
            absolute_path: path.to_string_lossy().to_string(),
            role,
            kind,
            preview_path,
        });
    }

    Ok(())
}

fn relative_project_path(root: &Path, path: &Path) -> Result<String, String> {
    Ok(path
        .strip_prefix(root)
        .map_err(|error| format!("Nu am putut calcula path relativ: {}", error))?
        .to_string_lossy()
        .replace('\\', "/"))
}

pub(crate) fn project_file_role_for_path(
    relative_path: &str,
    kind: &ProjectFileKind,
    zola_mode: bool,
) -> Option<ProjectFileRole> {
    if !zola_mode {
        return Some(match kind {
            ProjectFileKind::Html | ProjectFileKind::Md => ProjectFileRole::Page,
            ProjectFileKind::Css | ProjectFileKind::Scss => ProjectFileRole::Style,
            ProjectFileKind::Js => ProjectFileRole::Script,
            ProjectFileKind::Dir | ProjectFileKind::Image | ProjectFileKind::Other => {
                ProjectFileRole::Asset
            }
        });
    }

    let zola_relative_path = strip_zola_root_prefix(relative_path);
    let logical_zola_path = zola_path_without_theme_root(&zola_relative_path);

    if zola_relative_path.starts_with("content/") && matches!(kind, ProjectFileKind::Md) {
        return Some(ProjectFileRole::Page);
    }

    if logical_zola_path.starts_with("sass/") && matches!(kind, ProjectFileKind::Scss) {
        return Some(ProjectFileRole::Style);
    }

    if logical_zola_path.starts_with("static/") {
        return Some(match kind {
            ProjectFileKind::Css | ProjectFileKind::Scss => ProjectFileRole::Style,
            ProjectFileKind::Js => ProjectFileRole::Script,
            _ => ProjectFileRole::Asset,
        });
    }

    if is_template_relative_path(&zola_relative_path) && matches!(kind, ProjectFileKind::Html) {
        return Some(ProjectFileRole::Template);
    }

    Some(ProjectFileRole::Asset)
}

pub(crate) fn project_preview_path(
    relative_path: &str,
    kind: &ProjectFileKind,
    role: &ProjectFileRole,
    zola_mode: bool,
) -> Option<String> {
    if matches!(role, ProjectFileRole::Page) {
        if zola_mode {
            return zola_preview_path(strip_zola_root_prefix(relative_path));
        }

        if matches!(kind, ProjectFileKind::Html | ProjectFileKind::Md) {
            return Some(format!("/{}", relative_path));
        }
    }

    None
}

fn zola_preview_path(relative_path: &str) -> Option<String> {
    if !relative_path.starts_with("content/") || !relative_path.ends_with(".md") {
        return None;
    }

    let content_path = relative_path
        .trim_start_matches("content/")
        .trim_end_matches(".md");

    if content_path == "_index" {
        return Some("/".to_string());
    }

    if let Some(section_path) = content_path.strip_suffix("/_index") {
        return Some(format!("/{}/", section_path.trim_matches('/')));
    }

    Some(format!("/{}/", content_path.trim_matches('/')))
}

pub(crate) fn compare_project_files(left: &ProjectFile, right: &ProjectFile) -> std::cmp::Ordering {
    project_file_sort_key(left)
        .cmp(&project_file_sort_key(right))
        .then_with(|| left.relative_path.cmp(&right.relative_path))
}

fn project_file_sort_key(file: &ProjectFile) -> (u8, u8, usize, String) {
    match file.role {
        ProjectFileRole::Asset if matches!(file.kind, ProjectFileKind::Dir) => (
            0,
            0,
            file.relative_path.matches('/').count(),
            file.relative_path.to_ascii_lowercase(),
        ),
        ProjectFileRole::Page => {
            let path = file.relative_path.to_ascii_lowercase();
            let depth = path.matches('/').count();
            let is_root_index = path == "index.html";
            let is_nested_index = path.ends_with("/index.html");

            let page_rank = if is_root_index {
                0
            } else if is_nested_index {
                1
            } else if depth == 0 {
                2
            } else {
                3
            };

            (0, page_rank, depth, path)
        }
        ProjectFileRole::Template => (
            1,
            0,
            file.relative_path.matches('/').count(),
            file.relative_path.to_ascii_lowercase(),
        ),
        ProjectFileRole::Style => (
            2,
            0,
            file.relative_path.matches('/').count(),
            file.relative_path.to_ascii_lowercase(),
        ),
        ProjectFileRole::Script => (
            3,
            0,
            file.relative_path.matches('/').count(),
            file.relative_path.to_ascii_lowercase(),
        ),
        ProjectFileRole::Asset => (
            4,
            0,
            file.relative_path.matches('/').count(),
            file.relative_path.to_ascii_lowercase(),
        ),
    }
}

fn should_skip_dir(name: &str, zola_mode: bool) -> bool {
    if is_derived_or_internal_dir(name) {
        return true;
    }

    zola_mode && name == "public"
}

fn project_file_kind(path: &Path) -> Option<ProjectFileKind> {
    if path.file_name().and_then(|name| name.to_str()) == Some(".env") {
        return Some(ProjectFileKind::Other);
    }

    let extension = path.extension()?.to_string_lossy().to_ascii_lowercase();

    match extension.as_str() {
        "html" | "htm" => Some(ProjectFileKind::Html),
        "md" => Some(ProjectFileKind::Md),
        "css" => Some(ProjectFileKind::Css),
        "scss" => Some(ProjectFileKind::Scss),
        "js" | "mjs" => Some(ProjectFileKind::Js),
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" | "avif" | "ico" | "woff" | "woff2" => {
            Some(ProjectFileKind::Image)
        }
        _ => Some(ProjectFileKind::Other),
    }
}

fn has_zola_config(root: &Path) -> bool {
    is_real_file(&root.join("zola.toml")) || is_real_file(&root.join("config.toml"))
}

fn is_real_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
        .unwrap_or(false)
}

fn is_real_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{read_project_disk_manifest, AcceptedProjectDiskManifest};
    use std::{
        collections::{HashMap, HashSet},
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_project_root(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("panastudio-scan-test-{name}-{unique}"))
    }

    #[test]
    fn scan_includes_empty_directories() {
        let root = temp_project_root("empty-dir");
        fs::create_dir_all(root.join("sursa/static/imagini/goale")).unwrap();
        fs::write(root.join("sursa/zola.toml"), "").unwrap();
        fs::create_dir_all(root.join("sursa/content")).unwrap();

        let scan = scan_project_root(&root).unwrap();
        let empty_dir = scan
            .files
            .iter()
            .find(|file| file.relative_path == "sursa/static/imagini/goale")
            .expect("empty directory should be scanned");

        assert!(matches!(empty_dir.kind, ProjectFileKind::Dir));

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn scan_does_not_follow_symlink_files_or_directories() {
        use std::os::unix::fs::symlink;

        let root = temp_project_root("symlinks");
        let outside = temp_project_root("symlinks-outside");
        fs::create_dir_all(root.join("sursa/templates")).unwrap();
        fs::write(root.join("sursa/templates/local.html"), "local").unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.html"), "secret").unwrap();
        symlink(
            outside.join("secret.html"),
            root.join("sursa/templates/linked.html"),
        )
        .unwrap();
        symlink(&outside, root.join("sursa/external-dir")).unwrap();

        let scan = scan_project_root(&root).unwrap();

        assert!(scan
            .files
            .iter()
            .any(|file| file.relative_path == "sursa/templates/local.html"));
        assert!(!scan.files.iter().any(|file| {
            file.relative_path == "sursa/templates/linked.html"
                || file.relative_path.starts_with("sursa/external-dir")
        }));

        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside);
    }

    #[test]
    fn scan_includes_project_env_file_for_config_buffering() {
        let root = temp_project_root("env-file");
        fs::create_dir_all(root.join("sursa/content")).unwrap();
        fs::write(root.join("sursa/zola.toml"), "").unwrap();
        fs::write(root.join(".env"), "BUNNY_API_KEY=test\n").unwrap();

        let scan = scan_project_root(&root).unwrap();

        assert!(scan.files.iter().any(
            |file| file.relative_path == ".env" && matches!(file.kind, ProjectFileKind::Other)
        ));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scan_excludes_generated_export_tree_from_editable_projection() {
        let root = temp_project_root("generated-export");
        fs::create_dir_all(root.join("sursa/content")).unwrap();
        fs::create_dir_all(root.join("export/css-framework")).unwrap();
        fs::create_dir_all(root.join("export/pagini")).unwrap();
        fs::write(root.join("sursa/zola.toml"), "").unwrap();
        fs::write(root.join("export/css-framework/framework.css"), "body {}").unwrap();
        fs::write(root.join("export/pagini/index.css"), ".page {}").unwrap();

        let scan = scan_project_root(&root).unwrap();

        assert!(scan
            .files
            .iter()
            .all(|file| !file.relative_path.starts_with("export")));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn workspace_scan_projects_one_exact_revision_without_live_disk_overlay() {
        let root = temp_project_root("workspace-projection");
        fs::create_dir_all(root.join("sursa/content")).unwrap();
        fs::create_dir_all(root.join("sursa/templates")).unwrap();
        fs::create_dir_all(root.join("sursa/static")).unwrap();
        fs::write(root.join("sursa/zola.toml"), "theme = \"disk-theme\"\n").unwrap();
        fs::write(root.join("sursa/templates/index.html"), "disk template").unwrap();
        fs::write(root.join("sursa/static/removed.js"), "disk script").unwrap();
        fs::write(root.join("sursa/static/logo.png"), b"png").unwrap();

        let root = root.canonicalize().unwrap();
        let project_root = root.to_string_lossy().into_owned();
        let runtime_session_id = "scan-runtime-session".to_string();
        let accepted_disk = AcceptedProjectDiskManifest::new(
            runtime_session_id.clone(),
            project_root.clone(),
            read_project_disk_manifest(&root).unwrap(),
        )
        .unwrap();
        let projection = WorkspaceProjectionLease {
            project_root: project_root.clone(),
            runtime_session_id: runtime_session_id.clone(),
            revision: 7,
            workspace_transaction_id: Some("scan-test-7".to_string()),
            source_texts: HashMap::from([
                (
                    "sursa/zola.toml".to_string(),
                    "theme = \"workspace-theme\"\n".to_string(),
                ),
                (
                    "sursa/templates/index.html".to_string(),
                    "workspace template".to_string(),
                ),
                (
                    "sursa/templates/draft.html".to_string(),
                    "unsaved draft".to_string(),
                ),
            ]),
            resource_bytes: HashMap::new(),
            deleted_sources: HashSet::from(["sursa/static/removed.js".to_string()]),
            changed_paths: HashSet::from([
                "sursa/zola.toml".to_string(),
                "sursa/templates/index.html".to_string(),
                "sursa/templates/draft.html".to_string(),
                "sursa/static/removed.js".to_string(),
            ]),
            accepted_disk,
        };

        // These live-disk changes happen after the immutable lease was
        // captured and must not alter the Files-panel projection.
        fs::write(root.join("sursa/zola.toml"), "theme = \"external-theme\"\n").unwrap();
        fs::write(
            root.join("sursa/templates/external.html"),
            "unaccepted external file",
        )
        .unwrap();

        let scan = scan_project_workspace_projection(&projection).unwrap();
        let paths = scan
            .files
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect::<HashSet<_>>();

        assert_eq!(
            scan.kernel_session_id.as_deref(),
            Some(runtime_session_id.as_str())
        );
        assert_eq!(scan.accepted_disk_generation, Some(1));
        assert_eq!(scan.active_theme.as_deref(), Some("workspace-theme"));
        assert!(paths.contains("sursa/templates/draft.html"));
        assert!(paths.contains("sursa/static/logo.png"));
        assert!(!paths.contains("sursa/static/removed.js"));
        assert!(!paths.contains("sursa/templates/external.html"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn zola_mode_roles_include_active_theme_roots() {
        assert!(matches!(
            project_file_role_for_path(
                "sursa/themes/test-theme/templates/base.html",
                &ProjectFileKind::Html,
                true
            ),
            Some(ProjectFileRole::Template)
        ));
        assert!(matches!(
            project_file_role_for_path(
                "sursa/themes/test-theme/sass/pagini/index.scss",
                &ProjectFileKind::Scss,
                true
            ),
            Some(ProjectFileRole::Style)
        ));
        assert!(matches!(
            project_file_role_for_path(
                "sursa/themes/test-theme/static/js/site.js",
                &ProjectFileKind::Js,
                true
            ),
            Some(ProjectFileRole::Script)
        ));
    }

    #[test]
    fn zola_mode_roles_do_not_treat_arbitrary_nested_templates_as_zola_templates() {
        assert!(matches!(
            project_file_role_for_path(
                "sursa/content/example/templates/card.html",
                &ProjectFileKind::Html,
                true
            ),
            Some(ProjectFileRole::Asset)
        ));
    }
}
