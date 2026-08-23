use super::{fixtures::*, *};

#[test]
fn prepared_atomic_staged_competitor_remains_hot_and_untouched() {
    let fixture = AtomicRecoveryFixture::new("staged", false);
    let payload = b"new payload";
    let (coordinator, plan, record) = fixture.prepare("wal-staged-op", payload);
    let guard = coordinator.begin(record).unwrap();
    let temp = fixture.parent.join(plan.temp_leaf().unwrap());
    fs::write(&temp, payload).unwrap();
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
    assert_eq!(fs::read(&temp).unwrap(), payload);
    assert!(!fixture.target.exists());
    fixture.cleanup();
}

#[test]
fn prepared_atomic_target_competitor_is_never_adopted() {
    let fixture = AtomicRecoveryFixture::new("create-committed", false);
    let payload = b"committed create";
    let (coordinator, _plan, record) = fixture.prepare("wal-create-op", payload);
    let guard = coordinator.begin(record).unwrap();
    fs::write(&fixture.target, payload).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::Conflict
    );
    assert_eq!(fs::read(&fixture.target).unwrap(), payload);
    fixture.cleanup();
}

#[test]
fn prepared_atomic_exchange_shape_preserves_target_and_baseline_competitors() {
    let fixture = AtomicRecoveryFixture::new("replace-exchange", true);
    let payload = b"replacement";
    let (coordinator, plan, record) = fixture.prepare("wal-replace-op", payload);
    let guard = coordinator.begin(record).unwrap();
    let temp = fixture.parent.join(plan.temp_leaf().unwrap());
    fs::rename(&fixture.target, &temp).unwrap();
    fs::write(&fixture.target, payload).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::Conflict
    );
    assert_eq!(fs::read(&fixture.target).unwrap(), payload);
    assert_eq!(fs::read(&temp).unwrap(), b"baseline");
    fixture.cleanup();
}

#[test]
fn corrupt_preparing_record_blocks_without_destructive_cleanup() {
    let fixture = AtomicRecoveryFixture::new("corrupt", false);
    fs::write(fixture.wal.join("corrupt-op.preparing"), b"{torn").unwrap();
    let coordinator = fixture.restart_coordinator();
    let scan = coordinator.snapshot().unwrap();
    assert!(scan.blocked);
    assert_eq!(scan.record_count, 1);
    assert!(fixture.wal.join("corrupt-op.preparing").exists());
    fixture.cleanup();
}

#[test]
fn unknown_regular_file_never_makes_wal_look_clean() {
    let fixture = AtomicRecoveryFixture::new("unknown-regular-poison", false);
    fs::write(fixture.wal.join("unknown-entry"), b"competitor").unwrap();

    let coordinator = fixture.restart_coordinator();
    let scan = coordinator.snapshot().unwrap();

    assert!(scan.blocked, "{scan:?}");
    assert!(fixture.wal.join("unknown-entry").is_file());
    drop(coordinator);
    fixture.cleanup();
}

#[test]
fn unknown_symlink_never_makes_wal_look_clean() {
    let fixture = AtomicRecoveryFixture::new("unknown-symlink-poison", false);
    let outside = fixture.root.join("outside-archive");
    fs::create_dir_all(&outside).unwrap();
    symlink(&outside, fixture.wal.join("unknown-entry")).unwrap();

    let coordinator = fixture.restart_coordinator();
    let scan = coordinator.snapshot().unwrap();

    assert!(scan.blocked, "{scan:?}");
    assert!(outside.is_dir());
    drop(coordinator);
    fixture.cleanup();
}

#[test]
fn prepared_append_partial_competitor_remains_hot_and_untruncated() {
    let fixture = AtomicRecoveryFixture::new("append-partial", true);
    let payload = b"-append-payload";
    let (coordinator, record) = fixture.prepare_append("wal-append-partial", payload);
    let guard = coordinator.begin(record).unwrap();
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
        super::super::WriteAuthorityRecoveryClassification::Conflict
    );
    assert_eq!(fs::read(&fixture.target).unwrap(), b"baseline-append");
    fixture.cleanup();
}

#[test]
fn prepared_append_complete_competitor_is_never_adopted() {
    let fixture = AtomicRecoveryFixture::new("append-complete", true);
    let payload = b"-complete";
    let (coordinator, record) = fixture.prepare_append("wal-append-complete", payload);
    let guard = coordinator.begin(record).unwrap();
    OpenOptions::new()
        .append(true)
        .open(&fixture.target)
        .unwrap()
        .write_all(payload)
        .unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::Conflict
    );
    assert_eq!(fs::read(&fixture.target).unwrap(), b"baseline-complete");
    fixture.cleanup();
}

#[test]
fn prepared_append_new_leaf_competitor_is_conflict_and_untouched() {
    let fixture = AtomicRecoveryFixture::new("append-new-partial", false);
    let payload = b"new-append";
    let (coordinator, record) = fixture.prepare_append("wal-append-new-partial", payload);
    let guard = coordinator.begin(record).unwrap();
    fs::write(&fixture.target, &payload[..3]).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked);
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::Conflict
    );
    assert_eq!(fs::read(&fixture.target).unwrap(), &payload[..3]);
    fixture.cleanup();
}

#[test]
fn restart_recovery_requires_manual_review_for_unattributed_partial_directory_suffix() {
    let fixture = AtomicRecoveryFixture::new("mkdir-partial", false);
    let target = fixture.parent.join("first/second/third");
    let (coordinator, _intent, _plan, record) =
        fixture.prepare_directory("wal-mkdir-partial", &target);
    let guard = coordinator.begin(record).unwrap();
    fs::create_dir(fixture.parent.join("first")).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::PartialNamespaceCreation
    );
    assert!(!target.exists());
    fixture.cleanup();
}

#[test]
fn restart_recovery_requires_manual_review_for_unattributed_complete_directory_suffix() {
    let fixture = AtomicRecoveryFixture::new("mkdir-complete", false);
    let target = fixture.parent.join("first/second");
    let (coordinator, _intent, _plan, record) =
        fixture.prepare_directory("wal-mkdir-complete", &target);
    let guard = coordinator.begin(record).unwrap();
    fs::create_dir_all(&target).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked);
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::EffectCommitted
    );
    assert!(target.is_dir());
    fixture.cleanup();
}

#[test]
fn directory_target_durable_then_disappeared_is_never_cleared() {
    let fixture = AtomicRecoveryFixture::new("mkdir-target-durable-gone", false);
    let target = fixture.parent.join("created");
    let (coordinator, _intent, _plan, record) =
        fixture.prepare_directory("wal-mkdir-target-durable-gone", &target);
    let mut guard = coordinator.begin(record).unwrap();
    fs::create_dir(&target).unwrap();
    guard.mark_auxiliary_durable().unwrap();
    guard.mark_effect_visible().unwrap();
    guard.mark_target_durable().unwrap();
    fs::remove_dir(&target).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(scan.items[0].phase, Some(WalPhase::TargetDurable));
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::Conflict
    );
    assert!(!scan.items[0].automatic_recovery_available);
    fixture.cleanup();
}

#[test]
fn directory_v2_rejects_a_missing_final_parent_before_wal() {
    let fixture = AtomicRecoveryFixture::new("mkdir-v2-missing-parent", false);
    let target = fixture.parent.join("missing/created");
    let authority = capability::capture_directory_authority(
        &fixture.boundary,
        "test/mkdir-v2-missing-parent",
        DirectoryAuthorityScope::ApplicationPreviewCache,
    )
    .unwrap();
    let target = WriteTarget::new(&target, &fixture.boundary, "test/mkdir-v2-missing-parent")
        .bind_authority(authority)
        .unwrap();
    let error = capability::plan_directory(&target).unwrap_err();
    assert!(error.contains("parent final existent"), "{error}");
    assert!(!fixture.parent.join("missing").exists());
    fixture.cleanup();
}

#[test]
fn directory_v2_plan_rejects_regular_file_and_symlink_targets() {
    let fixture = AtomicRecoveryFixture::new("mkdir-v2-invalid-targets", false);
    let regular = fixture.parent.join("regular");
    fs::write(&regular, b"not a directory").unwrap();
    let symlink_target = fixture.parent.join("link");
    symlink(&fixture.parent, &symlink_target).unwrap();

    for (label, path) in [("regular", regular), ("symlink", symlink_target)] {
        let authority = capability::capture_directory_authority(
            &fixture.boundary,
            "test/mkdir-v2-invalid-target",
            DirectoryAuthorityScope::ApplicationPreviewCache,
        )
        .unwrap();
        let target = WriteTarget::new(
            &path,
            &fixture.boundary,
            format!("test/mkdir-v2-invalid-{label}"),
        )
        .bind_authority(authority)
        .unwrap();
        let error = capability::plan_directory(&target).unwrap_err();
        assert!(error.contains("non-directory"), "{label}: {error}");
    }
    fixture.cleanup();
}

#[test]
fn directory_v2_precheckpoint_capture_rejects_replaced_target_inode() {
    let fixture = AtomicRecoveryFixture::new("mkdir-v2-precheckpoint-replace", false);
    let target = fixture.parent.join("created");
    let (coordinator, intent, plan, record) =
        fixture.prepare_directory_v2("wal-mkdir-v2-precheckpoint-replace", &target);
    let mut guard = coordinator.begin(record).unwrap();
    let hook_target = target.clone();
    let effect = capability::with_before_directory_v2_checkpoint_capture_hook_for_test(
        move || {
            fs::remove_dir(&hook_target).unwrap();
            fs::create_dir(&hook_target).unwrap();
        },
        || capability::create_directory_all_wal(&intent.target, &plan, &mut guard),
    )
    .unwrap();
    assert!(effect.recovery_required, "{effect:?}");
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
fn directory_v2_precheckpoint_capture_rejects_child_add_remove_state_change() {
    let fixture = AtomicRecoveryFixture::new("mkdir-v2-precheckpoint-child", false);
    let target = fixture.parent.join("created");
    let (coordinator, intent, plan, record) =
        fixture.prepare_directory_v2("wal-mkdir-v2-precheckpoint-child", &target);
    let child = target.join("foreign");
    let mut guard = coordinator.begin(record).unwrap();
    let hook_child = child.clone();
    let effect = capability::with_before_directory_v2_checkpoint_capture_hook_for_test(
        move || {
            fs::write(&hook_child, b"foreign").unwrap();
            fs::remove_file(&hook_child).unwrap();
        },
        || capability::create_directory_all_wal(&intent.target, &plan, &mut guard),
    )
    .unwrap();
    assert!(effect.recovery_required, "{effect:?}");
    assert_eq!(guard.phase(), WalPhase::Prepared);
    assert!(target.is_dir());
    assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    assert!(restarted.snapshot().unwrap().blocked);
    assert!(target.is_dir());
    fixture.cleanup();
}

#[test]
fn directory_v2_target_competitor_after_checkpoint_is_never_adopted_or_removed() {
    let fixture = AtomicRecoveryFixture::new("mkdir-v2-target-competitor", false);
    let target = fixture.parent.join("created");
    let (coordinator, intent, plan, record) =
        fixture.prepare_directory_v2("wal-mkdir-v2-target-competitor", &target);
    let mut guard = coordinator.begin(record).unwrap();
    let hook_target = target.clone();
    let effect = capability::with_after_directory_v2_checkpoint_hook_for_test(
        move || {
            fs::remove_dir(&hook_target).unwrap();
            fs::create_dir(&hook_target).unwrap();
        },
        || capability::create_directory_all_wal(&intent.target, &plan, &mut guard),
    )
    .unwrap();
    assert!(effect.recovery_required);
    assert!(target.is_dir());
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::Conflict
    );
    assert!(target.is_dir());
    fixture.cleanup();
}

#[test]
fn directory_v2_child_injected_into_target_or_returned_to_empty_is_conflict() {
    for returned_to_empty in [false, true] {
        let suffix = if returned_to_empty {
            "returned"
        } else {
            "present"
        };
        let fixture = AtomicRecoveryFixture::new(&format!("mkdir-v2-child-{suffix}"), false);
        let target = fixture.parent.join("created");
        let operation_id = format!("wal-mkdir-v2-child-{suffix}");
        let (coordinator, intent, plan, record) =
            fixture.prepare_directory_v2(&operation_id, &target);
        let child = target.join("foreign");
        let mut guard = coordinator.begin(record).unwrap();
        let hook_child = child.clone();
        let effect = capability::with_after_directory_v2_checkpoint_hook_for_test(
            move || {
                fs::write(&hook_child, b"foreign").unwrap();
                if returned_to_empty {
                    fs::remove_file(&hook_child).unwrap();
                }
            },
            || capability::create_directory_all_wal(&intent.target, &plan, &mut guard),
        )
        .unwrap();
        assert!(effect.recovery_required, "{suffix}: {effect:?}");
        assert!(target.is_dir());
        drop(guard);
        drop(coordinator);

        let restarted = fixture.restart_coordinator();
        let scan = restarted.snapshot().unwrap();
        assert!(scan.blocked, "{suffix}: {scan:?}");
        assert_eq!(
            scan.items[0].classification,
            super::super::WriteAuthorityRecoveryClassification::Conflict
        );
        assert!(target.is_dir());
        drop(restarted);
        fixture.cleanup();
    }
}

#[test]
fn directory_v2_child_injected_into_target_blocks_target_durable_and_recovery() {
    let fixture = AtomicRecoveryFixture::new("mkdir-v2-target-child", false);
    let target = fixture.parent.join("created");
    let (coordinator, intent, plan, record) =
        fixture.prepare_directory_v2("wal-mkdir-v2-target-child", &target);
    let mut guard = coordinator.begin(record).unwrap();
    let child = target.join("foreign");
    let hook_child = child.clone();
    let effect = capability::with_before_directory_target_durable_hook_for_test(
        move || fs::write(&hook_child, b"foreign").unwrap(),
        || capability::create_directory_all_wal(&intent.target, &plan, &mut guard),
    )
    .unwrap();
    assert!(effect.recovery_required);
    assert_eq!(fs::read(&child).unwrap(), b"foreign");
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::Conflict
    );
    assert_eq!(fs::read(&child).unwrap(), b"foreign");
    fixture.cleanup();
}

#[test]
fn directory_v2_full_path_postflight_rejects_replaced_parent() {
    let fixture = AtomicRecoveryFixture::new("mkdir-v2-full-path-parent", false);
    let target = fixture.parent.join("created");
    let relocated_parent = fixture.boundary.join("nested-relocated");
    let (coordinator, intent, plan, record) =
        fixture.prepare_directory_v2("wal-mkdir-v2-full-path-parent", &target);
    let mut guard = coordinator.begin(record).unwrap();
    let original_parent = fixture.parent.clone();
    let replacement_parent = fixture.parent.clone();
    let hook_relocated = relocated_parent.clone();
    let effect = capability::with_before_directory_target_durable_hook_for_test(
        move || {
            fs::rename(&original_parent, &hook_relocated).unwrap();
            fs::create_dir(&replacement_parent).unwrap();
        },
        || capability::create_directory_all_wal(&intent.target, &plan, &mut guard),
    )
    .unwrap();
    assert!(effect.recovery_required, "{effect:?}");
    assert_eq!(guard.phase(), WalPhase::EffectVisible);
    assert!(!target.exists());
    assert!(relocated_parent.join("created").is_dir());
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert!(!scan.items[0].automatic_recovery_available);
    drop(restarted);
    fixture.cleanup();
}

#[test]
fn directory_v2_noop_full_path_rejects_replaced_baseline() {
    let fixture = AtomicRecoveryFixture::new("mkdir-v2-noop-full-path", false);
    let target = fixture.parent.join("created");
    fs::create_dir(&target).unwrap();
    let (coordinator, intent, plan, record) =
        fixture.prepare_directory_v2("wal-mkdir-v2-noop-full-path", &target);
    let mut guard = coordinator.begin(record).unwrap();
    let hook_target = target.clone();
    let result = capability::with_before_directory_v2_noop_full_path_hook_for_test(
        move || {
            fs::remove_dir(&hook_target).unwrap();
            fs::create_dir(&hook_target).unwrap();
        },
        || capability::create_directory_all_wal(&intent.target, &plan, &mut guard),
    );
    let error = result.unwrap_err();
    assert!(
        error.contains("baseline") || error.contains("checkpointed"),
        "{error}"
    );
    assert_eq!(guard.phase(), WalPhase::Prepared);
    assert!(target.is_dir());
    guard.abort_no_effect().unwrap();
    fixture.cleanup();
}

#[test]
fn restart_recovery_requires_manual_review_for_unattributed_exact_symlink() {
    let fixture = AtomicRecoveryFixture::new("symlink-complete", false);
    let target = fixture.parent.join("link");
    let source = PathBuf::from("../dangling-target");
    let (coordinator, _intent, _plan, record) =
        fixture.prepare_symlink("wal-symlink-complete", &target, &source);
    let guard = coordinator.begin(record).unwrap();
    std::os::unix::fs::symlink(&source, &target).unwrap();
    drop(guard);
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
fn symlink_target_durable_then_disappeared_is_never_cleared() {
    let fixture = AtomicRecoveryFixture::new("symlink-target-durable-gone", false);
    let target = fixture.parent.join("link");
    let source = PathBuf::from("dangling");
    let (coordinator, _intent, _plan, record) =
        fixture.prepare_symlink("wal-symlink-target-durable-gone", &target, &source);
    let mut guard = coordinator.begin(record).unwrap();
    symlink(&source, &target).unwrap();
    guard.mark_auxiliary_durable().unwrap();
    guard.mark_effect_visible().unwrap();
    guard.mark_target_durable().unwrap();
    fs::remove_file(&target).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(scan.items[0].phase, Some(WalPhase::TargetDurable));
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::Conflict
    );
    assert!(!scan.items[0].automatic_recovery_available);
    fixture.cleanup();
}

#[test]
fn symlink_plan_rejects_existing_non_symlink_without_mutation() {
    let fixture = AtomicRecoveryFixture::new("symlink-existing-file", false);
    let target_path = fixture.parent.join("link");
    fs::write(&target_path, b"sentinel").unwrap();
    let authority = capability::capture_directory_authority(
        &fixture.boundary,
        "test/symlink-existing-file",
        DirectoryAuthorityScope::ApplicationPreviewCache,
    )
    .unwrap();
    let target = WriteTarget::new(
        &target_path,
        &fixture.boundary,
        "test/symlink-existing-file",
    )
    .bind_authority(authority)
    .unwrap();
    let error = capability::plan_symlink(&target, Path::new("desired")).unwrap_err();
    assert!(error.contains("alt tip"), "{error}");
    assert_eq!(fs::read(&target_path).unwrap(), b"sentinel");
    fixture.cleanup();
}

#[test]
fn directory_postflight_detects_target_moved_before_target_durable() {
    let fixture = AtomicRecoveryFixture::new("mkdir-postflight-move", false);
    let target = fixture.parent.join("created");
    let moved = fixture.parent.join("moved");
    let (coordinator, intent, plan, record) =
        fixture.prepare_directory("wal-mkdir-postflight-move", &target);
    let mut guard = coordinator.begin(record).unwrap();
    let hook_target = target.clone();
    let hook_moved = moved.clone();
    let effect = capability::with_before_directory_target_durable_hook_for_test(
        move || fs::rename(&hook_target, &hook_moved).unwrap(),
        || capability::create_directory_all_wal(&intent.target, &plan, &mut guard),
    )
    .unwrap();
    assert!(effect.recovery_required);
    drop(guard);
    assert!(coordinator.snapshot().unwrap().blocked);
    assert!(!target.exists());
    assert!(moved.is_dir());
    fixture.cleanup();
}

#[test]
fn symlink_postflight_detects_leaf_removed_before_target_durable() {
    let fixture = AtomicRecoveryFixture::new("symlink-postflight-remove", false);
    let target = fixture.parent.join("link");
    let source = PathBuf::from("dangling");
    let (coordinator, intent, plan, record) =
        fixture.prepare_symlink("wal-symlink-postflight-remove", &target, &source);
    let mut guard = coordinator.begin(record).unwrap();
    let hook_target = target.clone();
    let effect = capability::with_before_symlink_target_durable_hook_for_test(
        move || fs::remove_file(&hook_target).unwrap(),
        || capability::symlink_entry_wal(&intent.target, &source, &plan, &mut guard),
    )
    .unwrap();
    assert!(effect.recovery_required);
    drop(guard);
    assert!(coordinator.snapshot().unwrap().blocked);
    assert!(!target.exists());
    fixture.cleanup();
}

#[test]
fn symlink_v2_runtime_rejects_post_plan_competitor_without_mutation() {
    let fixture = AtomicRecoveryFixture::new("symlink-v2-post-plan-competitor", false);
    let target = fixture.parent.join("link");
    let source = PathBuf::from("desired");
    let (coordinator, intent, plan, record) = fixture.prepare_symlink_v2(
        "wal-symlink-v2-post-plan-competitor",
        &target,
        &source,
        true,
    );
    symlink("competitor", &target).unwrap();
    let mut guard = coordinator.begin(record).unwrap();
    let error =
        capability::symlink_entry_wal(&intent.target, &source, &plan, &mut guard).unwrap_err();
    assert!(error.contains("a apărut după planificare"), "{error}");
    guard.abort_no_effect().unwrap();
    assert_eq!(fs::read_link(&target).unwrap(), PathBuf::from("competitor"));
    fixture.cleanup();
}

#[test]
fn symlink_v2_checkpoint_capture_rejects_same_literal_replacement() {
    let fixture = AtomicRecoveryFixture::new("symlink-v2-checkpoint-capture-replace", false);
    let target = fixture.parent.join("link");
    let displaced = fixture.parent.join("created-before-checkpoint-capture");
    let source = PathBuf::from("desired");
    let (coordinator, intent, plan, record) = fixture.prepare_symlink_v2(
        "wal-symlink-v2-checkpoint-capture-replace",
        &target,
        &source,
        true,
    );
    let mut guard = coordinator.begin(record).unwrap();
    let hook_target = target.clone();
    let hook_displaced = displaced.clone();
    let hook_source = source.clone();
    let effect = capability::with_before_symlink_v2_checkpoint_capture_hook_for_test(
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
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::PartialNamespaceCreation
    );
    assert!(!scan.items[0].automatic_recovery_available);
    assert_eq!(
        scan.items[0].available_resolution_actions,
        vec![WriteAuthorityRecoveryResolutionAction::AcceptCurrentState]
    );
    assert_eq!(fs::read_link(&target).unwrap(), source);
    assert_eq!(fs::read_link(&displaced).unwrap(), source);
    fixture.cleanup();
}

#[test]
fn symlink_v2_noop_full_path_rejects_same_literal_replacement() {
    let fixture = AtomicRecoveryFixture::new("symlink-v2-noop-full-path-replace", false);
    let target = fixture.parent.join("link");
    let displaced = fixture.parent.join("planned-baseline-link");
    let source = PathBuf::from("desired");
    symlink(&source, &target).unwrap();
    let (coordinator, intent, plan, record) = fixture.prepare_symlink_v2(
        "wal-symlink-v2-noop-full-path-replace",
        &target,
        &source,
        false,
    );
    let mut guard = coordinator.begin(record).unwrap();
    let hook_target = target.clone();
    let hook_displaced = displaced.clone();
    let hook_source = source.clone();
    let error = capability::with_before_symlink_v2_noop_full_path_hook_for_test(
        move || {
            fs::rename(&hook_target, &hook_displaced).unwrap();
            symlink(&hook_source, &hook_target).unwrap();
        },
        || capability::symlink_entry_wal(&intent.target, &source, &plan, &mut guard),
    )
    .unwrap_err();
    assert!(error.contains("full-path target diferă"), "{error}");
    assert_eq!(guard.phase(), WalPhase::Prepared);
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
fn symlink_v2_full_path_rejects_parent_replacement_after_effect_visible() {
    let fixture = AtomicRecoveryFixture::new("symlink-v2-parent-full-path-replace", false);
    let target = fixture.parent.join("link");
    let displaced_parent = fixture.boundary.join("nested-before-full-path");
    let source = PathBuf::from("desired");
    let (coordinator, intent, plan, record) = fixture.prepare_symlink_v2(
        "wal-symlink-v2-parent-full-path-replace",
        &target,
        &source,
        true,
    );
    let mut guard = coordinator.begin(record).unwrap();
    let hook_parent = fixture.parent.clone();
    let hook_displaced_parent = displaced_parent.clone();
    let hook_source = source.clone();
    let effect = capability::with_before_symlink_target_durable_hook_for_test(
        move || {
            fs::rename(&hook_parent, &hook_displaced_parent).unwrap();
            fs::create_dir(&hook_parent).unwrap();
            symlink(&hook_source, hook_parent.join("link")).unwrap();
        },
        || capability::symlink_entry_wal(&intent.target, &source, &plan, &mut guard),
    )
    .unwrap();
    assert!(effect.recovery_required, "{effect:?}");
    assert_eq!(guard.phase(), WalPhase::EffectVisible);
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::Conflict
    );
    assert_eq!(fs::read_link(&target).unwrap(), source);
    assert_eq!(
        fs::read_link(displaced_parent.join("link")).unwrap(),
        source
    );
    fixture.cleanup();
}

#[test]
fn atomic_recovery_read_budget_is_aggregate_and_fail_closed() {
    let fixture = AtomicRecoveryFixture::new("atomic-recovery-budget", true);
    let (_coordinator, _plan, record) =
        fixture.prepare("wal-atomic-recovery-budget", b"replacement");
    let mut budget = RecoveryReadBudget::with_limit(1);
    let error =
        capability::classify_atomic_recovery(&record, WalPhase::Prepared, &mut budget).unwrap_err();
    assert!(error.contains("bugetul agregat de citire"), "{error}");
    assert_eq!(fs::read(&fixture.target).unwrap(), b"baseline");
    fixture.cleanup();
}

#[test]
fn copy_plan_rejects_symlink_source_without_target_effect() {
    let fixture = AtomicRecoveryFixture::new("copy-source-symlink", false);
    let source = fixture.root.join("source.bin");
    let source_link = fixture.root.join("source-link.bin");
    fs::write(&source, b"payload").unwrap();
    std::os::unix::fs::symlink(&source, &source_link).unwrap();
    let authority = capability::capture_directory_authority(
        &fixture.boundary,
        "test/copy-source-symlink",
        DirectoryAuthorityScope::ApplicationPreviewCache,
    )
    .unwrap();
    let target = WriteTarget::new(
        &fixture.target,
        &fixture.boundary,
        "test/copy-source-symlink-target",
    )
    .bind_authority(authority)
    .unwrap();
    let error = capability::plan_copy(
        &target,
        &source_link,
        CapabilityReplacePolicy::Replace,
        "wal-copy-source-symlink",
    )
    .unwrap_err();
    assert!(
        error.contains("symlink") || error.contains("loop"),
        "{error}"
    );
    assert!(!fixture.target.exists());
    fixture.cleanup();
}

#[test]
fn copy_plan_rejects_sparse_source_over_resource_limit_before_hashing() {
    let fixture = AtomicRecoveryFixture::new("copy-source-limit", false);
    let source = fixture.root.join("source-large.bin");
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&source)
        .unwrap();
    file.set_len(super::super::MAX_WAL_COPY_BYTES + 1).unwrap();
    drop(file);
    let authority = capability::capture_directory_authority(
        &fixture.boundary,
        "test/copy-source-limit",
        DirectoryAuthorityScope::ApplicationPreviewCache,
    )
    .unwrap();
    let target = WriteTarget::new(
        &fixture.target,
        &fixture.boundary,
        "test/copy-source-limit-target",
    )
    .bind_authority(authority)
    .unwrap();
    let error = capability::plan_copy(
        &target,
        &source,
        CapabilityReplacePolicy::Replace,
        "wal-copy-source-limit",
    )
    .unwrap_err();
    assert!(error.contains("depășește limita"), "{error}");
    assert!(!fixture.target.exists());
    fixture.cleanup();
}

#[test]
fn restart_copy_recovery_preserves_exact_payload_with_wrong_mode_for_review() {
    let fixture = AtomicRecoveryFixture::new("copy-wrong-mode", false);
    let source = fixture.root.join("source.bin");
    fs::write(&source, b"same payload").unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o640)).unwrap();
    let (coordinator, _intent, plan, record) = fixture.prepare_copy(
        "wal-copy-wrong-mode",
        &fixture.target,
        &source,
        CapabilityReplacePolicy::Replace,
    );
    let guard = coordinator.begin(record).unwrap();
    fs::copy(&source, &fixture.target).unwrap();
    fs::set_permissions(&fixture.target, fs::Permissions::from_mode(0o600)).unwrap();
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
    assert_eq!(fs::read(&fixture.target).unwrap(), b"same payload");
    fixture.cleanup();
}

#[test]
fn copy_v2_plan_rejects_missing_parent_namespace_before_wal() {
    let fixture = AtomicRecoveryFixture::new("copy-new-parent", false);
    let target = fixture.boundary.join("new-parent/target.bin");
    let source = fixture.root.join("source.bin");
    fs::write(&source, b"new parent payload").unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o644)).unwrap();
    let authority = capability::capture_directory_authority(
        &fixture.boundary,
        "test/copy-missing-parent",
        DirectoryAuthorityScope::ApplicationPreviewCache,
    )
    .unwrap();
    let target = WriteTarget::new(&target, &fixture.boundary, "test/copy-missing-parent")
        .bind_authority(authority)
        .unwrap();
    let error = capability::plan_copy(
        &target,
        &source,
        CapabilityReplacePolicy::Replace,
        "wal-copy-new-parent",
    )
    .unwrap_err();
    assert!(error.contains("parent existent integral"), "{error}");
    assert!(!target.path.exists());
    assert!(!fixture.restart_coordinator().snapshot().unwrap().blocked);
    fixture.cleanup();
}

#[test]
fn copy_v2_plan_rejects_nested_missing_parent_without_partial_namespace() {
    let fixture = AtomicRecoveryFixture::new("copy-replaced-ancestor", false);
    let target = fixture.parent.join("new/target.bin");
    let source = fixture.root.join("source.bin");
    fs::write(&source, b"payload").unwrap();
    let authority = capability::capture_directory_authority(
        &fixture.boundary,
        "test/copy-nested-missing-parent",
        DirectoryAuthorityScope::ApplicationPreviewCache,
    )
    .unwrap();
    let target = WriteTarget::new(
        &target,
        &fixture.boundary,
        "test/copy-nested-missing-parent",
    )
    .bind_authority(authority)
    .unwrap();
    let error = capability::plan_copy(
        &target,
        &source,
        CapabilityReplacePolicy::Replace,
        "wal-copy-replaced-ancestor",
    )
    .unwrap_err();
    assert!(error.contains("parent existent integral"), "{error}");
    assert!(!target.path.exists());
    assert!(!fixture.parent.join("new").exists());
    fixture.cleanup();
}

#[test]
fn copy_source_mutation_after_temp_create_leaves_target_and_wal_recoverable() {
    let fixture = AtomicRecoveryFixture::new("copy-source-mutation", false);
    let source = fixture.root.join("source.bin");
    fs::write(&source, b"planned payload").unwrap();
    let (coordinator, intent, plan, record) = fixture.prepare_copy(
        "wal-copy-source-mutation",
        &fixture.target,
        &source,
        CapabilityReplacePolicy::Replace,
    );
    let mut guard = coordinator.begin(record).unwrap();
    let hook_source = source.clone();
    let effect = capability::with_before_copy_stream_hook_for_test(
        move || fs::write(&hook_source, b"mutated payload").unwrap(),
        || {
            capability::copy_file_wal(
                &intent.target,
                &source,
                CapabilityReplacePolicy::Replace,
                plan,
                &mut guard,
            )
        },
    )
    .unwrap();
    assert!(effect.recovery_required, "{effect:?}");
    drop(guard);
    let runtime_scan = coordinator.snapshot().unwrap();
    assert!(runtime_scan.blocked);
    assert_eq!(runtime_scan.items[0].file_name, "runtime-hot-guard");
    assert!(runtime_scan.items[0].diagnostic.contains("threadul UI"));
    assert!(!fixture.target.exists());
    fixture.cleanup();
}

#[test]
fn copy_postflight_detects_target_replacement_before_target_durable() {
    let fixture = AtomicRecoveryFixture::new("copy-postflight-swap", false);
    let source = fixture.root.join("source.bin");
    let moved = fixture.parent.join("moved-copy.bin");
    fs::write(&source, b"planned payload").unwrap();
    let (coordinator, intent, plan, record) = fixture.prepare_copy(
        "wal-copy-postflight-swap",
        &fixture.target,
        &source,
        CapabilityReplacePolicy::Replace,
    );
    let mut guard = coordinator.begin(record).unwrap();
    let hook_target = fixture.target.clone();
    let hook_moved = moved.clone();
    let effect = capability::with_before_copy_target_durable_hook_for_test(
        move || {
            fs::rename(&hook_target, &hook_moved).unwrap();
            fs::write(&hook_target, b"external replacement").unwrap();
        },
        || {
            capability::copy_file_wal(
                &intent.target,
                &source,
                CapabilityReplacePolicy::Replace,
                plan,
                &mut guard,
            )
        },
    )
    .unwrap();
    assert!(effect.recovery_required, "{effect:?}");
    drop(guard);
    assert!(coordinator.snapshot().unwrap().blocked);
    assert_eq!(fs::read(&fixture.target).unwrap(), b"external replacement");
    assert_eq!(fs::read(&moved).unwrap(), b"planned payload");
    fixture.cleanup();
}

#[test]
fn restart_copy_recovery_keeps_effect_visible_wal_when_created_target_was_moved_away() {
    let fixture = AtomicRecoveryFixture::new("copy-effect-visible-moved", false);
    let source = fixture.root.join("source.bin");
    let orphan = fixture.parent.join("orphan-copy.bin");
    fs::write(&source, b"effect-visible payload").unwrap();
    let (coordinator, intent, plan, record) = fixture.prepare_copy(
        "wal-copy-effect-visible-moved",
        &fixture.target,
        &source,
        CapabilityReplacePolicy::Replace,
    );
    let mut guard = coordinator.begin(record).unwrap();
    let hook_target = fixture.target.clone();
    let hook_orphan = orphan.clone();
    let effect = capability::with_before_copy_target_durable_hook_for_test(
        move || fs::rename(&hook_target, &hook_orphan).unwrap(),
        || {
            capability::copy_file_wal(
                &intent.target,
                &source,
                CapabilityReplacePolicy::Replace,
                plan,
                &mut guard,
            )
        },
    )
    .unwrap();
    assert!(effect.recovery_required, "{effect:?}");
    assert_eq!(guard.phase(), WalPhase::EffectVisible);
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
    assert_eq!(fs::read(&orphan).unwrap(), b"effect-visible payload");
    fixture.cleanup();
}

#[test]
fn restart_copy_recovery_keeps_target_durable_wal_when_created_target_was_moved_away() {
    let fixture = AtomicRecoveryFixture::new("copy-target-durable-moved", false);
    let source = fixture.root.join("source.bin");
    let orphan = fixture.parent.join("durable-orphan-copy.bin");
    fs::write(&source, b"target-durable payload").unwrap();
    let (coordinator, intent, plan, record) = fixture.prepare_copy(
        "wal-copy-target-durable-moved",
        &fixture.target,
        &source,
        CapabilityReplacePolicy::Replace,
    );
    let mut guard = coordinator.begin(record).unwrap();
    let effect = capability::copy_file_wal(
        &intent.target,
        &source,
        CapabilityReplacePolicy::Replace,
        plan,
        &mut guard,
    )
    .unwrap();
    assert!(effect.changed && !effect.recovery_required, "{effect:?}");
    assert_eq!(guard.phase(), WalPhase::TargetDurable);
    fs::rename(&fixture.target, &orphan).unwrap();
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
    assert!(scan.items[0].diagnostic.contains("TargetDurable"));
    assert_eq!(fs::read(&orphan).unwrap(), b"target-durable payload");
    fixture.cleanup();
}

#[test]
fn copy_v2_replace_full_path_cas_detects_parent_swap_after_target_fsync() {
    let fixture = AtomicRecoveryFixture::new("copy-replace-parent-swap", true);
    let source = fixture.root.join("source.bin");
    let displaced_parent = fixture.boundary.join("nested-displaced");
    fs::write(&source, b"replacement payload").unwrap();
    let (coordinator, intent, plan, record) = fixture.prepare_copy(
        "wal-copy-replace-parent-swap",
        &fixture.target,
        &source,
        CapabilityReplacePolicy::Replace,
    );
    let temp_leaf = plan.temp_leaf().unwrap();
    let displaced_target = displaced_parent.join("target.txt");
    let displaced_temp = displaced_parent.join(&temp_leaf);
    let mut guard = coordinator.begin(record).unwrap();
    let hook_parent = fixture.parent.clone();
    let hook_displaced = displaced_parent.clone();
    let hook_target = fixture.target.clone();
    let effect = capability::with_after_copy_target_fsync_hook_for_test(
        move || {
            fs::rename(&hook_parent, &hook_displaced).unwrap();
            fs::create_dir(&hook_parent).unwrap();
            fs::write(&hook_target, b"external replacement").unwrap();
        },
        || {
            capability::copy_file_wal(
                &intent.target,
                &source,
                CapabilityReplacePolicy::Replace,
                plan,
                &mut guard,
            )
        },
    )
    .unwrap();
    assert!(effect.recovery_required, "{effect:?}");
    assert_eq!(guard.phase(), WalPhase::EffectVisible);
    assert_eq!(fs::read(&fixture.target).unwrap(), b"external replacement");
    assert_eq!(fs::read(&displaced_target).unwrap(), b"replacement payload");
    assert!(!displaced_temp.exists());
    drop(guard);
    assert!(coordinator.snapshot().unwrap().blocked);
    fixture.cleanup();
}

#[test]
fn restart_rename_recovery_never_adopts_competing_destination() {
    let fixture = AtomicRecoveryFixture::new("rename-competitor", true);
    let destination = fixture.parent.join("renamed.txt");
    let orphan = fixture.parent.join("original-inode.txt");
    let (coordinator, _intent, _destination, plan, record) =
        fixture.prepare_rename("wal-rename-competitor", &destination);
    let guard = coordinator.begin(record).unwrap();
    fs::rename(&fixture.target, &destination).unwrap();
    fs::rename(&destination, &orphan).unwrap();
    fs::write(&destination, b"competitor").unwrap();
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
    assert_eq!(fs::read(&orphan).unwrap(), b"baseline");
    assert_eq!(fs::read(&destination).unwrap(), b"competitor");
    fixture.cleanup();
}

#[test]
fn runtime_rename_full_path_postflight_rejects_swapped_parent() {
    let fixture = AtomicRecoveryFixture::new("rename-parent-swap", true);
    let destination_path = fixture.parent.join("renamed.txt");
    let held_parent = fixture.boundary.join("nested-held");
    let public_parent = fixture.parent.clone();
    let hook_held_parent = held_parent.clone();
    let (coordinator, intent, destination, plan, record) =
        fixture.prepare_rename("wal-rename-parent-swap", &destination_path);
    let mut guard = coordinator.begin(record).unwrap();
    let effect = capability::with_before_rename_hook_for_test(
        move || {
            fs::rename(&public_parent, &hook_held_parent).unwrap();
            fs::create_dir(&public_parent).unwrap();
        },
        || capability::rename_entry_wal(&intent.target, &destination, plan, &mut guard),
    )
    .unwrap();
    assert!(effect.recovery_required, "{effect:?}");
    assert_eq!(guard.phase(), WalPhase::EffectVisible);
    assert!(!destination_path.exists());
    assert_eq!(
        fs::read(held_parent.join("renamed.txt")).unwrap(),
        b"baseline"
    );
    drop(guard);
    assert!(coordinator.snapshot().unwrap().blocked);
    fixture.cleanup();
}

#[test]
fn restart_remove_leaf_recovery_never_adopts_quarantine_competitor() {
    let fixture = AtomicRecoveryFixture::new("remove-quarantine-competitor", true);
    let orphan = fixture.parent.join("original-inode.txt");
    let (coordinator, _intent, plan, record) =
        fixture.prepare_remove_leaf("wal-remove-quarantine-competitor");
    let quarantine = fixture
        .parent
        .join(super::super::decode_component_hex(&plan.evidence.quarantine_leaf_hex).unwrap());
    let mut guard = coordinator.begin(record).unwrap();
    guard.mark_auxiliary_durable().unwrap();
    fs::rename(&fixture.target, &quarantine).unwrap();
    guard.mark_effect_visible().unwrap();
    fs::rename(&quarantine, &orphan).unwrap();
    fs::write(&quarantine, b"competitor").unwrap();
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
    assert_eq!(fs::read(&orphan).unwrap(), b"baseline");
    assert_eq!(fs::read(&quarantine).unwrap(), b"competitor");
    fixture.cleanup();
}

#[test]
fn runtime_remove_leaf_rejects_quarantine_swap_before_unlink() {
    let fixture = AtomicRecoveryFixture::new("remove-quarantine-swap", true);
    let orphan = fixture.parent.join("original-inode.txt");
    let (coordinator, intent, plan, record) =
        fixture.prepare_remove_leaf("wal-remove-quarantine-swap");
    let quarantine = fixture
        .parent
        .join(super::super::decode_component_hex(&plan.evidence.quarantine_leaf_hex).unwrap());
    let hook_quarantine = quarantine.clone();
    let hook_orphan = orphan.clone();
    let mut guard = coordinator.begin(record).unwrap();
    let effect = capability::with_before_remove_leaf_unlink_hook_for_test(
        move || {
            fs::rename(&hook_quarantine, &hook_orphan).unwrap();
            fs::write(&hook_quarantine, b"competitor").unwrap();
        },
        || capability::remove_leaf_wal(&intent.target, plan, &mut guard),
    )
    .unwrap();
    assert!(effect.recovery_required, "{effect:?}");
    assert_eq!(guard.phase(), WalPhase::EffectVisible);
    assert_eq!(fs::read(&orphan).unwrap(), b"baseline");
    assert_eq!(fs::read(&quarantine).unwrap(), b"competitor");
    drop(guard);
    fixture.cleanup();
}

#[test]
fn runtime_remove_leaf_full_path_postflight_rejects_parent_swap() {
    let fixture = AtomicRecoveryFixture::new("remove-parent-swap", true);
    let held_parent = fixture.boundary.join("nested-held");
    let public_parent = fixture.parent.clone();
    let hook_held_parent = held_parent.clone();
    let (coordinator, intent, plan, record) = fixture.prepare_remove_leaf("wal-remove-parent-swap");
    let mut guard = coordinator.begin(record).unwrap();
    let effect = capability::with_before_remove_leaf_quarantine_hook_for_test(
        move || {
            fs::rename(&public_parent, &hook_held_parent).unwrap();
            fs::create_dir(&public_parent).unwrap();
        },
        || capability::remove_leaf_wal(&intent.target, plan, &mut guard),
    )
    .unwrap();
    assert!(effect.recovery_required, "{effect:?}");
    assert_eq!(guard.phase(), WalPhase::EffectVisible);
    assert!(!fixture.target.exists());
    assert!(!held_parent.join("target.txt").exists());
    drop(guard);
    fixture.cleanup();
}

#[test]
fn restart_remove_tree_recovery_never_adopts_quarantine_competitor() {
    let fixture = AtomicRecoveryFixture::new("remove-tree-quarantine-competitor", false);
    fixture.create_tree();
    let (coordinator, _intent, plan, record) =
        fixture.prepare_remove_tree("wal-remove-tree-quarantine-competitor");
    let quarantine = fixture
        .parent
        .join(super::super::decode_component_hex(&plan.evidence.quarantine_leaf_hex).unwrap());
    let displaced = fixture.parent.join("displaced-original-tree");
    let mut guard = coordinator.begin(record).unwrap();
    guard.mark_auxiliary_durable().unwrap();
    fs::rename(&fixture.target, &quarantine).unwrap();
    guard.mark_effect_visible().unwrap();
    fs::rename(&quarantine, &displaced).unwrap();
    fs::create_dir(&quarantine).unwrap();
    fs::write(quarantine.join("competitor.txt"), b"competitor").unwrap();
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
    assert!(scan.items[0].available_resolution_actions.is_empty());
    assert_eq!(fs::read(displaced.join("nested/b.txt")).unwrap(), b"b");
    assert_eq!(
        fs::read(quarantine.join("competitor.txt")).unwrap(),
        b"competitor"
    );
    fixture.cleanup();
}

#[test]
fn append_v2_auxiliary_present_returned_to_baseline_never_clears_as_no_effect() {
    let fixture = AppendV2Fixture::new("append-v2-present-effect-removed", true);
    let payload = b"{\"effect_removed\":true}\n";
    let operation_id = "wal-append-v2-present-effect-removed";
    let (coordinator, intent, plan, record) = fixture.prepare(operation_id, payload);
    let mut guard = coordinator.begin(record).unwrap();
    let crashed = catch_unwind(AssertUnwindSafe(|| {
        capability::with_after_append_v2_write_before_phase_hook_for_test(
            || panic!("simulated crash after append before phase"),
            || capability::append_wal(&intent.target, payload, plan, &mut guard),
        )
    }));
    assert!(crashed.is_err());
    assert_eq!(guard.phase(), WalPhase::AuxiliaryDurable);
    OpenOptions::new()
        .write(true)
        .open(&fixture.target)
        .unwrap()
        .set_len(b"{\"baseline\":true}\n".len() as u64)
        .unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(scan.items[0].phase, Some(WalPhase::AuxiliaryDurable));
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::Conflict
    );
    assert!(!scan.items[0].automatic_recovery_available);
    assert_eq!(fs::read(&fixture.target).unwrap(), b"{\"baseline\":true}\n");
    drop(restarted);
    fixture.cleanup();
}

#[test]
fn append_v2_auxiliary_created_target_then_removed_never_clears_as_no_effect() {
    let fixture = AppendV2Fixture::new("append-v2-created-effect-removed", false);
    let payload = b"{\"effect_removed\":true}\n";
    let operation_id = "wal-append-v2-created-effect-removed";
    let (coordinator, intent, plan, record) = fixture.prepare(operation_id, payload);
    let mut guard = coordinator.begin(record).unwrap();
    let crashed = catch_unwind(AssertUnwindSafe(|| {
        capability::with_after_append_v2_link_before_phase_hook_for_test(
            || panic!("simulated crash after linkat before phase"),
            || capability::append_wal(&intent.target, payload, plan, &mut guard),
        )
    }));
    assert!(crashed.is_err());
    assert_eq!(guard.phase(), WalPhase::AuxiliaryDurable);
    fs::remove_file(&fixture.target).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(scan.items[0].phase, Some(WalPhase::AuxiliaryDurable));
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::Conflict
    );
    assert!(!scan.items[0].automatic_recovery_available);
    assert!(!fixture.target.exists());
    drop(restarted);
    fixture.cleanup();
}

#[test]
fn append_v2_target_durable_race_never_false_commits() {
    for with_target in [false, true] {
        let fixture = AppendV2Fixture::new(
            &format!("append-v2-target-durable-race-{with_target}"),
            with_target,
        );
        let payload = b"{\"target_durable\":true}\n";
        let operation_id = format!("wal-append-v2-target-durable-race-{with_target}");
        let (coordinator, intent, plan, record) = fixture.prepare(&operation_id, payload);
        let mut guard = coordinator.begin(record).unwrap();
        let target = fixture.target.clone();
        let expected_len = if with_target {
            b"{\"baseline\":true}\n".len() + payload.len()
        } else {
            payload.len()
        };
        let corrupt = vec![b'x'; expected_len];
        let effect = capability::with_after_append_v2_target_durable_hook_for_test(
            move || fs::write(&target, &corrupt).unwrap(),
            || capability::append_wal(&intent.target, payload, plan, &mut guard),
        )
        .unwrap();
        assert!(effect.recovery_required, "{effect:?}");
        assert_eq!(guard.phase(), WalPhase::TargetDurable);
        drop(guard);
        drop(coordinator);

        let restarted = fixture.restart_coordinator();
        let scan = restarted.snapshot().unwrap();
        assert!(scan.blocked, "{scan:?}");
        assert_eq!(
            scan.items[0].classification,
            super::super::WriteAuthorityRecoveryClassification::Conflict
        );
        assert_eq!(fs::read(&fixture.target).unwrap(), vec![b'x'; expected_len]);
        drop(restarted);
        fixture.cleanup();
    }
}

#[test]
fn append_v2_recovery_detects_same_inode_mutation_after_hash() {
    let fixture = AppendV2Fixture::new("append-v2-recovery-post-hash", true);
    let payload = b"{\"post_hash\":true}\n";
    let operation_id = "wal-append-v2-recovery-post-hash";
    let (coordinator, intent, plan, record) = fixture.prepare(operation_id, payload);
    let mut guard = coordinator.begin(record).unwrap();
    let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        capability::with_after_append_v2_write_before_phase_hook_for_test(
            || panic!("post-write crash"),
            || capability::append_wal(&intent.target, payload, plan, &mut guard),
        )
    }));
    assert!(crashed.is_err());
    assert_eq!(guard.phase(), WalPhase::AuxiliaryDurable);
    drop(guard);
    drop(coordinator);

    let target = fixture.target.clone();
    let corrupt = vec![b'x'; b"{\"baseline\":true}\n".len() + payload.len()];
    let restarted = capability::with_after_append_v2_recovery_hash_hook_for_test(
        move || fs::write(&target, &corrupt).unwrap(),
        || fixture.restart_coordinator(),
    );
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert!(
        scan.items[0].diagnostic.contains("hash") || scan.items[0].diagnostic.contains("schimbat")
    );
    drop(restarted);
    fixture.cleanup();
}

#[test]
fn append_v2_recovery_refuses_mode_changes_for_existing_and_created_targets() {
    for with_target in [false, true] {
        let fixture =
            AppendV2Fixture::new(&format!("append-v2-mode-change-{with_target}"), with_target);
        let payload = b"{\"mode_change\":true}\n";
        let operation_id = format!("wal-append-v2-mode-change-{with_target}");
        let (coordinator, intent, plan, record) = fixture.prepare(&operation_id, payload);
        let mut guard = coordinator.begin(record).unwrap();
        let crashed = catch_unwind(AssertUnwindSafe(|| {
            if with_target {
                capability::with_after_append_v2_write_before_phase_hook_for_test(
                    || panic!("simulated crash after append before phase"),
                    || capability::append_wal(&intent.target, payload, plan, &mut guard),
                )
            } else {
                capability::with_after_append_v2_link_before_phase_hook_for_test(
                    || panic!("simulated crash after link before phase"),
                    || capability::append_wal(&intent.target, payload, plan, &mut guard),
                )
            }
        }));
        assert!(crashed.is_err());
        fs::set_permissions(&fixture.target, fs::Permissions::from_mode(0o777)).unwrap();
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
        drop(restarted);
        fixture.cleanup();
    }
}

#[test]
fn copy_v2_checkpointed_identity_with_wrong_payload_stays_hot() {
    let fixture = AtomicRecoveryFixture::new("copy-v2-checkpoint-wrong-payload", false);
    let source = fixture.root.join("source.bin");
    let expected = b"payload-good";
    let corrupt = b"payload-evil";
    assert_eq!(expected.len(), corrupt.len());
    fs::write(&source, expected).unwrap();
    let operation_id = "wal-copy-v2-checkpoint-wrong-payload";
    let (coordinator, intent, plan, record) = fixture.prepare_copy_for_owner(
        operation_id,
        &fixture.target,
        &source,
        WriteOwner::ProjectInitializer,
        CapabilityReplacePolicy::CreateNew,
    );
    let mut guard = coordinator.begin(record).unwrap();
    let corrupt_target = fixture.target.clone();
    let effect = capability::with_after_copy_target_link_before_phase_hook_for_test(
        move || fs::write(&corrupt_target, corrupt).unwrap(),
        || {
            capability::copy_file_wal(
                &intent.target,
                &source,
                CapabilityReplacePolicy::CreateNew,
                plan,
                &mut guard,
            )
        },
    )
    .unwrap();
    assert!(effect.recovery_required, "{effect:?}");
    assert_eq!(guard.phase(), WalPhase::EffectVisible);
    assert_eq!(fs::read(&fixture.target).unwrap(), corrupt);
    drop(guard);
    drop(coordinator);

    let hot_name = copy_v2_wal_record_name(&fixture, operation_id);
    assert!(hot_name.contains(".effect-visible.cpc"), "{hot_name}");

    let restarted = fixture.restart_coordinator();
    let first_scan = restarted.snapshot().unwrap();
    assert!(first_scan.blocked, "{first_scan:?}");
    assert!(first_scan
        .items
        .iter()
        .any(|item| { item.diagnostic.contains("hash") || item.diagnostic.contains("payload") }));
    assert_eq!(fs::read(&fixture.target).unwrap(), corrupt);
    assert_eq!(copy_v2_wal_record_name(&fixture, operation_id), hot_name);
    drop(restarted);

    let second_restart = fixture.restart_coordinator();
    let second_scan = second_restart.snapshot().unwrap();
    assert!(second_scan.blocked, "{second_scan:?}");
    assert_eq!(fs::read(&fixture.target).unwrap(), corrupt);
    assert_eq!(copy_v2_wal_record_name(&fixture, operation_id), hot_name);
    drop(second_restart);
    fixture.cleanup();
}

#[test]
fn copy_v2_recovery_detects_same_inode_mutation_after_streaming_hash() {
    let fixture = AtomicRecoveryFixture::new("copy-v2-post-hash-mutation", false);
    let source = fixture.root.join("source.bin");
    let expected = b"payload-good";
    let corrupt = b"payload-evil";
    assert_eq!(expected.len(), corrupt.len());
    fs::write(&source, expected).unwrap();
    let operation_id = "wal-copy-v2-post-hash-mutation";
    let (coordinator, intent, plan, record) = fixture.prepare_copy_for_owner(
        operation_id,
        &fixture.target,
        &source,
        WriteOwner::ProjectInitializer,
        CapabilityReplacePolicy::CreateNew,
    );
    let mut guard = coordinator.begin(record).unwrap();
    let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        capability::with_after_copy_target_link_before_phase_hook_for_test(
            copy_v2_crash_now,
            || {
                capability::copy_file_wal(
                    &intent.target,
                    &source,
                    CapabilityReplacePolicy::CreateNew,
                    plan,
                    &mut guard,
                )
            },
        )
    }));
    assert!(crashed.is_err());
    assert_eq!(guard.phase(), WalPhase::AuxiliaryDurable);
    drop(guard);
    drop(coordinator);

    let mutate_target = fixture.target.clone();
    let restarted = capability::with_after_copy_recovery_hash_hook_for_test(
        move || fs::write(&mutate_target, corrupt).unwrap(),
        || fixture.restart_coordinator(),
    );
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert!(scan.items.iter().any(|item| {
        item.diagnostic.contains("schimbat") || item.diagnostic.contains("post-hash")
    }));
    assert_eq!(fs::read(&fixture.target).unwrap(), corrupt);
    assert!(copy_v2_wal_record_name(&fixture, operation_id).contains(".auxiliary-durable.cpc."));
    drop(restarted);
    fixture.cleanup();
}
