use std::{
    fs,
    path::{Path, PathBuf},
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};

use crate::{
    app_home::{project_session_dir, project_workspace_save_journal_dir},
    kernel::{
        file_buffer_store::hash_bytes,
        observability::now_ms,
        project_session::ProjectSessionSnapshot,
        write_authority::{
            WriteAuthority, WriteCategory, WriteIntent, WriteOperationKind, WriteOwner,
            WritePolicy, WriteReceipt, WriteTarget,
        },
    },
    project::{project_disk_metadata_version_token, resolve_project_write_path},
};

use super::save::{ProjectWorkspaceSavePlan, ProjectWorkspaceSavePlannedFile};

pub const PROJECT_WORKSPACE_SAVE_JOURNAL_SCHEMA_VERSION: u32 = 2;
const PROJECT_WORKSPACE_SAVE_JOURNAL_MAX_BYTES: u64 = 192 * 1024 * 1024;
const PROJECT_WORKSPACE_SAVE_JOURNAL_MAX_COUNT: usize = 128;
const PROJECT_WORKSPACE_SAVE_JOURNAL_SCAN_MAX_TOTAL_BYTES: u64 = 384 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectWorkspaceSaveHotJournalDiskState {
    BeforeState,
    PlannedState,
    MixedState,
    ConflictState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectWorkspaceSaveHotJournalFileDiskState {
    Before,
    Planned,
    Conflict,
    Unreadable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectWorkspaceSaveJournalContentKind {
    Text,
    Binary,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectWorkspaceSaveRecoveryAction {
    ClearStaleJournal,
    RollbackToBefore,
    ManualReviewMixedState,
    ManualReviewConflict,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkspaceSaveRecoveryPlan {
    pub action: ProjectWorkspaceSaveRecoveryAction,
    pub can_clear_journal: bool,
    pub can_rollback: bool,
    pub summary: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkspaceSaveHotJournalFile {
    pub relative_path: String,
    pub content_kind: ProjectWorkspaceSaveJournalContentKind,
    pub existed_before: bool,
    pub exists_after: bool,
    pub before_hash: String,
    pub planned_hash: Option<String>,
    pub disk_hash: Option<String>,
    pub disk_state: ProjectWorkspaceSaveHotJournalFileDiskState,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkspaceSaveHotJournal {
    pub schema_version: u32,
    pub transaction_id: String,
    pub path: String,
    pub runtime_session_id: String,
    pub project_root: String,
    pub revision: u64,
    pub prepared_at_ms: u128,
    pub touched_files: Vec<String>,
    pub file_count: usize,
    pub bytes_before: u64,
    pub disk_state: ProjectWorkspaceSaveHotJournalDiskState,
    pub recovery_plan: ProjectWorkspaceSaveRecoveryPlan,
    pub files: Vec<ProjectWorkspaceSaveHotJournalFile>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkspaceSaveRecoveryReceipt {
    pub schema_version: u32,
    pub transaction_id: String,
    pub action: ProjectWorkspaceSaveRecoveryAction,
    pub project_root: String,
    pub restored_files: Vec<String>,
    pub already_before_files: Vec<String>,
    pub journal_cleared: bool,
    pub write_receipts: Vec<WriteReceipt>,
    pub operator_diagnostic: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProjectWorkspaceSaveJournal {
    pub(super) schema_version: u32,
    pub(super) transaction_id: String,
    pub(super) runtime_session_id: String,
    pub(super) project_root: String,
    pub(super) revision: u64,
    pub(super) prepared_at_ms: u128,
    pub(super) touched_files: Vec<String>,
    pub(super) files: Vec<ProjectWorkspaceSaveJournalFile>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProjectWorkspaceSaveJournalFile {
    pub(super) relative_path: String,
    pub(super) content_kind: ProjectWorkspaceSaveJournalContentKind,
    pub(super) existed_before: bool,
    pub(super) exists_after: bool,
    pub(super) before_hash: String,
    pub(super) planned_hash: Option<String>,
    pub(super) before_contents_base64: String,
}

pub(super) fn prepare_project_workspace_save_journal<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    revision: u64,
    plan: &ProjectWorkspaceSavePlan,
) -> Result<(), String> {
    let journal = ProjectWorkspaceSaveJournal {
        schema_version: PROJECT_WORKSPACE_SAVE_JOURNAL_SCHEMA_VERSION,
        transaction_id: plan.id.clone(),
        runtime_session_id: session.runtime_instance_id(),
        project_root: session.project_root.clone(),
        revision,
        prepared_at_ms: now_ms(),
        touched_files: plan.touched_files.clone(),
        files: plan.files().map(journal_file_from_plan).collect::<Vec<_>>(),
    };
    let source = serde_json::to_string_pretty(&journal).map_err(|error| {
        format!("Jurnalul ProjectWorkspace Save nu poate fi serializat: {error}")
    })?;
    if source.len() as u64 > PROJECT_WORKSPACE_SAVE_JOURNAL_MAX_BYTES {
        return Err(format!(
            "Jurnalul ProjectWorkspace Save are {} bytes, peste limita de {}.",
            source.len(),
            PROJECT_WORKSPACE_SAVE_JOURNAL_MAX_BYTES
        ));
    }

    let path = save_journal_path(app, session, &plan.id)?;
    let boundary = project_session_dir(app, &session.project_root)?;
    let intent = WriteIntent::new(
        WriteCategory::InternalAppWrite,
        WriteOwner::ProjectWorkspace,
        WriteOperationKind::WriteText,
        WriteTarget::new(
            path,
            boundary,
            format!("sessions/project-workspace-save/{}.json", plan.id),
        )
        .with_expected_absent(),
        WritePolicy::project_workspace_save_journal_write(),
        format!("Prepare ProjectWorkspace Save journal {}", plan.id),
    );
    WriteAuthority::new(app)
        .write_text(intent, &format!("{source}\n"))
        .map_err(|error| error.into_terminal_diagnostic())?;
    Ok(())
}

pub(super) fn clear_project_workspace_save_journal<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    transaction_id: &str,
) -> Result<(), String> {
    let path = save_journal_path(app, session, transaction_id)?;
    let boundary = project_session_dir(app, &session.project_root)?;
    let intent = WriteIntent::new(
        WriteCategory::InternalAppWrite,
        WriteOwner::ProjectWorkspace,
        WriteOperationKind::RemoveFile,
        WriteTarget::new(
            path,
            boundary,
            format!("sessions/project-workspace-save/{transaction_id}.json"),
        ),
        WritePolicy::project_workspace_save_journal_remove(),
        format!("Clear ProjectWorkspace Save journal {transaction_id}"),
    );
    WriteAuthority::new(app)
        .remove_file_if_exists(intent)
        .map_err(|error| error.into_terminal_diagnostic())?;
    Ok(())
}

fn journal_file_from_plan(
    file: ProjectWorkspaceSavePlannedFile<'_>,
) -> ProjectWorkspaceSaveJournalFile {
    match file {
        ProjectWorkspaceSavePlannedFile::Write(change) => ProjectWorkspaceSaveJournalFile {
            relative_path: change.relative_path.clone(),
            content_kind: ProjectWorkspaceSaveJournalContentKind::Text,
            existed_before: change.existed_before,
            exists_after: true,
            before_hash: change.before_hash.clone(),
            planned_hash: Some(change.new_hash.clone()),
            before_contents_base64: BASE64_STANDARD.encode(change.before_text.as_bytes()),
        },
        ProjectWorkspaceSavePlannedFile::Delete(delete) => ProjectWorkspaceSaveJournalFile {
            relative_path: delete.relative_path.clone(),
            content_kind: ProjectWorkspaceSaveJournalContentKind::Text,
            existed_before: true,
            exists_after: false,
            before_hash: delete.before_hash.clone(),
            planned_hash: None,
            before_contents_base64: BASE64_STANDARD.encode(delete.before_text.as_bytes()),
        },
        ProjectWorkspaceSavePlannedFile::BinaryWrite(change) => ProjectWorkspaceSaveJournalFile {
            relative_path: change.relative_path.clone(),
            content_kind: ProjectWorkspaceSaveJournalContentKind::Binary,
            existed_before: change.existed_before,
            exists_after: true,
            before_hash: change.before_hash.clone(),
            planned_hash: Some(change.new_hash.clone()),
            before_contents_base64: BASE64_STANDARD.encode(&change.before_bytes),
        },
        ProjectWorkspaceSavePlannedFile::BinaryDelete(delete) => ProjectWorkspaceSaveJournalFile {
            relative_path: delete.relative_path.clone(),
            content_kind: ProjectWorkspaceSaveJournalContentKind::Binary,
            existed_before: true,
            exists_after: false,
            before_hash: delete.before_hash.clone(),
            planned_hash: None,
            before_contents_base64: BASE64_STANDARD.encode(&delete.before_bytes),
        },
    }
}

fn save_journal_path<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    transaction_id: &str,
) -> Result<std::path::PathBuf, String> {
    validate_transaction_id(transaction_id)?;
    Ok(
        project_workspace_save_journal_dir(app, &session.project_root)?
            .join(format!("{transaction_id}.json")),
    )
}

fn validate_transaction_id(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 160
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
    {
        return Err(format!(
            "ProjectWorkspace Save are id invalid pentru jurnal: {value}."
        ));
    }
    Ok(())
}

pub fn scan_project_workspace_save_hot_journals(
    session: &ProjectSessionSnapshot,
    dir: &Path,
) -> Result<Vec<ProjectWorkspaceSaveHotJournal>, String> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    if !dir.is_dir() {
        return Err(format!(
            "ProjectWorkspace Save recovery blocat: {} nu este director.",
            dir.display()
        ));
    }

    let mut entries = fs::read_dir(dir)
        .map_err(|error| format!("Nu pot scana {}: {error}", dir.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Nu pot citi o intrare din {}: {error}", dir.display()))?;
    entries.sort_by_key(|entry| entry.path());

    let mut journals = Vec::new();
    let mut total_bytes = 0u64;
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        if journals.len() >= PROJECT_WORKSPACE_SAVE_JOURNAL_MAX_COUNT {
            return Err(format!(
                "ProjectWorkspace Save recovery a depășit limita de {} jurnale active.",
                PROJECT_WORKSPACE_SAVE_JOURNAL_MAX_COUNT
            ));
        }
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("Nu pot inspecta {}: {error}", path.display()))?;
        if !metadata.file_type().is_file() {
            return Err(format!(
                "ProjectWorkspace Save recovery refuză intrarea non-file {}.",
                path.display()
            ));
        }
        if metadata.len() > PROJECT_WORKSPACE_SAVE_JOURNAL_MAX_BYTES {
            return Err(format!(
                "Jurnalul {} are {} bytes, peste limita de {}.",
                path.display(),
                metadata.len(),
                PROJECT_WORKSPACE_SAVE_JOURNAL_MAX_BYTES
            ));
        }
        total_bytes = total_bytes.saturating_add(metadata.len());
        if total_bytes > PROJECT_WORKSPACE_SAVE_JOURNAL_SCAN_MAX_TOTAL_BYTES {
            return Err(format!(
                "ProjectWorkspace Save recovery a depășit limita totală de {} bytes.",
                PROJECT_WORKSPACE_SAVE_JOURNAL_SCAN_MAX_TOTAL_BYTES
            ));
        }
        journals.push(read_hot_journal(session, &path)?);
    }
    Ok(journals)
}

pub fn recover_project_workspace_save_hot_journal<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    transaction_id: &str,
    action: ProjectWorkspaceSaveRecoveryAction,
    operator_diagnostic: String,
) -> Result<ProjectWorkspaceSaveRecoveryReceipt, String> {
    let operator_diagnostic = operator_diagnostic.trim().to_string();
    if operator_diagnostic.is_empty() {
        return Err(
            "ProjectWorkspace Save recovery cere un diagnostic explicit al operatorului."
                .to_string(),
        );
    }
    if project_root.to_string_lossy() != session.project_root {
        return Err(format!(
            "ProjectWorkspace Save recovery a primit root {}, dar sesiunea aparține {}.",
            project_root.display(),
            session.project_root
        ));
    }
    validate_transaction_id(transaction_id)?;
    let journal_dir = project_workspace_save_journal_dir(app, &session.project_root)?;
    let hot = scan_project_workspace_save_hot_journals(session, &journal_dir)?
        .into_iter()
        .find(|journal| journal.transaction_id == transaction_id)
        .ok_or_else(|| {
            format!("Jurnalul ProjectWorkspace Save {transaction_id} nu mai este activ.")
        })?;
    if hot.recovery_plan.action != action {
        return Err(format!(
            "ProjectWorkspace Save recovery a refuzat acțiunea {:?}; starea proaspătă cere {:?}.",
            action, hot.recovery_plan.action
        ));
    }

    match action {
        ProjectWorkspaceSaveRecoveryAction::ClearStaleJournal => {
            clear_project_workspace_save_journal(app, session, transaction_id)?;
            Ok(ProjectWorkspaceSaveRecoveryReceipt {
                schema_version: PROJECT_WORKSPACE_SAVE_JOURNAL_SCHEMA_VERSION,
                transaction_id: transaction_id.to_string(),
                action,
                project_root: session.project_root.clone(),
                restored_files: Vec::new(),
                already_before_files: hot
                    .files
                    .into_iter()
                    .map(|file| file.relative_path)
                    .collect(),
                journal_cleared: true,
                write_receipts: Vec::new(),
                operator_diagnostic,
            })
        }
        ProjectWorkspaceSaveRecoveryAction::RollbackToBefore => {
            rollback_hot_journal_to_before(
                app,
                session,
                project_root,
                hot,
                operator_diagnostic,
            )
        }
        ProjectWorkspaceSaveRecoveryAction::ManualReviewMixedState
        | ProjectWorkspaceSaveRecoveryAction::ManualReviewConflict => Err(format!(
            "ProjectWorkspace Save recovery a blocat {:?}: starea necesită investigație manuală și nu autorizează efecte pe disc.",
            action
        )),
    }
}

fn rollback_hot_journal_to_before<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    hot: ProjectWorkspaceSaveHotJournal,
    operator_diagnostic: String,
) -> Result<ProjectWorkspaceSaveRecoveryReceipt, String> {
    let path = PathBuf::from(&hot.path);
    let source = fs::read_to_string(&path)
        .map_err(|error| format!("Nu pot reciti jurnalul {}: {error}", path.display()))?;
    let record: ProjectWorkspaceSaveJournal = serde_json::from_str(&source)
        .map_err(|error| format!("Jurnalul {} nu mai este valid: {error}", path.display()))?;
    validate_hot_journal_identity(session, &path, &record)?;

    let states = hot
        .files
        .iter()
        .map(|file| (file.relative_path.as_str(), file.disk_state))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut restored_files = Vec::new();
    let mut already_before_files = Vec::new();
    let mut write_receipts = Vec::new();
    for file in record.files.iter().rev() {
        match states.get(file.relative_path.as_str()).copied() {
            Some(ProjectWorkspaceSaveHotJournalFileDiskState::Before) => {
                already_before_files.push(file.relative_path.clone());
            }
            Some(ProjectWorkspaceSaveHotJournalFileDiskState::Planned) => {
                let receipt = restore_hot_journal_file(app, session, project_root, file)?;
                restored_files.push(file.relative_path.clone());
                write_receipts.push(receipt);
            }
            _ => {
                return Err(format!(
                    "ProjectWorkspace Save recovery a detectat o stare neeligibilă pentru {} înainte de rollback.",
                    file.relative_path
                ));
            }
        }
    }

    let journal_dir = project_workspace_save_journal_dir(app, &session.project_root)?;
    let verified = scan_project_workspace_save_hot_journals(session, &journal_dir)?
        .into_iter()
        .find(|journal| journal.transaction_id == hot.transaction_id)
        .ok_or_else(|| {
            "ProjectWorkspace Save recovery nu mai găsește jurnalul pentru verificarea finală."
                .to_string()
        })?;
    if verified.disk_state != ProjectWorkspaceSaveHotJournalDiskState::BeforeState {
        return Err(format!(
            "ProjectWorkspace Save recovery a restaurat parțial, dar verificarea finală este {:?}; jurnalul rămâne activ.",
            verified.disk_state
        ));
    }
    clear_project_workspace_save_journal(app, session, &hot.transaction_id)?;
    Ok(ProjectWorkspaceSaveRecoveryReceipt {
        schema_version: PROJECT_WORKSPACE_SAVE_JOURNAL_SCHEMA_VERSION,
        transaction_id: hot.transaction_id,
        action: ProjectWorkspaceSaveRecoveryAction::RollbackToBefore,
        project_root: session.project_root.clone(),
        restored_files,
        already_before_files,
        journal_cleared: true,
        write_receipts,
        operator_diagnostic,
    })
}

fn restore_hot_journal_file<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    file: &ProjectWorkspaceSaveJournalFile,
) -> Result<WriteReceipt, String> {
    let path = resolve_project_write_path(project_root, &file.relative_path)?;
    let disk = read_hot_disk_baseline(&path)?;
    match (&disk, file.exists_after, file.planned_hash.as_deref()) {
        (None, false, None) => {}
        (Some(disk), true, Some(expected)) if disk.hash == expected => {}
        (Some(disk), _, _) => {
            return Err(format!(
                "ProjectWorkspace Save recovery a blocat {}: hash-ul live {} nu mai este planned.",
                file.relative_path, disk.hash
            ));
        }
        (None, _, _) => {
            return Err(format!(
                "ProjectWorkspace Save recovery a blocat {}: starea live absent nu mai este planned.",
                file.relative_path
            ));
        }
    }
    let target = WriteTarget::new(
        path,
        project_root.to_path_buf(),
        format!("project/{}", file.relative_path),
    )
    .with_expected_runtime_session_id(session.runtime_instance_id());

    if file.existed_before {
        let target = match disk {
            Some(disk) => target.with_expected_present(disk.version_token, Some(disk.hash)),
            None => target.with_expected_absent(),
        };
        let before_bytes = BASE64_STANDARD
            .decode(&file.before_contents_base64)
            .map_err(|error| {
                format!(
                    "ProjectWorkspace Save recovery nu poate decoda before pentru {}: {error}",
                    file.relative_path
                )
            })?;
        if hash_bytes(&before_bytes) != file.before_hash {
            return Err(format!(
                "ProjectWorkspace Save recovery a refuzat before corupt pentru {}.",
                file.relative_path
            ));
        }
        let operation = match file.content_kind {
            ProjectWorkspaceSaveJournalContentKind::Text => WriteOperationKind::WriteText,
            ProjectWorkspaceSaveJournalContentKind::Binary => WriteOperationKind::WriteBytes,
        };
        let intent = WriteIntent::new(
            category_for_project_path(&file.relative_path),
            WriteOwner::ProjectWorkspace,
            operation,
            target,
            WritePolicy::project_workspace_write(),
            format!(
                "ProjectWorkspace Save recovery restore before pentru project/{}",
                file.relative_path
            ),
        );
        match file.content_kind {
            ProjectWorkspaceSaveJournalContentKind::Text => {
                let before_text = String::from_utf8(before_bytes).map_err(|error| {
                    format!(
                        "ProjectWorkspace Save recovery a refuzat before text non-UTF-8 pentru {}: {error}",
                        file.relative_path
                    )
                })?;
                WriteAuthority::new(app)
                    .write_text(intent, &before_text)
                    .map_err(|error| error.into_terminal_diagnostic())
            }
            ProjectWorkspaceSaveJournalContentKind::Binary => WriteAuthority::new(app)
                .write_bytes(intent, &before_bytes)
                .map_err(|error| error.into_terminal_diagnostic()),
        }
    } else {
        let disk = disk.ok_or_else(|| {
            format!(
                "ProjectWorkspace Save recovery nu mai găsește fișierul creat {}.",
                file.relative_path
            )
        })?;
        let intent = WriteIntent::new(
            category_for_project_path(&file.relative_path),
            WriteOwner::ProjectWorkspace,
            WriteOperationKind::RemoveFile,
            target.with_expected_present(disk.version_token, Some(disk.hash)),
            WritePolicy::project_workspace_remove(),
            format!(
                "ProjectWorkspace Save recovery remove created pentru project/{}",
                file.relative_path
            ),
        );
        let receipt = WriteAuthority::new(app)
            .remove_file_if_exists(intent)
            .map_err(|error| error.into_terminal_diagnostic())?;
        if receipt.status != "committed" {
            return Err(format!(
                "ProjectWorkspace Save recovery nu a eliminat {}: efectul nu a fost comis.",
                file.relative_path
            ));
        }
        Ok(receipt)
    }
}

fn category_for_project_path(relative_path: &str) -> WriteCategory {
    if relative_path.starts_with("design/") {
        WriteCategory::ProjectDesignWrite
    } else {
        WriteCategory::ProjectSourceWrite
    }
}

struct HotDiskBaseline {
    hash: String,
    version_token: String,
}

fn read_hot_disk_baseline(path: &Path) -> Result<Option<HotDiskBaseline>, String> {
    let metadata_before = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Nu pot inspecta {}: {error}", path.display())),
    };
    if metadata_before.file_type().is_symlink() || !metadata_before.is_file() {
        return Err(format!(
            "ProjectWorkspace Save recovery refuză target-ul non-regular {}.",
            path.display()
        ));
    }
    if metadata_before.len() > PROJECT_WORKSPACE_SAVE_JOURNAL_MAX_BYTES {
        return Err(format!(
            "ProjectWorkspace Save recovery refuză {}: {} bytes depășesc limita.",
            path.display(),
            metadata_before.len()
        ));
    }
    let bytes =
        fs::read(path).map_err(|error| format!("Nu pot citi {}: {error}", path.display()))?;
    let metadata_after = fs::metadata(path)
        .map_err(|error| format!("Nu pot reverifica {}: {error}", path.display()))?;
    let version_before = project_disk_metadata_version_token(&metadata_before);
    let version_after = project_disk_metadata_version_token(&metadata_after);
    if version_before != version_after {
        return Err(format!(
            "ProjectWorkspace Save recovery a detectat o schimbare concurentă în {}.",
            path.display()
        ));
    }
    Ok(Some(HotDiskBaseline {
        hash: hash_bytes(&bytes),
        version_token: version_after,
    }))
}

fn read_hot_journal(
    session: &ProjectSessionSnapshot,
    path: &Path,
) -> Result<ProjectWorkspaceSaveHotJournal, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("Nu pot citi jurnalul {}: {error}", path.display()))?;
    let record: ProjectWorkspaceSaveJournal = serde_json::from_str(&source)
        .map_err(|error| format!("Jurnalul {} nu este JSON valid: {error}", path.display()))?;
    validate_hot_journal_identity(session, path, &record)?;
    inspect_hot_journal_disk_state(path, record)
}

fn validate_hot_journal_identity(
    session: &ProjectSessionSnapshot,
    path: &Path,
    record: &ProjectWorkspaceSaveJournal,
) -> Result<(), String> {
    if record.schema_version != PROJECT_WORKSPACE_SAVE_JOURNAL_SCHEMA_VERSION {
        return Err(format!(
            "Jurnalul {} are schema {}, dar nucleul acceptă {}.",
            path.display(),
            record.schema_version,
            PROJECT_WORKSPACE_SAVE_JOURNAL_SCHEMA_VERSION
        ));
    }
    validate_transaction_id(&record.transaction_id)?;
    let expected_name = format!("{}.json", record.transaction_id);
    if path.file_name().and_then(|value| value.to_str()) != Some(expected_name.as_str()) {
        return Err(format!(
            "Jurnalul {} nu corespunde transactionId {}.",
            path.display(),
            record.transaction_id
        ));
    }
    if record.runtime_session_id != session.runtime_instance_id()
        || record.project_root != session.project_root
    {
        return Err(format!(
            "Jurnalul {} nu aparține sesiunii ProjectWorkspace curente.",
            path.display()
        ));
    }
    if record.files.is_empty() || record.touched_files.is_empty() {
        return Err(format!(
            "Jurnalul {} nu conține efecte Save.",
            path.display()
        ));
    }
    for file in &record.files {
        let before = BASE64_STANDARD
            .decode(&file.before_contents_base64)
            .map_err(|error| {
                format!(
                    "Jurnalul {} are before invalid pentru {}: {error}",
                    path.display(),
                    file.relative_path
                )
            })?;
        if hash_bytes(&before) != file.before_hash {
            return Err(format!(
                "Jurnalul {} are hash before inconsistent pentru {}.",
                path.display(),
                file.relative_path
            ));
        }
    }
    Ok(())
}

fn inspect_hot_journal_disk_state(
    path: &Path,
    record: ProjectWorkspaceSaveJournal,
) -> Result<ProjectWorkspaceSaveHotJournal, String> {
    let project_root = PathBuf::from(&record.project_root);
    let files = record
        .files
        .iter()
        .map(|file| inspect_hot_journal_file(&project_root, file))
        .collect::<Vec<_>>();
    let before_count = files
        .iter()
        .filter(|file| file.disk_state == ProjectWorkspaceSaveHotJournalFileDiskState::Before)
        .count();
    let planned_count = files
        .iter()
        .filter(|file| file.disk_state == ProjectWorkspaceSaveHotJournalFileDiskState::Planned)
        .count();
    let conflict_count = files
        .iter()
        .filter(|file| {
            matches!(
                file.disk_state,
                ProjectWorkspaceSaveHotJournalFileDiskState::Conflict
                    | ProjectWorkspaceSaveHotJournalFileDiskState::Unreadable
            )
        })
        .count();
    let disk_state = if conflict_count > 0 {
        ProjectWorkspaceSaveHotJournalDiskState::ConflictState
    } else if before_count == files.len() {
        ProjectWorkspaceSaveHotJournalDiskState::BeforeState
    } else if planned_count == files.len() {
        ProjectWorkspaceSaveHotJournalDiskState::PlannedState
    } else {
        ProjectWorkspaceSaveHotJournalDiskState::MixedState
    };
    let recovery_plan = recovery_plan(disk_state, files.len());
    Ok(ProjectWorkspaceSaveHotJournal {
        schema_version: record.schema_version,
        transaction_id: record.transaction_id,
        path: path.to_string_lossy().into_owned(),
        runtime_session_id: record.runtime_session_id,
        project_root: record.project_root,
        revision: record.revision,
        prepared_at_ms: record.prepared_at_ms,
        touched_files: record.touched_files,
        file_count: files.len(),
        bytes_before: record
            .files
            .iter()
            .filter_map(|file| BASE64_STANDARD.decode(&file.before_contents_base64).ok())
            .map(|bytes| bytes.len() as u64)
            .sum(),
        disk_state,
        recovery_plan,
        files,
    })
}

fn inspect_hot_journal_file(
    project_root: &Path,
    file: &ProjectWorkspaceSaveJournalFile,
) -> ProjectWorkspaceSaveHotJournalFile {
    let result = (|| {
        let disk_path = resolve_project_write_path(project_root, &file.relative_path)?;
        read_hot_disk_baseline(&disk_path)
    })();
    let (disk_hash, disk_state, diagnostic) = match result {
        Ok(None) if !file.existed_before => (
            None,
            ProjectWorkspaceSaveHotJournalFileDiskState::Before,
            None,
        ),
        Ok(None) if !file.exists_after => (
            None,
            ProjectWorkspaceSaveHotJournalFileDiskState::Planned,
            None,
        ),
        Ok(None) => (
            None,
            ProjectWorkspaceSaveHotJournalFileDiskState::Conflict,
            Some("Fișierul lipsește, deși jurnalul îl cere prezent în ambele stări.".to_string()),
        ),
        Ok(Some(disk)) if file.existed_before && disk.hash == file.before_hash => (
            Some(disk.hash),
            ProjectWorkspaceSaveHotJournalFileDiskState::Before,
            None,
        ),
        Ok(Some(disk))
            if file
                .planned_hash
                .as_ref()
                .is_some_and(|hash| *hash == disk.hash) =>
        {
            (
                Some(disk.hash),
                ProjectWorkspaceSaveHotJournalFileDiskState::Planned,
                None,
            )
        }
        Ok(Some(disk)) => {
            let hash = disk.hash;
            (
                Some(hash.clone()),
                ProjectWorkspaceSaveHotJournalFileDiskState::Conflict,
                Some(format!(
                    "Hash-ul disk {hash} nu corespunde stării before sau planned din jurnal."
                )),
            )
        }
        Err(error) => (
            None,
            ProjectWorkspaceSaveHotJournalFileDiskState::Unreadable,
            Some(error),
        ),
    };
    ProjectWorkspaceSaveHotJournalFile {
        relative_path: file.relative_path.clone(),
        content_kind: file.content_kind,
        existed_before: file.existed_before,
        exists_after: file.exists_after,
        before_hash: file.before_hash.clone(),
        planned_hash: file.planned_hash.clone(),
        disk_hash,
        disk_state,
        diagnostic,
    }
}

fn recovery_plan(
    disk_state: ProjectWorkspaceSaveHotJournalDiskState,
    file_count: usize,
) -> ProjectWorkspaceSaveRecoveryPlan {
    match disk_state {
        ProjectWorkspaceSaveHotJournalDiskState::BeforeState => ProjectWorkspaceSaveRecoveryPlan {
            action: ProjectWorkspaceSaveRecoveryAction::ClearStaleJournal,
            can_clear_journal: true,
            can_rollback: false,
            summary: format!(
                "Toate cele {file_count} fișiere sunt în starea before; jurnalul poate fi curățat fără mutarea proiectului."
            ),
        },
        ProjectWorkspaceSaveHotJournalDiskState::PlannedState => ProjectWorkspaceSaveRecoveryPlan {
            action: ProjectWorkspaceSaveRecoveryAction::RollbackToBefore,
            can_clear_journal: false,
            can_rollback: true,
            summary: format!(
                "Toate cele {file_count} fișiere sunt în starea planned; jurnalul permite rollback complet la before."
            ),
        },
        ProjectWorkspaceSaveHotJournalDiskState::MixedState => ProjectWorkspaceSaveRecoveryPlan {
            action: ProjectWorkspaceSaveRecoveryAction::RollbackToBefore,
            can_clear_journal: false,
            can_rollback: true,
            summary: format!(
                "Cele {file_count} fișiere sunt împărțite între before și planned, fără hash-uri străine; rollback-ul poate continua idempotent până la before."
            ),
        },
        ProjectWorkspaceSaveHotJournalDiskState::ConflictState => ProjectWorkspaceSaveRecoveryPlan {
            action: ProjectWorkspaceSaveRecoveryAction::ManualReviewConflict,
            can_clear_journal: false,
            can_rollback: false,
            summary: format!(
                "Cel puțin unul dintre cele {file_count} fișiere nu corespunde hash-urilor tranzacției; recuperarea automată este blocată."
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use base64::Engine as _;

    use crate::kernel::file_buffer_store::hash_bytes;

    use super::*;
    use crate::kernel::project_workspace::save::{
        ProjectWorkspacePlannedBinaryDelete, ProjectWorkspacePlannedBinaryWrite,
    };

    #[test]
    fn binary_journal_records_exact_before_bytes_and_content_kind() {
        let before = vec![0, 159, 146, 150, 255];
        let after = vec![0x77, 0x4f, 0x46, 0x32];
        let write = ProjectWorkspacePlannedBinaryWrite {
            relative_path: "sursa/static/fonturi/test.woff2".to_string(),
            existed_before: true,
            before_bytes: before.clone(),
            new_bytes: after.clone(),
            before_hash: hash_bytes(&before),
            new_hash: hash_bytes(&after),
        };
        let journal = journal_file_from_plan(ProjectWorkspaceSavePlannedFile::BinaryWrite(&write));
        assert_eq!(
            journal.content_kind,
            ProjectWorkspaceSaveJournalContentKind::Binary
        );
        assert!(journal.existed_before);
        assert!(journal.exists_after);
        assert_eq!(journal.before_hash, hash_bytes(&before));
        assert_eq!(journal.planned_hash, Some(hash_bytes(&after)));
        assert_eq!(
            BASE64_STANDARD
                .decode(journal.before_contents_base64)
                .unwrap(),
            before
        );
    }

    #[test]
    fn binary_delete_journal_is_reversible_and_has_no_planned_hash() {
        let before = vec![1, 2, 3, 4];
        let delete = ProjectWorkspacePlannedBinaryDelete {
            relative_path: "resurse/imagini/old.webp".to_string(),
            before_bytes: before.clone(),
            before_hash: hash_bytes(&before),
        };
        let journal =
            journal_file_from_plan(ProjectWorkspaceSavePlannedFile::BinaryDelete(&delete));
        assert_eq!(
            journal.content_kind,
            ProjectWorkspaceSaveJournalContentKind::Binary
        );
        assert!(journal.existed_before);
        assert!(!journal.exists_after);
        assert!(journal.planned_hash.is_none());
        assert_eq!(
            BASE64_STANDARD
                .decode(journal.before_contents_base64)
                .unwrap(),
            before
        );
    }
}
