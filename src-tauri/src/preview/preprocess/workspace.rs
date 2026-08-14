use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    hash::{Hash, Hasher},
    io::ErrorKind,
    path::{Component, Path, PathBuf},
};

use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager, Runtime};

use super::{
    annotate::{is_template_relative_path, preprocess_template_with_revision, SourceIdIndex},
    project::preview_project_dir,
};
use crate::{
    kernel::{
        project_workspace::WorkspaceProjectionSnapshot,
        write_authority::{
            ActiveProjectReadLease, PendingProjectAuthority, PreviewProjectionGeneration,
            PreviewProjectionPublication, PreviewProjectionPublicationStats, WriteAuthority,
            WriteAuthorityRuntime, WriteCategory, WriteIntent, WriteOperationKind, WriteOwner,
            WritePolicy, WriteTarget,
        },
    },
    source_graph::model::SourceGraph,
    zola_theme::active_theme_from_source,
};

const MAX_WORKSPACE_PREVIEW_ENTRIES: usize = 16_384;
const MAX_WORKSPACE_PREVIEW_BYTES: u64 = 512 * 1024 * 1024;
const SKIPPED_SOURCE_DIRECTORIES: &[&str] = &[
    ".git",
    ".svelte-kit",
    ".panastudio",
    ".panastudio_preview",
    "build",
    "dist",
    "node_modules",
    "target",
    "public",
    "export",
];
const SENSITIVE_SOURCE_FILES: &[&str] = &[".env"];

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
    pub publication_stats: PreviewProjectionPublicationStats,
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

    fn remaining_bytes(&self) -> u64 {
        MAX_WORKSPACE_PREVIEW_BYTES.saturating_sub(self.bytes)
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

/// Synchronizes one exact ProjectWorkspace projection into a stable derived source
/// tree. The complete disk baseline is copied only for a new session or after
/// AcceptedDisk advances; ordinary edits touch only changed overlay paths.
pub(crate) fn sync_persistent_project_workspace<R: Runtime>(
    app: &AppHandle<R>,
    zola_root: &Path,
    session_root: &Path,
    previous: Option<&PersistentProjectionManifest>,
    projection: &WorkspaceProjectionSnapshot,
    source_graph: &SourceGraph,
    pending_project_authority: Option<&PendingProjectAuthority>,
) -> Result<(PersistentProjectionUpdate, PreviewProjectionPublication), String> {
    let require_live_disk = pending_project_authority.is_none();
    require_projection_root(zola_root, projection)?;
    require_disjoint_projection(projection)?;
    require_accepted_disk_baseline(projection, require_live_disk)?;

    let projection_root = session_root.join("source");
    let active_theme = projected_active_theme(projection);
    let needs_baseline = previous.is_none()
        || previous.is_some_and(|manifest| {
            manifest.accepted_disk_generation != projection.accepted_disk.generation
                || manifest.active_theme != active_theme
        })
        || !projection_root.is_dir();
    let manifest = projection_manifest(projection);
    let mut projected_paths = previous
        .map(|previous| projection_changed_paths(previous, &manifest))
        .unwrap_or_else(|| projected_overlay_paths(projection));
    projected_paths.sort();
    projected_paths.dedup();

    let container = preview_project_dir(app, zola_root)?;
    create_directory(app, &container, &container, "preview/root")?;
    create_directory(app, &container, &container.join("editor"), "preview/editor")?;
    create_directory(app, &container, session_root, "preview/editor-session")?;

    let authority = app.state::<WriteAuthorityRuntime>();
    let project_read = if let Some(pending) = pending_project_authority {
        pending.require_project_binding(zola_root, &projection.runtime_session_id)?;
        ProjectReadAccess::Pending(pending)
    } else {
        ProjectReadAccess::Active(authority.acquire_active_project_read_lease_for_session(
            zola_root,
            &projection.runtime_session_id,
        )?)
    };
    let mut generation =
        PreviewProjectionGeneration::begin(authority.inner(), session_root, &projection_root)?;
    let materialization = if needs_baseline {
        materialize_generation_contents(
            zola_root,
            &mut generation,
            projection,
            source_graph,
            &project_read,
        )
    } else {
        (|| {
            let excluded = projected_paths
                .iter()
                .map(|path| zola_relative_projection_path(path))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .collect::<BTreeSet<_>>();
            let (reused_entries, reused_bytes) = generation.seed_from_published(
                &excluded,
                MAX_WORKSPACE_PREVIEW_ENTRIES,
                MAX_WORKSPACE_PREVIEW_BYTES,
            )?;
            materialize_generation_delta(
                &mut generation,
                projection,
                source_graph,
                &project_read,
                &projected_paths,
                reused_entries,
                reused_bytes,
            )
        })()
    };
    if let Err(error) = materialization {
        let cleanup = generation.discard();
        return Err(match cleanup {
            Ok(()) => error,
            Err(cleanup_error) => {
                format!("{error} Cleanup-ul generației Preview a eșuat: {cleanup_error}")
            }
        });
    }
    require_accepted_disk_baseline(projection, require_live_disk)?;
    let publication = generation.publish()?;
    let publication_stats = publication.stats;

    require_accepted_disk_baseline(projection, require_live_disk)?;

    Ok((
        PersistentProjectionUpdate {
            projection_root,
            manifest,
            projected_paths,
            baseline_rebuilt: needs_baseline,
            publication_stats,
        },
        publication,
    ))
}

struct ProjectReadSnapshot {
    bytes: Vec<u8>,
}

enum ProjectReadAccess<'a> {
    Active(ActiveProjectReadLease<'a>),
    Pending(&'a PendingProjectAuthority),
}

impl ProjectReadAccess<'_> {
    fn read_bounded_regular_file(
        &self,
        relative_path: &Path,
        max_bytes: u64,
        public_label: &str,
    ) -> Result<Option<ProjectReadSnapshot>, String> {
        let snapshot = match self {
            Self::Active(active) => {
                active.read_bounded_regular_file(relative_path, max_bytes, public_label)?
            }
            Self::Pending(pending) => {
                pending.read_bounded_regular_file(relative_path, max_bytes, public_label)?
            }
        };
        Ok(snapshot.map(|snapshot| ProjectReadSnapshot {
            bytes: snapshot.bytes,
        }))
    }
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

fn materialize_generation_contents(
    zola_root: &Path,
    generation: &mut PreviewProjectionGeneration,
    projection: &WorkspaceProjectionSnapshot,
    source_graph: &SourceGraph,
    project_read: &ProjectReadAccess<'_>,
) -> Result<(), String> {
    let active_theme = projected_active_theme(projection);
    let mut budget = MaterializationBudget::default();
    copy_zola_sources(
        zola_root,
        generation,
        active_theme.as_deref(),
        projection,
        project_read,
        &mut budget,
    )?;

    let mut deleted = projection.deleted_sources.iter().collect::<Vec<_>>();
    deleted.sort();
    for project_relative in deleted {
        let Some(zola_relative) = zola_relative_projection_path(project_relative)? else {
            continue;
        };
        // The staging root starts empty and project-owned deleted paths were
        // excluded while planning the disk baseline. Resolving the path here
        // still enforces the same lexical authority before publication.
        let _ = zola_relative;
    }

    let source_ids = SourceIdIndex::for_source_graph(
        source_graph,
        projection
            .source_texts
            .iter()
            .map(|(path, source)| (path.as_str(), source.as_str())),
    )?;
    let mut source_texts = projection.source_texts.iter().collect::<Vec<_>>();
    source_texts.sort_by(|left, right| left.0.cmp(right.0));
    for (project_relative, source) in source_texts {
        let Some(zola_relative) = zola_relative_projection_path(project_relative)? else {
            continue;
        };
        budget.reserve(&zola_relative, source.len() as u64)?;
        let projected = if is_annotated_template_path(&zola_relative) {
            preprocess_template_with_revision(
                source,
                &zola_relative.to_string_lossy().replace('\\', "/"),
                Some(&source_ids),
                None,
            )
        } else {
            source.clone()
        };
        generation.write_text(&zola_relative, &projected)?;
    }

    let mut resource_bytes = projection.resource_bytes.iter().collect::<Vec<_>>();
    resource_bytes.sort_by(|left, right| left.0.cmp(right.0));
    for (project_relative, bytes) in resource_bytes {
        let Some(zola_relative) = zola_relative_projection_path(project_relative)? else {
            continue;
        };
        budget.reserve(&zola_relative, bytes.len() as u64)?;
        generation.write_bytes(&zola_relative, bytes)?;
    }

    Ok(())
}

fn materialize_generation_delta(
    generation: &mut PreviewProjectionGeneration,
    projection: &WorkspaceProjectionSnapshot,
    source_graph: &SourceGraph,
    project_read: &ProjectReadAccess<'_>,
    changed_paths: &[String],
    reused_entries: usize,
    reused_bytes: u64,
) -> Result<(), String> {
    let mut budget = MaterializationBudget {
        entries: reused_entries,
        bytes: reused_bytes,
    };
    let source_ids = SourceIdIndex::for_source_graph(
        source_graph,
        projection
            .source_texts
            .iter()
            .map(|(path, source)| (path.as_str(), source.as_str())),
    )?;
    for project_relative in changed_paths {
        let Some(zola_relative) = zola_relative_projection_path(project_relative)? else {
            continue;
        };
        if projection.deleted_sources.contains(project_relative) {
            continue;
        }
        if let Some(source) = projection.source_texts.get(project_relative) {
            budget.reserve(&zola_relative, source.len() as u64)?;
            let projected = if is_annotated_template_path(&zola_relative) {
                preprocess_template_with_revision(
                    source,
                    &zola_relative.to_string_lossy().replace('\\', "/"),
                    Some(&source_ids),
                    None,
                )
            } else {
                source.clone()
            };
            generation.write_text(&zola_relative, &projected)?;
            continue;
        }
        if let Some(bytes) = projection.resource_bytes.get(project_relative) {
            budget.reserve(&zola_relative, bytes.len() as u64)?;
            generation.write_bytes(&zola_relative, bytes)?;
            continue;
        }

        let accepted = project_read.read_bounded_regular_file(
            Path::new(project_relative),
            budget.remaining_bytes(),
            "preview/projection/reverted-accepted-file",
        )?;
        let Some(accepted) = accepted else {
            // Undo of a session-created file: the old derived file was
            // excluded from the clone and no accepted source replaces it.
            continue;
        };
        if is_annotated_template_path(&zola_relative) {
            return Err(format!(
                "ProjectWorkspace nu a furnizat sursa text autoritativă pentru template-ul {}.",
                project_relative
            ));
        }
        budget.reserve(&zola_relative, accepted.bytes.len() as u64)?;
        generation.write_bytes(&zola_relative, &accepted.bytes)?;
    }
    Ok(())
}

fn projection_manifest(projection: &WorkspaceProjectionSnapshot) -> PersistentProjectionManifest {
    PersistentProjectionManifest {
        accepted_disk_generation: projection.accepted_disk.generation,
        active_theme: projected_active_theme(projection),
        text_hashes: projection
            .source_texts
            .iter()
            .map(|(path, source)| (path.clone(), hash_bytes(source.as_bytes())))
            .collect(),
        resource_hashes: projection
            .resource_bytes
            .iter()
            .map(|(path, bytes)| (path.clone(), hash_bytes(bytes)))
            .collect(),
        deleted_sources: projection.deleted_sources.iter().cloned().collect(),
    }
}

fn projected_overlay_paths(projection: &WorkspaceProjectionSnapshot) -> Vec<String> {
    projection
        .changed_paths
        .iter()
        .chain(projection.deleted_sources.iter())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn projection_changed_paths(
    previous: &PersistentProjectionManifest,
    next: &PersistentProjectionManifest,
) -> Vec<String> {
    previous
        .text_hashes
        .keys()
        .chain(previous.resource_hashes.keys())
        .chain(previous.deleted_sources.iter())
        .chain(next.text_hashes.keys())
        .chain(next.resource_hashes.keys())
        .chain(next.deleted_sources.iter())
        .filter(|path| {
            previous.text_hashes.get(*path) != next.text_hashes.get(*path)
                || previous.resource_hashes.get(*path) != next.resource_hashes.get(*path)
                || previous.deleted_sources.contains(*path) != next.deleted_sources.contains(*path)
        })
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
    projection: &WorkspaceProjectionSnapshot,
) -> Result<(), String> {
    let expected = Path::new(&projection.project_root).to_path_buf();
    let expected = expected.canonicalize().unwrap_or(expected);
    let actual = zola_root
        .canonicalize()
        .unwrap_or_else(|_| zola_root.to_path_buf());
    if expected != actual {
        return Err(format!(
            "Proiecția Preview a refuzat un projection ProjectWorkspace pentru alt root: așteptat {}, primit {}.",
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

fn require_disjoint_projection(projection: &WorkspaceProjectionSnapshot) -> Result<(), String> {
    if let Some(path) = projection
        .source_texts
        .keys()
        .find(|path| projection.deleted_sources.contains(*path))
    {
        return Err(format!(
            "ProjectWorkspace a produs o proiecție ambiguă pentru {path}: draft și delete simultan."
        ));
    }
    if let Some(path) = projection.resource_bytes.keys().find(|path| {
        projection.deleted_sources.contains(*path) || projection.source_texts.contains_key(*path)
    }) {
        return Err(format!(
            "ProjectWorkspace a produs o proiecție ambiguă pentru {path}: resursă binară suprapusă peste text sau delete."
        ));
    }
    Ok(())
}

fn require_accepted_disk_baseline(
    projection: &WorkspaceProjectionSnapshot,
    require_live_disk: bool,
) -> Result<(), String> {
    if require_live_disk {
        return projection.accepted_disk.require_live_complete(
            &projection.runtime_session_id,
            &projection.project_root,
            Path::new(&projection.project_root),
        );
    }
    projection
        .accepted_disk
        .require_identity(&projection.runtime_session_id, &projection.project_root)?;
    projection.accepted_disk.require_complete()?;
    if projection.accepted_disk.manifest.root != projection.project_root {
        return Err("AcceptedDisk provizoriu aparține altui root canonic.".to_string());
    }
    Ok(())
}

fn projected_active_theme(projection: &WorkspaceProjectionSnapshot) -> Option<String> {
    ["zola.toml", "config.toml"]
        .iter()
        .find_map(|relative| {
            if projection.deleted_sources.contains(*relative) {
                return None;
            }
            projection.source_texts.get(*relative).cloned()
        })
        .and_then(|source| active_theme_from_source(&source))
}

fn copy_zola_sources(
    zola_root: &Path,
    generation: &mut PreviewProjectionGeneration,
    active_theme: Option<&str>,
    projection: &WorkspaceProjectionSnapshot,
    project_read: &ProjectReadAccess<'_>,
    budget: &mut MaterializationBudget,
) -> Result<(), String> {
    let output_root = crate::deploy::resolve_artifact_root(zola_root, zola_root).ok();
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
        if output_root.as_deref() == Some(source.as_path()) {
            continue;
        }
        let target = PathBuf::from(&name);
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
                    generation.create_directory(&target)?;
                    copy_entry_recursive(
                        zola_root,
                        &theme_source,
                        &target.join(theme),
                        generation,
                        projection,
                        project_read,
                        budget,
                        output_root.as_deref(),
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
            zola_root,
            &source,
            &target,
            generation,
            projection,
            project_read,
            budget,
            output_root.as_deref(),
        )?;
    }
    Ok(())
}

// The recursive materializer keeps source/target trust roots and its resource budget explicit.
#[allow(clippy::too_many_arguments)]
fn copy_entry_recursive(
    zola_root: &Path,
    source: &Path,
    relative_target: &Path,
    generation: &mut PreviewProjectionGeneration,
    projection: &WorkspaceProjectionSnapshot,
    project_read: &ProjectReadAccess<'_>,
    budget: &mut MaterializationBudget,
    output_root: Option<&Path>,
) -> Result<(), String> {
    if output_root == Some(source) {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(source)
        .map_err(|error| format!("Nu am putut inspecta {}: {error}", source.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "Proiecția Preview refuză symlink-ul sursă {}.",
            source.display()
        ));
    }
    if metadata.is_dir() {
        budget.reserve(source, 0)?;
        generation.create_directory(relative_target)?;
        for entry in sorted_directory_entries(source)? {
            let name = entry.file_name();
            let name_text = name.to_string_lossy();
            if SENSITIVE_SOURCE_FILES
                .iter()
                .any(|sensitive| name_text.eq_ignore_ascii_case(sensitive))
            {
                continue;
            }
            if SKIPPED_SOURCE_DIRECTORIES
                .iter()
                .any(|skip| name_text.eq_ignore_ascii_case(skip))
            {
                continue;
            }
            copy_entry_recursive(
                zola_root,
                &entry.path(),
                &relative_target.join(name),
                generation,
                projection,
                project_read,
                budget,
                output_root,
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
    if workspace_owns_source_file(zola_root, source, projection)? {
        budget.reserve(source, metadata.len())?;
        return Ok(());
    }
    if is_annotated_template_path(relative_target) {
        return Err(format!(
            "ProjectWorkspace nu a furnizat sursa text autoritativă pentru template-ul {}.",
            source.display()
        ));
    }
    let project_relative = source.strip_prefix(zola_root).map_err(|_| {
        format!(
            "Proiecția Preview a primit o sursă în afara root-ului Zola: {}.",
            source.display()
        )
    })?;
    let snapshot = project_read
        .read_bounded_regular_file(
            project_relative,
            budget.remaining_bytes(),
            "preview/projection/accepted-binary",
        )?
        .ok_or_else(|| {
            format!(
                "AcceptedDisk a pierdut fișierul {} în timpul materializării.",
                source.display()
            )
        })?;
    budget.reserve(source, snapshot.bytes.len() as u64)?;
    generation.write_bytes(relative_target, &snapshot.bytes)
}

fn workspace_owns_source_file(
    zola_root: &Path,
    source: &Path,
    projection: &WorkspaceProjectionSnapshot,
) -> Result<bool, String> {
    let relative = source.strip_prefix(zola_root).map_err(|_| {
        format!(
            "Proiecția Preview a primit o sursă în afara root-ului Zola: {}.",
            source.display()
        )
    })?;
    let project_relative = relative.to_string_lossy().replace('\\', "/");
    Ok(projection.source_texts.contains_key(&project_relative)
        || projection.resource_bytes.contains_key(&project_relative)
        || projection.deleted_sources.contains(&project_relative))
}

fn is_annotated_template_path(relative: &Path) -> bool {
    is_template_relative_path(&relative.to_string_lossy())
        && matches!(
            relative
                .extension()
                .and_then(|extension| extension.to_str()),
            Some("html" | "md")
        )
}

fn zola_relative_projection_path(project_relative: &str) -> Result<Option<PathBuf>, String> {
    let normalized = project_relative.trim().replace('\\', "/");
    let relative = normalized.as_str();
    if relative.is_empty() {
        return Err("Proiecția Preview refuză rădăcina proiectului ca document.".to_string());
    }
    let path = Path::new(relative);
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            SENSITIVE_SOURCE_FILES
                .iter()
                .any(|sensitive| name.eq_ignore_ascii_case(sensitive))
        })
    {
        return Ok(None);
    }
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(format!(
                "Proiecția Preview refuză path-ul nesigur {project_relative}."
            ));
        }
    }
    if path
        .iter()
        .next()
        .and_then(|component| component.to_str())
        .is_some_and(|component| {
            SKIPPED_SOURCE_DIRECTORIES
                .iter()
                .any(|skip| component.eq_ignore_ascii_case(skip))
        })
    {
        return Ok(None);
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
    if !target.starts_with(boundary) {
        return Err(format!(
            "Proiecția Preview a refuzat directorul din afara limitei {}: {}.",
            boundary.display(),
            target.display()
        ));
    }
    match fs::symlink_metadata(target) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => return Ok(()),
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(format!(
                "Proiecția Preview a refuzat directorul symlink {}.",
                target.display()
            ))
        }
        Ok(_) => {
            return Err(format!(
                "Proiecția Preview aștepta un director la {}, dar a găsit alt tip de intrare.",
                target.display()
            ))
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "Proiecția Preview nu a putut inspecta directorul {}: {error}",
                target.display()
            ))
        }
    }
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

    #[test]
    fn projection_paths_accept_safe_zola_root_files_and_descendants() {
        assert_eq!(
            zola_relative_projection_path("templates/index.html").unwrap(),
            Some(PathBuf::from("templates/index.html"))
        );
        assert_eq!(
            zola_relative_projection_path("README.md").unwrap(),
            Some(PathBuf::from("README.md"))
        );
        assert_eq!(
            zola_relative_projection_path(".panastudio/motion/templates/index.json").unwrap(),
            None
        );
        assert_eq!(zola_relative_projection_path(".env").unwrap(), None);
        assert!(zola_relative_projection_path("templates/../outside.html").is_err());
    }

    #[test]
    fn draft_and_delete_for_same_path_are_rejected() {
        let mut source_texts = std::collections::HashMap::new();
        source_texts.insert("templates/index.html".to_string(), "draft".to_string());
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
        let projection = WorkspaceProjectionSnapshot {
            project_root: "/tmp/project".to_string(),
            runtime_session_id: "session".to_string(),
            revision: 1,
            workspace_transaction_id: Some("projection-test-1".to_string()),
            source_texts,
            resource_bytes: std::collections::HashMap::new(),
            deleted_sources: std::collections::HashSet::from(["templates/index.html".to_string()]),
            changed_paths: std::collections::HashSet::new(),
            accepted_disk,
        };
        assert!(require_disjoint_projection(&projection).is_err());
    }

    #[test]
    fn staged_projection_delta_includes_reverts_and_kind_changes() {
        let previous = PersistentProjectionManifest {
            text_hashes: BTreeMap::from([
                ("templates/index.html".to_string(), "draft".to_string()),
                ("templates/removed.html".to_string(), "old".to_string()),
            ]),
            resource_hashes: BTreeMap::from([(
                "static/data.bin".to_string(),
                "binary-old".to_string(),
            )]),
            deleted_sources: BTreeSet::from(["content/hidden.md".to_string()]),
            ..Default::default()
        };
        let next = PersistentProjectionManifest {
            text_hashes: BTreeMap::from([
                ("templates/index.html".to_string(), "accepted".to_string()),
                ("static/data.bin".to_string(), "text-now".to_string()),
            ]),
            deleted_sources: BTreeSet::from(["templates/removed.html".to_string()]),
            ..Default::default()
        };

        assert_eq!(
            projection_changed_paths(&previous, &next),
            vec![
                "content/hidden.md".to_string(),
                "static/data.bin".to_string(),
                "templates/index.html".to_string(),
                "templates/removed.html".to_string(),
            ]
        );
    }
}
