use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime, State};

use crate::{
    commands::workspace_entries::{
        require_bound_workspace, WorkspaceEntryMutationReceipt,
        WORKSPACE_ENTRY_MUTATION_SCHEMA_VERSION,
    },
    kernel::{
        data_mutation::{
            read_data_node_editor_snapshot, stage_validated_data_mutation, DataMutationInput,
            DataMutationPlan, DataNodeEditorSnapshot,
        },
        file_buffer_store::FileBufferRequestIdentity,
        observability::now_ms,
        project_workspace::commit_project_workspace_session_mutation,
    },
    state::AppState,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataMutationApplyReceipt {
    pub plan: DataMutationPlan,
    pub workspace: WorkspaceEntryMutationReceipt,
}

#[tauri::command(async)]
pub fn read_data_node_editor(
    file: String,
    node_id: String,
    identity: FileBufferRequestIdentity,
    state: State<AppState>,
) -> Result<DataNodeEditorSnapshot, String> {
    let (root, slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = slot
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru date.".to_string())?;
    read_data_node_editor_snapshot(&root, workspace, &file, &node_id)
}

#[tauri::command]
pub async fn apply_data_mutation(
    input: DataMutationInput,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
) -> Result<DataMutationApplyReceipt, String> {
    apply_data_mutation_task(input, identity, app).await
}

async fn apply_data_mutation_task<R: Runtime>(
    input: DataMutationInput,
    identity: FileBufferRequestIdentity,
    app: AppHandle<R>,
) -> Result<DataMutationApplyReceipt, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        apply_data_mutation_blocking(&app, state.inner(), input, identity)
    })
    .await
    .map_err(|error| format!("Mutația datelor TOML a căzut în task-ul de fundal: {error}"))?
}

fn apply_data_mutation_blocking<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    input: DataMutationInput,
    identity: FileBufferRequestIdentity,
) -> Result<DataMutationApplyReceipt, String> {
    let (root, mut slot) = require_bound_workspace(state, &identity)?;
    let workspace = slot
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru date.".to_string())?;
    let (plan, mutation) =
        commit_project_workspace_session_mutation(app, workspace, |candidate| {
            stage_validated_data_mutation(&root, candidate, input, now_ms())
        })?;
    let workspace_receipt = WorkspaceEntryMutationReceipt {
        schema_version: WORKSPACE_ENTRY_MUTATION_SCHEMA_VERSION,
        project_root: workspace.session.project_root.clone(),
        runtime_session_id: workspace.runtime_session_id(),
        relative_path: Some(plan.file.clone()),
        mutation,
        workspace: workspace.snapshot(),
    };
    Ok(DataMutationApplyReceipt {
        plan,
        workspace: workspace_receipt,
    })
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        app_home::{ensure_app_home, TEST_APP_ENV_LOCK},
        commands::components::apply_component_mutation_task,
        js::PageJsDraftStore,
        kernel::{
            component_mutation::{
                ComponentDraftKind, ComponentMutationInput, ComponentMutationOperation,
            },
            data_mutation::{DataMutationInput, DataMutationOperation},
            file_buffer_store::{
                hash_text, FileBufferBaseline, FileBufferEntry, FileBufferRequestIdentity,
                FileBufferStore, FileBufferStoreLimits, TextBufferLanguage, TextBufferRole,
            },
            project_session::{
                ProjectRootFingerprint, ProjectSessionScanSummary, ProjectSessionSnapshot,
            },
            project_workspace::ProjectWorkspace,
        },
        project::{read_project_disk_manifest, AcceptedProjectDiskManifest},
        state::AppState,
    };

    use super::apply_data_mutation_task;

    #[test]
    fn mutation_commands_keep_embedded_zola_outside_the_async_runtime() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let fixture = temp_dir("async-boundary");
        let _env_guard = TestEnvGuard::from_root(&fixture.join("app-home"));
        let project = fixture.join("project");
        write_zola_project(&project);

        let workspace = test_workspace(&project);
        let identity = FileBufferRequestIdentity {
            expected_project_root: workspace.session.project_root.clone(),
            expected_session_id: workspace.runtime_session_id(),
        };
        let state = AppState::default();
        *state.current_root.lock().unwrap() = Some(project.canonicalize().unwrap());
        *state.project_workspace.lock().unwrap() = Some(workspace);
        state
            .ai_coordination
            .bind_project(Some(identity.expected_session_id.clone()), 1)
            .unwrap();
        let app = tauri::test::mock_builder()
            .manage(state)
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock app");
        ensure_app_home(app.handle()).unwrap();

        let data_receipt = tauri::async_runtime::block_on(apply_data_mutation_task(
            DataMutationInput {
                operation: DataMutationOperation::CreateFile,
                file: "date/site.toml".to_string(),
                node_id: None,
                key: None,
                draft_kind: None,
                value: Some("titlu = \"Pană Studio\"\n".to_string()),
            },
            identity.clone(),
            app.handle().clone(),
        ))
        .unwrap();
        assert_eq!(data_receipt.plan.file, "date/site.toml");

        let component_receipt = tauri::async_runtime::block_on(apply_component_mutation_task(
            ComponentMutationInput {
                operation: ComponentMutationOperation::Create,
                definition_id: None,
                kind: Some(ComponentDraftKind::Partial),
                name: Some("test/card".to_string()),
                destination_name: None,
                contents: Some("<article>Card</article>\n".to_string()),
                source_file: None,
                source_range: None,
                companions: Vec::new(),
            },
            identity,
            app.handle().clone(),
        ))
        .unwrap();
        assert_eq!(
            component_receipt.plan.destination_relative_path.as_deref(),
            Some("templates/partials/test/card.html")
        );

        drop(app);
        fs::remove_dir_all(fixture).unwrap();
    }

    fn write_zola_project(project: &Path) {
        fs::create_dir_all(project.join("templates")).unwrap();
        fs::create_dir_all(project.join("content")).unwrap();
        fs::create_dir_all(project.join("date")).unwrap();
        fs::write(
            project.join("zola.toml"),
            "base_url = \"https://example.test\"\n",
        )
        .unwrap();
        fs::write(project.join("templates/index.html"), "<main>Test</main>\n").unwrap();
        fs::write(project.join("content/_index.md"), "+++\n+++\n").unwrap();
    }

    fn test_workspace(project: &Path) -> ProjectWorkspace {
        let canonical = project.canonicalize().unwrap();
        let canonical_source = canonical.to_string_lossy().into_owned();
        let session = ProjectSessionSnapshot {
            schema_version: 1,
            id: "command-async-boundary-test".to_string(),
            project_root: canonical_source.clone(),
            zola_root: canonical_source.clone(),
            session_dir: project.join("session").to_string_lossy().into_owned(),
            manifest_path: project
                .join("session/manifest.json")
                .to_string_lossy()
                .into_owned(),
            opened_at_ms: 1,
            last_seen_at_ms: 1,
            root_fingerprint: ProjectRootFingerprint {
                canonical_path: canonical_source,
                modified_ms: 1,
                size: 0,
                readonly: false,
                unix_device: None,
                unix_inode: None,
            },
            scan_summary: ProjectSessionScanSummary {
                is_zola: true,
                is_empty: false,
                active_theme: None,
                file_count: 3,
                directory_count: 3,
            },
        };
        let mut documents = FileBufferStore::for_project_session(
            &session,
            1,
            FileBufferStoreLimits {
                max_files: 32,
                max_file_bytes: 1024 * 1024,
                max_total_bytes: 4 * 1024 * 1024,
            },
        );
        for (relative_path, language, role) in [
            (
                "zola.toml",
                TextBufferLanguage::Toml,
                TextBufferRole::Config,
            ),
            (
                "templates/index.html",
                TextBufferLanguage::Html,
                TextBufferRole::Template,
            ),
            (
                "content/_index.md",
                TextBufferLanguage::Markdown,
                TextBufferRole::Page,
            ),
        ] {
            let source = fs::read_to_string(project.join(relative_path)).unwrap();
            documents.insert_loaded_file(FileBufferEntry {
                relative_path: relative_path.to_string(),
                absolute_path: project.join(relative_path).to_string_lossy().into_owned(),
                language,
                role,
                baseline: FileBufferBaseline {
                    hash: hash_text(&source),
                    modified_ms: 1,
                    size: source.len() as u64,
                    readonly: false,
                },
                baseline_text: source,
                draft: None,
                revision: 1,
            });
        }
        let accepted = AcceptedProjectDiskManifest::new(
            session.runtime_instance_id(),
            session.project_root.clone(),
            read_project_disk_manifest(project).unwrap(),
        )
        .unwrap();
        ProjectWorkspace::new(
            session.clone(),
            accepted,
            documents,
            PageJsDraftStore::new(&session),
        )
        .unwrap()
    }

    fn temp_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!(
            "pana-command-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    struct TestEnvGuard {
        previous_values: Vec<(&'static str, Option<String>)>,
    }

    impl TestEnvGuard {
        fn from_root(root: &Path) -> Self {
            let bindings = [
                ("XDG_CONFIG_HOME", root.join("config")),
                ("XDG_DATA_HOME", root.join("data")),
                ("XDG_CACHE_HOME", root.join("cache")),
                ("XDG_STATE_HOME", root.join("state")),
            ];
            let previous_values = bindings
                .iter()
                .map(|(key, _)| (*key, env::var(key).ok()))
                .collect::<Vec<_>>();
            for (key, path) in bindings {
                env::set_var(key, path);
            }
            Self { previous_values }
        }
    }

    impl Drop for TestEnvGuard {
        fn drop(&mut self) {
            for (key, value) in &self.previous_values {
                if let Some(value) = value {
                    env::set_var(key, value);
                } else {
                    env::remove_var(key);
                }
            }
        }
    }
}
