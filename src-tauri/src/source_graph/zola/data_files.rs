use std::{
    collections::{HashMap, HashSet},
    fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
};

use crate::{
    deploy::{resolve_artifact_root, resolve_artifact_root_from_config_source},
    source_graph::{
        model::{
            SourceCapabilities, SourceCapabilityReason, SourceDataFormat, SourceDataLocation,
            SourceOrigin,
        },
        structured_data::data_format_for_file,
    },
};

const GENERATED_OR_INTERNAL_DIRECTORIES: &[&str] = &[
    ".git",
    ".panastudio",
    ".panastudio_preview",
    ".svelte-kit",
    "build",
    "dist",
    "export",
    "node_modules",
    "target",
];

pub(crate) struct ZolaDataResolutionContext<'a> {
    pub(crate) project_root: &'a Path,
    pub(crate) zola_root: &'a Path,
    pub(crate) active_theme: Option<&'a str>,
    pub(crate) output_root: Option<&'a Path>,
    pub(crate) projected_sources: &'a HashMap<String, String>,
    pub(crate) deleted_sources: &'a HashSet<String>,
    pub(crate) exact_workspace_projection: bool,
}

#[derive(Clone)]
pub(crate) struct ResolvedZolaDataFile {
    pub(crate) path: PathBuf,
    pub(crate) file: String,
    pub(crate) logical_path: String,
    pub(crate) load_paths: Vec<String>,
    pub(crate) location: SourceDataLocation,
    pub(crate) origin: SourceOrigin,
    pub(crate) theme_name: Option<String>,
    pub(crate) capabilities: SourceCapabilities,
}

pub(crate) fn resolve_zola_output_root(
    project_root: &Path,
    zola_root: &Path,
    projected_config: Option<&str>,
) -> Result<PathBuf, String> {
    match projected_config {
        Some(source) => resolve_artifact_root_from_config_source(project_root, zola_root, source),
        None => resolve_artifact_root(project_root, zola_root),
    }
}

pub(crate) fn conventional_zola_data_file(
    context: &ZolaDataResolutionContext<'_>,
    path: PathBuf,
) -> Result<ResolvedZolaDataFile, String> {
    let file = project_relative_file(context.project_root, &path)?;
    if !file.starts_with("date/") {
        return Err(format!(
            "Catalogul convențional Date a primit un fișier din afara date/: {file}."
        ));
    }
    ensure_source_path_is_catalogable(&file, SourceDataLocation::Date)?;
    Ok(resolved_file(
        context,
        path,
        file.clone(),
        vec![file],
        SourceDataLocation::Date,
    ))
}

pub(crate) fn resolve_zola_load_data_file(
    context: &ZolaDataResolutionContext<'_>,
    reference: &str,
) -> Result<Option<ResolvedZolaDataFile>, String> {
    let normalized_reference = normalize_load_data_reference(reference)?;
    let search_path = if let Some(content_path) = normalized_reference.strip_prefix("@/") {
        format!("content/{content_path}")
    } else {
        normalized_reference.trim_start_matches('/').to_string()
    };
    let search_path = normalize_safe_relative_reference(&search_path)?;

    let mut candidates = vec![context.zola_root.join(&search_path)];
    candidates.push(context.zola_root.join("static").join(&search_path));
    candidates.push(context.zola_root.join("content").join(&search_path));
    if let Some(output_root) = context.output_root {
        candidates.push(output_root.join(&search_path));
    }
    if let Some(theme) = context.active_theme {
        candidates.push(
            context
                .zola_root
                .join("themes")
                .join(theme)
                .join("static")
                .join(&search_path),
        );
    }

    let mut seen = HashSet::new();
    for path in candidates {
        if !seen.insert(path.clone()) {
            continue;
        }
        if !source_file_exists(context, &path)? {
            continue;
        }
        let location = data_location(context, &path)?;
        let file = physical_file_label(context, &path, &location)?;
        ensure_source_path_is_catalogable(&file, location.clone())?;
        return Ok(Some(resolved_file(
            context,
            path,
            file,
            vec![normalized_reference],
            location,
        )));
    }
    Ok(None)
}

pub(crate) fn editable_local_toml_path(
    project_root: &Path,
    zola_root: &Path,
    relative_path: &str,
    projected_config: Option<&str>,
) -> Result<(), String> {
    let normalized = normalize_safe_relative_reference(relative_path)?;
    if normalized != relative_path {
        return Err(format!(
            "Calea fișierului de date trebuie să fie canonică: {normalized}."
        ));
    }
    if !normalized.to_ascii_lowercase().ends_with(".toml") {
        return Err("Editarea vizuală sigură este disponibilă numai pentru TOML.".to_string());
    }
    let output_root = resolve_zola_output_root(project_root, zola_root, projected_config)?;
    let path = project_root.join(&normalized);
    let projected_sources = HashMap::new();
    let deleted_sources = HashSet::new();
    let context = ZolaDataResolutionContext {
        project_root,
        zola_root,
        active_theme: None,
        output_root: Some(output_root.as_path()),
        projected_sources: &projected_sources,
        deleted_sources: &deleted_sources,
        exact_workspace_projection: false,
    };
    let location = data_location(&context, &path)?;
    ensure_source_path_is_catalogable(&normalized, location.clone())?;
    match location {
        SourceDataLocation::Output => {
            Err("Fișierele din output_dir Zola sunt generate și read-only.".to_string())
        }
        SourceDataLocation::Theme => {
            Err("Fișierele temei sunt read-only în activitatea Date.".to_string())
        }
        SourceDataLocation::Date
        | SourceDataLocation::Project
        | SourceDataLocation::Static
        | SourceDataLocation::Content => Ok(()),
    }
}

fn resolved_file(
    context: &ZolaDataResolutionContext<'_>,
    path: PathBuf,
    file: String,
    load_paths: Vec<String>,
    location: SourceDataLocation,
) -> ResolvedZolaDataFile {
    let format = data_format_for_file(&file);
    let capabilities = data_file_capabilities(&format, &location);
    let origin = if location == SourceDataLocation::Theme {
        SourceOrigin::Theme
    } else {
        SourceOrigin::Local
    };
    let theme_name = (location == SourceDataLocation::Theme)
        .then(|| context.active_theme.map(str::to_string))
        .flatten();
    ResolvedZolaDataFile {
        path,
        logical_path: file.clone(),
        file,
        load_paths,
        location,
        origin,
        theme_name,
        capabilities,
    }
}

fn data_file_capabilities(
    format: &SourceDataFormat,
    location: &SourceDataLocation,
) -> SourceCapabilities {
    let reason_code = match location {
        SourceDataLocation::Output => Some(SourceCapabilityReason::DataOutputReadOnly),
        SourceDataLocation::Theme => Some(SourceCapabilityReason::DataThemeReadOnly),
        _ if *format != SourceDataFormat::Toml => {
            Some(SourceCapabilityReason::DataFormatVisualUnsupported)
        }
        _ => None,
    };
    SourceCapabilities {
        can_open_in_code: !matches!(
            location,
            SourceDataLocation::Output | SourceDataLocation::Theme
        ),
        can_edit_visual: reason_code.is_none(),
        can_edit_text: false,
        can_edit_attributes: false,
        can_move: false,
        can_extract_partial: false,
        reason_code,
    }
}

fn data_location(
    context: &ZolaDataResolutionContext<'_>,
    path: &Path,
) -> Result<SourceDataLocation, String> {
    if context
        .output_root
        .is_some_and(|output_root| path.starts_with(output_root))
    {
        return Ok(SourceDataLocation::Output);
    }
    if let Some(theme) = context.active_theme {
        if path.starts_with(context.zola_root.join("themes").join(theme).join("static")) {
            return Ok(SourceDataLocation::Theme);
        }
    }
    let relative = project_relative_file(context.project_root, path)?;
    if relative.starts_with("date/") {
        Ok(SourceDataLocation::Date)
    } else if relative.starts_with("static/") {
        Ok(SourceDataLocation::Static)
    } else if relative.starts_with("content/") {
        Ok(SourceDataLocation::Content)
    } else if relative.starts_with("themes/") {
        Ok(SourceDataLocation::Theme)
    } else {
        Ok(SourceDataLocation::Project)
    }
}

fn physical_file_label(
    context: &ZolaDataResolutionContext<'_>,
    path: &Path,
    location: &SourceDataLocation,
) -> Result<String, String> {
    if let Ok(file) = path.strip_prefix(context.project_root) {
        return normalize_safe_relative_reference(&file.to_string_lossy().replace('\\', "/"));
    }
    if *location == SourceDataLocation::Output {
        let output_root = context
            .output_root
            .ok_or_else(|| "Ținta output nu are o rădăcină Zola capturată.".to_string())?;
        let relative = path
            .strip_prefix(output_root)
            .map_err(|_| "Ținta output nu aparține rădăcinii output_dir capturate.".to_string())?;
        let relative =
            normalize_safe_relative_reference(&relative.to_string_lossy().replace('\\', "/"))?;
        return Ok(format!("@output/{relative}"));
    }
    Err(format!(
        "Fișierul de date rezolvat nu aparține proiectului: {}.",
        path.display()
    ))
}

fn project_relative_file(project_root: &Path, path: &Path) -> Result<String, String> {
    let relative = path.strip_prefix(project_root).map_err(|_| {
        format!(
            "Fișierul de date {} nu aparține ProjectRoot.",
            path.display()
        )
    })?;
    normalize_safe_relative_reference(&relative.to_string_lossy().replace('\\', "/"))
}

fn ensure_source_path_is_catalogable(
    file: &str,
    location: SourceDataLocation,
) -> Result<(), String> {
    if matches!(file, "zola.toml" | "config.toml") {
        return Err(format!(
            "{file} este configurație Zola protejată, nu fișier de date editabil."
        ));
    }
    if location != SourceDataLocation::Output
        && location != SourceDataLocation::Theme
        && file
            .split('/')
            .next()
            .is_some_and(|segment| GENERATED_OR_INTERNAL_DIRECTORIES.contains(&segment))
    {
        return Err(format!(
            "{file} aparține unui director generat sau intern și nu este catalogat ca dată."
        ));
    }
    Ok(())
}

fn normalize_load_data_reference(reference: &str) -> Result<String, String> {
    let normalized = reference.trim().replace('\\', "/");
    if normalized.is_empty() {
        return Err("load_data folosește o cale locală goală.".to_string());
    }
    if normalized.starts_with("http://")
        || normalized.starts_with("https://")
        || normalized.starts_with("//")
    {
        return Err("Resolverul local nu acceptă URL-uri load_data.".to_string());
    }
    Ok(normalized)
}

fn normalize_safe_relative_reference(path: &str) -> Result<String, String> {
    let mut segments = Vec::new();
    for component in Path::new(path).components() {
        match component {
            Component::Normal(segment) => {
                let segment = segment.to_string_lossy();
                if segment.is_empty() {
                    continue;
                }
                segments.push(segment.into_owned());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(format!(
                    "Calea de date `{path}` încearcă să iasă din rădăcina Zola."
                ))
            }
            Component::RootDir | Component::Prefix(_) => {}
        }
    }
    if segments.is_empty() {
        return Err("Calea locală de date este goală după normalizare.".to_string());
    }
    Ok(segments.join("/"))
}

fn source_file_exists(
    context: &ZolaDataResolutionContext<'_>,
    path: &Path,
) -> Result<bool, String> {
    if let Some(output_root) = context.output_root {
        if path.starts_with(output_root) {
            return regular_file_without_symlinks(output_root, path);
        }
    }
    if let Ok(relative) = path.strip_prefix(context.project_root) {
        let relative =
            normalize_safe_relative_reference(&relative.to_string_lossy().replace('\\', "/"))?;
        if context.deleted_sources.contains(&relative) {
            return Ok(false);
        }
        if context.projected_sources.contains_key(&relative) {
            return Ok(true);
        }
        if context.exact_workspace_projection {
            return Ok(false);
        }
        return regular_file_without_symlinks(context.project_root, path);
    }
    Ok(false)
}

fn regular_file_without_symlinks(root: &Path, path: &Path) -> Result<bool, String> {
    let relative = path.strip_prefix(root).map_err(|_| {
        format!(
            "Resolverul de date a refuzat calea din afara rădăcinii {}.",
            root.display()
        )
    })?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(format!(
                "Resolverul de date a refuzat calea necanonică {}.",
                path.display()
            ));
        };
        current.push(segment);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "Resolverul de date a refuzat symlink-ul {}.",
                    current.display()
                ))
            }
            Ok(metadata) if current == path => return Ok(metadata.is_file()),
            Ok(metadata) if !metadata.is_dir() => return Ok(false),
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
            Err(error) => {
                return Err(format!(
                    "Resolverul de date nu a putut inspecta {}: {error}.",
                    current.display()
                ))
            }
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_traversal_and_protected_zola_configuration_targets() {
        let root = test_root("protected");
        fs::write(root.join("zola.toml"), "base_url = '/'\n").unwrap();
        let projected_sources = HashMap::new();
        let deleted_sources = HashSet::new();
        let context = ZolaDataResolutionContext {
            project_root: &root,
            zola_root: &root,
            active_theme: None,
            output_root: None,
            projected_sources: &projected_sources,
            deleted_sources: &deleted_sources,
            exact_workspace_projection: false,
        };

        assert!(resolve_zola_load_data_file(&context, "../secret.toml")
            .err()
            .unwrap()
            .contains("iasă din rădăcina Zola"));
        assert!(resolve_zola_load_data_file(&context, "zola.toml")
            .err()
            .unwrap()
            .contains("configurație Zola protejată"));

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlinked_load_data_targets() {
        use std::os::unix::fs::symlink;

        let root = test_root("symlink");
        let outside = test_root("symlink-outside");
        fs::write(outside.join("secret.json"), "{}").unwrap();
        symlink(outside.join("secret.json"), root.join("catalog.json")).unwrap();
        let projected_sources = HashMap::new();
        let deleted_sources = HashSet::new();
        let context = ZolaDataResolutionContext {
            project_root: &root,
            zola_root: &root,
            active_theme: None,
            output_root: None,
            projected_sources: &projected_sources,
            deleted_sources: &deleted_sources,
            exact_workspace_projection: false,
        };

        assert!(resolve_zola_load_data_file(&context, "catalog.json")
            .err()
            .unwrap()
            .contains("symlink"));

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }

    fn test_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "pana-zola-data-resolver-{label}-{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        root
    }
}
