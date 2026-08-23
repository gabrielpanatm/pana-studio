use super::{support::*, *};

#[test]
fn mutation_stages_metadata_as_one_undoable_project_workspace_transaction() {
    let root = fixture_root("workspace");
    fs::create_dir_all(&root).unwrap();
    let sources = HashMap::from([
        (
            "zola.toml".to_string(),
            "base_url = \"https://example.com\"\n".to_string(),
        ),
        (
            "content/_index.md".to_string(),
            "+++\ntitle = \"Acasă\"\ntemplate = \"index.html\"\n+++\n".to_string(),
        ),
        (
            "templates/index.html".to_string(),
            "<main>Acasă</main>\n".to_string(),
        ),
    ]);
    let mut workspace = test_workspace(&root, sources);
    let projection = workspace.capture_projection_snapshot().unwrap();
    let graph =
        crate::source_graph::build_source_graph_from_workspace_projection(&root, &projection)
            .unwrap();
    let planned = plan_content_model_mutation(
        &root,
        &graph,
        &projection.source_texts,
        &ContentModelMutationInput {
            operation: ContentModelMutationOperation::CreateModel {
                id: "service".to_string(),
                label: "Serviciu".to_string(),
                description: "Contract test".to_string(),
            },
        },
    )
    .unwrap();
    let (plan, receipt) = stage_content_model_mutation(&mut workspace, planned, 2).unwrap();
    assert!(receipt.changed);
    assert_eq!(receipt.history.undo_count, 1);
    assert!(plan
        .touched_files
        .contains(&CONTENT_MODEL_PROJECT_PATH.to_string()));
    assert!(workspace
        .documents
        .text_for(&model_path("service"))
        .is_some());

    let undo_identity = ProjectWorkspaceIdentity {
        expected_project_root: workspace.session.project_root.clone(),
        expected_session_id: workspace.runtime_session_id(),
        expected_revision: workspace.revision,
    };
    workspace
        .undo(&undo_identity, 3)
        .expect("content model undo");
    assert!(workspace
        .documents
        .text_for(&model_path("service"))
        .is_none());
    let redo_identity = ProjectWorkspaceIdentity {
        expected_project_root: workspace.session.project_root.clone(),
        expected_session_id: workspace.runtime_session_id(),
        expected_revision: workspace.revision,
    };
    workspace
        .redo(&redo_identity, 4)
        .expect("content model redo");
    assert!(workspace
        .documents
        .text_for(&model_path("service"))
        .is_some());
    let _ = fs::remove_dir_all(root);
}
