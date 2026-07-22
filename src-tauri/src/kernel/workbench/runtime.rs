use std::sync::Mutex;

use crate::kernel::{
    project_path::normalize_project_relative_path, project_session::ProjectSessionSnapshot,
};

use super::model::{
    WorkbenchActivity, WorkbenchBottomPanelSnapshot, WorkbenchCanvasViewportSnapshot,
    WorkbenchCommandReceipt, WorkbenchDocumentSnapshot, WorkbenchGroupId, WorkbenchGroupSnapshot,
    WorkbenchIdentity, WorkbenchIntent, WorkbenchSnapshot, WorkbenchSplit, WorkbenchSurface,
    WORKBENCH_COMMAND_SCHEMA_VERSION, WORKBENCH_DEFAULT_SPLIT_RATIO_BASIS_POINTS,
    WORKBENCH_MAX_OPEN_DOCUMENTS, WORKBENCH_MAX_SPLIT_RATIO_BASIS_POINTS,
    WORKBENCH_MAX_VIEWPORT_WIDTH_PX, WORKBENCH_MAX_VIEWPORT_ZOOM_PERCENT,
    WORKBENCH_MIN_SPLIT_RATIO_BASIS_POINTS, WORKBENCH_MIN_VIEWPORT_WIDTH_PX,
    WORKBENCH_MIN_VIEWPORT_ZOOM_PERCENT, WORKBENCH_SCHEMA_VERSION,
};

#[derive(Clone, Debug)]
struct WorkbenchSession {
    project_root: String,
    project_session_id: String,
    runtime_session_id: String,
    revision: u64,
    active_activity: WorkbenchActivity,
    active_group_id: WorkbenchGroupId,
    split: WorkbenchSplit,
    split_ratio_basis_points: u16,
    canvas_viewport: WorkbenchCanvasViewportSnapshot,
    groups: Vec<WorkbenchGroupSnapshot>,
    bottom_panel: WorkbenchBottomPanelSnapshot,
}

impl WorkbenchSession {
    fn new(session: &ProjectSessionSnapshot) -> Self {
        Self {
            project_root: session.project_root.clone(),
            project_session_id: session.id.clone(),
            runtime_session_id: session.runtime_instance_id(),
            revision: 0,
            active_activity: WorkbenchActivity::Editor,
            active_group_id: WorkbenchGroupId::Primary,
            split: WorkbenchSplit::None,
            split_ratio_basis_points: WORKBENCH_DEFAULT_SPLIT_RATIO_BASIS_POINTS,
            canvas_viewport: WorkbenchCanvasViewportSnapshot::default(),
            groups: vec![empty_group(WorkbenchGroupId::Primary)],
            bottom_panel: WorkbenchBottomPanelSnapshot {
                open: false,
                active_view: Default::default(),
            },
        }
    }

    fn rebind(&mut self, session: &ProjectSessionSnapshot) {
        self.project_root = session.project_root.clone();
        self.runtime_session_id = session.runtime_instance_id();
    }

    fn from_persisted(
        session: &ProjectSessionSnapshot,
        snapshot: WorkbenchSnapshot,
    ) -> Result<Self, String> {
        if snapshot.schema_version != WORKBENCH_SCHEMA_VERSION
            || snapshot.project_session_id != session.id
            || snapshot.project_root != session.project_root
        {
            return Err(
                "Workbench nu poate restaura o proiecție pentru alt proiect sau altă schemă."
                    .to_string(),
            );
        }
        let candidate = Self {
            project_root: session.project_root.clone(),
            project_session_id: session.id.clone(),
            runtime_session_id: session.runtime_instance_id(),
            revision: snapshot.revision,
            active_activity: snapshot.active_activity,
            active_group_id: snapshot.active_group_id,
            split: snapshot.split,
            split_ratio_basis_points: snapshot.split_ratio_basis_points,
            canvas_viewport: snapshot.canvas_viewport,
            groups: snapshot.groups,
            bottom_panel: snapshot.bottom_panel,
        };
        candidate.require_invariants()?;
        Ok(candidate)
    }

    fn snapshot(&self) -> WorkbenchSnapshot {
        WorkbenchSnapshot {
            schema_version: WORKBENCH_SCHEMA_VERSION,
            project_root: self.project_root.clone(),
            project_session_id: self.project_session_id.clone(),
            runtime_session_id: self.runtime_session_id.clone(),
            revision: self.revision,
            active_activity: self.active_activity,
            active_group_id: self.active_group_id,
            split: self.split,
            split_ratio_basis_points: self.split_ratio_basis_points,
            canvas_viewport: self.canvas_viewport.clone(),
            groups: self.groups.clone(),
            bottom_panel: self.bottom_panel.clone(),
        }
    }

    fn require_identity(&self, identity: &WorkbenchIdentity) -> Result<(), String> {
        if identity.expected_project_root != self.project_root
            || identity.expected_runtime_session_id != self.runtime_session_id
        {
            return Err(format!(
                "Workbench a refuzat un intent stale: aștepta {}/{}, dar Rust deține {}/{}.",
                identity.expected_project_root,
                identity.expected_runtime_session_id,
                self.project_root,
                self.runtime_session_id,
            ));
        }
        if identity.expected_revision != self.revision {
            return Err(format!(
                "Workbench a refuzat revizia stale {}: revizia Rust este {}.",
                identity.expected_revision, self.revision,
            ));
        }
        Ok(())
    }

    fn apply(&mut self, intent: WorkbenchIntent) -> Result<(), String> {
        match intent {
            WorkbenchIntent::OpenDocument {
                relative_path,
                group_id,
                surface,
                pinned,
            } => self.open_document(relative_path, group_id, surface, pinned),
            WorkbenchIntent::ActivateDocument {
                document_id,
                group_id,
            } => self.activate_document(&document_id, group_id),
            WorkbenchIntent::CloseDocument {
                document_id,
                group_id,
            } => self.close_document(&document_id, group_id),
            WorkbenchIntent::MoveDocument {
                document_id,
                from_group_id,
                to_group_id,
                index,
            } => self.move_document(&document_id, from_group_id, to_group_id, index),
            WorkbenchIntent::SetDocumentSurface {
                document_id,
                group_id,
                surface,
            } => self.set_document_surface(&document_id, group_id, surface),
            WorkbenchIntent::SetSplit { split } => self.set_split(split),
            WorkbenchIntent::ConfigureSynchronizedSplit {
                split,
                relative_path,
                secondary_surface,
            } => self.configure_synchronized_split(split, relative_path, secondary_surface),
            WorkbenchIntent::SetSplitRatio { ratio_basis_points } => {
                self.set_split_ratio(ratio_basis_points)
            }
            WorkbenchIntent::SetCanvasViewport { viewport } => {
                validate_canvas_viewport(&viewport)?;
                self.canvas_viewport = viewport;
                Ok(())
            }
            WorkbenchIntent::SetActivity { activity } => {
                self.active_activity = activity;
                Ok(())
            }
            WorkbenchIntent::SetBottomPanel { open, active_view } => {
                self.bottom_panel = WorkbenchBottomPanelSnapshot { open, active_view };
                Ok(())
            }
        }?;
        self.require_invariants()
    }

    fn open_document(
        &mut self,
        relative_path: String,
        group_id: WorkbenchGroupId,
        surface: WorkbenchSurface,
        pinned: bool,
    ) -> Result<(), String> {
        let relative_path = normalize_project_relative_path(&relative_path)?;
        let document_id = document_id(&relative_path);
        let title = document_title(&relative_path);
        let total_documents = self
            .groups
            .iter()
            .map(|group| group.documents.len())
            .sum::<usize>();
        let group = self.require_group_mut(group_id)?;
        if let Some(document) = group
            .documents
            .iter_mut()
            .find(|document| document.document_id == document_id)
        {
            document.surface = surface;
            document.pinned |= pinned;
            group.active_document_id = Some(document_id);
            self.active_group_id = group_id;
            return Ok(());
        }
        if total_documents >= WORKBENCH_MAX_OPEN_DOCUMENTS {
            return Err(format!(
                "Workbench permite cel mult {WORKBENCH_MAX_OPEN_DOCUMENTS} documente deschise."
            ));
        }
        group.documents.push(WorkbenchDocumentSnapshot {
            document_id: document_id.clone(),
            relative_path,
            title,
            surface,
            pinned,
        });
        group.active_document_id = Some(document_id);
        self.active_group_id = group_id;
        Ok(())
    }

    fn activate_document(
        &mut self,
        document_id: &str,
        group_id: WorkbenchGroupId,
    ) -> Result<(), String> {
        let group = self.require_group_mut(group_id)?;
        if !group
            .documents
            .iter()
            .any(|document| document.document_id == document_id)
        {
            return Err(format!(
                "Documentul {document_id} nu este deschis în grupul {group_id:?}."
            ));
        }
        group.active_document_id = Some(document_id.to_string());
        self.active_group_id = group_id;
        Ok(())
    }

    fn close_document(
        &mut self,
        document_id: &str,
        group_id: WorkbenchGroupId,
    ) -> Result<(), String> {
        let group = self.require_group_mut(group_id)?;
        let index = group
            .documents
            .iter()
            .position(|document| document.document_id == document_id)
            .ok_or_else(|| {
                format!("Documentul {document_id} nu este deschis în grupul {group_id:?}.")
            })?;
        let was_active = group.active_document_id.as_deref() == Some(document_id);
        group.documents.remove(index);
        if was_active {
            group.active_document_id = group
                .documents
                .get(index.saturating_sub(1))
                .or_else(|| group.documents.get(index))
                .map(|document| document.document_id.clone());
        }
        Ok(())
    }

    fn move_document(
        &mut self,
        document_id: &str,
        from_group_id: WorkbenchGroupId,
        to_group_id: WorkbenchGroupId,
        index: Option<usize>,
    ) -> Result<(), String> {
        self.require_group(to_group_id)?;
        let document = {
            let source = self.require_group_mut(from_group_id)?;
            let source_index = source
                .documents
                .iter()
                .position(|document| document.document_id == document_id)
                .ok_or_else(|| {
                    format!("Documentul {document_id} nu este deschis în grupul {from_group_id:?}.")
                })?;
            let document = source.documents.remove(source_index);
            if source.active_document_id.as_deref() == Some(document_id) {
                source.active_document_id = source
                    .documents
                    .get(source_index.saturating_sub(1))
                    .or_else(|| source.documents.get(source_index))
                    .map(|candidate| candidate.document_id.clone());
            }
            document
        };

        let target = self.require_group_mut(to_group_id)?;
        if let Some(existing) = target
            .documents
            .iter()
            .position(|candidate| candidate.document_id == document_id)
        {
            target.documents.remove(existing);
        }
        let target_index = index
            .unwrap_or(target.documents.len())
            .min(target.documents.len());
        target.documents.insert(target_index, document);
        target.active_document_id = Some(document_id.to_string());
        self.active_group_id = to_group_id;
        Ok(())
    }

    fn set_document_surface(
        &mut self,
        document_id: &str,
        group_id: WorkbenchGroupId,
        surface: WorkbenchSurface,
    ) -> Result<(), String> {
        let group = self.require_group_mut(group_id)?;
        let document = group
            .documents
            .iter_mut()
            .find(|document| document.document_id == document_id)
            .ok_or_else(|| {
                format!("Documentul {document_id} nu este deschis în grupul {group_id:?}.")
            })?;
        document.surface = surface;
        Ok(())
    }

    fn set_split(&mut self, split: WorkbenchSplit) -> Result<(), String> {
        if self.split == split {
            return Ok(());
        }
        if split == WorkbenchSplit::None {
            self.collapse_secondary_group()?;
            self.split = WorkbenchSplit::None;
            self.active_group_id = WorkbenchGroupId::Primary;
            return Ok(());
        }
        if self.group(WorkbenchGroupId::Secondary).is_none() {
            self.groups.push(empty_group(WorkbenchGroupId::Secondary));
        }
        self.split = split;
        Ok(())
    }

    fn configure_synchronized_split(
        &mut self,
        split: WorkbenchSplit,
        relative_path: String,
        secondary_surface: WorkbenchSurface,
    ) -> Result<(), String> {
        if split == WorkbenchSplit::None {
            return Err(
                "Un split sincronizat trebuie să aibă orientare verticală sau orizontală."
                    .to_string(),
            );
        }
        let relative_path = normalize_project_relative_path(&relative_path)?;
        self.set_split(split)?;
        self.open_document(
            relative_path.clone(),
            WorkbenchGroupId::Primary,
            WorkbenchSurface::Visual,
            false,
        )?;
        self.open_document(
            relative_path,
            WorkbenchGroupId::Secondary,
            secondary_surface,
            false,
        )
    }

    fn set_split_ratio(&mut self, ratio_basis_points: u16) -> Result<(), String> {
        if !(WORKBENCH_MIN_SPLIT_RATIO_BASIS_POINTS..=WORKBENCH_MAX_SPLIT_RATIO_BASIS_POINTS)
            .contains(&ratio_basis_points)
        {
            return Err(format!(
                "Proporția split trebuie să fie între {}% și {}%.",
                WORKBENCH_MIN_SPLIT_RATIO_BASIS_POINTS / 100,
                WORKBENCH_MAX_SPLIT_RATIO_BASIS_POINTS / 100,
            ));
        }
        self.split_ratio_basis_points = ratio_basis_points;
        Ok(())
    }

    fn collapse_secondary_group(&mut self) -> Result<(), String> {
        let secondary_index = self
            .groups
            .iter()
            .position(|group| group.group_id == WorkbenchGroupId::Secondary);
        let Some(secondary_index) = secondary_index else {
            return Ok(());
        };
        let secondary = self.groups.remove(secondary_index);
        let secondary_active = secondary.active_document_id.clone();
        let secondary_was_active = self.active_group_id == WorkbenchGroupId::Secondary;
        let primary = self.require_group_mut(WorkbenchGroupId::Primary)?;
        for document in secondary.documents {
            if !primary
                .documents
                .iter()
                .any(|candidate| candidate.document_id == document.document_id)
            {
                primary.documents.push(document);
            }
        }
        if secondary_was_active {
            primary.active_document_id = secondary_active
                .filter(|document_id| {
                    primary
                        .documents
                        .iter()
                        .any(|document| &document.document_id == document_id)
                })
                .or_else(|| primary.active_document_id.clone());
        }
        Ok(())
    }

    fn group(&self, group_id: WorkbenchGroupId) -> Option<&WorkbenchGroupSnapshot> {
        self.groups.iter().find(|group| group.group_id == group_id)
    }

    fn require_group(&self, group_id: WorkbenchGroupId) -> Result<&WorkbenchGroupSnapshot, String> {
        self.group(group_id).ok_or_else(|| {
            format!("Grupul {group_id:?} nu este disponibil în layout-ul Workbench curent.")
        })
    }

    fn require_group_mut(
        &mut self,
        group_id: WorkbenchGroupId,
    ) -> Result<&mut WorkbenchGroupSnapshot, String> {
        self.groups
            .iter_mut()
            .find(|group| group.group_id == group_id)
            .ok_or_else(|| {
                format!("Grupul {group_id:?} nu este disponibil în layout-ul Workbench curent.")
            })
    }

    fn require_invariants(&self) -> Result<(), String> {
        if self.group(WorkbenchGroupId::Primary).is_none() {
            return Err("Workbench nu poate exista fără grupul primary.".to_string());
        }
        let has_secondary = self.group(WorkbenchGroupId::Secondary).is_some();
        if (self.split == WorkbenchSplit::None) == has_secondary {
            return Err("Grupurile Workbench nu corespund modului split.".to_string());
        }
        if !(WORKBENCH_MIN_SPLIT_RATIO_BASIS_POINTS..=WORKBENCH_MAX_SPLIT_RATIO_BASIS_POINTS)
            .contains(&self.split_ratio_basis_points)
        {
            return Err("Proporția split din Workbench este invalidă.".to_string());
        }
        validate_canvas_viewport(&self.canvas_viewport)?;
        let total_documents = self
            .groups
            .iter()
            .map(|group| group.documents.len())
            .sum::<usize>();
        if total_documents > WORKBENCH_MAX_OPEN_DOCUMENTS {
            return Err("Workbench a depășit limita de documente deschise.".to_string());
        }
        for group in &self.groups {
            let mut document_ids = std::collections::HashSet::new();
            for document in &group.documents {
                let normalized = normalize_project_relative_path(&document.relative_path)?;
                if normalized != document.relative_path
                    || document.document_id != document_id(&document.relative_path)
                    || !document_ids.insert(document.document_id.as_str())
                {
                    return Err(format!(
                        "Grupul {:?} conține o identitate de document invalidă.",
                        group.group_id
                    ));
                }
            }
            if group.active_document_id.as_ref().is_some_and(|active| {
                !group
                    .documents
                    .iter()
                    .any(|document| &document.document_id == active)
            }) {
                return Err(format!(
                    "Grupul {:?} are un document activ care nu este deschis.",
                    group.group_id
                ));
            }
        }
        self.require_group(self.active_group_id)?;
        Ok(())
    }
}

fn validate_canvas_viewport(viewport: &WorkbenchCanvasViewportSnapshot) -> Result<(), String> {
    if !(WORKBENCH_MIN_VIEWPORT_WIDTH_PX..=WORKBENCH_MAX_VIEWPORT_WIDTH_PX)
        .contains(&viewport.width_px)
    {
        return Err(format!(
            "Lățimea canvas-ului trebuie să fie între {WORKBENCH_MIN_VIEWPORT_WIDTH_PX}px și {WORKBENCH_MAX_VIEWPORT_WIDTH_PX}px."
        ));
    }
    if !(WORKBENCH_MIN_VIEWPORT_ZOOM_PERCENT..=WORKBENCH_MAX_VIEWPORT_ZOOM_PERCENT)
        .contains(&viewport.zoom_percent)
    {
        return Err(format!(
            "Zoom-ul canvas-ului trebuie să fie între {WORKBENCH_MIN_VIEWPORT_ZOOM_PERCENT}% și {WORKBENCH_MAX_VIEWPORT_ZOOM_PERCENT}%."
        ));
    }
    Ok(())
}

#[derive(Default)]
pub struct WorkbenchRuntime {
    state: Mutex<Option<WorkbenchSession>>,
}

impl WorkbenchRuntime {
    pub fn read(&self, session: &ProjectSessionSnapshot) -> Result<WorkbenchSnapshot, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "WorkbenchRuntime mutex este compromis.".to_string())?;
        let current = ensure_bound_session(&mut state, session);
        current.require_invariants()?;
        Ok(current.snapshot())
    }

    pub fn apply(
        &self,
        session: &ProjectSessionSnapshot,
        identity: &WorkbenchIdentity,
        intent: WorkbenchIntent,
    ) -> Result<WorkbenchCommandReceipt, String> {
        self.apply_persisted(session, identity, intent, |_| Ok(()))
    }

    pub fn read_or_restore(
        &self,
        session: &ProjectSessionSnapshot,
        restore: impl FnOnce() -> Result<Option<WorkbenchSnapshot>, String>,
    ) -> Result<WorkbenchSnapshot, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "WorkbenchRuntime mutex este compromis.".to_string())?;
        let same_project = state
            .as_ref()
            .is_some_and(|current| current.project_session_id == session.id);
        if !same_project {
            *state = Some(match restore()? {
                Some(snapshot) => WorkbenchSession::from_persisted(session, snapshot)?,
                None => WorkbenchSession::new(session),
            });
        } else if let Some(current) = state.as_mut() {
            current.rebind(session);
        }
        let current = state
            .as_ref()
            .expect("WorkbenchSession was restored or initialized");
        current.require_invariants()?;
        Ok(current.snapshot())
    }

    pub fn apply_persisted(
        &self,
        session: &ProjectSessionSnapshot,
        identity: &WorkbenchIdentity,
        intent: WorkbenchIntent,
        persist: impl FnOnce(&WorkbenchSnapshot) -> Result<(), String>,
    ) -> Result<WorkbenchCommandReceipt, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "WorkbenchRuntime mutex este compromis.".to_string())?;
        let current = ensure_bound_session(&mut state, session);
        current.require_identity(identity)?;
        let before = current.snapshot();
        let mut candidate = current.clone();
        candidate.apply(intent)?;
        let changed = candidate.snapshot() != before;
        if changed {
            candidate.revision = current.revision.saturating_add(1);
            persist(&candidate.snapshot())?;
            *current = candidate;
        }
        let snapshot = current.snapshot();
        Ok(WorkbenchCommandReceipt {
            schema_version: WORKBENCH_COMMAND_SCHEMA_VERSION,
            changed,
            project_root: snapshot.project_root.clone(),
            runtime_session_id: snapshot.runtime_session_id.clone(),
            revision_before: before.revision,
            revision_after: snapshot.revision,
            snapshot,
        })
    }
}

fn ensure_bound_session<'a>(
    state: &'a mut Option<WorkbenchSession>,
    session: &ProjectSessionSnapshot,
) -> &'a mut WorkbenchSession {
    let same_project = state
        .as_ref()
        .is_some_and(|current| current.project_session_id == session.id);
    if !same_project {
        *state = Some(WorkbenchSession::new(session));
    } else if let Some(current) = state.as_mut() {
        if current.project_root != session.project_root
            || current.runtime_session_id != session.runtime_instance_id()
        {
            current.rebind(session);
        }
    }
    state.as_mut().expect("WorkbenchSession was initialized")
}

fn empty_group(group_id: WorkbenchGroupId) -> WorkbenchGroupSnapshot {
    WorkbenchGroupSnapshot {
        group_id,
        documents: Vec::new(),
        active_document_id: None,
    }
}

fn document_id(relative_path: &str) -> String {
    format!("project:{relative_path}")
}

fn document_title(relative_path: &str) -> String {
    relative_path
        .rsplit('/')
        .next()
        .unwrap_or(relative_path)
        .to_string()
}

#[cfg(test)]
mod tests {
    use crate::kernel::project_session::{
        ProjectRootFingerprint, ProjectSessionScanSummary, ProjectSessionSnapshot,
    };

    use super::*;
    use crate::kernel::workbench::model::{
        WorkbenchBottomPanelView, WorkbenchCanvasMode, WorkbenchCanvasPreset, WorkbenchIntent,
        WorkbenchSplit, WorkbenchSurface,
    };

    fn session(id: &str, root: &str, opened_at_ms: u128) -> ProjectSessionSnapshot {
        ProjectSessionSnapshot {
            schema_version: 1,
            id: id.to_string(),
            project_root: root.to_string(),
            zola_root: root.to_string(),
            session_dir: format!("/tmp/{id}"),
            manifest_path: format!("/tmp/{id}/manifest.json"),
            opened_at_ms,
            last_seen_at_ms: opened_at_ms,
            root_fingerprint: ProjectRootFingerprint {
                canonical_path: root.to_string(),
                modified_ms: 0,
                size: 0,
                readonly: false,
                unix_device: None,
                unix_inode: None,
            },
            scan_summary: ProjectSessionScanSummary {
                is_zola: true,
                is_empty: false,
                active_theme: None,
                file_count: 1,
                directory_count: 1,
            },
        }
    }

    fn identity(snapshot: &WorkbenchSnapshot) -> WorkbenchIdentity {
        WorkbenchIdentity {
            expected_project_root: snapshot.project_root.clone(),
            expected_runtime_session_id: snapshot.runtime_session_id.clone(),
            expected_revision: snapshot.revision,
        }
    }

    #[test]
    fn initializes_a_session_bound_default_snapshot() {
        let runtime = WorkbenchRuntime::default();
        let project = session("project-a", "/project/a", 10);
        let snapshot = runtime.read(&project).unwrap();

        assert_eq!(snapshot.schema_version, WORKBENCH_SCHEMA_VERSION);
        assert_eq!(snapshot.project_session_id, "project-a");
        assert_eq!(snapshot.runtime_session_id, project.runtime_instance_id());
        assert_eq!(snapshot.split, WorkbenchSplit::None);
        assert_eq!(snapshot.groups.len(), 1);
    }

    #[test]
    fn opens_and_normalizes_project_documents() {
        let runtime = WorkbenchRuntime::default();
        let project = session("project-a", "/project/a", 10);
        let before = runtime.read(&project).unwrap();
        let receipt = runtime
            .apply(
                &project,
                &identity(&before),
                WorkbenchIntent::OpenDocument {
                    relative_path: " ./sursa\\templates\\index.html ".to_string(),
                    group_id: WorkbenchGroupId::Primary,
                    surface: WorkbenchSurface::Code,
                    pinned: false,
                },
            )
            .unwrap();

        assert!(receipt.changed);
        assert_eq!(receipt.revision_after, 1);
        let document = &receipt.snapshot.groups[0].documents[0];
        assert_eq!(document.relative_path, "sursa/templates/index.html");
        assert_eq!(document.document_id, "project:sursa/templates/index.html");
        assert_eq!(document.title, "index.html");
    }

    #[test]
    fn rejects_stale_revisions_without_mutating_state() {
        let runtime = WorkbenchRuntime::default();
        let project = session("project-a", "/project/a", 10);
        let before = runtime.read(&project).unwrap();
        let mut stale = identity(&before);
        stale.expected_revision = 9;

        let error = runtime
            .apply(
                &project,
                &stale,
                WorkbenchIntent::SetActivity {
                    activity: WorkbenchActivity::Assets,
                },
            )
            .unwrap_err();

        assert!(error.contains("revizia stale"));
        assert_eq!(runtime.read(&project).unwrap(), before);
    }

    #[test]
    fn no_op_intents_do_not_advance_revision() {
        let runtime = WorkbenchRuntime::default();
        let project = session("project-a", "/project/a", 10);
        let before = runtime.read(&project).unwrap();
        let receipt = runtime
            .apply(
                &project,
                &identity(&before),
                WorkbenchIntent::SetActivity {
                    activity: WorkbenchActivity::Editor,
                },
            )
            .unwrap();

        assert!(!receipt.changed);
        assert_eq!(receipt.revision_after, receipt.revision_before);
    }

    #[test]
    fn split_groups_can_hold_synchronized_surfaces() {
        let runtime = WorkbenchRuntime::default();
        let project = session("project-a", "/project/a", 10);
        let before = runtime.read(&project).unwrap();
        let split = runtime
            .apply(
                &project,
                &identity(&before),
                WorkbenchIntent::ConfigureSynchronizedSplit {
                    split: WorkbenchSplit::Vertical,
                    relative_path: "templates/index.html".to_string(),
                    secondary_surface: WorkbenchSurface::Code,
                },
            )
            .unwrap();

        assert_eq!(split.snapshot.groups.len(), 2);
        assert_eq!(split.revision_after, before.revision + 1);
        assert_eq!(
            split.snapshot.groups[0].documents[0].surface,
            WorkbenchSurface::Visual
        );
        assert_eq!(
            split.snapshot.groups[1].documents[0].surface,
            WorkbenchSurface::Code
        );

        let resized = runtime
            .apply(
                &project,
                &identity(&split.snapshot),
                WorkbenchIntent::SetSplitRatio {
                    ratio_basis_points: 6_250,
                },
            )
            .unwrap();
        assert_eq!(resized.snapshot.split_ratio_basis_points, 6_250);
    }

    #[test]
    fn split_ratio_rejects_values_outside_product_bounds() {
        let runtime = WorkbenchRuntime::default();
        let project = session("project-a", "/project/a", 10);
        let before = runtime.read(&project).unwrap();
        let error = runtime
            .apply(
                &project,
                &identity(&before),
                WorkbenchIntent::SetSplitRatio {
                    ratio_basis_points: 9_000,
                },
            )
            .unwrap_err();

        assert!(error.contains("între 20% și 80%"));
        assert_eq!(runtime.read(&project).unwrap(), before);
    }

    #[test]
    fn canvas_viewport_is_validated_and_owned_by_workbench() {
        let runtime = WorkbenchRuntime::default();
        let project = session("project-a", "/project/a", 10);
        let before = runtime.read(&project).unwrap();
        let viewport = WorkbenchCanvasViewportSnapshot {
            mode: WorkbenchCanvasMode::Fixed,
            preset: WorkbenchCanvasPreset::Custom,
            width_px: 1_136,
            zoom_percent: 75,
            show_rulers: false,
        };
        let receipt = runtime
            .apply(
                &project,
                &identity(&before),
                WorkbenchIntent::SetCanvasViewport {
                    viewport: viewport.clone(),
                },
            )
            .unwrap();
        assert_eq!(receipt.snapshot.canvas_viewport, viewport);

        let error = runtime
            .apply(
                &project,
                &identity(&receipt.snapshot),
                WorkbenchIntent::SetCanvasViewport {
                    viewport: WorkbenchCanvasViewportSnapshot {
                        width_px: 200,
                        ..WorkbenchCanvasViewportSnapshot::default()
                    },
                },
            )
            .unwrap_err();
        assert!(error.contains("între 320px și 3840px"));
        assert_eq!(runtime.read(&project).unwrap(), receipt.snapshot);
    }

    #[test]
    fn collapsing_split_merges_unique_documents() {
        let runtime = WorkbenchRuntime::default();
        let project = session("project-a", "/project/a", 10);
        let mut snapshot = runtime.read(&project).unwrap();
        for intent in [
            WorkbenchIntent::SetSplit {
                split: WorkbenchSplit::Horizontal,
            },
            WorkbenchIntent::OpenDocument {
                relative_path: "templates/index.html".to_string(),
                group_id: WorkbenchGroupId::Primary,
                surface: WorkbenchSurface::Visual,
                pinned: false,
            },
            WorkbenchIntent::OpenDocument {
                relative_path: "sass/app.scss".to_string(),
                group_id: WorkbenchGroupId::Secondary,
                surface: WorkbenchSurface::Code,
                pinned: false,
            },
            WorkbenchIntent::SetSplit {
                split: WorkbenchSplit::None,
            },
        ] {
            snapshot = runtime
                .apply(&project, &identity(&snapshot), intent)
                .unwrap()
                .snapshot;
        }

        assert_eq!(snapshot.groups.len(), 1);
        assert_eq!(snapshot.groups[0].documents.len(), 2);
        assert_eq!(snapshot.active_group_id, WorkbenchGroupId::Primary);
    }

    #[test]
    fn bottom_panel_state_is_rust_owned() {
        let runtime = WorkbenchRuntime::default();
        let project = session("project-a", "/project/a", 10);
        let before = runtime.read(&project).unwrap();
        let receipt = runtime
            .apply(
                &project,
                &identity(&before),
                WorkbenchIntent::SetBottomPanel {
                    open: true,
                    active_view: WorkbenchBottomPanelView::Terminal,
                },
            )
            .unwrap();

        assert!(receipt.snapshot.bottom_panel.open);
        assert_eq!(
            receipt.snapshot.bottom_panel.active_view,
            WorkbenchBottomPanelView::Terminal
        );
    }

    #[test]
    fn rebind_preserves_same_project_and_resets_different_project() {
        let runtime = WorkbenchRuntime::default();
        let first_runtime = session("project-a", "/project/a", 10);
        let before = runtime.read(&first_runtime).unwrap();
        runtime
            .apply(
                &first_runtime,
                &identity(&before),
                WorkbenchIntent::OpenDocument {
                    relative_path: "templates/index.html".to_string(),
                    group_id: WorkbenchGroupId::Primary,
                    surface: WorkbenchSurface::Visual,
                    pinned: false,
                },
            )
            .unwrap();

        let second_runtime = session("project-a", "/project/a", 20);
        let rebound = runtime.read(&second_runtime).unwrap();
        assert_eq!(rebound.groups[0].documents.len(), 1);
        assert_eq!(
            rebound.runtime_session_id,
            second_runtime.runtime_instance_id()
        );

        let other_project = session("project-b", "/project/b", 30);
        let reset = runtime.read(&other_project).unwrap();
        assert!(reset.groups[0].documents.is_empty());
        assert_eq!(reset.revision, 0);
    }

    #[test]
    fn rejects_paths_outside_the_project_boundary() {
        let runtime = WorkbenchRuntime::default();
        let project = session("project-a", "/project/a", 10);
        let before = runtime.read(&project).unwrap();
        let error = runtime
            .apply(
                &project,
                &identity(&before),
                WorkbenchIntent::OpenDocument {
                    relative_path: "../secret".to_string(),
                    group_id: WorkbenchGroupId::Primary,
                    surface: WorkbenchSurface::Code,
                    pinned: false,
                },
            )
            .unwrap_err();

        assert!(error.contains("nu este relativ sigur"));
        assert_eq!(runtime.read(&project).unwrap(), before);
    }

    #[test]
    fn persistence_failure_keeps_the_previous_canonical_state() {
        let runtime = WorkbenchRuntime::default();
        let project = session("project-a", "/project/a", 10);
        let before = runtime.read(&project).unwrap();
        let error = runtime
            .apply_persisted(
                &project,
                &identity(&before),
                WorkbenchIntent::SetActivity {
                    activity: WorkbenchActivity::Assets,
                },
                |_| Err("disk unavailable".to_string()),
            )
            .unwrap_err();

        assert_eq!(error, "disk unavailable");
        assert_eq!(runtime.read(&project).unwrap(), before);
    }

    #[test]
    fn a_fresh_runtime_restores_and_rebinds_a_persisted_projection() {
        let source_runtime = WorkbenchRuntime::default();
        let first_session = session("project-a", "/project/a", 10);
        let before = source_runtime.read(&first_session).unwrap();
        let persisted = source_runtime
            .apply(
                &first_session,
                &identity(&before),
                WorkbenchIntent::OpenDocument {
                    relative_path: "templates/index.html".to_string(),
                    group_id: WorkbenchGroupId::Primary,
                    surface: WorkbenchSurface::Visual,
                    pinned: false,
                },
            )
            .unwrap()
            .snapshot;

        let restored_runtime = WorkbenchRuntime::default();
        let next_session = session("project-a", "/project/a", 20);
        let restored = restored_runtime
            .read_or_restore(&next_session, || Ok(Some(persisted.clone())))
            .unwrap();

        assert_eq!(restored.groups[0].documents.len(), 1);
        assert_eq!(restored.revision, persisted.revision);
        assert_eq!(
            restored.runtime_session_id,
            next_session.runtime_instance_id()
        );
    }
}
