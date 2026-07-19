use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    hash::{Hash, Hasher},
    io::ErrorKind,
    path::{Component, Path, PathBuf},
};

use sha2::{Digest, Sha256};
use tauri::{AppHandle, Runtime};
use walkdir::WalkDir;

use super::{
    annotate::{is_template_relative_path, preprocess_template_with_revision, SourceIdIndex},
    project::preview_project_dir,
};
use crate::{
    kernel::{
        project_workspace::WorkspaceProjectionLease,
        write_authority::{
            WriteAuthority, WriteCategory, WriteIntent, WriteOperationKind, WriteOwner,
            WritePolicy, WriteTarget,
        },
    },
    zola_theme::active_theme_from_source,
};

const MAX_WORKSPACE_PREVIEW_ENTRIES: usize = 16_384;
const MAX_WORKSPACE_PREVIEW_BYTES: u64 = 512 * 1024 * 1024;
const MAX_SEEDED_ARTIFACT_FILES: usize = 16_384;
const MAX_SEEDED_ARTIFACT_BYTES: u64 = 512 * 1024 * 1024;
const SKIPPED_SOURCE_DIRECTORIES: &[&str] = &[
    ".git",
    ".svelte-kit",
    "node_modules",
    "target",
    "public",
    "export",
];

/// Exact source state materialized in the persistent editor-preview root.
/// Hashes describe the unannotated ProjectWorkspace sources, even though HTML
/// templates stored in the derived root contain Pană source anchors.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PersistentProjectionManifest {
    pub accepted_disk_generation: u64,
    pub active_theme: Option<String>,
    pub text_hashes: BTreeMap<String, String>,
    pub resource_hashes: BTreeMap<String, String>,
    pub deleted_sources: BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PersistentProjectionUpdate {
    pub projection_root: PathBuf,
    pub manifest: PersistentProjectionManifest,
    pub projected_paths: Vec<String>,
    pub baseline_rebuilt: bool,
}

#[derive(Default)]
struct MaterializationBudget {
    entries: usize,
    bytes: u64,
}

impl MaterializationBudget {
    fn reserve(&mut self, path: &Path, bytes: u64) -> Result<(), String> {
        self.entries = self.entries.saturating_add(1);
        if self.entries > MAX_WORKSPACE_PREVIEW_ENTRIES {
            return Err(format!(
                "Proiecția ProjectWorkspace depășește limita de {MAX_WORKSPACE_PREVIEW_ENTRIES} intrări la {}.",
                path.display()
            ));
        }
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| "Proiecția ProjectWorkspace a depășit contorul de bytes.".to_string())?;
        if self.bytes > MAX_WORKSPACE_PREVIEW_BYTES {
            return Err(format!(
                "Proiecția ProjectWorkspace depășește limita de {MAX_WORKSPACE_PREVIEW_BYTES} bytes la {}.",
                path.display()
            ));
        }
        Ok(())
    }
}

pub(crate) fn persistent_project_workspace_session_root<R: Runtime>(
    app: &AppHandle<R>,
    zola_root: &Path,
    runtime_session_id: &str,
) -> Result<PathBuf, String> {
    if runtime_session_id.trim().is_empty() {
        return Err("Preview-ul persistent cere runtime session id nenul.".to_string());
    }
    let container = preview_project_dir(app, zola_root)?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    runtime_session_id.hash(&mut hasher);
    Ok(container
        .join("editor")
        .join(format!("session-{:x}", hasher.finish())))
}

pub(crate) fn source_browser_session_root<R: Runtime>(
    app: &AppHandle<R>,
    zola_root: &Path,
    runtime_session_id: &str,
) -> Result<PathBuf, String> {
    if runtime_session_id.trim().is_empty() {
        return Err("Source Browser cere runtime session id nenul.".to_string());
    }
    let container = preview_project_dir(app, zola_root)?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    runtime_session_id.hash(&mut hasher);
    Ok(container
        .join("source-browser")
        .join(format!("session-{:x}", hasher.finish())))
}

/// Rebuilds one immutable historical Git source tree in the application-owned
/// Preview cache. Every byte comes from a validated Git blob; live project
/// sources are never used as a destination.
pub(crate) fn materialize_version_source_tree<R: Runtime>(
    app: &AppHandle<R>,
    live_zola_root: &Path,
    runtime_session_id: &str,
    commit_oid: &str,
    files: &[(String, Vec<u8>)],
) -> Result<PathBuf, String> {
    if runtime_session_id.trim().is_empty()
        || !matches!(commit_oid.len(), 40 | 64)
        || !commit_oid.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("Materializarea versiunii cere session ID și commit OID valide.".to_string());
    }
    if files.len() > MAX_WORKSPACE_PREVIEW_ENTRIES {
        return Err(format!(
            "Versiunea depășește limita Preview de {MAX_WORKSPACE_PREVIEW_ENTRIES} fișiere."
        ));
    }
    let container = preview_project_dir(app, live_zola_root)?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    runtime_session_id.hash(&mut hasher);
    let session_root = container
        .join("versions")
        .join(format!("session-{:x}", hasher.finish()));
    let commit_root = session_root.join(format!("commit-{commit_oid}"));
    let source_root = commit_root.join("source");

    create_directory(app, &container, &container, "preview/root")?;
    create_directory(
        app,
        &container,
        &container.join("versions"),
        "preview/versions",
    )?;
    remove_entry(app, &container, &session_root)?;
    create_directory(app, &container, &session_root, "preview/versions/session")?;
    create_directory(app, &session_root, &commit_root, "preview/versions/commit")?;
    create_directory(
        app,
        &session_root,
        &source_root,
        "preview/versions/commit/source",
    )?;

    let mut budget = MaterializationBudget::default();
    for (relative_path, bytes) in files {
        let relative = Path::new(relative_path);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
            || relative
                .components()
                .any(|component| component.as_os_str() == ".git")
        {
            return Err(format!(
                "Materializarea versiunii a refuzat path-ul {relative_path:?}."
            ));
        }
        let target = source_root.join(relative);
        budget.reserve(&target, bytes.len() as u64)?;
        create_parent_directories(app, &source_root, &target)?;
        write_bytes(
            app,
            &source_root,
            &target,
            bytes,
            "Historical Git Preview blob",
        )?;
    }
    Ok(source_root)
}

/// Synchronizes one exact ProjectWorkspace lease into a stable derived source
/// tree. The complete disk baseline is copied only for a new session or after
/// AcceptedDisk advances; ordinary edits touch only changed overlay paths.
pub(crate) fn sync_persistent_project_workspace<R: Runtime>(
    app: &AppHandle<R>,
    zola_root: &Path,
    session_root: &Path,
    previous: Option<&PersistentProjectionManifest>,
    lease: &WorkspaceProjectionLease,
) -> Result<PersistentProjectionUpdate, String> {
    require_projection_root(zola_root, lease)?;
    require_disjoint_projection(lease)?;
    require_accepted_disk_baseline(lease)?;

    let projection_root = session_root.join("source");
    let active_theme = projected_active_theme(lease);
    let needs_baseline = previous.is_none()
        || previous.is_some_and(|manifest| {
            manifest.accepted_disk_generation != lease.accepted_disk.generation
                || manifest.active_theme != active_theme
        })
        || !projection_root.is_dir();

    let result = if needs_baseline {
        rebuild_persistent_projection(app, zola_root, session_root, &projection_root, lease)
    } else {
        apply_persistent_projection_delta(
            app,
            zola_root,
            &projection_root,
            previous.expect("baseline decision proves manifest exists"),
            lease,
        )
    };

    let (manifest, mut projected_paths) = result?;
    require_accepted_disk_baseline(lease)?;
    projected_paths.sort();
    projected_paths.dedup();

    Ok(PersistentProjectionUpdate {
        projection_root,
        manifest,
        projected_paths,
        baseline_rebuilt: needs_baseline,
    })
}

pub(crate) fn create_persistent_preview_artifact_root<R: Runtime>(
    app: &AppHandle<R>,
    session_root: &Path,
    preview_revision: &str,
) -> Result<PathBuf, String> {
    if preview_revision.trim().is_empty()
        || preview_revision
            .chars()
            .any(|character| !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_')))
    {
        return Err("Preview-ul persistent a refuzat o revizie derivată nesigură.".to_string());
    }
    let artifacts_root = session_root.join("artifacts");
    let artifact_root = artifacts_root.join(preview_revision);
    create_directory(app, session_root, session_root, "preview/editor-session")?;
    create_directory(
        app,
        session_root,
        &artifacts_root,
        "preview/editor-session/artifacts",
    )?;
    remove_entry(app, session_root, &artifact_root)?;
    create_directory(
        app,
        session_root,
        &artifact_root,
        &format!("preview/editor-session/artifacts/{preview_revision}"),
    )?;
    Ok(artifact_root)
}

pub(crate) fn create_source_browser_artifact_root<R: Runtime>(
    app: &AppHandle<R>,
    session_root: &Path,
    source_revision: &str,
) -> Result<PathBuf, String> {
    if source_revision.trim().is_empty()
        || source_revision
            .chars()
            .any(|character| !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_')))
    {
        return Err("Source Browser a refuzat o revizie derivată nesigură.".to_string());
    }
    let artifacts_root = session_root.join("artifacts");
    let artifact_root = artifacts_root.join(source_revision);
    create_directory(
        app,
        session_root,
        &artifacts_root,
        "preview/source-browser-session/artifacts",
    )?;
    remove_entry(app, session_root, &artifact_root)?;
    create_directory(
        app,
        session_root,
        &artifact_root,
        &format!("preview/source-browser-session/artifacts/{source_revision}"),
    )?;
    Ok(artifact_root)
}

/// Reuses the immutable assets of the last canonical generation without
/// escaping the Preview WriteAuthority. Planning is completed before the
/// first effect so a symlink or budget violation cannot leave a partial seed.
pub(crate) fn seed_persistent_preview_artifacts<R: Runtime>(
    app: &AppHandle<R>,
    session_root: &Path,
    source_root: &Path,
    target_root: &Path,
) -> Result<(), String> {
    if !target_root.starts_with(session_root.join("artifacts")) {
        return Err(format!(
            "Canvas Runtime a refuzat seed-ul în afara sesiunii: {}.",
            target_root.display()
        ));
    }
    let plan = plan_persistent_preview_artifact_seed(source_root, target_root)?;
    for target in plan.directories {
        create_directory(
            app,
            session_root,
            &target,
            &preview_label(session_root, &target),
        )?;
    }
    for (source, target) in plan.files {
        create_parent_directories(app, session_root, &target)?;
        copy_file(app, session_root, &source, &target)?;
    }
    Ok(())
}

#[derive(Debug, Default, Eq, PartialEq)]
struct PersistentPreviewArtifactSeedPlan {
    directories: Vec<PathBuf>,
    files: Vec<(PathBuf, PathBuf)>,
}

fn plan_persistent_preview_artifact_seed(
    source_root: &Path,
    target_root: &Path,
) -> Result<PersistentPreviewArtifactSeedPlan, String> {
    if source_root == target_root {
        return Ok(PersistentPreviewArtifactSeedPlan::default());
    }
    for (label, root) in [("precedent", source_root), ("candidat", target_root)] {
        let metadata = fs::symlink_metadata(root).map_err(|error| {
            format!(
                "Canvas Runtime nu a putut inspecta root-ul {label} {}: {error}",
                root.display()
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(format!(
                "Canvas Runtime cere un root {label} real: {}.",
                root.display()
            ));
        }
    }

    let mut plan = PersistentPreviewArtifactSeedPlan::default();
    let mut total_bytes = 0u64;
    for entry in WalkDir::new(source_root)
        .follow_links(false)
        .sort_by_file_name()
    {
        let entry = entry.map_err(|error| {
            format!("Canvas Runtime nu a putut parcurge artifactele precedente: {error}")
        })?;
        if entry.file_type().is_symlink() {
            return Err(format!(
                "Canvas Runtime a refuzat symlink-ul din artifactele precedente: {}.",
                entry.path().display()
            ));
        }
        let relative = entry.path().strip_prefix(source_root).map_err(|_| {
            "Canvas Runtime a găsit un artifact precedent în afara root-ului.".to_string()
        })?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let target = target_root.join(relative);
        if entry.file_type().is_dir() {
            plan.directories.push(target);
            continue;
        }
        if !entry.file_type().is_file() {
            return Err(format!(
                "Canvas Runtime a refuzat artifactul precedent non-regular: {}.",
                entry.path().display()
            ));
        }
        if plan.files.len() >= MAX_SEEDED_ARTIFACT_FILES {
            return Err(format!(
                "Canvas Runtime refuză peste {MAX_SEEDED_ARTIFACT_FILES} artifacte precedente."
            ));
        }
        let size = entry
            .metadata()
            .map_err(|error| {
                format!(
                    "Canvas Runtime nu a putut măsura {}: {error}",
                    entry.path().display()
                )
            })?
            .len();
        total_bytes = total_bytes.checked_add(size).ok_or_else(|| {
            "Canvas Runtime a depășit contorul artifactelor precedente.".to_string()
        })?;
        if total_bytes > MAX_SEEDED_ARTIFACT_BYTES {
            return Err(format!(
                "Canvas Runtime refuză peste {MAX_SEEDED_ARTIFACT_BYTES} bytes de artifacte precedente."
            ));
        }
        plan.files.push((entry.path().to_path_buf(), target));
    }
    Ok(plan)
}

pub(crate) fn remove_persistent_preview_artifact_root<R: Runtime>(
    app: &AppHandle<R>,
    session_root: &Path,
    artifact_root: &Path,
) -> Result<(), String> {
    if artifact_root == session_root || !artifact_root.starts_with(session_root.join("artifacts")) {
        return Err(format!(
            "Cleanup-ul Preview persistent a refuzat artifactul din afara sesiunii: {}.",
            artifact_root.display()
        ));
    }
    remove_entry(app, session_root, artifact_root)
}

pub(crate) fn remove_source_browser_artifact_root<R: Runtime>(
    app: &AppHandle<R>,
    session_root: &Path,
    artifact_root: &Path,
) -> Result<(), String> {
    if artifact_root == session_root || !artifact_root.starts_with(session_root.join("artifacts")) {
        return Err(format!(
            "Cleanup-ul Source Browser a refuzat artifactul din afara sesiunii: {}.",
            artifact_root.display()
        ));
    }
    remove_entry(app, session_root, artifact_root)
}

pub(crate) fn remove_persistent_preview_session<R: Runtime>(
    app: &AppHandle<R>,
    zola_root: &Path,
    session_root: &Path,
) -> Result<(), String> {
    let container = preview_project_dir(app, zola_root)?;
    if session_root == container || !session_root.starts_with(container.join("editor")) {
        return Err(format!(
            "Cleanup-ul Preview persistent a refuzat sesiunea din afara containerului: {}.",
            session_root.display()
        ));
    }
    remove_entry(app, &container, session_root)
}

pub(crate) fn remove_source_browser_session<R: Runtime>(
    app: &AppHandle<R>,
    zola_root: &Path,
    session_root: &Path,
) -> Result<(), String> {
    let container = preview_project_dir(app, zola_root)?;
    if session_root == container || !session_root.starts_with(container.join("source-browser")) {
        return Err(format!(
            "Cleanup-ul Source Browser a refuzat sesiunea din afara containerului: {}.",
            session_root.display()
        ));
    }
    remove_entry(app, &container, session_root)
}

pub(crate) fn reset_source_browser_cache<R: Runtime>(
    app: &AppHandle<R>,
    zola_root: &Path,
) -> Result<(), String> {
    let container = preview_project_dir(app, zola_root)?;
    remove_entry(app, &container, &container.join("source-browser"))
}

/// Establishes the Source Browser namespace one directory at a time. Directory
/// v2 deliberately refuses implicit parent creation, so the project container,
/// browser namespace and runtime-session leaf must each be authorized while
/// their direct parent already exists.
pub(crate) fn prepare_source_browser_session<R: Runtime>(
    app: &AppHandle<R>,
    zola_root: &Path,
    session_root: &Path,
) -> Result<(), String> {
    let container = preview_project_dir(app, zola_root)?;
    let browser_root = container.join("source-browser");
    if session_root.parent() != Some(browser_root.as_path()) {
        return Err(format!(
            "Inițializarea Source Browser a refuzat sesiunea din afara namespace-ului curent: {}.",
            session_root.display()
        ));
    }
    create_directory(app, &container, &container, "preview/root")?;
    create_directory(app, &container, &browser_root, "preview/source-browser")?;
    create_directory(
        app,
        &container,
        session_root,
        "preview/source-browser-session",
    )
}

pub(crate) fn reset_persistent_preview_editor_cache<R: Runtime>(
    app: &AppHandle<R>,
    zola_root: &Path,
) -> Result<(), String> {
    let container = preview_project_dir(app, zola_root)?;
    remove_entry(app, &container, &container.join("editor"))
}

fn materialize_generation_contents<R: Runtime>(
    app: &AppHandle<R>,
    zola_root: &Path,
    generation_root: &Path,
    preview_revision: Option<&str>,
    lease: &WorkspaceProjectionLease,
) -> Result<(), String> {
    let active_theme = projected_active_theme(lease);
    let mut budget = MaterializationBudget::default();
    copy_zola_sources(
        app,
        zola_root,
        generation_root,
        active_theme.as_deref(),
        lease,
        &mut budget,
    )?;

    let mut deleted = lease.deleted_sources.iter().collect::<Vec<_>>();
    deleted.sort();
    for project_relative in deleted {
        let Some(zola_relative) = zola_relative_projection_path(project_relative)? else {
            continue;
        };
        let target = generation_root.join(&zola_relative);
        remove_entry(app, generation_root, &target)?;
    }

    let mut source_texts = lease.source_texts.iter().collect::<Vec<_>>();
    source_texts.sort_by(|left, right| left.0.cmp(right.0));
    for (project_relative, source) in source_texts {
        let Some(zola_relative) = zola_relative_projection_path(project_relative)? else {
            continue;
        };
        let target = generation_root.join(&zola_relative);
        budget.reserve(&target, source.len() as u64)?;
        create_parent_directories(app, generation_root, &target)?;
        write_text(
            app,
            generation_root,
            &target,
            source,
            "ProjectWorkspace Preview overlay",
        )?;
    }

    let mut resource_bytes = lease.resource_bytes.iter().collect::<Vec<_>>();
    resource_bytes.sort_by(|left, right| left.0.cmp(right.0));
    for (project_relative, bytes) in resource_bytes {
        let Some(zola_relative) = zola_relative_projection_path(project_relative)? else {
            continue;
        };
        let target = generation_root.join(&zola_relative);
        budget.reserve(&target, bytes.len() as u64)?;
        create_parent_directories(app, generation_root, &target)?;
        write_bytes(
            app,
            generation_root,
            &target,
            bytes,
            "ProjectWorkspace Preview binary overlay",
        )?;
    }

    preprocess_projected_templates(app, generation_root, preview_revision)
}

fn rebuild_persistent_projection<R: Runtime>(
    app: &AppHandle<R>,
    zola_root: &Path,
    session_root: &Path,
    projection_root: &Path,
    lease: &WorkspaceProjectionLease,
) -> Result<(PersistentProjectionManifest, Vec<String>), String> {
    let container = preview_project_dir(app, zola_root)?;
    create_directory(app, &container, &container, "preview/root")?;
    create_directory(app, &container, &container.join("editor"), "preview/editor")?;
    create_directory(app, &container, session_root, "preview/editor-session")?;
    // The published generation may still serve assets from `artifacts/` while
    // a new accepted baseline is prepared. Rebuild only the derived source
    // projection; publication/retirement owns artifact lifetime separately.
    remove_entry(app, session_root, projection_root)?;
    create_directory(
        app,
        session_root,
        projection_root,
        "preview/editor-session/source",
    )?;

    materialize_generation_contents(app, zola_root, projection_root, None, lease)?;
    Ok((projection_manifest(lease), projected_overlay_paths(lease)))
}

fn apply_persistent_projection_delta<R: Runtime>(
    app: &AppHandle<R>,
    zola_root: &Path,
    projection_root: &Path,
    previous: &PersistentProjectionManifest,
    lease: &WorkspaceProjectionLease,
) -> Result<(PersistentProjectionManifest, Vec<String>), String> {
    let next = projection_manifest(lease);
    let mut projected_paths = BTreeSet::new();
    let mut changed_templates = BTreeSet::new();

    for path in previous
        .text_hashes
        .keys()
        .chain(previous.resource_hashes.keys())
    {
        if next.text_hashes.contains_key(path)
            || next.resource_hashes.contains_key(path)
            || lease.deleted_sources.contains(path)
        {
            continue;
        }
        restore_projection_path_from_disk(
            app,
            zola_root,
            projection_root,
            path,
            &mut changed_templates,
        )?;
        projected_paths.insert(path.clone());
    }

    for path in previous.deleted_sources.difference(&next.deleted_sources) {
        restore_projection_path_from_disk(
            app,
            zola_root,
            projection_root,
            path,
            &mut changed_templates,
        )?;
        projected_paths.insert(path.clone());
    }

    let mut deleted = lease.deleted_sources.iter().collect::<Vec<_>>();
    deleted.sort();
    for project_relative in deleted {
        if previous.deleted_sources.contains(project_relative) {
            continue;
        }
        let Some(relative) = zola_relative_projection_path(project_relative)? else {
            continue;
        };
        remove_entry(app, projection_root, &projection_root.join(relative))?;
        projected_paths.insert(project_relative.clone());
    }

    let mut source_texts = lease.source_texts.iter().collect::<Vec<_>>();
    source_texts.sort_by(|left, right| left.0.cmp(right.0));
    for (project_relative, source) in source_texts {
        if previous.text_hashes.get(project_relative) == next.text_hashes.get(project_relative) {
            continue;
        }
        let Some(relative) = zola_relative_projection_path(project_relative)? else {
            continue;
        };
        let target = projection_root.join(&relative);
        create_parent_directories(app, projection_root, &target)?;
        write_text(
            app,
            projection_root,
            &target,
            source,
            "ProjectWorkspace persistent Preview text delta",
        )?;
        if is_template_relative_path(&relative.to_string_lossy())
            && target.extension().and_then(|extension| extension.to_str()) == Some("html")
        {
            changed_templates.insert(target.clone());
        }
        projected_paths.insert(project_relative.clone());
    }

    let mut resources = lease.resource_bytes.iter().collect::<Vec<_>>();
    resources.sort_by(|left, right| left.0.cmp(right.0));
    for (project_relative, bytes) in resources {
        if previous.resource_hashes.get(project_relative)
            == next.resource_hashes.get(project_relative)
        {
            continue;
        }
        let Some(relative) = zola_relative_projection_path(project_relative)? else {
            continue;
        };
        let target = projection_root.join(relative);
        create_parent_directories(app, projection_root, &target)?;
        write_bytes(
            app,
            projection_root,
            &target,
            bytes,
            "ProjectWorkspace persistent Preview binary delta",
        )?;
        projected_paths.insert(project_relative.clone());
    }

    preprocess_selected_templates(
        app,
        projection_root,
        changed_templates.into_iter().collect(),
        None,
    )?;

    Ok((next, projected_paths.into_iter().collect()))
}

fn restore_projection_path_from_disk<R: Runtime>(
    app: &AppHandle<R>,
    zola_root: &Path,
    projection_root: &Path,
    project_relative: &str,
    changed_templates: &mut BTreeSet<PathBuf>,
) -> Result<(), String> {
    let Some(relative) = zola_relative_projection_path(project_relative)? else {
        return Ok(());
    };
    let source = zola_root.join(&relative);
    let target = projection_root.join(&relative);
    match fs::symlink_metadata(&source) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(format!(
            "Preview-ul persistent refuză restaurarea symlink-ului {}.",
            source.display()
        )),
        Ok(metadata) if metadata.is_file() => {
            create_parent_directories(app, projection_root, &target)?;
            copy_file(app, projection_root, &source, &target)?;
            if is_template_relative_path(&relative.to_string_lossy())
                && target.extension().and_then(|extension| extension.to_str()) == Some("html")
            {
                changed_templates.insert(target.clone());
            }
            Ok(())
        }
        Ok(_) => Err(format!(
            "Preview-ul persistent nu poate restaura tipul de sursă {}.",
            source.display()
        )),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            remove_entry(app, projection_root, &target)
        }
        Err(error) => Err(format!(
            "Preview-ul persistent nu poate inspecta {}: {error}",
            source.display()
        )),
    }
}

fn projection_manifest(lease: &WorkspaceProjectionLease) -> PersistentProjectionManifest {
    PersistentProjectionManifest {
        accepted_disk_generation: lease.accepted_disk.generation,
        active_theme: projected_active_theme(lease),
        text_hashes: lease
            .source_texts
            .iter()
            .filter(|(path, _)| path.starts_with("sursa/"))
            .map(|(path, source)| (path.clone(), hash_bytes(source.as_bytes())))
            .collect(),
        resource_hashes: lease
            .resource_bytes
            .iter()
            .filter(|(path, _)| path.starts_with("sursa/"))
            .map(|(path, bytes)| (path.clone(), hash_bytes(bytes)))
            .collect(),
        deleted_sources: lease
            .deleted_sources
            .iter()
            .filter(|path| path.starts_with("sursa/"))
            .cloned()
            .collect(),
    }
}

fn projected_overlay_paths(lease: &WorkspaceProjectionLease) -> Vec<String> {
    lease
        .changed_paths
        .iter()
        .chain(lease.deleted_sources.iter())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn hash_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn require_projection_root(
    zola_root: &Path,
    lease: &WorkspaceProjectionLease,
) -> Result<(), String> {
    let expected = Path::new(&lease.project_root).join("sursa");
    let expected = expected.canonicalize().unwrap_or(expected);
    let actual = zola_root
        .canonicalize()
        .unwrap_or_else(|_| zola_root.to_path_buf());
    if expected != actual {
        return Err(format!(
            "Proiecția Preview a refuzat un lease ProjectWorkspace pentru alt root: așteptat {}, primit {}.",
            expected.display(),
            actual.display()
        ));
    }
    let metadata = fs::symlink_metadata(&actual).map_err(|error| {
        format!(
            "Proiecția Preview nu poate inspecta rădăcina Zola {}: {error}",
            actual.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("Proiecția Preview cere o rădăcină Zola reală.".to_string());
    }
    Ok(())
}

fn require_disjoint_projection(lease: &WorkspaceProjectionLease) -> Result<(), String> {
    if let Some(path) = lease
        .source_texts
        .keys()
        .find(|path| lease.deleted_sources.contains(*path))
    {
        return Err(format!(
            "ProjectWorkspace a produs o proiecție ambiguă pentru {path}: draft și delete simultan."
        ));
    }
    if let Some(path) = lease.resource_bytes.keys().find(|path| {
        lease.deleted_sources.contains(*path) || lease.source_texts.contains_key(*path)
    }) {
        return Err(format!(
            "ProjectWorkspace a produs o proiecție ambiguă pentru {path}: resursă binară suprapusă peste text sau delete."
        ));
    }
    Ok(())
}

fn require_accepted_disk_baseline(lease: &WorkspaceProjectionLease) -> Result<(), String> {
    lease.accepted_disk.require_live_complete(
        &lease.runtime_session_id,
        &lease.project_root,
        Path::new(&lease.project_root),
    )
}

fn projected_active_theme(lease: &WorkspaceProjectionLease) -> Option<String> {
    ["sursa/zola.toml", "sursa/config.toml"]
        .iter()
        .find_map(|relative| {
            if lease.deleted_sources.contains(*relative) {
                return None;
            }
            lease.source_texts.get(*relative).cloned()
        })
        .and_then(|source| active_theme_from_source(&source))
}

fn copy_zola_sources<R: Runtime>(
    app: &AppHandle<R>,
    zola_root: &Path,
    generation_root: &Path,
    active_theme: Option<&str>,
    lease: &WorkspaceProjectionLease,
    budget: &mut MaterializationBudget,
) -> Result<(), String> {
    for entry in sorted_directory_entries(zola_root)? {
        let name = entry.file_name();
        let name_text = name.to_string_lossy();
        if SKIPPED_SOURCE_DIRECTORIES
            .iter()
            .any(|skip| name_text.eq_ignore_ascii_case(skip))
        {
            continue;
        }
        let source = entry.path();
        let target = generation_root.join(&name);
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "Proiecția Preview nu poate inspecta {}: {error}",
                source.display()
            )
        })?;
        if file_type.is_symlink() {
            return Err(format!(
                "Proiecția Preview refuză symlink-ul sursă {}.",
                source.display()
            ));
        }
        if file_type.is_dir() && name_text == "themes" {
            let Some(theme) = active_theme else {
                continue;
            };
            let theme_source = source.join(theme);
            match fs::symlink_metadata(&theme_source) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(format!(
                        "Proiecția Preview refuză tema symlink {}.",
                        theme_source.display()
                    ));
                }
                Ok(metadata) if metadata.is_dir() => {
                    create_directory(app, generation_root, &target, "preview/themes")?;
                    copy_entry_recursive(
                        app,
                        zola_root,
                        &theme_source,
                        &target.join(theme),
                        generation_root,
                        lease,
                        budget,
                    )?;
                }
                Ok(_) => {
                    return Err(format!(
                        "Proiecția Preview refuză tema non-director {}.",
                        theme_source.display()
                    ));
                }
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(format!("Nu am putut inspecta tema {theme}: {error}")),
            }
            continue;
        }
        copy_entry_recursive(
            app,
            zola_root,
            &source,
            &target,
            generation_root,
            lease,
            budget,
        )?;
    }
    Ok(())
}

fn copy_entry_recursive<R: Runtime>(
    app: &AppHandle<R>,
    zola_root: &Path,
    source: &Path,
    target: &Path,
    generation_root: &Path,
    lease: &WorkspaceProjectionLease,
    budget: &mut MaterializationBudget,
) -> Result<(), String> {
    let metadata = fs::symlink_metadata(source)
        .map_err(|error| format!("Nu am putut inspecta {}: {error}", source.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "Proiecția Preview refuză symlink-ul sursă {}.",
            source.display()
        ));
    }
    budget.reserve(
        source,
        metadata.is_file().then_some(metadata.len()).unwrap_or(0),
    )?;
    if metadata.is_dir() {
        create_directory(
            app,
            generation_root,
            target,
            &preview_label(generation_root, target),
        )?;
        for entry in sorted_directory_entries(source)? {
            let name = entry.file_name();
            let name_text = name.to_string_lossy();
            if SKIPPED_SOURCE_DIRECTORIES
                .iter()
                .any(|skip| name_text.eq_ignore_ascii_case(skip))
            {
                continue;
            }
            copy_entry_recursive(
                app,
                zola_root,
                &entry.path(),
                &target.join(name),
                generation_root,
                lease,
                budget,
            )?;
        }
        return Ok(());
    }
    if !metadata.is_file() {
        return Err(format!(
            "Proiecția Preview refuză tipul de sursă {}.",
            source.display()
        ));
    }
    if workspace_owns_source_file(zola_root, source, lease)? {
        return Ok(());
    }
    create_parent_directories(app, generation_root, target)?;
    copy_file(app, generation_root, source, target)
}

fn workspace_owns_source_file(
    zola_root: &Path,
    source: &Path,
    lease: &WorkspaceProjectionLease,
) -> Result<bool, String> {
    let relative = source.strip_prefix(zola_root).map_err(|_| {
        format!(
            "Proiecția Preview a primit o sursă în afara root-ului Zola: {}.",
            source.display()
        )
    })?;
    let project_relative = format!("sursa/{}", relative.to_string_lossy().replace('\\', "/"));
    Ok(lease.source_texts.contains_key(&project_relative)
        || lease.resource_bytes.contains_key(&project_relative)
        || lease.deleted_sources.contains(&project_relative))
}

fn preprocess_projected_templates<R: Runtime>(
    app: &AppHandle<R>,
    generation_root: &Path,
    preview_revision: Option<&str>,
) -> Result<(), String> {
    let mut paths = Vec::new();
    collect_template_paths(
        &generation_root.join("templates"),
        generation_root,
        &mut paths,
    )?;
    if let Some(theme) = crate::zola_theme::read_active_theme(generation_root) {
        collect_template_paths(
            &generation_root.join("themes").join(theme).join("templates"),
            generation_root,
            &mut paths,
        )?;
    }
    paths.sort();
    preprocess_selected_templates(app, generation_root, paths, preview_revision)
}

fn preprocess_selected_templates<R: Runtime>(
    app: &AppHandle<R>,
    generation_root: &Path,
    paths: Vec<PathBuf>,
    preview_revision: Option<&str>,
) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let source_ids = SourceIdIndex::for_zola_root(generation_root)?;
    for path in paths {
        let relative = path
            .strip_prefix(generation_root)
            .map_err(|_| "Template-ul proiectat a ieșit din generația Preview.".to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let source = fs::read_to_string(&path).map_err(|error| {
            format!("Nu am putut citi template-ul proiectat {relative}: {error}")
        })?;
        let processed = preprocess_template_with_revision(
            &source,
            &relative,
            Some(&source_ids),
            preview_revision,
        );
        write_text(
            app,
            generation_root,
            &path,
            &processed,
            "ProjectWorkspace Preview template annotation",
        )?;
    }
    Ok(())
}

fn collect_template_paths(
    root: &Path,
    generation_root: &Path,
    output: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let metadata = match fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("Nu am putut inspecta {}: {error}", root.display())),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "Rădăcina de template proiectată nu este director real: {}.",
            root.display()
        ));
    }
    for entry in sorted_directory_entries(root)? {
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "Nu am putut inspecta template-ul {}: {error}",
                path.display()
            )
        })?;
        if file_type.is_symlink() {
            return Err(format!(
                "Proiecția Preview refuză symlink-ul template {}.",
                path.display()
            ));
        }
        if file_type.is_dir() {
            collect_template_paths(&path, generation_root, output)?;
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(generation_root)
                .map_err(|_| "Template-ul a ieșit din generația Preview.".to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            if is_template_relative_path(&relative)
                && path.extension().and_then(|extension| extension.to_str()) == Some("html")
            {
                output.push(path);
            }
        }
    }
    Ok(())
}

fn zola_relative_projection_path(project_relative: &str) -> Result<Option<PathBuf>, String> {
    let normalized = project_relative.trim().replace('\\', "/");
    let Some(relative) = normalized.strip_prefix("sursa/") else {
        return Ok(None);
    };
    if relative.is_empty() {
        return Err("Proiecția Preview refuză root-ul `sursa` ca document.".to_string());
    }
    let path = Path::new(relative);
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(format!(
                "Proiecția Preview refuză path-ul nesigur {project_relative}."
            ));
        }
    }
    Ok(Some(path.to_path_buf()))
}

fn sorted_directory_entries(root: &Path) -> Result<Vec<fs::DirEntry>, String> {
    let mut entries = fs::read_dir(root)
        .map_err(|error| format!("Nu am putut citi {}: {error}", root.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Nu am putut citi o intrare din {}: {error}", root.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    Ok(entries)
}

fn create_parent_directories<R: Runtime>(
    app: &AppHandle<R>,
    boundary: &Path,
    target: &Path,
) -> Result<(), String> {
    let parent = target
        .parent()
        .ok_or_else(|| "Destinația Preview nu are director părinte.".to_string())?;
    let relative = parent.strip_prefix(boundary).map_err(|_| {
        format!(
            "Destinația Preview {} a ieșit din generație.",
            target.display()
        )
    })?;
    let mut current = boundary.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err("Destinația Preview conține un ancestor nesigur.".to_string());
        };
        current.push(name);
        create_directory(app, boundary, &current, &preview_label(boundary, &current))?;
    }
    Ok(())
}

fn remove_entry<R: Runtime>(
    app: &AppHandle<R>,
    boundary: &Path,
    target: &Path,
) -> Result<(), String> {
    let metadata = match fs::symlink_metadata(target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "Nu am putut inspecta {}: {error}",
                target.display()
            ))
        }
    };
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "Proiecția Preview refuză cleanup-ul symlink-ului {}.",
            target.display()
        ));
    }
    if metadata.is_dir() {
        remove_tree(app, boundary, target)
    } else if metadata.is_file() {
        remove_file(app, boundary, target)
    } else {
        Err(format!(
            "Proiecția Preview refuză tipul destinației {}.",
            target.display()
        ))
    }
}

fn create_directory<R: Runtime>(
    app: &AppHandle<R>,
    boundary: &Path,
    target: &Path,
    label: &str,
) -> Result<(), String> {
    let intent = WriteIntent::new(
        WriteCategory::PreviewWorkspaceWrite,
        WriteOwner::Preview,
        WriteOperationKind::CreateDirectory,
        WriteTarget::new(target.to_path_buf(), boundary.to_path_buf(), label),
        WritePolicy::preview_workspace_lifecycle(),
        "ProjectWorkspace Preview directory",
    );
    WriteAuthority::new(app)
        .create_directory_all(intent)
        .map_err(|error| error.into_terminal_diagnostic())?;
    Ok(())
}

fn write_text<R: Runtime>(
    app: &AppHandle<R>,
    boundary: &Path,
    target: &Path,
    contents: &str,
    description: &str,
) -> Result<(), String> {
    let intent = WriteIntent::new(
        WriteCategory::PreviewWorkspaceWrite,
        WriteOwner::Preview,
        WriteOperationKind::WriteText,
        WriteTarget::new(
            target.to_path_buf(),
            boundary.to_path_buf(),
            preview_label(boundary, target),
        ),
        WritePolicy::preview_workspace_atomic(),
        description,
    );
    WriteAuthority::new(app)
        .write_text(intent, contents)
        .map_err(|error| error.into_terminal_diagnostic())?;
    Ok(())
}

fn write_bytes<R: Runtime>(
    app: &AppHandle<R>,
    boundary: &Path,
    target: &Path,
    contents: &[u8],
    description: &str,
) -> Result<(), String> {
    let intent = WriteIntent::new(
        WriteCategory::PreviewWorkspaceWrite,
        WriteOwner::Preview,
        WriteOperationKind::WriteBytes,
        WriteTarget::new(
            target.to_path_buf(),
            boundary.to_path_buf(),
            preview_label(boundary, target),
        ),
        WritePolicy::preview_workspace_atomic(),
        description,
    );
    WriteAuthority::new(app)
        .write_bytes(intent, contents)
        .map_err(|error| error.into_terminal_diagnostic())?;
    Ok(())
}

fn copy_file<R: Runtime>(
    app: &AppHandle<R>,
    boundary: &Path,
    source: &Path,
    target: &Path,
) -> Result<(), String> {
    let intent = WriteIntent::new(
        WriteCategory::PreviewWorkspaceWrite,
        WriteOwner::Preview,
        WriteOperationKind::Copy,
        WriteTarget::new(
            target.to_path_buf(),
            boundary.to_path_buf(),
            preview_label(boundary, target),
        ),
        WritePolicy::preview_workspace_lifecycle(),
        "ProjectWorkspace Preview source copy",
    );
    WriteAuthority::new(app)
        .copy_file(intent, source)
        .map_err(|error| error.into_terminal_diagnostic())?;
    Ok(())
}

fn remove_file<R: Runtime>(
    app: &AppHandle<R>,
    boundary: &Path,
    target: &Path,
) -> Result<(), String> {
    let intent = WriteIntent::new(
        WriteCategory::PreviewWorkspaceWrite,
        WriteOwner::Preview,
        WriteOperationKind::RemoveFile,
        WriteTarget::new(
            target.to_path_buf(),
            boundary.to_path_buf(),
            preview_label(boundary, target),
        ),
        WritePolicy::preview_workspace_lifecycle(),
        "ProjectWorkspace Preview delete overlay",
    );
    WriteAuthority::new(app)
        .remove_file_if_exists(intent)
        .map_err(|error| error.into_terminal_diagnostic())?;
    Ok(())
}

fn remove_tree<R: Runtime>(
    app: &AppHandle<R>,
    boundary: &Path,
    target: &Path,
) -> Result<(), String> {
    let intent = WriteIntent::new(
        WriteCategory::PreviewWorkspaceWrite,
        WriteOwner::Preview,
        WriteOperationKind::RemoveDirectoryTree,
        WriteTarget::new(
            target.to_path_buf(),
            boundary.to_path_buf(),
            preview_label(boundary, target),
        ),
        WritePolicy::preview_workspace_lifecycle(),
        "ProjectWorkspace Preview generation cleanup",
    );
    WriteAuthority::new(app)
        .remove_directory_tree_if_exists(intent)
        .map_err(|error| error.into_terminal_diagnostic())?;
    Ok(())
}

fn preview_label(boundary: &Path, target: &Path) -> String {
    target
        .strip_prefix(boundary)
        .ok()
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .filter(|relative| !relative.is_empty())
        .map(|relative| format!("preview/{relative}"))
        .unwrap_or_else(|| "preview/root".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn artifact_seed_fixture(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "pana-preview-artifact-seed-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    #[test]
    fn projection_paths_accept_only_zola_descendants() {
        assert_eq!(
            zola_relative_projection_path("sursa/templates/index.html").unwrap(),
            Some(PathBuf::from("templates/index.html"))
        );
        assert_eq!(zola_relative_projection_path("README.md").unwrap(), None);
        assert!(zola_relative_projection_path("sursa/templates/../outside.html").is_err());
    }

    #[test]
    fn draft_and_delete_for_same_path_are_rejected() {
        let mut source_texts = std::collections::HashMap::new();
        source_texts.insert(
            "sursa/templates/index.html".to_string(),
            "draft".to_string(),
        );
        let accepted_disk = crate::project::AcceptedProjectDiskManifest::new(
            "session",
            "/tmp/project",
            crate::project::ProjectDiskManifest {
                root: "/tmp/project".to_string(),
                files: Vec::new(),
                truncated: false,
                max_files: 1000,
            },
        )
        .unwrap();
        let lease = WorkspaceProjectionLease {
            project_root: "/tmp/project".to_string(),
            runtime_session_id: "session".to_string(),
            revision: 1,
            workspace_transaction_id: Some("projection-test-1".to_string()),
            source_texts,
            resource_bytes: std::collections::HashMap::new(),
            deleted_sources: std::collections::HashSet::from([
                "sursa/templates/index.html".to_string()
            ]),
            changed_paths: std::collections::HashSet::new(),
            accepted_disk,
        };
        assert!(require_disjoint_projection(&lease).is_err());
    }

    #[test]
    fn artifact_seed_is_fully_planned_before_write_authority_effects() {
        let fixture = artifact_seed_fixture("plan");
        let source = fixture.join("active");
        let target = fixture.join("staged");
        fs::create_dir_all(source.join("nested")).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(source.join("site.css"), b"body{color:#123}").unwrap();
        fs::write(source.join("nested/font.woff2"), b"font-bytes").unwrap();

        let plan = plan_persistent_preview_artifact_seed(&source, &target).unwrap();
        assert_eq!(plan.directories, vec![target.join("nested")]);
        assert_eq!(
            plan.files,
            vec![
                (
                    source.join("nested/font.woff2"),
                    target.join("nested/font.woff2")
                ),
                (source.join("site.css"), target.join("site.css")),
            ]
        );
        assert!(!target.join("site.css").exists());
        fs::remove_dir_all(fixture).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn artifact_seed_plan_refuses_symlink_before_any_effect() {
        use std::os::unix::fs::symlink;

        let fixture = artifact_seed_fixture("symlink");
        let source = fixture.join("active");
        let target = fixture.join("staged");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(fixture.join("outside.css"), b"outside").unwrap();
        symlink(fixture.join("outside.css"), source.join("site.css")).unwrap();

        let error = plan_persistent_preview_artifact_seed(&source, &target).unwrap_err();
        assert!(error.contains("symlink"));
        assert!(!target.join("site.css").exists());
        fs::remove_dir_all(fixture).unwrap();
    }
}
