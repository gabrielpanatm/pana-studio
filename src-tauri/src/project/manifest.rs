use std::{collections::BTreeMap, fs, path::Path, time::UNIX_EPOCH};

#[cfg(test)]
use std::cell::Cell;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::scope::is_derived_or_internal_dir;
use super::PROJECT_CAPACITY;
pub const ACCEPTED_PROJECT_DISK_MANIFEST_SCHEMA_VERSION: u32 = 2;

#[cfg(test)]
thread_local! {
    static PROJECT_DISK_MANIFEST_TRAVERSALS: Cell<u64> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_project_disk_manifest_traversals() {
    PROJECT_DISK_MANIFEST_TRAVERSALS.with(|counter| counter.set(0));
}

#[cfg(test)]
pub(crate) fn project_disk_manifest_traversals() -> u64 {
    PROJECT_DISK_MANIFEST_TRAVERSALS.with(Cell::get)
}

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

#[derive(Clone, Debug)]
pub(crate) struct ProjectDiskInspection {
    pub manifest: ProjectDiskManifest,
    pub entry_count: usize,
    pub inventory_truncated: bool,
    pub inventory_fingerprint: String,
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
        format!(
            "unix:{}:{}:{}:{}:{}:{}:{}:{}",
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mtime(),
            metadata.mtime_nsec(),
            metadata.ctime(),
            metadata.ctime_nsec(),
            metadata.mode(),
        )
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
    #[cfg(test)]
    PROJECT_DISK_MANIFEST_TRAVERSALS.with(|counter| counter.set(counter.get().saturating_add(1)));
    Ok(inspect_project_disk(root)?.manifest)
}

/// Captures the startup inventory and accepted-disk manifest in one sorted
/// traversal. The candidate token and the later ProjectLifecycle inspection
/// therefore describe the same filesystem observation.
pub(crate) fn inspect_project_disk(root: &Path) -> Result<ProjectDiskInspection, String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("Nu am putut rezolva rădăcina proiectului: {error}"))?;
    let output_root = crate::deploy::resolve_artifact_root(&root, &root).ok();
    let mut files = Vec::new();
    let mut manifest_truncated = false;
    let mut inventory_records = Vec::new();
    let mut inventory_truncated = false;
    collect_inspection_entries(
        &root,
        &root,
        &mut files,
        &mut manifest_truncated,
        &mut inventory_records,
        &mut inventory_truncated,
        output_root.as_deref(),
    )?;
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    inventory_records.sort();
    let mut hasher = Sha256::new();
    for record in &inventory_records {
        hasher.update(record.as_bytes());
        hasher.update([0]);
    }
    Ok(ProjectDiskInspection {
        manifest: ProjectDiskManifest {
            root: root.to_string_lossy().to_string(),
            files,
            truncated: manifest_truncated,
            max_files: PROJECT_CAPACITY.max_tracked_files,
        },
        entry_count: inventory_records.len(),
        inventory_truncated,
        inventory_fingerprint: format!("{:x}", hasher.finalize()),
    })
}

#[allow(clippy::too_many_arguments)]
fn collect_inspection_entries(
    root: &Path,
    current: &Path,
    files: &mut Vec<ProjectDiskManifestEntry>,
    manifest_truncated: &mut bool,
    inventory_records: &mut Vec<String>,
    inventory_truncated: &mut bool,
    output_root: Option<&Path>,
) -> Result<(), String> {
    let mut entries = fs::read_dir(current)
        .map_err(|error| format!("Nu am putut citi folderul {}: {error}", current.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            format!(
                "Nu am putut citi o intrare din {}: {error}",
                current.display()
            )
        })?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        if inventory_records.len() >= PROJECT_CAPACITY.max_disk_inventory_entries {
            *inventory_truncated = true;
            *manifest_truncated = true;
            break;
        }
        let path = entry.path();
        let relative_path = relative_project_path(root, &path)?;
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("Nu am putut inspecta {}: {error}", path.display()))?;
        let kind = if metadata.file_type().is_symlink() {
            "symlink"
        } else if metadata.is_dir() {
            "directory"
        } else if metadata.is_file() {
            "file"
        } else {
            "other"
        };
        inventory_records.push(format!(
            "{relative_path}\0{kind}\0{}",
            project_disk_metadata_version_token(&metadata)
        ));

        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            if !should_skip_dir(&path, output_root) {
                collect_inspection_entries(
                    root,
                    &path,
                    files,
                    manifest_truncated,
                    inventory_records,
                    inventory_truncated,
                    output_root,
                )?;
            }
            continue;
        }
        if !metadata.is_file() || !project_disk_manifest_tracks_relative_file(&relative_path) {
            continue;
        }
        if files.len() >= PROJECT_CAPACITY.max_tracked_files {
            *manifest_truncated = true;
            continue;
        }
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

fn relative_project_path(root: &Path, path: &Path) -> Result<String, String> {
    Ok(path
        .strip_prefix(root)
        .map_err(|error| format!("Nu am putut calcula path relativ: {}", error))?
        .to_string_lossy()
        .replace('\\', "/"))
}

fn should_skip_dir(path: &Path, output_root: Option<&Path>) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    is_derived_or_internal_dir(name) || output_root.is_some_and(|output| output == path)
}

fn should_track_file(relative_path: &str) -> bool {
    !relative_path.is_empty()
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
        inspect_project_disk, project_disk_manifest_changed_paths,
        project_disk_metadata_version_token, read_project_disk_manifest,
        AcceptedProjectDiskManifest, PROJECT_CAPACITY,
    };

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn startup_inspection_and_accepted_manifest_share_one_snapshot_contract() {
        let root = test_root("unified-inspection");
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(root.join("zola.toml"), "base_url = '/'\n").unwrap();
        fs::write(root.join("content/_index.md"), "+++\n+++\n").unwrap();
        fs::write(root.join("templates/index.html"), "ok").unwrap();

        let inspection = inspect_project_disk(&root).unwrap();
        let independently_read = read_project_disk_manifest(&root).unwrap();

        assert_eq!(inspection.manifest, independently_read);
        assert!(inspection.entry_count > inspection.manifest.files.len());
        assert!(!inspection.inventory_truncated);
        assert!(!inspection.inventory_fingerprint.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn metadata_version_token_changes_for_same_size_rewrite() {
        let root = test_root("same-size-rewrite");
        let path = root.join("templates/index.html");
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
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(root.join("templates/local.html"), "local").unwrap();
        fs::create_dir_all(outside.join("nested")).unwrap();
        fs::write(outside.join("secret.html"), "secret").unwrap();
        fs::write(outside.join("nested/secret.html"), "nested").unwrap();
        symlink(
            outside.join("secret.html"),
            root.join("templates/linked.html"),
        )
        .unwrap();
        symlink(&outside, root.join("external-dir")).unwrap();

        let manifest = read_project_disk_manifest(&root).unwrap();

        assert_eq!(
            manifest
                .files
                .iter()
                .map(|entry| entry.relative_path.as_str())
                .collect::<Vec<_>>(),
            vec!["templates/local.html"]
        );

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }

    #[test]
    fn manifest_tracks_regular_files_anywhere_in_the_zola_root() {
        let root = test_root("retired-design-tree");
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::create_dir_all(root.join("design")).unwrap();
        fs::write(root.join("templates/index.html"), "active").unwrap();
        fs::write(root.join("design/legacy.json"), "{}\n").unwrap();

        let manifest = read_project_disk_manifest(&root).unwrap();

        assert_eq!(
            manifest
                .files
                .iter()
                .map(|entry| entry.relative_path.as_str())
                .collect::<Vec<_>>(),
            vec!["design/legacy.json", "templates/index.html"]
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn manifest_never_tracks_default_or_configured_build_output() {
        for (label, config, output) in [
            ("default-public", "base_url = '/'\n", "public"),
            (
                "custom-output",
                "base_url = '/'\noutput_dir = 'generated/site'\n",
                "generated/site",
            ),
        ] {
            let root = test_root(label);
            fs::create_dir_all(root.join("content")).unwrap();
            fs::create_dir_all(root.join(output)).unwrap();
            fs::write(root.join("zola.toml"), config).unwrap();
            fs::write(root.join("content/_index.md"), "+++\n+++").unwrap();
            fs::write(root.join(output).join("index.html"), "generated").unwrap();

            let manifest = read_project_disk_manifest(&root).unwrap();

            assert!(manifest.files.iter().all(|entry| {
                entry.relative_path != output
                    && !entry.relative_path.starts_with(&format!("{output}/"))
            }));
            assert!(manifest
                .files
                .iter()
                .any(|entry| entry.relative_path == "content/_index.md"));
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn manifest_accepts_exactly_one_thousand_files_and_rejects_the_next_file() {
        for (count, expected_truncated) in [(1_000, false), (1_001, true)] {
            let root = test_root(&format!("file-capacity-{count}"));
            for index in 0..count {
                fs::write(root.join(format!("file-{index:04}.txt")), "x").unwrap();
            }

            let manifest = read_project_disk_manifest(&root).unwrap();

            assert_eq!(manifest.max_files, PROJECT_CAPACITY.max_tracked_files);
            assert_eq!(manifest.files.len(), PROJECT_CAPACITY.max_tracked_files);
            assert_eq!(manifest.truncated, expected_truncated);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn accepted_manifest_is_session_bound_and_advances_checked_generation() {
        let root = test_root("accepted-generation");
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(root.join("templates/index.html"), "before").unwrap();
        let root = root.canonicalize().unwrap();
        let root_string = root.to_string_lossy().to_string();
        let before = read_project_disk_manifest(&root).unwrap();
        let accepted =
            AcceptedProjectDiskManifest::new("runtime/one", root_string.clone(), before.clone())
                .unwrap();
        accepted
            .require_live_complete("runtime/one", &root_string, &root)
            .unwrap();

        fs::write(root.join("templates/index.html"), "after").unwrap();
        let after = read_project_disk_manifest(&root).unwrap();
        assert_eq!(
            project_disk_manifest_changed_paths(&before, &after).unwrap(),
            vec!["templates/index.html".to_string()]
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
