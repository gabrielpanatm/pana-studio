mod canvas;
mod engine;
pub(crate) mod http;
pub(crate) mod inject;
mod process;
mod server;
mod source_browser;
mod zola;

pub mod preprocess;

pub(crate) use canvas::{
    CanvasGraph, CanvasProjectionPlan, CanvasProjectionTransaction, CanvasResourceManifest,
    PreviewImpact,
};
pub use canvas::{CanvasProjectionIdentity, CanvasProjectionPhase, PreviewPhaseReceipt};
pub(crate) use engine::{
    PersistentPreviewCandidate, PersistentPreviewOwner, PersistentZolaPreviewEngine,
};
pub use http::read_http_document;
pub use process::{
    require_browser_preview_session, require_project_preview_session,
    require_project_preview_workspace_revision, BrowserPreviewRequestIdentity,
    BrowserPreviewStartReceipt, ProjectPreviewMutationKind, ProjectPreviewMutationReceipt,
    ProjectPreviewRequestIdentity, ProjectPreviewStartReceipt,
};
pub(crate) use source_browser::{
    schedule_source_browser_refresh, start_or_refresh_source_browser, SourceBrowserEngine,
};
pub(crate) use source_browser::{
    start_version_source_browser, stop_source_browser, stop_version_source_browser,
};
pub use zola::resolve_zola_binary_path;

pub fn stop_project_preview<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    state: &crate::state::AppState,
) {
    let engine = state
        .preview_engine
        .lock()
        .ok()
        .and_then(|mut slot| slot.take());
    if let Some(engine) = engine {
        if let Err(error) = engine.stop(app) {
            eprintln!("[Pană Studio] Cleanup Preview persistent incomplet: {error}");
        }
    }
}
