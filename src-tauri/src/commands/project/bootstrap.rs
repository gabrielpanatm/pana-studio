use crate::{
    commands::config::{project_settings_from_store, ProjectSettingsSnapshot},
    deploy::DeploySettings,
    kernel::{
        project_session::ProjectSessionSnapshot,
        project_workspace::{ProjectWorkspace, ProjectWorkspaceSnapshot},
        workbench::{
            read_persisted_workbench, WorkbenchActivity, WorkbenchBottomPanelView,
            WorkbenchDocumentPresentation, WorkbenchDocumentPresentationEntry, WorkbenchGroupId,
            WorkbenchIdentity, WorkbenchIntent, WorkbenchRuntime, WorkbenchSnapshot,
            WorkbenchSplit, WorkbenchSurface,
        },
    },
    project::{
        apply_project_model_preview_routes, ProjectFile, ProjectFileKind, ProjectFileRole,
        ProjectLifecycleSnapshot, ProjectScan, PROJECT_OPEN_BOOTSTRAP_SCHEMA_VERSION,
    },
    project_model::model::ProjectModel,
};

use super::contracts::{
    ProjectBootstrapDocument, ProjectBootstrapInitialSurface, ProjectBootstrapSourceLocation,
    ProjectOpenBootstrapReceipt,
};

pub(super) struct ProjectBootstrapAssembler {
    pub project: ProjectScan,
    workspace: ProjectWorkspaceSnapshot,
    project_settings: ProjectSettingsSnapshot,
    deploy_settings: DeploySettings,
    active_document: Option<ProjectBootstrapDocument>,
    target_css_file: Option<String>,
}

impl ProjectBootstrapAssembler {
    pub(super) fn prepare(
        mut project: ProjectScan,
        workspace: &ProjectWorkspace,
        workbench: &WorkbenchSnapshot,
        project_model: Option<&ProjectModel>,
        diagnostic_target: Option<&(String, ProjectBootstrapSourceLocation)>,
    ) -> Result<Self, String> {
        if let Some(model) = project_model {
            apply_project_model_preview_routes(
                &mut project,
                model
                    .source_graph
                    .pages
                    .iter()
                    .map(|page| (page.file.as_str(), page.url.as_str())),
            );
        }
        let active_document = initial_project_file(&project, workbench).and_then(|file| {
            workspace
                .documents
                .text_for(&file.relative_path)
                .map(|source| ProjectBootstrapDocument {
                    relative_path: file.relative_path.clone(),
                    source,
                    preview_path: file.preview_path.clone(),
                    diagnostic_location: diagnostic_target
                        .filter(|(relative_path, _)| relative_path == &file.relative_path)
                        .map(|(_, location)| *location),
                })
        });
        let target_css_file = project
            .files
            .iter()
            .find(|file| {
                matches!(file.kind, ProjectFileKind::Css | ProjectFileKind::Scss)
                    && file.role == ProjectFileRole::Style
            })
            .map(|file| file.relative_path.clone());
        Ok(Self {
            project,
            workspace: workspace.snapshot(),
            project_settings: project_settings_from_store(
                &workspace.documents,
                workspace.revision,
            )?,
            deploy_settings: crate::deploy::read_deploy_settings_from_store(
                &workspace.documents,
                workspace.revision,
            )?,
            active_document,
            target_css_file,
        })
    }

    pub(super) fn finish(
        self,
        lifecycle: ProjectLifecycleSnapshot,
        workbench: WorkbenchSnapshot,
        initial_surface: Option<ProjectBootstrapInitialSurface>,
    ) -> ProjectOpenBootstrapReceipt {
        ProjectOpenBootstrapReceipt {
            schema_version: PROJECT_OPEN_BOOTSTRAP_SCHEMA_VERSION,
            project: self.project,
            lifecycle,
            workspace: self.workspace,
            project_settings: self.project_settings,
            deploy_settings: self.deploy_settings,
            workbench,
            active_document: self.active_document,
            target_css_file: self.target_css_file,
            initial_surface,
        }
    }
}

fn active_workbench_relative_path(snapshot: &WorkbenchSnapshot) -> Option<String> {
    let group = snapshot
        .groups
        .iter()
        .find(|group| group.group_id == snapshot.active_group_id)?;
    let active_id = group.active_document_id.as_deref()?;
    group
        .documents
        .iter()
        .find(|document| document.document_id == active_id)
        .map(|document| document.relative_path.clone())
}

pub(super) fn initial_project_file<'a>(
    scan: &'a ProjectScan,
    workbench: &WorkbenchSnapshot,
) -> Option<&'a ProjectFile> {
    active_workbench_relative_path(workbench)
        .and_then(|path| scan.files.iter().find(|file| file.relative_path == path))
        .or_else(|| project_index_file(scan))
}

fn project_index_file(scan: &ProjectScan) -> Option<&ProjectFile> {
    scan.files
        .iter()
        .find(|file| file.relative_path == "templates/index.html")
        .or_else(|| {
            let active_theme = scan.active_theme.as_deref()?;
            let themed_index = format!("themes/{active_theme}/templates/index.html");
            scan.files
                .iter()
                .find(|file| file.relative_path == themed_index)
        })
        .or_else(|| {
            scan.files.iter().find(|file| {
                file.role == ProjectFileRole::Page && file.preview_path.as_deref() == Some("/")
            })
        })
        .or_else(|| {
            scan.files
                .iter()
                .find(|file| file.role == ProjectFileRole::Page)
        })
        .or_else(|| {
            scan.files.iter().find(|file| {
                !matches!(
                    file.kind,
                    ProjectFileKind::Dir | ProjectFileKind::Image | ProjectFileKind::Font
                )
            })
        })
}

fn workbench_surface_for_file(file: &ProjectFile) -> WorkbenchSurface {
    if file.kind == ProjectFileKind::Html {
        WorkbenchSurface::Visual
    } else {
        WorkbenchSurface::Code
    }
}

fn workbench_presentation_for_file(file: &ProjectFile) -> WorkbenchDocumentPresentation {
    WorkbenchDocumentPresentation::from_project_file_kind(file.kind)
}

pub(super) fn prepare_bootstrap_workbench(
    session: &ProjectSessionSnapshot,
    scan: &ProjectScan,
) -> Result<WorkbenchSnapshot, String> {
    let file = project_index_file(scan);
    prepare_bootstrap_workbench_for_file(session, scan, file, None)
}

pub(super) fn prepare_bootstrap_workbench_for_file(
    session: &ProjectSessionSnapshot,
    scan: &ProjectScan,
    file: Option<&ProjectFile>,
    surface_override: Option<WorkbenchSurface>,
) -> Result<WorkbenchSnapshot, String> {
    let runtime = WorkbenchRuntime::default();
    let mut snapshot = runtime.read_or_restore(session, || read_persisted_workbench(session))?;
    let identity = WorkbenchIdentity {
        expected_project_root: snapshot.project_root.clone(),
        expected_runtime_session_id: snapshot.runtime_session_id.clone(),
        expected_revision: snapshot.revision,
    };
    snapshot = runtime
        .apply(
            session,
            &identity,
            WorkbenchIntent::ReconcileDocumentPresentations {
                documents: scan
                    .files
                    .iter()
                    .map(|file| WorkbenchDocumentPresentationEntry {
                        relative_path: file.relative_path.clone(),
                        presentation: workbench_presentation_for_file(file),
                    })
                    .collect(),
            },
        )?
        .snapshot;
    let identity = WorkbenchIdentity {
        expected_project_root: snapshot.project_root.clone(),
        expected_runtime_session_id: snapshot.runtime_session_id.clone(),
        expected_revision: snapshot.revision,
    };
    snapshot = runtime
        .apply(
            session,
            &identity,
            WorkbenchIntent::SetBottomPanel {
                open: false,
                active_view: WorkbenchBottomPanelView::Terminal,
            },
        )?
        .snapshot;
    let Some(file) = file else {
        return Ok(snapshot);
    };
    for intent in [
        WorkbenchIntent::SetSplit {
            split: WorkbenchSplit::None,
        },
        WorkbenchIntent::SetActivity {
            activity: WorkbenchActivity::Editor,
        },
        WorkbenchIntent::OpenDocument {
            relative_path: file.relative_path.clone(),
            group_id: WorkbenchGroupId::Primary,
            surface: surface_override
                .filter(|surface| {
                    *surface != WorkbenchSurface::Visual
                        || workbench_presentation_for_file(file).supports_visual()
                })
                .unwrap_or_else(|| workbench_surface_for_file(file)),
            presentation: workbench_presentation_for_file(file),
            pinned: false,
        },
    ] {
        let identity = WorkbenchIdentity {
            expected_project_root: snapshot.project_root.clone(),
            expected_runtime_session_id: snapshot.runtime_session_id.clone(),
            expected_revision: snapshot.revision,
        };
        snapshot = runtime.apply(session, &identity, intent)?.snapshot;
    }
    Ok(snapshot)
}

pub(super) fn project_file_from_preview_diagnostic<'a>(
    scan: &'a ProjectScan,
    diagnostic: &str,
) -> Option<&'a ProjectFile> {
    // Zola reports the private projected path, not the original root. Match
    // against the authoritative workspace namespace so bootstrap remains
    // independent from the private cache session.
    scan.files
        .iter()
        .filter(|file| {
            !matches!(
                file.kind,
                ProjectFileKind::Dir | ProjectFileKind::Image | ProjectFileKind::Font
            )
        })
        .filter(|file| diagnostic.contains(&file.relative_path))
        .max_by_key(|file| file.relative_path.len())
}

pub(super) fn project_source_location_from_preview_diagnostic(
    diagnostic: &str,
    relative_path: &str,
) -> Option<ProjectBootstrapSourceLocation> {
    let path_end = diagnostic.rfind(relative_path)? + relative_path.len();
    let location = diagnostic.get(path_end..)?.strip_prefix(':')?;
    let (line, remainder) = parse_diagnostic_coordinate(location)?;
    let column = remainder
        .strip_prefix(':')
        .and_then(parse_diagnostic_coordinate)
        .map(|(column, _)| column)
        .unwrap_or(1);
    Some(ProjectBootstrapSourceLocation { line, column })
}

fn parse_diagnostic_coordinate(value: &str) -> Option<(u32, &str)> {
    let digits = value
        .char_indices()
        .take_while(|(_, character)| character.is_ascii_digit())
        .map(|(index, character)| index + character.len_utf8())
        .last()?;
    let coordinate = value.get(..digits)?.parse::<u32>().ok()?;
    (coordinate > 0).then(|| (coordinate, &value[digits..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_the_workspace_file_from_a_private_zola_projection_path() {
        let scan = ProjectScan {
            root: "/project".to_string(),
            preview_base_url: None,
            preview_warning: None,
            active_theme: None,
            files: vec![
                ProjectFile {
                    name: "index.scss".to_string(),
                    relative_path: "sass/pagini/index.scss".to_string(),
                    absolute_path: "/project/sass/pagini/index.scss".to_string(),
                    kind: ProjectFileKind::Scss,
                    role: ProjectFileRole::Style,
                    preview_path: None,
                },
                ProjectFile {
                    name: "index.html".to_string(),
                    relative_path: "templates/index.html".to_string(),
                    absolute_path: "/project/templates/index.html".to_string(),
                    kind: ProjectFileKind::Html,
                    role: ProjectFileRole::Template,
                    preview_path: None,
                },
            ],
            kernel_session_id: None,
            workspace_revision: None,
            accepted_disk_manifest: None,
            accepted_disk_generation: None,
        };
        let diagnostic = concat!(
            "Zola nu a putut randa: Expected expression. | 1170 | ",
            "//cache/preview/session/source/sass/pagini/index.scss:1170:23",
        );

        let file = project_file_from_preview_diagnostic(&scan, diagnostic)
            .expect("diagnostic source file");
        assert_eq!(file.relative_path, "sass/pagini/index.scss");
        assert_eq!(
            project_source_location_from_preview_diagnostic(diagnostic, &file.relative_path)
                .map(|location| (location.line, location.column)),
            Some((1170, 23)),
        );
    }
}
