use super::{fixtures::*, *};

#[test]
fn atomic_staged_operator_discards_only_verified_temp_and_preserves_original() {
    let fixture = AtomicRecoveryFixture::new("aux-atomic-discard-staged", true);
    let operation_id = "wal-aux-atomic-discard-staged";
    let payload = b"replacement payload";
    let (coordinator, plan, record) = fixture.prepare(operation_id, payload);
    let mut guard = coordinator.begin(record).unwrap();
    let temp = fixture.parent.join(plan.temp_leaf().unwrap());
    fs::write(&temp, payload).unwrap();
    guard.mark_auxiliary_durable().unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    let item = scan.items.first().expect("AtomicFile staged item");
    assert_eq!(
        item.classification,
        super::super::WriteAuthorityRecoveryClassification::StagedOnly
    );
    assert_eq!(
        item.available_resolution_actions,
        vec![WriteAuthorityRecoveryResolutionAction::DiscardStagedWrite]
    );
    let receipt = restarted
        .resolve_operator_exclusive(WriteAuthorityRecoveryResolutionInput {
            operation_id: operation_id.into(),
            expected_phase: item.phase.unwrap(),
            evidence_hash: item.evidence_hash.clone().unwrap(),
            action: WriteAuthorityRecoveryResolutionAction::DiscardStagedWrite,
        })
        .unwrap();
    assert!(!receipt.recovery_scan.blocked, "{receipt:?}");
    assert!(receipt.diagnostic.contains("target-ul original"));
    assert_eq!(fs::read(&fixture.target).unwrap(), b"baseline");
    assert!(!temp.exists());
    drop(restarted);
    fixture.cleanup();
}

#[test]
fn atomic_staged_operator_refuses_changed_target_and_preserves_temp() {
    let fixture = AtomicRecoveryFixture::new("aux-atomic-discard-stale", true);
    let operation_id = "wal-aux-atomic-discard-stale";
    let payload = b"replacement payload";
    let (coordinator, plan, record) = fixture.prepare(operation_id, payload);
    let mut guard = coordinator.begin(record).unwrap();
    let temp = fixture.parent.join(plan.temp_leaf().unwrap());
    fs::write(&temp, payload).unwrap();
    guard.mark_auxiliary_durable().unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    let item = scan.items.first().expect("AtomicFile staged item");
    let input = WriteAuthorityRecoveryResolutionInput {
        operation_id: operation_id.into(),
        expected_phase: item.phase.unwrap(),
        evidence_hash: item.evidence_hash.clone().unwrap(),
        action: WriteAuthorityRecoveryResolutionAction::DiscardStagedWrite,
    };
    fs::write(&fixture.target, b"concurrent target").unwrap();
    let error = restarted.resolve_operator_exclusive(input).unwrap_err();
    assert!(
        error.contains("stale") || error.contains("Conflict"),
        "{error}"
    );
    assert_eq!(fs::read(&fixture.target).unwrap(), b"concurrent target");
    assert_eq!(fs::read(&temp).unwrap(), payload);
    assert!(restarted.snapshot().unwrap().blocked);
    drop(restarted);
    fixture.cleanup();
}

#[test]
fn external_config_auxiliary_relocated_baseline_restores_target() {
    let fixture = AtomicRecoveryFixture::new("external-v2-aux-rollback", true);
    let payload = b"new-config";
    let backup = fixture.parent.join("target.txt.pana-studio-aux.bak");
    let (coordinator, plan, record) =
        fixture.prepare_external_config("wal-external-v2-aux", payload, &backup);
    let mut guard = coordinator.begin(record).unwrap();
    fixture.materialize_external_relocated_baseline(
        &plan,
        &mut guard,
        &backup,
        WalPhase::AuxiliaryDurable,
    );
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();

    assert!(!scan.blocked, "{scan:?}");
    assert_eq!(fs::read(&fixture.target).unwrap(), b"baseline");
    assert!(!backup.exists());
    drop(restarted);
    fixture.cleanup();
}

#[test]
fn external_config_effect_relocated_baseline_restores_target() {
    let fixture = AtomicRecoveryFixture::new("external-v2-effect-rollback", true);
    let payload = b"new-config";
    let backup = fixture.parent.join("target.txt.pana-studio-effect.bak");
    let (coordinator, plan, record) =
        fixture.prepare_external_config("wal-external-v2-effect", payload, &backup);
    let mut guard = coordinator.begin(record).unwrap();
    fixture.materialize_external_relocated_baseline(
        &plan,
        &mut guard,
        &backup,
        WalPhase::EffectVisible,
    );
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();

    assert!(!scan.blocked, "{scan:?}");
    assert_eq!(fs::read(&fixture.target).unwrap(), b"baseline");
    assert!(!backup.exists());
    drop(restarted);
    fixture.cleanup();
}

#[test]
fn external_config_effect_completed_rollback_is_restart_idempotent() {
    let fixture = AtomicRecoveryFixture::new("external-v2-rollback-complete", true);
    let payload = b"new-config";
    let backup = fixture
        .parent
        .join("target.txt.pana-studio-rollback-complete.bak");
    let (coordinator, _plan, record) =
        fixture.prepare_external_config("wal-external-v2-rollback-complete", payload, &backup);
    let mut guard = coordinator.begin(record).unwrap();
    guard
        .mark_external_auxiliary_durable(
            super::super::WalExternalStageCheckpoint::new("a".repeat(32), None).unwrap(),
        )
        .unwrap();
    guard.mark_effect_visible().unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();

    assert!(!scan.blocked, "{scan:?}");
    assert_eq!(fs::read(&fixture.target).unwrap(), b"baseline");
    assert!(!backup.exists());
    drop(restarted);
    fixture.cleanup();
}

#[test]
fn external_config_crash_after_rollback_rename_finalizes_restored_baseline() {
    let fixture = AtomicRecoveryFixture::new("external-v2-rollback-rename-crash", true);
    let payload = b"new-config";
    let backup = fixture
        .parent
        .join("target.txt.pana-studio-rollback-crash.bak");
    let (coordinator, _plan, record) =
        fixture.prepare_external_config("wal-external-v2-rollback-rename-crash", payload, &backup);
    let mut guard = coordinator.begin(record).unwrap();
    guard
        .mark_external_auxiliary_durable(
            super::super::WalExternalStageCheckpoint::new("a".repeat(32), None).unwrap(),
        )
        .unwrap();
    fs::rename(&fixture.target, &backup).unwrap();
    guard.mark_effect_visible().unwrap();
    fs::rename(&backup, &fixture.target).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(!scan.blocked, "{scan:?}");
    assert_eq!(fs::read(&fixture.target).unwrap(), b"baseline");
    assert!(!backup.exists());
    drop(restarted);

    let restarted_again = fixture.restart_coordinator();
    assert!(!restarted_again.snapshot().unwrap().blocked);
    drop(restarted_again);
    fixture.cleanup();
}

#[test]
fn directory_v3_operator_accepts_only_the_bound_current_empty_directory() {
    let fixture = AtomicRecoveryFixture::new("mkdir-v3-accept-current", false);
    let target = fixture.parent.join("created");
    let operation_id = "wal-mkdir-v3-accept-current";
    let (coordinator, _intent, _plan, record) = fixture.prepare_directory_v2(operation_id, &target);
    let guard = coordinator.begin(record).unwrap();
    fs::create_dir(&target).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    let item = scan.items.first().expect("Directory current-state item");
    assert_eq!(
        item.classification,
        super::super::WriteAuthorityRecoveryClassification::PartialNamespaceCreation
    );
    assert!(!item.automatic_recovery_available);
    assert_eq!(
        item.available_resolution_actions,
        vec![WriteAuthorityRecoveryResolutionAction::AcceptCurrentState]
    );
    let before = fs::metadata(&target).unwrap();
    let receipt = restarted
        .resolve_operator_exclusive(WriteAuthorityRecoveryResolutionInput {
            operation_id: operation_id.into(),
            expected_phase: item.phase.unwrap(),
            evidence_hash: item.evidence_hash.clone().unwrap(),
            action: WriteAuthorityRecoveryResolutionAction::AcceptCurrentState,
        })
        .unwrap();
    assert_eq!(
        receipt.schema_version,
        WRITE_AUTHORITY_RECOVERY_RESOLUTION_SCHEMA_VERSION
    );
    assert_eq!(
        receipt.action,
        WriteAuthorityRecoveryResolutionAction::AcceptCurrentState
    );
    assert!(!receipt.recovery_scan.blocked, "{receipt:?}");
    assert!(receipt.diagnostic.contains("lifetime+state"));
    let after = fs::metadata(&target).unwrap();
    assert_eq!((after.dev(), after.ino()), (before.dev(), before.ino()));
    assert_eq!(after.permissions().mode() & 0o7777, 0o755);
    assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
    drop(restarted);
    fixture.cleanup();
}

#[test]
fn directory_v3_accept_current_state_token_rejects_empty_directory_replacement() {
    let fixture = AtomicRecoveryFixture::new("mkdir-v3-current-stale-replace", false);
    let target = fixture.parent.join("created");
    let displaced = fixture.parent.join("created-before-scan");
    let operation_id = "wal-mkdir-v3-current-stale-replace";
    let (coordinator, _intent, _plan, record) = fixture.prepare_directory_v2(operation_id, &target);
    let guard = coordinator.begin(record).unwrap();
    fs::create_dir(&target).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    let item = scan.items.first().expect("Directory current-state item");
    let input = WriteAuthorityRecoveryResolutionInput {
        operation_id: operation_id.into(),
        expected_phase: item.phase.unwrap(),
        evidence_hash: item.evidence_hash.clone().unwrap(),
        action: WriteAuthorityRecoveryResolutionAction::AcceptCurrentState,
    };
    let original = fs::metadata(&target).unwrap();
    fs::rename(&target, &displaced).unwrap();
    fs::create_dir(&target).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).unwrap();
    let replacement = fs::metadata(&target).unwrap();
    assert_ne!(
        (replacement.dev(), replacement.ino()),
        (original.dev(), original.ino())
    );

    let error = restarted.resolve_operator_exclusive(input).unwrap_err();
    assert!(error.contains("evidence hash stale"), "{error}");
    assert!(restarted.snapshot().unwrap().blocked);
    assert!(target.is_dir());
    assert!(displaced.is_dir());
    drop(restarted);
    fixture.cleanup();
}

#[test]
fn directory_v3_accept_current_state_token_rejects_add_remove_state_change() {
    let fixture = AtomicRecoveryFixture::new("mkdir-v3-current-stale-state", false);
    let target = fixture.parent.join("created");
    let child = target.join("foreign");
    let operation_id = "wal-mkdir-v3-current-stale-state";
    let (coordinator, _intent, _plan, record) = fixture.prepare_directory_v2(operation_id, &target);
    let guard = coordinator.begin(record).unwrap();
    fs::create_dir(&target).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    let item = scan.items.first().expect("Directory current-state item");
    let input = WriteAuthorityRecoveryResolutionInput {
        operation_id: operation_id.into(),
        expected_phase: item.phase.unwrap(),
        evidence_hash: item.evidence_hash.clone().unwrap(),
        action: WriteAuthorityRecoveryResolutionAction::AcceptCurrentState,
    };
    fs::write(&child, b"foreign").unwrap();
    fs::remove_file(&child).unwrap();
    OpenOptions::new()
        .read(true)
        .open(&target)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(1)))
        .unwrap();

    let error = restarted.resolve_operator_exclusive(input).unwrap_err();
    assert!(error.contains("evidence hash stale"), "{error}");
    assert!(restarted.snapshot().unwrap().blocked);
    assert!(target.is_dir());
    assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
    drop(restarted);
    fixture.cleanup();
}

#[test]
fn directory_v3_accept_current_state_fresh_recapture_rejects_internal_replacement() {
    let fixture = AtomicRecoveryFixture::new("mkdir-v3-current-fresh-race", false);
    let target = fixture.parent.join("created");
    let displaced = fixture.parent.join("created-before-fresh-capture");
    let operation_id = "wal-mkdir-v3-current-fresh-race";
    let (coordinator, _intent, _plan, record) = fixture.prepare_directory_v2(operation_id, &target);
    let guard = coordinator.begin(record).unwrap();
    fs::create_dir(&target).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    let item = scan.items.first().expect("Directory current-state item");
    let input = WriteAuthorityRecoveryResolutionInput {
        operation_id: operation_id.into(),
        expected_phase: item.phase.unwrap(),
        evidence_hash: item.evidence_hash.clone().unwrap(),
        action: WriteAuthorityRecoveryResolutionAction::AcceptCurrentState,
    };
    let hook_target = target.clone();
    let hook_displaced = displaced.clone();
    let error = capability::with_before_directory_current_state_fresh_capture_hook_for_test(
        move || {
            fs::rename(&hook_target, &hook_displaced).unwrap();
            fs::create_dir(&hook_target).unwrap();
            fs::set_permissions(&hook_target, fs::Permissions::from_mode(0o755)).unwrap();
        },
        || restarted.resolve_operator_exclusive(input),
    )
    .unwrap_err();
    assert!(error.contains("fresh lifetime/state"), "{error}");
    assert!(restarted.snapshot().unwrap().blocked);
    assert!(target.is_dir());
    assert!(displaced.is_dir());
    drop(restarted);
    fixture.cleanup();
}

#[test]
fn directory_v3_accept_current_state_rejects_unsafe_target_shapes() {
    for shape in ["file", "symlink", "nonempty", "wrong-mode"] {
        let fixture =
            AtomicRecoveryFixture::new(&format!("mkdir-v3-current-unsafe-{shape}"), false);
        let target = fixture.parent.join("created");
        let operation_id = format!("wal-mkdir-v3-current-unsafe-{shape}");
        let (coordinator, _intent, _plan, record) =
            fixture.prepare_directory_v2(&operation_id, &target);
        let guard = coordinator.begin(record).unwrap();
        match shape {
            "file" => fs::write(&target, b"not a directory").unwrap(),
            "symlink" => symlink(&fixture.parent, &target).unwrap(),
            "nonempty" => {
                fs::create_dir(&target).unwrap();
                fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).unwrap();
                fs::write(target.join("foreign"), b"foreign").unwrap();
            }
            "wrong-mode" => {
                fs::create_dir(&target).unwrap();
                fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
            }
            _ => unreachable!(),
        }
        drop(guard);
        drop(coordinator);

        let restarted = fixture.restart_coordinator();
        let scan = restarted.snapshot().unwrap();
        let item = scan.items.first().expect("Directory unsafe item");
        assert_eq!(
            item.classification,
            super::super::WriteAuthorityRecoveryClassification::Conflict,
            "{shape}: {item:?}"
        );
        assert!(
            item.available_resolution_actions.is_empty(),
            "{shape}: {item:?}"
        );
        let error = restarted
            .resolve_operator_exclusive(WriteAuthorityRecoveryResolutionInput {
                operation_id: operation_id.clone(),
                expected_phase: item.phase.unwrap(),
                evidence_hash: item.evidence_hash.clone().unwrap(),
                action: WriteAuthorityRecoveryResolutionAction::AcceptCurrentState,
            })
            .unwrap_err();
        assert!(
            error.contains("director real, stabil, gol"),
            "{shape}: {error}"
        );
        assert!(restarted.snapshot().unwrap().blocked, "{shape}");
        drop(restarted);
        fixture.cleanup();
    }
}

#[test]
fn directory_v3_accept_current_state_serialization_contract() {
    assert_eq!(
        serde_json::to_string(&WriteAuthorityRecoveryResolutionAction::AcceptCurrentState).unwrap(),
        r#""accept_current_state""#
    );
    let decoded: WriteAuthorityRecoveryResolutionAction =
        serde_json::from_str(r#""accept_current_state""#).unwrap();
    assert_eq!(
        decoded,
        WriteAuthorityRecoveryResolutionAction::AcceptCurrentState
    );
    assert_eq!(WRITE_AUTHORITY_RECOVERY_RESOLUTION_SCHEMA_VERSION, 7);
}

#[test]
fn directory_v2_prepared_direct_target_removed_stays_hot_until_operator_accepts() {
    let fixture = AtomicRecoveryFixture::new("mkdir-v2-direct-removed", false);
    let target = fixture.parent.join("created");
    let (coordinator, intent, plan, record) =
        fixture.prepare_directory_v2("wal-mkdir-v2-direct-removed", &target);
    let mut guard = coordinator.begin(record).unwrap();
    let hook_target = target.clone();
    let crashed = catch_unwind(AssertUnwindSafe(|| {
        capability::with_after_directory_create_before_phase_hook_for_test(
            move || {
                fs::remove_dir(&hook_target).unwrap();
                panic!("simulated crash after direct target disappeared");
            },
            || capability::create_directory_all_wal(&intent.target, &plan, &mut guard),
        )
    }));
    assert!(crashed.is_err());
    assert!(!target.exists());
    assert_eq!(guard.phase(), WalPhase::Prepared);
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::RollbackCompleted
    );
    assert!(!scan.items[0].automatic_recovery_available);
    assert_eq!(
        scan.items[0].available_resolution_actions,
        vec![WriteAuthorityRecoveryResolutionAction::AcceptRestoredState]
    );
    drop(restarted);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    let item = scan.items.first().expect("Directory operator item");
    let receipt = restarted
        .resolve_operator_exclusive(WriteAuthorityRecoveryResolutionInput {
            operation_id: item.operation_id.clone().unwrap(),
            expected_phase: item.phase.unwrap(),
            evidence_hash: item.evidence_hash.clone().unwrap(),
            action: WriteAuthorityRecoveryResolutionAction::AcceptRestoredState,
        })
        .unwrap();
    assert!(!receipt.recovery_scan.blocked, "{receipt:?}");
    assert!(!target.exists());
    drop(restarted);

    let restarted_again = fixture.restart_coordinator();
    assert!(!restarted_again.snapshot().unwrap().blocked);
    assert!(!target.exists());
    drop(restarted_again);
    fixture.cleanup();
}

#[test]
fn directory_v2_operator_rejects_stale_evidence_hash_and_keeps_wal_hot() {
    let fixture = AtomicRecoveryFixture::new("mkdir-v2-operator-stale-hash", false);
    let target = fixture.parent.join("created");
    let operation_id = "wal-mkdir-v2-operator-stale-hash";
    let (coordinator, _intent, _plan, record) = fixture.prepare_directory_v2(operation_id, &target);
    drop(coordinator.begin(record).unwrap());
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    let item = scan.items.first().expect("Directory operator item");
    assert_eq!(
        item.classification,
        super::super::WriteAuthorityRecoveryClassification::RollbackCompleted
    );
    let error = restarted
        .resolve_operator_exclusive(WriteAuthorityRecoveryResolutionInput {
            operation_id: operation_id.into(),
            expected_phase: item.phase.unwrap(),
            evidence_hash: "00".repeat(32),
            action: WriteAuthorityRecoveryResolutionAction::AcceptRestoredState,
        })
        .unwrap_err();
    assert!(error.contains("evidence hash stale"), "{error}");
    assert!(restarted.snapshot().unwrap().blocked);
    assert!(!target.exists());
    drop(restarted);
    fixture.cleanup();
}

#[test]
fn directory_v2_operator_rejects_target_reappeared_after_scan() {
    let fixture = AtomicRecoveryFixture::new("mkdir-v2-operator-target-reappeared", false);
    let target = fixture.parent.join("created");
    let operation_id = "wal-mkdir-v2-operator-target-reappeared";
    let (coordinator, _intent, _plan, record) = fixture.prepare_directory_v2(operation_id, &target);
    drop(coordinator.begin(record).unwrap());
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    let item = scan.items.first().expect("Directory operator item");
    let input = WriteAuthorityRecoveryResolutionInput {
        operation_id: operation_id.into(),
        expected_phase: item.phase.unwrap(),
        evidence_hash: item.evidence_hash.clone().unwrap(),
        action: WriteAuthorityRecoveryResolutionAction::AcceptRestoredState,
    };
    fs::create_dir(&target).unwrap();
    let error = restarted.resolve_operator_exclusive(input).unwrap_err();
    assert!(
        error.contains("nu mai poate accepta") || error.contains("reapărut"),
        "{error}"
    );
    assert!(restarted.snapshot().unwrap().blocked);
    assert!(target.is_dir());
    drop(restarted);
    fixture.cleanup();
}

#[test]
fn symlink_v2_crash_after_create_before_checkpoint_is_operator_only() {
    let fixture = AtomicRecoveryFixture::new("symlink-v2-precheckpoint", false);
    let target = fixture.parent.join("link");
    let source = PathBuf::from("desired");
    let (coordinator, intent, plan, record) =
        fixture.prepare_symlink_v2("wal-symlink-v2-precheckpoint", &target, &source, true);
    let mut guard = coordinator.begin(record).unwrap();
    let crashed = catch_unwind(AssertUnwindSafe(|| {
        capability::with_after_symlink_create_before_phase_hook_for_test(
            || panic!("simulated crash before Symlink v2 checkpoint"),
            || capability::symlink_entry_wal(&intent.target, &source, &plan, &mut guard),
        )
    }));
    assert!(crashed.is_err());
    assert_eq!(guard.phase(), WalPhase::Prepared);
    drop(guard);
    drop(coordinator);

    for _ in 0..2 {
        let restarted = fixture.restart_coordinator();
        let scan = restarted.snapshot().unwrap();
        assert!(scan.blocked, "{scan:?}");
        assert_eq!(
            scan.items[0].classification,
            super::super::WriteAuthorityRecoveryClassification::PartialNamespaceCreation
        );
        assert_eq!(
            scan.items[0].available_resolution_actions,
            vec![WriteAuthorityRecoveryResolutionAction::AcceptCurrentState]
        );
        assert_eq!(fs::read_link(&target).unwrap(), source);
        drop(restarted);
    }
    fixture.cleanup();
}

#[test]
fn symlink_v2_removed_precheckpoint_requires_accept_restored_state() {
    let fixture = AtomicRecoveryFixture::new("symlink-v2-removed-precheckpoint", false);
    let target = fixture.parent.join("link");
    let source = PathBuf::from("desired");
    let operation_id = "wal-symlink-v2-removed-precheckpoint";
    let (coordinator, intent, plan, record) =
        fixture.prepare_symlink_v2(operation_id, &target, &source, true);
    let mut guard = coordinator.begin(record).unwrap();
    let hook_target = target.clone();
    let crashed = catch_unwind(AssertUnwindSafe(|| {
        capability::with_after_symlink_create_before_phase_hook_for_test(
            move || {
                fs::remove_file(&hook_target).unwrap();
                panic!("simulated crash after Symlink v2 disappeared");
            },
            || capability::symlink_entry_wal(&intent.target, &source, &plan, &mut guard),
        )
    }));
    assert!(crashed.is_err());
    assert!(!target.exists());
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    let item = scan.items.first().unwrap();
    assert_eq!(
        item.classification,
        super::super::WriteAuthorityRecoveryClassification::RollbackCompleted
    );
    assert_eq!(
        item.available_resolution_actions,
        vec![WriteAuthorityRecoveryResolutionAction::AcceptRestoredState]
    );
    let receipt = restarted
        .resolve_operator_exclusive(WriteAuthorityRecoveryResolutionInput {
            operation_id: operation_id.into(),
            expected_phase: item.phase.unwrap(),
            evidence_hash: item.evidence_hash.clone().unwrap(),
            action: WriteAuthorityRecoveryResolutionAction::AcceptRestoredState,
        })
        .unwrap();
    assert!(!receipt.recovery_scan.blocked, "{receipt:?}");
    assert!(!target.exists());
    fixture.cleanup();
}

#[test]
fn symlink_v2_accept_restored_rejects_target_that_appears_after_scan() {
    let fixture = AtomicRecoveryFixture::new("symlink-v2-restored-stale", false);
    let target = fixture.parent.join("link");
    let source = PathBuf::from("desired");
    let operation_id = "wal-symlink-v2-restored-stale";
    let (coordinator, _intent, _plan, record) =
        fixture.prepare_symlink_v2(operation_id, &target, &source, true);
    drop(coordinator.begin(record).unwrap());
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    let item = scan.items.first().unwrap();
    assert_eq!(
        item.classification,
        super::super::WriteAuthorityRecoveryClassification::RollbackCompleted
    );
    let input = WriteAuthorityRecoveryResolutionInput {
        operation_id: operation_id.into(),
        expected_phase: item.phase.unwrap(),
        evidence_hash: item.evidence_hash.clone().unwrap(),
        action: WriteAuthorityRecoveryResolutionAction::AcceptRestoredState,
    };
    symlink(&source, &target).unwrap();
    let error = restarted.resolve_operator_exclusive(input).unwrap_err();
    assert!(error.contains("nu mai poate accepta"), "{error}");
    assert!(restarted.snapshot().unwrap().blocked);
    assert_eq!(fs::read_link(&target).unwrap(), source);
    fixture.cleanup();
}

#[test]
fn symlink_v2_operator_accepts_only_bound_current_lifetime_state_literal() {
    let fixture = AtomicRecoveryFixture::new("symlink-v2-accept-current", false);
    let target = fixture.parent.join("link");
    let source = PathBuf::from("desired");
    let operation_id = "wal-symlink-v2-accept-current";
    let (coordinator, _intent, _plan, record) =
        fixture.prepare_symlink_v2(operation_id, &target, &source, true);
    let guard = coordinator.begin(record).unwrap();
    symlink(&source, &target).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    let item = scan.items.first().unwrap();
    let before = fs::symlink_metadata(&target).unwrap();
    let receipt = restarted
        .resolve_operator_exclusive(WriteAuthorityRecoveryResolutionInput {
            operation_id: operation_id.into(),
            expected_phase: item.phase.unwrap(),
            evidence_hash: item.evidence_hash.clone().unwrap(),
            action: WriteAuthorityRecoveryResolutionAction::AcceptCurrentState,
        })
        .unwrap();
    assert!(!receipt.recovery_scan.blocked, "{receipt:?}");
    assert!(receipt.diagnostic.contains("lifetime+state+literal"));
    let after = fs::symlink_metadata(&target).unwrap();
    assert_eq!((after.dev(), after.ino()), (before.dev(), before.ino()));
    assert_eq!(fs::read_link(&target).unwrap(), source);
    fixture.cleanup();
}

#[test]
fn symlink_v2_accept_current_rejects_replacement_after_scan() {
    let fixture = AtomicRecoveryFixture::new("symlink-v2-current-stale", false);
    let target = fixture.parent.join("link");
    let displaced = fixture.parent.join("scanned-link");
    let source = PathBuf::from("desired");
    let operation_id = "wal-symlink-v2-current-stale";
    let (coordinator, _intent, _plan, record) =
        fixture.prepare_symlink_v2(operation_id, &target, &source, true);
    let guard = coordinator.begin(record).unwrap();
    symlink(&source, &target).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    let item = scan.items.first().unwrap();
    let input = WriteAuthorityRecoveryResolutionInput {
        operation_id: operation_id.into(),
        expected_phase: item.phase.unwrap(),
        evidence_hash: item.evidence_hash.clone().unwrap(),
        action: WriteAuthorityRecoveryResolutionAction::AcceptCurrentState,
    };
    fs::rename(&target, &displaced).unwrap();
    symlink(&source, &target).unwrap();
    let error = restarted.resolve_operator_exclusive(input).unwrap_err();
    assert!(error.contains("evidence hash stale"), "{error}");
    assert!(restarted.snapshot().unwrap().blocked);
    assert_eq!(fs::read_link(&target).unwrap(), source);
    assert_eq!(fs::read_link(&displaced).unwrap(), source);
    fixture.cleanup();
}

#[test]
fn symlink_v2_accept_current_fresh_recapture_rejects_internal_replacement() {
    let fixture = AtomicRecoveryFixture::new("symlink-v2-current-fresh", false);
    let target = fixture.parent.join("link");
    let displaced = fixture.parent.join("before-fresh-link");
    let source = PathBuf::from("desired");
    let operation_id = "wal-symlink-v2-current-fresh";
    let (coordinator, _intent, _plan, record) =
        fixture.prepare_symlink_v2(operation_id, &target, &source, true);
    let guard = coordinator.begin(record).unwrap();
    symlink(&source, &target).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    let item = scan.items.first().unwrap();
    let input = WriteAuthorityRecoveryResolutionInput {
        operation_id: operation_id.into(),
        expected_phase: item.phase.unwrap(),
        evidence_hash: item.evidence_hash.clone().unwrap(),
        action: WriteAuthorityRecoveryResolutionAction::AcceptCurrentState,
    };
    let hook_target = target.clone();
    let hook_displaced = displaced.clone();
    let hook_source = source.clone();
    let error = capability::with_before_symlink_current_state_fresh_capture_hook_for_test(
        move || {
            fs::rename(&hook_target, &hook_displaced).unwrap();
            symlink(&hook_source, &hook_target).unwrap();
        },
        || restarted.resolve_operator_exclusive(input),
    )
    .unwrap_err();
    assert!(error.contains("fresh lifetime/state"), "{error}");
    assert!(restarted.snapshot().unwrap().blocked);
    assert_eq!(fs::read_link(&target).unwrap(), source);
    assert_eq!(fs::read_link(&displaced).unwrap(), source);
    fixture.cleanup();
}

#[test]
fn restart_remove_leaf_recovery_preserves_exact_quarantine_for_operator() {
    let fixture = AtomicRecoveryFixture::new("remove-quarantine-manual", true);
    let (coordinator, _intent, plan, record) =
        fixture.prepare_remove_leaf("wal-remove-quarantine-manual");
    let quarantine = fixture
        .parent
        .join(super::super::decode_component_hex(&plan.evidence.quarantine_leaf_hex).unwrap());
    let mut guard = coordinator.begin(record).unwrap();
    guard.mark_auxiliary_durable().unwrap();
    fs::rename(&fixture.target, &quarantine).unwrap();
    guard.mark_effect_visible().unwrap();
    drop(plan);
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::CleanupRequired
    );
    assert!(!fixture.target.exists());
    assert_eq!(fs::read(&quarantine).unwrap(), b"baseline");
    fixture.cleanup();
}

#[test]
fn remove_leaf_operator_restores_exact_quarantine_and_clears_wal() {
    let fixture = AtomicRecoveryFixture::new("remove-operator-restore", true);
    let operation_id = "wal-remove-operator-restore";
    let (coordinator, _intent, plan, record) = fixture.prepare_remove_leaf(operation_id);
    let evidence_hash = record.evidence_hash.clone();
    let quarantine = fixture
        .parent
        .join(super::super::decode_component_hex(&plan.evidence.quarantine_leaf_hex).unwrap());
    let mut guard = coordinator.begin(record).unwrap();
    guard.mark_auxiliary_durable().unwrap();
    fs::rename(&fixture.target, &quarantine).unwrap();
    guard.mark_effect_visible().unwrap();
    drop(plan);
    drop(guard);
    let scan = coordinator.rescan_and_recover_exclusive().unwrap();
    assert_eq!(
        scan.items[0].available_resolution_actions,
        vec![WriteAuthorityRecoveryResolutionAction::RestoreOriginal]
    );

    let receipt = coordinator
        .resolve_operator_exclusive(WriteAuthorityRecoveryResolutionInput {
            operation_id: operation_id.into(),
            expected_phase: WalPhase::EffectVisible,
            evidence_hash,
            action: WriteAuthorityRecoveryResolutionAction::RestoreOriginal,
        })
        .unwrap();
    assert!(!receipt.recovery_scan.blocked, "{receipt:?}");
    assert_eq!(fs::read(&fixture.target).unwrap(), b"baseline");
    assert!(!quarantine.exists());
    fixture.cleanup();
}

#[test]
fn remove_leaf_operator_restore_never_overwrites_recreated_target() {
    let fixture = AtomicRecoveryFixture::new("remove-operator-target-conflict", true);
    let operation_id = "wal-remove-operator-target-conflict";
    let (coordinator, _intent, plan, record) = fixture.prepare_remove_leaf(operation_id);
    let evidence_hash = record.evidence_hash.clone();
    let quarantine = fixture
        .parent
        .join(super::super::decode_component_hex(&plan.evidence.quarantine_leaf_hex).unwrap());
    let mut guard = coordinator.begin(record).unwrap();
    guard.mark_auxiliary_durable().unwrap();
    fs::rename(&fixture.target, &quarantine).unwrap();
    guard.mark_effect_visible().unwrap();
    fs::write(&fixture.target, b"competitor").unwrap();
    drop(plan);
    drop(guard);

    let error = coordinator
        .resolve_operator_exclusive(WriteAuthorityRecoveryResolutionInput {
            operation_id: operation_id.into(),
            expected_phase: WalPhase::EffectVisible,
            evidence_hash,
            action: WriteAuthorityRecoveryResolutionAction::RestoreOriginal,
        })
        .unwrap_err();
    assert!(error.contains("nu este permis") || error.contains("nu mai este absent"));
    assert_eq!(fs::read(&fixture.target).unwrap(), b"competitor");
    assert_eq!(fs::read(&quarantine).unwrap(), b"baseline");
    assert!(coordinator.snapshot().unwrap().blocked);
    fixture.cleanup();
}

#[test]
fn remove_leaf_operator_accepts_only_exact_restored_state() {
    let fixture = AtomicRecoveryFixture::new("remove-operator-accept-restored", true);
    let operation_id = "wal-remove-operator-accept-restored";
    let (coordinator, _intent, plan, record) = fixture.prepare_remove_leaf(operation_id);
    let evidence_hash = record.evidence_hash.clone();
    let quarantine = fixture
        .parent
        .join(super::super::decode_component_hex(&plan.evidence.quarantine_leaf_hex).unwrap());
    let mut guard = coordinator.begin(record).unwrap();
    guard.mark_auxiliary_durable().unwrap();
    fs::rename(&fixture.target, &quarantine).unwrap();
    guard.mark_effect_visible().unwrap();
    fs::rename(&quarantine, &fixture.target).unwrap();
    drop(plan);
    drop(guard);
    let scan = coordinator.rescan_and_recover_exclusive().unwrap();
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::RollbackCompleted
    );
    assert_eq!(
        scan.items[0].available_resolution_actions,
        vec![WriteAuthorityRecoveryResolutionAction::AcceptRestoredState]
    );

    let receipt = coordinator
        .resolve_operator_exclusive(WriteAuthorityRecoveryResolutionInput {
            operation_id: operation_id.into(),
            expected_phase: WalPhase::EffectVisible,
            evidence_hash,
            action: WriteAuthorityRecoveryResolutionAction::AcceptRestoredState,
        })
        .unwrap();
    assert!(!receipt.recovery_scan.blocked, "{receipt:?}");
    assert_eq!(fs::read(&fixture.target).unwrap(), b"baseline");
    fixture.cleanup();
}

#[test]
fn remove_leaf_operator_rejects_stale_evidence_hash() {
    let fixture = AtomicRecoveryFixture::new("remove-operator-stale-hash", true);
    let operation_id = "wal-remove-operator-stale-hash";
    let (coordinator, _intent, plan, record) = fixture.prepare_remove_leaf(operation_id);
    let quarantine = fixture
        .parent
        .join(super::super::decode_component_hex(&plan.evidence.quarantine_leaf_hex).unwrap());
    let mut guard = coordinator.begin(record).unwrap();
    guard.mark_auxiliary_durable().unwrap();
    fs::rename(&fixture.target, &quarantine).unwrap();
    guard.mark_effect_visible().unwrap();
    drop(plan);
    drop(guard);

    let error = coordinator
        .resolve_operator_exclusive(WriteAuthorityRecoveryResolutionInput {
            operation_id: operation_id.into(),
            expected_phase: WalPhase::EffectVisible,
            evidence_hash: "00".repeat(32),
            action: WriteAuthorityRecoveryResolutionAction::RestoreOriginal,
        })
        .unwrap_err();
    assert!(error.contains("evidence hash stale"), "{error}");
    assert!(!fixture.target.exists());
    assert_eq!(fs::read(&quarantine).unwrap(), b"baseline");
    assert!(coordinator.snapshot().unwrap().blocked);
    fixture.cleanup();
}

#[test]
fn remove_tree_operator_restores_only_intact_quarantine() {
    let fixture = AtomicRecoveryFixture::new("remove-tree-restore-intact", false);
    fixture.create_tree();
    let operation_id = "wal-remove-tree-restore-intact";
    let (coordinator, _intent, plan, record) = fixture.prepare_remove_tree(operation_id);
    let evidence_hash = record.evidence_hash.clone();
    let quarantine = fixture
        .parent
        .join(super::super::decode_component_hex(&plan.evidence.quarantine_leaf_hex).unwrap());
    let mut guard = coordinator.begin(record).unwrap();
    guard.mark_auxiliary_durable().unwrap();
    fs::rename(&fixture.target, &quarantine).unwrap();
    guard.mark_effect_visible().unwrap();
    drop(plan);
    drop(guard);

    let scan = coordinator.rescan_and_recover_exclusive().unwrap();
    assert_eq!(
        scan.items[0].available_resolution_actions,
        vec![
            WriteAuthorityRecoveryResolutionAction::RestoreOriginal,
            WriteAuthorityRecoveryResolutionAction::ContinueTreeRemoval,
        ]
    );
    let receipt = coordinator
        .resolve_operator_exclusive(WriteAuthorityRecoveryResolutionInput {
            operation_id: operation_id.into(),
            expected_phase: WalPhase::EffectVisible,
            evidence_hash,
            action: WriteAuthorityRecoveryResolutionAction::RestoreOriginal,
        })
        .unwrap();

    assert!(!receipt.recovery_scan.blocked, "{receipt:?}");
    assert_eq!(fs::read(fixture.target.join("nested/b.txt")).unwrap(), b"b");
    assert!(!quarantine.exists());
    fixture.cleanup();
}

#[test]
fn remove_tree_operator_restores_only_remaining_partial_tree() {
    let fixture = AtomicRecoveryFixture::new("remove-tree-restore-partial", false);
    fixture.create_tree();
    let operation_id = "wal-remove-tree-restore-partial";
    let (coordinator, _intent, plan, record) = fixture.prepare_remove_tree(operation_id);
    let evidence_hash = record.evidence_hash.clone();
    let quarantine = fixture
        .parent
        .join(super::super::decode_component_hex(&plan.evidence.quarantine_leaf_hex).unwrap());
    let mut guard = coordinator.begin(record).unwrap();
    guard.mark_auxiliary_durable().unwrap();
    fs::rename(&fixture.target, &quarantine).unwrap();
    guard.mark_effect_visible().unwrap();
    fs::remove_file(quarantine.join("a.txt")).unwrap();
    drop(plan);
    drop(guard);

    let scan = coordinator.rescan_and_recover_exclusive().unwrap();
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::PartialTreeRemoval
    );
    assert_eq!(
        scan.items[0].available_resolution_actions,
        vec![
            WriteAuthorityRecoveryResolutionAction::RestoreRemainingTree,
            WriteAuthorityRecoveryResolutionAction::ContinueTreeRemoval,
        ]
    );
    let receipt = coordinator
        .resolve_operator_exclusive(WriteAuthorityRecoveryResolutionInput {
            operation_id: operation_id.into(),
            expected_phase: WalPhase::EffectVisible,
            evidence_hash,
            action: WriteAuthorityRecoveryResolutionAction::RestoreRemainingTree,
        })
        .unwrap();

    assert!(!receipt.recovery_scan.blocked, "{receipt:?}");
    assert!(!fixture.target.join("a.txt").exists());
    assert_eq!(fs::read(fixture.target.join("nested/b.txt")).unwrap(), b"b");
    fixture.cleanup();
}

#[test]
fn remove_tree_operator_can_explicitly_finish_partial_tree() {
    let fixture = AtomicRecoveryFixture::new("remove-tree-continue-partial", false);
    fixture.create_tree();
    let operation_id = "wal-remove-tree-continue-partial";
    let (coordinator, _intent, plan, record) = fixture.prepare_remove_tree(operation_id);
    let evidence_hash = record.evidence_hash.clone();
    let quarantine = fixture
        .parent
        .join(super::super::decode_component_hex(&plan.evidence.quarantine_leaf_hex).unwrap());
    let mut guard = coordinator.begin(record).unwrap();
    guard.mark_auxiliary_durable().unwrap();
    fs::rename(&fixture.target, &quarantine).unwrap();
    guard.mark_effect_visible().unwrap();
    fs::remove_file(quarantine.join("a.txt")).unwrap();
    drop(plan);
    drop(guard);

    let receipt = coordinator
        .resolve_operator_exclusive(WriteAuthorityRecoveryResolutionInput {
            operation_id: operation_id.into(),
            expected_phase: WalPhase::EffectVisible,
            evidence_hash,
            action: WriteAuthorityRecoveryResolutionAction::ContinueTreeRemoval,
        })
        .unwrap();

    assert!(!receipt.recovery_scan.blocked, "{receipt:?}");
    assert!(!fixture.target.exists());
    assert!(!quarantine.exists());
    fixture.cleanup();
}

#[test]
fn append_v2_short_write_recovery_continues_only_exact_remainder() {
    let fixture = AppendV2Fixture::new("append-v2-short-write", true);
    let payload = b"{\"short_write\":true}\n";
    let operation_id = "wal-append-v2-short-write";
    let (coordinator, intent, plan, record) = fixture.prepare(operation_id, payload);
    let mut guard = coordinator.begin(record).unwrap();
    let effect = capability::with_append_v2_short_write_for_test(7, || {
        capability::append_wal(&intent.target, payload, plan, &mut guard)
    })
    .unwrap();
    assert!(effect.recovery_required, "{effect:?}");
    assert_eq!(guard.phase(), WalPhase::AuxiliaryDurable);
    let partial = fs::read(&fixture.target).unwrap();
    assert_eq!(
        partial,
        [b"{\"baseline\":true}\n".as_slice(), &payload[..7]].concat()
    );
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    assert!(!restarted.snapshot().unwrap().blocked);
    assert_eq!(
        fs::read(&fixture.target).unwrap(),
        [b"{\"baseline\":true}\n".as_slice(), payload.as_slice()].concat()
    );
    drop(restarted);
    let second = fixture.restart_coordinator();
    assert!(!second.snapshot().unwrap().blocked);
    assert_eq!(
        fs::read(&fixture.target).unwrap(),
        [b"{\"baseline\":true}\n".as_slice(), payload.as_slice()].concat()
    );
    drop(second);
    fixture.cleanup();
}
