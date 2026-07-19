use std::{collections::BTreeSet, path::Path};

use crate::{kernel::file_buffer_store::FileBufferStore, project::resolve_project_write_path};

use super::{
    model::{
        KernelDiskConflictGateAction, KernelDiskConflictGateDecision,
        KernelDiskConflictGateDiagnostic, KernelDiskConflictGateDiagnosticSeverity,
        KernelDiskConflictGatePolicy, KernelDiskConflictGateRequest, KernelDiskConflictGateResult,
        KernelDiskConflictKind, KERNEL_DISK_CONFLICT_GATE_SCHEMA_VERSION,
    },
    scanner::scan_disk_conflict_entry,
};

pub fn evaluate_disk_conflict_gate(
    store: &FileBufferStore,
    request: KernelDiskConflictGateRequest,
) -> Result<KernelDiskConflictGateResult, String> {
    let operation_label = normalize_operation_label(&request.operation_label)?;
    let target_paths =
        normalize_target_paths(request.target_paths, request.allow_empty_target_paths)?;

    let mut decisions = Vec::with_capacity(target_paths.len());
    let mut diagnostics = Vec::new();

    for relative_path in target_paths {
        let decision = match store.files.get(&relative_path) {
            Some(entry) if request.policy == KernelDiskConflictGatePolicy::ProjectEntryRestore => {
                restore_tracked_target_decision(store, entry)
            }
            Some(entry) => {
                let snapshot = scan_disk_conflict_entry(store, entry);
                decision_from_snapshot(request.policy, snapshot)
            }
            None => untracked_target_decision(store, request.policy, relative_path),
        };

        if !matches!(decision.action, KernelDiskConflictGateAction::Allow) {
            diagnostics.push(diagnostic_from_decision(&decision));
        }
        decisions.push(decision);
    }

    let blocked_target_count = decisions
        .iter()
        .filter(|decision| decision.action == KernelDiskConflictGateAction::Block)
        .count();
    let info_count = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == KernelDiskConflictGateDiagnosticSeverity::Info)
        .count();
    let warning_count = diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.severity == KernelDiskConflictGateDiagnosticSeverity::Warning
        })
        .count();

    Ok(KernelDiskConflictGateResult {
        schema_version: KERNEL_DISK_CONFLICT_GATE_SCHEMA_VERSION,
        operation_label,
        policy: request.policy,
        allowed: blocked_target_count == 0,
        target_count: decisions.len(),
        blocked_target_count,
        info_count,
        warning_count,
        decisions,
        diagnostics,
    })
}

fn normalize_operation_label(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("Disk Conflict Gate are operation_label gol.".to_string());
    }
    if value.contains('\0') {
        return Err("Disk Conflict Gate are operation_label invalid.".to_string());
    }
    Ok(value.chars().take(160).collect())
}

fn normalize_target_paths(
    target_paths: Vec<String>,
    allow_empty_target_paths: bool,
) -> Result<Vec<String>, String> {
    if target_paths.is_empty() && !allow_empty_target_paths {
        return Err("Disk Conflict Gate nu a primit target-uri.".to_string());
    }

    let mut unique = BTreeSet::new();
    for target_path in target_paths {
        unique.insert(normalize_target_path(&target_path)?);
    }
    Ok(unique.into_iter().collect())
}

fn normalize_target_path(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.contains('\0') {
        return Err("Disk Conflict Gate a primit target path invalid.".to_string());
    }

    let mut normalized = Vec::new();
    for segment in value.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            return Err(format!(
                "Disk Conflict Gate a primit target path în afara proiectului: {value}."
            ));
        }
        normalized.push(segment);
    }

    if normalized.is_empty() {
        return Err("Disk Conflict Gate a primit target path gol.".to_string());
    }

    Ok(normalized.join("/"))
}

fn decision_from_snapshot(
    policy: KernelDiskConflictGatePolicy,
    snapshot: super::model::KernelDiskConflictFileSnapshot,
) -> KernelDiskConflictGateDecision {
    let action = action_for_kind(policy, snapshot.kind);
    let code = code_for_kind(policy, snapshot.kind, action);
    let message = message_for_kind(policy, snapshot.kind, &snapshot.message);

    KernelDiskConflictGateDecision {
        relative_path: snapshot.relative_path,
        action,
        status: Some(snapshot.status),
        kind: Some(snapshot.kind),
        code: code.to_string(),
        message,
        baseline_hash: Some(snapshot.baseline.hash),
        disk_hash: snapshot.disk.map(|disk| disk.hash),
    }
}

fn untracked_target_decision(
    store: &FileBufferStore,
    policy: KernelDiskConflictGatePolicy,
    relative_path: String,
) -> KernelDiskConflictGateDecision {
    if policy == KernelDiskConflictGatePolicy::ProjectFileSaveText {
        return save_text_untracked_target_decision(store, relative_path);
    }
    if policy == KernelDiskConflictGatePolicy::WorkspaceMutationText
        || policy == KernelDiskConflictGatePolicy::WorkspaceMutationCurrentBufferText
    {
        return workspace_mutation_untracked_target_decision(store, policy, relative_path);
    }
    if policy == KernelDiskConflictGatePolicy::ProjectFileUndoCreatedText {
        return undo_created_text_untracked_target_decision(relative_path);
    }
    if policy == KernelDiskConflictGatePolicy::ProjectFileDeleteText {
        return delete_text_untracked_target_decision(relative_path);
    }
    if policy == KernelDiskConflictGatePolicy::ProjectEntryRestore {
        return restore_untracked_target_decision(store, relative_path);
    }

    KernelDiskConflictGateDecision {
        relative_path: relative_path.clone(),
        action: KernelDiskConflictGateAction::Block,
        status: None,
        kind: None,
        code: "disk_conflict_gate_untracked_target".to_string(),
        message: format!(
            "Target-ul {relative_path} nu este urmărit în FileBufferStore; kernel-ul nu are baseline pentru conflict gate."
        ),
        baseline_hash: None,
        disk_hash: None,
    }
}

fn save_text_untracked_target_decision(
    store: &FileBufferStore,
    relative_path: String,
) -> KernelDiskConflictGateDecision {
    text_create_untracked_target_decision(
        store,
        relative_path,
        "Save-ul",
        "disk_conflict_gate_new_save_target",
        "disk_conflict_gate_save_existing_without_baseline",
    )
}

fn workspace_mutation_untracked_target_decision(
    store: &FileBufferStore,
    policy: KernelDiskConflictGatePolicy,
    relative_path: String,
) -> KernelDiskConflictGateDecision {
    let create_code = match policy {
        KernelDiskConflictGatePolicy::WorkspaceMutationCurrentBufferText => {
            "disk_conflict_gate_new_workspace_current_buffer_target"
        }
        _ => "disk_conflict_gate_new_workspace_target",
    };
    text_create_untracked_target_decision(
        store,
        relative_path,
        "WorkspaceMutation",
        create_code,
        "disk_conflict_gate_workspace_existing_without_baseline",
    )
}

fn text_create_untracked_target_decision(
    store: &FileBufferStore,
    relative_path: String,
    operation_label: &str,
    create_code: &str,
    existing_code: &str,
) -> KernelDiskConflictGateDecision {
    let path = match resolve_project_write_path(Path::new(&store.project_root), &relative_path) {
        Ok(path) => path,
        Err(error) => {
            return KernelDiskConflictGateDecision {
                relative_path,
                action: KernelDiskConflictGateAction::Block,
                status: Some(super::model::KernelDiskConflictStatus::Error),
                kind: Some(KernelDiskConflictKind::InvalidPath),
                code: "disk_conflict_gate_invalid_path".to_string(),
                message: format!(
                    "{operation_label} a fost blocat: target-ul nu poate fi rezolvat în proiect. {error}"
                ),
                baseline_hash: None,
                disk_hash: None,
            };
        }
    };

    match path.try_exists() {
        Ok(false) => {
            return KernelDiskConflictGateDecision {
                relative_path: relative_path.clone(),
                action: KernelDiskConflictGateAction::Allow,
                status: Some(super::model::KernelDiskConflictStatus::Clean),
                kind: None,
                code: create_code.to_string(),
                message: format!(
                    "{operation_label} poate crea {relative_path}: target-ul nu există pe disk și nu are baseline anterior."
                ),
                baseline_hash: None,
                disk_hash: None,
            };
        }
        Err(error) => {
            return KernelDiskConflictGateDecision {
                relative_path,
                action: KernelDiskConflictGateAction::Block,
                status: Some(super::model::KernelDiskConflictStatus::Error),
                kind: Some(KernelDiskConflictKind::Unreadable),
                code: "disk_conflict_gate_unreadable".to_string(),
                message: format!(
                    "{operation_label} a fost blocat: existența target-ului nu poate fi verificată pentru conflict gate: {error}"
                ),
                baseline_hash: None,
                disk_hash: None,
            };
        }
        Ok(true) => {}
    }

    let message = match path.metadata() {
        Ok(metadata) if !metadata.is_file() => format!(
            "{operation_label} a fost blocat pentru {relative_path}: target-ul există pe disk, dar nu este fișier text urmărit în FileBufferStore."
        ),
        Ok(metadata) if metadata.permissions().readonly() => format!(
            "{operation_label} a fost blocat pentru {relative_path}: target-ul există pe disk fără baseline în FileBufferStore și este readonly."
        ),
        Ok(_) => format!(
            "{operation_label} a fost blocat pentru {relative_path}: fișierul există pe disk, dar nu are baseline în FileBufferStore."
        ),
        Err(error) => format!(
            "{operation_label} a fost blocat pentru {relative_path}: target-ul există, dar metadata nu poate fi citită pentru conflict gate: {error}"
        ),
    };

    KernelDiskConflictGateDecision {
        relative_path,
        action: KernelDiskConflictGateAction::Block,
        status: Some(super::model::KernelDiskConflictStatus::Error),
        kind: None,
        code: existing_code.to_string(),
        message,
        baseline_hash: None,
        disk_hash: None,
    }
}

fn undo_created_text_untracked_target_decision(
    relative_path: String,
) -> KernelDiskConflictGateDecision {
    KernelDiskConflictGateDecision {
        relative_path: relative_path.clone(),
        action: KernelDiskConflictGateAction::Block,
        status: None,
        kind: None,
        code: "disk_conflict_gate_untracked_undo_created_target".to_string(),
        message: format!(
            "Undo create a fost blocat pentru {relative_path}: FileBufferStore nu are baseline pentru fișierul creat în sesiune."
        ),
        baseline_hash: None,
        disk_hash: None,
    }
}

fn delete_text_untracked_target_decision(relative_path: String) -> KernelDiskConflictGateDecision {
    KernelDiskConflictGateDecision {
        relative_path: relative_path.clone(),
        action: KernelDiskConflictGateAction::Block,
        status: None,
        kind: None,
        code: "disk_conflict_gate_untracked_delete_text_target".to_string(),
        message: format!(
            "Delete text a fost blocat pentru {relative_path}: FileBufferStore nu are baseline pentru fișierul care ar fi șters."
        ),
        baseline_hash: None,
        disk_hash: None,
    }
}

fn restore_tracked_target_decision(
    store: &FileBufferStore,
    entry: &crate::kernel::file_buffer_store::FileBufferEntry,
) -> KernelDiskConflictGateDecision {
    let snapshot = scan_disk_conflict_entry(store, entry);
    let relative_path = snapshot.relative_path;

    KernelDiskConflictGateDecision {
        relative_path: relative_path.clone(),
        action: KernelDiskConflictGateAction::Block,
        status: Some(snapshot.status),
        kind: Some(snapshot.kind),
        code: "disk_conflict_gate_restore_tracked_destination".to_string(),
        message: format!(
            "Restore-ul a fost blocat: FileBufferStore are deja baseline pentru {}; restaurarea din trash nu are voie să suprascrie sau să ascundă o destinație urmărită.",
            relative_path
        ),
        baseline_hash: Some(snapshot.baseline.hash),
        disk_hash: snapshot.disk.map(|disk| disk.hash),
    }
}

fn restore_untracked_target_decision(
    store: &FileBufferStore,
    relative_path: String,
) -> KernelDiskConflictGateDecision {
    let path = match resolve_project_write_path(Path::new(&store.project_root), &relative_path) {
        Ok(path) => path,
        Err(error) => {
            return KernelDiskConflictGateDecision {
                relative_path,
                action: KernelDiskConflictGateAction::Block,
                status: Some(super::model::KernelDiskConflictStatus::Error),
                kind: Some(KernelDiskConflictKind::InvalidPath),
                code: "disk_conflict_gate_invalid_path".to_string(),
                message: format!(
                    "Restore-ul a fost blocat: destinația nu poate fi rezolvată în proiect. {error}"
                ),
                baseline_hash: None,
                disk_hash: None,
            };
        }
    };

    match path.try_exists() {
        Ok(false) => KernelDiskConflictGateDecision {
            relative_path: relative_path.clone(),
            action: KernelDiskConflictGateAction::Allow,
            status: Some(super::model::KernelDiskConflictStatus::Clean),
            kind: None,
            code: "disk_conflict_gate_restore_missing_destination".to_string(),
            message: format!(
                "Restore-ul poate continua pentru {relative_path}: destinația proiectului este liberă pe disk și nu are baseline în FileBufferStore."
            ),
            baseline_hash: None,
            disk_hash: None,
        },
        Ok(true) => {
            let message = match path.metadata() {
                Ok(metadata) if metadata.permissions().readonly() => format!(
                    "Restore-ul a fost blocat pentru {relative_path}: destinația există deja pe disk și este readonly."
                ),
                Ok(metadata) if metadata.is_file() => format!(
                    "Restore-ul a fost blocat pentru {relative_path}: destinația există deja pe disk ca fișier."
                ),
                Ok(metadata) if metadata.is_dir() => format!(
                    "Restore-ul a fost blocat pentru {relative_path}: destinația există deja pe disk ca dosar."
                ),
                Ok(_) => format!(
                    "Restore-ul a fost blocat pentru {relative_path}: destinația există deja pe disk și nu este o intrare proiect simplă."
                ),
                Err(error) => format!(
                    "Restore-ul a fost blocat pentru {relative_path}: destinația există, dar metadata nu poate fi citită pentru conflict gate: {error}"
                ),
            };

            KernelDiskConflictGateDecision {
                relative_path,
                action: KernelDiskConflictGateAction::Block,
                status: Some(super::model::KernelDiskConflictStatus::Error),
                kind: None,
                code: "disk_conflict_gate_restore_destination_exists".to_string(),
                message,
                baseline_hash: None,
                disk_hash: None,
            }
        }
        Err(error) => KernelDiskConflictGateDecision {
            relative_path,
            action: KernelDiskConflictGateAction::Block,
            status: Some(super::model::KernelDiskConflictStatus::Error),
            kind: Some(KernelDiskConflictKind::Unreadable),
            code: "disk_conflict_gate_unreadable".to_string(),
            message: format!(
                "Restore-ul a fost blocat: existența destinației nu poate fi verificată pentru conflict gate: {error}"
            ),
            baseline_hash: None,
            disk_hash: None,
        },
    }
}

fn action_for_kind(
    policy: KernelDiskConflictGatePolicy,
    kind: KernelDiskConflictKind,
) -> KernelDiskConflictGateAction {
    match kind {
        KernelDiskConflictKind::Clean => KernelDiskConflictGateAction::Allow,
        KernelDiskConflictKind::MetadataChanged => KernelDiskConflictGateAction::AllowWithInfo,
        KernelDiskConflictKind::DirtyOnly => match policy {
            KernelDiskConflictGatePolicy::ProjectFileSaveText
            | KernelDiskConflictGatePolicy::WorkspaceMutationCurrentBufferText => {
                KernelDiskConflictGateAction::AllowWithInfo
            }
            KernelDiskConflictGatePolicy::ProjectFileUndoCreatedText
            | KernelDiskConflictGatePolicy::ProjectFileDeleteText
            | KernelDiskConflictGatePolicy::WorkspaceMutationText
            | KernelDiskConflictGatePolicy::ProjectEntryMove
            | KernelDiskConflictGatePolicy::ProjectEntryRename
            | KernelDiskConflictGatePolicy::ProjectEntryTrash
            | KernelDiskConflictGatePolicy::ProjectEntryRestore => {
                KernelDiskConflictGateAction::Block
            }
        },
        KernelDiskConflictKind::DiskChanged
        | KernelDiskConflictKind::MissingOnDisk
        | KernelDiskConflictKind::Readonly
        | KernelDiskConflictKind::NotFile
        | KernelDiskConflictKind::Oversized
        | KernelDiskConflictKind::Unreadable
        | KernelDiskConflictKind::InvalidPath => KernelDiskConflictGateAction::Block,
    }
}

fn code_for_kind(
    policy: KernelDiskConflictGatePolicy,
    kind: KernelDiskConflictKind,
    action: KernelDiskConflictGateAction,
) -> &'static str {
    match (policy, kind, action) {
        (_, KernelDiskConflictKind::Clean, _) => "disk_conflict_gate_clean",
        (_, KernelDiskConflictKind::MetadataChanged, _) => "disk_conflict_gate_metadata_changed",
        (
            KernelDiskConflictGatePolicy::ProjectFileSaveText,
            KernelDiskConflictKind::DirtyOnly,
            KernelDiskConflictGateAction::AllowWithInfo,
        ) => "disk_conflict_gate_dirty_save_target",
        (
            KernelDiskConflictGatePolicy::ProjectFileUndoCreatedText,
            KernelDiskConflictKind::DirtyOnly,
            KernelDiskConflictGateAction::Block,
        ) => "disk_conflict_gate_dirty_undo_created_target",
        (
            KernelDiskConflictGatePolicy::ProjectFileDeleteText,
            KernelDiskConflictKind::DirtyOnly,
            KernelDiskConflictGateAction::Block,
        ) => "disk_conflict_gate_dirty_delete_text_target",
        (
            KernelDiskConflictGatePolicy::WorkspaceMutationText,
            KernelDiskConflictKind::DirtyOnly,
            KernelDiskConflictGateAction::Block,
        ) => "disk_conflict_gate_dirty_workspace_target",
        (
            KernelDiskConflictGatePolicy::WorkspaceMutationCurrentBufferText,
            KernelDiskConflictKind::DirtyOnly,
            KernelDiskConflictGateAction::AllowWithInfo,
        ) => "disk_conflict_gate_dirty_workspace_current_buffer_target",
        (
            KernelDiskConflictGatePolicy::ProjectEntryMove,
            KernelDiskConflictKind::DirtyOnly,
            KernelDiskConflictGateAction::Block,
        ) => "disk_conflict_gate_dirty_move_source",
        (
            KernelDiskConflictGatePolicy::ProjectEntryRename,
            KernelDiskConflictKind::DirtyOnly,
            KernelDiskConflictGateAction::Block,
        ) => "disk_conflict_gate_dirty_rename_source",
        (
            KernelDiskConflictGatePolicy::ProjectEntryTrash,
            KernelDiskConflictKind::DirtyOnly,
            KernelDiskConflictGateAction::Block,
        ) => "disk_conflict_gate_dirty_trash_source",
        (
            KernelDiskConflictGatePolicy::ProjectEntryRestore,
            KernelDiskConflictKind::DirtyOnly,
            KernelDiskConflictGateAction::Block,
        ) => "disk_conflict_gate_dirty_restore_destination",
        (_, KernelDiskConflictKind::DirtyOnly, _) => "disk_conflict_gate_dirty_target",
        (_, KernelDiskConflictKind::DiskChanged, _) => "disk_conflict_gate_disk_changed",
        (_, KernelDiskConflictKind::MissingOnDisk, _) => "disk_conflict_gate_missing_on_disk",
        (_, KernelDiskConflictKind::Readonly, _) => "disk_conflict_gate_readonly",
        (_, KernelDiskConflictKind::NotFile, _) => "disk_conflict_gate_not_file",
        (_, KernelDiskConflictKind::Oversized, _) => "disk_conflict_gate_oversized",
        (_, KernelDiskConflictKind::Unreadable, _) => "disk_conflict_gate_unreadable",
        (_, KernelDiskConflictKind::InvalidPath, _) => "disk_conflict_gate_invalid_path",
    }
}

fn message_for_kind(
    policy: KernelDiskConflictGatePolicy,
    kind: KernelDiskConflictKind,
    snapshot_message: &str,
) -> String {
    match (policy, kind) {
        (KernelDiskConflictGatePolicy::ProjectFileSaveText, KernelDiskConflictKind::DirtyOnly) => {
            "Save-ul simplu poate continua: target-ul are draft local, iar disk-ul este încă la baseline-ul sesiunii.".to_string()
        }
        (
            KernelDiskConflictGatePolicy::ProjectFileUndoCreatedText,
            KernelDiskConflictKind::DirtyOnly,
        ) => {
            "Undo create a fost blocat: target-ul are draft local nesalvat; ștergerea fișierului creat nu are voie să piardă modificări locale.".to_string()
        }
        (
            KernelDiskConflictGatePolicy::ProjectFileDeleteText,
            KernelDiskConflictKind::DirtyOnly,
        ) => {
            "Delete text a fost blocat: target-ul are draft local nesalvat; ștergerea fișierului nu are voie să piardă modificări locale.".to_string()
        }
        (KernelDiskConflictGatePolicy::WorkspaceMutationText, KernelDiskConflictKind::DirtyOnly) => {
            "WorkspaceMutation a fost blocat: target-ul are draft nesalvat în FileBufferStore, iar planul multi-fișier lucrează pe baseline controlat.".to_string()
        }
        (
            KernelDiskConflictGatePolicy::WorkspaceMutationCurrentBufferText,
            KernelDiskConflictKind::DirtyOnly,
        ) => {
            "WorkspaceMutation current-buffer poate continua: target-ul are draft local validat, iar disk-ul este încă la baseline-ul sesiunii.".to_string()
        }
        (KernelDiskConflictGatePolicy::ProjectEntryMove, KernelDiskConflictKind::DirtyOnly) => {
            "ProjectLifecycle move a fost blocat: sursa conține draft nesalvat în FileBufferStore; mutarea path-ului cere mai întâi salvare sau decizie explicită.".to_string()
        }
        (KernelDiskConflictGatePolicy::ProjectEntryRename, KernelDiskConflictKind::DirtyOnly) => {
            "ProjectLifecycle rename a fost blocat: sursa conține draft nesalvat în FileBufferStore; redenumirea path-ului cere mai întâi salvare sau decizie explicită.".to_string()
        }
        (KernelDiskConflictGatePolicy::ProjectEntryTrash, KernelDiskConflictKind::DirtyOnly) => {
            "ProjectLifecycle trash a fost blocat: sursa conține draft nesalvat în FileBufferStore; trimiterea la trash nu are voie să ascundă modificări locale nesalvate.".to_string()
        }
        (KernelDiskConflictGatePolicy::ProjectEntryRestore, KernelDiskConflictKind::DirtyOnly) => {
            "ProjectLifecycle restore a fost blocat: destinația are draft nesalvat în FileBufferStore; restaurarea din trash cere destinație liberă.".to_string()
        }
        (_, KernelDiskConflictKind::MetadataChanged) => {
            format!("{snapshot_message} Gate-ul permite execuția deoarece hash-ul text este stabil.")
        }
        _ => snapshot_message.to_string(),
    }
}

fn diagnostic_from_decision(
    decision: &KernelDiskConflictGateDecision,
) -> KernelDiskConflictGateDiagnostic {
    let blocking = decision.action == KernelDiskConflictGateAction::Block;
    let severity = if blocking {
        match decision.kind {
            Some(KernelDiskConflictKind::NotFile)
            | Some(KernelDiskConflictKind::Oversized)
            | Some(KernelDiskConflictKind::Unreadable)
            | Some(KernelDiskConflictKind::InvalidPath)
            | None => KernelDiskConflictGateDiagnosticSeverity::Error,
            _ => KernelDiskConflictGateDiagnosticSeverity::Warning,
        }
    } else {
        KernelDiskConflictGateDiagnosticSeverity::Info
    };

    KernelDiskConflictGateDiagnostic {
        severity,
        code: decision.code.clone(),
        relative_path: Some(decision.relative_path.clone()),
        message: decision.message.clone(),
        blocking,
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, time::UNIX_EPOCH};

    use crate::kernel::file_buffer_store::{
        hash_text, FileBufferBaseline, FileBufferEntry, FileBufferStore, FileBufferStoreLimits,
        TextBufferLanguage, TextBufferRole,
    };

    use super::*;

    #[test]
    fn gate_allows_clean_targets() {
        let root = test_root("allows-clean");
        write_text(&root, "sursa/templates/base.html", "base");
        let mut store = store(&root);
        store.insert_loaded_file(entry_from_disk(&root, "sursa/templates/base.html", "base"));

        let result = evaluate_disk_conflict_gate(
            &store,
            KernelDiskConflictGateRequest::workspace_mutation_text(
                "WorkspaceMutation",
                vec!["sursa/templates/base.html".to_string()],
            ),
        )
        .unwrap();

        assert!(result.allowed);
        assert_eq!(result.blocked_target_count, 0);
        assert!(result.diagnostics.is_empty());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn gate_blocks_disk_changed_targets() {
        let root = test_root("blocks-disk-changed");
        write_text(&root, "sursa/templates/base.html", "base");
        let mut store = store(&root);
        store.insert_loaded_file(entry_from_disk(&root, "sursa/templates/base.html", "base"));
        write_text(&root, "sursa/templates/base.html", "external");

        let result = evaluate_disk_conflict_gate(
            &store,
            KernelDiskConflictGateRequest::workspace_mutation_text(
                "WorkspaceMutation",
                vec!["sursa/templates/base.html".to_string()],
            ),
        )
        .unwrap();

        assert!(!result.allowed);
        assert_eq!(result.blocked_target_count, 1);
        assert_eq!(
            result.decisions[0].kind,
            Some(KernelDiskConflictKind::DiskChanged)
        );
        assert!(result
            .blocking_message()
            .unwrap()
            .contains("Disk Conflict Gate"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn workspace_mutation_policy_blocks_dirty_targets() {
        let root = test_root("blocks-dirty");
        write_text(&root, "sursa/templates/base.html", "base");
        let mut store = store(&root);
        store.insert_loaded_file(entry_from_disk(&root, "sursa/templates/base.html", "base"));
        store
            .set_draft("sursa/templates/base.html", "draft".to_string(), 10)
            .unwrap();

        let result = evaluate_disk_conflict_gate(
            &store,
            KernelDiskConflictGateRequest::workspace_mutation_text(
                "WorkspaceMutation",
                vec!["sursa/templates/base.html".to_string()],
            ),
        )
        .unwrap();

        assert!(!result.allowed);
        assert_eq!(
            result.diagnostics[0].code,
            "disk_conflict_gate_dirty_workspace_target"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn workspace_mutation_current_buffer_policy_allows_dirty_targets() {
        let root = test_root("allows-dirty-current-buffer");
        write_text(&root, "sursa/templates/base.html", "base");
        let mut store = store(&root);
        store.insert_loaded_file(entry_from_disk(&root, "sursa/templates/base.html", "base"));
        store
            .set_draft("sursa/templates/base.html", "draft".to_string(), 10)
            .unwrap();

        let result = evaluate_disk_conflict_gate(
            &store,
            KernelDiskConflictGateRequest::workspace_mutation_current_buffer_text(
                "WorkspaceMutation current buffer",
                vec!["sursa/templates/base.html".to_string()],
            ),
        )
        .unwrap();

        assert!(result.allowed);
        assert_eq!(
            result.diagnostics[0].code,
            "disk_conflict_gate_dirty_workspace_current_buffer_target"
        );
        assert!(!result.diagnostics[0].blocking);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn lifecycle_policy_allows_entries_without_tracked_text_targets() {
        let root = test_root("allows-empty-lifecycle");
        let store = store(&root);

        let result = evaluate_disk_conflict_gate(
            &store,
            KernelDiskConflictGateRequest::project_entry_move("project.entry.move", Vec::new()),
        )
        .unwrap();

        assert!(result.allowed);
        assert_eq!(result.target_count, 0);
        assert!(result.diagnostics.is_empty());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn lifecycle_policy_blocks_dirty_sources() {
        let root = test_root("blocks-dirty-lifecycle");
        write_text(&root, "sursa/templates/base.html", "base");
        let mut store = store(&root);
        store.insert_loaded_file(entry_from_disk(&root, "sursa/templates/base.html", "base"));
        store
            .set_draft("sursa/templates/base.html", "draft".to_string(), 10)
            .unwrap();

        let result = evaluate_disk_conflict_gate(
            &store,
            KernelDiskConflictGateRequest::project_entry_rename(
                "project.entry.rename",
                vec!["sursa/templates/base.html".to_string()],
            ),
        )
        .unwrap();

        assert!(!result.allowed);
        assert_eq!(
            result.diagnostics[0].code,
            "disk_conflict_gate_dirty_rename_source"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn save_policy_allows_new_file_when_disk_target_is_missing() {
        let root = test_root("allows-new-save");
        let store = store(&root);

        let result = evaluate_disk_conflict_gate(
            &store,
            KernelDiskConflictGateRequest::project_file_save_text(
                "project.file.save_text",
                vec!["sursa/content/new.md".to_string()],
            ),
        )
        .unwrap();

        assert!(result.allowed);
        assert_eq!(
            result.decisions[0].code,
            "disk_conflict_gate_new_save_target"
        );
        assert!(result.diagnostics.is_empty());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn save_policy_blocks_existing_file_without_baseline() {
        let root = test_root("blocks-existing-without-baseline");
        write_text(&root, "sursa/content/existing.md", "existing");
        let store = store(&root);

        let result = evaluate_disk_conflict_gate(
            &store,
            KernelDiskConflictGateRequest::project_file_save_text(
                "project.file.save_text",
                vec!["sursa/content/existing.md".to_string()],
            ),
        )
        .unwrap();

        assert!(!result.allowed);
        assert_eq!(
            result.decisions[0].code,
            "disk_conflict_gate_save_existing_without_baseline"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn save_policy_allows_dirty_tracked_target_as_local_save() {
        let root = test_root("allows-dirty-save");
        write_text(&root, "sursa/templates/base.html", "base");
        let mut store = store(&root);
        store.insert_loaded_file(entry_from_disk(&root, "sursa/templates/base.html", "base"));
        store
            .set_draft("sursa/templates/base.html", "draft".to_string(), 10)
            .unwrap();

        let result = evaluate_disk_conflict_gate(
            &store,
            KernelDiskConflictGateRequest::project_file_save_text(
                "project.file.save_text",
                vec!["sursa/templates/base.html".to_string()],
            ),
        )
        .unwrap();

        assert!(result.allowed);
        assert_eq!(result.info_count, 1);
        assert_eq!(
            result.diagnostics[0].code,
            "disk_conflict_gate_dirty_save_target"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn undo_created_policy_allows_clean_tracked_target() {
        let root = test_root("allows-clean-undo-created");
        write_text(&root, "sursa/content/new.md", "created");
        let mut store = store(&root);
        store.insert_loaded_file(entry_from_disk(&root, "sursa/content/new.md", "created"));

        let result = evaluate_disk_conflict_gate(
            &store,
            KernelDiskConflictGateRequest::project_file_undo_created_text(
                "project.file.undo_created_text",
                vec!["sursa/content/new.md".to_string()],
            ),
        )
        .unwrap();

        assert!(result.allowed);
        assert_eq!(result.blocked_target_count, 0);
        assert!(result.diagnostics.is_empty());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn undo_created_policy_blocks_dirty_tracked_target() {
        let root = test_root("blocks-dirty-undo-created");
        write_text(&root, "sursa/content/new.md", "created");
        let mut store = store(&root);
        store.insert_loaded_file(entry_from_disk(&root, "sursa/content/new.md", "created"));
        store
            .set_draft("sursa/content/new.md", "local draft".to_string(), 10)
            .unwrap();

        let result = evaluate_disk_conflict_gate(
            &store,
            KernelDiskConflictGateRequest::project_file_undo_created_text(
                "project.file.undo_created_text",
                vec!["sursa/content/new.md".to_string()],
            ),
        )
        .unwrap();

        assert!(!result.allowed);
        assert_eq!(
            result.diagnostics[0].code,
            "disk_conflict_gate_dirty_undo_created_target"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn undo_created_policy_blocks_untracked_target() {
        let root = test_root("blocks-untracked-undo-created");
        write_text(&root, "sursa/content/new.md", "created");
        let store = store(&root);

        let result = evaluate_disk_conflict_gate(
            &store,
            KernelDiskConflictGateRequest::project_file_undo_created_text(
                "project.file.undo_created_text",
                vec!["sursa/content/new.md".to_string()],
            ),
        )
        .unwrap();

        assert!(!result.allowed);
        assert_eq!(
            result.diagnostics[0].code,
            "disk_conflict_gate_untracked_undo_created_target"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn undo_created_policy_blocks_changed_disk_target() {
        let root = test_root("blocks-changed-undo-created");
        write_text(&root, "sursa/content/new.md", "created");
        let mut store = store(&root);
        store.insert_loaded_file(entry_from_disk(&root, "sursa/content/new.md", "created"));
        write_text(&root, "sursa/content/new.md", "external change");

        let result = evaluate_disk_conflict_gate(
            &store,
            KernelDiskConflictGateRequest::project_file_undo_created_text(
                "project.file.undo_created_text",
                vec!["sursa/content/new.md".to_string()],
            ),
        )
        .unwrap();

        assert!(!result.allowed);
        assert_eq!(
            result.decisions[0].kind,
            Some(KernelDiskConflictKind::DiskChanged)
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn restore_policy_allows_missing_untracked_destination() {
        let root = test_root("allows-missing-restore-destination");
        let store = store(&root);

        let result = evaluate_disk_conflict_gate(
            &store,
            KernelDiskConflictGateRequest::project_entry_restore(
                "project.entry.restore",
                vec!["sursa/templates/restored.html".to_string()],
            ),
        )
        .unwrap();

        assert!(result.allowed);
        assert_eq!(
            result.decisions[0].code,
            "disk_conflict_gate_restore_missing_destination"
        );
        assert!(result.diagnostics.is_empty());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn restore_policy_blocks_existing_untracked_destination() {
        let root = test_root("blocks-existing-restore-destination");
        write_text(&root, "sursa/templates/restored.html", "existing");
        let store = store(&root);

        let result = evaluate_disk_conflict_gate(
            &store,
            KernelDiskConflictGateRequest::project_entry_restore(
                "project.entry.restore",
                vec!["sursa/templates/restored.html".to_string()],
            ),
        )
        .unwrap();

        assert!(!result.allowed);
        assert_eq!(
            result.decisions[0].code,
            "disk_conflict_gate_restore_destination_exists"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn restore_policy_blocks_tracked_destination_even_when_clean() {
        let root = test_root("blocks-tracked-restore-destination");
        write_text(&root, "sursa/templates/restored.html", "existing");
        let mut store = store(&root);
        store.insert_loaded_file(entry_from_disk(
            &root,
            "sursa/templates/restored.html",
            "existing",
        ));

        let result = evaluate_disk_conflict_gate(
            &store,
            KernelDiskConflictGateRequest::project_entry_restore(
                "project.entry.restore",
                vec!["sursa/templates/restored.html".to_string()],
            ),
        )
        .unwrap();

        assert!(!result.allowed);
        assert_eq!(
            result.decisions[0].code,
            "disk_conflict_gate_restore_tracked_destination"
        );

        let _ = fs::remove_dir_all(root);
    }

    fn test_root(name: &str) -> PathBuf {
        let mut root = std::env::temp_dir();
        root.push(format!(
            "pana-studio-disk-conflict-gate-{name}-{}",
            now_ms()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn store(root: &std::path::Path) -> FileBufferStore {
        FileBufferStore::new(
            "session-1",
            root.to_string_lossy(),
            1,
            FileBufferStoreLimits {
                max_files: 20,
                max_file_bytes: 1024,
                max_total_bytes: 4096,
            },
        )
    }

    fn entry_from_disk(
        root: &std::path::Path,
        relative_path: &str,
        baseline_text: &str,
    ) -> FileBufferEntry {
        let absolute_path = root.join(relative_path);
        FileBufferEntry {
            relative_path: relative_path.to_string(),
            absolute_path: absolute_path.to_string_lossy().to_string(),
            language: TextBufferLanguage::Html,
            role: TextBufferRole::Template,
            baseline: baseline_from_disk(root, relative_path, baseline_text),
            baseline_text: baseline_text.to_string(),
            draft: None,
            revision: 1,
        }
    }

    fn baseline_from_disk(
        root: &std::path::Path,
        relative_path: &str,
        text: &str,
    ) -> FileBufferBaseline {
        let metadata = fs::metadata(root.join(relative_path)).unwrap();
        FileBufferBaseline {
            hash: hash_text(text),
            modified_ms: metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_millis())
                .unwrap_or(0),
            size: metadata.len(),
            readonly: metadata.permissions().readonly(),
        }
    }

    fn write_text(root: &std::path::Path, relative_path: &str, text: &str) {
        let path = root.join(relative_path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn now_ms() -> u128 {
        std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default()
    }
}
