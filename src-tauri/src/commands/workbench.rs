use tauri::{AppHandle, State};

use crate::{
    kernel::{
        project_workspace::ProjectWorkspace,
        workbench::{
            persist_workbench, read_persisted_workbench, WorkbenchCommandReceipt,
            WorkbenchDocumentPresentation, WorkbenchDocumentPresentationEntry, WorkbenchIdentity,
            WorkbenchIntent, WorkbenchSnapshot, WorkbenchSurface,
        },
    },
    state::AppState,
};

#[tauri::command]
pub fn read_workbench_state(
    app: AppHandle,
    state: State<AppState>,
) -> Result<Option<WorkbenchSnapshot>, String> {
    let Some((session, presentations)) = workbench_authority(&state)? else {
        return Ok(None);
    };
    state
        .workbench
        .read_or_restore(&session, || read_persisted_workbench(&session))?;
    state
        .workbench
        .apply_latest_persisted(
            &session,
            WorkbenchIntent::ReconcileDocumentPresentations {
                documents: presentations,
            },
            |snapshot| persist_workbench(&app, &session, snapshot),
        )
        .map(|receipt| Some(receipt.snapshot))
}

#[tauri::command]
pub fn apply_workbench_intent(
    identity: WorkbenchIdentity,
    intent: WorkbenchIntent,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkbenchCommandReceipt, String> {
    let (session, presentations) = workbench_authority(&state)?
        .ok_or_else(|| "Workbench nu poate aplica intenții fără un proiect activ.".to_string())?;
    let intent = bind_intent_to_authority(intent, &presentations)?;
    state
        .workbench
        .read_or_restore(&session, || read_persisted_workbench(&session))?;
    if intent_uses_projection_write_behind(&intent) {
        let receipt = state.workbench.apply(&session, &identity, intent)?;
        if receipt.changed {
            let snapshot = receipt.snapshot.clone();
            let revision = snapshot.revision;
            if let Err(error) =
                state
                    .workbench_projection_persistence
                    .schedule(app.clone(), session, snapshot)
            {
                eprintln!(
                    "[Pană Studio] Workbench projection write-behind scheduling failed at revision {}: {}",
                    revision, error
                );
            }
        }
        return Ok(receipt);
    }
    state
        .workbench
        .apply_persisted(&session, &identity, intent, |snapshot| {
            persist_workbench(&app, &session, snapshot)
        })
}

fn workbench_authority(
    state: &State<AppState>,
) -> Result<
    Option<(
        crate::kernel::project_session::ProjectSessionSnapshot,
        Vec<WorkbenchDocumentPresentationEntry>,
    )>,
    String,
> {
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru Workbench.".to_string())?;
    Ok(workspace.as_ref().map(|workspace| {
        (
            workspace.session.clone(),
            authoritative_presentations(workspace),
        )
    }))
}

fn authoritative_presentations(
    workspace: &ProjectWorkspace,
) -> Vec<WorkbenchDocumentPresentationEntry> {
    workspace
        .documents
        .files
        .values()
        .map(|entry| WorkbenchDocumentPresentationEntry {
            relative_path: entry.relative_path.clone(),
            presentation: WorkbenchDocumentPresentation::from_text_language(entry.language),
        })
        .collect()
}

fn bind_intent_to_authority(
    intent: WorkbenchIntent,
    presentations: &[WorkbenchDocumentPresentationEntry],
) -> Result<WorkbenchIntent, String> {
    let presentation_for = |relative_path: &str| {
        presentations
            .iter()
            .find(|entry| entry.relative_path == relative_path)
            .map(|entry| entry.presentation)
            .ok_or_else(|| {
                format!(
                    "Workbench nu găsește documentul {relative_path} în autoritatea ProjectWorkspace."
                )
            })
    };
    Ok(match intent {
        WorkbenchIntent::OpenDocument {
            relative_path,
            group_id,
            surface,
            pinned,
            ..
        } => {
            let presentation = presentation_for(&relative_path)?;
            WorkbenchIntent::OpenDocument {
                relative_path,
                group_id,
                surface: if presentation.supports_visual() {
                    surface
                } else {
                    WorkbenchSurface::Code
                },
                presentation,
                pinned,
            }
        }
        WorkbenchIntent::SelectProjectEntry {
            relative_path,
            entry_kind,
            open_surface,
            ..
        } => {
            let open_presentation = open_surface
                .map(|_| presentation_for(&relative_path))
                .transpose()?;
            let open_surface = match (open_surface, open_presentation) {
                (Some(surface), Some(presentation)) if presentation.supports_visual() => {
                    Some(surface)
                }
                (Some(_), Some(_)) => Some(WorkbenchSurface::Code),
                (surface, _) => surface,
            };
            WorkbenchIntent::SelectProjectEntry {
                relative_path,
                entry_kind,
                open_surface,
                open_presentation,
            }
        }
        WorkbenchIntent::ConfigureSynchronizedSplit {
            split,
            relative_path,
            secondary_surface,
            ..
        } => WorkbenchIntent::ConfigureSynchronizedSplit {
            split,
            presentation: presentation_for(&relative_path)?,
            relative_path,
            secondary_surface,
        },
        WorkbenchIntent::ReconcileDocumentPresentations { .. } => {
            WorkbenchIntent::ReconcileDocumentPresentations {
                documents: presentations.to_vec(),
            }
        }
        intent => intent,
    })
}

fn intent_uses_projection_write_behind(intent: &WorkbenchIntent) -> bool {
    matches!(
        intent,
        WorkbenchIntent::SetActivity { .. } | WorkbenchIntent::ActivateDocument { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::workbench::WorkbenchGroupId;

    #[test]
    fn frontend_cannot_spoof_visual_capability_for_non_html_document() {
        let presentations = vec![WorkbenchDocumentPresentationEntry {
            relative_path: "sass/site.scss".to_string(),
            presentation: WorkbenchDocumentPresentation::CodeOnly,
        }];

        let bound = bind_intent_to_authority(
            WorkbenchIntent::OpenDocument {
                relative_path: "sass/site.scss".to_string(),
                group_id: WorkbenchGroupId::Secondary,
                surface: WorkbenchSurface::Visual,
                presentation: WorkbenchDocumentPresentation::Html,
                pinned: false,
            },
            &presentations,
        )
        .unwrap();

        assert_eq!(
            bound,
            WorkbenchIntent::OpenDocument {
                relative_path: "sass/site.scss".to_string(),
                group_id: WorkbenchGroupId::Secondary,
                surface: WorkbenchSurface::Code,
                presentation: WorkbenchDocumentPresentation::CodeOnly,
                pinned: false,
            }
        );
    }

    #[test]
    fn html_capability_preserves_requested_visual_surface() {
        let presentations = vec![WorkbenchDocumentPresentationEntry {
            relative_path: "templates/index.html".to_string(),
            presentation: WorkbenchDocumentPresentation::Html,
        }];

        let bound = bind_intent_to_authority(
            WorkbenchIntent::OpenDocument {
                relative_path: "templates/index.html".to_string(),
                group_id: WorkbenchGroupId::Primary,
                surface: WorkbenchSurface::Visual,
                presentation: WorkbenchDocumentPresentation::CodeOnly,
                pinned: false,
            },
            &presentations,
        )
        .unwrap();

        assert!(matches!(
            bound,
            WorkbenchIntent::OpenDocument {
                surface: WorkbenchSurface::Visual,
                presentation: WorkbenchDocumentPresentation::Html,
                ..
            }
        ));
    }
}
