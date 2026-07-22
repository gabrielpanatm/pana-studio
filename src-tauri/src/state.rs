use std::{
    path::PathBuf,
    sync::Mutex,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

use crate::kernel::{
    ai_coordination::AiCoordinationRuntime, context_hub::ContextHubRuntime,
    project_workspace::ProjectWorkspace, publish_operation::PublishOperationControl,
    recovery_coordinator::RecoveryCoordinatorScan, workbench::WorkbenchRuntime,
};
use crate::preview::{PersistentZolaPreviewEngine, SourceBrowserEngine};
use crate::versioning::VersionNetworkOperationControl;

pub struct McpServerHandle {
    pub cancellation_token: CancellationToken,
    pub thread: Option<JoinHandle<()>>,
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

pub struct AppState {
    pub ai_coordination: AiCoordinationRuntime,
    pub context_hub: ContextHubRuntime,
    pub mcp_access_token: Mutex<Option<String>>,
    pub current_root: Mutex<Option<PathBuf>>,
    pub project_workspace: Mutex<Option<ProjectWorkspace>>,
    pub workbench: WorkbenchRuntime,
    pub publish_operation: Mutex<Option<PublishOperationControl>>,
    pub versioning_operation: Mutex<()>,
    pub versioning_network_operation: Mutex<Option<VersionNetworkOperationControl>>,
    pub recovery_coordinator_scan: Mutex<Option<RecoveryCoordinatorScan>>,
    pub zola_binary_path: Mutex<Option<PathBuf>>,
    pub preview_workspace_operation: Mutex<()>,
    pub preview_engine: Mutex<Option<PersistentZolaPreviewEngine>>,
    pub source_browser_operation: Mutex<()>,
    pub source_browser_engine: Mutex<Option<SourceBrowserEngine>>,
    pub version_preview_operation: Mutex<()>,
    pub version_preview_engine: Mutex<Option<SourceBrowserEngine>>,
    pub mcp_server: Mutex<Option<McpServerHandle>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            ai_coordination: AiCoordinationRuntime::default(),
            context_hub: ContextHubRuntime::default(),
            mcp_access_token: Mutex::new(None),
            current_root: Mutex::new(None),
            project_workspace: Mutex::new(None),
            workbench: WorkbenchRuntime::default(),
            publish_operation: Mutex::new(None),
            versioning_operation: Mutex::new(()),
            versioning_network_operation: Mutex::new(None),
            recovery_coordinator_scan: Mutex::new(None),
            zola_binary_path: Mutex::new(None),
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
