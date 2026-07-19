use super::super::transition_decision::{
    KernelProjectTransitionDecisionRecord,
    KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
};
use super::{
    KernelProjectTransitionDecisionRecoveryIssue, KernelProjectTransitionDecisionRecoveryIssueKind,
    KernelProjectTransitionDecisionRecoveryIssueSeverity,
};

pub(super) fn build_integrity_issues(
    read_diagnostic_count: usize,
    invalid_records: &[&KernelProjectTransitionDecisionRecord],
    duplicate_ids: &[(String, usize)],
    duplicate_id_count: usize,
) -> Vec<KernelProjectTransitionDecisionRecoveryIssue> {
    let mut issues = Vec::new();
    if read_diagnostic_count > 0 {
        issues.push(KernelProjectTransitionDecisionRecoveryIssue {
            schema_version: KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
            kind: KernelProjectTransitionDecisionRecoveryIssueKind::ReadDiagnostic,
            severity: KernelProjectTransitionDecisionRecoveryIssueSeverity::Error,
            record_id: None,
            count: read_diagnostic_count,
            title: "Linii JSONL necitibile".to_string(),
            detail: format!(
                "{} linii din Decision Journal nu pot fi parsate ca recorduri valide.",
                read_diagnostic_count
            ),
            recommended_action:
                "Nu consuma decizii din jurnal până când operatorul inspectează liniile necitibile."
                    .to_string(),
        });
    }
    for record in invalid_records {
        issues.push(KernelProjectTransitionDecisionRecoveryIssue {
            schema_version: KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
            kind: KernelProjectTransitionDecisionRecoveryIssueKind::InvalidEvidenceHash,
            severity: KernelProjectTransitionDecisionRecoveryIssueSeverity::Error,
            record_id: Some(record.id.clone()),
            count: 1,
            title: "Evidence hash invalid".to_string(),
            detail: format!(
                "Decizia {} nu mai corespunde evidenței serializate din record.",
                record.id
            ),
            recommended_action:
                "Blochează consumul jurnalului și inspectează sursa alterării înainte de orice tranziție dependentă."
                    .to_string(),
        });
    }
    for (id, count) in duplicate_ids {
        issues.push(KernelProjectTransitionDecisionRecoveryIssue {
            schema_version: KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
            kind: KernelProjectTransitionDecisionRecoveryIssueKind::DuplicateDecisionId,
            severity: KernelProjectTransitionDecisionRecoveryIssueSeverity::Error,
            record_id: Some(id.clone()),
            count: *count,
            title: "Decision ID duplicat".to_string(),
            detail: format!("ID-ul {} apare de {} ori în jurnal.", id, count),
            recommended_action:
                "Nu alege automat unul dintre recorduri; jurnalul trebuie tratat ca neîncredere operațională."
                    .to_string(),
        });
    }
    if duplicate_id_count > 0 && duplicate_ids.is_empty() {
        issues.push(KernelProjectTransitionDecisionRecoveryIssue {
            schema_version: KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
            kind: KernelProjectTransitionDecisionRecoveryIssueKind::DuplicateDecisionId,
            severity: KernelProjectTransitionDecisionRecoveryIssueSeverity::Error,
            record_id: None,
            count: duplicate_id_count,
            title: "Decision IDs duplicate".to_string(),
            detail: format!(
                "{} duplicate au fost detectate în Decision Journal.",
                duplicate_id_count
            ),
            recommended_action: "Inspectează jurnalul înainte de orice consum de decizie operator."
                .to_string(),
        });
    }
    issues
}
