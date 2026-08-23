use std::{
    path::PathBuf,
    sync::{atomic::AtomicU64, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

use crate::kernel::{
    ai_coordination::AiCoordinationRuntime,
    audit::{AuditRequest, AuditRunReceipt},
    canvas_interaction::CanvasInteractionRuntime,
    context_hub::ContextHubRuntime,
    editor_navigation::EditorNavigationRuntime,
    file_explorer::FileExplorerRuntime,
    global_status::GlobalStatusRuntime,
    project_workspace::ProjectWorkspace,
    publish_operation::PublishOperationControl,
    publish_preflight::{PublishBuildReceipt, PublishPreflightReceipt},
    recovery_coordinator::RecoveryCoordinatorScan,
    selection_coordinator::SelectionCoordinatorRuntime,
    workbench::{WorkbenchProjectionPersistence, WorkbenchRuntime},
};
use crate::preview::{PersistentZolaPreviewEngine, SourceBrowserEngine};
use crate::project::{ProjectDiskWatchHandle, ProjectLifecycleRuntime, StartupFlowRuntime};
use crate::versioning::VersionNetworkOperationRuntime;

pub(crate) struct McpServerHandle {
    pub(crate) cancellation_token: CancellationToken,
    pub(crate) thread: Option<JoinHandle<()>>,
}

#[derive(Clone)]
pub(crate) struct ProjectAuditAuthority {
    pub request: AuditRequest,
    pub receipt: AuditRunReceipt,
}

impl McpServerHandle {
    pub fn is_running(&self) -> bool {
        self.thread
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
    }

    pub fn stop(mut self) {
        self.cancellation_token.cancel();
        let deadline = Instant::now() + Duration::from_millis(1_500);
        while self
            .thread
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
            && Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(10));
        }
        if self.thread.as_ref().is_some_and(JoinHandle::is_finished) {
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        } else {
            eprintln!(
                "[Pană Studio] Shutdown MCP a depășit 1500ms; thread-ul este detașat, iar procesul poate continua închiderea."
            );
            self.thread.take();
        }
    }
}

impl Drop for McpServerHandle {
    fn drop(&mut self) {
        self.cancellation_token.cancel();
    }
}

pub(crate) struct AppState {
    pub(crate) ai_coordination: AiCoordinationRuntime,
    pub(crate) ai_coordination_deadline_generation: AtomicU64,
    pub(crate) canvas_interaction: CanvasInteractionRuntime,
    pub(crate) context_hub: ContextHubRuntime,
    pub(crate) editor_navigation: EditorNavigationRuntime,
    pub(crate) file_explorer: FileExplorerRuntime,
    pub(crate) global_status: GlobalStatusRuntime,
    pub(crate) selection_coordinator: SelectionCoordinatorRuntime,
    pub(crate) startup_flow: StartupFlowRuntime,
    pub(crate) project_lifecycle: ProjectLifecycleRuntime,
    pub(crate) project_lifecycle_transition: Mutex<()>,
    pub(crate) mcp_access_token: Mutex<Option<String>>,
    pub(crate) current_root: Mutex<Option<PathBuf>>,
    pub(crate) project_disk_watch: Mutex<Option<ProjectDiskWatchHandle>>,
    pub(crate) project_disk_watch_transition: Mutex<()>,
    pub(crate) project_workspace: Mutex<Option<ProjectWorkspace>>,
    pub(crate) project_audit_authority: Mutex<Option<ProjectAuditAuthority>>,
    pub(crate) workbench: WorkbenchRuntime,
    pub(crate) workbench_projection_persistence: WorkbenchProjectionPersistence,
    pub(crate) publish_operation: Mutex<Option<PublishOperationControl>>,
    pub(crate) publish_authorization_gate: Mutex<()>,
    pub(crate) publish_preflight_receipt: Mutex<Option<PublishPreflightReceipt>>,
    pub(crate) publish_build_receipt: Mutex<Option<PublishBuildReceipt>>,
    pub(crate) versioning_operation: Mutex<()>,
    pub(crate) versioning_network_operation: VersionNetworkOperationRuntime,
    pub(crate) recovery_coordinator_scan: Mutex<Option<RecoveryCoordinatorScan>>,
    pub(crate) preview_workspace_operation: Mutex<()>,
    pub(crate) preview_engine: Mutex<Option<PersistentZolaPreviewEngine>>,
    pub(crate) source_browser_operation: Mutex<()>,
    pub(crate) source_browser_engine: Mutex<Option<SourceBrowserEngine>>,
    pub(crate) version_preview_operation: Mutex<()>,
    pub(crate) version_preview_engine: Mutex<Option<SourceBrowserEngine>>,
    pub(crate) mcp_server: Mutex<Option<McpServerHandle>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            ai_coordination: AiCoordinationRuntime::default(),
            ai_coordination_deadline_generation: AtomicU64::new(0),
            canvas_interaction: CanvasInteractionRuntime::default(),
            context_hub: ContextHubRuntime::default(),
            editor_navigation: EditorNavigationRuntime::default(),
            file_explorer: FileExplorerRuntime::default(),
            global_status: GlobalStatusRuntime::default(),
            selection_coordinator: SelectionCoordinatorRuntime::default(),
            startup_flow: StartupFlowRuntime::default(),
            project_lifecycle: ProjectLifecycleRuntime::default(),
            project_lifecycle_transition: Mutex::new(()),
            mcp_access_token: Mutex::new(None),
            current_root: Mutex::new(None),
            project_disk_watch: Mutex::new(None),
            project_disk_watch_transition: Mutex::new(()),
            project_workspace: Mutex::new(None),
            project_audit_authority: Mutex::new(None),
            workbench: WorkbenchRuntime::default(),
            workbench_projection_persistence: WorkbenchProjectionPersistence::default(),
            publish_operation: Mutex::new(None),
            publish_authorization_gate: Mutex::new(()),
            publish_preflight_receipt: Mutex::new(None),
            publish_build_receipt: Mutex::new(None),
            versioning_operation: Mutex::new(()),
            versioning_network_operation: VersionNetworkOperationRuntime::default(),
            recovery_coordinator_scan: Mutex::new(None),
            preview_workspace_operation: Mutex::new(()),
            preview_engine: Mutex::new(None),
            source_browser_operation: Mutex::new(()),
            source_browser_engine: Mutex::new(None),
            version_preview_operation: Mutex::new(()),
            version_preview_engine: Mutex::new(None),
            mcp_server: Mutex::new(None),
        }
    }
}

impl AppState {
    pub(crate) fn clear_publish_authorization(&self) -> Result<(), String> {
        self.publish_preflight_receipt
            .lock()
            .map_err(|_| "Nu am putut invalida Publish Preflight.".to_string())?
            .take();
        self.publish_build_receipt
            .lock()
            .map_err(|_| "Nu am putut invalida buildul pentru publicare.".to_string())?
            .take();
        Ok(())
    }
}
