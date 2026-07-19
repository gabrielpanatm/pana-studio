use super::{
    AiClientIdentity, AiCoordinationRuntime, EditAuthority, EditLeaseRequest, EditLeaseStatus,
    ProjectCoordinationBlocker, ProjectCoordinationBlockerKind, ProjectCoordinationEvidence,
    ReconciliationInput, ReleaseEditLeaseInput, RequiredUserAction, UiQuiescenceAcknowledgement,
    DEFAULT_EDIT_LEASE_TTL_MS,
};

const NOW: u128 = 1_000_000;
const PROJECT_SESSION: &str = "project-session-1";
const CLIENT_SESSION: &str = "mcp-client-1";

fn runtime() -> AiCoordinationRuntime {
    let runtime = AiCoordinationRuntime::default();
    runtime
        .register_client(
            AiClientIdentity {
                session_id: CLIENT_SESSION.to_string(),
                client_name: "codex".to_string(),
                client_version: Some("test".to_string()),
            },
            NOW,
        )
        .unwrap();
    runtime
        .bind_project(Some(PROJECT_SESSION.to_string()), NOW)
        .unwrap();
    runtime
}

fn request(request_id: &str, revision: u64) -> EditLeaseRequest {
    EditLeaseRequest {
        client_session_id: CLIENT_SESSION.to_string(),
        expected_project_session_id: PROJECT_SESSION.to_string(),
        expected_project_revision: revision,
        request_id: request_id.to_string(),
        intent: "Actualizez sursele proiectului".to_string(),
    }
}

fn acknowledge(
    request_id: &str,
    revision: u64,
    dirty_files: Vec<String>,
) -> UiQuiescenceAcknowledgement {
    UiQuiescenceAcknowledgement {
        request_id: request_id.to_string(),
        project_session_id: PROJECT_SESSION.to_string(),
        project_revision: revision,
        ui_revision: 7,
        ui_quiescent: true,
        blocker_reason: None,
        dirty_files,
    }
}

fn grant(runtime: &AiCoordinationRuntime, request_id: &str, revision: u64) -> String {
    let evidence = ProjectCoordinationEvidence::clean(PROJECT_SESSION, revision);
    let pending = runtime
        .request_edit_lease(request(request_id, revision), &evidence, NOW + 1)
        .unwrap();
    assert_eq!(pending.status, EditLeaseStatus::PendingUiQuiescence);
    let granted = runtime
        .acknowledge_ui_quiescence(
            CLIENT_SESSION,
            acknowledge(request_id, revision, Vec::new()),
            &evidence,
            NOW + 2,
        )
        .unwrap();
    assert_eq!(granted.status, EditLeaseStatus::Granted);
    granted.lease.unwrap().id
}

fn evidence_with_disk_changes(revision: u64, files: Vec<&str>) -> ProjectCoordinationEvidence {
    let mut evidence = ProjectCoordinationEvidence::clean(PROJECT_SESSION, revision);
    evidence.blockers.push(ProjectCoordinationBlocker {
        kind: ProjectCoordinationBlockerKind::DiskConflict,
        reason: "Discul diferă de AcceptedDisk.".to_string(),
        files: files.into_iter().map(str::to_string).collect(),
    });
    evidence
}

#[test]
fn dirty_workspace_blocks_before_ui_quiescence() {
    let runtime = runtime();
    let evidence = ProjectCoordinationEvidence::dirty(
        PROJECT_SESSION,
        3,
        vec!["sursa/templates/index.html".to_string()],
    );
    let receipt = runtime
        .request_edit_lease(request("request-dirty", 3), &evidence, NOW + 1)
        .unwrap();

    assert_eq!(receipt.status, EditLeaseStatus::Blocked);
    assert_eq!(
        receipt.required_user_action,
        Some(RequiredUserAction::SaveOrDiscard)
    );
    assert!(matches!(receipt.authority, EditAuthority::UserActive));
}

#[test]
fn clean_workspace_requires_ui_quiescence_before_grant() {
    let runtime = runtime();
    let evidence = ProjectCoordinationEvidence::clean(PROJECT_SESSION, 4);
    let pending = runtime
        .request_edit_lease(request("request-clean", 4), &evidence, NOW + 1)
        .unwrap();
    assert_eq!(pending.status, EditLeaseStatus::PendingUiQuiescence);
    assert!(matches!(
        pending.authority,
        EditAuthority::AiRequested { .. }
    ));

    let granted = runtime
        .acknowledge_ui_quiescence(
            CLIENT_SESSION,
            acknowledge("request-clean", 4, Vec::new()),
            &evidence,
            NOW + 2,
        )
        .unwrap();
    assert_eq!(granted.status, EditLeaseStatus::Granted);
    assert!(matches!(granted.authority, EditAuthority::AiActive { .. }));
}

#[test]
fn frontend_dirty_ack_returns_authority_to_user() {
    let runtime = runtime();
    let evidence = ProjectCoordinationEvidence::clean(PROJECT_SESSION, 5);
    runtime
        .request_edit_lease(request("request-race", 5), &evidence, NOW + 1)
        .unwrap();
    let receipt = runtime
        .acknowledge_ui_quiescence(
            CLIENT_SESSION,
            acknowledge(
                "request-race",
                5,
                vec!["sursa/sass/pagini/index.scss".to_string()],
            ),
            &evidence,
            NOW + 2,
        )
        .unwrap();

    assert_eq!(receipt.status, EditLeaseStatus::Blocked);
    assert!(matches!(receipt.authority, EditAuthority::UserActive));
}

#[test]
fn repeated_request_id_is_idempotent() {
    let runtime = runtime();
    let evidence = ProjectCoordinationEvidence::clean(PROJECT_SESSION, 6);
    let first = runtime
        .request_edit_lease(request("request-repeat", 6), &evidence, NOW + 1)
        .unwrap();
    let second = runtime
        .request_edit_lease(request("request-repeat", 6), &evidence, NOW + 2)
        .unwrap();

    assert_eq!(first.status, EditLeaseStatus::PendingUiQuiescence);
    assert_eq!(second.status, EditLeaseStatus::PendingUiQuiescence);
    assert_eq!(first.coordination_revision, second.coordination_revision);
}

#[test]
fn another_ai_cannot_take_an_active_lease() {
    let runtime = runtime();
    grant(&runtime, "request-owner", 7);
    runtime
        .register_client(
            AiClientIdentity {
                session_id: "mcp-client-2".to_string(),
                client_name: "second-agent".to_string(),
                client_version: None,
            },
            NOW + 3,
        )
        .unwrap();
    let receipt = runtime
        .request_edit_lease(
            EditLeaseRequest {
                client_session_id: "mcp-client-2".to_string(),
                expected_project_session_id: PROJECT_SESSION.to_string(),
                expected_project_revision: 7,
                request_id: "request-second".to_string(),
                intent: "Încerc o editare concurentă".to_string(),
            },
            &ProjectCoordinationEvidence::clean(PROJECT_SESSION, 7),
            NOW + 4,
        )
        .unwrap();

    assert_eq!(receipt.status, EditLeaseStatus::Busy);
    assert_eq!(
        receipt.required_user_action,
        Some(RequiredUserAction::WaitForAi)
    );
}

#[test]
fn release_requires_reconciliation_before_user_control() {
    let runtime = runtime();
    let lease_id = grant(&runtime, "request-release", 8);
    let released = runtime
        .release_edit_lease(
            ReleaseEditLeaseInput {
                client_session_id: CLIENT_SESSION.to_string(),
                lease_id: lease_id.clone(),
                expected_changed_files: vec!["sursa/templates/index.html".to_string()],
                summary: Some("Titlu actualizat".to_string()),
            },
            &evidence_with_disk_changes(8, vec!["sursa/templates/index.html"]),
            NOW + 3,
        )
        .unwrap();
    assert_eq!(released.status, EditLeaseStatus::Reconciling);

    let completed = runtime
        .complete_reconciliation(
            ReconciliationInput {
                lease_id,
                project_session_id: PROJECT_SESSION.to_string(),
                project_revision: 9,
                observed_changed_files: vec!["sursa/templates/index.html".to_string()],
                conflict_files: Vec::new(),
            },
            NOW + 4,
        )
        .unwrap();
    assert_eq!(completed.status, EditLeaseStatus::ReleasedToUser);
    assert!(matches!(completed.authority, EditAuthority::UserActive));
}

#[test]
fn reconciliation_holds_conflict_when_observed_files_differ_from_ai_declaration() {
    let runtime = runtime();
    let lease_id = grant(&runtime, "request-causality", 18);
    runtime
        .release_edit_lease(
            ReleaseEditLeaseInput {
                client_session_id: CLIENT_SESSION.to_string(),
                lease_id: lease_id.clone(),
                expected_changed_files: vec!["sursa/templates/index.html".to_string()],
                summary: None,
            },
            &evidence_with_disk_changes(18, vec!["sursa/templates/index.html"]),
            NOW + 3,
        )
        .unwrap();

    let receipt = runtime
        .complete_reconciliation(
            ReconciliationInput {
                lease_id,
                project_session_id: PROJECT_SESSION.to_string(),
                project_revision: 19,
                observed_changed_files: vec!["sursa/sass/site.scss".to_string()],
                conflict_files: Vec::new(),
            },
            NOW + 4,
        )
        .unwrap();

    assert_eq!(receipt.status, EditLeaseStatus::Conflict);
    assert_eq!(
        receipt.dirty_files,
        vec![
            "sursa/sass/site.scss".to_string(),
            "sursa/templates/index.html".to_string()
        ]
    );
    assert!(matches!(receipt.authority, EditAuthority::Conflict { .. }));
}

#[test]
fn release_detects_unknown_disk_writer_and_user_can_accept_reconciliation() {
    let runtime = runtime();
    let lease_id = grant(&runtime, "request-release-mismatch", 20);
    let conflict = runtime
        .release_edit_lease(
            ReleaseEditLeaseInput {
                client_session_id: CLIENT_SESSION.to_string(),
                lease_id: lease_id.clone(),
                expected_changed_files: vec!["sursa/templates/index.html".to_string()],
                summary: None,
            },
            &evidence_with_disk_changes(20, vec!["sursa/sass/site.scss"]),
            NOW + 3,
        )
        .unwrap();
    assert_eq!(conflict.status, EditLeaseStatus::Conflict);

    let reviewed = runtime
        .accept_conflict_for_reconciliation(PROJECT_SESSION, 20, NOW + 4)
        .unwrap();
    assert_eq!(reviewed.status, EditLeaseStatus::Reconciling);
    let recovery_lease_id = match reviewed.authority {
        EditAuthority::Reconciling { lease_id, .. } => lease_id,
        authority => panic!("expected reviewed reconciliation, got {authority:?}"),
    };

    let completed = runtime
        .complete_reconciliation(
            ReconciliationInput {
                lease_id: recovery_lease_id,
                project_session_id: PROJECT_SESSION.to_string(),
                project_revision: 21,
                observed_changed_files: Vec::new(),
                conflict_files: Vec::new(),
            },
            NOW + 5,
        )
        .unwrap();
    assert_eq!(completed.status, EditLeaseStatus::ReleasedToUser);
}

#[test]
fn expired_lease_enters_orphaned_barrier_and_same_session_rebind_cannot_unlock() {
    let runtime = runtime();
    grant(&runtime, "request-expire", 10);
    let receipt = runtime
        .expire(NOW + 2 + DEFAULT_EDIT_LEASE_TTL_MS)
        .unwrap()
        .unwrap();

    assert_eq!(receipt.status, EditLeaseStatus::Orphaned);
    assert!(matches!(
        receipt.authority,
        EditAuthority::AiOrphaned { .. }
    ));
    assert!(runtime.require_user_source_mutation().is_err());
    assert!(runtime.require_project_transition().is_err());
    assert!(runtime.require_external_reconciliation().is_ok());

    runtime
        .bind_project(
            Some(PROJECT_SESSION.to_string()),
            NOW + 3 + DEFAULT_EDIT_LEASE_TTL_MS,
        )
        .unwrap();
    assert!(matches!(
        runtime
            .snapshot(NOW + 3 + DEFAULT_EDIT_LEASE_TTL_MS)
            .unwrap()
            .authority,
        EditAuthority::AiOrphaned { .. }
    ));
}

#[test]
fn expired_lease_cannot_be_revived_by_renewal_without_a_frontend_poll() {
    let runtime = runtime();
    let lease_id = grant(&runtime, "request-expired-renew", 16);
    let receipt = runtime
        .renew_edit_lease(
            CLIENT_SESSION,
            &lease_id,
            &ProjectCoordinationEvidence::clean(PROJECT_SESSION, 16),
            NOW + 2 + DEFAULT_EDIT_LEASE_TTL_MS,
        )
        .unwrap();

    assert_eq!(receipt.status, EditLeaseStatus::Stale);
    assert!(matches!(
        receipt.authority,
        EditAuthority::AiOrphaned { .. }
    ));
}

#[test]
fn orphaned_lease_adopts_the_current_stable_disk_only_after_explicit_recovery() {
    let runtime = runtime();
    let lease_id = grant(&runtime, "request-orphaned-recovery", 18);
    runtime.expire(NOW + 2 + DEFAULT_EDIT_LEASE_TTL_MS).unwrap();
    let evidence = evidence_with_disk_changes(
        18,
        vec![
            "sursa/content/portofoliu.md",
            "sursa/templates/portofoliu.html",
        ],
    );

    let receipt = runtime
        .authorize_reconciliation_recovery_reload(&evidence, NOW + 3 + DEFAULT_EDIT_LEASE_TTL_MS)
        .unwrap();

    assert_eq!(receipt.status, EditLeaseStatus::Reconciling);
    assert!(matches!(
        receipt.authority,
        EditAuthority::Reconciling {
            lease_id: ref recovered_lease,
            recovery_reload_authorized: true,
            declaration_reviewed_by_user: true,
            ref expected_changed_files,
            ref observed_changed_files,
            ..
        } if recovered_lease == &lease_id
            && expected_changed_files == &vec![
                "sursa/content/portofoliu.md".to_string(),
                "sursa/templates/portofoliu.html".to_string(),
            ]
            && observed_changed_files == expected_changed_files
    ));
    assert!(runtime.require_user_source_mutation().is_err());
    assert!(runtime.require_project_transition().is_ok());
}

#[test]
fn renewal_advances_coordination_revision_and_ttl() {
    let runtime = runtime();
    let lease_id = grant(&runtime, "request-renew-revision", 17);
    let before = runtime.snapshot(NOW + 2).unwrap();
    let before_expiry = match before.authority {
        EditAuthority::AiActive { lease } => lease.expires_at_ms,
        authority => panic!("expected active lease, got {authority:?}"),
    };

    let receipt = runtime
        .renew_edit_lease(
            CLIENT_SESSION,
            &lease_id,
            &ProjectCoordinationEvidence::clean(PROJECT_SESSION, 17),
            NOW + 10,
        )
        .unwrap();

    assert_eq!(receipt.status, EditLeaseStatus::Granted);
    assert!(receipt.coordination_revision > before.coordination_revision);
    assert!(receipt.lease.unwrap().expires_at_ms > before_expiry);
}

#[test]
fn project_transition_cannot_revoke_an_active_ai_lease() {
    let runtime = runtime();
    grant(&runtime, "request-project-change", 11);
    let error = runtime
        .bind_project(Some("project-session-2".to_string()), NOW + 3)
        .unwrap_err();

    assert!(error.diagnostic.contains("nu poate fi schimbat"));
    let snapshot = runtime.snapshot(NOW + 3).unwrap();
    assert_eq!(
        snapshot.project_session_id.as_deref(),
        Some(PROJECT_SESSION)
    );
    assert!(matches!(snapshot.authority, EditAuthority::AiActive { .. }));
}

#[test]
fn user_mutation_gate_is_closed_during_ai_lease() {
    let runtime = runtime();
    grant(&runtime, "request-user-gate", 12);

    let error = runtime.require_user_source_mutation().unwrap_err();
    assert!(error.diagnostic.contains("aparține temporar sesiunii AI"));
}

#[test]
fn pending_request_allows_existing_user_mutations_to_drain() {
    let runtime = runtime();
    let evidence = ProjectCoordinationEvidence::clean(PROJECT_SESSION, 15);
    runtime
        .request_edit_lease(request("request-drain", 15), &evidence, NOW + 1)
        .unwrap();

    assert!(runtime.require_user_source_mutation().is_ok());
    assert!(runtime.require_project_transition().is_err());
}

#[test]
fn recovery_blocker_prevents_lease_negotiation() {
    let runtime = runtime();
    let mut evidence = ProjectCoordinationEvidence::clean(PROJECT_SESSION, 13);
    evidence.blockers.push(ProjectCoordinationBlocker {
        kind: ProjectCoordinationBlockerKind::RecoveryNeedsAttention,
        reason: "RecoveryCoordinator cere intervenție.".to_string(),
        files: Vec::new(),
    });

    let receipt = runtime
        .request_edit_lease(request("request-recovery", 13), &evidence, NOW + 1)
        .unwrap();

    assert_eq!(receipt.status, EditLeaseStatus::Blocked);
    assert_eq!(
        receipt.required_user_action,
        Some(RequiredUserAction::ResolveConflict)
    );
    assert!(matches!(receipt.authority, EditAuthority::UserActive));
}

#[test]
fn external_reconciliation_is_only_allowed_outside_ai_write_window() {
    let runtime = runtime();
    let lease_id = grant(&runtime, "request-external-reconcile", 14);
    assert!(runtime.require_external_reconciliation().is_err());

    runtime
        .release_edit_lease(
            ReleaseEditLeaseInput {
                client_session_id: CLIENT_SESSION.to_string(),
                lease_id,
                expected_changed_files: Vec::new(),
                summary: None,
            },
            &ProjectCoordinationEvidence::clean(PROJECT_SESSION, 14),
            NOW + 3,
        )
        .unwrap();

    assert!(runtime.require_external_reconciliation().is_ok());
    assert!(runtime.require_project_transition().is_err());
}

#[test]
fn projection_recovery_authorizes_only_the_guarded_project_transition() {
    let runtime = runtime();
    let lease_id = grant(&runtime, "request-projection-recovery", 16);
    let evidence = ProjectCoordinationEvidence::clean(PROJECT_SESSION, 17);
    runtime
        .release_edit_lease(
            ReleaseEditLeaseInput {
                client_session_id: CLIENT_SESSION.to_string(),
                lease_id: lease_id.clone(),
                expected_changed_files: Vec::new(),
                summary: None,
            },
            &evidence,
            NOW + 3,
        )
        .unwrap();

    assert!(runtime.require_project_transition().is_err());
    let receipt = runtime
        .authorize_reconciliation_recovery_reload(&evidence, NOW + 4)
        .unwrap();
    assert_eq!(receipt.status, EditLeaseStatus::Reconciling);
    assert!(matches!(
        receipt.authority,
        EditAuthority::Reconciling {
            recovery_reload_authorized: true,
            ..
        }
    ));
    assert!(runtime.require_project_transition().is_ok());
    assert!(runtime.require_user_source_mutation().is_err());

    runtime
        .bind_project(Some("replacement-session".to_string()), NOW + 5)
        .unwrap();
    let snapshot = runtime.snapshot(NOW + 5).unwrap();
    assert_eq!(
        snapshot.project_session_id.as_deref(),
        Some("replacement-session")
    );
    assert!(matches!(
        snapshot.authority,
        EditAuthority::Reconciling {
            recovery_reload_authorized: true,
            recovery_reload_replacement_session_id: Some(ref replacement),
            ref project_session_id,
            ..
        } if replacement == "replacement-session"
            && project_session_id == "replacement-session"
    ));
    assert!(runtime.require_user_source_mutation().is_err());

    let stale = runtime
        .complete_reconciliation_recovery_reload(
            &lease_id,
            "foreign-session",
            &ProjectCoordinationEvidence::clean("replacement-session", 0),
            NOW + 6,
        )
        .unwrap();
    assert_eq!(stale.status, EditLeaseStatus::Stale);
    assert!(runtime.require_user_source_mutation().is_err());

    let completed = runtime
        .complete_reconciliation_recovery_reload(
            &lease_id,
            "replacement-session",
            &ProjectCoordinationEvidence::clean("replacement-session", 0),
            NOW + 7,
        )
        .unwrap();
    assert_eq!(completed.status, EditLeaseStatus::ReleasedToUser);
    assert!(matches!(completed.authority, EditAuthority::UserActive));
    assert!(runtime.require_user_source_mutation().is_ok());
}

#[test]
fn projection_recovery_reload_is_refused_for_dirty_workspace() {
    let runtime = runtime();
    let lease_id = grant(&runtime, "request-dirty-projection-recovery", 18);
    runtime
        .release_edit_lease(
            ReleaseEditLeaseInput {
                client_session_id: CLIENT_SESSION.to_string(),
                lease_id,
                expected_changed_files: Vec::new(),
                summary: None,
            },
            &ProjectCoordinationEvidence::clean(PROJECT_SESSION, 18),
            NOW + 3,
        )
        .unwrap();

    let receipt = runtime
        .authorize_reconciliation_recovery_reload(
            &ProjectCoordinationEvidence::dirty(
                PROJECT_SESSION,
                19,
                vec!["sursa/templates/index.html".to_string()],
            ),
            NOW + 4,
        )
        .unwrap();
    assert_eq!(receipt.status, EditLeaseStatus::Blocked);
    assert_eq!(
        receipt.required_user_action,
        Some(RequiredUserAction::SaveOrDiscard)
    );
    assert!(runtime.require_project_transition().is_err());
}

#[test]
fn topology_reload_is_authorized_when_disk_still_matches_the_ai_manifest() {
    let runtime = runtime();
    let lease_id = grant(&runtime, "request-topology-reload", 20);
    let evidence = evidence_with_disk_changes(20, vec!["sursa/content/servicii.md"]);
    runtime
        .release_edit_lease(
            ReleaseEditLeaseInput {
                client_session_id: CLIENT_SESSION.to_string(),
                lease_id,
                expected_changed_files: vec!["sursa/content/servicii.md".to_string()],
                summary: None,
            },
            &evidence,
            NOW + 3,
        )
        .unwrap();

    let receipt = runtime
        .authorize_reconciliation_recovery_reload(&evidence, NOW + 4)
        .unwrap();
    assert_eq!(receipt.status, EditLeaseStatus::Reconciling);
    assert!(matches!(
        receipt.authority,
        EditAuthority::Reconciling {
            recovery_reload_authorized: true,
            ..
        }
    ));
    assert!(runtime.require_project_transition().is_ok());
}

#[test]
fn topology_reload_becomes_conflict_when_disk_changes_again_after_release() {
    let runtime = runtime();
    let lease_id = grant(&runtime, "request-stale-topology-reload", 21);
    runtime
        .release_edit_lease(
            ReleaseEditLeaseInput {
                client_session_id: CLIENT_SESSION.to_string(),
                lease_id,
                expected_changed_files: vec!["sursa/content/servicii.md".to_string()],
                summary: None,
            },
            &evidence_with_disk_changes(21, vec!["sursa/content/servicii.md"]),
            NOW + 3,
        )
        .unwrap();

    let receipt = runtime
        .authorize_reconciliation_recovery_reload(
            &evidence_with_disk_changes(
                21,
                vec!["sursa/content/servicii.md", "sursa/date/meniu.toml"],
            ),
            NOW + 4,
        )
        .unwrap();
    assert_eq!(receipt.status, EditLeaseStatus::Conflict);
    assert_eq!(
        receipt.required_user_action,
        Some(RequiredUserAction::ResolveConflict)
    );
    assert!(runtime.require_project_transition().is_err());
}
