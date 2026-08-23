use std::path::Path;

use tauri::{AppHandle, State};

use crate::{
    kernel::{
        file_buffer_store::{
            map_text_changes, now_ms as file_buffer_now_ms, require_file_buffer_session_binding,
            FileBufferChangeSetInput, FileBufferChangeSetResult, FileBufferCommandReceipt,
            FileBufferFileSnapshot, FileBufferMutationExpectation, FileBufferRequestIdentity,
            FileBufferTextSnapshot,
        },
        project_workspace::{
            commit_project_workspace_session_mutation, ProjectWorkspace, ProjectWorkspaceIdentity,
            WorkspaceDocumentMutation, WorkspaceMutationMetadata,
        },
    },
    project_model::{
        model::ProjectModel, rebuild_project_model_after_workspace_change_with_source_changes,
        ProjectModelIncrementalIntent,
    },
    source_graph::identity::{SourceChangeSet, SourceTextEdit},
    state::AppState,
};

#[tauri::command(async)]
pub fn read_file_buffer_text(
    relative_path: String,
    identity: FileBufferRequestIdentity,
    state: State<AppState>,
) -> Result<FileBufferCommandReceipt<FileBufferTextSnapshot>, String> {
    read_file_buffer_text_impl(relative_path, identity, state.inner())
}

fn read_file_buffer_text_impl(
    relative_path: String,
    identity: FileBufferRequestIdentity,
    state: &AppState,
) -> Result<FileBufferCommandReceipt<FileBufferTextSnapshot>, String> {
    with_bound_project_workspace(state, &identity, |workspace| {
        let payload = workspace
            .projected_text_snapshot(&relative_path)?
            .ok_or_else(|| {
                format!("ProjectWorkspace nu are text proiectat pentru {relative_path}.")
            })?;
        Ok(FileBufferCommandReceipt::new(
            &workspace.session,
            workspace.revision,
            payload,
        ))
    })
}

fn with_bound_project_workspace<T>(
    state: &AppState,
    identity: &FileBufferRequestIdentity,
    operation: impl FnOnce(&mut ProjectWorkspace) -> Result<T, String>,
) -> Result<T, String> {
    let current_root_guard = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul curent pentru FileBufferStore.".to_string())?;
    let current_root_path = current_root_guard
        .as_ref()
        .ok_or_else(|| "Nu există proiect curent pentru FileBufferStore.".to_string())?;
    let current_root = current_root_path.to_string_lossy().into_owned();
    let mut project_workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru FileBufferStore.".to_string())?;
    let workspace = project_workspace.as_mut().ok_or_else(|| {
        "ProjectWorkspace nu este inițializat pentru FileBufferStore.".to_string()
    })?;
    require_file_buffer_session_binding(
        &current_root,
        &workspace.session,
        &workspace.documents,
        identity,
    )?;
    operation(workspace)
}

#[tauri::command(async)]
pub fn set_file_buffer_draft(
    relative_path: String,
    contents: String,
    expectation: FileBufferMutationExpectation,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<FileBufferCommandReceipt<FileBufferFileSnapshot>, String> {
    set_file_buffer_draft_impl(
        relative_path,
        contents,
        expectation,
        identity,
        &app,
        state.inner(),
    )
}

fn set_file_buffer_draft_impl(
    relative_path: String,
    contents: String,
    expectation: FileBufferMutationExpectation,
    identity: FileBufferRequestIdentity,
    app: &AppHandle,
    state: &AppState,
) -> Result<FileBufferCommandReceipt<FileBufferFileSnapshot>, String> {
    let receipt = with_bound_project_workspace(state, &identity, |workspace| {
        let file = commit_project_workspace_session_mutation(app, workspace, |candidate| {
            let previous_model = candidate.project_model.clone();
            let previous_model_source_revision = candidate.project_model_source_revision;
            let before_source = candidate
                .projected_text_snapshot(&relative_path)?
                .ok_or_else(|| {
                    format!("ProjectWorkspace nu urmărește documentul {relative_path}.")
                })?
                .text;
            let file = candidate.stage_projected_document_text(
                &workspace_identity(candidate),
                WorkspaceMutationMetadata {
                    label: "Editare document".to_string(),
                    source: "code_editor.full_draft".to_string(),
                    coalesce_key: Some(format!("document:{relative_path}")),
                    transaction_id: None,
                },
                &relative_path,
                contents,
                &expectation,
                file_buffer_now_ms(),
            )?;
            if candidate
                .projected_text_snapshot(&relative_path)?
                .is_some_and(|snapshot| snapshot.text != before_source)
            {
                publish_code_edit_project_model(
                    candidate,
                    previous_model.as_deref(),
                    previous_model_source_revision,
                    &relative_path,
                    &before_source,
                    None,
                )?;
            }
            Ok(file)
        })?;
        Ok(FileBufferCommandReceipt::new(
            &workspace.session,
            workspace.revision,
            file,
        ))
    })?;
    invalidate_code_selection_after_commit(state, &receipt);
    Ok(receipt)
}

#[tauri::command(async)]
pub fn apply_file_buffer_changeset(
    input: FileBufferChangeSetInput,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<FileBufferCommandReceipt<FileBufferChangeSetResult>, String> {
    apply_file_buffer_changeset_impl(input, identity, &app, state.inner())
}

fn apply_file_buffer_changeset_impl(
    input: FileBufferChangeSetInput,
    identity: FileBufferRequestIdentity,
    app: &AppHandle,
    state: &AppState,
) -> Result<FileBufferCommandReceipt<FileBufferChangeSetResult>, String> {
    let receipt = with_bound_project_workspace(state, &identity, |workspace| {
        let source = input
            .source
            .clone()
            .unwrap_or_else(|| "code_editor.changeset".to_string());
        let relative_path = input.relative_path.clone();
        let result = commit_project_workspace_session_mutation(app, workspace, |candidate| {
            let previous_model = candidate.project_model.clone();
            let previous_model_source_revision = candidate.project_model_source_revision;
            let before_source = candidate
                .projected_text_snapshot(&relative_path)?
                .ok_or_else(|| {
                    format!("ProjectWorkspace nu urmărește documentul {relative_path}.")
                })?
                .text;
            let exact_edits =
                map_text_changes(&before_source, &input.changes, input.coordinate_space)?
                    .into_iter()
                    .map(|change| SourceTextEdit {
                        old_start: change.old_start,
                        old_end: change.old_end,
                        new_start: change.new_start,
                        new_end: change.new_end,
                    })
                    .collect::<Vec<_>>();
            let result = candidate.apply_projected_document_changeset(
                &workspace_identity(candidate),
                WorkspaceMutationMetadata {
                    label: "Editare document".to_string(),
                    source,
                    coalesce_key: Some(format!("document:{relative_path}")),
                    transaction_id: None,
                },
                input,
                file_buffer_now_ms(),
            )?;
            if result.applied {
                publish_code_edit_project_model(
                    candidate,
                    previous_model.as_deref(),
                    previous_model_source_revision,
                    &relative_path,
                    &before_source,
                    Some(exact_edits),
                )?;
            }
            Ok(result)
        })?;
        Ok(FileBufferCommandReceipt::new(
            &workspace.session,
            workspace.revision,
            result,
        ))
    })?;
    invalidate_code_selection_after_commit(state, &receipt);
    Ok(receipt)
}

#[tauri::command(async)]
pub fn clear_file_buffer_draft(
    relative_path: String,
    expectation: FileBufferMutationExpectation,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<FileBufferCommandReceipt<FileBufferFileSnapshot>, String> {
    clear_file_buffer_draft_impl(relative_path, expectation, identity, &app, state.inner())
}

fn clear_file_buffer_draft_impl(
    relative_path: String,
    expectation: FileBufferMutationExpectation,
    identity: FileBufferRequestIdentity,
    app: &AppHandle,
    state: &AppState,
) -> Result<FileBufferCommandReceipt<FileBufferFileSnapshot>, String> {
    let receipt = with_bound_project_workspace(state, &identity, |workspace| {
        let file = commit_project_workspace_session_mutation(app, workspace, |candidate| {
            let previous_model = candidate.project_model.clone();
            let previous_model_source_revision = candidate.project_model_source_revision;
            let before_source = candidate
                .documents
                .text_snapshot(&relative_path)
                .ok_or_else(|| {
                    format!("ProjectWorkspace nu urmărește documentul {relative_path}.")
                })?
                .text;
            candidate
                .documents
                .validate_clear_draft_if_current(&relative_path, &expectation)?;
            let baseline = candidate
                .documents
                .baseline_text_for(&relative_path)
                .ok_or_else(|| {
                    format!("ProjectWorkspace nu are baseline pentru {relative_path}.")
                })?;
            let receipt = candidate.stage_document_texts(
                &workspace_identity(candidate),
                WorkspaceMutationMetadata {
                    label: "Renunțare la modificările documentului".to_string(),
                    source: "code_editor.clear_draft".to_string(),
                    coalesce_key: None,
                    transaction_id: None,
                },
                vec![WorkspaceDocumentMutation {
                    relative_path: relative_path.clone(),
                    contents: baseline,
                }],
                file_buffer_now_ms(),
            )?;
            if receipt.changed {
                publish_code_edit_project_model(
                    candidate,
                    previous_model.as_deref(),
                    previous_model_source_revision,
                    &relative_path,
                    &before_source,
                    None,
                )?;
            }
            receipt
                .files
                .into_iter()
                .next()
                .ok_or_else(|| "ProjectWorkspace nu a returnat documentul curățat.".to_string())
        })?;
        Ok(FileBufferCommandReceipt::new(
            &workspace.session,
            workspace.revision,
            file,
        ))
    })?;
    invalidate_code_selection_after_commit(state, &receipt);
    Ok(receipt)
}

fn publish_code_edit_project_model(
    candidate: &mut ProjectWorkspace,
    previous_model: Option<&ProjectModel>,
    previous_model_source_revision: Option<u64>,
    relative_path: &str,
    actual_before_source: &str,
    exact_edits: Option<Vec<SourceTextEdit>>,
) -> Result<(), String> {
    let projection = candidate.capture_projection_snapshot()?;
    let Some(model) = build_code_edit_project_model(
        Path::new(&candidate.session.project_root),
        previous_model,
        previous_model_source_revision,
        &projection,
        relative_path,
        actual_before_source,
        exact_edits,
    )?
    else {
        // A code draft is the textual authority even while it is temporarily
        // invalid Tera. ProjectWorkspace deliberately retains the immutable
        // prior model and its source revision; freshness gates keep Canvas and
        // structural commands from consuming that stale semantic projection.
        return Ok(());
    };
    candidate.publish_project_model(&projection, model)
}

fn build_code_edit_project_model(
    project_root: &Path,
    previous_model: Option<&ProjectModel>,
    previous_model_source_revision: Option<u64>,
    projection: &crate::kernel::project_workspace::WorkspaceProjectionSnapshot,
    relative_path: &str,
    actual_before_source: &str,
    exact_edits: Option<Vec<SourceTextEdit>>,
) -> Result<Option<ProjectModel>, String> {
    let source_changes = previous_model.and_then(|model| {
        let before = model
            .files
            .iter()
            .find(|file| file.relative_path == relative_path)?;
        let after = projection.source_texts.get(relative_path)?;
        let mut change = SourceChangeSet::between(relative_path, &before.contents, after);
        if previous_model_source_revision
            .is_some_and(|revision| revision.checked_add(1) == Some(projection.revision))
            && before.contents == actual_before_source
        {
            if let Some(edits) = exact_edits {
                change = change.with_exact_text_edits(edits);
            }
        }
        Some(vec![change])
    });
    let intent = if relative_path.starts_with("templates/") && relative_path.ends_with(".html") {
        ProjectModelIncrementalIntent::HtmlStructural
    } else if relative_path.ends_with(".css") {
        ProjectModelIncrementalIntent::StyleDeclaration
    } else {
        ProjectModelIncrementalIntent::Unsupported
    };
    let build = match rebuild_project_model_after_workspace_change_with_source_changes(
        project_root,
        previous_model,
        previous_model_source_revision,
        projection,
        &[relative_path.to_string()],
        intent,
        source_changes,
    ) {
        Ok(build) => build,
        Err(error) if is_source_conformance_diagnostic(&error) => return Ok(None),
        Err(error) => return Err(error),
    };
    Ok(Some(build.model))
}

fn is_source_conformance_diagnostic(error: &str) -> bool {
    serde_json::from_str::<crate::localization::LocalizedDiagnostic>(error)
        .is_ok_and(|diagnostic| diagnostic.code.starts_with("source-graph-"))
}

fn invalidate_code_selection_after_commit<T>(
    state: &AppState,
    receipt: &FileBufferCommandReceipt<T>,
) {
    let retained = (|| -> Result<std::collections::HashSet<String>, String> {
        let workspace = state
            .project_workspace
            .lock()
            .map_err(|_| "Nu am putut valida selecția după tranzacția Code.".to_string())?;
        let workspace = workspace
            .as_ref()
            .ok_or_else(|| "ProjectWorkspace lipsește după tranzacția Code.".to_string())?;
        if workspace.runtime_session_id() != receipt.runtime_session_id
            || workspace.revision != receipt.workspace_revision
        {
            return Err("Selecția Code nu poate fi validată pe o revizie stale.".to_string());
        }
        if workspace.project_model_source_revision != Some(receipt.workspace_revision) {
            return Ok(std::collections::HashSet::new());
        }
        let model = workspace
            .project_model
            .as_ref()
            .ok_or_else(|| "ProjectModel lipsește după tranzacția Code.".to_string())?;
        Ok(model
            .source_graph
            .nodes
            .iter()
            .map(|node| node.id.clone())
            .collect())
    })();
    let result = retained.and_then(|retained| {
        state
            .selection_coordinator
            .invalidate_missing_source_target(&receipt.runtime_session_id, &retained)
    });
    if let Err(diagnostic) = result {
        eprintln!("[Pană Studio] {diagnostic}");
    }
}

fn workspace_identity(workspace: &ProjectWorkspace) -> ProjectWorkspaceIdentity {
    ProjectWorkspaceIdentity {
        expected_project_root: workspace.session.project_root.clone(),
        expected_session_id: workspace.runtime_session_id(),
        expected_revision: workspace.revision,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use crate::project_model::test_support::ProjectModelTestFixture;

    use super::*;

    #[test]
    fn invalid_tera_retains_the_last_valid_model_and_a_later_valid_draft_recovers() {
        let root = unique_code_edit_test_dir();
        let initial = concat!(
            "{% block content %}\n",
            "{% if visible %}\n",
            "<main><div class=\"existing\">Inițial</div></main>\n",
            "{% endif %}\n",
            "{% endblock %}\n",
        );
        let invalid = concat!(
            "{% block content %}\n",
            "{% if visible %}\n",
            "<main><div class=\"existing\">Draft</div></main>\n",
            "{% endblock %}\n",
        );
        let recovered = concat!(
            "{% block content %}\n",
            "{% if visible %}\n",
            "<main><div class=\"existing\">Final</div></main>\n",
            "{% endif %}\n",
            "{% endblock %}\n",
        );
        let mut fixture = ProjectModelTestFixture::standard_zola(&root, initial).unwrap();
        let initial_model = fixture.build_model().unwrap();
        let initial_existing_id = html_node_id(&initial_model, "<div .existing>");

        fixture
            .draft("templates/index.html", invalid)
            .revision(1, Some("code-invalid-1"));
        let invalid_projection = fixture.projection();
        let invalid_refresh = build_code_edit_project_model(
            fixture.root(),
            Some(&initial_model),
            Some(0),
            &invalid_projection,
            "templates/index.html",
            initial,
            None,
        )
        .unwrap();
        assert!(invalid_refresh.is_none());

        fixture
            .draft("templates/index.html", recovered)
            .revision(2, Some("code-recovered-2"));
        let recovered_projection = fixture.projection();
        let recovered_model = build_code_edit_project_model(
            fixture.root(),
            Some(&initial_model),
            Some(0),
            &recovered_projection,
            "templates/index.html",
            invalid,
            None,
        )
        .unwrap()
        .expect("valid Tera must refresh the semantic model after an invalid draft");

        assert_eq!(
            recovered_model
                .files
                .iter()
                .find(|file| file.relative_path == "templates/index.html")
                .map(|file| file.contents.as_str()),
            Some(recovered),
        );
        assert_eq!(
            html_node_id(&recovered_model, "<div .existing>"),
            initial_existing_id,
        );
        assert!(!is_source_conformance_diagnostic("filesystem unavailable"));
        std::fs::remove_dir_all(root).unwrap();
    }

    fn html_node_id(model: &ProjectModel, label: &str) -> String {
        model
            .source_graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == crate::source_graph::model::SourceNodeKind::Html && node.label == label
            })
            .map(|node| node.id.clone())
            .expect("HTML node")
    }

    fn unique_code_edit_test_dir() -> PathBuf {
        static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(1);
        std::env::temp_dir().join(format!(
            "pana-code-edit-model-{}-{}",
            std::process::id(),
            NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed),
        ))
    }
}
