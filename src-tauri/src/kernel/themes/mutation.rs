use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

use crate::kernel::{
    component_mutation::validate_semantic_workspace_candidate,
    project_path::normalize_project_relative_path,
    project_workspace::{
        ProjectWorkspace, ProjectWorkspaceIdentity, ProjectWorkspaceMutationReceipt,
        WorkspaceBinaryRestoreChange, WorkspaceMutationMetadata, WorkspaceResourceMutation,
    },
};

use super::{
    model::{
        ThemeImpactItem, ThemeOperation, ThemePlan, ThemePlanRequest, THEME_PLAN_SCHEMA_VERSION,
    },
    registry::{find_config_path, join_project_path, zola_prefix, ThemePack, ThemeRegistry},
};

pub fn plan_theme_operation(
    registry: &ThemeRegistry,
    workspace: &ProjectWorkspace,
    request: &ThemePlanRequest,
) -> Result<ThemePlan, String> {
    workspace.require_identity(&request.identity)?;
    let pack = registry.require(&request.theme_id)?;
    let projection = workspace.capture_projection_lease()?;
    let prefix = zola_prefix(workspace)?;
    let paths = project_paths(workspace, &projection);
    let installed_files = pack.project_theme_files(&prefix);
    let installed_count = installed_files
        .iter()
        .filter(|file| paths.contains(&file.relative_path))
        .count();
    let install_complete = installed_count == installed_files.len() && !installed_files.is_empty();
    let mut conflicts = Vec::new();
    let mut missing_requirements = Vec::new();
    let mut local_overrides = Vec::new();
    let mut notices = Vec::new();
    let mut affected_files = Vec::new();
    let mut changed = false;

    match request.operation {
        ThemeOperation::Install => {
            if install_complete {
                notices.push(impact(
                    "theme_already_installed",
                    "Tema este deja instalată integral; planul nu va modifica proiectul.",
                    None,
                    false,
                ));
            } else {
                for file in &installed_files {
                    if paths.contains(&file.relative_path) {
                        conflicts.push(impact(
                            "theme_install_destination_exists",
                            format!(
                                "Instalarea nu suprascrie destinația existentă `{}`.",
                                file.relative_path
                            ),
                            Some(file.relative_path.clone()),
                            true,
                        ));
                    } else {
                        affected_files.push(file.relative_path.clone());
                    }
                }
                changed = conflicts.is_empty() && !affected_files.is_empty();
            }
            append_missing_requirements(pack, &prefix, &paths, false, &mut missing_requirements);
        }
        ThemeOperation::Activate => {
            if !install_complete {
                conflicts.push(impact(
                    "theme_not_installed",
                    if installed_count == 0 {
                        "Tema trebuie instalată înainte de activare.".to_string()
                    } else {
                        format!(
                            "Instalarea temei este incompletă: {installed_count}/{} fișiere.",
                            installed_files.len()
                        )
                    },
                    None,
                    true,
                ));
            }
            append_missing_requirements(pack, &prefix, &paths, true, &mut missing_requirements);
            append_local_overrides(pack, &prefix, &paths, &mut local_overrides);

            let config_path = find_config_path(&projection.source_texts, &prefix);
            if let Some(config_path) = config_path {
                let active = projection
                    .source_texts
                    .get(&config_path)
                    .and_then(|source| crate::zola_theme::active_theme_from_source(source));
                if active.as_deref() == Some(pack.manifest.id.as_str()) {
                    notices.push(impact(
                        "theme_already_active",
                        "Tema este deja activă; planul nu va modifica proiectul.",
                        Some(config_path),
                        false,
                    ));
                } else {
                    affected_files.push(config_path);
                    changed = true;
                }
            } else {
                conflicts.push(impact(
                    "theme_config_missing",
                    "Proiectul nu conține zola.toml sau config.toml în Zola root.",
                    None,
                    true,
                ));
            }
        }
    }
    affected_files.sort();
    affected_files.dedup();
    let blocking = conflicts.iter().any(|item| item.blocking)
        || missing_requirements.iter().any(|item| item.blocking);
    if blocking {
        changed = false;
    }
    let mut plan = ThemePlan {
        schema_version: THEME_PLAN_SCHEMA_VERSION,
        theme_id: pack.manifest.id.clone(),
        operation: request.operation,
        expected_project_root: request.identity.expected_project_root.clone(),
        expected_session_id: request.identity.expected_session_id.clone(),
        expected_revision: request.identity.expected_revision,
        plan_token: String::new(),
        changed,
        blocking,
        affected_files,
        conflicts,
        missing_requirements,
        local_overrides,
        notices,
    };
    plan.plan_token = plan_token(&plan)?;
    Ok(plan)
}

pub fn apply_theme_plan(
    registry: &ThemeRegistry,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    request: ThemePlanRequest,
    expected_plan_token: &str,
    now_ms: u128,
) -> Result<(ThemePlan, ProjectWorkspaceMutationReceipt), String> {
    let plan = plan_theme_operation(registry, workspace, &request)?;
    if expected_plan_token != plan.plan_token {
        return Err(format!(
            "[theme_plan_stale] Planul temei nu mai corespunde reviziei {}.",
            workspace.revision
        ));
    }
    if plan.blocking {
        return Err("[theme_plan_blocked] Planul temei conține impact blocant.".to_string());
    }
    let mut candidate = workspace.clone();
    let identity = ProjectWorkspaceIdentity {
        expected_project_root: candidate.session.project_root.clone(),
        expected_session_id: candidate.runtime_session_id(),
        expected_revision: candidate.revision,
    };
    let metadata = WorkspaceMutationMetadata {
        label: match plan.operation {
            ThemeOperation::Install => format!("Instalează tema {}", plan.theme_id),
            ThemeOperation::Activate => format!("Activează tema {}", plan.theme_id),
        },
        source: "themes.workspace".to_string(),
        coalesce_key: None,
        transaction_id: None,
    };
    let receipt = match plan.operation {
        ThemeOperation::Install => {
            let pack = registry.require(&plan.theme_id)?;
            let prefix = zola_prefix(&candidate)?;
            let (text_changes, binary_changes) = theme_install_changes(pack, &prefix)?;
            candidate.stage_project_bundle_changes(
                &identity,
                metadata,
                text_changes,
                Vec::new(),
                binary_changes,
                now_ms,
            )?
        }
        ThemeOperation::Activate => {
            let projection = candidate.capture_projection_lease()?;
            let prefix = zola_prefix(&candidate)?;
            let config_path =
                find_config_path(&projection.source_texts, &prefix).ok_or_else(|| {
                    "[theme_config_missing] Configurația Zola a dispărut înainte de apply."
                        .to_string()
                })?;
            let current = projection.source_texts.get(&config_path).ok_or_else(|| {
                format!("[theme_config_missing] `{config_path}` nu este proiectat ca text.")
            })?;
            let patched = patch_theme_key(current, &plan.theme_id)?;
            candidate.stage_project_bundle_changes(
                &identity,
                metadata,
                vec![WorkspaceResourceMutation {
                    relative_path: config_path,
                    contents: patched,
                    create_only: false,
                }],
                Vec::new(),
                Vec::new(),
                now_ms,
            )?
        }
    };
    if receipt.changed {
        validate_semantic_workspace_candidate(project_root, &candidate, "Mutația temei")?;
    }
    *workspace = candidate;
    Ok((plan, receipt))
}

fn append_missing_requirements(
    pack: &ThemePack,
    prefix: &str,
    paths: &HashSet<String>,
    blocking: bool,
    output: &mut Vec<ThemeImpactItem>,
) {
    for (kind, requirements) in [
        ("page", &pack.manifest.required_pages),
        ("data", &pack.manifest.required_data),
    ] {
        for requirement in requirements {
            let path = join_project_path(prefix, requirement);
            if !paths.contains(&path) {
                output.push(impact(
                    format!("theme_required_{kind}_missing"),
                    format!("Cerința temei lipsește: `{path}`."),
                    Some(path),
                    blocking,
                ));
            }
        }
    }
}

fn append_local_overrides(
    pack: &ThemePack,
    prefix: &str,
    paths: &HashSet<String>,
    output: &mut Vec<ThemeImpactItem>,
) {
    for file in &pack.theme_files {
        let Some(relative) = file.relative_path.strip_prefix("templates/") else {
            continue;
        };
        let local = join_project_path(prefix, &format!("templates/{relative}"));
        if paths.contains(&local) {
            output.push(impact(
                "theme_local_override",
                format!("Template-ul local `{local}` va avea prioritate peste tema activă."),
                Some(local),
                false,
            ));
        }
    }
}

fn project_paths(
    workspace: &ProjectWorkspace,
    projection: &crate::kernel::project_workspace::WorkspaceProjectionLease,
) -> HashSet<String> {
    projection
        .accepted_disk
        .manifest
        .files
        .iter()
        .map(|entry| entry.relative_path.clone())
        .chain(projection.source_texts.keys().cloned())
        .chain(projection.resource_bytes.keys().cloned())
        .filter(|path| !projection.deleted_sources.contains(path))
        .filter(|path| {
            PathBuf::from(&workspace.session.project_root)
                .join(path)
                .starts_with(&workspace.session.project_root)
        })
        .collect()
}

fn theme_install_changes(
    pack: &ThemePack,
    prefix: &str,
) -> Result<
    (
        Vec<WorkspaceResourceMutation>,
        Vec<WorkspaceBinaryRestoreChange>,
    ),
    String,
> {
    let mut text = Vec::new();
    let mut binary = Vec::new();
    for file in pack.project_theme_files(prefix) {
        normalize_project_relative_path(&file.relative_path)?;
        if is_text_theme_file(&file.relative_path) {
            let contents = String::from_utf8(file.bytes).map_err(|_| {
                format!(
                    "[theme_text_invalid] `{}` este declarat text, dar nu este UTF-8.",
                    file.relative_path
                )
            })?;
            text.push(WorkspaceResourceMutation {
                relative_path: file.relative_path,
                contents,
                create_only: true,
            });
        } else {
            binary.push(WorkspaceBinaryRestoreChange {
                relative_path: file.relative_path,
                before: None,
                after: Some(file.bytes),
            });
        }
    }
    Ok((text, binary))
}

fn is_text_theme_file(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "html"
                    | "htm"
                    | "toml"
                    | "scss"
                    | "sass"
                    | "css"
                    | "js"
                    | "mjs"
                    | "cjs"
                    | "ts"
                    | "json"
                    | "md"
                    | "txt"
                    | "xml"
                    | "svg"
                    | "csv"
                    | "yaml"
                    | "yml"
                    | "bib"
            )
        })
}

fn patch_theme_key(source: &str, theme_id: &str) -> Result<String, String> {
    crate::zola_theme::set_active_theme_in_source(source, theme_id)
        .map_err(|error| format!("[theme_config_invalid] {error}"))
}

fn impact(
    code: impl Into<String>,
    message: impl Into<String>,
    relative_path: Option<String>,
    blocking: bool,
) -> ThemeImpactItem {
    ThemeImpactItem {
        code: code.into(),
        message: message.into(),
        relative_path,
        blocking,
    }
}

fn plan_token(plan: &ThemePlan) -> Result<String, String> {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct TokenMaterial<'a> {
        schema_version: u32,
        theme_id: &'a str,
        operation: ThemeOperation,
        expected_project_root: &'a str,
        expected_session_id: &'a str,
        expected_revision: u64,
        changed: bool,
        blocking: bool,
        affected_files: &'a [String],
        conflicts: &'a [ThemeImpactItem],
        missing_requirements: &'a [ThemeImpactItem],
        local_overrides: &'a [ThemeImpactItem],
        notices: &'a [ThemeImpactItem],
    }
    let bytes = serde_json::to_vec(&TokenMaterial {
        schema_version: plan.schema_version,
        theme_id: &plan.theme_id,
        operation: plan.operation,
        expected_project_root: &plan.expected_project_root,
        expected_session_id: &plan.expected_session_id,
        expected_revision: plan.expected_revision,
        changed: plan.changed,
        blocking: plan.blocking,
        affected_files: &plan.affected_files,
        conflicts: &plan.conflicts,
        missing_requirements: &plan.missing_requirements,
        local_overrides: &plan.local_overrides,
        notices: &plan.notices,
    })
    .map_err(|error| format!("Planul temei nu a putut fi serializat: {error}"))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app_home::{ensure_app_home, TEST_APP_ENV_LOCK},
        js::PageJsDraftStore,
        kernel::{
            file_buffer_store::{
                hash_text, FileBufferBaseline, FileBufferEntry, FileBufferStore,
                FileBufferStoreLimits, TextBufferLanguage, TextBufferRole,
            },
            project_session::{
                ProjectRootFingerprint, ProjectSessionScanSummary, ProjectSessionSnapshot,
            },
            project_workspace::{
                commit_project_workspace_session_mutation, restore_project_workspace_recovery,
                save_project_workspace, ProjectWorkspaceRecoveryStatus,
            },
            write_authority::test_support::install_test_project_authority,
        },
        project::{read_project_disk_manifest, AcceptedProjectDiskManifest},
    };
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn activation_patch_is_lossless_outside_top_level_theme() {
        let source = "# keep\nbase_url = \"/\"\n\n[extra]\ntheme = \"content-value\"\n";
        let patched = patch_theme_key(source, "pana-studio").unwrap();
        assert!(patched.contains("# keep"));
        assert!(patched.contains("[extra]\ntheme = \"content-value\""));
        assert_eq!(
            crate::zola_theme::active_theme_from_source(&patched).as_deref(),
            Some("pana-studio")
        );

        let existing =
            "base_url = \"/\"\ntheme = \"old-theme\" # alegerea utilizatorului\n\n[extra]\nx = 1\n";
        let updated = patch_theme_key(existing, "pana-studio").unwrap();
        assert!(updated.contains("# alegerea utilizatorului"));
        assert!(updated.contains("[extra]\nx = 1"));
    }

    #[test]
    fn install_and_activate_are_distinct_single_history_transactions_with_undo_redo() {
        let root = valid_project("install-activate");
        let registry = ThemeRegistry::load_from_root(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/theme-packs"),
        )
        .unwrap();
        let mut workspace = workspace(&root);

        let install_request = request(&workspace, ThemeOperation::Install);
        let install_plan = plan_theme_operation(&registry, &workspace, &install_request).unwrap();
        assert!(install_plan.changed);
        assert!(!install_plan.blocking);
        let (_, install_receipt) = apply_theme_plan(
            &registry,
            &root,
            &mut workspace,
            install_request,
            &install_plan.plan_token,
            10,
        )
        .unwrap();
        assert!(install_receipt.changed);
        assert_eq!(workspace.snapshot().history.undo_count, 1);
        assert!(workspace
            .documents
            .text_for("themes/pana-studio/theme.toml")
            .is_some());
        assert_eq!(
            crate::zola_theme::active_theme_from_source(
                &workspace.documents.text_for("zola.toml").unwrap()
            ),
            None
        );

        let activate_request = request(&workspace, ThemeOperation::Activate);
        let activate_plan = plan_theme_operation(&registry, &workspace, &activate_request).unwrap();
        assert!(activate_plan.changed);
        assert!(!activate_plan.blocking);
        let (_, activate_receipt) = apply_theme_plan(
            &registry,
            &root,
            &mut workspace,
            activate_request,
            &activate_plan.plan_token,
            20,
        )
        .unwrap();
        assert!(activate_receipt.changed);
        assert_eq!(workspace.snapshot().history.undo_count, 2);
        assert_eq!(
            crate::zola_theme::active_theme_from_source(
                &workspace.documents.text_for("zola.toml").unwrap()
            )
            .as_deref(),
            Some("pana-studio")
        );
        let graph =
            validate_semantic_workspace_candidate(&root, &workspace, "Test proiecție temă activă")
                .unwrap();
        assert_eq!(graph.active_theme.as_deref(), Some("pana-studio"));
        assert!(graph
            .templates
            .iter()
            .any(|template| template.file == "themes/pana-studio/templates/base.html"));
        let repeated_request = request(&workspace, ThemeOperation::Activate);
        let repeated_plan = plan_theme_operation(&registry, &workspace, &repeated_request).unwrap();
        assert!(!repeated_plan.changed);
        let revision_before_repeat = workspace.revision;
        let (_, repeated_receipt) = apply_theme_plan(
            &registry,
            &root,
            &mut workspace,
            repeated_request,
            &repeated_plan.plan_token,
            25,
        )
        .unwrap();
        assert!(!repeated_receipt.changed);
        assert_eq!(workspace.revision, revision_before_repeat);
        assert_eq!(workspace.snapshot().history.undo_count, 2);

        workspace.undo(&identity(&workspace), 30).unwrap();
        assert_eq!(
            crate::zola_theme::active_theme_from_source(
                &workspace.documents.text_for("zola.toml").unwrap()
            ),
            None
        );
        workspace.redo(&identity(&workspace), 40).unwrap();
        assert_eq!(
            crate::zola_theme::active_theme_from_source(
                &workspace.documents.text_for("zola.toml").unwrap()
            )
            .as_deref(),
            Some("pana-studio")
        );
        cleanup(root);
    }

    #[test]
    fn planning_rejects_stale_identity_and_apply_rejects_changed_plan_token() {
        let root = valid_project("stale");
        let registry = ThemeRegistry::load_from_root(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/theme-packs"),
        )
        .unwrap();
        let mut workspace = workspace(&root);
        let mut stale = request(&workspace, ThemeOperation::Install);
        stale.identity.expected_revision += 1;
        assert!(plan_theme_operation(&registry, &workspace, &stale)
            .unwrap_err()
            .contains("stale"));

        let request = request(&workspace, ThemeOperation::Install);
        let error = apply_theme_plan(&registry, &root, &mut workspace, request, "wrong-token", 10)
            .unwrap_err();
        assert!(error.contains("theme_plan_stale"));
        assert_eq!(workspace.revision, 0);
        assert_eq!(workspace.snapshot().history.undo_count, 0);
        cleanup(root);
    }

    #[test]
    fn install_plan_reports_existing_destination_without_overwrite() {
        let root = valid_project("conflict");
        fs::create_dir_all(root.join("themes/pana-studio")).unwrap();
        fs::write(
            root.join("themes/pana-studio/theme.toml"),
            "name = \"external\"\n",
        )
        .unwrap();
        let registry = ThemeRegistry::load_from_root(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/theme-packs"),
        )
        .unwrap();
        let workspace = workspace(&root);
        let plan = plan_theme_operation(
            &registry,
            &workspace,
            &request(&workspace, ThemeOperation::Install),
        )
        .unwrap();
        assert!(plan.blocking);
        assert!(plan
            .conflicts
            .iter()
            .any(|item| item.code == "theme_install_destination_exists"));
        cleanup(root);
    }

    #[test]
    fn activation_plan_reports_local_templates_that_mask_the_theme() {
        let root = valid_project("local-override");
        fs::write(
            root.join("templates/base.html"),
            "<!doctype html><title>Override local</title>",
        )
        .unwrap();
        let registry = ThemeRegistry::load_from_root(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/theme-packs"),
        )
        .unwrap();
        let mut workspace = workspace(&root);
        let install_request = request(&workspace, ThemeOperation::Install);
        let install_plan = plan_theme_operation(&registry, &workspace, &install_request).unwrap();
        apply_theme_plan(
            &registry,
            &root,
            &mut workspace,
            install_request,
            &install_plan.plan_token,
            10,
        )
        .unwrap();

        let activate = plan_theme_operation(
            &registry,
            &workspace,
            &request(&workspace, ThemeOperation::Activate),
        )
        .unwrap();
        assert!(activate.local_overrides.iter().any(|item| {
            item.relative_path.as_deref() == Some("templates/base.html") && !item.blocking
        }));
        cleanup(root);
    }

    #[test]
    fn theme_recovery_save_and_reopen_preserve_the_authoritative_active_theme() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = fs::canonicalize(valid_project("recovery-save-reopen")).unwrap();
        let _env = TestEnvGuard::from_root(&root.with_extension("app-home"));
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        ensure_app_home(app.handle()).unwrap();
        let registry = ThemeRegistry::load_from_root(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/theme-packs"),
        )
        .unwrap();
        let mut live = workspace(&root);
        let session_dir = PathBuf::from(&live.session.session_dir);
        fs::create_dir_all(&session_dir).unwrap();
        install_test_project_authority(
            app.handle(),
            &live.runtime_session_id(),
            &root,
            &session_dir,
        )
        .unwrap();

        let install_request = request(&live, ThemeOperation::Install);
        let install_plan = plan_theme_operation(&registry, &live, &install_request).unwrap();
        commit_project_workspace_session_mutation(app.handle(), &mut live, |candidate| {
            apply_theme_plan(
                &registry,
                &root,
                candidate,
                install_request,
                &install_plan.plan_token,
                10,
            )
        })
        .unwrap();

        let mut restored = workspace(&root);
        assert_eq!(
            restore_project_workspace_recovery(app.handle(), &mut restored).unwrap(),
            ProjectWorkspaceRecoveryStatus::Restored
        );
        assert_eq!(restored.revision, live.revision);
        assert_eq!(restored.snapshot().history.undo_count, 1);
        assert!(restored
            .documents
            .text_for("themes/pana-studio/theme.toml")
            .is_some());

        let activate_request = request(&restored, ThemeOperation::Activate);
        let activate_plan = plan_theme_operation(&registry, &restored, &activate_request).unwrap();
        commit_project_workspace_session_mutation(app.handle(), &mut restored, |candidate| {
            apply_theme_plan(
                &registry,
                &root,
                candidate,
                activate_request,
                &activate_plan.plan_token,
                20,
            )
        })
        .unwrap();
        let save_identity = identity(&restored);
        let save =
            save_project_workspace(app.handle(), &root, &mut restored, &save_identity).unwrap();
        assert!(save.written_files.iter().any(|path| path == "zola.toml"));
        assert!(root.join("themes/pana-studio/theme.toml").is_file());
        assert_eq!(
            crate::zola_theme::read_active_theme(&root).as_deref(),
            Some("pana-studio")
        );

        let reopened = workspace(&root);
        let snapshot = registry.snapshot(Some(&reopened)).unwrap();
        let theme = snapshot
            .themes
            .iter()
            .find(|theme| theme.id == "pana-studio")
            .unwrap();
        assert_eq!(theme.status, crate::kernel::themes::ThemeStatus::Active);
        assert!(theme.install_complete);
        crate::deploy::run_zola_check(&root, &root).unwrap();

        drop(app);
        let _ = fs::remove_dir_all(session_dir);
        cleanup(root);
    }

    fn valid_project(label: &str) -> PathBuf {
        let root = temp_dir(label);
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(
            root.join("zola.toml"),
            "base_url = \"http://127.0.0.1:1111\"\ntitle = \"Test\"\ncompile_sass = false\n",
        )
        .unwrap();
        fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = \"Acasă\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("templates/index.html"),
            "<!doctype html><title>{{ section.title }}</title><main>{{ section.content | safe }}</main>",
        )
        .unwrap();
        root
    }

    fn workspace(root: &Path) -> ProjectWorkspace {
        let session = ProjectSessionSnapshot {
            schema_version: 1,
            id: "theme-test".to_string(),
            project_root: root.to_string_lossy().into_owned(),
            zola_root: root.to_string_lossy().into_owned(),
            session_dir: root
                .with_extension("theme-session")
                .to_string_lossy()
                .into_owned(),
            manifest_path: root
                .with_extension("theme-session")
                .join("manifest.json")
                .to_string_lossy()
                .into_owned(),
            opened_at_ms: 7,
            last_seen_at_ms: 7,
            root_fingerprint: ProjectRootFingerprint {
                canonical_path: root.to_string_lossy().into_owned(),
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
                max_files: 100,
                max_file_bytes: 2 * 1024 * 1024,
                max_total_bytes: 16 * 1024 * 1024,
            },
        );
        for relative_path in ["zola.toml", "content/_index.md", "templates/index.html"] {
            let path = root.join(relative_path);
            let text = fs::read_to_string(&path).unwrap();
            let (language, role) = if relative_path.ends_with(".toml") {
                (TextBufferLanguage::Toml, TextBufferRole::Config)
            } else if relative_path.ends_with(".md") {
                (TextBufferLanguage::Markdown, TextBufferRole::Page)
            } else {
                (TextBufferLanguage::Html, TextBufferRole::Template)
            };
            documents.insert_loaded_file(FileBufferEntry {
                relative_path: relative_path.to_string(),
                absolute_path: path.to_string_lossy().into_owned(),
                language,
                role,
                baseline: FileBufferBaseline {
                    hash: hash_text(&text),
                    modified_ms: 1,
                    size: text.len() as u64,
                    readonly: false,
                },
                baseline_text: text,
                draft: None,
                revision: 1,
            });
        }
        let accepted = AcceptedProjectDiskManifest::new(
            session.runtime_instance_id(),
            session.project_root.clone(),
            read_project_disk_manifest(root).unwrap(),
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

    fn request(workspace: &ProjectWorkspace, operation: ThemeOperation) -> ThemePlanRequest {
        ThemePlanRequest {
            theme_id: "pana-studio".to_string(),
            operation,
            identity: identity(workspace),
        }
    }

    fn identity(workspace: &ProjectWorkspace) -> ProjectWorkspaceIdentity {
        ProjectWorkspaceIdentity {
            expected_project_root: workspace.session.project_root.clone(),
            expected_session_id: workspace.runtime_session_id(),
            expected_revision: workspace.revision,
        }
    }

    fn temp_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "pana-theme-mutation-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn cleanup(path: PathBuf) {
        let _ = fs::remove_dir_all(path);
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
                .map(|(key, _)| (*key, std::env::var(key).ok()))
                .collect();
            for (key, path) in bindings {
                std::env::set_var(key, path);
            }
            Self { previous_values }
        }
    }

    impl Drop for TestEnvGuard {
        fn drop(&mut self) {
            for (key, value) in &self.previous_values {
                if let Some(value) = value {
                    std::env::set_var(key, value);
                } else {
                    std::env::remove_var(key);
                }
            }
        }
    }
}
