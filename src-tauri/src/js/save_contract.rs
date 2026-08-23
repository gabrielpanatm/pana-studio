use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{
    kernel::{
        file_buffer_store::FileBufferStore,
        project_session::ProjectSessionSnapshot,
        project_workspace::{WorkspaceTextChange, WorkspaceTextDelete},
    },
    zola_links::template_contains_asset_path,
    zola_theme::{active_theme_from_source, ZolaThemeResolver},
};

use super::{
    ensure_base_scripts_block, ensure_page_scripts_block, ensure_script_tags, extract_extends,
    generate_page_js, js_relative_path, page_scripts_html, paths::normalize_template_path,
    reader::read_optional_project_text, remove_page_scripts_contract, template_to_slug,
    PageJsConfig, PageRuntimePlan,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageJsSaveContractRequest {
    pub template_path: String,
    pub config: PageJsConfig,
    pub cachebust_assets: bool,
    pub template_source: Option<String>,
    pub base_template_path: Option<String>,
    pub base_template_source: Option<String>,
    pub existing_page_js_source: Option<String>,
    pub page_js_file_exists: bool,
    pub page_js_tracked: bool,
    pub page_js_disk_exists: bool,
    pub page_js_created_in_session: bool,
    pub preserve_existing_page_js: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PageJsSaveFileAction {
    None,
    Write,
    Remove,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageJsSaveFilePlan {
    pub relative_path: String,
    pub action: PageJsSaveFileAction,
    pub contents: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PageJsGeneratedResourceStatus {
    NoopMissing,
    UnchangedTracked,
    CreateMissing,
    UpdateTracked,
    DeleteTracked,
    BlockedUntrackedExisting,
    BlockedMissingTrackedDisk,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageJsGeneratedResourcePlan {
    pub zola_path: String,
    pub project_path: String,
    pub action: PageJsSaveFileAction,
    pub status: PageJsGeneratedResourceStatus,
    pub tracked_in_file_buffer: bool,
    pub exists_on_disk: bool,
    pub blocked: bool,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageJsSaveTemplatePlan {
    pub relative_path: String,
    pub changed: bool,
    pub contents: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageJsSaveContractPlan {
    pub template_path: String,
    pub js_slug: String,
    pub js_path: String,
    pub has_page_js: bool,
    pub has_motion: bool,
    pub page_js: PageJsSaveFilePlan,
    pub page_js_resource: PageJsGeneratedResourcePlan,
    pub template: Option<PageJsSaveTemplatePlan>,
    pub base_template: Option<PageJsSaveTemplatePlan>,
    pub project_structure_changed: bool,
    pub diagnostics: Vec<String>,
}

fn plan_page_js_save_contract(request: PageJsSaveContractRequest) -> PageJsSaveContractPlan {
    // Requests reach the planner only through
    // `build_page_js_save_contract_request`, which validates and canonicalizes
    // every path before any FileBuffer or filesystem read.
    let template_path = request.template_path.clone();
    let js_slug = template_to_slug(&template_path);
    let js_path = js_relative_path(&template_path);
    let runtime_plan = PageRuntimePlan::from_sources(
        request.template_source.as_deref().unwrap_or_default(),
        &request.config,
    );
    let generated_page_js = runtime_plan.has_runtime();
    let has_motion = runtime_plan.has_motion();
    let mut diagnostics = Vec::new();
    let page_js_file_exists =
        request.page_js_file_exists || request.page_js_tracked || request.page_js_disk_exists;
    let preserves_manual_source = request.preserve_existing_page_js && page_js_file_exists;
    let has_page_js = generated_page_js || preserves_manual_source;

    let page_js = if preserves_manual_source {
        PageJsSaveFilePlan {
            relative_path: js_path.clone(),
            action: PageJsSaveFileAction::None,
            contents: None,
        }
    } else if generated_page_js {
        let next_js = generate_page_js(&runtime_plan);
        if request.existing_page_js_source.as_deref() == Some(next_js.as_str()) {
            PageJsSaveFilePlan {
                relative_path: js_path.clone(),
                action: PageJsSaveFileAction::None,
                contents: None,
            }
        } else {
            PageJsSaveFilePlan {
                relative_path: js_path.clone(),
                action: PageJsSaveFileAction::Write,
                contents: Some(next_js),
            }
        }
    } else if page_js_file_exists {
        PageJsSaveFilePlan {
            relative_path: js_path.clone(),
            action: PageJsSaveFileAction::Remove,
            contents: None,
        }
    } else {
        PageJsSaveFilePlan {
            relative_path: js_path.clone(),
            action: PageJsSaveFileAction::None,
            contents: None,
        }
    };
    let page_js_resource = plan_page_js_generated_resource(&request, &page_js);
    if page_js_resource.blocked {
        diagnostics.push(page_js_resource.message.clone());
    }

    let template = request.template_source.as_ref().map(|source| {
        plan_template_scripts(
            &template_path,
            &js_slug,
            source,
            has_page_js,
            has_motion,
            request.cachebust_assets,
        )
    });

    let base_template = if has_page_js && template_extends(request.template_source.as_deref()) {
        match (
            request.base_template_path.as_ref(),
            request.base_template_source.as_ref(),
        ) {
            (Some(path), Some(source)) => {
                let next = ensure_base_scripts_block(source);
                Some(PageJsSaveTemplatePlan {
                    relative_path: path.clone(),
                    changed: next != *source,
                    contents: next,
                })
            }
            _ => {
                diagnostics.push(
                    "Template-ul extinde un layout, dar layout-ul de bază nu a fost rezolvat pentru block scripts.".to_string(),
                );
                None
            }
        }
    } else {
        None
    };

    let project_structure_changed = page_js.action != PageJsSaveFileAction::None
        || template.as_ref().map(|plan| plan.changed).unwrap_or(false)
        || base_template
            .as_ref()
            .map(|plan| plan.changed)
            .unwrap_or(false);

    PageJsSaveContractPlan {
        template_path,
        js_slug,
        js_path,
        has_page_js,
        has_motion,
        page_js,
        page_js_resource,
        template,
        base_template,
        project_structure_changed,
        diagnostics,
    }
}

pub fn plan_page_js_save_for_project(
    zola_root: &Path,
    session: &ProjectSessionSnapshot,
    store: &FileBufferStore,
    template_path: &str,
    config: PageJsConfig,
    cachebust_assets: bool,
) -> Result<PageJsSaveContractPlan, String> {
    plan_page_js_save_for_project_preserving_source(
        zola_root,
        session,
        store,
        template_path,
        config,
        cachebust_assets,
        false,
    )
}

pub(crate) fn plan_page_js_save_for_project_preserving_source(
    zola_root: &Path,
    session: &ProjectSessionSnapshot,
    store: &FileBufferStore,
    template_path: &str,
    config: PageJsConfig,
    cachebust_assets: bool,
    preserve_existing_page_js: bool,
) -> Result<PageJsSaveContractPlan, String> {
    let request = build_page_js_save_contract_request(
        zola_root,
        session,
        store,
        template_path,
        config,
        cachebust_assets,
        preserve_existing_page_js,
    )?;
    Ok(plan_page_js_save_contract(request))
}

fn build_page_js_save_contract_request(
    root: &Path,
    session: &ProjectSessionSnapshot,
    store: &FileBufferStore,
    template_path: &str,
    config: PageJsConfig,
    cachebust_assets: bool,
    preserve_existing_page_js: bool,
) -> Result<PageJsSaveContractRequest, String> {
    let template_path = normalize_template_path(template_path)?;
    require_page_js_resource_identity_available(session, store, &template_path)?;
    let template_source = read_optional_zola_text(root, store, &template_path)?;
    let (base_template_path, base_template_source) =
        resolve_base_template(root, store, &template_path, template_source.as_deref())?;
    let js_abs = root.join(js_relative_path(&template_path));
    let js_path = js_relative_path(&template_path);
    let page_js_project_path = to_project_relative_path(&js_path);
    let page_js_tracked = store.files.contains_key(&page_js_project_path);
    let page_js_created_in_session = store.files.get(&page_js_project_path).is_some_and(|entry| {
        entry.baseline.size == 0 && entry.baseline.modified_ms == 0 && entry.is_dirty()
    });
    let page_js_disk_exists = js_abs.try_exists().map_err(|error| {
        format!(
            "Nu am putut verifica existența Page JS {}: {}",
            js_abs.display(),
            error
        )
    })?;
    let page_js_file_exists = page_js_tracked || page_js_disk_exists;
    let existing_page_js_source = read_optional_zola_text(root, store, &js_path)?;

    Ok(PageJsSaveContractRequest {
        template_path,
        config,
        cachebust_assets,
        template_source,
        base_template_path,
        base_template_source,
        existing_page_js_source,
        page_js_file_exists,
        page_js_tracked,
        page_js_disk_exists,
        page_js_created_in_session,
        preserve_existing_page_js,
    })
}

fn resolve_base_template(
    root: &Path,
    store: &FileBufferStore,
    template_path: &str,
    template_source: Option<&str>,
) -> Result<(Option<String>, Option<String>), String> {
    let Some(source) = template_source else {
        return Ok((None, None));
    };
    let Some(parent_name) = extract_extends(source) else {
        return Ok((None, None));
    };
    // Resolve the active theme from the current FileBuffer authority, including
    // an unsaved config draft. The bounded/no-follow fallback is used only when
    // a config was not loaded; legacy `ZolaThemeResolver::for_root` would read
    // config directly and could follow a symlink or allocate without limit.
    let active_theme = read_current_active_theme(root, store)?;
    let resolver = ZolaThemeResolver::new(active_theme);
    let Some(base_rel) = resolver.resolve_template_reference(root, template_path, &parent_name)
    else {
        return Ok((None, None));
    };
    let base_source = read_optional_zola_text(root, store, &base_rel)?;
    Ok((Some(base_rel), base_source))
}

fn read_current_active_theme(
    zola_root: &Path,
    store: &FileBufferStore,
) -> Result<Option<String>, String> {
    for relative_path in ["zola.toml", "config.toml"] {
        if let Some(source) = read_optional_zola_text(zola_root, store, relative_path)? {
            return Ok(active_theme_from_source(&source));
        }
    }
    Ok(None)
}

fn plan_template_scripts(
    template_path: &str,
    js_slug: &str,
    source: &str,
    has_page_js: bool,
    has_motion: bool,
    cachebust_assets: bool,
) -> PageJsSaveTemplatePlan {
    let next = if has_page_js {
        if extract_extends(source).is_some() {
            let scripts_html = page_scripts_html(js_slug, has_motion, cachebust_assets);
            ensure_page_scripts_block(source, &scripts_html)
        } else {
            ensure_script_tags(source, js_slug, has_motion, cachebust_assets)
        }
    } else {
        remove_page_scripts_contract(source, js_slug)
    };
    PageJsSaveTemplatePlan {
        relative_path: template_path.to_string(),
        changed: next != source,
        contents: next,
    }
}

fn template_extends(source: Option<&str>) -> bool {
    source.and_then(extract_extends).is_some()
}

pub(crate) fn page_js_text_changes_from_plan(
    plan: &PageJsSaveContractPlan,
) -> Vec<WorkspaceTextChange> {
    let mut changes = Vec::new();
    if plan.page_js.action == PageJsSaveFileAction::Write {
        changes.push(WorkspaceTextChange {
            relative_path: to_project_relative_path(&plan.page_js.relative_path),
            new_text: plan.page_js.contents.clone().unwrap_or_default(),
        });
    }
    if let Some(base_template) = plan.base_template.as_ref().filter(|plan| plan.changed) {
        changes.push(WorkspaceTextChange {
            relative_path: to_project_relative_path(&base_template.relative_path),
            new_text: base_template.contents.clone(),
        });
    }
    if let Some(template) = plan.template.as_ref().filter(|plan| plan.changed) {
        changes.push(WorkspaceTextChange {
            relative_path: to_project_relative_path(&template.relative_path),
            new_text: template.contents.clone(),
        });
    }
    changes
}

pub(crate) fn page_js_text_deletes_from_plan(
    plan: &PageJsSaveContractPlan,
) -> Vec<WorkspaceTextDelete> {
    if plan.page_js.action == PageJsSaveFileAction::Remove {
        return vec![WorkspaceTextDelete {
            relative_path: to_project_relative_path(&plan.page_js.relative_path),
        }];
    }
    Vec::new()
}

fn plan_page_js_generated_resource(
    request: &PageJsSaveContractRequest,
    page_js: &PageJsSaveFilePlan,
) -> PageJsGeneratedResourcePlan {
    let exists_on_disk =
        request.page_js_disk_exists || (request.page_js_file_exists && !request.page_js_tracked);
    let project_path = to_project_relative_path(&page_js.relative_path);

    let (status, blocked, message) = if request.page_js_tracked
        && !exists_on_disk
        && !request.page_js_created_in_session
    {
        (
            PageJsGeneratedResourceStatus::BlockedMissingTrackedDisk,
            true,
            format!(
                "Save Page JS a fost blocat pentru {project_path}: FileBufferStore are baseline, dar fișierul generat lipsește de pe disk."
            ),
        )
    } else if exists_on_disk && !request.page_js_tracked {
        (
            PageJsGeneratedResourceStatus::BlockedUntrackedExisting,
            true,
            format!(
                "Save Page JS a fost blocat pentru {project_path}: fișierul generat există pe disk, dar nu are baseline în FileBufferStore."
            ),
        )
    } else {
        let status = match &page_js.action {
            PageJsSaveFileAction::None if request.page_js_tracked => {
                PageJsGeneratedResourceStatus::UnchangedTracked
            }
            PageJsSaveFileAction::None => PageJsGeneratedResourceStatus::NoopMissing,
            PageJsSaveFileAction::Write if request.page_js_tracked => {
                PageJsGeneratedResourceStatus::UpdateTracked
            }
            PageJsSaveFileAction::Write => PageJsGeneratedResourceStatus::CreateMissing,
            PageJsSaveFileAction::Remove => PageJsGeneratedResourceStatus::DeleteTracked,
        };
        (
            status,
            false,
            match &page_js.action {
                PageJsSaveFileAction::None => {
                    format!("Save Page JS nu are mutație text pentru {project_path}.")
                }
                PageJsSaveFileAction::Write if request.page_js_tracked => {
                    format!("Save Page JS va actualiza resursa generată urmărită {project_path}.")
                }
                PageJsSaveFileAction::Write => {
                    format!(
                        "Save Page JS va crea resursa generată {project_path} fără baseline fals."
                    )
                }
                PageJsSaveFileAction::Remove => {
                    format!("Save Page JS va șterge resursa generată urmărită {project_path}.")
                }
            },
        )
    };

    PageJsGeneratedResourcePlan {
        zola_path: page_js.relative_path.clone(),
        project_path,
        action: page_js.action.clone(),
        status,
        tracked_in_file_buffer: request.page_js_tracked,
        exists_on_disk,
        blocked,
        message,
    }
}

fn require_complete_page_js_source_inventory(
    _session: &ProjectSessionSnapshot,
    store: &FileBufferStore,
) -> Result<(), String> {
    if let Some(diagnostic) = store.diagnostics.first() {
        return Err(format!(
            "FileBufferStore nu are acoperire completă: {}",
            diagnostic.message
        ));
    }
    Ok(())
}

fn normalized_template_from_project_path(path: &str) -> Option<String> {
    normalize_template_path(path).ok()
}

fn require_page_js_resource_identity_available(
    session: &ProjectSessionSnapshot,
    store: &FileBufferStore,
    template_path: &str,
) -> Result<(), String> {
    require_complete_page_js_source_inventory(session, store)?;
    let target_resource = js_relative_path(template_path);
    let target_asset = target_resource
        .strip_prefix("static/")
        .unwrap_or(&target_resource);

    for (project_path, entry) in store.files.iter() {
        let Some(candidate) = normalized_template_from_project_path(project_path) else {
            continue;
        };
        if candidate == template_path || js_relative_path(&candidate) != target_resource {
            continue;
        }
        if template_contains_asset_path(entry.current_text(), target_asset) {
            return Err(format!(
                "Page JS a blocat o coliziune de resursă: template-urile {template_path} și {candidate} revendică ambele {target_resource}. Niciun fișier nu a fost modificat."
            ));
        }
    }

    for diagnostic in &store.diagnostics {
        let Some(path) = diagnostic.relative_path.as_deref() else {
            continue;
        };
        let Some(candidate) = normalized_template_from_project_path(path) else {
            continue;
        };
        if candidate != template_path && js_relative_path(&candidate) == target_resource {
            return Err(format!(
                "Page JS nu poate exclude o coliziune cu {candidate}, deoarece FileBufferStore raportează: {}",
                diagnostic.message
            ));
        }
    }
    Ok(())
}

fn read_optional_zola_text(
    zola_root: &Path,
    store: &FileBufferStore,
    zola_relative_path: &str,
) -> Result<Option<String>, String> {
    let project_root = Path::new(&store.project_root);
    let expected_zola_root = project_root.to_path_buf();
    if zola_root != expected_zola_root {
        return Err(format!(
            "Page JS a refuzat autorități divergente: Zola root {} nu corespunde FileBufferStore {}.",
            zola_root.display(),
            expected_zola_root.display()
        ));
    }
    let project_relative_path = to_project_relative_path(zola_relative_path);
    read_optional_project_text(project_root, store, &project_relative_path)
}

fn to_project_relative_path(path: &str) -> String {
    let normalized = path.trim().trim_start_matches('/');
    normalized.to_string()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::json;

    use super::*;
    use crate::js::{MotionDocument, MotionRuntimeContract};

    fn request(template_source: Option<&str>, config: PageJsConfig) -> PageJsSaveContractRequest {
        PageJsSaveContractRequest {
            template_path: "templates/index.html".to_string(),
            config,
            cachebust_assets: false,
            template_source: template_source.map(str::to_string),
            base_template_path: None,
            base_template_source: None,
            existing_page_js_source: None,
            page_js_file_exists: false,
            page_js_tracked: false,
            page_js_disk_exists: false,
            page_js_created_in_session: false,
            preserve_existing_page_js: false,
        }
    }

    #[test]
    fn manual_page_js_override_is_preserved_and_remains_linked() {
        let mut input = request(Some("<main></main>"), PageJsConfig::default());
        input.existing_page_js_source = Some("window.userCode = true;\n".to_string());
        input.page_js_file_exists = true;
        input.page_js_tracked = true;
        input.page_js_disk_exists = true;
        input.preserve_existing_page_js = true;

        let plan = plan_page_js_save_contract(input);

        assert!(plan.has_page_js);
        assert_eq!(plan.page_js.action, PageJsSaveFileAction::None);
        assert!(plan
            .template
            .as_ref()
            .unwrap()
            .contents
            .contains("js/pana-index.js"));
    }

    #[test]
    fn project_plan_rejects_template_traversal_before_reading_outside_zola_root() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let project = std::env::temp_dir().join(format!("pana-page-js-plan-{nonce}"));
        let zola_root = project.to_path_buf();
        fs::create_dir_all(&zola_root).unwrap();
        fs::write(project.join("secret.html"), "TOP_SECRET_PAGE_JS_SENTINEL").unwrap();
        let live = ProjectSessionSnapshot {
            schema_version: 1,
            id: "page-js-plan-traversal".to_string(),
            project_root: project.to_string_lossy().into_owned(),
            zola_root: zola_root.to_string_lossy().into_owned(),
            session_dir: project.join("session").to_string_lossy().into_owned(),
            manifest_path: project
                .join("session/session.json")
                .to_string_lossy()
                .into_owned(),
            opened_at_ms: 1,
            last_seen_at_ms: 1,
            root_fingerprint: crate::kernel::project_session::ProjectRootFingerprint {
                canonical_path: project.to_string_lossy().into_owned(),
                modified_ms: 1,
                size: 0,
                readonly: false,
                unix_device: None,
                unix_inode: None,
            },
            scan_summary: crate::kernel::project_session::ProjectSessionScanSummary {
                active_theme: None,
                file_count: 0,
                directory_count: 0,
            },
        };
        let store = FileBufferStore::for_project_session(
            &live,
            1,
            crate::kernel::file_buffer_store::FileBufferStoreLimits {
                max_files: 10,
                max_file_bytes: 4096,
                max_total_bytes: 8192,
            },
        );

        let error = plan_page_js_save_for_project(
            &zola_root,
            &live,
            &store,
            "../secret.html",
            PageJsConfig::default(),
            false,
        )
        .unwrap_err();

        assert!(error.contains("traversal"));
        fs::remove_dir_all(project).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn project_plan_rejects_symlink_template_and_oversized_source() {
        use std::os::unix::fs::symlink;

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let project = std::env::temp_dir().join(format!("pana-page-js-safe-read-{nonce}"));
        let zola_root = project.to_path_buf();
        let templates = zola_root.join("templates");
        fs::create_dir_all(&templates).unwrap();
        let outside = std::env::temp_dir().join(format!("pana-page-js-secret-{nonce}.html"));
        fs::write(&outside, "TOP_SECRET_PAGE_JS_SENTINEL").unwrap();
        symlink(&outside, templates.join("leak.html")).unwrap();
        fs::write(templates.join("large.html"), vec![b'x'; 4097]).unwrap();
        let live = ProjectSessionSnapshot {
            schema_version: 1,
            id: "page-js-safe-read".to_string(),
            project_root: project.to_string_lossy().into_owned(),
            zola_root: zola_root.to_string_lossy().into_owned(),
            session_dir: project.join("session").to_string_lossy().into_owned(),
            manifest_path: project
                .join("session/session.json")
                .to_string_lossy()
                .into_owned(),
            opened_at_ms: 1,
            last_seen_at_ms: 1,
            root_fingerprint: crate::kernel::project_session::ProjectRootFingerprint {
                canonical_path: project.to_string_lossy().into_owned(),
                modified_ms: 1,
                size: 0,
                readonly: false,
                unix_device: None,
                unix_inode: None,
            },
            scan_summary: crate::kernel::project_session::ProjectSessionScanSummary {
                active_theme: None,
                file_count: 0,
                directory_count: 0,
            },
        };
        let store = FileBufferStore::for_project_session(
            &live,
            1,
            crate::kernel::file_buffer_store::FileBufferStoreLimits {
                max_files: 10,
                max_file_bytes: 4096,
                max_total_bytes: 8192,
            },
        );

        let symlink_error = plan_page_js_save_for_project(
            &zola_root,
            &live,
            &store,
            "templates/leak.html",
            PageJsConfig::default(),
            false,
        )
        .unwrap_err();
        let oversized_error = plan_page_js_save_for_project(
            &zola_root,
            &live,
            &store,
            "templates/large.html",
            PageJsConfig::default(),
            false,
        )
        .unwrap_err();

        assert!(symlink_error.contains("sigur"));
        assert!(oversized_error.contains("depășesc limita"));
        fs::remove_dir_all(project).unwrap();
        fs::remove_file(outside).unwrap();
    }

    #[test]
    fn project_plan_blocks_legacy_page_js_resource_collision_before_write() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let project = std::env::temp_dir().join(format!("pana-page-js-collision-{nonce}"));
        let zola_root = project.to_path_buf();
        fs::create_dir_all(zola_root.join("templates/foo")).unwrap();
        let target = r#"<main data-pana-block="tabs">Target</main>"#;
        let owner = r#"<main>Owner</main><script src="{{ get_url(path = 'js/pana-foo-bar.js') }}" defer></script>"#;
        fs::write(zola_root.join("templates/foo/bar.html"), target).unwrap();
        fs::write(zola_root.join("templates/foo-bar.html"), owner).unwrap();
        let live = ProjectSessionSnapshot {
            schema_version: 1,
            id: "page-js-collision".to_string(),
            project_root: project.to_string_lossy().into_owned(),
            zola_root: zola_root.to_string_lossy().into_owned(),
            session_dir: project.join("session").to_string_lossy().into_owned(),
            manifest_path: project
                .join("session/session.json")
                .to_string_lossy()
                .into_owned(),
            opened_at_ms: 1,
            last_seen_at_ms: 1,
            root_fingerprint: crate::kernel::project_session::ProjectRootFingerprint {
                canonical_path: project.to_string_lossy().into_owned(),
                modified_ms: 1,
                size: 0,
                readonly: false,
                unix_device: None,
                unix_inode: None,
            },
            scan_summary: crate::kernel::project_session::ProjectSessionScanSummary {
                active_theme: None,
                file_count: 2,
                directory_count: 2,
            },
        };
        let mut store = FileBufferStore::for_project_session(
            &live,
            1,
            crate::kernel::file_buffer_store::FileBufferStoreLimits {
                max_files: 10,
                max_file_bytes: 4096,
                max_total_bytes: 8192,
            },
        );
        store
            .record_saved_text("templates/foo/bar.html", target.to_string())
            .unwrap();
        store
            .record_saved_text("templates/foo-bar.html", owner.to_string())
            .unwrap();

        let error = plan_page_js_save_for_project(
            &zola_root,
            &live,
            &store,
            "templates/foo/bar.html",
            PageJsConfig::default(),
            false,
        )
        .unwrap_err();

        assert!(error.contains("coliziune de resursă"));
        assert!(!zola_root.join("static/js/pana-foo-bar.js").exists());
        fs::remove_dir_all(project).unwrap();
    }

    #[test]
    fn project_plan_resolves_base_from_current_bounded_config_not_session_snapshot() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let project = std::env::temp_dir().join(format!("pana-page-js-theme-draft-{nonce}"));
        let zola_root = project.to_path_buf();
        fs::create_dir_all(zola_root.join("templates")).unwrap();
        fs::create_dir_all(zola_root.join("themes/theme-b/templates")).unwrap();
        let child = r#"{% extends "base.html" %}{% block content %}<main data-pana-block="tabs"></main>{% endblock content %}"#;
        let base_b = "<html><body>{% block content %}{% endblock content %}</body></html>";
        fs::write(zola_root.join("zola.toml"), "theme = 'theme-a'\n").unwrap();
        fs::write(zola_root.join("templates/index.html"), child).unwrap();
        fs::write(zola_root.join("themes/theme-b/templates/base.html"), base_b).unwrap();
        let live = ProjectSessionSnapshot {
            schema_version: 1,
            id: "page-js-theme-draft".to_string(),
            project_root: project.to_string_lossy().into_owned(),
            zola_root: zola_root.to_string_lossy().into_owned(),
            session_dir: project.join("session").to_string_lossy().into_owned(),
            manifest_path: project
                .join("session/session.json")
                .to_string_lossy()
                .into_owned(),
            opened_at_ms: 1,
            last_seen_at_ms: 1,
            root_fingerprint: crate::kernel::project_session::ProjectRootFingerprint {
                canonical_path: project.to_string_lossy().into_owned(),
                modified_ms: 1,
                size: 0,
                readonly: false,
                unix_device: None,
                unix_inode: None,
            },
            scan_summary: crate::kernel::project_session::ProjectSessionScanSummary {
                active_theme: Some("theme-a".to_string()),
                file_count: 3,
                directory_count: 4,
            },
        };
        let mut store = FileBufferStore::for_project_session(
            &live,
            1,
            crate::kernel::file_buffer_store::FileBufferStoreLimits {
                max_files: 10,
                max_file_bytes: 4096,
                max_total_bytes: 16384,
            },
        );
        store
            .record_saved_text("zola.toml", "theme = 'theme-b'\n".to_string())
            .unwrap();
        store
            .record_saved_text("templates/index.html", child.to_string())
            .unwrap();
        store
            .record_saved_text("themes/theme-b/templates/base.html", base_b.to_string())
            .unwrap();

        let plan = plan_page_js_save_for_project(
            &zola_root,
            &live,
            &store,
            "templates/index.html",
            PageJsConfig::default(),
            false,
        )
        .unwrap();

        assert_eq!(
            plan.base_template
                .as_ref()
                .map(|base| base.relative_path.as_str()),
            Some("themes/theme-b/templates/base.html")
        );
        fs::remove_dir_all(project).unwrap();
    }

    #[test]
    fn plans_child_template_scripts_and_base_block_for_motion_page_js() {
        let mut req = request(
            Some(
                r#"{% extends "base.html" %}
{% block content %}<main></main>{% endblock content %}
"#,
            ),
            PageJsConfig {
                motion: Some(
                    MotionDocument::from_value(json!({
                        "schemaVersion": 2,
                        "animeVersion": MotionRuntimeContract::current().anime_version,
                        "interactions": [{
                            "id": "animation-a",
                            "name": "Hero",
                            "trigger": { "type": "load" },
                            "triggerTarget": { "kind": "element", "dataAnim": "hero" },
                            "actions": [{
                                "type": "animate",
                                "id": "fade",
                                "name": "Fade",
                                "target": { "kind": "element", "dataAnim": "hero" },
                                "properties": [{
                                    "id": "opacity",
                                    "name": "opacity",
                                    "category": "style",
                                    "from": { "kind": "number", "value": "0" },
                                    "to": { "kind": "number", "value": "1" }
                                }]
                            }]
                        }]
                    }))
                    .expect("strict Motion document"),
                ),
            },
        );
        req.base_template_path = Some("templates/base.html".to_string());
        req.base_template_source =
            Some("<html><body>{% block content %}{% endblock content %}</body></html>".to_string());

        let plan = plan_page_js_save_contract(req);

        assert!(plan.has_page_js);
        assert!(plan.has_motion);
        assert_eq!(plan.page_js.action, PageJsSaveFileAction::Write);
        let generated = plan
            .page_js
            .contents
            .as_deref()
            .expect("generated Motion JS");
        assert!(!generated.contains("PanaBlockRuntime"));
        assert!(!generated.contains("PANA BLOCK PROVIDER:"));
        assert!(plan
            .template
            .as_ref()
            .unwrap()
            .contents
            .contains("{{ super() }}"));
        assert!(!plan
            .template
            .as_ref()
            .unwrap()
            .contents
            .contains("js/anime.min.js"));
        assert!(!plan
            .template
            .as_ref()
            .unwrap()
            .contents
            .contains("js/pana-motion-runtime.js"));
        assert!(plan
            .template
            .as_ref()
            .unwrap()
            .contents
            .contains("js/pana-index.js"));
        assert!(plan
            .base_template
            .as_ref()
            .unwrap()
            .contents
            .contains("{% block scripts %}"));
    }

    #[test]
    fn plans_generated_contract_removal_when_page_js_is_empty() {
        let mut req = request(
            Some(
                r#"{% block scripts %}
{{ super() }}
  <script src="/js/anime.min.js" defer></script>
  <script src="/js/pana-index.js" defer></script>
{% endblock scripts %}
"#,
            ),
            PageJsConfig::default(),
        );
        req.page_js_file_exists = true;
        req.page_js_tracked = true;
        req.page_js_disk_exists = true;

        let plan = plan_page_js_save_contract(req);

        assert!(!plan.has_page_js);
        assert_eq!(plan.page_js.action, PageJsSaveFileAction::Remove);
        assert_eq!(
            plan.page_js_resource.status,
            PageJsGeneratedResourceStatus::DeleteTracked
        );
        assert!(plan.template.as_ref().unwrap().changed);
        assert!(!plan
            .template
            .as_ref()
            .unwrap()
            .contents
            .contains("pana-index.js"));
        assert!(!plan
            .template
            .as_ref()
            .unwrap()
            .contents
            .contains("{% block scripts %}"));
    }

    #[test]
    fn unknown_block_metadata_cannot_keep_a_generated_page_script_alive() {
        let mut req = request(
            Some(r#"<script src="/js/pana-index.js" defer></script>"#),
            PageJsConfig::default(),
        );
        req.page_js_file_exists = true;
        req.page_js_tracked = true;
        req.page_js_disk_exists = true;

        let plan = plan_page_js_save_contract(req);

        assert!(!plan.has_page_js);
        assert_eq!(plan.page_js.action, PageJsSaveFileAction::Remove);
        assert_eq!(
            plan.page_js_resource.status,
            PageJsGeneratedResourceStatus::DeleteTracked
        );
    }

    #[test]
    fn plans_standalone_component_js_without_stale_anime_script() {
        let plan = plan_page_js_save_contract(request(
            Some(
                r#"<body>
  <section data-pana-block="accordion"></section>
  <script src="/js/anime.min.js" defer></script>
</body>"#,
            ),
            PageJsConfig::default(),
        ));

        let template = plan.template.as_ref().unwrap();
        assert!(plan.has_page_js);
        assert!(!plan.has_motion);
        assert!(template.contents.contains("js/pana-index.js"));
        assert!(!template.contents.contains("js/anime.min.js"));
        assert!(!template.contents.contains("js/pana-motion-runtime.js"));
    }

    #[test]
    fn skips_page_js_write_when_generated_source_is_unchanged() {
        let template = r#"<main data-pana-block="tabs"></main>"#;
        let config = PageJsConfig::default();
        let existing = generate_page_js(&PageRuntimePlan::from_sources(template, &config));
        let mut req = request(Some(template), config);
        req.existing_page_js_source = Some(existing);
        req.page_js_file_exists = true;
        req.page_js_tracked = true;
        req.page_js_disk_exists = true;

        let plan = plan_page_js_save_contract(req);

        assert_eq!(plan.page_js.action, PageJsSaveFileAction::None);
        assert_eq!(
            plan.page_js_resource.status,
            PageJsGeneratedResourceStatus::UnchangedTracked
        );
    }

    #[test]
    fn blocks_untracked_existing_generated_page_js_before_side_effects() {
        let mut req = request(
            Some(r#"<main data-pana-block="tabs"></main>"#),
            PageJsConfig::default(),
        );
        req.page_js_file_exists = true;
        req.page_js_disk_exists = true;
        req.page_js_tracked = false;

        let plan = plan_page_js_save_contract(req);

        assert_eq!(plan.page_js.action, PageJsSaveFileAction::Write);
        assert!(plan.page_js_resource.blocked);
        assert_eq!(
            plan.page_js_resource.status,
            PageJsGeneratedResourceStatus::BlockedUntrackedExisting
        );
        assert!(plan
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.contains("nu are baseline în FileBufferStore") }));
    }

    #[test]
    fn blocks_tracked_generated_page_js_missing_on_disk() {
        let mut req = request(
            Some(r#"<main data-pana-block="tabs"></main>"#),
            PageJsConfig::default(),
        );
        req.page_js_file_exists = true;
        req.page_js_disk_exists = false;
        req.page_js_tracked = true;

        let plan = plan_page_js_save_contract(req);

        assert_eq!(plan.page_js.action, PageJsSaveFileAction::Write);
        assert!(plan.page_js_resource.blocked);
        assert_eq!(
            plan.page_js_resource.status,
            PageJsGeneratedResourceStatus::BlockedMissingTrackedDisk
        );
        assert!(plan
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("lipsește de pe disk")));
    }
}
