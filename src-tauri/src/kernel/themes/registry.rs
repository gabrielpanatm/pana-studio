use std::{
    collections::{BTreeMap, HashSet},
    fmt, fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Deserialize;
use tauri::{AppHandle, Manager, Runtime};
use walkdir::WalkDir;

use crate::{
    kernel::{project_path::normalize_project_relative_path, project_workspace::ProjectWorkspace},
    zola_engine::EMBEDDED_ZOLA_VERSION,
};

use super::model::{
    ThemeCatalogSnapshot, ThemeCompatibilitySnapshot, ThemeManifest, ThemePackSnapshot,
    ThemeStatus, THEME_CATALOG_SCHEMA_VERSION, THEME_PACK_SCHEMA_VERSION,
};

const MAX_THEME_PACKS: usize = 16;
const MAX_PACK_FILES: usize = 512;
const MAX_PACK_BYTES: u64 = 64 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const MAX_PREVIEW_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct ThemePackFile {
    pub relative_path: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct ThemePack {
    pub root: PathBuf,
    pub manifest: ThemeManifest,
    pub theme_name: String,
    pub theme_description: String,
    pub theme_author: Option<String>,
    pub theme_license: Option<String>,
    pub theme_homepage: Option<String>,
    pub preview_bytes: Vec<u8>,
    pub theme_files: Vec<ThemePackFile>,
    pub recipe_files: Vec<ThemePackFile>,
}

impl ThemePack {
    pub fn project_theme_files(&self, zola_prefix: &str) -> Vec<ThemePackFile> {
        self.theme_files
            .iter()
            .map(|file| ThemePackFile {
                relative_path: join_project_path(
                    zola_prefix,
                    &format!("themes/{}/{}", self.manifest.id, file.relative_path),
                ),
                bytes: file.bytes.clone(),
            })
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct ThemeRegistry {
    root: PathBuf,
    packs: BTreeMap<String, ThemePack>,
    version: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ThemeRegistryError {
    ResourceRootMissing,
    Io(String),
    Limit(String),
    UnsafeEntry(String),
    InvalidManifest(String),
    DuplicateId(String),
    Incompatible(String),
}

impl fmt::Display for ThemeRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (code, message) = match self {
            Self::ResourceRootMissing => (
                "theme_registry_root_missing",
                "Catalogul bundled de teme nu a fost găsit.".to_string(),
            ),
            Self::Io(message) => ("theme_registry_io", message.clone()),
            Self::Limit(message) => ("theme_registry_limit", message.clone()),
            Self::UnsafeEntry(message) => ("theme_registry_unsafe_entry", message.clone()),
            Self::InvalidManifest(message) => ("theme_registry_manifest_invalid", message.clone()),
            Self::DuplicateId(message) => ("theme_registry_duplicate_id", message.clone()),
            Self::Incompatible(message) => ("theme_registry_incompatible", message.clone()),
        };
        write!(formatter, "[{code}] {message}")
    }
}

impl std::error::Error for ThemeRegistryError {}

#[derive(Debug, Deserialize)]
struct OfficialThemeManifest {
    name: String,
    description: Option<String>,
    license: Option<String>,
    homepage: Option<String>,
    min_version: Option<String>,
    author: Option<OfficialThemeAuthor>,
}

#[derive(Debug, Deserialize)]
struct OfficialThemeAuthor {
    name: Option<String>,
}

impl ThemeRegistry {
    pub fn load<R: Runtime>(app: &AppHandle<R>) -> Result<Self, ThemeRegistryError> {
        let root = theme_pack_resource_candidates(app)
            .into_iter()
            .find(|candidate| candidate.is_dir())
            .ok_or(ThemeRegistryError::ResourceRootMissing)?;
        Self::load_from_root(root)
    }

    pub fn load_from_root(root: PathBuf) -> Result<Self, ThemeRegistryError> {
        require_regular_directory(&root, "catalog")?;
        let mut pack_dirs = fs::read_dir(&root)
            .map_err(|error| io_error(&root, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| io_error(&root, error))?;
        pack_dirs.sort_by_key(|entry| entry.file_name());
        if pack_dirs.len() > MAX_THEME_PACKS {
            return Err(ThemeRegistryError::Limit(format!(
                "Catalogul conține {} pachete; limita este {MAX_THEME_PACKS}.",
                pack_dirs.len()
            )));
        }

        let mut loaded = Vec::new();
        for entry in pack_dirs {
            let file_type = entry
                .file_type()
                .map_err(|error| io_error(&entry.path(), error))?;
            if file_type.is_symlink() || !file_type.is_dir() {
                return Err(ThemeRegistryError::UnsafeEntry(format!(
                    "Catalogul acceptă numai directoare regulate: {}.",
                    entry.path().display()
                )));
            }
            let pack = load_pack(&entry.path())?;
            let directory_id = entry.file_name().to_string_lossy().into_owned();
            loaded.push((directory_id, pack));
        }
        let mut packs = BTreeMap::new();
        for (_directory_id, pack) in &loaded {
            if packs
                .insert(pack.manifest.id.clone(), pack.clone())
                .is_some()
            {
                return Err(ThemeRegistryError::DuplicateId(pack.manifest.id.clone()));
            }
        }
        for (directory_id, pack) in &loaded {
            if directory_id != &pack.manifest.id {
                return Err(ThemeRegistryError::InvalidManifest(format!(
                    "ID-ul `{}` nu corespunde directorului `{directory_id}`.",
                    pack.manifest.id
                )));
            }
        }
        if packs.is_empty() {
            return Err(ThemeRegistryError::InvalidManifest(
                "Catalogul bundled nu conține nicio temă.".to_string(),
            ));
        }
        let version = registry_version(&packs);
        Ok(Self {
            root,
            packs,
            version,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn require(&self, id: &str) -> Result<&ThemePack, String> {
        self.packs
            .get(id)
            .ok_or_else(|| format!("[theme_unknown] Tema bundled `{id}` nu există."))
    }

    pub fn snapshot(
        &self,
        workspace: Option<&ProjectWorkspace>,
    ) -> Result<ThemeCatalogSnapshot, String> {
        let context = workspace
            .map(ProjectThemeContext::from_workspace)
            .transpose()?;
        let themes = self
            .packs
            .values()
            .map(|pack| snapshot_pack(pack, context.as_ref()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ThemeCatalogSnapshot {
            schema_version: THEME_CATALOG_SCHEMA_VERSION,
            registry_version: self.version.clone(),
            embedded_zola_version: EMBEDDED_ZOLA_VERSION.to_string(),
            project_root: workspace.map(|workspace| workspace.session.project_root.clone()),
            runtime_session_id: workspace.map(ProjectWorkspace::runtime_session_id),
            revision: workspace.map(|workspace| workspace.revision),
            active_theme_id: context.and_then(|context| context.active_theme),
            themes,
        })
    }
}

struct ProjectThemeContext {
    active_theme: Option<String>,
    paths: HashSet<String>,
    zola_prefix: String,
}

impl ProjectThemeContext {
    fn from_workspace(workspace: &ProjectWorkspace) -> Result<Self, String> {
        let projection = workspace.capture_projection_lease()?;
        let zola_prefix = zola_prefix(workspace)?;
        let config_path = find_config_path(&projection.source_texts, &zola_prefix);
        let active_theme = config_path
            .and_then(|path| projection.source_texts.get(&path))
            .and_then(|source| crate::zola_theme::active_theme_from_source(source));
        let paths = projection
            .accepted_disk
            .manifest
            .files
            .iter()
            .map(|entry| entry.relative_path.clone())
            .chain(
                projection
                    .source_texts
                    .keys()
                    .chain(projection.resource_bytes.keys())
                    .cloned(),
            )
            .filter(|path| !projection.deleted_sources.contains(path))
            .collect();
        Ok(Self {
            active_theme,
            paths,
            zola_prefix,
        })
    }
}

fn snapshot_pack(
    pack: &ThemePack,
    context: Option<&ProjectThemeContext>,
) -> Result<ThemePackSnapshot, String> {
    let expected = context
        .map(|context| pack.project_theme_files(&context.zola_prefix))
        .unwrap_or_default();
    let installed_count = context
        .map(|context| {
            expected
                .iter()
                .filter(|file| context.paths.contains(&file.relative_path))
                .count()
        })
        .unwrap_or(0);
    let install_complete = !expected.is_empty() && installed_count == expected.len();
    let active = context.and_then(|context| context.active_theme.as_deref())
        == Some(pack.manifest.id.as_str());
    let status = if active {
        ThemeStatus::Active
    } else if installed_count > 0 {
        ThemeStatus::Installed
    } else {
        ThemeStatus::Available
    };
    let local_override_count = context
        .map(|context| local_template_overrides(pack, context).len())
        .unwrap_or(0);
    Ok(ThemePackSnapshot {
        id: pack.manifest.id.clone(),
        name: pack.manifest.display_name.clone(),
        description: pack.manifest.summary.clone(),
        version: pack.manifest.version.clone(),
        category: pack.manifest.category.clone(),
        author: pack.theme_author.clone(),
        license: pack.theme_license.clone(),
        homepage: pack.theme_homepage.clone(),
        preview_data_url: format!(
            "data:image/webp;base64,{}",
            STANDARD.encode(&pack.preview_bytes)
        ),
        compatibility: ThemeCompatibilitySnapshot {
            minimum: pack.manifest.zola.minimum.clone(),
            tested: pack.manifest.zola.tested.clone(),
            embedded: EMBEDDED_ZOLA_VERSION.to_string(),
            compatible: is_zola_compatible(&pack.manifest),
        },
        capabilities: pack.manifest.capabilities.clone(),
        required_pages: pack.manifest.required_pages.clone(),
        required_data: pack.manifest.required_data.clone(),
        editor_anchors: pack.manifest.editor_anchors.clone(),
        theme_file_count: pack.theme_files.len(),
        theme_bytes: pack
            .theme_files
            .iter()
            .map(|file| file.bytes.len() as u64)
            .sum(),
        recipe_file_count: pack.recipe_files.len(),
        recipe_bytes: pack
            .recipe_files
            .iter()
            .map(|file| file.bytes.len() as u64)
            .sum(),
        status,
        install_complete,
        local_override_count,
    })
}

fn local_template_overrides(pack: &ThemePack, context: &ProjectThemeContext) -> Vec<String> {
    pack.theme_files
        .iter()
        .filter_map(|file| {
            file.relative_path
                .strip_prefix("templates/")
                .map(|relative| {
                    join_project_path(&context.zola_prefix, &format!("templates/{relative}"))
                })
        })
        .filter(|path| context.paths.contains(path))
        .collect()
}

pub(crate) fn zola_prefix(workspace: &ProjectWorkspace) -> Result<String, String> {
    let project_root = Path::new(&workspace.session.project_root);
    let zola_root = Path::new(&workspace.session.zola_root);
    let relative = zola_root.strip_prefix(project_root).map_err(|_| {
        "ThemeRegistry a refuzat sesiunea: Zola root nu aparține proiectului.".to_string()
    })?;
    normalize_prefix(relative)
}

pub(crate) fn find_config_path(
    sources: &std::collections::HashMap<String, String>,
    prefix: &str,
) -> Option<String> {
    ["zola.toml", "config.toml"]
        .into_iter()
        .map(|name| join_project_path(prefix, name))
        .find(|path| sources.contains_key(path))
}

fn load_pack(root: &Path) -> Result<ThemePack, ThemeRegistryError> {
    require_safe_id(
        root.file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                ThemeRegistryError::UnsafeEntry(format!(
                    "Directorul pachetului nu este UTF-8: {}.",
                    root.display()
                ))
            })?,
    )?;
    validate_pack_root_entries(root)?;
    let manifest_path = root.join("pana-theme.toml");
    let manifest_source = read_bounded_regular_file(&manifest_path, MAX_MANIFEST_BYTES)?;
    let manifest_text = std::str::from_utf8(&manifest_source).map_err(|_| {
        ThemeRegistryError::InvalidManifest("pana-theme.toml nu este UTF-8.".to_string())
    })?;
    let manifest: ThemeManifest = toml_edit::de::from_str(manifest_text).map_err(|error| {
        ThemeRegistryError::InvalidManifest(format!(
            "{} nu respectă schema: {error}.",
            manifest_path.display()
        ))
    })?;
    validate_manifest(&manifest)?;

    let preview_path = safe_join(root, &manifest.preview)?;
    if preview_path.file_name().and_then(|name| name.to_str()) != Some("preview.webp") {
        return Err(ThemeRegistryError::InvalidManifest(
            "Preview-ul canonic trebuie să fie `preview.webp`.".to_string(),
        ));
    }
    let preview_bytes = read_bounded_regular_file(&preview_path, MAX_PREVIEW_BYTES)?;
    require_webp_signature(&preview_bytes)?;

    let theme_root = root.join("theme");
    let recipe_root = root.join("recipe");
    require_regular_directory(&theme_root, "theme")?;
    require_regular_directory(&recipe_root, "recipe")?;
    let theme_files = collect_tree(&theme_root, PackTreeKind::Theme)?;
    let recipe_files = collect_tree(&recipe_root, PackTreeKind::Recipe)?;
    let theme_paths = theme_files
        .iter()
        .map(|file| file.relative_path.as_str())
        .collect::<HashSet<_>>();
    for anchor in &manifest.editor_anchors {
        let normalized =
            normalize_project_relative_path(anchor).map_err(ThemeRegistryError::InvalidManifest)?;
        if normalized != *anchor || !theme_paths.contains(anchor.as_str()) {
            return Err(ThemeRegistryError::InvalidManifest(format!(
                "Ancora editor `{anchor}` nu indică un fișier canonic din theme/."
            )));
        }
    }
    let recipe_paths = recipe_files
        .iter()
        .map(|file| file.relative_path.as_str())
        .collect::<HashSet<_>>();
    for requirement in manifest
        .required_pages
        .iter()
        .chain(manifest.required_data.iter())
    {
        if !recipe_paths.contains(requirement.as_str()) {
            return Err(ThemeRegistryError::InvalidManifest(format!(
                "Cerința `{requirement}` nu este furnizată de recipe/."
            )));
        }
    }
    let total_files = theme_files.len() + recipe_files.len() + 2;
    let total_bytes = theme_files
        .iter()
        .chain(recipe_files.iter())
        .try_fold(
            (manifest_source.len() + preview_bytes.len()) as u64,
            |total, file| total.checked_add(file.bytes.len() as u64),
        )
        .ok_or_else(|| {
            ThemeRegistryError::Limit("Contorul pachetului a depășit u64.".to_string())
        })?;
    if total_files > MAX_PACK_FILES || total_bytes > MAX_PACK_BYTES {
        return Err(ThemeRegistryError::Limit(format!(
            "Pachetul `{}` are {total_files} fișiere/{total_bytes} bytes; limitele sunt {MAX_PACK_FILES}/{MAX_PACK_BYTES}.",
            manifest.id
        )));
    }
    if !theme_files
        .iter()
        .any(|file| file.relative_path == "theme.toml")
    {
        return Err(ThemeRegistryError::InvalidManifest(format!(
            "Pachetul `{}` nu conține theme/theme.toml.",
            manifest.id
        )));
    }
    let official_source = theme_files
        .iter()
        .find(|file| file.relative_path == "theme.toml")
        .expect("verificat mai sus");
    let official_text = std::str::from_utf8(&official_source.bytes).map_err(|_| {
        ThemeRegistryError::InvalidManifest("theme/theme.toml nu este UTF-8.".to_string())
    })?;
    let official: OfficialThemeManifest =
        toml_edit::de::from_str(official_text).map_err(|error| {
            ThemeRegistryError::InvalidManifest(format!("theme/theme.toml este invalid: {error}."))
        })?;
    if official.name != manifest.id {
        return Err(ThemeRegistryError::InvalidManifest(format!(
            "theme.toml declară `{}`, dar ID-ul pachetului este `{}`.",
            official.name, manifest.id
        )));
    }
    if let Some(minimum) = official.min_version.as_deref() {
        if compare_versions(minimum, &manifest.zola.minimum).is_gt() {
            return Err(ThemeRegistryError::InvalidManifest(format!(
                "theme.toml cere Zola {minimum}, peste minimul {} declarat de pachet.",
                manifest.zola.minimum
            )));
        }
    }
    if !is_zola_compatible(&manifest) {
        return Err(ThemeRegistryError::Incompatible(format!(
            "Tema `{}` cere Zola {}, dar aplicația integrează {}.",
            manifest.id, manifest.zola.minimum, EMBEDDED_ZOLA_VERSION
        )));
    }
    Ok(ThemePack {
        root: root.to_path_buf(),
        manifest,
        theme_name: official.name,
        theme_description: official
            .description
            .unwrap_or_else(|| "Temă Zola bundled în Pană Studio.".to_string()),
        theme_author: official.author.and_then(|author| author.name),
        theme_license: official.license,
        theme_homepage: official.homepage,
        preview_bytes,
        theme_files,
        recipe_files,
    })
}

fn validate_manifest(manifest: &ThemeManifest) -> Result<(), ThemeRegistryError> {
    if manifest.schema_version != THEME_PACK_SCHEMA_VERSION {
        return Err(ThemeRegistryError::InvalidManifest(format!(
            "schema_version={} nu este suportată; versiunea curentă este {THEME_PACK_SCHEMA_VERSION}.",
            manifest.schema_version
        )));
    }
    require_safe_id(&manifest.id)?;
    require_nonempty_token("display_name", &manifest.display_name)?;
    require_nonempty_token("summary", &manifest.summary)?;
    require_version("version", &manifest.version)?;
    require_nonempty_token("category", &manifest.category)?;
    require_version("zola.minimum", &manifest.zola.minimum)?;
    require_version("zola.tested", &manifest.zola.tested)?;
    if compare_versions(&manifest.zola.minimum, &manifest.zola.tested).is_gt() {
        return Err(ThemeRegistryError::InvalidManifest(
            "zola.minimum este mai mare decât zola.tested.".to_string(),
        ));
    }
    for path in &manifest.required_pages {
        let normalized =
            normalize_project_relative_path(path).map_err(ThemeRegistryError::InvalidManifest)?;
        if normalized != *path || !normalized.starts_with("content/") {
            return Err(ThemeRegistryError::InvalidManifest(format!(
                "Pagina obligatorie trebuie să fie un path canonic content/: `{path}`."
            )));
        }
    }
    for path in &manifest.required_data {
        let normalized =
            normalize_project_relative_path(path).map_err(ThemeRegistryError::InvalidManifest)?;
        if normalized != *path || !normalized.starts_with("data/") {
            return Err(ThemeRegistryError::InvalidManifest(format!(
                "Data obligatorie trebuie să fie un path canonic data/: `{path}`."
            )));
        }
    }
    for values in [&manifest.capabilities, &manifest.editor_anchors] {
        let mut seen = HashSet::new();
        for value in values {
            require_nonempty_token("list item", value)?;
            if !seen.insert(value) {
                return Err(ThemeRegistryError::InvalidManifest(format!(
                    "Valoare duplicată în manifest: `{value}`."
                )));
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum PackTreeKind {
    Theme,
    Recipe,
}

fn collect_tree(
    root: &Path,
    tree_kind: PackTreeKind,
) -> Result<Vec<ThemePackFile>, ThemeRegistryError> {
    let mut files = Vec::new();
    for entry in WalkDir::new(root).follow_links(false).sort_by_file_name() {
        let entry = entry.map_err(|error| {
            ThemeRegistryError::Io(format!("Nu am putut parcurge {}: {error}.", root.display()))
        })?;
        if entry.path() == root {
            continue;
        }
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|error| io_error(entry.path(), error))?;
        if metadata.file_type().is_symlink() {
            return Err(ThemeRegistryError::UnsafeEntry(format!(
                "Symlink interzis în pachet: {}.",
                entry.path().display()
            )));
        }
        if metadata.is_dir() {
            continue;
        }
        if !metadata.is_file() {
            return Err(ThemeRegistryError::UnsafeEntry(format!(
                "Intrare neregulată în pachet: {}.",
                entry.path().display()
            )));
        }
        let relative = entry.path().strip_prefix(root).map_err(|_| {
            ThemeRegistryError::UnsafeEntry(format!(
                "{} a ieșit din rădăcina pachetului.",
                entry.path().display()
            ))
        })?;
        let relative = normalize_relative_path(relative)?;
        validate_pack_file_path(&relative, tree_kind)?;
        let bytes = read_bounded_regular_file(entry.path(), MAX_PACK_BYTES)?;
        files.push(ThemePackFile {
            relative_path: relative,
            bytes,
        });
        if files.len() > MAX_PACK_FILES {
            return Err(ThemeRegistryError::Limit(format!(
                "Arborele {} depășește {MAX_PACK_FILES} fișiere.",
                root.display()
            )));
        }
    }
    Ok(files)
}

fn validate_pack_root_entries(root: &Path) -> Result<(), ThemeRegistryError> {
    let expected = ["pana-theme.toml", "preview.webp", "theme", "recipe"];
    for entry in fs::read_dir(root)
        .map_err(|error| io_error(root, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io_error(root, error))?
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !expected.contains(&name.as_str()) {
            return Err(ThemeRegistryError::UnsafeEntry(format!(
                "Intrare top-level necunoscută în pachet: `{name}`."
            )));
        }
        let file_type = entry
            .file_type()
            .map_err(|error| io_error(&entry.path(), error))?;
        if file_type.is_symlink() {
            return Err(ThemeRegistryError::UnsafeEntry(format!(
                "Symlink top-level interzis: {}.",
                entry.path().display()
            )));
        }
    }
    Ok(())
}

fn validate_pack_file_path(
    relative: &str,
    tree_kind: PackTreeKind,
) -> Result<(), ThemeRegistryError> {
    let mut segments = relative.split('/');
    let first = segments.next().unwrap_or_default();
    let allowed_root = match tree_kind {
        PackTreeKind::Theme => {
            relative == "theme.toml" || matches!(first, "templates" | "sass" | "static")
        }
        PackTreeKind::Recipe => {
            matches!(first, "content" | "data" | "templates" | "sass" | "static")
        }
    };
    if !allowed_root {
        return Err(ThemeRegistryError::UnsafeEntry(format!(
            "Path-ul `{relative}` nu aparține unui root permis pentru acest arbore."
        )));
    }
    if relative == "theme.toml" {
        return Ok(());
    }
    let extension = Path::new(relative)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| {
            ThemeRegistryError::UnsafeEntry(format!(
                "Fișierul `{relative}` nu are extensie verificabilă."
            ))
        })?;
    const ALLOWED: &[&str] = &[
        "html", "htm", "toml", "scss", "sass", "css", "js", "mjs", "cjs", "ts", "json", "md",
        "txt", "xml", "svg", "csv", "yaml", "yml", "bib", "png", "jpg", "jpeg", "gif", "webp",
        "avif", "ico", "woff", "woff2", "ttf", "otf", "eot", "wasm", "map", "pdf",
    ];
    if !ALLOWED.contains(&extension.as_str()) {
        return Err(ThemeRegistryError::UnsafeEntry(format!(
            "Extensia `.{extension}` nu este permisă în pachetul bundled: `{relative}`."
        )));
    }
    Ok(())
}

fn normalize_relative_path(path: &Path) -> Result<String, ThemeRegistryError> {
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ThemeRegistryError::UnsafeEntry(format!(
            "Path necanonic în pachet: {}.",
            path.display()
        )));
    }
    let value = path.to_str().ok_or_else(|| {
        ThemeRegistryError::UnsafeEntry(format!("Path non-UTF-8: {}.", path.display()))
    })?;
    normalize_project_relative_path(&value.replace('\\', "/"))
        .map_err(ThemeRegistryError::UnsafeEntry)
}

fn safe_join(root: &Path, relative: &str) -> Result<PathBuf, ThemeRegistryError> {
    let normalized =
        normalize_project_relative_path(relative).map_err(ThemeRegistryError::UnsafeEntry)?;
    let joined = root.join(&normalized);
    if !joined.starts_with(root) {
        return Err(ThemeRegistryError::UnsafeEntry(format!(
            "Path-ul `{relative}` părăsește pachetul."
        )));
    }
    Ok(joined)
}

fn read_bounded_regular_file(path: &Path, limit: u64) -> Result<Vec<u8>, ThemeRegistryError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_error(path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ThemeRegistryError::UnsafeEntry(format!(
            "{} nu este fișier regulat.",
            path.display()
        )));
    }
    if metadata.len() > limit {
        return Err(ThemeRegistryError::Limit(format!(
            "{} are {} bytes; limita este {limit}.",
            path.display(),
            metadata.len()
        )));
    }
    fs::read(path).map_err(|error| io_error(path, error))
}

fn require_regular_directory(path: &Path, label: &str) -> Result<(), ThemeRegistryError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == ErrorKind::NotFound {
            ThemeRegistryError::InvalidManifest(format!(
                "Directorul obligatoriu `{label}` lipsește: {}.",
                path.display()
            ))
        } else {
            io_error(path, error)
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ThemeRegistryError::UnsafeEntry(format!(
            "`{label}` trebuie să fie director regulat: {}.",
            path.display()
        )));
    }
    Ok(())
}

fn require_safe_id(id: &str) -> Result<(), ThemeRegistryError> {
    if id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        || id.starts_with('-')
        || id.ends_with('-')
    {
        return Err(ThemeRegistryError::InvalidManifest(format!(
            "ID de temă nesigur: `{id}`."
        )));
    }
    Ok(())
}

fn require_nonempty_token(label: &str, value: &str) -> Result<(), ThemeRegistryError> {
    if value.trim().is_empty() || value.len() > 128 || value.contains('\0') {
        return Err(ThemeRegistryError::InvalidManifest(format!(
            "`{label}` este gol sau depășește limita."
        )));
    }
    Ok(())
}

fn require_version(label: &str, value: &str) -> Result<(), ThemeRegistryError> {
    require_nonempty_token(label, value)?;
    if parse_version(value).is_none() {
        return Err(ThemeRegistryError::InvalidManifest(format!(
            "`{label}` nu este o versiune numerică validă: `{value}`."
        )));
    }
    Ok(())
}

fn is_zola_compatible(manifest: &ThemeManifest) -> bool {
    compare_versions(&manifest.zola.minimum, EMBEDDED_ZOLA_VERSION).is_le()
}

fn parse_version(value: &str) -> Option<(u64, u64, u64)> {
    let normalized = value.trim().trim_start_matches('v');
    let core = normalized
        .split_once('-')
        .map_or(normalized, |(core, _)| core);
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

fn compare_versions(left: &str, right: &str) -> std::cmp::Ordering {
    parse_version(left)
        .unwrap_or((u64::MAX, u64::MAX, u64::MAX))
        .cmp(&parse_version(right).unwrap_or((0, 0, 0)))
}

fn require_webp_signature(bytes: &[u8]) -> Result<(), ThemeRegistryError> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return Err(ThemeRegistryError::InvalidManifest(
            "preview.webp nu are semnătura RIFF/WEBP validă.".to_string(),
        ));
    }
    Ok(())
}

fn registry_version(packs: &BTreeMap<String, ThemePack>) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for (id, pack) in packs {
        hasher.update(id.as_bytes());
        hasher.update([0]);
        hasher.update(pack.manifest.version.as_bytes());
        hasher.update([0]);
        for file in pack.theme_files.iter().chain(pack.recipe_files.iter()) {
            hasher.update(file.relative_path.as_bytes());
            hasher.update((file.bytes.len() as u64).to_le_bytes());
            hasher.update(&file.bytes);
        }
    }
    format!("{:x}", hasher.finalize())
}

fn io_error(path: &Path, error: std::io::Error) -> ThemeRegistryError {
    ThemeRegistryError::Io(format!("{}: {error}.", path.display()))
}

pub(crate) fn join_project_path(prefix: &str, relative: &str) -> String {
    if prefix.is_empty() {
        relative.to_string()
    } else {
        format!("{prefix}/{relative}")
    }
}

fn normalize_prefix(path: &Path) -> Result<String, String> {
    if path.as_os_str().is_empty() {
        return Ok(String::new());
    }
    normalize_relative_path(path).map_err(|error| error.to_string())
}

fn theme_pack_resource_candidates<R: Runtime>(app: &AppHandle<R>) -> Vec<PathBuf> {
    let mut candidates = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("theme-packs")];
    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("resources/theme-packs"));
        candidates.push(resource_dir.join("theme-packs"));
        candidates.push(resource_dir.join("src-tauri/resources/theme-packs"));
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn bundled_registry_loads_every_shipped_theme_contract() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/theme-packs");
        let registry = ThemeRegistry::load_from_root(root).unwrap();
        assert_eq!(
            registry
                .packs
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["cadru", "nord", "pana-studio", "radacini"]
        );

        let pack = registry.require("pana-studio").unwrap();
        assert_eq!(pack.manifest.schema_version, THEME_PACK_SCHEMA_VERSION);
        assert!(pack
            .theme_files
            .iter()
            .any(|file| file.relative_path == "theme.toml"));
        assert!(pack
            .recipe_files
            .iter()
            .any(|file| file.relative_path == "content/_index.md"));

        for id in ["nord", "cadru", "radacini"] {
            let pack = registry.require(id).unwrap();
            assert_eq!(pack.manifest.schema_version, THEME_PACK_SCHEMA_VERSION);
            assert_eq!(pack.manifest.preview, "preview.webp");
            assert_eq!(pack.manifest.zola.tested, EMBEDDED_ZOLA_VERSION);
            assert!(pack
                .theme_files
                .iter()
                .any(|file| file.relative_path == "templates/index.html"));
            assert!(pack
                .theme_files
                .iter()
                .any(|file| file.relative_path == "sass/tema.scss"));
            assert!(pack
                .recipe_files
                .iter()
                .any(|file| file.relative_path == "data/meniu.toml"));
            assert!(pack
                .recipe_files
                .iter()
                .any(|file| file.relative_path == "data/site.toml"));
        }
    }

    #[test]
    fn registry_rejects_invalid_manifest_and_traversal() {
        let root = temp_dir("invalid-manifest");
        create_pack(&root, "demo", "demo", "0.22.0");
        fs::write(
            root.join("demo/pana-theme.toml"),
            manifest("demo", "0.22.0").replace(
                "preview = \"preview.webp\"",
                "preview = \"../preview.webp\"",
            ),
        )
        .unwrap();
        let error = ThemeRegistry::load_from_root(root.clone()).unwrap_err();
        assert!(matches!(
            error,
            ThemeRegistryError::UnsafeEntry(_) | ThemeRegistryError::InvalidManifest(_)
        ));
        cleanup(root);
    }

    #[test]
    fn registry_rejects_duplicate_manifest_ids_before_directory_aliases() {
        let root = temp_dir("duplicate");
        create_pack(&root, "one", "same", "0.22.0");
        create_pack(&root, "two", "same", "0.22.0");
        let error = ThemeRegistry::load_from_root(root.clone()).unwrap_err();
        assert_eq!(error, ThemeRegistryError::DuplicateId("same".to_string()));
        cleanup(root);
    }

    #[test]
    fn registry_rejects_manifest_limit_and_incompatible_zola() {
        let oversized = temp_dir("oversized");
        create_pack(&oversized, "demo", "demo", "0.22.0");
        fs::write(
            oversized.join("demo/pana-theme.toml"),
            "x".repeat((MAX_MANIFEST_BYTES + 1) as usize),
        )
        .unwrap();
        assert!(matches!(
            ThemeRegistry::load_from_root(oversized.clone()).unwrap_err(),
            ThemeRegistryError::Limit(_)
        ));
        cleanup(oversized);

        let incompatible = temp_dir("incompatible");
        create_pack(&incompatible, "future", "future", "99.0.0");
        assert!(matches!(
            ThemeRegistry::load_from_root(incompatible.clone()).unwrap_err(),
            ThemeRegistryError::Incompatible(_)
        ));
        cleanup(incompatible);
    }

    #[test]
    fn registry_rejects_reserved_recipe_roots_and_unknown_extensions() {
        let reserved = temp_dir("reserved");
        create_pack(&reserved, "demo", "demo", "0.22.0");
        fs::write(reserved.join("demo/recipe/zola.toml"), "theme = 'x'\n").unwrap();
        assert!(matches!(
            ThemeRegistry::load_from_root(reserved.clone()).unwrap_err(),
            ThemeRegistryError::UnsafeEntry(_)
        ));
        cleanup(reserved);

        let executable = temp_dir("extension");
        create_pack(&executable, "demo", "demo", "0.22.0");
        fs::create_dir_all(executable.join("demo/theme/static")).unwrap();
        fs::write(executable.join("demo/theme/static/setup.sh"), "exit 0\n").unwrap();
        assert!(matches!(
            ThemeRegistry::load_from_root(executable.clone()).unwrap_err(),
            ThemeRegistryError::UnsafeEntry(_)
        ));
        cleanup(executable);
    }

    #[cfg(unix)]
    #[test]
    fn registry_rejects_symlinks_anywhere_in_a_pack() {
        use std::os::unix::fs::symlink;

        let root = temp_dir("symlink");
        create_pack(&root, "demo", "demo", "0.22.0");
        symlink(
            root.join("demo/theme/theme.toml"),
            root.join("demo/theme/templates/linked.html"),
        )
        .unwrap();
        assert!(matches!(
            ThemeRegistry::load_from_root(root.clone()).unwrap_err(),
            ThemeRegistryError::UnsafeEntry(_)
        ));
        cleanup(root);
    }

    fn create_pack(root: &Path, directory: &str, id: &str, minimum: &str) {
        let pack = root.join(directory);
        fs::create_dir_all(pack.join("theme/templates")).unwrap();
        fs::create_dir_all(pack.join("recipe/content")).unwrap();
        fs::write(pack.join("pana-theme.toml"), manifest(id, minimum)).unwrap();
        fs::write(pack.join("preview.webp"), b"RIFF\x04\0\0\0WEBP").unwrap();
        fs::write(
            pack.join("theme/theme.toml"),
            format!("name = \"{id}\"\ndescription = \"Fixture\"\nmin_version = \"{minimum}\"\n"),
        )
        .unwrap();
        fs::write(pack.join("theme/templates/base.html"), "<main></main>").unwrap();
        fs::write(pack.join("recipe/content/_index.md"), "+++\n+++\n").unwrap();
    }

    fn manifest(id: &str, minimum: &str) -> String {
        format!(
            "schema_version = 1\nid = \"{id}\"\ndisplay_name = \"Fixture\"\nsummary = \"Fixture de test\"\nversion = \"1.0.0\"\ncategory = \"test\"\npreview = \"preview.webp\"\ncapabilities = []\nrequired_pages = []\nrequired_data = []\neditor_anchors = []\n\n[zola]\nminimum = \"{minimum}\"\ntested = \"{minimum}\"\n"
        )
    }

    fn temp_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "pana-theme-registry-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn cleanup(path: PathBuf) {
        let _ = fs::remove_dir_all(path);
    }
}
