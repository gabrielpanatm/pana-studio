mod canvas;
mod engine;
pub(crate) mod http;
pub(crate) mod inject;
mod process;
mod resource_url;
mod server;
mod source_browser;

pub mod preprocess;

pub(crate) use canvas::{
    CanvasBoundaryInstance, CanvasGraph, CanvasMarkdownProvenanceState, CanvasNodeOrigin,
    CanvasProjectionPlan, CanvasProjectionTransaction, CanvasRenderNode, CanvasResourceManifest,
    PreviewImpact,
};
#[cfg(test)]
pub(crate) use canvas::{CanvasBoundaryMarkerKind, CanvasDocumentGraph, CanvasNodeCapabilities};
pub use canvas::{CanvasProjectionIdentity, CanvasProjectionPhase, PreviewPhaseReceipt};
pub(crate) use engine::{
    PersistentPreviewCandidate, PersistentPreviewOwner, PersistentZolaPreviewEngine,
    TemplateWorkbenchReuseQuery,
};
pub use http::read_http_document;
pub use process::{
    require_browser_preview_session, require_project_preview_session,
    require_project_preview_workspace_revision, BrowserPreviewRequestIdentity,
    BrowserPreviewStartReceipt, ProjectPreviewMutationKind, ProjectPreviewMutationReceipt,
    ProjectPreviewRequestIdentity, ProjectPreviewStartReceipt,
};
pub(crate) use server::ActivePreviewGeneration;
pub(crate) use source_browser::{
    schedule_source_browser_refresh, start_or_refresh_source_browser, SourceBrowserEngine,
};
pub(crate) use source_browser::{
    start_version_source_browser, stop_source_browser, stop_version_source_browser,
};

/// Zola's in-memory map does not encode directory indexes uniformly: pages
/// commonly keep `despre/`, while sections, taxonomies and pagers can keep
/// `blog` for the same public `/blog/` shape. HTTP surfaces therefore probe
/// the alternate spelling only for HTML content, after the exact key.
pub(crate) fn alternate_zola_directory_content_key(content_key: &str) -> Option<String> {
    if content_key.is_empty() {
        return None;
    }
    if let Some(trimmed) = content_key.strip_suffix('/') {
        return (!trimmed.is_empty()).then(|| trimmed.to_string());
    }
    Some(format!("{content_key}/"))
}

pub fn stop_project_preview<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    state: &crate::state::AppState,
) {
    state.canvas_interaction.revoke_all();
    state.editor_navigation.revoke_all();
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
