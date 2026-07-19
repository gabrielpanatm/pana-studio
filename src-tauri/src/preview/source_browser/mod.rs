mod server;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use tauri::{AppHandle, Manager, Runtime};

use self::server::{SourceBrowserContent, SourceBrowserGeneration, SourceBrowserServer};
use super::{
    engine::render_official_zola_disk_generation,
    preprocess::{
        create_source_browser_artifact_root, prepare_source_browser_session,
        remove_source_browser_artifact_root, remove_source_browser_session,
        reset_source_browser_cache, source_browser_session_root,
    },
    process::{
        require_browser_preview_session, BrowserPreviewRequestIdentity, BrowserPreviewStartReceipt,
    },
};
use crate::{
    kernel::observability::{append_event, KernelEventKind, KernelLogEvent, KernelLogLevel},
    project::zola_project_root,
    state::AppState,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceBrowserOwner {
    project_root: String,
    runtime_session_id: String,
}

impl SourceBrowserOwner {
    fn from_identity(identity: &BrowserPreviewRequestIdentity) -> Self {
        Self {
            project_root: identity.expected_project_root.clone(),
            runtime_session_id: identity.expected_session_id.clone(),
        }
    }

    fn matches_generation(&self, generation: &SourceBrowserGeneration) -> bool {
        generation.owner_matches(&self.project_root, &self.runtime_session_id)
    }
}

struct SourceBrowserCandidate {
    generation: Arc<SourceBrowserGeneration>,
}

pub(crate) struct SourceBrowserEngine {
    owner: SourceBrowserOwner,
    zola_root: PathBuf,
    session_root: PathBuf,
    server: Option<SourceBrowserServer>,
    retired: Vec<Arc<SourceBrowserGeneration>>,
}

impl SourceBrowserEngine {
    fn start<R: Runtime>(
        app: &AppHandle<R>,
        zola_root: &Path,
        owner: SourceBrowserOwner,
    ) -> Result<Self, String> {
        let zola_root = zola_root
            .canonicalize()
            .unwrap_or_else(|_| zola_root.to_path_buf());
        reset_source_browser_cache(app, &zola_root)?;
        let session_root = source_browser_session_root(app, &zola_root, &owner.runtime_session_id)?;
        prepare_source_browser_session(app, &zola_root, &session_root)?;
        let server = SourceBrowserServer::start()?;
        Ok(Self {
            owner,
            zola_root,
            session_root,
            server: Some(server),
            retired: Vec::new(),
        })
    }

    fn owner_matches(&self, owner: &SourceBrowserOwner) -> bool {
        self.owner == *owner
    }

    fn url(&self) -> Result<String, String> {
        self.server
            .as_ref()
            .map(SourceBrowserServer::url)
            .ok_or_else(|| "Source Browser server a fost oprit.".to_string())
    }

    fn active_matches_generation(&self, disk_generation: u64) -> Result<bool, String> {
        self.server
            .as_ref()
            .ok_or_else(|| "Source Browser server a fost oprit.".to_string())?
            .active()
            .map(|generation| {
                generation.is_some_and(|generation| {
                    self.owner.matches_generation(&generation)
                        && generation.disk_generation == disk_generation
                })
            })
    }

    fn render_candidate<R: Runtime>(
        &mut self,
        app: &AppHandle<R>,
        disk_generation: u64,
    ) -> Result<SourceBrowserCandidate, String> {
        self.collect_retired(app);
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| "Source Browser server a fost oprit.".to_string())?;
        server.mark_building(disk_generation)?;
        let source_revision = next_source_revision(disk_generation);
        let artifact_root =
            create_source_browser_artifact_root(app, &self.session_root, &source_revision)?;
        let rendered = render_official_zola_disk_generation(
            &self.zola_root,
            &artifact_root,
            &server.url(),
            disk_generation,
        );
        let rendered = match rendered {
            Ok(rendered) => rendered,
            Err(error) => {
                let cleanup =
                    remove_source_browser_artifact_root(app, &self.session_root, &artifact_root);
                return Err(match cleanup {
                    Ok(()) => error,
                    Err(cleanup_error) => {
                        format!("{error} Cleanup Source Browser eșuat: {cleanup_error}")
                    }
                });
            }
        };
        let content = rendered
            .into_iter()
            .map(|(path, body)| {
                let extension = Path::new(&path)
                    .extension()
                    .and_then(|value| value.to_str());
                let content = match extension {
                    Some("xml") => SourceBrowserContent::Text {
                        body: body.into_bytes(),
                        content_type: "text/xml; charset=utf-8".to_string(),
                    },
                    Some("json") => SourceBrowserContent::Text {
                        body: body.into_bytes(),
                        content_type: "application/json; charset=utf-8".to_string(),
                    },
                    Some("txt") => SourceBrowserContent::Text {
                        body: body.into_bytes(),
                        content_type: "text/plain; charset=utf-8".to_string(),
                    },
                    _ => SourceBrowserContent::Html(body),
                };
                (path, content)
            })
            .collect::<HashMap<_, _>>();
        Ok(SourceBrowserCandidate {
            generation: Arc::new(SourceBrowserGeneration {
                project_root: self.owner.project_root.clone(),
                runtime_session_id: self.owner.runtime_session_id.clone(),
                disk_generation,
                content,
                assets_root: artifact_root,
            }),
        })
    }

    fn publish_candidate<R: Runtime>(
        &mut self,
        app: &AppHandle<R>,
        candidate: SourceBrowserCandidate,
    ) -> Result<(), String> {
        if !self.owner.matches_generation(&candidate.generation) {
            return Err("Candidatul Source Browser aparține altei sesiuni.".to_string());
        }
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| "Source Browser server a fost oprit.".to_string())?;
        if let Some(previous) = server.publish(candidate.generation)? {
            self.retired.push(previous);
        }
        self.collect_retired(app);
        Ok(())
    }

    fn discard_candidate<R: Runtime>(
        &mut self,
        app: &AppHandle<R>,
        candidate: SourceBrowserCandidate,
    ) -> Result<(), String> {
        let artifact_root = candidate.generation.assets_root.clone();
        drop(candidate);
        remove_source_browser_artifact_root(app, &self.session_root, &artifact_root)
    }

    fn publish_failure(&self, disk_generation: u64, diagnostic: String) {
        if let Some(server) = self.server.as_ref() {
            let _ = server.publish_failure(disk_generation, diagnostic);
        }
    }

    fn collect_retired<R: Runtime>(&mut self, app: &AppHandle<R>) {
        let mut retained = Vec::new();
        for generation in self.retired.drain(..) {
            if Arc::strong_count(&generation) == 1 {
                let root = generation.assets_root.clone();
                drop(generation);
                let _ = remove_source_browser_artifact_root(app, &self.session_root, &root);
            } else {
                retained.push(generation);
            }
        }
        self.retired = retained;
    }

    fn stop<R: Runtime>(mut self, app: &AppHandle<R>) -> Result<(), String> {
        if let Some(server) = self.server.take() {
            server.stop();
        }
        self.retired.clear();
        remove_source_browser_session(app, &self.zola_root, &self.session_root)
    }
}

pub(crate) fn start_or_refresh_source_browser<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    identity: &BrowserPreviewRequestIdentity,
    start_if_missing: bool,
) -> Result<Option<BrowserPreviewStartReceipt>, String> {
    let _operation = state
        .source_browser_operation
        .lock()
        .map_err(|_| "Nu am putut serializa Source Browser.".to_string())?;
    let project_root = require_browser_preview_session(state, identity)?;
    let zola_root = zola_project_root(&project_root);
    let owner = SourceBrowserOwner::from_identity(identity);
    let mut slot = state
        .source_browser_engine
        .lock()
        .map_err(|_| "Nu am putut bloca Source Browser engine.".to_string())?;

    if slot
        .as_ref()
        .is_some_and(|engine| !engine.owner_matches(&owner))
    {
        let previous = slot.take();
        drop(slot);
        if let Some(previous) = previous {
            previous.stop(app)?;
        }
        require_browser_preview_session(state, identity)?;
        slot = state
            .source_browser_engine
            .lock()
            .map_err(|_| "Nu am putut bloca Source Browser engine.".to_string())?;
    }

    if slot.is_none() {
        if !start_if_missing {
            return Ok(None);
        }
        *slot = Some(SourceBrowserEngine::start(app, &zola_root, owner.clone())?);
    }
    let engine = slot
        .as_mut()
        .ok_or_else(|| "Source Browser engine nu a fost inițializat.".to_string())?;

    if !engine.active_matches_generation(identity.expected_disk_generation)? {
        log_source_browser_event(
            app,
            identity,
            KernelEventKind::SourceBrowserBuildStarted,
            KernelLogLevel::Info,
            "Source Browser construiește o generație AcceptedDisk.",
            None,
        );
        let candidate = match engine.render_candidate(app, identity.expected_disk_generation) {
            Ok(candidate) => candidate,
            Err(error) => {
                if require_browser_preview_session(state, identity).is_ok() {
                    engine.publish_failure(identity.expected_disk_generation, error.clone());
                    log_source_browser_event(
                        app,
                        identity,
                        KernelEventKind::SourceBrowserFailed,
                        KernelLogLevel::Error,
                        "Source Browser nu a putut construi generația AcceptedDisk.",
                        Some(error.clone()),
                    );
                }
                return Err(error);
            }
        };
        if let Err(stale) = require_browser_preview_session(state, identity) {
            let cleanup = engine.discard_candidate(app, candidate);
            log_source_browser_event(
                app,
                identity,
                KernelEventKind::SourceBrowserStaleDiscarded,
                KernelLogLevel::Warn,
                "Source Browser a eliminat un candidat depășit.",
                Some(stale.clone()),
            );
            return Err(match cleanup {
                Ok(()) => stale,
                Err(cleanup_error) => format!("{stale} Cleanup candidat eșuat: {cleanup_error}"),
            });
        }
        engine.publish_candidate(app, candidate)?;
        log_source_browser_event(
            app,
            identity,
            KernelEventKind::SourceBrowserPublished,
            KernelLogLevel::Info,
            "Source Browser a publicat atomic generația AcceptedDisk.",
            None,
        );
    }
    require_browser_preview_session(state, identity)?;
    Ok(Some(BrowserPreviewStartReceipt {
        url: engine.url()?,
        project_root: identity.expected_project_root.clone(),
        runtime_session_id: identity.expected_session_id.clone(),
        accepted_disk_generation: identity.expected_disk_generation,
    }))
}

pub(crate) fn schedule_source_browser_refresh<R: Runtime>(
    app: &AppHandle<R>,
    identity: BrowserPreviewRequestIdentity,
) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        if let Err(error) = start_or_refresh_source_browser(&app, state.inner(), &identity, false) {
            eprintln!("[Pană Studio] Rebuild Source Browser eșuat: {error}");
        }
    });
}

pub(crate) fn stop_source_browser<R: Runtime>(app: &AppHandle<R>, state: &AppState) {
    let Ok(_operation) = state.source_browser_operation.lock() else {
        return;
    };
    let engine = state
        .source_browser_engine
        .lock()
        .ok()
        .and_then(|mut slot| slot.take());
    if let Some(engine) = engine {
        if let Err(error) = engine.stop(app) {
            eprintln!("[Pană Studio] Cleanup Source Browser incomplet: {error}");
        }
    }
    stop_version_source_browser(app, state);
}

pub(crate) fn start_version_source_browser<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    historical_zola_root: &Path,
    project_root: &str,
    runtime_session_id: &str,
    commit_oid: &str,
) -> Result<String, String> {
    let _operation = state
        .version_preview_operation
        .lock()
        .map_err(|_| "Nu am putut serializa Preview-ul unei versiuni Git.".to_string())?;
    let owner = SourceBrowserOwner {
        project_root: project_root.to_string(),
        runtime_session_id: format!("{runtime_session_id}:git:{commit_oid}"),
    };
    let mut slot = state
        .version_preview_engine
        .lock()
        .map_err(|_| "Nu am putut bloca motorul Preview pentru versiuni.".to_string())?;
    if slot
        .as_ref()
        .is_some_and(|engine| !engine.owner_matches(&owner))
    {
        let previous = slot.take();
        drop(slot);
        if let Some(previous) = previous {
            previous.stop(app)?;
        }
        slot = state
            .version_preview_engine
            .lock()
            .map_err(|_| "Nu am putut rebloca motorul Preview pentru versiuni.".to_string())?;
    }
    if slot.is_none() {
        *slot = Some(SourceBrowserEngine::start(
            app,
            historical_zola_root,
            owner.clone(),
        )?);
    }
    let engine = slot
        .as_mut()
        .ok_or_else(|| "Motorul Preview pentru versiuni nu a fost inițializat.".to_string())?;
    let generation = u64::from_str_radix(&commit_oid[..commit_oid.len().min(16)], 16)
        .map_err(|error| format!("Commit OID invalid pentru generația Preview: {error}"))?;
    if !engine.active_matches_generation(generation)? {
        let candidate = engine.render_candidate(app, generation)?;
        engine.publish_candidate(app, candidate)?;
    }
    engine.url()
}

pub(crate) fn stop_version_source_browser<R: Runtime>(app: &AppHandle<R>, state: &AppState) {
    let Ok(_operation) = state.version_preview_operation.lock() else {
        return;
    };
    let engine = state
        .version_preview_engine
        .lock()
        .ok()
        .and_then(|mut slot| slot.take());
    if let Some(engine) = engine {
        if let Err(error) = engine.stop(app) {
            eprintln!("[Pană Studio] Cleanup Version Preview incomplet: {error}");
        }
    }
}

fn next_source_revision(disk_generation: u64) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| format!("{}-{}", duration.as_secs(), duration.subsec_nanos()))
        .unwrap_or_else(|_| "0-0".to_string());
    format!("disk-{disk_generation}-{timestamp}")
}

fn log_source_browser_event<R: Runtime>(
    app: &AppHandle<R>,
    identity: &BrowserPreviewRequestIdentity,
    kind: KernelEventKind,
    level: KernelLogLevel,
    message: &str,
    diagnostic: Option<String>,
) {
    let _ = append_event(
        app,
        KernelLogEvent::new(
            level,
            kind,
            "source_browser",
            "preview_projection",
            "source_browser.generation",
            Some(identity.expected_project_root.clone()),
            message,
            diagnostic,
        )
        .with_attribute("projectRoot", &identity.expected_project_root)
        .with_attribute("runtimeSessionId", &identity.expected_session_id)
        .with_attribute("acceptedDiskGeneration", identity.expected_disk_generation),
    );
}
