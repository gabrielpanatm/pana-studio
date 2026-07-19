use std::path::PathBuf;

use tauri::{AppHandle, Runtime};

use crate::kernel::{
    project_session::ProjectSessionSnapshot,
    write_authority::{
        WriteAuthority, WriteCategory, WriteIntent, WriteOperationKind, WriteOwner, WritePolicy,
        WriteTarget,
    },
};

use super::{
    project_transition_decision_recovery_ack_journal_path,
    KernelProjectTransitionDecisionRecoveryAckRecord,
};

pub(super) fn append_project_transition_decision_recovery_ack_journal_record<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    record: &KernelProjectTransitionDecisionRecoveryAckRecord,
) -> Result<(), String> {
    let journal_path = project_transition_decision_recovery_ack_journal_path(session);
    let target = WriteTarget::new(
        journal_path,
        PathBuf::from(&session.session_dir),
        "session/project-transition-decision-recovery-acknowledgements.jsonl",
    );
    let intent = WriteIntent::new(
        WriteCategory::InternalAppWrite,
        WriteOwner::Kernel,
        WriteOperationKind::AppendText,
        target,
        WritePolicy::internal_append(),
        "Append ProjectTransition Decision recovery acknowledgement journal",
    );
    let body = serde_json::to_string(record).map_err(|error| {
        format!(
            "Nu am putut serializa Project Transition Decision recovery acknowledgement: {error}"
        )
    })?;
    WriteAuthority::new(app)
        .append_text(intent, &(body + "\n"))
        .map_err(|error| error.into_terminal_diagnostic())
        .map(|_| ())
}
