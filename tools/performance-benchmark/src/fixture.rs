use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::model::{FixtureSnapshot, RUN_SCHEMA_VERSION};

const IGNORED_DIRECTORIES: &[&str] = &[
    ".git",
    ".zola-cache",
    "export",
    "node_modules",
    "public",
    "target",
];

fn copy_tree(source: &Path, destination: &Path) -> Result<(), String> {
    if !source.is_dir() {
        return Err(format!("Directorul sursă nu există: {}", source.display()));
    }
    fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    let walker = WalkDir::new(source)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            entry.depth() == 0
                || !entry.file_type().is_dir()
                || !IGNORED_DIRECTORIES.contains(&entry.file_name().to_string_lossy().as_ref())
        });
    for entry in walker {
        let entry = entry.map_err(|error| error.to_string())?;
        if entry.depth() == 0 || entry.file_type().is_symlink() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|error| error.to_string())?;
        let target = destination.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target).map_err(|error| error.to_string())?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            fs::copy(entry.path(), &target)
                .map_err(|error| format!("Copierea {} a eșuat: {error}", entry.path().display()))?;
        }
    }
    Ok(())
}

fn generator_manifest(root: &Path) -> PathBuf {
    root.join("instrumente/generator-stres/Cargo.toml")
}

fn generate_profile(root: &Path, profile: &str) -> Result<(), String> {
    let manifest = generator_manifest(root);
    if !manifest.is_file() {
        return Err(format!(
            "Generatorul INDEX ZERO lipsește: {}",
            manifest.display()
        ));
    }
    let status = Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "--manifest-path",
            manifest.to_string_lossy().as_ref(),
            "--",
            "genereaza",
            profile,
        ])
        .current_dir(root)
        .status()
        .map_err(|error| format!("Generatorul INDEX ZERO nu a pornit: {error}"))?;
    if !status.success() {
        return Err(format!(
            "Generatorul INDEX ZERO a eșuat pentru profilul {profile}."
        ));
    }
    let generated_target = root.join("instrumente/generator-stres/target");
    if generated_target.is_dir() {
        fs::remove_dir_all(&generated_target)
            .map_err(|error| format!("Curățarea target-ului generatorului a eșuat: {error}"))?;
    }
    Ok(())
}

fn inventory(root: &Path) -> Result<(usize, usize, u64), String> {
    let mut files = 0;
    let mut directories = 0;
    let mut bytes = 0_u64;
    for entry in WalkDir::new(root).follow_links(false).min_depth(1) {
        let entry = entry.map_err(|error| error.to_string())?;
        if entry.file_type().is_symlink() {
            return Err(format!(
                "Fixture-ul conține un symlink: {}",
                entry.path().display()
            ));
        }
        if entry.file_type().is_dir() {
            directories += 1;
        } else if entry.file_type().is_file() {
            files += 1;
            bytes =
                bytes.saturating_add(entry.metadata().map_err(|error| error.to_string())?.len());
        }
    }
    Ok((files, directories, bytes))
}

fn normalize_boundary_files(root: &Path, expected_files: usize) -> Result<(), String> {
    let (current, _, _) = inventory(root)?;
    if current > expected_files {
        return Err(format!(
            "Proiectul normalizat are deja {current} fișiere și depășește ținta {expected_files}."
        ));
    }
    let boundary = root.join("benchmark-disk-boundary");
    fs::create_dir_all(&boundary).map_err(|error| error.to_string())?;
    for index in 0..expected_files.saturating_sub(current) {
        fs::write(
            boundary.join(format!("intrare-{index:04}.txt")),
            format!("PANA BENCHMARK DISK BOUNDARY {expected_files} / {index:04}\n"),
        )
        .map_err(|error| error.to_string())?;
    }
    let (actual, _, _) = inventory(root)?;
    if actual != expected_files {
        return Err(format!(
            "Normalizarea limitei a produs {actual} fișiere în loc de {expected_files}."
        ));
    }
    Ok(())
}

fn fixture_hash(root: &Path) -> Result<String, String> {
    let mut paths = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    paths.retain(|entry| entry.file_type().is_file());
    paths.sort_by_key(|entry| {
        entry
            .path()
            .strip_prefix(root)
            .unwrap_or(entry.path())
            .to_path_buf()
    });
    let mut hasher = Sha256::new();
    for entry in paths {
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|error| error.to_string())?;
        let relative = relative.to_string_lossy().replace('\\', "/");
        let contents = fs::read(entry.path()).map_err(|error| error.to_string())?;
        hasher.update((relative.len() as u64).to_le_bytes());
        hasher.update(relative.as_bytes());
        hasher.update((contents.len() as u64).to_le_bytes());
        hasher.update(contents);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(unix)]
fn make_read_only(root: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mut entries = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.depth()));
    for entry in entries {
        let mode = if entry.file_type().is_dir() {
            0o555
        } else {
            0o444
        };
        fs::set_permissions(entry.path(), fs::Permissions::from_mode(mode))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn make_read_only(root: &Path) -> Result<(), String> {
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(|error| error.to_string())?;
        if entry.file_type().is_file() {
            let mut permissions = entry
                .metadata()
                .map_err(|error| error.to_string())?
                .permissions();
            permissions.set_readonly(true);
            fs::set_permissions(entry.path(), permissions).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn source_manifest(root: &Path) -> Value {
    let marker = fs::read_to_string(root.join(".index-zero-generator.json"))
        .ok()
        .and_then(|value| serde_json::from_str::<Value>(&value).ok())
        .unwrap_or(Value::Null);
    let stress_manifest = fs::read_to_string(root.join("manifest-stres.toml")).unwrap_or_default();
    json!({
        "generator": marker,
        "stressManifestToml": stress_manifest,
        "seed": 20270821_u64,
    })
}

pub fn materialize_profile(
    canonical_root: &Path,
    fixtures_root: &Path,
    profile: &str,
) -> Result<FixtureSnapshot, String> {
    let profile_root = fixtures_root.join(profile);
    if profile_root.exists() {
        return Err(format!(
            "Directorul fixture există deja și nu va fi suprascris: {}",
            profile_root.display()
        ));
    }
    let generator_root = profile_root.join("generator-source");
    copy_tree(canonical_root, &generator_root)?;
    generate_profile(&generator_root, profile)?;
    let project_root = profile_root.join("project");
    copy_tree(&generator_root.join("sursa"), &project_root)?;
    match profile {
        "margine-disk" => normalize_boundary_files(&project_root, 991)?,
        "peste-limita" => normalize_boundary_files(&project_root, 1_001)?,
        _ => {}
    }
    let manifest = source_manifest(&generator_root);
    let (file_count, directory_count, total_bytes) = inventory(&project_root)?;
    let sha256 = fixture_hash(&project_root)?;
    make_read_only(&project_root)?;
    Ok(FixtureSnapshot {
        schema_version: RUN_SCHEMA_VERSION,
        profile: profile.to_string(),
        source_root: canonical_root.to_string_lossy().into_owned(),
        project_root: project_root.to_string_lossy().into_owned(),
        sha256,
        file_count,
        directory_count,
        total_bytes,
        expected_outcome: if profile == "peste-limita" {
            "rejected_fail_closed".to_string()
        } else {
            "accepted".to_string()
        },
        source_manifest: manifest,
    })
}

pub fn verify_immutable_fixture(snapshot: &FixtureSnapshot) -> Result<(), String> {
    let root = Path::new(&snapshot.project_root);
    let (files, directories, bytes) = inventory(root)?;
    let hash = fixture_hash(root)?;
    if files != snapshot.file_count
        || directories != snapshot.directory_count
        || bytes != snapshot.total_bytes
        || hash != snapshot.sha256
    {
        return Err(format!(
            "Fixture-ul {} s-a modificat după materializare.",
            snapshot.profile
        ));
    }
    Ok(())
}

pub fn remove_materialized_fixtures(root: &Path) -> Result<(), String> {
    if !root.exists() {
        return Ok(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        for entry in WalkDir::new(root).follow_links(false).into_iter().flatten() {
            let mode = if entry.file_type().is_dir() {
                0o755
            } else {
                0o644
            };
            fs::set_permissions(entry.path(), fs::Permissions::from_mode(mode))
                .map_err(|error| error.to_string())?;
        }
    }
    fs::remove_dir_all(root).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn hash_is_stable_and_detects_content_changes() {
        let root = std::env::temp_dir().join(format!(
            "pana-fixture-hash-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::write(root.join("nested/a.txt"), "a").unwrap();
        let first = fixture_hash(&root).unwrap();
        assert_eq!(fixture_hash(&root).unwrap(), first);
        fs::write(root.join("nested/a.txt"), "b").unwrap();
        assert_ne!(fixture_hash(&root).unwrap(), first);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "materializează de două ori toate profilele canonice INDEX ZERO"]
    fn canonical_profiles_are_deterministic_and_leave_source_immutable() {
        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let canonical_root = repository_root.join("tests/fixtures/projects/index-zero");
        let workspace_root = std::env::temp_dir().join(format!(
            "pana-canonical-fixture-contract-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        let canonical_hash_before = fixture_hash(&canonical_root).unwrap();

        for profile in [
            "control",
            "mare",
            "densitate",
            "margine-disk",
            "peste-limita",
        ] {
            let first =
                materialize_profile(&canonical_root, &workspace_root.join("first"), profile)
                    .unwrap();
            let second =
                materialize_profile(&canonical_root, &workspace_root.join("second"), profile)
                    .unwrap();

            assert_eq!(first.sha256, second.sha256, "profil {profile}");
            assert_eq!(first.file_count, second.file_count, "profil {profile}");
            assert_eq!(
                first.directory_count, second.directory_count,
                "profil {profile}"
            );
            assert_eq!(first.total_bytes, second.total_bytes, "profil {profile}");
            assert_eq!(first.expected_outcome, second.expected_outcome);
            verify_immutable_fixture(&first).unwrap();
            verify_immutable_fixture(&second).unwrap();

            match profile {
                "margine-disk" => assert_eq!(first.file_count, 991),
                "peste-limita" => {
                    assert_eq!(first.file_count, 1_001);
                    assert_eq!(first.expected_outcome, "rejected_fail_closed");
                }
                _ => assert_eq!(first.expected_outcome, "accepted"),
            }
        }

        assert_eq!(
            fixture_hash(&canonical_root).unwrap(),
            canonical_hash_before
        );
        remove_materialized_fixtures(&workspace_root).unwrap();
    }
}
