use std::{collections::BTreeMap, fs, path::Path, time::UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::scope::is_derived_or_internal_dir;

const MAX_MANIFEST_FILES: usize = 1000;
const TRACKED_ROOT_DIRS: &[&str] = &["sursa", "resurse"];
const TRACKED_ROOT_FILES: &[&str] = &[
    "AGENTS.md",
    "brief.md",
    "structura.md",
    "readme.md",
    "README.md",
    "VIZIUNE.md",
    "ARHITECTURA.md",
    "STRATEGIE.md",
    "JURNAL.md",
];

pub const ACCEPTED_PROJECT_DISK_MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDiskManifest {
    pub root: String,
    pub files: Vec<ProjectDiskManifestEntry>,
    pub truncated: bool,
    pub max_files: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDiskManifestEntry {
    pub relative_path: String,
    pub modified_ms: u128,
    pub size: u64,
    #[serde(default)]
    pub version_token: String,
}

/// Runtime authority describing the exact disk snapshot accepted by the
/// current ProjectSession instance. The manifest is deliberately coupled to
/// the ephemeral runtime session id: reopening the same project creates a new
/// authority even when the disk contents are identical.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptedProjectDiskManifest {
    pub schema_version: u32,
    pub generation: u64,
    pub runtime_session_id: String,
    pub project_root: String,
    pub manifest: ProjectDiskManifest,
}

impl AcceptedProjectDiskManifest {
    pub fn new(
        runtime_session_id: impl Into<String>,
        project_root: impl Into<String>,
        manifest: ProjectDiskManifest,
    ) -> Result<Self, String> {
        let runtime_session_id = runtime_session_id.into();
        let project_root = project_root.into();
        validate_accepted_manifest_identity(&runtime_session_id, &project_root, &manifest)?;
        Ok(Self {
            schema_version: ACCEPTED_PROJECT_DISK_MANIFEST_SCHEMA_VERSION,
            generation: 1,
            runtime_session_id,
            project_root,
            manifest,
        })
    }

    pub fn next(
        &self,
        runtime_session_id: &str,
        project_root: &str,
        manifest: ProjectDiskManifest,
    ) -> Result<Self, String> {
        self.require_identity(runtime_session_id, project_root)?;
        validate_accepted_manifest_identity(runtime_session_id, project_root, &manifest)?;
        let generation = self.generation.checked_add(1).ok_or_else(|| {
            "Accepted disk manifest generation a atins limita u64; acceptarea a fost blocată fail-closed."
                .to_string()
        })?;
        Ok(Self {
            schema_version: ACCEPTED_PROJECT_DISK_MANIFEST_SCHEMA_VERSION,
            generation,
            runtime_session_id: runtime_session_id.to_string(),
            project_root: project_root.to_string(),
            manifest,
        })
    }

    pub fn require_identity(
        &self,
        runtime_session_id: &str,
        project_root: &str,
    ) -> Result<(), String> {
        if self.runtime_session_id != runtime_session_id || self.project_root != project_root {
            return Err(format!(
                "Accepted disk manifest aparține session/root {}/{}, nu {}/{}.",
                self.runtime_session_id, self.project_root, runtime_session_id, project_root
            ));
        }
        Ok(())
    }

    pub fn require_complete(&self) -> Result<(), String> {
        if self.manifest.truncated {
            return Err(
                "Accepted disk manifest este trunchiat; autoritatea disk completă nu poate fi demonstrată."
                    .to_string(),
            );
        }
        Ok(())
    }

    /// Proves that this runtime-scoped authority still describes the complete
    /// live project disk. Callers must retain their authority lease until the
    /// read or planned write has crossed its effect boundary.
    pub fn require_live_complete(
        &self,
        runtime_session_id: &str,
        project_root: &str,
        root: &Path,
    ) -> Result<(), String> {
        self.require_identity(runtime_session_id, project_root)?;
        self.require_complete()?;
        if root != Path::new(project_root) {
            return Err(format!(
                "Accepted disk manifest a refuzat root-ul live {} pentru autoritatea {}.",
                root.display(),
                project_root
            ));
        }
        let live = read_project_disk_manifest(root)?;
        if live.truncated {
            return Err(
                "Disk-ul live produce un manifest trunchiat; autoritatea completă nu poate fi demonstrată."
                    .to_string(),
            );
        }
        if live != self.manifest {
            return Err(
                "Disk-ul live conține schimbări neacceptate de ProjectSession.".to_string(),
            );
        }
        Ok(())
    }
}

fn validate_accepted_manifest_identity(
    runtime_session_id: &str,
    project_root: &str,
    manifest: &ProjectDiskManifest,
) -> Result<(), String> {
    if runtime_session_id.trim().is_empty() || project_root.trim().is_empty() {
        return Err(
            "Accepted disk manifest cere runtime session id și project root nenule.".to_string(),
        );
    }
    if manifest.root != project_root {
        return Err(format!(
            "Accepted disk manifest root este {}, nu {}.",
            manifest.root, project_root
        ));
    }
    Ok(())
}

pub(crate) fn project_disk_metadata_version_token(metadata: &fs::Metadata) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        return format!(
            "unix:{}:{}:{}:{}:{}:{}:{}:{}",
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mtime(),
            metadata.mtime_nsec(),
            metadata.ctime(),
            metadata.ctime_nsec(),
            metadata.mode(),
        );
    }

    #[cfg(not(unix))]
    {
        let modified_ns = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        format!(
            "portable:{modified_ns}:{}:{}",
            metadata.len(),
            metadata.permissions().readonly()
        )
    }
}

pub fn read_project_disk_manifest(root: &Path) -> Result<ProjectDiskManifest, String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("Nu am putut rezolva rădăcina proiectului: {}", error))?;
    let mut files = Vec::new();
    let mut truncated = false;

    collect_manifest_entries(&root, &root, &mut files, &mut truncated)?;
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

    Ok(ProjectDiskManifest {
        root: root.to_string_lossy().to_string(),
        files,
        truncated,
        max_files: MAX_MANIFEST_FILES,
    })
}

pub(crate) fn project_disk_manifest_changed_paths(
    before: &ProjectDiskManifest,
    after: &ProjectDiskManifest,
) -> Result<Vec<String>, String> {
    if before.root != after.root {
        return Err(format!(
            "Manifestele aparțin unor proiecte diferite: {} și {}.",
            before.root, after.root
        ));
    }
    if before.truncated || after.truncated {
        return Err(
            "Delta manifestului nu poate fi demonstrată dintr-un manifest trunchiat.".to_string(),
        );
    }

    let before_entries = before
        .files
        .iter()
        .map(|entry| (entry.relative_path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let after_entries = after
        .files
        .iter()
        .map(|entry| (entry.relative_path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut paths = before_entries
        .keys()
        .chain(after_entries.keys())
        .copied()
        .collect::<Vec<_>>();
    paths.sort_unstable();
    paths.dedup();

    Ok(paths
        .into_iter()
        .filter(|path| before_entries.get(path) != after_entries.get(path))
        .map(str::to_string)
        .collect())
}

fn collect_manifest_entries(
    root: &Path,
    current: &Path,
    files: &mut Vec<ProjectDiskManifestEntry>,
    truncated: &mut bool,
) -> Result<(), String> {
    if files.len() >= MAX_MANIFEST_FILES {
        *truncated = true;
        return Ok(());
    }

    let entries = fs::read_dir(current).map_err(|error| {
        format!(
            "Nu am putut citi folderul pentru manifest {}: {}",
            current.display(),
            error
        )
    })?;

    for entry in entries {
        if files.len() >= MAX_MANIFEST_FILES {
            *truncated = true;
            break;
        }

        let entry =
            entry.map_err(|error| format!("Nu am putut citi o intrare din manifest: {}", error))?;
        let path = entry.path();
        let relative_path = relative_project_path(root, &path)?;
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "Nu am putut citi tipul intrării {}: {}",
                path.display(),
                error
            )
        })?;

        // The external-change manifest is evidence, not a filesystem crawler.
        // Never follow project symlinks into another authority root.
        if file_type.is_symlink() {
            continue;
        }

        if file_type.is_dir() {
            if should_skip_dir(&path, &relative_path) {
                continue;
            }
            collect_manifest_entries(root, &path, files, truncated)?;
            continue;
        }

        if !file_type.is_file() || !project_disk_manifest_tracks_relative_file(&relative_path) {
            continue;
        }

        let metadata = path.metadata().map_err(|error| {
            format!(
                "Nu am putut citi metadata pentru {}: {}",
                path.display(),
                error
            )
        })?;
        let modified_ms = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis())
            .unwrap_or(0);

        files.push(ProjectDiskManifestEntry {
            relative_path,
            modified_ms,
            size: metadata.len(),
            version_token: project_disk_metadata_version_token(&metadata),
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

fn should_skip_dir(path: &Path, relative_path: &str) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if is_derived_or_internal_dir(name) {
        return true;
    }
    if relative_path.is_empty() {
        return false;
    }
    let first_segment = relative_path.split('/').next().unwrap_or("");
    !TRACKED_ROOT_DIRS.contains(&first_segment)
}

fn should_track_file(relative_path: &str) -> bool {
    if TRACKED_ROOT_FILES.contains(&relative_path) {
        return true;
    }
    let first_segment = relative_path.split('/').next().unwrap_or("");
    TRACKED_ROOT_DIRS.contains(&first_segment)
}

/// Returns whether a regular project-relative file belongs to the same
/// authority surface as `read_project_disk_manifest`.
///
/// This predicate is intentionally shared with structural leaf-CAS capture so
/// that an authoritative manifest cannot be validated with one traversal
/// policy and committed with another. Directory components ignored by the
/// manifest stay outside that evidence surface; a regular file merely named
/// `build` is still tracked because only directories are ignored.
pub(crate) fn project_disk_manifest_tracks_relative_file(relative_path: &str) -> bool {
    if !should_track_file(relative_path) {
        return false;
    }
    let components = relative_path.split('/').collect::<Vec<_>>();
    if components
        .iter()
        .take(components.len().saturating_sub(1))
        .any(|component| is_derived_or_internal_dir(component))
    {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::{
        project_disk_manifest_changed_paths, project_disk_metadata_version_token,
        read_project_disk_manifest, AcceptedProjectDiskManifest,
    };

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn metadata_version_token_changes_for_same_size_rewrite() {
        let root = test_root("same-size-rewrite");
        let path = root.join("sursa/templates/index.html");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "old!").unwrap();
        let before = fs::metadata(&path).unwrap();
        let before_token = project_disk_metadata_version_token(&before);

        fs::write(&path, "new!").unwrap();
        let after = fs::metadata(&path).unwrap();
        let after_token = project_disk_metadata_version_token(&after);

        assert_eq!(before.len(), after.len());
        assert_ne!(before_token, after_token);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn manifest_does_not_follow_symlink_files_or_directories() {
        use std::os::unix::fs::symlink;

        let root = test_root("symlink-root");
        let outside = test_root("symlink-outside");
        fs::create_dir_all(root.join("sursa/templates")).unwrap();
        fs::write(root.join("sursa/templates/local.html"), "local").unwrap();
        fs::create_dir_all(outside.join("nested")).unwrap();
        fs::write(outside.join("secret.html"), "secret").unwrap();
        fs::write(outside.join("nested/secret.html"), "nested").unwrap();
        symlink(
            outside.join("secret.html"),
            root.join("sursa/templates/linked.html"),
        )
        .unwrap();
        symlink(&outside, root.join("sursa/external-dir")).unwrap();

        let manifest = read_project_disk_manifest(&root).unwrap();

        assert_eq!(
            manifest
                .files
                .iter()
                .map(|entry| entry.relative_path.as_str())
                .collect::<Vec<_>>(),
            vec!["sursa/templates/local.html"]
        );

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }

    #[test]
    fn manifest_leaves_retired_design_tree_outside_runtime_authority() {
        let root = test_root("retired-design-tree");
        fs::create_dir_all(root.join("sursa/templates")).unwrap();
        fs::create_dir_all(root.join("design")).unwrap();
        fs::write(root.join("sursa/templates/index.html"), "active").unwrap();
        fs::write(root.join("design/legacy.json"), "{}\n").unwrap();

        let manifest = read_project_disk_manifest(&root).unwrap();

        assert_eq!(
            manifest
                .files
                .iter()
                .map(|entry| entry.relative_path.as_str())
                .collect::<Vec<_>>(),
            vec!["sursa/templates/index.html"]
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn accepted_manifest_is_session_bound_and_advances_checked_generation() {
        let root = test_root("accepted-generation");
        fs::create_dir_all(root.join("sursa/templates")).unwrap();
        fs::write(root.join("sursa/templates/index.html"), "before").unwrap();
        let root = root.canonicalize().unwrap();
        let root_string = root.to_string_lossy().to_string();
        let before = read_project_disk_manifest(&root).unwrap();
        let accepted =
            AcceptedProjectDiskManifest::new("runtime/one", root_string.clone(), before.clone())
                .unwrap();
        accepted
            .require_live_complete("runtime/one", &root_string, &root)
            .unwrap();

        fs::write(root.join("sursa/templates/index.html"), "after").unwrap();
        let after = read_project_disk_manifest(&root).unwrap();
        assert_eq!(
            project_disk_manifest_changed_paths(&before, &after).unwrap(),
            vec!["sursa/templates/index.html".to_string()]
        );
        assert!(accepted
            .require_live_complete("runtime/one", &root_string, &root)
            .unwrap_err()
            .contains("schimbări neacceptate"));
        let next = accepted.next("runtime/one", &root_string, after).unwrap();
        assert_eq!(next.generation, accepted.generation + 1);
        assert!(accepted
            .next("runtime/two", &root_string, before)
            .unwrap_err()
            .contains("aparține session/root"));

        fs::remove_dir_all(root).unwrap();
    }

    fn test_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "pana-project-manifest-{label}-{}",
            TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }
}
