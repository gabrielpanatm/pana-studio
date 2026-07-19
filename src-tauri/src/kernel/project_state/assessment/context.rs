use crate::kernel::project_session::ProjectSessionSnapshot;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ProjectStateAssessmentContext {
    pub(super) project_open: bool,
    pub(super) session_available: bool,
    pub(super) session_id: Option<String>,
    pub(super) project_root: Option<String>,
    pub(super) project_workspace_available: bool,
    pub(super) disk_conflict_snapshot_available: bool,
}

impl ProjectStateAssessmentContext {
    pub(super) fn from_inputs(
        project_root: Option<&str>,
        session: Option<&ProjectSessionSnapshot>,
        project_workspace_available: bool,
        disk_conflict_snapshot_available: bool,
    ) -> Self {
        let project_open = project_root.is_some() || session.is_some();
        let project_root = session
            .map(|session| session.project_root.clone())
            .or_else(|| project_root.map(str::to_string));
        let session_id = session.map(|session| session.id.clone());

        Self {
            project_open,
            session_available: session.is_some(),
            session_id,
            project_root,
            project_workspace_available,
            disk_conflict_snapshot_available,
        }
    }
}
