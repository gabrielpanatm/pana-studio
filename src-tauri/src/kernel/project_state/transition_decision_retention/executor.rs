use std::path::Path;

use tauri::{AppHandle, Runtime};

use crate::kernel::{
    bounded_journal_reader::lock_bounded_journal_parent_exclusive, file_buffer_store::hash_text,
    observability::now_ms, project_session::ProjectSessionSnapshot,
};

use super::super::{
    read_kernel_project_transition_decision_journal_snapshot,
    KernelProjectTransitionDecisionRecoveryPlanStatus,
};
use super::{
    acknowledgement::require_retention_acknowledgement,
    coordination::lock_project_transition_decision_retention,
    events::{
        append_retention_completion_event, append_retention_failed_event,
        append_retention_planned_event,
    },
    guards::{
        ensure_no_active_hot_journals, normalize_operator_diagnostic,
        validate_hot_journal_payload_budget, validate_retention_plan_hash,
    },
    ids::next_project_transition_decision_retention_id,
    journal_split::parse_decision_journal_for_retention,
    model::ProjectTransitionDecisionRetentionJournal,
    paths::{decision_journal_path, retention_archive_path, retention_hot_journal_path},
    stable_file::capture_retention_file_baseline,
    writes::{
        clear_project_transition_decision_retention_hot_journal,
        write_project_transition_decision_journal,
        write_project_transition_decision_retention_archive,
        write_project_transition_decision_retention_hot_journal,
    },
    KernelProjectTransitionDecisionRetentionInput, KernelProjectTransitionDecisionRetentionReceipt,
    KernelProjectTransitionDecisionRetentionStatus,
    KERNEL_PROJECT_TRANSITION_DECISION_RETENTION_HOT_JOURNAL_SCHEMA_VERSION,
    KERNEL_PROJECT_TRANSITION_DECISION_RETENTION_SCHEMA_VERSION,
};

pub fn execute_project_transition_decision_retention<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    input: KernelProjectTransitionDecisionRetentionInput,
) -> Result<KernelProjectTransitionDecisionRetentionReceipt, String> {
    let started_at_ms = now_ms();
    let retention_id = next_project_transition_decision_retention_id();
    let result = execute_project_transition_decision_retention_inner(
        app,
        session,
        input,
        started_at_ms,
        retention_id.clone(),
    );

    match result {
        Ok(receipt) => {
            append_retention_completion_event(app, session, &receipt)?;
            Ok(receipt)
        }
        Err(error) => {
            let receipt = KernelProjectTransitionDecisionRetentionReceipt {
                schema_version: KERNEL_PROJECT_TRANSITION_DECISION_RETENTION_SCHEMA_VERSION,
                retention_id,
                session_id: session.id.clone(),
                decision_journal_path: decision_journal_path(session).to_string_lossy().to_string(),
                archive_path: None,
                hot_journal_path: None,
                status: KernelProjectTransitionDecisionRetentionStatus::RecoveryAttention,
                started_at_ms,
                completed_at_ms: now_ms(),
                acknowledgement_id: String::new(),
                recovery_plan_evidence_hash: String::new(),
                diagnostic: String::new(),
                candidate_record_ids: Vec::new(),
                before_journal_hash: String::new(),
                after_journal_hash: String::new(),
                archive_hash: String::new(),
                hot_journal_written: false,
                archive_written: false,
                active_journal_written: false,
                hot_journal_cleared: false,
                retention_candidate_count: 0,
                archived_record_count: 0,
                kept_record_count: 0,
                write_receipts: Vec::new(),
                recovery_diagnostic: Some(error.clone()),
            };
            let _ = append_retention_failed_event(app, session, &receipt, error.clone());
            Err(error)
        }
    }
}

fn execute_project_transition_decision_retention_inner<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    input: KernelProjectTransitionDecisionRetentionInput,
    started_at_ms: u128,
    retention_id: String,
) -> Result<KernelProjectTransitionDecisionRetentionReceipt, String> {
    let _retention_guard = lock_project_transition_decision_retention();
    let diagnostic = normalize_operator_diagnostic(input.diagnostic)?;
    ensure_no_active_hot_journals(session)?;
    let decision_journal =
        read_kernel_project_transition_decision_journal_snapshot(session, Some(500))?;
    validate_retention_plan_hash(
        &decision_journal.recovery_plan.evidence_hash,
        &input.recovery_plan_evidence_hash,
    )?;
    if decision_journal.recovery_plan.status
        != KernelProjectTransitionDecisionRecoveryPlanStatus::RetentionReview
    {
        return Err(format!(
            "ProjectTransition Decision retention blocat: recovery plan-ul curent este {:?}, nu retention_review.",
            decision_journal.recovery_plan.status
        ));
    }
    if decision_journal
        .recovery_plan
        .retention_candidates
        .is_empty()
    {
        return Err(
            "ProjectTransition Decision retention blocat: planul retention_review nu conține candidați."
                .to_string(),
        );
    }
    let acknowledgement = require_retention_acknowledgement(
        session,
        &decision_journal.recovery_plan.evidence_hash,
        &input.acknowledgement_id,
    )?;
    let active_journal_path = Path::new(&decision_journal.path);
    // Append v2 writers lock exact același parent. Lock-ul protejează numai
    // captura fresh; trebuie eliberat înainte de WriteAuthority pentru a păstra
    // ordinea globală WAL -> parent și a evita inversiunea cu Append v2.
    // Baseline-ul version+hash de mai jos transformă orice append intercalat
    // după release într-un conflict CAS, fără pierderea recordului nou.
    let active_journal_lock = lock_bounded_journal_parent_exclusive(
        active_journal_path,
        "ProjectTransition Decision retention active journal",
    )?;
    let parsed_journal = parse_decision_journal_for_retention(
        active_journal_path,
        &decision_journal
            .recovery_plan
            .retention_candidates
            .iter()
            .map(|candidate| candidate.record_id.clone())
            .collect::<Vec<_>>(),
        &active_journal_lock,
    )?;
    drop(active_journal_lock);
    let hot_journal_path = retention_hot_journal_path(session, &retention_id);
    let archive_path = retention_archive_path(session, &retention_id);
    let hot_journal = ProjectTransitionDecisionRetentionJournal {
        schema_version: KERNEL_PROJECT_TRANSITION_DECISION_RETENTION_HOT_JOURNAL_SCHEMA_VERSION,
        retention_id: retention_id.clone(),
        session_id: session.id.clone(),
        project_root: session.project_root.clone(),
        decision_journal_path: decision_journal.path.clone(),
        archive_path: archive_path.to_string_lossy().to_string(),
        created_at_ms: now_ms(),
        acknowledgement_id: acknowledgement.id,
        recovery_plan_evidence_hash: decision_journal.recovery_plan.evidence_hash.clone(),
        diagnostic: diagnostic.clone(),
        candidate_record_ids: parsed_journal.candidate_record_ids.clone(),
        candidate_count: parsed_journal.candidate_record_ids.len(),
        archived_record_count: parsed_journal.archived_record_count,
        kept_record_count: parsed_journal.kept_record_count,
        before_journal_hash: parsed_journal.before_hash.clone(),
        after_journal_hash: parsed_journal.after_hash.clone(),
        archive_hash: parsed_journal.archive_hash.clone(),
        before_journal_text: parsed_journal.before_text.clone(),
        after_journal_text: parsed_journal.after_text.clone(),
        archive_text: parsed_journal.archive_text.clone(),
    };
    validate_hot_journal_payload_budget(&hot_journal)?;
    let expected_hot_journal_hash = hash_text(
        &serde_json::to_string_pretty(&hot_journal).map_err(|error| {
            format!(
                "ProjectTransition Decision retention nu poate serializa baseline-ul hot journal: {error}"
            )
        })?,
    );

    let mut write_receipts = Vec::new();
    let hot_journal_receipt =
        write_project_transition_decision_retention_hot_journal(app, session, &hot_journal)?;
    write_receipts.push(hot_journal_receipt);
    let planned_receipt = KernelProjectTransitionDecisionRetentionReceipt {
        schema_version: KERNEL_PROJECT_TRANSITION_DECISION_RETENTION_SCHEMA_VERSION,
        retention_id: retention_id.clone(),
        session_id: session.id.clone(),
        decision_journal_path: decision_journal.path.clone(),
        archive_path: Some(archive_path.to_string_lossy().to_string()),
        hot_journal_path: Some(hot_journal_path.to_string_lossy().to_string()),
        status: KernelProjectTransitionDecisionRetentionStatus::RecoveryAttention,
        started_at_ms,
        completed_at_ms: now_ms(),
        acknowledgement_id: hot_journal.acknowledgement_id.clone(),
        recovery_plan_evidence_hash: hot_journal.recovery_plan_evidence_hash.clone(),
        diagnostic: diagnostic.clone(),
        candidate_record_ids: hot_journal.candidate_record_ids.clone(),
        before_journal_hash: hot_journal.before_journal_hash.clone(),
        after_journal_hash: hot_journal.after_journal_hash.clone(),
        archive_hash: hot_journal.archive_hash.clone(),
        hot_journal_written: true,
        archive_written: false,
        active_journal_written: false,
        hot_journal_cleared: false,
        retention_candidate_count: hot_journal.candidate_count,
        archived_record_count: hot_journal.archived_record_count,
        kept_record_count: hot_journal.kept_record_count,
        write_receipts: write_receipts.clone(),
        recovery_diagnostic: None,
    };
    append_retention_planned_event(app, session, &planned_receipt)?;

    let archive_receipt = write_project_transition_decision_retention_archive(
        app,
        session,
        &archive_path,
        &hot_journal,
    )?;
    write_receipts.push(archive_receipt);
    let active_receipt = write_project_transition_decision_journal(
        app,
        session,
        Path::new(&decision_journal.path),
        &hot_journal.after_journal_text,
        &retention_id,
        &parsed_journal.before_version_token,
        &parsed_journal.before_hash,
    )?;
    write_receipts.push(active_receipt);

    let mut status = KernelProjectTransitionDecisionRetentionStatus::Committed;
    let mut hot_journal_cleared = true;
    let mut recovery_diagnostic = None;
    let clear_result = capture_retention_file_baseline(
        &hot_journal_path,
        "ProjectTransition Decision retention hot journal clear",
    )
    .and_then(|baseline| {
        if baseline.content_hash != expected_hot_journal_hash {
            return Err(format!(
                "ProjectTransition Decision retention refuză clear: hot journal-ul {} s-a schimbat după publicare.",
                hot_journal_path.display()
            ));
        }
        clear_project_transition_decision_retention_hot_journal(
            app,
            session,
            &hot_journal_path,
            &retention_id,
            &baseline.version_token,
            &baseline.content_hash,
        )
    });
    match clear_result {
        Ok(receipt) => write_receipts.push(receipt),
        Err(error) => {
            status = KernelProjectTransitionDecisionRetentionStatus::RecoveryAttention;
            hot_journal_cleared = false;
            recovery_diagnostic = Some(format!(
                "Retention-ul a comis arhiva și jurnalul activ, dar hot journal-ul nu a putut fi curățat: {error}"
            ));
        }
    }
    Ok(KernelProjectTransitionDecisionRetentionReceipt {
        status,
        completed_at_ms: now_ms(),
        write_receipts,
        archive_written: true,
        active_journal_written: true,
        hot_journal_cleared,
        recovery_diagnostic,
        ..planned_receipt
    })
}
