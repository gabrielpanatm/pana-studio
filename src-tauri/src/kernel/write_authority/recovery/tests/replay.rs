use super::{fixtures::*, *};

#[test]
fn prepared_atomic_exact_no_effect_is_the_only_automatic_legacy_action() {
    let fixture = AtomicRecoveryFixture::new("prepared-atomic-no-effect", false);
    let (coordinator, _plan, record) = fixture.prepare("wal-prepared-atomic-no-effect", b"payload");
    let guard = coordinator.begin(record).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    assert!(!restarted.snapshot().unwrap().blocked);
    assert!(!fixture.target.exists());
    fixture.cleanup();
}

#[test]
fn auxiliary_atomic_staged_payload_remains_hot_without_unlink() {
    let fixture = AtomicRecoveryFixture::new("aux-atomic-staged", false);
    let payload = b"staged payload";
    let (coordinator, plan, record) = fixture.prepare("wal-aux-atomic-staged", payload);
    let mut guard = coordinator.begin(record).unwrap();
    let temp = fixture.parent.join(plan.temp_leaf().unwrap());
    fs::write(&temp, payload).unwrap();
    guard.mark_auxiliary_durable().unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::StagedOnly
    );
    assert_eq!(
        scan.items[0].available_resolution_actions,
        vec![WriteAuthorityRecoveryResolutionAction::DiscardStagedWrite]
    );
    assert_eq!(fs::read(&temp).unwrap(), payload);
    assert!(!fixture.target.exists());
    fixture.cleanup();
}

#[test]
fn atomic_payload_shape_never_auto_finalizes_without_causal_checkpoint() {
    for phase in [
        WalPhase::AuxiliaryDurable,
        WalPhase::EffectVisible,
        WalPhase::TargetDurable,
    ] {
        let label = format!("atomic-noncausal-finalize-{phase:?}");
        let fixture = AtomicRecoveryFixture::new(&label, false);
        let payload = b"committed-shape";
        let (coordinator, plan, record) = fixture.prepare(&label, payload);
        let mut guard = coordinator.begin(record).unwrap();
        let temp = fixture.parent.join(plan.temp_leaf().unwrap());
        fs::write(&temp, payload).unwrap();
        guard.mark_auxiliary_durable().unwrap();
        fs::rename(&temp, &fixture.target).unwrap();
        if phase >= WalPhase::EffectVisible {
            guard.mark_effect_visible().unwrap();
        }
        if phase >= WalPhase::TargetDurable {
            guard.mark_target_durable().unwrap();
        }
        drop(guard);
        drop(coordinator);

        let restarted = fixture.restart_coordinator();
        let scan = restarted.snapshot().unwrap();
        assert!(scan.blocked, "{phase:?}: {scan:?}");
        assert_eq!(
            scan.items[0].classification,
            super::super::WriteAuthorityRecoveryClassification::EffectCommitted
        );
        assert!(!scan.items[0].automatic_recovery_available);
        assert_eq!(fs::read(&fixture.target).unwrap(), payload);
        drop(restarted);
        fixture.cleanup();
    }
}

#[test]
fn prepared_append_exact_no_effect_is_the_only_automatic_legacy_action() {
    let fixture = AtomicRecoveryFixture::new("prepared-append-no-effect", true);
    let (coordinator, record) =
        fixture.prepare_append("wal-prepared-append-no-effect", b"-payload");
    let guard = coordinator.begin(record).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    assert!(!restarted.snapshot().unwrap().blocked);
    assert_eq!(fs::read(&fixture.target).unwrap(), b"baseline");
    fixture.cleanup();
}

#[test]
fn auxiliary_append_partial_payload_remains_hot_and_untruncated() {
    let fixture = AtomicRecoveryFixture::new("aux-append-partial", true);
    let payload = b"-append-payload";
    let (coordinator, record) = fixture.prepare_append("wal-aux-append-partial", payload);
    let mut guard = coordinator.begin(record).unwrap();
    guard.mark_auxiliary_durable().unwrap();
    OpenOptions::new()
        .append(true)
        .open(&fixture.target)
        .unwrap()
        .write_all(&payload[..7])
        .unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::PartialAppend
    );
    assert_eq!(fs::read(&fixture.target).unwrap(), b"baseline-append");
    fixture.cleanup();
}

#[test]
fn append_complete_shape_never_auto_finalizes_without_causal_checkpoint() {
    for phase in [
        WalPhase::AuxiliaryDurable,
        WalPhase::EffectVisible,
        WalPhase::TargetDurable,
    ] {
        let label = format!("append-noncausal-finalize-{phase:?}");
        let fixture = AtomicRecoveryFixture::new(&label, true);
        let payload = b"-complete";
        let (coordinator, record) = fixture.prepare_append(&label, payload);
        let mut guard = coordinator.begin(record).unwrap();
        guard.mark_auxiliary_durable().unwrap();
        OpenOptions::new()
            .append(true)
            .open(&fixture.target)
            .unwrap()
            .write_all(payload)
            .unwrap();
        if phase >= WalPhase::EffectVisible {
            guard.mark_effect_visible().unwrap();
        }
        if phase >= WalPhase::TargetDurable {
            guard.mark_target_durable().unwrap();
        }
        drop(guard);
        drop(coordinator);

        let restarted = fixture.restart_coordinator();
        let scan = restarted.snapshot().unwrap();
        assert!(scan.blocked, "{phase:?}: {scan:?}");
        assert_eq!(
            scan.items[0].classification,
            super::super::WriteAuthorityRecoveryClassification::EffectCommitted
        );
        assert!(!scan.items[0].automatic_recovery_available);
        assert_eq!(fs::read(&fixture.target).unwrap(), b"baseline-complete");
        drop(restarted);
        fixture.cleanup();
    }
}

#[test]
fn restart_recovery_clears_directory_noop_with_same_identity() {
    let fixture = AtomicRecoveryFixture::new("mkdir-noop", false);
    let target = fixture.parent.join("already-there");
    fs::create_dir(&target).unwrap();
    let (coordinator, _intent, _plan, record) =
        fixture.prepare_directory("wal-mkdir-noop", &target);
    let guard = coordinator.begin(record).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    assert!(!restarted.snapshot().unwrap().blocked);
    assert!(target.is_dir());
    fixture.cleanup();
}

#[test]
fn prepared_directory_absent_shape_remains_hot_because_mkdir_precedes_phase() {
    let fixture = AtomicRecoveryFixture::new("mkdir-prepared-absent", false);
    let target = fixture.parent.join("created");
    let (coordinator, _intent, _plan, record) =
        fixture.prepare_directory("wal-mkdir-prepared-absent", &target);
    let guard = coordinator.begin(record).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::Conflict
    );
    assert!(!scan.items[0].automatic_recovery_available);
    assert!(!target.exists());
    fixture.cleanup();
}

#[test]
fn directory_absent_shape_is_never_automatic_in_any_legacy_runtime_phase() {
    for phase in [
        WalPhase::Prepared,
        WalPhase::AuxiliaryDurable,
        WalPhase::EffectVisible,
        WalPhase::TargetDurable,
    ] {
        let label = format!("mkdir-absent-phase-{phase:?}");
        let fixture = AtomicRecoveryFixture::new(&label, false);
        let target = fixture.parent.join("created");
        let (coordinator, _intent, _plan, record) = fixture.prepare_directory(&label, &target);
        let mut guard = coordinator.begin(record).unwrap();
        if phase >= WalPhase::AuxiliaryDurable {
            guard.mark_auxiliary_durable().unwrap();
        }
        if phase >= WalPhase::EffectVisible {
            guard.mark_effect_visible().unwrap();
        }
        if phase >= WalPhase::TargetDurable {
            guard.mark_target_durable().unwrap();
        }
        drop(guard);
        drop(coordinator);

        let restarted = fixture.restart_coordinator();
        let scan = restarted.snapshot().unwrap();
        assert!(scan.blocked, "{phase:?}: {scan:?}");
        assert_eq!(
            scan.items[0].classification,
            super::super::WriteAuthorityRecoveryClassification::Conflict
        );
        assert!(!scan.items[0].automatic_recovery_available);
        assert!(!target.exists());
        drop(restarted);
        fixture.cleanup();
    }
}

#[test]
fn directory_crash_after_mkdir_before_phase_then_removed_remains_hot() {
    let fixture = AtomicRecoveryFixture::new("mkdir-prephase-crash-removed", false);
    let target = fixture.parent.join("created");
    let (coordinator, intent, plan, record) =
        fixture.prepare_directory("wal-mkdir-prephase-crash-removed", &target);
    let mut guard = coordinator.begin(record).unwrap();
    let result = catch_unwind(AssertUnwindSafe(|| {
        capability::with_after_directory_create_before_phase_hook_for_test(
            || panic!("simulated crash after mkdirat before WAL phase"),
            || capability::create_directory_all_wal(&intent.target, &plan, &mut guard),
        )
    }));
    assert!(result.is_err());
    assert!(target.is_dir());
    fs::remove_dir(&target).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].phase,
        Some(WalPhase::Prepared),
        "mkdirat a precedat prima tranziție WAL"
    );
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::Conflict
    );
    assert!(!target.exists());
    fixture.cleanup();
}

#[test]
fn directory_existing_noop_is_automatic_only_in_prepared_phase() {
    for phase in [
        WalPhase::AuxiliaryDurable,
        WalPhase::EffectVisible,
        WalPhase::TargetDurable,
    ] {
        let label = format!("mkdir-existing-impossible-{phase:?}");
        let fixture = AtomicRecoveryFixture::new(&label, false);
        let target = fixture.parent.join("already-there");
        fs::create_dir(&target).unwrap();
        let (coordinator, _intent, _plan, record) = fixture.prepare_directory(&label, &target);
        let mut guard = coordinator.begin(record).unwrap();
        guard.mark_auxiliary_durable().unwrap();
        if phase >= WalPhase::EffectVisible {
            guard.mark_effect_visible().unwrap();
        }
        if phase >= WalPhase::TargetDurable {
            guard.mark_target_durable().unwrap();
        }
        drop(guard);
        drop(coordinator);

        let restarted = fixture.restart_coordinator();
        let scan = restarted.snapshot().unwrap();
        assert!(scan.blocked, "{phase:?}: {scan:?}");
        assert_eq!(
            scan.items[0].classification,
            super::super::WriteAuthorityRecoveryClassification::Conflict
        );
        assert!(!scan.items[0].automatic_recovery_available);
        assert!(target.is_dir());
        drop(restarted);
        fixture.cleanup();
    }
}

#[test]
fn directory_v2_single_leaf_commits_direct_empty_mode_exact() {
    let fixture = AtomicRecoveryFixture::new("mkdir-v2-commit", false);
    let target = fixture.parent.join("created");
    let (coordinator, intent, plan, record) =
        fixture.prepare_directory_v2("wal-mkdir-v2-commit", &target);
    let mut guard = coordinator.begin(record).unwrap();
    let effect = capability::create_directory_all_wal(&intent.target, &plan, &mut guard).unwrap();
    assert!(effect.changed);
    assert!(!effect.recovery_required, "{effect:?}");
    assert_eq!(guard.phase(), WalPhase::TargetDurable);
    guard.commit().unwrap();
    assert!(target.is_dir());
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o7777,
        0o755
    );
    assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
    fixture.cleanup();
}

#[test]
fn directory_v2_restart_finalizes_exact_checkpointed_target_idempotently() {
    let fixture = AtomicRecoveryFixture::new("mkdir-v2-finalize", false);
    let target = fixture.parent.join("created");
    let (coordinator, intent, plan, record) =
        fixture.prepare_directory_v2("wal-mkdir-v2-finalize", &target);
    let mut guard = coordinator.begin(record).unwrap();
    let crashed = catch_unwind(AssertUnwindSafe(|| {
        capability::with_after_directory_v2_checkpoint_hook_for_test(
            || panic!("simulated crash after Directory v2 checkpoint"),
            || capability::create_directory_all_wal(&intent.target, &plan, &mut guard),
        )
    }));
    assert!(crashed.is_err());
    assert!(target.is_dir());
    assert_eq!(guard.phase(), WalPhase::AuxiliaryDurable);
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    assert!(!restarted.snapshot().unwrap().blocked);
    assert!(target.is_dir());
    drop(restarted);
    let second_restart = fixture.restart_coordinator();
    assert!(!second_restart.snapshot().unwrap().blocked);
    assert!(target.is_dir());
    fixture.cleanup();
}

#[test]
fn directory_v2_prepared_existing_exact_is_noop_but_absent_stays_hot() {
    let existing_fixture = AtomicRecoveryFixture::new("mkdir-v2-noop-existing", false);
    let existing_target = existing_fixture.parent.join("created");
    fs::create_dir(&existing_target).unwrap();
    fs::write(existing_target.join("preexisting-child"), b"kept").unwrap();
    let (coordinator, _intent, _plan, record) =
        existing_fixture.prepare_directory_v2("wal-mkdir-v2-noop-existing", &existing_target);
    drop(coordinator.begin(record).unwrap());
    drop(coordinator);
    let restarted = existing_fixture.restart_coordinator();
    assert!(!restarted.snapshot().unwrap().blocked);
    assert_eq!(
        fs::read(existing_target.join("preexisting-child")).unwrap(),
        b"kept"
    );
    drop(restarted);
    existing_fixture.cleanup();

    let absent_fixture = AtomicRecoveryFixture::new("mkdir-v2-prepared-absent", false);
    let absent_target = absent_fixture.parent.join("created");
    let (coordinator, _intent, _plan, record) =
        absent_fixture.prepare_directory_v2("wal-mkdir-v2-prepared-absent", &absent_target);
    drop(coordinator.begin(record).unwrap());
    drop(coordinator);
    for _ in 0..2 {
        let restarted = absent_fixture.restart_coordinator();
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
    }
    assert!(!absent_target.exists());
    absent_fixture.cleanup();
}

#[test]
fn directory_v2_crash_after_direct_mkdir_before_checkpoint_stays_hot_twice() {
    let fixture = AtomicRecoveryFixture::new("mkdir-v2-direct-precheckpoint", false);
    let target = fixture.parent.join("created");
    let (coordinator, intent, plan, record) =
        fixture.prepare_directory_v2("wal-mkdir-v2-direct-precheckpoint", &target);
    let mut guard = coordinator.begin(record).unwrap();
    let hook_target = target.clone();
    let crashed = catch_unwind(AssertUnwindSafe(|| {
        capability::with_after_directory_create_before_phase_hook_for_test(
            move || {
                fs::set_permissions(&hook_target, fs::Permissions::from_mode(0o755)).unwrap();
                panic!("simulated crash after direct mkdir/open");
            },
            || capability::create_directory_all_wal(&intent.target, &plan, &mut guard),
        )
    }));
    assert!(crashed.is_err());
    assert!(target.is_dir());
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
        assert!(target.is_dir());
        drop(restarted);
    }
    fixture.cleanup();
}

#[test]
fn directory_v2_detects_target_replacement_after_first_open_before_checkpoint() {
    let fixture = AtomicRecoveryFixture::new("mkdir-v2-first-open-race", false);
    let target = fixture.parent.join("created");
    let (coordinator, intent, plan, record) =
        fixture.prepare_directory_v2("wal-mkdir-v2-first-open-race", &target);
    let mut guard = coordinator.begin(record).unwrap();
    let hook_target = target.clone();
    let effect = capability::with_after_directory_create_before_phase_hook_for_test(
        move || {
            fs::remove_dir(&hook_target).unwrap();
            fs::create_dir(&hook_target).unwrap();
        },
        || capability::create_directory_all_wal(&intent.target, &plan, &mut guard),
    )
    .unwrap();
    assert!(effect.recovery_required);
    assert_eq!(guard.phase(), WalPhase::Prepared);
    assert!(target.is_dir());
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    assert!(restarted.snapshot().unwrap().blocked);
    assert!(target.is_dir());
    fixture.cleanup();
}

#[test]
fn directory_v2_parent_fsync_failure_stays_prepared_with_direct_target_hot() {
    let fixture = AtomicRecoveryFixture::new("mkdir-v2-parent-fsync", false);
    let target = fixture.parent.join("created");
    let (coordinator, intent, plan, record) =
        fixture.prepare_directory_v2("wal-mkdir-v2-parent-fsync", &target);
    let mut guard = coordinator.begin(record).unwrap();
    let effect = capability::with_directory_sync_failure_for_test(|| {
        capability::create_directory_all_wal(&intent.target, &plan, &mut guard)
    })
    .unwrap();
    assert!(effect.recovery_required);
    assert_eq!(guard.phase(), WalPhase::Prepared);
    assert!(target.is_dir());
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert!(target.is_dir());
    fixture.cleanup();
}

#[test]
fn directory_sync_failure_leaves_hot_record_for_manual_restart_review() {
    let fixture = AtomicRecoveryFixture::new("mkdir-sync-failure", false);
    let target = fixture.parent.join("first/second");
    let (coordinator, intent, plan, record) =
        fixture.prepare_directory("wal-mkdir-sync-failure", &target);
    let mut guard = coordinator.begin(record).unwrap();
    let effect = capability::with_directory_sync_failure_for_test(|| {
        capability::create_directory_all_wal(&intent.target, &plan, &mut guard)
    })
    .unwrap();
    assert!(effect.recovery_required);
    drop(guard);
    assert!(coordinator.snapshot().unwrap().blocked);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked);
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::PartialNamespaceCreation
    );
    assert!(!target.exists());
    fixture.cleanup();
}

#[test]
fn restart_recovery_requires_manual_review_after_partial_symlink_parent_creation() {
    let fixture = AtomicRecoveryFixture::new("symlink-parent-partial", false);
    let target = fixture.parent.join("first/second/link");
    let source = PathBuf::from("../../missing-source");
    let (coordinator, _intent, _plan, record) =
        fixture.prepare_symlink("wal-symlink-parent-partial", &target, &source);
    let guard = coordinator.begin(record).unwrap();
    fs::create_dir(fixture.parent.join("first")).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked);
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::PartialNamespaceCreation
    );
    assert!(!target.exists());
    fixture.cleanup();
}

#[test]
fn restart_recovery_clears_exact_existing_symlink_noop() {
    let fixture = AtomicRecoveryFixture::new("symlink-noop", false);
    let target = fixture.parent.join("link");
    let source = PathBuf::from("relative/source");
    std::os::unix::fs::symlink(&source, &target).unwrap();
    let (coordinator, _intent, _plan, record) =
        fixture.prepare_symlink("wal-symlink-noop", &target, &source);
    let guard = coordinator.begin(record).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    assert!(!restarted.snapshot().unwrap().blocked);
    assert_eq!(fs::read_link(&target).unwrap(), source);
    fixture.cleanup();
}

#[test]
fn symlink_crash_after_create_before_phase_then_removed_remains_hot() {
    let fixture = AtomicRecoveryFixture::new("symlink-prephase-crash-removed", false);
    let target = fixture.parent.join("link");
    let source = PathBuf::from("dangling");
    let (coordinator, intent, plan, record) =
        fixture.prepare_symlink("wal-symlink-prephase-crash-removed", &target, &source);
    let mut guard = coordinator.begin(record).unwrap();
    let result = catch_unwind(AssertUnwindSafe(|| {
        capability::with_after_symlink_create_before_phase_hook_for_test(
            || panic!("simulated crash after symlinkat before WAL phase"),
            || capability::symlink_entry_wal(&intent.target, &source, &plan, &mut guard),
        )
    }));
    assert!(result.is_err());
    assert_eq!(fs::read_link(&target).unwrap(), source);
    fs::remove_file(&target).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].phase,
        Some(WalPhase::Prepared),
        "symlinkat a precedat prima tranziție WAL"
    );
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::Conflict
    );
    assert!(!target.exists());
    fixture.cleanup();
}

#[test]
fn symlink_absent_shape_is_never_automatic_in_any_legacy_runtime_phase() {
    for phase in [
        WalPhase::Prepared,
        WalPhase::AuxiliaryDurable,
        WalPhase::EffectVisible,
        WalPhase::TargetDurable,
    ] {
        let label = format!("symlink-absent-phase-{phase:?}");
        let fixture = AtomicRecoveryFixture::new(&label, false);
        let target = fixture.parent.join("link");
        let source = PathBuf::from("dangling");
        let (coordinator, _intent, _plan, record) =
            fixture.prepare_symlink(&label, &target, &source);
        let mut guard = coordinator.begin(record).unwrap();
        if phase >= WalPhase::AuxiliaryDurable {
            guard.mark_auxiliary_durable().unwrap();
        }
        if phase >= WalPhase::EffectVisible {
            guard.mark_effect_visible().unwrap();
        }
        if phase >= WalPhase::TargetDurable {
            guard.mark_target_durable().unwrap();
        }
        drop(guard);
        drop(coordinator);

        let restarted = fixture.restart_coordinator();
        let scan = restarted.snapshot().unwrap();
        assert!(scan.blocked, "{phase:?}: {scan:?}");
        assert_eq!(
            scan.items[0].classification,
            super::super::WriteAuthorityRecoveryClassification::Conflict
        );
        assert!(!scan.items[0].automatic_recovery_available);
        assert!(!target.exists());
        drop(restarted);
        fixture.cleanup();
    }
}

#[test]
fn symlink_existing_noop_is_automatic_only_in_prepared_phase() {
    for phase in [
        WalPhase::AuxiliaryDurable,
        WalPhase::EffectVisible,
        WalPhase::TargetDurable,
    ] {
        let label = format!("symlink-existing-impossible-{phase:?}");
        let fixture = AtomicRecoveryFixture::new(&label, false);
        let target = fixture.parent.join("link");
        let source = PathBuf::from("relative/source");
        symlink(&source, &target).unwrap();
        let (coordinator, _intent, _plan, record) =
            fixture.prepare_symlink(&label, &target, &source);
        let mut guard = coordinator.begin(record).unwrap();
        guard.mark_auxiliary_durable().unwrap();
        if phase >= WalPhase::EffectVisible {
            guard.mark_effect_visible().unwrap();
        }
        if phase >= WalPhase::TargetDurable {
            guard.mark_target_durable().unwrap();
        }
        drop(guard);
        drop(coordinator);

        let restarted = fixture.restart_coordinator();
        let scan = restarted.snapshot().unwrap();
        assert!(scan.blocked, "{phase:?}: {scan:?}");
        assert_eq!(
            scan.items[0].classification,
            super::super::WriteAuthorityRecoveryClassification::Conflict
        );
        assert!(!scan.items[0].automatic_recovery_available);
        assert_eq!(fs::read_link(&target).unwrap(), source);
        drop(restarted);
        fixture.cleanup();
    }
}

#[test]
fn restart_recovery_preserves_conflicting_symlink_without_unlink() {
    let fixture = AtomicRecoveryFixture::new("symlink-conflict", false);
    let target = fixture.parent.join("link");
    let desired = PathBuf::from("desired");
    let conflicting = PathBuf::from("conflicting");
    let (coordinator, _intent, _plan, record) =
        fixture.prepare_symlink("wal-symlink-conflict", &target, &desired);
    let guard = coordinator.begin(record).unwrap();
    std::os::unix::fs::symlink(&conflicting, &target).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked);
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::Conflict
    );
    assert_eq!(fs::read_link(&target).unwrap(), conflicting);
    fixture.cleanup();
}

#[test]
fn symlink_sync_failure_leaves_hot_record_for_manual_restart_review() {
    let fixture = AtomicRecoveryFixture::new("symlink-sync-failure", false);
    let target = fixture.parent.join("link");
    let source = PathBuf::from("dangling");
    let (coordinator, intent, plan, record) =
        fixture.prepare_symlink("wal-symlink-sync-failure", &target, &source);
    let mut guard = coordinator.begin(record).unwrap();
    let effect = capability::with_directory_sync_failure_for_test(|| {
        capability::symlink_entry_wal(&intent.target, &source, &plan, &mut guard)
    })
    .unwrap();
    assert!(effect.recovery_required);
    drop(guard);
    assert!(coordinator.snapshot().unwrap().blocked);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked);
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::EffectCommitted
    );
    assert_eq!(fs::read_link(&target).unwrap(), source);
    fixture.cleanup();
}

#[test]
fn symlink_eio_stays_hot_because_legacy_wal_does_not_persist_the_syscall_result() {
    let fixture = AtomicRecoveryFixture::new("symlink-eio", false);
    let target = fixture.parent.join("link");
    let source = PathBuf::from("dangling");
    let (coordinator, intent, plan, record) =
        fixture.prepare_symlink("wal-symlink-eio", &target, &source);
    let mut guard = coordinator.begin(record).unwrap();
    let effect = capability::with_symlink_eio_for_test(|| {
        capability::symlink_entry_wal(&intent.target, &source, &plan, &mut guard)
    })
    .unwrap();
    assert!(effect.recovery_required);
    drop(guard);
    assert!(coordinator.snapshot().unwrap().blocked);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::Conflict
    );
    assert!(!scan.items[0].automatic_recovery_available);
    assert!(!target.exists());
    fixture.cleanup();
}

#[test]
fn symlink_v2_direct_create_commits_without_namespace_artifacts() {
    let fixture = AtomicRecoveryFixture::new("symlink-v2-direct-commit", false);
    let target = fixture.parent.join("link");
    let source = PathBuf::from("../dangling-target");
    let (coordinator, intent, plan, record) =
        fixture.prepare_symlink_v2("wal-symlink-v2-direct-commit", &target, &source, false);
    let mut guard = coordinator.begin(record).unwrap();
    let effect = capability::symlink_entry_wal(&intent.target, &source, &plan, &mut guard).unwrap();
    assert!(effect.changed, "{effect:?}");
    assert!(!effect.recovery_required, "{effect:?}");
    assert_eq!(guard.phase(), WalPhase::TargetDurable);
    guard.commit().unwrap();
    assert_eq!(fs::read_link(&target).unwrap(), source);
    let names = fs::read_dir(&fixture.parent)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    assert_eq!(names, vec![OsString::from("link")]);
    fixture.cleanup();
}

#[test]
fn symlink_v2_existing_exact_is_descriptor_bound_noop() {
    let fixture = AtomicRecoveryFixture::new("symlink-v2-noop", false);
    let target = fixture.parent.join("link");
    let source = PathBuf::from("relative/source");
    symlink(&source, &target).unwrap();
    let before = fs::symlink_metadata(&target).unwrap();
    let (coordinator, intent, plan, record) =
        fixture.prepare_symlink_v2("wal-symlink-v2-noop", &target, &source, false);
    let mut guard = coordinator.begin(record).unwrap();
    let effect = capability::symlink_entry_wal(&intent.target, &source, &plan, &mut guard).unwrap();
    assert!(!effect.changed, "{effect:?}");
    assert!(!effect.recovery_required, "{effect:?}");
    guard.abort_no_effect().unwrap();
    let after = fs::symlink_metadata(&target).unwrap();
    assert_eq!((after.dev(), after.ino()), (before.dev(), before.ino()));
    assert_eq!(fs::read_link(&target).unwrap(), source);
    fixture.cleanup();
}

#[test]
fn symlink_v2_first_open_window_never_auto_adopts_same_literal_replacement() {
    let fixture = AtomicRecoveryFixture::new("symlink-v2-first-open-replace", false);
    let target = fixture.parent.join("link");
    let displaced = fixture.parent.join("original-created-link");
    let source = PathBuf::from("desired");
    let operation_id = "wal-symlink-v2-first-open-replace";
    let (coordinator, intent, plan, record) =
        fixture.prepare_symlink_v2(operation_id, &target, &source, true);
    let mut guard = coordinator.begin(record).unwrap();
    let hook_target = target.clone();
    let hook_displaced = displaced.clone();
    let hook_source = source.clone();
    let effect = capability::with_after_symlink_v2_first_open_before_capture_hook_for_test(
        move || {
            fs::rename(&hook_target, &hook_displaced).unwrap();
            symlink(&hook_source, &hook_target).unwrap();
        },
        || capability::symlink_entry_wal(&intent.target, &source, &plan, &mut guard),
    )
    .unwrap();
    assert!(effect.recovery_required, "{effect:?}");
    assert_eq!(guard.phase(), WalPhase::Prepared);
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    let item = scan.items.first().unwrap();
    assert_eq!(
        item.classification,
        super::super::WriteAuthorityRecoveryClassification::PartialNamespaceCreation
    );
    assert!(!item.automatic_recovery_available);
    assert_eq!(
        item.available_resolution_actions,
        vec![WriteAuthorityRecoveryResolutionAction::AcceptCurrentState]
    );
    assert_eq!(fs::read_link(&target).unwrap(), source);
    assert_eq!(fs::read_link(&displaced).unwrap(), source);
    fixture.cleanup();
}

#[test]
fn symlink_v2_checkpointed_crash_auto_finalizes_idempotently() {
    let fixture = AtomicRecoveryFixture::new("symlink-v2-checkpoint-finalize", false);
    let target = fixture.parent.join("link");
    let source = PathBuf::from("desired");
    let (coordinator, intent, plan, record) =
        fixture.prepare_symlink_v2("wal-symlink-v2-checkpoint-finalize", &target, &source, true);
    let mut guard = coordinator.begin(record).unwrap();
    let crashed = catch_unwind(AssertUnwindSafe(|| {
        capability::with_after_symlink_v2_checkpoint_hook_for_test(
            || panic!("simulated crash after Symlink v2 checkpoint"),
            || capability::symlink_entry_wal(&intent.target, &source, &plan, &mut guard),
        )
    }));
    assert!(crashed.is_err());
    assert_eq!(guard.phase(), WalPhase::AuxiliaryDurable);
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    assert!(!restarted.snapshot().unwrap().blocked);
    assert_eq!(fs::read_link(&target).unwrap(), source);
    drop(restarted);
    let restarted_again = fixture.restart_coordinator();
    assert!(!restarted_again.snapshot().unwrap().blocked);
    assert_eq!(fs::read_link(&target).unwrap(), source);
    fixture.cleanup();
}

#[test]
fn symlink_v2_checkpointed_replacement_stays_conflict_and_preserved() {
    let fixture = AtomicRecoveryFixture::new("symlink-v2-checkpoint-replace", false);
    let target = fixture.parent.join("link");
    let displaced = fixture.parent.join("checkpointed-original");
    let source = PathBuf::from("desired");
    let (coordinator, intent, plan, record) =
        fixture.prepare_symlink_v2("wal-symlink-v2-checkpoint-replace", &target, &source, true);
    let mut guard = coordinator.begin(record).unwrap();
    let hook_target = target.clone();
    let hook_displaced = displaced.clone();
    let hook_source = source.clone();
    let effect = capability::with_after_symlink_v2_checkpoint_hook_for_test(
        move || {
            fs::rename(&hook_target, &hook_displaced).unwrap();
            symlink(&hook_source, &hook_target).unwrap();
        },
        || capability::symlink_entry_wal(&intent.target, &source, &plan, &mut guard),
    )
    .unwrap();
    assert!(effect.recovery_required, "{effect:?}");
    assert_eq!(guard.phase(), WalPhase::AuxiliaryDurable);
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::Conflict
    );
    assert!(scan.items[0].available_resolution_actions.is_empty());
    assert_eq!(fs::read_link(&target).unwrap(), source);
    assert_eq!(fs::read_link(&displaced).unwrap(), source);
    fixture.cleanup();
}

#[test]
fn symlink_v2_parent_fsync_failure_keeps_prepared_target_hot() {
    let fixture = AtomicRecoveryFixture::new("symlink-v2-parent-fsync", false);
    let target = fixture.parent.join("link");
    let source = PathBuf::from("desired");
    let (coordinator, intent, plan, record) =
        fixture.prepare_symlink_v2("wal-symlink-v2-parent-fsync", &target, &source, true);
    let mut guard = coordinator.begin(record).unwrap();
    let effect = capability::with_directory_sync_failure_for_test(|| {
        capability::symlink_entry_wal(&intent.target, &source, &plan, &mut guard)
    })
    .unwrap();
    assert!(effect.recovery_required, "{effect:?}");
    assert_eq!(guard.phase(), WalPhase::Prepared);
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::PartialNamespaceCreation
    );
    assert_eq!(fs::read_link(&target).unwrap(), source);
    fixture.cleanup();
}

#[test]
fn recovery_treats_no_effect_shape_as_conflict_after_effect_visible_phase() {
    let fixture = AtomicRecoveryFixture::new("atomic-phase-no-effect", false);
    let (coordinator, plan, record) = fixture.prepare("wal-atomic-phase-no-effect", b"replacement");
    let mut guard = coordinator.begin(record).unwrap();
    guard.mark_auxiliary_durable().unwrap();
    guard.mark_effect_visible().unwrap();
    drop(plan);
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::Conflict
    );
    assert!(!scan.items[0].automatic_recovery_available);
    assert!(scan.items[0].diagnostic.contains("EffectVisible"));
    fixture.cleanup();
}

#[test]
fn restart_legacy_copy_recovery_preserves_staged_temp_without_causal_identity() {
    let fixture = AtomicRecoveryFixture::new("copy-staged", false);
    let source = fixture.root.join("source.bin");
    fs::write(&source, b"staged payload").unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
    let (coordinator, _intent, plan, record) = fixture.prepare_legacy_copy(
        "wal-copy-staged",
        &fixture.target,
        &source,
        CapabilityReplacePolicy::Replace,
    );
    let temp = fixture.parent.join(plan.temp_leaf().unwrap());
    let guard = coordinator.begin(record).unwrap();
    fs::copy(&source, &temp).unwrap();
    fs::set_permissions(&temp, fs::Permissions::from_mode(0o600)).unwrap();
    fs::remove_file(&source).unwrap();
    drop(plan);
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::StagedOnly
    );
    assert!(!scan.items[0].automatic_recovery_available);
    assert!(temp.exists());
    assert!(!fixture.target.exists());
    fixture.cleanup();
}

#[test]
fn restart_legacy_copy_recovery_preserves_committed_target_without_causal_identity() {
    let fixture = AtomicRecoveryFixture::new("copy-committed", false);
    let source = fixture.root.join("source.bin");
    fs::write(&source, b"committed payload").unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o644)).unwrap();
    let (coordinator, _intent, plan, record) = fixture.prepare_legacy_copy(
        "wal-copy-committed",
        &fixture.target,
        &source,
        CapabilityReplacePolicy::Replace,
    );
    let guard = coordinator.begin(record).unwrap();
    fs::copy(&source, &fixture.target).unwrap();
    fs::set_permissions(&fixture.target, fs::Permissions::from_mode(0o644)).unwrap();
    fs::remove_file(&source).unwrap();
    drop(plan);
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::EffectCommitted
    );
    assert!(!scan.items[0].automatic_recovery_available);
    assert_eq!(fs::read(&fixture.target).unwrap(), b"committed payload");
    fixture.cleanup();
}

#[test]
fn restart_legacy_copy_recovery_preserves_exchange_baseline_for_manual_review() {
    let fixture = AtomicRecoveryFixture::new("copy-exchange", true);
    fs::set_permissions(&fixture.target, fs::Permissions::from_mode(0o600)).unwrap();
    let source = fixture.root.join("source.bin");
    fs::write(&source, b"replacement payload").unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o640)).unwrap();
    let (coordinator, _intent, plan, record) = fixture.prepare_legacy_copy(
        "wal-copy-exchange",
        &fixture.target,
        &source,
        CapabilityReplacePolicy::Replace,
    );
    let temp = fixture.parent.join(plan.temp_leaf().unwrap());
    let guard = coordinator.begin(record).unwrap();
    fs::rename(&fixture.target, &temp).unwrap();
    fs::copy(&source, &fixture.target).unwrap();
    fs::set_permissions(&fixture.target, fs::Permissions::from_mode(0o640)).unwrap();
    fs::remove_file(&source).unwrap();
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
    assert!(!scan.items[0].automatic_recovery_available);
    assert_eq!(fs::read(&fixture.target).unwrap(), b"replacement payload");
    assert_eq!(fs::read(&temp).unwrap(), b"baseline");
    fixture.cleanup();
}

#[test]
fn restart_copy_recovery_classifies_missing_baseline_parent_as_conflict() {
    let fixture = AtomicRecoveryFixture::new("copy-missing-baseline-parent", false);
    let source = fixture.root.join("source.bin");
    let moved_parent = fixture.boundary.join("nested-moved");
    fs::write(&source, b"payload").unwrap();
    let (coordinator, _intent, plan, record) = fixture.prepare_copy(
        "wal-copy-missing-baseline-parent",
        &fixture.target,
        &source,
        CapabilityReplacePolicy::Replace,
    );
    let guard = coordinator.begin(record).unwrap();
    fs::rename(&fixture.parent, &moved_parent).unwrap();
    drop(plan);
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::Conflict
    );
    assert!(moved_parent.is_dir());
    fixture.cleanup();
}

#[test]
fn copy_v2_directory_sync_failure_is_finalized_from_checkpoint_on_restart() {
    let fixture = AtomicRecoveryFixture::new("copy-temp-sync-failure", false);
    let source = fixture.root.join("source.bin");
    fs::write(&source, b"staged after sync failure").unwrap();
    let (coordinator, intent, plan, record) = fixture.prepare_copy(
        "wal-copy-temp-sync-failure",
        &fixture.target,
        &source,
        CapabilityReplacePolicy::Replace,
    );
    let temp = fixture.parent.join(plan.temp_leaf().unwrap());
    let mut guard = coordinator.begin(record).unwrap();
    let effect = capability::with_directory_sync_failure_for_test(|| {
        capability::copy_file_wal(
            &intent.target,
            &source,
            CapabilityReplacePolicy::Replace,
            plan,
            &mut guard,
        )
    })
    .unwrap();
    assert!(effect.recovery_required, "{effect:?}");
    assert!(!temp.exists());
    assert_eq!(
        fs::read(&fixture.target).unwrap(),
        b"staged after sync failure"
    );
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(!scan.blocked, "{scan:?}");
    assert!(!temp.exists());
    assert_eq!(
        fs::read(&fixture.target).unwrap(),
        b"staged after sync failure"
    );
    fixture.cleanup();
}

#[test]
fn restart_copy_recovery_auto_clears_only_exact_no_effect() {
    let fixture = AtomicRecoveryFixture::new("copy-no-effect", false);
    let source = fixture.root.join("source.bin");
    fs::write(&source, b"no effect payload").unwrap();
    let (coordinator, _intent, plan, record) = fixture.prepare_copy(
        "wal-copy-no-effect",
        &fixture.target,
        &source,
        CapabilityReplacePolicy::Replace,
    );
    let guard = coordinator.begin(record).unwrap();
    fs::remove_file(&source).unwrap();
    drop(plan);
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(!scan.blocked, "{scan:?}");
    assert!(!fixture.target.exists());
    fixture.cleanup();
}

#[test]
fn restart_rename_recovery_clears_exact_prepared_no_effect() {
    let fixture = AtomicRecoveryFixture::new("rename-no-effect", true);
    let destination = fixture.parent.join("renamed.txt");
    let (coordinator, _intent, _destination, plan, record) =
        fixture.prepare_rename("wal-rename-no-effect", &destination);
    let guard = coordinator.begin(record).unwrap();
    drop(plan);
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(!scan.blocked, "{scan:?}");
    assert_eq!(fs::read(&fixture.target).unwrap(), b"baseline");
    assert!(!destination.exists());
    fixture.cleanup();
}

#[test]
fn restart_rename_recovery_finalizes_exact_committed_inode() {
    let fixture = AtomicRecoveryFixture::new("rename-committed", true);
    let destination = fixture.parent.join("renamed.txt");
    let (coordinator, _intent, _destination, plan, record) =
        fixture.prepare_rename("wal-rename-committed", &destination);
    let guard = coordinator.begin(record).unwrap();
    fs::rename(&fixture.target, &destination).unwrap();
    drop(plan);
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(!scan.blocked, "{scan:?}");
    assert!(!fixture.target.exists());
    assert_eq!(fs::read(&destination).unwrap(), b"baseline");
    fixture.cleanup();
}

#[test]
fn restart_rename_recovery_preserves_partial_destination_namespace() {
    let fixture = AtomicRecoveryFixture::new("rename-partial-parent", true);
    let destination = fixture.boundary.join("new/deep/renamed.txt");
    let (coordinator, _intent, _destination, plan, record) =
        fixture.prepare_rename("wal-rename-partial-parent", &destination);
    let guard = coordinator.begin(record).unwrap();
    fs::create_dir(fixture.boundary.join("new")).unwrap();
    drop(plan);
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::PartialNamespaceCreation
    );
    assert!(fixture.boundary.join("new").is_dir());
    assert_eq!(fs::read(&fixture.target).unwrap(), b"baseline");
    fixture.cleanup();
}

#[test]
fn runtime_rename_reaches_target_durable_and_restart_closes_hot_record() {
    let fixture = AtomicRecoveryFixture::new("rename-target-durable", true);
    let destination_path = fixture.boundary.join("new/deep/renamed.txt");
    let (coordinator, intent, destination, plan, record) =
        fixture.prepare_rename("wal-rename-target-durable", &destination_path);
    let mut guard = coordinator.begin(record).unwrap();
    let effect =
        capability::rename_entry_wal(&intent.target, &destination, plan, &mut guard).unwrap();
    assert!(effect.changed, "{effect:?}");
    assert!(!effect.recovery_required, "{effect:?}");
    assert_eq!(guard.phase(), WalPhase::TargetDurable);
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(!scan.blocked, "{scan:?}");
    assert!(!fixture.target.exists());
    assert_eq!(fs::read(&destination_path).unwrap(), b"baseline");
    fixture.cleanup();
}

#[test]
fn restart_remove_leaf_recovery_clears_exact_prepared_no_effect() {
    let fixture = AtomicRecoveryFixture::new("remove-prepared-no-effect", true);
    let (coordinator, _intent, plan, record) =
        fixture.prepare_remove_leaf("wal-remove-prepared-no-effect");
    let guard = coordinator.begin(record).unwrap();
    drop(plan);
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(!scan.blocked, "{scan:?}");
    assert_eq!(fs::read(&fixture.target).unwrap(), b"baseline");
    fixture.cleanup();
}

#[test]
fn restart_remove_leaf_recovery_finalizes_only_absence_after_effect_visible() {
    let fixture = AtomicRecoveryFixture::new("remove-effect-committed", true);
    let (coordinator, _intent, plan, record) =
        fixture.prepare_remove_leaf("wal-remove-effect-committed");
    let quarantine = fixture
        .parent
        .join(super::super::decode_component_hex(&plan.evidence.quarantine_leaf_hex).unwrap());
    let mut guard = coordinator.begin(record).unwrap();
    guard.mark_auxiliary_durable().unwrap();
    fs::rename(&fixture.target, &quarantine).unwrap();
    guard.mark_effect_visible().unwrap();
    fs::remove_file(&quarantine).unwrap();
    drop(plan);
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(!scan.blocked, "{scan:?}");
    assert!(!fixture.target.exists());
    assert!(!quarantine.exists());
    fixture.cleanup();
}

#[test]
fn runtime_remove_leaf_reaches_target_durable() {
    let fixture = AtomicRecoveryFixture::new("remove-target-durable", true);
    let (coordinator, intent, plan, record) =
        fixture.prepare_remove_leaf("wal-remove-target-durable");
    let mut guard = coordinator.begin(record).unwrap();
    let effect = capability::remove_leaf_wal(&intent.target, plan, &mut guard).unwrap();
    assert!(effect.changed, "{effect:?}");
    assert!(!effect.recovery_required, "{effect:?}");
    assert_eq!(guard.phase(), WalPhase::TargetDurable);
    assert!(!fixture.target.exists());
    drop(guard);
    fixture.cleanup();
}

#[test]
fn runtime_remove_leaf_detects_name_recreated_before_target_durable() {
    let fixture = AtomicRecoveryFixture::new("remove-recreated-target", true);
    let target = fixture.target.clone();
    let (coordinator, intent, plan, record) =
        fixture.prepare_remove_leaf("wal-remove-recreated-target");
    let mut guard = coordinator.begin(record).unwrap();
    let effect = capability::with_before_remove_leaf_target_durable_hook_for_test(
        move || fs::write(&target, b"competitor").unwrap(),
        || capability::remove_leaf_wal(&intent.target, plan, &mut guard),
    )
    .unwrap();
    assert!(effect.recovery_required, "{effect:?}");
    assert_eq!(guard.phase(), WalPhase::TargetDurable);
    assert_eq!(fs::read(&fixture.target).unwrap(), b"competitor");
    drop(guard);
    fixture.cleanup();
}

#[test]
fn runtime_remove_leaf_unlinks_symlink_without_following_target() {
    let fixture = AtomicRecoveryFixture::new("remove-symlink", false);
    let external = fixture.root.join("external.txt");
    fs::write(&external, b"outside").unwrap();
    symlink(&external, &fixture.target).unwrap();
    let (coordinator, intent, plan, record) =
        fixture.prepare_remove_leaf_unchecked("wal-remove-symlink");
    let mut guard = coordinator.begin(record).unwrap();
    let effect = capability::remove_leaf_wal(&intent.target, plan, &mut guard).unwrap();
    assert!(effect.changed, "{effect:?}");
    assert!(!fixture.target.exists());
    assert_eq!(fs::read(&external).unwrap(), b"outside");
    fixture.cleanup();
}

#[test]
fn runtime_remove_leaf_handles_fifo_without_blocking() {
    let fixture = AtomicRecoveryFixture::new("remove-fifo", false);
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &fixture.target,
        rustix::fs::Mode::from_raw_mode(0o600),
    )
    .unwrap();
    let (coordinator, intent, plan, record) =
        fixture.prepare_remove_leaf_unchecked("wal-remove-fifo");
    let mut guard = coordinator.begin(record).unwrap();
    let effect = capability::remove_leaf_wal(&intent.target, plan, &mut guard).unwrap();
    assert!(effect.changed, "{effect:?}");
    assert!(!fixture.target.exists());
    fixture.cleanup();
}

#[test]
fn runtime_remove_tree_reaches_target_durable_without_following_symlinks() {
    let fixture = AtomicRecoveryFixture::new("remove-tree-target-durable", false);
    fixture.create_tree();
    let external = fixture.root.join("external.txt");
    fs::write(&external, b"outside").unwrap();
    symlink(&external, fixture.target.join("nested/external-link")).unwrap();
    let non_utf8 = fixture
        .target
        .join(OsString::from_vec(vec![b'n', 0xff, b'x']));
    fs::write(non_utf8, b"lossless-name").unwrap();
    let (coordinator, intent, plan, record) =
        fixture.prepare_remove_tree("wal-remove-tree-target-durable");
    let mut guard = coordinator.begin(record).unwrap();

    let effect = capability::remove_tree_wal(&intent.target, plan, &mut guard).unwrap();

    assert!(effect.changed, "{effect:?}");
    assert!(!effect.recovery_required, "{effect:?}");
    assert_eq!(guard.phase(), WalPhase::TargetDurable);
    assert!(!fixture.target.exists());
    assert_eq!(fs::read(&external).unwrap(), b"outside");
    fixture.cleanup();
}

#[test]
fn runtime_remove_tree_stops_before_deleting_unplanned_quarantine_child() {
    let fixture = AtomicRecoveryFixture::new("remove-tree-unplanned-child", false);
    fixture.create_tree();
    let (coordinator, intent, plan, record) =
        fixture.prepare_remove_tree("wal-remove-tree-unplanned-child");
    let quarantine = fixture
        .parent
        .join(super::super::decode_component_hex(&plan.evidence.quarantine_leaf_hex).unwrap());
    let hook_quarantine = quarantine.clone();
    let mut guard = coordinator.begin(record).unwrap();

    let effect = capability::with_before_remove_tree_traversal_hook_for_test(
        move || fs::write(hook_quarantine.join("competitor.txt"), b"competitor").unwrap(),
        || capability::remove_tree_wal(&intent.target, plan, &mut guard),
    )
    .unwrap();

    assert!(effect.recovery_required, "{effect:?}");
    assert_eq!(guard.phase(), WalPhase::EffectVisible);
    assert!(!fixture.target.exists());
    assert_eq!(
        fs::read(quarantine.join("competitor.txt")).unwrap(),
        b"competitor"
    );
    assert_eq!(fs::read(quarantine.join("a.txt")).unwrap(), b"a");
    drop(guard);
    let scan = coordinator.rescan_and_recover_exclusive().unwrap();
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::PartialTreeRemoval,
        "{scan:?}"
    );
    fixture.cleanup();
}

#[test]
fn runtime_remove_tree_rechecks_tree_after_quarantine_rename() {
    let fixture = AtomicRecoveryFixture::new("remove-tree-post-rename-recheck", false);
    fixture.create_tree();
    let target = fixture.target.clone();
    let (coordinator, intent, plan, record) =
        fixture.prepare_remove_tree("wal-remove-tree-post-rename-recheck");
    let quarantine = fixture
        .parent
        .join(super::super::decode_component_hex(&plan.evidence.quarantine_leaf_hex).unwrap());
    let mut guard = coordinator.begin(record).unwrap();

    let effect = capability::with_before_remove_tree_quarantine_hook_for_test(
        move || fs::write(target.join("late.txt"), b"late").unwrap(),
        || capability::remove_tree_wal(&intent.target, plan, &mut guard),
    )
    .unwrap();

    assert!(effect.recovery_required, "{effect:?}");
    assert_eq!(guard.phase(), WalPhase::EffectVisible);
    assert!(!fixture.target.exists());
    assert_eq!(fs::read(quarantine.join("late.txt")).unwrap(), b"late");
    assert_eq!(fs::read(quarantine.join("a.txt")).unwrap(), b"a");
    fixture.cleanup();
}

#[test]
fn runtime_remove_tree_preserves_recreated_public_target() {
    let fixture = AtomicRecoveryFixture::new("remove-tree-recreated-target", false);
    fixture.create_tree();
    let target = fixture.target.clone();
    let (coordinator, intent, plan, record) =
        fixture.prepare_remove_tree("wal-remove-tree-recreated-target");
    let mut guard = coordinator.begin(record).unwrap();

    let effect = capability::with_before_remove_tree_target_durable_hook_for_test(
        move || {
            fs::create_dir(&target).unwrap();
            fs::write(target.join("competitor.txt"), b"competitor").unwrap();
        },
        || capability::remove_tree_wal(&intent.target, plan, &mut guard),
    )
    .unwrap();

    assert!(effect.recovery_required, "{effect:?}");
    assert_eq!(guard.phase(), WalPhase::TargetDurable);
    assert_eq!(
        fs::read(fixture.target.join("competitor.txt")).unwrap(),
        b"competitor"
    );
    fixture.cleanup();
}

#[test]
fn restart_remove_tree_recovery_clears_exact_prepared_no_effect() {
    let fixture = AtomicRecoveryFixture::new("remove-tree-prepared", false);
    fixture.create_tree();
    let (coordinator, _intent, plan, record) =
        fixture.prepare_remove_tree("wal-remove-tree-prepared");
    let guard = coordinator.begin(record).unwrap();
    drop(plan);
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    assert!(!restarted.snapshot().unwrap().blocked);
    assert_eq!(fs::read(fixture.target.join("a.txt")).unwrap(), b"a");
    fixture.cleanup();
}

#[test]
fn restart_remove_tree_recovery_finalizes_absence_after_effect_visible() {
    let fixture = AtomicRecoveryFixture::new("remove-tree-effect-committed", false);
    fixture.create_tree();
    let (coordinator, _intent, plan, record) =
        fixture.prepare_remove_tree("wal-remove-tree-effect-committed");
    let quarantine = fixture
        .parent
        .join(super::super::decode_component_hex(&plan.evidence.quarantine_leaf_hex).unwrap());
    let mut guard = coordinator.begin(record).unwrap();
    guard.mark_auxiliary_durable().unwrap();
    fs::rename(&fixture.target, &quarantine).unwrap();
    guard.mark_effect_visible().unwrap();
    fs::remove_dir_all(&quarantine).unwrap();
    drop(plan);
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    assert!(!restarted.snapshot().unwrap().blocked);
    assert!(!fixture.target.exists());
    fixture.cleanup();
}

#[test]
fn append_v2_present_runtime_crash_matrix_is_restart_idempotent() {
    for checkpoint in [
        AppendV2CrashCheckpoint::Checkpoint,
        AppendV2CrashCheckpoint::EffectBeforePhase,
        AppendV2CrashCheckpoint::TargetFsync,
        AppendV2CrashCheckpoint::TargetDurable,
    ] {
        run_append_v2_crash_case(true, checkpoint);
    }
}

#[test]
fn append_v2_absent_runtime_crash_matrix_is_restart_idempotent() {
    for checkpoint in [
        AppendV2CrashCheckpoint::Checkpoint,
        AppendV2CrashCheckpoint::EffectBeforePhase,
        AppendV2CrashCheckpoint::TargetFsync,
        AppendV2CrashCheckpoint::TargetDurable,
    ] {
        run_append_v2_crash_case(false, checkpoint);
    }
}
