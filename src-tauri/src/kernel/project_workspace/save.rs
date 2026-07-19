use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

use tauri::{AppHandle, Runtime};

use crate::{
    js::{
        page_js_text_changes_from_plan, page_js_text_deletes_from_plan,
        plan_page_js_save_for_project, PageJsConfig,
    },
    kernel::{
        file_buffer_store::{
            hash_bytes, hash_text, FileBufferMutationExpectation, FileBufferStore,
        },
        generated_assets::{
            plan_generated_asset_intent, registry::generated_asset_definition,
            GeneratedAssetAction, GeneratedAssetId, GeneratedAssetPlanStatus,
        },
        project_path::normalize_project_relative_path,
        write_authority::WriteReceipt,
    },
    project::{
        project_disk_manifest_changed_paths, read_project_disk_manifest, resolve_project_write_path,
    },
};

use super::save_journal::{
    clear_project_workspace_save_journal, prepare_project_workspace_save_journal,
};
use super::{
    disk_boundary::{
        delete_binary_file, delete_text_file, read_disk_text_baseline,
        remove_created_text_file_for_undo, save_binary_file, save_text_file,
        ProjectWorkspaceDiskError,
    },
    model::{
        ProjectWorkspaceIdentity, ProjectWorkspaceSaveError, ProjectWorkspaceSaveReceipt,
        ProjectWorkspaceSaveStatus, PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_BYTES,
        PROJECT_WORKSPACE_SCHEMA_VERSION,
    },
    workspace::ProjectWorkspace,
};

static PROJECT_WORKSPACE_SAVE_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub(super) struct ProjectWorkspaceSavePlan {
    pub(super) id: String,
    pub(super) changes: Vec<ProjectWorkspacePlannedWrite>,
    pub(super) deletes: Vec<ProjectWorkspacePlannedDelete>,
    pub(super) binary_changes: Vec<ProjectWorkspacePlannedBinaryWrite>,
    pub(super) binary_deletes: Vec<ProjectWorkspacePlannedBinaryDelete>,
    pub(super) touched_files: Vec<String>,
}

#[derive(Clone, Debug)]
pub(super) struct ProjectWorkspacePlannedWrite {
    pub(super) relative_path: String,
    pub(super) existed_before: bool,
    pub(super) before_text: String,
    pub(super) new_text: String,
    pub(super) before_hash: String,
    pub(super) new_hash: String,
}

#[derive(Clone, Debug)]
pub(super) struct ProjectWorkspacePlannedDelete {
    pub(super) relative_path: String,
    pub(super) before_text: String,
    pub(super) before_hash: String,
}

#[derive(Clone, Debug)]
pub(super) struct ProjectWorkspacePlannedBinaryWrite {
    pub(super) relative_path: String,
    pub(super) existed_before: bool,
    pub(super) before_bytes: Vec<u8>,
    pub(super) new_bytes: Vec<u8>,
    pub(super) before_hash: String,
    pub(super) new_hash: String,
}

#[derive(Clone, Debug)]
pub(super) struct ProjectWorkspacePlannedBinaryDelete {
    pub(super) relative_path: String,
    pub(super) before_bytes: Vec<u8>,
    pub(super) before_hash: String,
}

pub(super) enum ProjectWorkspaceSavePlannedFile<'a> {
    Write(&'a ProjectWorkspacePlannedWrite),
    Delete(&'a ProjectWorkspacePlannedDelete),
    BinaryWrite(&'a ProjectWorkspacePlannedBinaryWrite),
    BinaryDelete(&'a ProjectWorkspacePlannedBinaryDelete),
}

impl ProjectWorkspaceSavePlan {
    pub(super) fn files(&self) -> impl Iterator<Item = ProjectWorkspaceSavePlannedFile<'_>> {
        self.changes
            .iter()
            .map(ProjectWorkspaceSavePlannedFile::Write)
            .chain(
                self.deletes
                    .iter()
                    .map(ProjectWorkspaceSavePlannedFile::Delete),
            )
            .chain(
                self.binary_changes
                    .iter()
                    .map(ProjectWorkspaceSavePlannedFile::BinaryWrite),
            )
            .chain(
                self.binary_deletes
                    .iter()
                    .map(ProjectWorkspaceSavePlannedFile::BinaryDelete),
            )
    }
}

#[derive(Clone)]
enum AppliedSaveOperation {
    Write(ProjectWorkspacePlannedWrite),
    Delete(ProjectWorkspacePlannedDelete),
    BinaryWrite(ProjectWorkspacePlannedBinaryWrite),
    BinaryDelete(ProjectWorkspacePlannedBinaryDelete),
}

pub fn save_project_workspace<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    identity: &ProjectWorkspaceIdentity,
) -> Result<ProjectWorkspaceSaveReceipt, ProjectWorkspaceSaveError> {
    workspace
        .require_identity(identity)
        .map_err(ProjectWorkspaceSaveError::rejected)?;
    workspace
        .accepted_disk
        .require_live_complete(
            &workspace.runtime_session_id(),
            &workspace.session.project_root,
            project_root,
        )
        .map_err(ProjectWorkspaceSaveError::rejected)?;
    let revision_before = workspace.revision;
    let disk_generation_before = workspace.accepted_disk.generation;
    let live_manifest =
        read_project_disk_manifest(project_root).map_err(ProjectWorkspaceSaveError::rejected)?;
    if live_manifest != workspace.accepted_disk.manifest {
        return Err(ProjectWorkspaceSaveError::rejected(
            "Save a fost blocat: proiectul s-a schimbat extern față de baseline-ul acceptat de ProjectWorkspace. Reconciliază sau reîncarcă înainte de Save.",
        ));
    }
    let materialized = materialize_workspace_for_save(project_root, workspace)
        .map_err(ProjectWorkspaceSaveError::rejected)?;
    let writes = workspace_document_writes(workspace, &materialized.documents);
    let deletes = materialized
        .deleted_documents
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let binary_writes = workspace_binary_writes(project_root, workspace, &materialized)
        .map_err(ProjectWorkspaceSaveError::rejected)?;
    let binary_deletes = materialized
        .deleted_binary_resources
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    if writes.is_empty()
        && deletes.is_empty()
        && binary_writes.is_empty()
        && binary_deletes.is_empty()
    {
        if workspace.page_js.dirty_count() > 0
            || !workspace.binary_resources.is_empty()
            || !workspace.deleted_binary_resources.is_empty()
        {
            let accepted_disk = workspace.accepted_disk.clone();
            workspace
                .accept_saved_documents(
                    identity,
                    materialized.documents,
                    materialized.accepted_page_js,
                    accepted_disk,
                )
                .map_err(ProjectWorkspaceSaveError::rejected)?;
            return Ok(ProjectWorkspaceSaveReceipt {
                schema_version: PROJECT_WORKSPACE_SCHEMA_VERSION,
                transaction_id: None,
                status: ProjectWorkspaceSaveStatus::Saved,
                project_root: workspace.session.project_root.clone(),
                runtime_session_id: workspace.runtime_session_id(),
                revision_before,
                revision_after: workspace.revision,
                disk_generation_before,
                disk_generation_after: disk_generation_before,
                written_files: Vec::new(),
                removed_files: Vec::new(),
                write_receipts: Vec::new(),
                accepted_manifest: workspace.accepted_disk.manifest.clone(),
                workspace: workspace.snapshot(),
            });
        }
        return Ok(ProjectWorkspaceSaveReceipt {
            schema_version: PROJECT_WORKSPACE_SCHEMA_VERSION,
            transaction_id: None,
            status: ProjectWorkspaceSaveStatus::Noop,
            project_root: workspace.session.project_root.clone(),
            runtime_session_id: workspace.runtime_session_id(),
            revision_before,
            revision_after: revision_before,
            disk_generation_before,
            disk_generation_after: disk_generation_before,
            written_files: Vec::new(),
            removed_files: Vec::new(),
            write_receipts: Vec::new(),
            accepted_manifest: workspace.accepted_disk.manifest.clone(),
            workspace: workspace.snapshot(),
        });
    }

    let mut transaction_store = accepted_transaction_store(workspace);
    let plan = plan_project_workspace_save(
        project_root,
        &transaction_store,
        writes,
        deletes,
        binary_writes,
        binary_deletes,
        revision_before,
    )
    .map_err(ProjectWorkspaceSaveError::rejected)?;
    let transaction_id = plan.id.clone();
    let touched_files = plan.touched_files.clone();
    prepare_project_workspace_save_journal(app, &workspace.session, revision_before, &plan)
        .map_err(|error| {
            ProjectWorkspaceSaveError::recovery_required(
                transaction_id.clone(),
                touched_files.clone(),
                Vec::new(),
                format!("Jurnalul tranzacției Save nu a putut fi pregătit durabil: {error}"),
            )
        })?;

    let mut applied = Vec::with_capacity(plan.touched_files.len());
    let mut write_receipts = Vec::with_capacity(plan.touched_files.len());
    for change in &plan.changes {
        match save_text_file(
            app,
            project_root,
            &mut transaction_store,
            &change.relative_path,
            change.new_text.clone(),
            None,
        ) {
            Ok(result) => {
                write_receipts.push(result.receipt);
                applied.push(AppliedSaveOperation::Write(change.clone()));
            }
            Err(error) => {
                return fail_save_transaction(
                    app,
                    project_root,
                    workspace,
                    &plan.id,
                    &plan.touched_files,
                    &mut transaction_store,
                    &applied,
                    write_receipts,
                    error,
                );
            }
        }
    }
    for delete in &plan.deletes {
        match delete_text_file(
            app,
            project_root,
            &mut transaction_store,
            &delete.relative_path,
        ) {
            Ok(result) => {
                write_receipts.push(result.receipt);
                applied.push(AppliedSaveOperation::Delete(delete.clone()));
            }
            Err(error) => {
                return fail_save_transaction(
                    app,
                    project_root,
                    workspace,
                    &plan.id,
                    &plan.touched_files,
                    &mut transaction_store,
                    &applied,
                    write_receipts,
                    error,
                );
            }
        }
    }
    for change in &plan.binary_changes {
        match save_binary_file(
            app,
            project_root,
            &workspace.runtime_session_id(),
            &change.relative_path,
            &change.new_bytes,
            change.existed_before,
            &change.before_hash,
        ) {
            Ok(receipt) => {
                write_receipts.push(receipt);
                applied.push(AppliedSaveOperation::BinaryWrite(change.clone()));
            }
            Err(error) => {
                return fail_save_transaction(
                    app,
                    project_root,
                    workspace,
                    &plan.id,
                    &plan.touched_files,
                    &mut transaction_store,
                    &applied,
                    write_receipts,
                    error,
                );
            }
        }
    }
    for delete in &plan.binary_deletes {
        match delete_binary_file(
            app,
            project_root,
            &workspace.runtime_session_id(),
            &delete.relative_path,
            &delete.before_hash,
        ) {
            Ok(receipt) => {
                write_receipts.push(receipt);
                applied.push(AppliedSaveOperation::BinaryDelete(delete.clone()));
            }
            Err(error) => {
                return fail_save_transaction(
                    app,
                    project_root,
                    workspace,
                    &plan.id,
                    &plan.touched_files,
                    &mut transaction_store,
                    &applied,
                    write_receipts,
                    error,
                );
            }
        }
    }

    let manifest = match read_project_disk_manifest(project_root) {
        Ok(manifest) if !manifest.truncated => manifest,
        Ok(_) => {
            return rollback_after_post_commit_failure(
                app,
                project_root,
                workspace,
                &plan.id,
                &plan.touched_files,
                &mut transaction_store,
                &applied,
                write_receipts,
                "Manifestul rezultat după Save este trunchiat.".to_string(),
            );
        }
        Err(error) => {
            return rollback_after_post_commit_failure(
                app,
                project_root,
                workspace,
                &plan.id,
                &plan.touched_files,
                &mut transaction_store,
                &applied,
                write_receipts,
                format!("Manifestul rezultat după Save nu a putut fi citit: {error}"),
            );
        }
    };
    let changed_paths =
        match project_disk_manifest_changed_paths(&workspace.accepted_disk.manifest, &manifest) {
            Ok(paths) => paths,
            Err(error) => {
                return rollback_after_post_commit_failure(
                    app,
                    project_root,
                    workspace,
                    &plan.id,
                    &plan.touched_files,
                    &mut transaction_store,
                    &applied,
                    write_receipts,
                    format!("Save nu a putut verifica domeniul exact al efectelor: {error}"),
                );
            }
        };
    let touched = touched_files.iter().cloned().collect::<BTreeSet<_>>();
    let unexpected = changed_paths
        .into_iter()
        .filter(|path| !touched.contains(path))
        .collect::<Vec<_>>();
    if !unexpected.is_empty() {
        return rollback_after_post_commit_failure(
            app,
            project_root,
            workspace,
            &plan.id,
            &plan.touched_files,
            &mut transaction_store,
            &applied,
            write_receipts,
            format!(
                "Save a detectat schimbări externe concurente în afara tranzacției: {}.",
                unexpected.join(", ")
            ),
        );
    }
    let accepted_disk = match workspace.accepted_disk.next(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        manifest.clone(),
    ) {
        Ok(accepted) => accepted,
        Err(error) => {
            return rollback_after_post_commit_failure(
                app,
                project_root,
                workspace,
                &plan.id,
                &plan.touched_files,
                &mut transaction_store,
                &applied,
                write_receipts,
                format!("Noul baseline disk nu a putut fi acceptat: {error}"),
            );
        }
    };
    if let Err(error) = clear_project_workspace_save_journal(app, &workspace.session, &plan.id) {
        return rollback_after_post_commit_failure(
            app,
            project_root,
            workspace,
            &plan.id,
            &plan.touched_files,
            &mut transaction_store,
            &applied,
            write_receipts,
            format!("Jurnalul tranzacției Save nu a putut fi finalizat: {error}"),
        );
    }

    let accepted_page_js = materialized.accepted_page_js;
    workspace
        .accept_saved_documents(identity, transaction_store, accepted_page_js, accepted_disk)
        .map_err(|error| {
            ProjectWorkspaceSaveError::recovery_required(
                transaction_id.clone(),
                touched_files.clone(),
                write_receipts.clone(),
                format!(
                    "Fișierele au fost persistate, dar ProjectWorkspace nu a putut accepta noul baseline: {error}"
                ),
            )
        })?;

    Ok(ProjectWorkspaceSaveReceipt {
        schema_version: PROJECT_WORKSPACE_SCHEMA_VERSION,
        transaction_id: Some(transaction_id),
        status: ProjectWorkspaceSaveStatus::Saved,
        project_root: workspace.session.project_root.clone(),
        runtime_session_id: workspace.runtime_session_id(),
        revision_before,
        revision_after: workspace.revision,
        disk_generation_before,
        disk_generation_after: workspace.accepted_disk.generation,
        written_files: plan
            .changes
            .iter()
            .map(|change| change.relative_path.clone())
            .chain(
                plan.binary_changes
                    .iter()
                    .map(|change| change.relative_path.clone()),
            )
            .collect(),
        removed_files: plan
            .deletes
            .iter()
            .map(|delete| delete.relative_path.clone())
            .chain(
                plan.binary_deletes
                    .iter()
                    .map(|delete| delete.relative_path.clone()),
            )
            .collect(),
        write_receipts,
        accepted_manifest: manifest,
        workspace: workspace.snapshot(),
    })
}

pub(super) struct MaterializedWorkspaceSave {
    pub(super) documents: FileBufferStore,
    pub(super) deleted_documents: BTreeSet<String>,
    pub(super) accepted_page_js: BTreeMap<String, PageJsConfig>,
    pub(super) binary_resources: BTreeMap<String, super::model::WorkspaceBinaryResource>,
    pub(super) deleted_binary_resources: BTreeSet<String>,
}

pub(super) fn materialize_workspace_for_projection(
    workspace: &ProjectWorkspace,
) -> Result<MaterializedWorkspaceSave, String> {
    materialize_workspace_for_save(Path::new(&workspace.session.project_root), workspace)
}

fn materialize_workspace_for_save(
    project_root: &Path,
    workspace: &ProjectWorkspace,
) -> Result<MaterializedWorkspaceSave, String> {
    let zola_root = Path::new(&workspace.session.zola_root);
    if zola_root != project_root.join("sursa") {
        return Err(format!(
            "ProjectWorkspace a refuzat Save: Zola root {} nu corespunde proiectului {}.",
            zola_root.display(),
            project_root.display()
        ));
    }

    let mut documents = workspace.documents.clone();
    let mut deleted_documents = workspace
        .deleted_document_paths()
        .into_iter()
        .collect::<BTreeSet<_>>();
    let user_changed_paths = workspace
        .documents
        .files
        .iter()
        .filter(|(path, entry)| {
            entry.is_dirty() || !workspace.accepted_documents.contains_key(*path)
        })
        .map(|(path, _)| path.clone())
        .collect::<HashSet<_>>();
    let mut derived_generated_paths = HashSet::new();
    let mut accepted_page_js = workspace.accepted_page_js.clone();
    let mut requires_anime = false;
    let now_ms = crate::kernel::observability::now_ms();

    for draft in workspace.page_js.drafts.values() {
        let template_project_path = project_relative_zola_path(&draft.template_path);
        if deleted_documents.contains(&template_project_path)
            || !documents.files.contains_key(&template_project_path)
        {
            return Err(format!(
                "Save Page JS a fost blocat pentru {}: template-ul nu există în proiecția curentă ProjectWorkspace.",
                draft.template_path
            ));
        }
        let plan = plan_page_js_save_for_project(
            zola_root,
            &workspace.session,
            &documents,
            &draft.template_path,
            draft.current.clone(),
            draft.cachebust_assets,
        )?;
        if plan.page_js_resource.blocked {
            return Err(plan.page_js_resource.message.clone());
        }
        requires_anime |= plan.ensure_anime_asset;

        for change in page_js_text_changes_from_plan(&plan) {
            if deleted_documents.contains(&change.relative_path) {
                return Err(format!(
                    "Save Page JS a detectat un conflict: {} este șters în sesiune, dar materializarea JS încearcă să îl scrie.",
                    change.relative_path
                ));
            }
            if change.relative_path == plan.page_js_resource.project_path
                && user_changed_paths.contains(&change.relative_path)
                && !derived_generated_paths.contains(&change.relative_path)
                && documents.text_for(&change.relative_path).as_deref()
                    != Some(change.new_text.as_str())
            {
                return Err(format!(
                    "Save Page JS a detectat un conflict cu editarea directă a fișierului generat {}. Alege configurația Page JS sau codul manual înainte de Save.",
                    change.relative_path
                ));
            }
            stage_materialized_text(
                &mut documents,
                &change.relative_path,
                change.new_text,
                now_ms,
            )?;
            if change.relative_path == plan.page_js_resource.project_path {
                derived_generated_paths.insert(change.relative_path);
            }
        }
        for delete in page_js_text_deletes_from_plan(&plan) {
            if user_changed_paths.contains(&delete.relative_path)
                && !derived_generated_paths.contains(&delete.relative_path)
            {
                return Err(format!(
                    "Save Page JS a detectat un conflict: fișierul generat {} are editări directe și nu poate fi șters implicit.",
                    delete.relative_path
                ));
            }
            documents.files.remove(&delete.relative_path);
            deleted_documents.insert(delete.relative_path);
        }
        accepted_page_js.insert(draft.template_path.clone(), draft.current.clone());
    }

    if requires_anime {
        materialize_anime_runtime(zola_root, &mut documents, &deleted_documents, now_ms)?;
    }

    Ok(MaterializedWorkspaceSave {
        documents,
        deleted_documents,
        accepted_page_js,
        binary_resources: workspace.binary_resources.clone(),
        deleted_binary_resources: workspace.deleted_binary_resources.clone(),
    })
}

fn materialize_anime_runtime(
    zola_root: &Path,
    documents: &mut FileBufferStore,
    deleted_documents: &BTreeSet<String>,
    now_ms: u128,
) -> Result<(), String> {
    let definition = generated_asset_definition(GeneratedAssetId::AnimeJsRuntime);
    if deleted_documents.contains(definition.project_relative_path) {
        return Err(format!(
            "Save Page JS a fost blocat: {} este necesar, dar este șters în sesiunea curentă.",
            definition.project_relative_path
        ));
    }
    if let Some(current) = documents.text_for(definition.project_relative_path) {
        if current.as_bytes() != definition.bytes {
            return Err(format!(
                "Save Page JS nu poate suprascrie {}: conținutul curent diferă de runtime-ul administrat de nucleu.",
                definition.project_relative_path
            ));
        }
        return Ok(());
    }

    let plan = plan_generated_asset_intent(
        zola_root,
        GeneratedAssetId::AnimeJsRuntime,
        GeneratedAssetAction::EnsurePresent,
    );
    match plan.status {
        GeneratedAssetPlanStatus::Blocked => Err(format!(
            "Save Page JS a blocat runtime-ul Anime: {}",
            plan.diagnostics.join(" ")
        )),
        GeneratedAssetPlanStatus::Noop => Ok(()),
        GeneratedAssetPlanStatus::Ready => {
            let contents = std::str::from_utf8(definition.bytes).map_err(|_| {
                "Registry-ul Anime conține bytes non-UTF-8 și nu poate intra în tranzacția text ProjectWorkspace."
                    .to_string()
            })?;
            stage_materialized_text(
                documents,
                definition.project_relative_path,
                contents.to_string(),
                now_ms,
            )
        }
    }
}

fn stage_materialized_text(
    documents: &mut FileBufferStore,
    relative_path: &str,
    contents: String,
    now_ms: u128,
) -> Result<(), String> {
    if let Some(current) = documents.text_snapshot(relative_path) {
        if current.text == contents {
            return Ok(());
        }
        documents.set_draft_if_current(
            relative_path,
            contents,
            &FileBufferMutationExpectation {
                expected_revision: current.revision,
                expected_hash: current.hash,
            },
            now_ms,
        )?;
    } else {
        documents.stage_new_text_file(relative_path, contents, now_ms)?;
    }
    Ok(())
}

fn project_relative_zola_path(path: &str) -> String {
    let normalized = path.trim().trim_start_matches('/');
    if normalized.starts_with("sursa/") {
        normalized.to_string()
    } else {
        format!("sursa/{normalized}")
    }
}

fn workspace_document_writes(
    workspace: &ProjectWorkspace,
    documents: &FileBufferStore,
) -> Vec<(String, String)> {
    documents
        .files
        .iter()
        .filter(|(path, entry)| {
            workspace
                .accepted_documents
                .get(*path)
                .map(|accepted| accepted.current_text() != entry.current_text())
                .unwrap_or(true)
        })
        .map(|(path, entry)| (path.clone(), entry.current_text().to_string()))
        .collect()
}

fn workspace_binary_writes(
    project_root: &Path,
    workspace: &ProjectWorkspace,
    materialized: &MaterializedWorkspaceSave,
) -> Result<Vec<(String, Vec<u8>)>, String> {
    let accepted_paths = workspace
        .accepted_disk
        .manifest
        .files
        .iter()
        .map(|entry| entry.relative_path.as_str())
        .collect::<HashSet<_>>();
    let mut writes = Vec::new();
    for (path, resource) in &materialized.binary_resources {
        if accepted_paths.contains(path.as_str()) {
            let disk_path = resolve_project_write_path(project_root, path)?;
            let disk = read_regular_binary_file(&disk_path, path)?.ok_or_else(|| {
                format!("ProjectWorkspace nu mai găsește resursa binară acceptată {path}.")
            })?;
            if hash_bytes(&disk) == hash_bytes(&resource.bytes) {
                continue;
            }
        }
        writes.push((path.clone(), resource.bytes.clone()));
    }
    Ok(writes)
}

fn accepted_transaction_store(workspace: &ProjectWorkspace) -> FileBufferStore {
    let mut store = workspace.documents.clone();
    store.files = workspace.accepted_documents.clone();
    store
}

#[allow(clippy::too_many_arguments)]
fn fail_save_transaction<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &Path,
    workspace: &ProjectWorkspace,
    transaction_id: &str,
    touched_files: &[String],
    transaction_store: &mut FileBufferStore,
    applied: &[AppliedSaveOperation],
    committed_writes: Vec<WriteReceipt>,
    error: ProjectWorkspaceDiskError,
) -> Result<ProjectWorkspaceSaveReceipt, ProjectWorkspaceSaveError> {
    if error.recovery().is_some() {
        return Err(ProjectWorkspaceSaveError::recovery_required(
            transaction_id,
            touched_files.to_vec(),
            committed_writes,
            format!(
                "Save a intrat în recovery la un efect de filesystem: {}",
                error.diagnostic()
            ),
        ));
    }
    rollback_after_post_commit_failure(
        app,
        project_root,
        workspace,
        transaction_id,
        touched_files,
        transaction_store,
        applied,
        committed_writes,
        error.diagnostic().to_string(),
    )
}

#[allow(clippy::too_many_arguments)]
fn rollback_after_post_commit_failure<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &Path,
    workspace: &ProjectWorkspace,
    transaction_id: &str,
    touched_files: &[String],
    transaction_store: &mut FileBufferStore,
    applied: &[AppliedSaveOperation],
    committed_writes: Vec<WriteReceipt>,
    diagnostic: String,
) -> Result<ProjectWorkspaceSaveReceipt, ProjectWorkspaceSaveError> {
    if let Err(rollback_error) =
        rollback_applied_operations(app, project_root, transaction_store, applied)
    {
        return Err(ProjectWorkspaceSaveError::recovery_required(
            transaction_id,
            touched_files.to_vec(),
            committed_writes,
            format!("{diagnostic} Rollback-ul tranzacției a eșuat: {rollback_error}"),
        ));
    }
    if let Err(clear_error) =
        clear_project_workspace_save_journal(app, &workspace.session, transaction_id)
    {
        return Err(ProjectWorkspaceSaveError::recovery_required(
            transaction_id,
            touched_files.to_vec(),
            committed_writes,
            format!(
                "{diagnostic} Efectele proiectului au fost restaurate, dar jurnalul Save nu a putut fi eliminat: {clear_error}"
            ),
        ));
    }
    Err(ProjectWorkspaceSaveError::rejected(format!(
        "{diagnostic} Niciun efect al tranzacției Save nu a rămas pe disc; rollback complet."
    )))
}

fn rollback_applied_operations<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &Path,
    store: &mut FileBufferStore,
    applied: &[AppliedSaveOperation],
) -> Result<(), String> {
    for operation in applied.iter().rev() {
        match operation {
            AppliedSaveOperation::Write(change) if change.existed_before => {
                save_text_file(
                    app,
                    project_root,
                    store,
                    &change.relative_path,
                    change.before_text.clone(),
                    None,
                )
                .map_err(|error| error.to_string())?;
            }
            AppliedSaveOperation::Write(change) => {
                remove_created_text_file_for_undo(
                    app,
                    project_root,
                    store,
                    &change.relative_path,
                    &hash_text(&change.new_text),
                )
                .map_err(|error| error.to_string())?;
            }
            AppliedSaveOperation::Delete(delete) => {
                save_text_file(
                    app,
                    project_root,
                    store,
                    &delete.relative_path,
                    delete.before_text.clone(),
                    None,
                )
                .map_err(|error| error.to_string())?;
            }
            AppliedSaveOperation::BinaryWrite(change) if change.existed_before => {
                save_binary_file(
                    app,
                    project_root,
                    &store.runtime_session_id,
                    &change.relative_path,
                    &change.before_bytes,
                    true,
                    &change.new_hash,
                )
                .map_err(|error| error.to_string())?;
            }
            AppliedSaveOperation::BinaryWrite(change) => {
                delete_binary_file(
                    app,
                    project_root,
                    &store.runtime_session_id,
                    &change.relative_path,
                    &change.new_hash,
                )
                .map_err(|error| error.to_string())?;
            }
            AppliedSaveOperation::BinaryDelete(delete) => {
                save_binary_file(
                    app,
                    project_root,
                    &store.runtime_session_id,
                    &delete.relative_path,
                    &delete.before_bytes,
                    false,
                    &hash_bytes(&[]),
                )
                .map_err(|error| error.to_string())?;
            }
        }
    }
    Ok(())
}

fn plan_project_workspace_save(
    project_root: &Path,
    store: &FileBufferStore,
    writes: Vec<(String, String)>,
    deletes: Vec<String>,
    binary_writes: Vec<(String, Vec<u8>)>,
    binary_deletes: Vec<String>,
    revision: u64,
) -> Result<ProjectWorkspaceSavePlan, String> {
    if store.project_root != project_root.to_string_lossy() {
        return Err(format!(
            "ProjectWorkspace Save a primit FileBufferStore pentru {}, nu pentru {}.",
            store.project_root,
            project_root.display()
        ));
    }
    if writes.is_empty()
        && deletes.is_empty()
        && binary_writes.is_empty()
        && binary_deletes.is_empty()
    {
        return Err("ProjectWorkspace Save nu are operații de persistat.".to_string());
    }

    let mut seen = BTreeSet::new();
    let mut changes = Vec::with_capacity(writes.len());
    for (relative_path, new_text) in writes {
        let normalized = normalize_project_relative_path(&relative_path)?;
        if normalized != relative_path {
            return Err(format!(
                "ProjectWorkspace Save a refuzat path-ul necanonic {relative_path}."
            ));
        }
        if !seen.insert(normalized.clone()) {
            return Err(format!(
                "ProjectWorkspace Save a primit path duplicat: {normalized}."
            ));
        }
        if new_text.len() as u64 > store.limits.max_file_bytes {
            return Err(format!(
                "ProjectWorkspace Save a refuzat {normalized}: {} bytes depășesc limita de {}.",
                new_text.len(),
                store.limits.max_file_bytes
            ));
        }
        let disk_path = resolve_project_write_path(project_root, &normalized)?;
        let (existed_before, before_text, before_hash) = match store.files.get(&normalized) {
            Some(entry) => {
                if entry.draft.is_some() {
                    return Err(format!(
                        "ProjectWorkspace Save a primit un baseline tranzacțional dirty pentru {normalized}."
                    ));
                }
                let disk =
                    read_disk_text_baseline(&disk_path, &store.limits)?.ok_or_else(|| {
                        format!(
                        "ProjectWorkspace Save a fost blocat: {normalized} lipsește de pe disc."
                    )
                    })?;
                if disk.baseline.readonly || disk.baseline.hash != entry.baseline.hash {
                    return Err(format!(
                        "ProjectWorkspace Save a fost blocat pentru {normalized}: disk-ul nu mai corespunde baseline-ului acceptat."
                    ));
                }
                (
                    true,
                    entry.baseline_text.clone(),
                    entry.baseline.hash.clone(),
                )
            }
            None => match disk_path.try_exists() {
                Ok(false) => (false, String::new(), hash_text("")),
                Ok(true) => {
                    return Err(format!(
                        "ProjectWorkspace Save a fost blocat pentru {normalized}: target-ul există extern fără baseline în sesiune."
                    ));
                }
                Err(error) => {
                    return Err(format!(
                        "ProjectWorkspace Save nu a putut verifica target-ul {normalized}: {error}"
                    ));
                }
            },
        };
        changes.push(ProjectWorkspacePlannedWrite {
            relative_path: normalized,
            existed_before,
            before_text,
            new_hash: hash_text(&new_text),
            new_text,
            before_hash,
        });
    }

    let mut planned_deletes = Vec::with_capacity(deletes.len());
    for relative_path in deletes {
        let normalized = normalize_project_relative_path(&relative_path)?;
        if normalized != relative_path || !seen.insert(normalized.clone()) {
            return Err(format!(
                "ProjectWorkspace Save a primit path necanonic sau duplicat: {relative_path}."
            ));
        }
        let entry = store.files.get(&normalized).ok_or_else(|| {
            format!("ProjectWorkspace Save nu are baseline pentru ștergerea {normalized}.")
        })?;
        if entry.draft.is_some() {
            return Err(format!(
                "ProjectWorkspace Save a primit un baseline tranzacțional dirty pentru ștergerea {normalized}."
            ));
        }
        let disk_path = resolve_project_write_path(project_root, &normalized)?;
        let disk = read_disk_text_baseline(&disk_path, &store.limits)?.ok_or_else(|| {
            format!("ProjectWorkspace Save nu găsește {normalized} pe disc pentru ștergere.")
        })?;
        if disk.baseline.readonly || disk.baseline.hash != entry.baseline.hash {
            return Err(format!(
                "ProjectWorkspace Save a blocat ștergerea {normalized}: disk-ul nu mai corespunde baseline-ului acceptat."
            ));
        }
        planned_deletes.push(ProjectWorkspacePlannedDelete {
            relative_path: normalized,
            before_text: entry.baseline_text.clone(),
            before_hash: entry.baseline.hash.clone(),
        });
    }

    let mut planned_binary_changes = Vec::with_capacity(binary_writes.len());
    for (relative_path, new_bytes) in binary_writes {
        let normalized = normalize_project_relative_path(&relative_path)?;
        if normalized != relative_path || !seen.insert(normalized.clone()) {
            return Err(format!(
                "ProjectWorkspace Save a primit path binar necanonic sau duplicat: {relative_path}."
            ));
        }
        if new_bytes.len() as u64 > PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_BYTES {
            return Err(format!(
                "ProjectWorkspace Save a refuzat {normalized}: resursa binară depășește limita de {PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_BYTES} bytes."
            ));
        }
        let disk_path = resolve_project_write_path(project_root, &normalized)?;
        let (existed_before, before_bytes) =
            match read_regular_binary_file(&disk_path, &normalized)? {
                Some(bytes) => (true, bytes),
                None => (false, Vec::new()),
            };
        planned_binary_changes.push(ProjectWorkspacePlannedBinaryWrite {
            relative_path: normalized,
            existed_before,
            before_hash: hash_bytes(&before_bytes),
            new_hash: hash_bytes(&new_bytes),
            before_bytes,
            new_bytes,
        });
    }

    let mut planned_binary_deletes = Vec::with_capacity(binary_deletes.len());
    for relative_path in binary_deletes {
        let normalized = normalize_project_relative_path(&relative_path)?;
        if normalized != relative_path || !seen.insert(normalized.clone()) {
            return Err(format!(
                "ProjectWorkspace Save a primit delete binar necanonic sau duplicat: {relative_path}."
            ));
        }
        let disk_path = resolve_project_write_path(project_root, &normalized)?;
        let before_bytes = read_regular_binary_file(&disk_path, &normalized)?.ok_or_else(|| {
            format!("ProjectWorkspace Save nu găsește resursa binară {normalized} pentru delete.")
        })?;
        planned_binary_deletes.push(ProjectWorkspacePlannedBinaryDelete {
            relative_path: normalized,
            before_hash: hash_bytes(&before_bytes),
            before_bytes,
        });
    }

    changes.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    planned_deletes.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    planned_binary_changes.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    planned_binary_deletes.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let touched_files = changes
        .iter()
        .map(|change| change.relative_path.clone())
        .chain(
            planned_deletes
                .iter()
                .map(|delete| delete.relative_path.clone()),
        )
        .chain(
            planned_binary_changes
                .iter()
                .map(|change| change.relative_path.clone()),
        )
        .chain(
            planned_binary_deletes
                .iter()
                .map(|delete| delete.relative_path.clone()),
        )
        .collect::<Vec<_>>();
    let sequence = PROJECT_WORKSPACE_SAVE_COUNTER.fetch_add(1, Ordering::Relaxed);
    Ok(ProjectWorkspaceSavePlan {
        id: format!(
            "project-workspace-save-{}-{}-{}",
            revision,
            std::process::id(),
            sequence
        ),
        changes,
        deletes: planned_deletes,
        binary_changes: planned_binary_changes,
        binary_deletes: planned_binary_deletes,
        touched_files,
    })
}

fn read_regular_binary_file(path: &Path, relative_path: &str) -> Result<Option<Vec<u8>>, String> {
    let metadata_before = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "ProjectWorkspace Save nu a putut inspecta {relative_path}: {error}"
            ))
        }
    };
    if metadata_before.file_type().is_symlink() || !metadata_before.is_file() {
        return Err(format!(
            "ProjectWorkspace Save a refuzat resursa binară non-regular {relative_path}."
        ));
    }
    if metadata_before.len() > PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_BYTES {
        return Err(format!(
            "ProjectWorkspace Save a refuzat {relative_path}: baseline-ul binar depășește {PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_BYTES} bytes."
        ));
    }
    let bytes = std::fs::read(path).map_err(|error| {
        format!("ProjectWorkspace Save nu a putut citi {relative_path}: {error}")
    })?;
    let metadata_after = std::fs::metadata(path).map_err(|error| {
        format!("ProjectWorkspace Save nu a putut reverifica {relative_path}: {error}")
    })?;
    if crate::project::project_disk_metadata_version_token(&metadata_before)
        != crate::project::project_disk_metadata_version_token(&metadata_after)
    {
        return Err(format!(
            "ProjectWorkspace Save a detectat o schimbare concurentă în {relative_path}."
        ));
    }
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    use crate::{
        app_home::{ensure_app_home, TEST_APP_ENV_LOCK},
        js::PageJsDraftStore,
        kernel::{
            file_buffer_store::{
                FileBufferBaseline, FileBufferEntry, FileBufferStoreLimits, TextBufferLanguage,
                TextBufferRole,
            },
            project_session::{
                ProjectRootFingerprint, ProjectSessionScanSummary, ProjectSessionSnapshot,
            },
            project_workspace::{WorkspaceBinaryResource, WorkspaceMutationMetadata},
            write_authority::test_support::install_test_project_authority,
        },
        project::AcceptedProjectDiskManifest,
    };

    use super::*;

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn binary_create_and_delete_are_planned_with_reversible_bytes() {
        let root = unique_test_dir();
        std::fs::create_dir_all(root.join("sursa/static")).unwrap();
        let store = FileBufferStore::new(
            "save-test-session",
            root.to_string_lossy(),
            1,
            FileBufferStoreLimits {
                max_files: 100,
                max_file_bytes: 1024 * 1024,
                max_total_bytes: 4 * 1024 * 1024,
            },
        );
        let created_path = "sursa/static/new.woff2";
        let created_bytes = vec![0x77, 0x4f, 0x46, 0x32, 9];
        let deleted_path = "sursa/static/old.woff2";
        let deleted_bytes = vec![0x77, 0x4f, 0x46, 0x32, 1];
        std::fs::write(root.join(deleted_path), &deleted_bytes).unwrap();

        let plan = plan_project_workspace_save(
            &root,
            &store,
            Vec::new(),
            Vec::new(),
            vec![(created_path.to_string(), created_bytes.clone())],
            vec![deleted_path.to_string()],
            7,
        )
        .unwrap();

        assert_eq!(plan.touched_files, vec![created_path, deleted_path]);
        assert_eq!(plan.binary_changes.len(), 1);
        assert!(!plan.binary_changes[0].existed_before);
        assert!(plan.binary_changes[0].before_bytes.is_empty());
        assert_eq!(plan.binary_changes[0].new_bytes, created_bytes);
        assert_eq!(plan.binary_deletes.len(), 1);
        assert_eq!(plan.binary_deletes[0].before_bytes, deleted_bytes);
        assert_eq!(
            plan.binary_deletes[0].before_hash,
            hash_bytes(&plan.binary_deletes[0].before_bytes)
        );
        assert!(!root.join(created_path).exists());
        assert!(root.join(deleted_path).exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn save_plan_rejects_a_path_shared_by_text_and_binary_operations() {
        let root = unique_test_dir();
        std::fs::create_dir_all(root.join("sursa/static")).unwrap();
        let store = FileBufferStore::new(
            "save-test-session",
            root.to_string_lossy(),
            1,
            FileBufferStoreLimits {
                max_files: 100,
                max_file_bytes: 1024 * 1024,
                max_total_bytes: 4 * 1024 * 1024,
            },
        );
        let error = plan_project_workspace_save(
            &root,
            &store,
            vec![("sursa/static/collision.dat".to_string(), "text".to_string())],
            Vec::new(),
            vec![("sursa/static/collision.dat".to_string(), vec![1, 2, 3])],
            Vec::new(),
            8,
        )
        .unwrap_err();
        assert!(error.contains("duplicat"));
        assert!(!root.join("sursa/static/collision.dat").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn save_is_the_only_binary_disk_boundary_across_undo_redo() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir();
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let project = root.join("project");
        std::fs::create_dir_all(project.join("sursa/static/fonturi/inter")).unwrap();
        std::fs::write(project.join("sursa/zola.toml"), "base_url = '/'\n").unwrap();
        let project = project.canonicalize().unwrap();

        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock app");
        let app_home = ensure_app_home(app.handle()).unwrap();
        let session_dir = PathBuf::from(&app_home.sessions_dir).join("binary-save-session");
        std::fs::create_dir_all(&session_dir).unwrap();
        let session = test_session(&project, &session_dir);
        install_test_project_authority(
            app.handle(),
            &session.runtime_instance_id(),
            &project,
            &session_dir,
        )
        .unwrap();

        let manifest = read_project_disk_manifest(&project).unwrap();
        let accepted = AcceptedProjectDiskManifest::new(
            session.runtime_instance_id(),
            session.project_root.clone(),
            manifest,
        )
        .unwrap();
        let documents = FileBufferStore::for_project_session(
            &session,
            1,
            FileBufferStoreLimits {
                max_files: 100,
                max_file_bytes: 1024 * 1024,
                max_total_bytes: 4 * 1024 * 1024,
            },
        );
        let page_js = PageJsDraftStore::new(&session);
        let mut workspace = ProjectWorkspace::new(session, accepted, documents, page_js).unwrap();
        let relative_path = "sursa/static/fonturi/inter/inter-regular.woff2";
        let disk_path = project.join(relative_path);
        let bytes = vec![0x77, 0x4f, 0x46, 0x32, 11, 12, 13];
        workspace
            .stage_binary_resource_creates(
                &workspace_identity(&workspace),
                WorkspaceMutationMetadata {
                    label: "Download Inter".to_string(),
                    source: "test.binary_save".to_string(),
                    coalesce_key: None,
                    transaction_id: None,
                },
                vec![WorkspaceBinaryResource::new(relative_path, bytes.clone())],
                10,
            )
            .unwrap();
        assert!(!disk_path.exists());

        let identity = workspace_identity(&workspace);
        let first_save =
            save_project_workspace(app.handle(), &project, &mut workspace, &identity).unwrap();
        assert!(matches!(
            first_save.status,
            ProjectWorkspaceSaveStatus::Saved
        ));
        assert_eq!(first_save.written_files, vec![relative_path]);
        assert_eq!(std::fs::read(&disk_path).unwrap(), bytes);
        assert!(!workspace.is_dirty());

        workspace.undo(&workspace_identity(&workspace), 11).unwrap();
        assert_eq!(std::fs::read(&disk_path).unwrap(), bytes);
        assert_eq!(
            workspace.deleted_binary_resources().collect::<Vec<_>>(),
            vec![relative_path]
        );

        // Redo reaches the exact accepted bytes, so it must normalize to a
        // clean workspace instead of staging a redundant disk rewrite.
        workspace.redo(&workspace_identity(&workspace), 12).unwrap();
        assert_eq!(std::fs::read(&disk_path).unwrap(), bytes);
        assert!(workspace.staged_binary_resource(relative_path).is_none());
        assert!(workspace.deleted_binary_resources().next().is_none());
        assert!(!workspace.is_dirty());
        let clean_revision = workspace.revision;
        let identity = workspace_identity(&workspace);
        let clean_save =
            save_project_workspace(app.handle(), &project, &mut workspace, &identity).unwrap();
        assert!(matches!(
            clean_save.status,
            ProjectWorkspaceSaveStatus::Noop
        ));
        assert_eq!(workspace.revision, clean_revision);

        workspace.undo(&workspace_identity(&workspace), 13).unwrap();
        let identity = workspace_identity(&workspace);
        let delete_save =
            save_project_workspace(app.handle(), &project, &mut workspace, &identity).unwrap();
        assert_eq!(delete_save.removed_files, vec![relative_path]);
        assert!(!disk_path.exists());
        assert!(!workspace.is_dirty());

        workspace.redo(&workspace_identity(&workspace), 14).unwrap();
        assert!(!disk_path.exists());
        assert_eq!(
            workspace.staged_binary_resource(relative_path),
            Some(bytes.as_slice())
        );
        let identity = workspace_identity(&workspace);
        save_project_workspace(app.handle(), &project, &mut workspace, &identity).unwrap();
        assert_eq!(std::fs::read(&disk_path).unwrap(), bytes);
        assert!(!workspace.is_dirty());

        drop(app);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mixed_text_binary_save_rolls_back_both_on_concurrent_external_change() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir();
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let project = root.join("project");
        std::fs::create_dir_all(project.join("sursa/templates")).unwrap();
        std::fs::create_dir_all(project.join("sursa/static/fonturi/inter")).unwrap();
        std::fs::write(project.join("sursa/zola.toml"), "base_url = '/'\n").unwrap();
        let template_relative_path = "sursa/templates/index.html";
        let template_path = project.join(template_relative_path);
        let baseline_text = "<main>Baseline</main>";
        let draft_text = "<main>Draft</main>";
        std::fs::write(&template_path, baseline_text).unwrap();
        let project = project.canonicalize().unwrap();

        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock app");
        let app_home = ensure_app_home(app.handle()).unwrap();
        let session_dir = PathBuf::from(&app_home.sessions_dir).join("mixed-save-session");
        std::fs::create_dir_all(&session_dir).unwrap();
        let session = test_session(&project, &session_dir);
        install_test_project_authority(
            app.handle(),
            &session.runtime_instance_id(),
            &project,
            &session_dir,
        )
        .unwrap();

        let accepted = AcceptedProjectDiskManifest::new(
            session.runtime_instance_id(),
            session.project_root.clone(),
            read_project_disk_manifest(&project).unwrap(),
        )
        .unwrap();
        let mut documents = FileBufferStore::for_project_session(
            &session,
            1,
            FileBufferStoreLimits {
                max_files: 100,
                max_file_bytes: 1024 * 1024,
                max_total_bytes: 4 * 1024 * 1024,
            },
        );
        documents.insert_loaded_file(FileBufferEntry {
            relative_path: template_relative_path.to_string(),
            absolute_path: template_path.to_string_lossy().into_owned(),
            language: TextBufferLanguage::Html,
            role: TextBufferRole::Template,
            baseline: FileBufferBaseline {
                hash: hash_text(baseline_text),
                modified_ms: 0,
                size: baseline_text.len() as u64,
                readonly: false,
            },
            baseline_text: baseline_text.to_string(),
            draft: None,
            revision: 1,
        });
        let page_js = PageJsDraftStore::new(&session);
        let mut workspace = ProjectWorkspace::new(session, accepted, documents, page_js).unwrap();
        workspace
            .stage_document_texts(
                &workspace_identity(&workspace),
                WorkspaceMutationMetadata {
                    label: "Edit template".to_string(),
                    source: "test.mixed_save".to_string(),
                    coalesce_key: None,
                    transaction_id: None,
                },
                vec![super::super::model::WorkspaceDocumentMutation {
                    relative_path: template_relative_path.to_string(),
                    contents: draft_text.to_string(),
                }],
                20,
            )
            .unwrap();
        let binary_relative_path = "sursa/static/fonturi/inter/inter-regular.woff2";
        let intended_bytes = vec![0x77, 0x4f, 0x46, 0x32, 41];
        let concurrent_bytes = b"external concurrent edit".to_vec();
        workspace
            .stage_binary_resource_creates(
                &workspace_identity(&workspace),
                WorkspaceMutationMetadata {
                    label: "Download Inter".to_string(),
                    source: "test.mixed_save".to_string(),
                    coalesce_key: None,
                    transaction_id: None,
                },
                vec![WorkspaceBinaryResource::new(
                    binary_relative_path,
                    intended_bytes.clone(),
                )],
                21,
            )
            .unwrap();
        let revision_before_save = workspace.revision;
        let identity = workspace_identity(&workspace);
        let concurrent_relative_path = "sursa/static/external-concurrent.txt";
        let concurrent_path = project.join(concurrent_relative_path);
        let concurrent_bytes_for_hook = concurrent_bytes.clone();
        let result = super::super::disk_boundary::with_after_text_write_before_file_buffer_projection_hook_for_test(
            move |_store, relative_path| {
                if relative_path == template_relative_path {
                    std::fs::write(&concurrent_path, &concurrent_bytes_for_hook).unwrap();
                }
            },
            || save_project_workspace(app.handle(), &project, &mut workspace, &identity),
        );

        let error = result.unwrap_err();
        assert!(matches!(error, ProjectWorkspaceSaveError::Rejected { .. }));
        assert!(error.to_string().contains("schimbări externe concurente"));
        assert!(error.to_string().contains("rollback complet"));
        assert_eq!(
            std::fs::read_to_string(&template_path).unwrap(),
            baseline_text
        );
        assert_eq!(
            std::fs::read(project.join(concurrent_relative_path)).unwrap(),
            concurrent_bytes
        );
        assert!(!project.join(binary_relative_path).exists());
        assert_eq!(workspace.revision, revision_before_save);
        assert_eq!(
            workspace.documents.text_for(template_relative_path),
            Some(draft_text.to_string())
        );
        assert_eq!(
            workspace.staged_binary_resource(binary_relative_path),
            Some(intended_bytes.as_slice())
        );
        assert!(workspace.is_dirty());

        drop(app);
        std::fs::remove_dir_all(root).unwrap();
    }

    fn unique_test_dir() -> std::path::PathBuf {
        let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "pana-project-workspace-binary-save-{}-{sequence}",
            std::process::id()
        ))
    }

    fn workspace_identity(workspace: &ProjectWorkspace) -> ProjectWorkspaceIdentity {
        ProjectWorkspaceIdentity {
            expected_project_root: workspace.session.project_root.clone(),
            expected_session_id: workspace.runtime_session_id(),
            expected_revision: workspace.revision,
        }
    }

    fn test_session(project: &Path, session_dir: &Path) -> ProjectSessionSnapshot {
        ProjectSessionSnapshot {
            schema_version: 1,
            id: "binary-save-session".to_string(),
            project_root: project.to_string_lossy().into_owned(),
            zola_root: project.join("sursa").to_string_lossy().into_owned(),
            session_dir: session_dir.to_string_lossy().into_owned(),
            manifest_path: session_dir
                .join("manifest.json")
                .to_string_lossy()
                .into_owned(),
            opened_at_ms: 77,
            last_seen_at_ms: 77,
            root_fingerprint: ProjectRootFingerprint {
                canonical_path: project.to_string_lossy().into_owned(),
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
                file_count: 1,
                directory_count: 4,
            },
        }
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
