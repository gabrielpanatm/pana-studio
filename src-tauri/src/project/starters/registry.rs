use std::{
    collections::{BTreeMap, HashSet},
    fmt, fs,
    path::{Component, Path, PathBuf},
};

use tauri::{AppHandle, Manager, Runtime};
use walkdir::WalkDir;

use crate::{
    kernel::project_path::normalize_project_relative_path, project::PROJECT_CAPACITY,
    zola_engine::EMBEDDED_ZOLA_VERSION,
};

use super::model::{ProjectStarterKind, ProjectStarterManifest, PROJECT_STARTER_SCHEMA_VERSION};

const MAX_STARTERS: usize = 16;
const MAX_STARTER_BYTES: u64 = 64 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const MAX_PREVIEW_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct ProjectStarterFile {
    pub relative_path: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct ProjectStarterPack {
    pub manifest: ProjectStarterManifest,
    pub preview_bytes: Option<Vec<u8>>,
    pub project_files: Vec<ProjectStarterFile>,
}

#[derive(Clone, Debug)]
pub struct ProjectStarterRegistry {
    packs: BTreeMap<String, ProjectStarterPack>,
    version: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectStarterRegistryError {
    ResourceRootMissing,
    Io(String),
    Limit(String),
    UnsafeEntry(String),
    InvalidManifest(String),
    DuplicateId(String),
    Incompatible(String),
}

impl fmt::Display for ProjectStarterRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (code, message) = match self {
            Self::ResourceRootMissing => (
                "project_starter_registry_root_missing",
                "Catalogul bundled de puncte de pornire nu a fost găsit.".to_string(),
            ),
            Self::Io(message) => ("project_starter_registry_io", message.clone()),
            Self::Limit(message) => ("project_starter_registry_limit", message.clone()),
            Self::UnsafeEntry(message) => {
                ("project_starter_registry_unsafe_entry", message.clone())
            }
            Self::InvalidManifest(message) => {
                ("project_starter_registry_manifest_invalid", message.clone())
            }
            Self::DuplicateId(message) => {
                ("project_starter_registry_duplicate_id", message.clone())
            }
            Self::Incompatible(message) => {
                ("project_starter_registry_incompatible", message.clone())
            }
        };
        write!(formatter, "[{code}] {message}")
    }
}

impl std::error::Error for ProjectStarterRegistryError {}

impl ProjectStarterRegistry {
    pub fn load<R: Runtime>(app: &AppHandle<R>) -> Result<Self, ProjectStarterRegistryError> {
        let root = resource_candidates(app)
            .into_iter()
            .find(|candidate| candidate.is_dir())
            .ok_or(ProjectStarterRegistryError::ResourceRootMissing)?;
        Self::load_from_root(root)
    }

    pub fn load_from_root(root: PathBuf) -> Result<Self, ProjectStarterRegistryError> {
        require_regular_directory(&root, "catalog")?;
        let mut entries = fs::read_dir(&root)
            .map_err(|error| io_error(&root, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| io_error(&root, error))?;
        entries.sort_by_key(|entry| entry.file_name());
        if entries.len() > MAX_STARTERS {
            return Err(ProjectStarterRegistryError::Limit(format!(
                "Catalogul conține {} puncte de pornire; limita este {MAX_STARTERS}.",
                entries.len()
            )));
        }

        let mut packs = BTreeMap::new();
        for entry in entries {
            let file_type = entry
                .file_type()
                .map_err(|error| io_error(&entry.path(), error))?;
            if file_type.is_symlink() || !file_type.is_dir() {
                return Err(ProjectStarterRegistryError::UnsafeEntry(format!(
                    "Catalogul acceptă numai directoare regulate: {}.",
                    entry.path().display()
                )));
            }
            let directory_id = entry.file_name().to_string_lossy().into_owned();
            let pack = load_pack(&entry.path())?;
            if directory_id != pack.manifest.id {
                return Err(ProjectStarterRegistryError::InvalidManifest(format!(
                    "ID-ul `{}` nu corespunde directorului `{directory_id}`.",
                    pack.manifest.id
                )));
            }
            if packs.insert(pack.manifest.id.clone(), pack).is_some() {
                return Err(ProjectStarterRegistryError::DuplicateId(directory_id));
            }
        }
        if packs.is_empty() {
            return Err(ProjectStarterRegistryError::InvalidManifest(
                "Catalogul bundled nu conține niciun punct de pornire.".to_string(),
            ));
        }
        let minimal_count = packs
            .values()
            .filter(|pack| pack.manifest.kind == ProjectStarterKind::Minimal)
            .count();
        if minimal_count != 1 {
            return Err(ProjectStarterRegistryError::InvalidManifest(format!(
                "Catalogul trebuie să conțină exact un punct minimal; au fost găsite {minimal_count}."
            )));
        }
        let version = registry_version(&packs);
        Ok(Self { packs, version })
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn packs(&self) -> impl Iterator<Item = &ProjectStarterPack> {
        self.packs.values()
    }

    pub fn require(&self, id: &str) -> Result<&ProjectStarterPack, String> {
        self.packs.get(id).ok_or_else(|| {
            format!("[project_starter_unknown] Punctul de pornire bundled `{id}` nu există.")
        })
    }
}

fn load_pack(root: &Path) -> Result<ProjectStarterPack, ProjectStarterRegistryError> {
    let directory_id = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            ProjectStarterRegistryError::UnsafeEntry(format!(
                "Directorul punctului de pornire nu este UTF-8: {}.",
                root.display()
            ))
        })?;
    require_safe_id(directory_id)?;
    validate_pack_root_entries(root)?;

    let manifest_path = root.join("starter.toml");
    let manifest_source = read_bounded_regular_file(&manifest_path, MAX_MANIFEST_BYTES)?;
    let manifest_text = std::str::from_utf8(&manifest_source).map_err(|_| {
        ProjectStarterRegistryError::InvalidManifest(
            "starter.toml nu este UTF-8 valid.".to_string(),
        )
    })?;
    let manifest: ProjectStarterManifest =
        toml_edit::de::from_str(manifest_text).map_err(|error| {
            ProjectStarterRegistryError::InvalidManifest(format!(
                "{} nu respectă schema: {error}.",
                manifest_path.display()
            ))
        })?;
    validate_manifest(&manifest)?;

    let preview_bytes = match manifest.preview.as_deref() {
        Some("preview.webp") => {
            let bytes = read_bounded_regular_file(&root.join("preview.webp"), MAX_PREVIEW_BYTES)?;
            require_webp_signature(&bytes)?;
            Some(bytes)
        }
        Some(preview) => {
            return Err(ProjectStarterRegistryError::InvalidManifest(format!(
                "Preview-ul canonic trebuie să fie `preview.webp`, nu `{preview}`."
            )));
        }
        None => None,
    };

    let project_root = root.join("project");
    require_regular_directory(&project_root, "project")?;
    let project_files = collect_project_tree(&project_root)?;
    validate_normalized_project(&manifest, &project_files)?;

    let total_bytes = manifest_source.len() as u64
        + preview_bytes
            .as_ref()
            .map(|bytes| bytes.len() as u64)
            .unwrap_or_default()
        + project_files
            .iter()
            .map(|file| file.bytes.len() as u64)
            .sum::<u64>();
    if total_bytes > MAX_STARTER_BYTES {
        return Err(ProjectStarterRegistryError::Limit(format!(
            "Punctul de pornire `{}` are {total_bytes} bytes; limita este {MAX_STARTER_BYTES}.",
            manifest.id
        )));
    }

    Ok(ProjectStarterPack {
        manifest,
        preview_bytes,
        project_files,
    })
}

fn validate_manifest(manifest: &ProjectStarterManifest) -> Result<(), ProjectStarterRegistryError> {
    if manifest.schema_version != PROJECT_STARTER_SCHEMA_VERSION {
        return Err(ProjectStarterRegistryError::InvalidManifest(format!(
            "schema_version={} nu este suportată; versiunea curentă este {PROJECT_STARTER_SCHEMA_VERSION}.",
            manifest.schema_version
        )));
    }
    require_safe_id(&manifest.id)?;
    require_nonempty("display_name", &manifest.display_name)?;
    require_nonempty("summary", &manifest.summary)?;
    require_version("version", &manifest.version)?;
    require_nonempty("category", &manifest.category)?;
    require_version("zola.minimum", &manifest.zola.minimum)?;
    require_version("zola.tested", &manifest.zola.tested)?;
    if compare_versions(&manifest.zola.minimum, &manifest.zola.tested).is_gt() {
        return Err(ProjectStarterRegistryError::InvalidManifest(
            "zola.minimum este mai mare decât zola.tested.".to_string(),
        ));
    }
    if compare_versions(&manifest.zola.minimum, EMBEDDED_ZOLA_VERSION).is_gt() {
        return Err(ProjectStarterRegistryError::Incompatible(format!(
            "Punctul de pornire `{}` cere Zola {}, dar aplicația integrează {}.",
            manifest.id, manifest.zola.minimum, EMBEDDED_ZOLA_VERSION
        )));
    }
    let mut capabilities = HashSet::new();
    for capability in &manifest.capabilities {
        require_nonempty("capability", capability)?;
        if !capabilities.insert(capability) {
            return Err(ProjectStarterRegistryError::InvalidManifest(format!(
                "Capabilitate duplicată în manifest: `{capability}`."
            )));
        }
    }
    Ok(())
}

fn validate_normalized_project(
    manifest: &ProjectStarterManifest,
    files: &[ProjectStarterFile],
) -> Result<(), ProjectStarterRegistryError> {
    let paths = files
        .iter()
        .map(|file| file.relative_path.as_str())
        .collect::<HashSet<_>>();
    for required in [
        ".gitignore",
        "zola.toml",
        "content/_index.md",
        "templates/index.html",
    ] {
        if !paths.contains(required) {
            return Err(ProjectStarterRegistryError::InvalidManifest(format!(
                "Punctul de pornire `{}` nu conține `{required}`.",
                manifest.id
            )));
        }
    }
    if paths.iter().any(|path| path.starts_with("themes/")) {
        return Err(ProjectStarterRegistryError::InvalidManifest(format!(
            "Punctul de pornire `{}` nu este normalizat: conține directorul `themes/`.",
            manifest.id
        )));
    }
    let config = files
        .iter()
        .find(|file| file.relative_path == "zola.toml")
        .expect("verificat mai sus");
    let config = std::str::from_utf8(&config.bytes).map_err(|_| {
        ProjectStarterRegistryError::InvalidManifest("zola.toml nu este UTF-8 valid.".to_string())
    })?;
    let document = config.parse::<toml_edit::DocumentMut>().map_err(|error| {
        ProjectStarterRegistryError::InvalidManifest(format!(
            "zola.toml din `{}` nu este TOML valid: {error}.",
            manifest.id
        ))
    })?;
    if document.get("theme").is_some() {
        return Err(ProjectStarterRegistryError::InvalidManifest(format!(
            "Punctul de pornire `{}` nu este normalizat: zola.toml declară cheia `theme`.",
            manifest.id
        )));
    }
    Ok(())
}

fn collect_project_tree(
    root: &Path,
) -> Result<Vec<ProjectStarterFile>, ProjectStarterRegistryError> {
    let mut files = Vec::new();
    for entry in WalkDir::new(root).follow_links(false).sort_by_file_name() {
        let entry = entry.map_err(|error| {
            ProjectStarterRegistryError::Io(format!(
                "Nu am putut parcurge {}: {error}.",
                root.display()
            ))
        })?;
        if entry.path() == root {
            continue;
        }
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|error| io_error(entry.path(), error))?;
        if metadata.file_type().is_symlink() {
            return Err(ProjectStarterRegistryError::UnsafeEntry(format!(
                "Symlink interzis în punctul de pornire: {}.",
                entry.path().display()
            )));
        }
        if metadata.is_dir() {
            continue;
        }
        if !metadata.is_file() {
            return Err(ProjectStarterRegistryError::UnsafeEntry(format!(
                "Intrare neregulată în punctul de pornire: {}.",
                entry.path().display()
            )));
        }
        let relative = entry.path().strip_prefix(root).map_err(|_| {
            ProjectStarterRegistryError::UnsafeEntry(format!(
                "{} a ieșit din rădăcina proiectului normalizat.",
                entry.path().display()
            ))
        })?;
        let relative = normalize_relative_path(relative)?;
        validate_project_file_path(&relative)?;
        let bytes = read_bounded_regular_file(entry.path(), MAX_STARTER_BYTES)?;
        files.push(ProjectStarterFile {
            relative_path: relative,
            bytes,
        });
        if files.len() > PROJECT_CAPACITY.max_tracked_files {
            return Err(ProjectStarterRegistryError::Limit(format!(
                "Arborele {} depășește {} fișiere.",
                root.display(),
                PROJECT_CAPACITY.max_tracked_files
            )));
        }
    }
    Ok(files)
}

fn validate_pack_root_entries(root: &Path) -> Result<(), ProjectStarterRegistryError> {
    let mut seen_preview = false;
    for entry in fs::read_dir(root)
        .map_err(|error| io_error(root, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io_error(root, error))?
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !matches!(name.as_str(), "starter.toml" | "preview.webp" | "project") {
            return Err(ProjectStarterRegistryError::UnsafeEntry(format!(
                "Intrare top-level necunoscută în punctul de pornire: `{name}`."
            )));
        }
        if name == "preview.webp" {
            seen_preview = true;
        }
        let file_type = entry
            .file_type()
            .map_err(|error| io_error(&entry.path(), error))?;
        if file_type.is_symlink() {
            return Err(ProjectStarterRegistryError::UnsafeEntry(format!(
                "Symlink top-level interzis: {}.",
                entry.path().display()
            )));
        }
    }
    let manifest_path = root.join("starter.toml");
    if !manifest_path.is_file() || !root.join("project").is_dir() {
        return Err(ProjectStarterRegistryError::InvalidManifest(format!(
            "{} trebuie să conțină starter.toml și project/.",
            root.display()
        )));
    }
    if seen_preview && !root.join("preview.webp").is_file() {
        return Err(ProjectStarterRegistryError::UnsafeEntry(
            "preview.webp nu este fișier regulat.".to_string(),
        ));
    }
    Ok(())
}

fn validate_project_file_path(relative: &str) -> Result<(), ProjectStarterRegistryError> {
    let first = relative.split('/').next().unwrap_or_default();
    let allowed = matches!(relative, ".gitignore" | "zola.toml")
        || matches!(first, "content" | "templates" | "data" | "sass" | "static");
    if !allowed || first == "themes" {
        return Err(ProjectStarterRegistryError::UnsafeEntry(format!(
            "Path-ul `{relative}` nu aparține structurii Zola normalizate permise."
        )));
    }
    if matches!(relative, ".gitignore" | "zola.toml") {
        return Ok(());
    }
    let extension = Path::new(relative)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| {
            ProjectStarterRegistryError::UnsafeEntry(format!(
                "Fișierul `{relative}` nu are extensie verificabilă."
            ))
        })?;
    const ALLOWED: &[&str] = &[
        "html", "htm", "toml", "scss", "sass", "css", "js", "mjs", "cjs", "ts", "json", "md",
        "txt", "xml", "svg", "csv", "yaml", "yml", "bib", "png", "jpg", "jpeg", "gif", "webp",
        "avif", "ico", "woff", "woff2", "ttf", "otf", "eot", "wasm", "map", "pdf",
    ];
    if !ALLOWED.contains(&extension.as_str()) {
        return Err(ProjectStarterRegistryError::UnsafeEntry(format!(
            "Extensia `.{extension}` nu este permisă în `{relative}`."
        )));
    }
    Ok(())
}

fn normalize_relative_path(path: &Path) -> Result<String, ProjectStarterRegistryError> {
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ProjectStarterRegistryError::UnsafeEntry(format!(
            "Path necanonic în punctul de pornire: {}.",
            path.display()
        )));
    }
    let relative = path.to_str().ok_or_else(|| {
        ProjectStarterRegistryError::UnsafeEntry(format!(
            "Path non-UTF-8 în punctul de pornire: {}.",
            path.display()
        ))
    })?;
    normalize_project_relative_path(relative).map_err(ProjectStarterRegistryError::UnsafeEntry)
}

fn read_bounded_regular_file(
    path: &Path,
    limit: u64,
) -> Result<Vec<u8>, ProjectStarterRegistryError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_error(path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ProjectStarterRegistryError::UnsafeEntry(format!(
            "Fișier regulat obligatoriu: {}.",
            path.display()
        )));
    }
    if metadata.len() > limit {
        return Err(ProjectStarterRegistryError::Limit(format!(
            "{} depășește limita de {limit} bytes.",
            path.display()
        )));
    }
    fs::read(path).map_err(|error| io_error(path, error))
}

fn require_regular_directory(path: &Path, label: &str) -> Result<(), ProjectStarterRegistryError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ProjectStarterRegistryError::InvalidManifest(format!(
                "Directorul obligatoriu `{label}` lipsește: {}.",
                path.display()
            ))
        } else {
            io_error(path, error)
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ProjectStarterRegistryError::UnsafeEntry(format!(
            "`{label}` trebuie să fie director regulat: {}.",
            path.display()
        )));
    }
    Ok(())
}

fn require_safe_id(id: &str) -> Result<(), ProjectStarterRegistryError> {
    let valid = !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !id.starts_with('-')
        && !id.ends_with('-');
    if !valid {
        return Err(ProjectStarterRegistryError::InvalidManifest(format!(
            "ID de punct de pornire invalid: `{id}`."
        )));
    }
    Ok(())
}

fn require_nonempty(label: &str, value: &str) -> Result<(), ProjectStarterRegistryError> {
    if value.trim().is_empty() || value.len() > 512 {
        return Err(ProjectStarterRegistryError::InvalidManifest(format!(
            "Câmpul `{label}` trebuie să fie nevid și bounded."
        )));
    }
    Ok(())
}

fn require_version(label: &str, value: &str) -> Result<(), ProjectStarterRegistryError> {
    if parse_version(value).is_none() {
        return Err(ProjectStarterRegistryError::InvalidManifest(format!(
            "Versiune invalidă în `{label}`: `{value}`."
        )));
    }
    Ok(())
}

fn parse_version(value: &str) -> Option<(u64, u64, u64)> {
    let mut parts = value.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    (parts.next().is_none()).then_some((major, minor, patch))
}

fn compare_versions(left: &str, right: &str) -> std::cmp::Ordering {
    parse_version(left)
        .unwrap_or_default()
        .cmp(&parse_version(right).unwrap_or_default())
}

fn require_webp_signature(bytes: &[u8]) -> Result<(), ProjectStarterRegistryError> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return Err(ProjectStarterRegistryError::InvalidManifest(
            "preview.webp nu are semnătura RIFF/WEBP validă.".to_string(),
        ));
    }
    Ok(())
}

fn registry_version(packs: &BTreeMap<String, ProjectStarterPack>) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for (id, pack) in packs {
        hasher.update(id.as_bytes());
        hasher.update([0]);
        hasher.update(pack.manifest.version.as_bytes());
        hasher.update([0]);
        if let Some(preview) = &pack.preview_bytes {
            hasher.update(preview);
        }
        for file in &pack.project_files {
            hasher.update(file.relative_path.as_bytes());
            hasher.update((file.bytes.len() as u64).to_le_bytes());
            hasher.update(&file.bytes);
        }
    }
    format!("{:x}", hasher.finalize())
}

fn resource_candidates<R: Runtime>(app: &AppHandle<R>) -> Vec<PathBuf> {
    let mut candidates =
        vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/project-starters")];
    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("resources/project-starters"));
        candidates.push(resource_dir.join("project-starters"));
        candidates.push(resource_dir.join("src-tauri/resources/project-starters"));
    }
    candidates
}

fn io_error(path: &Path, error: std::io::Error) -> ProjectStarterRegistryError {
    ProjectStarterRegistryError::Io(format!("{}: {error}.", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_starters_are_normalized_complete_projects() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/project-starters");
        let registry = ProjectStarterRegistry::load_from_root(root).unwrap();
        assert_eq!(registry.packs().count(), 5);
        assert!(registry.packs().all(|pack| {
            pack.project_files
                .iter()
                .all(|file| !file.relative_path.starts_with("themes/"))
        }));
        assert!(registry.packs().all(|pack| {
            pack.project_files
                .iter()
                .find(|file| file.relative_path == "zola.toml")
                .and_then(|file| std::str::from_utf8(&file.bytes).ok())
                .is_some_and(|source| {
                    source
                        .parse::<toml_edit::DocumentMut>()
                        .ok()
                        .is_some_and(|document| document.get("theme").is_none())
                })
        }));
    }
}
