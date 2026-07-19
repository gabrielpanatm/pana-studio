use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs::{self, File},
    io::Read,
    path::{Component, Path, PathBuf},
};

pub const MAX_ARTIFACT_FILES: usize = 50_000;
pub const MAX_ARTIFACT_DIRECTORIES: usize = 50_000;
pub const MAX_ARTIFACT_TOTAL_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_ARTIFACT_FILE_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_ARTIFACT_PATH_DEPTH: usize = 64;
pub const MAX_ARTIFACT_RELATIVE_PATH_BYTES: usize = 4096;
const MAX_ZOLA_CONFIG_BYTES: u64 = 1024 * 1024;

#[derive(Debug)]
pub struct DeployArtifactManifest {
    pub root: PathBuf,
    pub files: Vec<DeployArtifactFile>,
    pub total_bytes: u64,
}

#[derive(Debug)]
pub struct DeployArtifactFile {
    pub relative_path: String,
    pub bytes: Vec<u8>,
    pub sha256_uppercase: String,
}

/// Resolves Zola's output directory against the canonical ProjectRoot.
///
/// `..` is accepted only while it remains inside ProjectRoot, which preserves
/// the starter contract `sursa/../export`. Absolute output paths, source
/// overlap and symlinked existing path components are rejected.
pub fn resolve_artifact_root(project_root: &Path, zola_root: &Path) -> Result<PathBuf, String> {
    let project_root = canonical_project_root(project_root)?;
    let zola_root = canonical_zola_root(&project_root, zola_root)?;
    let configured = configured_output_dir(&zola_root)?;
    let output_relative = normalize_output_relative(&project_root, &zola_root, &configured)?;
    let output_root = project_root.join(&output_relative);

    reject_source_overlap(&output_root, &zola_root)?;
    validate_existing_components_no_follow(&project_root, &output_relative)?;
    Ok(output_root)
}

/// Captures the complete deploy payload before the caller is allowed to make
/// a network request. Every file is read into the bounded manifest, with
/// entry/open/post-read name binding and a final root postflight. Upload then
/// consumes these captured bytes rather than reopening filesystem paths.
pub fn build_deploy_artifact_manifest(
    project_root: &Path,
    zola_root: &Path,
) -> Result<DeployArtifactManifest, String> {
    let root = resolve_artifact_root(project_root, zola_root)?;
    let metadata = fs::symlink_metadata(&root).map_err(|error| {
        format!(
            "Artifactul Zola nu există sau nu poate fi inspectat la {}: {error}. Rulează mai întâi Zola Build.",
            root.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "Artifactul Zola trebuie să fie un director real, fără symlink: {}.",
            root.display()
        ));
    }
    let mut directory_snapshots = vec![(root.clone(), metadata_snapshot(&metadata))];

    let mut files = Vec::new();
    let mut total_bytes = 0u64;
    for walked in walkdir::WalkDir::new(&root).follow_links(false).into_iter() {
        let entry = walked.map_err(|error| {
            format!("Scanarea artifactului Zola a eșuat și deploy-ul a fost blocat: {error}.")
        })?;
        if entry.path() == root {
            continue;
        }

        let relative = entry.path().strip_prefix(&root).map_err(|error| {
            format!("Artifactul conține un path care nu mai aparține root-ului capturat: {error}.")
        })?;
        let depth = relative.components().count();
        if depth == 0 || depth > MAX_ARTIFACT_PATH_DEPTH {
            return Err(format!(
                "Artifactul depășește adâncimea maximă de {MAX_ARTIFACT_PATH_DEPTH} segmente la {}.",
                relative.display()
            ));
        }

        let file_type = entry.file_type();
        if file_type.is_symlink() {
            return Err(format!(
                "Artifactul conține un symlink interzis: {}.",
                relative.display()
            ));
        }
        if file_type.is_dir() {
            if directory_snapshots.len() >= MAX_ARTIFACT_DIRECTORIES {
                return Err(format!(
                    "Artifactul depășește limita de {MAX_ARTIFACT_DIRECTORIES} directoare."
                ));
            }
            let metadata = entry.metadata().map_err(|error| {
                format!(
                    "Directorul artifact {} nu poate fi capturat: {error}.",
                    relative.display()
                )
            })?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(format!(
                    "Directorul artifact și-a schimbat tipul în timpul scanării: {}.",
                    relative.display()
                ));
            }
            directory_snapshots.push((entry.path().to_path_buf(), metadata_snapshot(&metadata)));
            continue;
        }
        if !file_type.is_file() {
            return Err(format!(
                "Artifactul conține un obiect special interzis: {}.",
                relative.display()
            ));
        }

        if files.len() >= MAX_ARTIFACT_FILES {
            return Err(format!(
                "Artifactul depășește limita de {MAX_ARTIFACT_FILES} fișiere."
            ));
        }
        let metadata = entry.metadata().map_err(|error| {
            format!(
                "Metadata artifactului nu poate fi citită pentru {}: {error}.",
                relative.display()
            )
        })?;
        let entry_snapshot = metadata_snapshot(&metadata);
        let size = entry_snapshot.size;
        if size > MAX_ARTIFACT_FILE_BYTES {
            return Err(format!(
                "Fișierul artifact {} are {size} bytes și depășește limita per fișier de {MAX_ARTIFACT_FILE_BYTES} bytes.",
                relative.display()
            ));
        }
        total_bytes = total_bytes.checked_add(size).ok_or_else(|| {
            "Dimensiunea totală a artifactului a produs overflow; deploy-ul a fost blocat."
                .to_string()
        })?;
        if total_bytes > MAX_ARTIFACT_TOTAL_BYTES {
            return Err(format!(
                "Artifactul depășește limita totală de {MAX_ARTIFACT_TOTAL_BYTES} bytes."
            ));
        }

        let relative_path = portable_relative_path(relative)?;
        let bytes = read_regular_file_no_follow(entry.path(), &entry_snapshot)?;
        let sha256_uppercase = sha256_uppercase(&bytes);
        files.push(DeployArtifactFile {
            relative_path,
            bytes,
            sha256_uppercase,
        });
    }

    if files.is_empty() {
        return Err("Artifactul Zola este gol; deploy-ul și purge-ul au fost blocate.".to_string());
    }
    for (directory, expected) in directory_snapshots {
        let observed = fs::symlink_metadata(&directory).map_err(|error| {
            format!(
                "Directorul artifact {} nu mai poate fi verificat după preflight: {error}.",
                directory.display()
            )
        })?;
        if metadata_snapshot(&observed) != expected {
            return Err(format!(
                "Directorul artifact {} s-a schimbat în timpul preflight-ului; manifestul ar putea fi incomplet și deploy-ul a fost blocat.",
                directory.display()
            ));
        }
    }
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(DeployArtifactManifest {
        root,
        files,
        total_bytes,
    })
}

fn canonical_project_root(project_root: &Path) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(project_root).map_err(|error| {
        format!(
            "ProjectRoot nu poate fi capturat pentru deploy la {}: {error}.",
            project_root.display()
        )
    })?;
    let metadata = fs::symlink_metadata(&canonical)
        .map_err(|error| format!("ProjectRoot nu poate fi inspectat: {error}."))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("ProjectRoot pentru deploy nu este un director real.".to_string());
    }
    Ok(canonical)
}

fn canonical_zola_root(project_root: &Path, zola_root: &Path) -> Result<PathBuf, String> {
    let relative = zola_root.strip_prefix(project_root).map_err(|_| {
        format!(
            "Zola root {} nu aparține ProjectRoot {}.",
            zola_root.display(),
            project_root.display()
        )
    })?;
    validate_existing_components_no_follow(project_root, relative)?;
    let canonical = fs::canonicalize(zola_root)
        .map_err(|error| format!("Zola root nu poate fi capturat: {error}."))?;
    if !canonical.starts_with(project_root) {
        return Err("Zola root canonizat a ieșit din ProjectRoot.".to_string());
    }
    let metadata = fs::symlink_metadata(&canonical)
        .map_err(|error| format!("Zola root nu poate fi inspectat: {error}."))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("Zola root trebuie să fie un director real, fără symlink.".to_string());
    }
    Ok(canonical)
}

fn configured_output_dir(zola_root: &Path) -> Result<String, String> {
    for config_name in ["zola.toml", "config.toml"] {
        let config_path = zola_root.join(config_name);
        let metadata = match fs::symlink_metadata(&config_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "Configurația Zola {} nu poate fi inspectată: {error}.",
                    config_path.display()
                ))
            }
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(format!(
                "Configurația Zola trebuie să fie un fișier regular fără symlink: {}.",
                config_path.display()
            ));
        }
        if metadata.len() > MAX_ZOLA_CONFIG_BYTES {
            return Err(format!(
                "Configurația Zola depășește limita de {MAX_ZOLA_CONFIG_BYTES} bytes."
            ));
        }
        let snapshot = metadata_snapshot(&metadata);
        let bytes = read_regular_file_no_follow(&config_path, &snapshot)?;
        let source = String::from_utf8(bytes)
            .map_err(|error| format!("Configurația Zola nu este UTF-8 valid: {error}."))?;
        let document = source.parse::<toml_edit::DocumentMut>().map_err(|error| {
            format!(
                "Configurația Zola {} este TOML invalid: {error}.",
                config_path.display()
            )
        })?;
        return document
            .get("output_dir")
            .map(|value| {
                value.as_str().map(str::to_owned).ok_or_else(|| {
                    "output_dir din configurația Zola trebuie să fie string.".to_string()
                })
            })
            .transpose()
            .map(|value| value.unwrap_or_else(|| "public".to_string()));
    }
    Ok("public".to_string())
}

fn normalize_output_relative(
    project_root: &Path,
    zola_root: &Path,
    configured: &str,
) -> Result<PathBuf, String> {
    if configured.trim().is_empty() {
        return Err("output_dir din configurația Zola este gol.".to_string());
    }
    if configured.contains('\\') {
        return Err(
            "output_dir cu separator Windows/backslash este interzis; folosește un path relativ portabil."
                .to_string(),
        );
    }
    let configured = Path::new(configured);
    if configured.is_absolute() {
        return Err(
            "output_dir absolut este interzis; artifactul trebuie să rămână în ProjectRoot."
                .to_string(),
        );
    }

    let mut segments = zola_root
        .strip_prefix(project_root)
        .map_err(|_| "Zola root nu aparține ProjectRoot.".to_string())?
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_os_string()),
            _ => None,
        })
        .collect::<Vec<OsString>>();

    for component in configured.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => segments.push(value.to_os_string()),
            Component::ParentDir => {
                if segments.pop().is_none() {
                    return Err(
                        "output_dir traversează în afara ProjectRoot; deploy-ul a fost blocat."
                            .to_string(),
                    );
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err("output_dir absolut este interzis.".to_string())
            }
        }
    }
    if segments.is_empty() {
        return Err("output_dir nu poate fi chiar ProjectRoot.".to_string());
    }
    Ok(segments.into_iter().collect())
}

fn reject_source_overlap(output_root: &Path, zola_root: &Path) -> Result<(), String> {
    let mut protected = vec![zola_root.join("zola.toml"), zola_root.join("config.toml")];
    protected.extend(
        [
            "content",
            "templates",
            "sass",
            "static",
            "themes",
            "date",
            "data",
        ]
        .into_iter()
        .map(|name| zola_root.join(name)),
    );
    if let Some(conflict) = protected.iter().find(|source| {
        output_root == source.as_path()
            || output_root.starts_with(source.as_path())
            || source.starts_with(output_root)
    }) {
        return Err(format!(
            "output_dir {} se suprapune cu sursa Zola protejată {}; build/deploy a fost blocat.",
            output_root.display(),
            conflict.display()
        ));
    }
    Ok(())
}

fn validate_existing_components_no_follow(root: &Path, relative: &Path) -> Result<(), String> {
    let mut current = root.to_path_buf();
    let mut missing_parent = false;
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(format!(
                "Path-ul {} nu este normalizat sub ProjectRoot.",
                relative.display()
            ));
        };
        current.push(segment);
        if missing_parent {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(format!(
                        "Path-ul de artifact conține un symlink intermediar interzis: {}.",
                        current.display()
                    ));
                }
                if !metadata.is_dir() {
                    return Err(format!(
                        "Path-ul de artifact traversează un obiect care nu este director: {}.",
                        current.display()
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => missing_parent = true,
            Err(error) => {
                return Err(format!(
                    "Path-ul de artifact nu poate fi inspectat la {}: {error}.",
                    current.display()
                ))
            }
        }
    }
    Ok(())
}

fn portable_relative_path(path: &Path) -> Result<String, String> {
    let mut parts = Vec::new();
    for component in path.components() {
        let Component::Normal(value) = component else {
            return Err(format!("Path artifact nenormalizat: {}.", path.display()));
        };
        let value = value.to_str().ok_or_else(|| {
            format!(
                "Path-ul artifactului nu este UTF-8 și nu poate fi publicat sigur: {}.",
                path.display()
            )
        })?;
        if value.is_empty() || value.chars().any(char::is_control) {
            return Err(format!(
                "Path-ul artifactului conține un segment invalid: {}.",
                path.display()
            ));
        }
        parts.push(value);
    }
    let portable = parts.join("/");
    if portable.len() > MAX_ARTIFACT_RELATIVE_PATH_BYTES {
        return Err(format!(
            "Path-ul artifactului depășește limita de {MAX_ARTIFACT_RELATIVE_PATH_BYTES} bytes: {}.",
            path.display()
        ));
    }
    Ok(portable)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MetadataSnapshot {
    size: u64,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    mode: u32,
    #[cfg(unix)]
    modified_seconds: i64,
    #[cfg(unix)]
    modified_nanoseconds: i64,
    #[cfg(unix)]
    changed_seconds: i64,
    #[cfg(unix)]
    changed_nanoseconds: i64,
}

fn metadata_snapshot(metadata: &fs::Metadata) -> MetadataSnapshot {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        MetadataSnapshot {
            size: metadata.len(),
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }
    #[cfg(not(unix))]
    {
        MetadataSnapshot {
            size: metadata.len(),
        }
    }
}

fn read_regular_file_no_follow(
    path: &Path,
    expected: &MetadataSnapshot,
) -> Result<Vec<u8>, String> {
    let mut file = open_regular_file_no_follow(path)?;
    let before = file.metadata().map_err(|error| {
        format!(
            "Artifactul {} nu poate fi verificat: {error}.",
            path.display()
        )
    })?;
    let before = metadata_snapshot(&before);
    if &before != expected {
        return Err(format!(
            "Artifactul {} nu mai corespunde intrării WalkDir capturate.",
            path.display()
        ));
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(before.size)
            .map_err(|_| format!("Fișierul artifact {} nu încape în memorie.", path.display()))?,
    );
    file.by_ref()
        .take(before.size + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Artifactul {} nu poate fi citit: {error}.", path.display()))?;
    if bytes.len() as u64 != before.size {
        return Err(format!(
            "Artifactul {} s-a schimbat în timpul preflight-ului (așteptat {} bytes, citit {}).",
            path.display(),
            before.size,
            bytes.len()
        ));
    }
    let after = file.metadata().map_err(|error| {
        format!(
            "Artifactul {} nu poate fi reverificat după citire: {error}.",
            path.display()
        )
    })?;
    let after = metadata_snapshot(&after);
    if after != before {
        return Err(format!(
            "Artifactul {} și-a schimbat identitatea, dimensiunea sau versiunea în timpul citirii.",
            path.display()
        ));
    }
    let named_after = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "Numele artifactului {} nu mai poate fi verificat după citire: {error}.",
            path.display()
        )
    })?;
    if metadata_snapshot(&named_after) != after {
        return Err(format!(
            "Numele artifactului {} nu mai indică fișierul capturat.",
            path.display()
        ));
    }
    Ok(bytes)
}

#[cfg(target_os = "linux")]
fn open_regular_file_no_follow(path: &Path) -> Result<File, String> {
    crate::kernel::write_authority::capability_open_regular_file_readonly_no_follow(
        path,
        &format!("deploy/artifact: {}", path.display()),
    )
}

#[cfg(not(target_os = "linux"))]
fn open_regular_file_no_follow(path: &Path) -> Result<File, String> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&current).map_err(|error| {
            format!(
                "Artifactul {} nu poate fi inspectat: {error}.",
                current.display()
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "Artifactul conține symlink: {}.",
                current.display()
            ));
        }
    }
    let file = File::open(path).map_err(|error| {
        format!(
            "Artifactul {} nu poate fi deschis: {error}.",
            path.display()
        )
    })?;
    if !file
        .metadata()
        .map_err(|error| format!("Artifactul nu poate fi verificat: {error}."))?
        .is_file()
    {
        return Err(format!(
            "Artifactul {} nu este fișier regular.",
            path.display()
        ));
    }
    Ok(file)
}

fn sha256_uppercase(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn starter_parent_output_is_contained_and_manifest_is_deterministic() {
        let root = fixture("starter");
        write_config(&root, "../export");
        fs::create_dir_all(root.join("export/assets")).unwrap();
        fs::write(root.join("export/index.html"), "index").unwrap();
        fs::write(root.join("export/assets/app.js"), "app").unwrap();

        let manifest = build_deploy_artifact_manifest(&root, &root.join("sursa")).unwrap();

        assert_eq!(manifest.root, root.join("export"));
        assert_eq!(manifest.total_bytes, 8);
        assert_eq!(
            manifest
                .files
                .iter()
                .map(|file| file.relative_path.as_str())
                .collect::<Vec<_>>(),
            vec!["assets/app.js", "index.html"]
        );
        assert!(manifest.files.iter().all(|file| file
            .sha256_uppercase
            .chars()
            .all(|c| !c.is_ascii_lowercase())));
        cleanup(root);
    }

    #[test]
    fn absolute_and_outside_traversal_are_rejected() {
        let absolute = fixture("absolute");
        write_config(&absolute, "/tmp/export");
        assert!(resolve_artifact_root(&absolute, &absolute.join("sursa"))
            .unwrap_err()
            .contains("absolut"));
        cleanup(absolute);

        let windows_absolute = fixture("windows-absolute");
        write_config(&windows_absolute, r"C:\export");
        assert!(
            resolve_artifact_root(&windows_absolute, &windows_absolute.join("sursa"))
                .unwrap_err()
                .contains("separator Windows")
        );
        cleanup(windows_absolute);

        let traversal = fixture("traversal");
        write_config(&traversal, "../../outside");
        assert!(resolve_artifact_root(&traversal, &traversal.join("sursa"))
            .unwrap_err()
            .contains("în afara ProjectRoot"));
        cleanup(traversal);
    }

    #[test]
    fn source_overlap_is_rejected_but_default_public_is_allowed() {
        let source = fixture("source-overlap");
        write_config(&source, "templates/generated");
        assert!(resolve_artifact_root(&source, &source.join("sursa"))
            .unwrap_err()
            .contains("se suprapune"));
        cleanup(source);

        let public = fixture("public");
        fs::write(public.join("sursa/zola.toml"), "base_url = '/'\n").unwrap();
        assert_eq!(
            resolve_artifact_root(&public, &public.join("sursa")).unwrap(),
            public.join("sursa/public")
        );
        cleanup(public);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_output_and_nested_symlink_are_rejected() {
        use std::os::unix::fs::symlink;

        let root = fixture("output-symlink");
        write_config(&root, "../export");
        let outside = unique_temp_dir("outside");
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, root.join("export")).unwrap();
        assert!(resolve_artifact_root(&root, &root.join("sursa"))
            .unwrap_err()
            .contains("symlink"));
        cleanup(root);
        cleanup(outside);

        let root = fixture("nested-symlink");
        write_config(&root, "../export");
        fs::create_dir_all(root.join("export")).unwrap();
        fs::write(root.join("target.txt"), "target").unwrap();
        symlink(root.join("target.txt"), root.join("export/link.txt")).unwrap();
        assert!(build_deploy_artifact_manifest(&root, &root.join("sursa"))
            .unwrap_err()
            .contains("symlink"));
        cleanup(root);
    }

    #[cfg(unix)]
    #[test]
    fn special_objects_are_rejected() {
        use std::process::Command;

        let root = fixture("special");
        write_config(&root, "../export");
        fs::create_dir_all(root.join("export")).unwrap();
        let status = Command::new("mkfifo")
            .arg(root.join("export/pipe"))
            .status()
            .unwrap();
        assert!(status.success());
        assert!(build_deploy_artifact_manifest(&root, &root.join("sursa"))
            .unwrap_err()
            .contains("obiect special"));
        cleanup(root);
    }

    #[test]
    fn file_and_depth_budgets_are_enforced() {
        let large = fixture("large-file");
        write_config(&large, "../export");
        fs::create_dir_all(large.join("export")).unwrap();
        let file = File::create(large.join("export/large.bin")).unwrap();
        file.set_len(MAX_ARTIFACT_FILE_BYTES + 1).unwrap();
        assert!(build_deploy_artifact_manifest(&large, &large.join("sursa"))
            .unwrap_err()
            .contains("limita per fișier"));
        cleanup(large);

        let deep = fixture("deep");
        write_config(&deep, "../export");
        let mut directory = deep.join("export");
        for _ in 0..MAX_ARTIFACT_PATH_DEPTH {
            directory.push("d");
        }
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("file.txt"), "x").unwrap();
        assert!(build_deploy_artifact_manifest(&deep, &deep.join("sursa"))
            .unwrap_err()
            .contains("adâncimea maximă"));
        cleanup(deep);
    }

    #[cfg(unix)]
    #[test]
    fn directory_snapshot_detects_nested_manifest_invalidation() {
        let root = fixture("directory-postflight");
        let nested = root.join("export/nested");
        fs::create_dir_all(&nested).unwrap();
        let before = metadata_snapshot(&fs::symlink_metadata(&nested).unwrap());

        fs::write(nested.join("late.txt"), "late").unwrap();

        let after = metadata_snapshot(&fs::symlink_metadata(&nested).unwrap());
        assert_ne!(before, after);
        cleanup(root);
    }

    fn fixture(label: &str) -> PathBuf {
        let root = unique_temp_dir(label);
        fs::create_dir_all(root.join("sursa")).unwrap();
        root.canonicalize().unwrap()
    }

    fn write_config(root: &Path, output_dir: &str) {
        fs::write(
            root.join("sursa/zola.toml"),
            format!("base_url = '/'\noutput_dir = {output_dir:?}\n"),
        )
        .unwrap();
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "panastudio-deploy-artifact-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn cleanup(path: PathBuf) {
        let _ = fs::remove_dir_all(path);
    }
}
