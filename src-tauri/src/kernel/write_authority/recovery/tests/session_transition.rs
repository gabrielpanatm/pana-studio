use super::{fixtures::*, *};

#[test]
fn legacy_mcp_staged_projection_is_discarded_without_blocking_restart() {
    let root = unique_test_dir("legacy-mcp-staged-projection");
    let config = root.join("config");
    let parent = config.join("mcp");
    let target_path = parent.join("mcp.json");
    let wal_path = root.join("data/kernel/write-authority-wal");
    fs::create_dir_all(&parent).unwrap();
    fs::create_dir_all(&wal_path).unwrap();
    fs::write(&target_path, b"{\"processId\":1}\n").unwrap();

    let target_authority = capability::capture_directory_authority(
        &config,
        "test/mcp-config",
        DirectoryAuthorityScope::ApplicationConfig,
    )
    .unwrap();
    let target = WriteTarget::new(&target_path, &config, "mcp/mcp.json")
        .bind_authority(target_authority)
        .unwrap();
    let intent = WriteIntent::new(
        WriteCategory::InternalAppWrite,
        WriteOwner::McpContext,
        WriteOperationKind::WriteText,
        target,
        // This is the legacy policy whose interrupted temp write used to
        // create an unresolvable global recovery barrier.
        WritePolicy::internal_atomic(),
        "Legacy MCP recovery fixture.",
    );
    let operation_id = "legacy-mcp-staged";
    let payload = b"{\"processId\":2}\n";
    let plan = capability::plan_atomic_write(
        &intent.target,
        payload,
        CapabilityReplacePolicy::Replace,
        operation_id,
    )
    .unwrap();
    let temp_path = parent.join(plan.temp_leaf().unwrap());
    let record = build_atomic_wal_record(operation_id, 1, &intent, &plan).unwrap();
    let wal_authority = capability::capture_directory_authority(
        &wal_path,
        "test/mcp-write-authority-wal",
        DirectoryAuthorityScope::ApplicationWriteAuthorityWal,
    )
    .unwrap();
    let coordinator = RecoveryCoordinator::bootstrap(wal_authority).unwrap();
    let guard = coordinator.begin(record).unwrap();
    fs::write(&temp_path, payload).unwrap();
    drop(guard);
    drop(coordinator);

    let restart_authority = capability::capture_directory_authority(
        &wal_path,
        "test/mcp-write-authority-wal-restart",
        DirectoryAuthorityScope::ApplicationWriteAuthorityWal,
    )
    .unwrap();
    let restarted = RecoveryCoordinator::bootstrap(restart_authority).unwrap();
    assert!(!restarted.snapshot().unwrap().blocked);
    assert!(!temp_path.exists());
    assert_eq!(fs::read(&target_path).unwrap(), b"{\"processId\":1}\n");
    drop(restarted);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn external_config_prepared_replace_clears_only_no_effect() {
    let fixture = AtomicRecoveryFixture::new("external-v2-prepared-replace", true);
    let payload = b"new-config";
    let backup = fixture.parent.join("target.txt.pana-studio-prepared.bak");
    let (coordinator, _plan, record) =
        fixture.prepare_external_config("wal-external-v2-prepared", payload, &backup);
    let guard = coordinator.begin(record).unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(!scan.blocked, "{scan:?}");
    assert_eq!(fs::read(&fixture.target).unwrap(), b"baseline");
    assert!(!backup.exists());
    drop(restarted);

    let second_restart = fixture.restart_coordinator();
    assert!(!second_restart.snapshot().unwrap().blocked);
    assert_eq!(fs::read(&fixture.target).unwrap(), b"baseline");
    assert!(!backup.exists());
    drop(second_restart);
    fixture.cleanup();
}

#[test]
fn external_config_committed_pair_finalizes_from_auxiliary_checkpoint() {
    let fixture = AtomicRecoveryFixture::new("external-v2-aux-commit", true);
    let payload = b"new-config";
    let backup = fixture.parent.join("target.txt.pana-studio-aux-commit.bak");
    let (coordinator, plan, record) =
        fixture.prepare_external_config("wal-external-v2-aux-commit", payload, &backup);
    let mut guard = coordinator.begin(record).unwrap();
    fixture.materialize_external_committed_pair(
        &plan,
        &mut guard,
        payload,
        &backup,
        WalPhase::AuxiliaryDurable,
    );
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();

    assert!(!scan.blocked, "{scan:?}");
    assert_eq!(fs::read(&fixture.target).unwrap(), payload);
    assert_eq!(fs::read(&backup).unwrap(), b"baseline");
    drop(restarted);
    fixture.cleanup();
}

#[test]
fn external_config_committed_pair_finalizes_from_target_durable() {
    let fixture = AtomicRecoveryFixture::new("external-v2-target-durable", true);
    let payload = b"new-config";
    let backup = fixture
        .parent
        .join("target.txt.pana-studio-target-durable.bak");
    let (coordinator, plan, record) =
        fixture.prepare_external_config("wal-external-v2-target", payload, &backup);
    let mut guard = coordinator.begin(record).unwrap();
    fixture.materialize_external_committed_pair(
        &plan,
        &mut guard,
        payload,
        &backup,
        WalPhase::TargetDurable,
    );
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();

    assert!(!scan.blocked, "{scan:?}");
    assert_eq!(fs::read(&fixture.target).unwrap(), payload);
    assert_eq!(fs::read(&backup).unwrap(), b"baseline");
    drop(restarted);
    fixture.cleanup();
}

#[test]
fn external_config_create_new_crash_matrix_is_restart_idempotent() {
    for (label, phase, target_present) in [
        ("aux-absent", WalPhase::AuxiliaryDurable, false),
        ("aux-exact", WalPhase::AuxiliaryDurable, true),
        ("effect-absent", WalPhase::EffectVisible, false),
        ("effect-exact", WalPhase::EffectVisible, true),
        ("target-exact", WalPhase::TargetDurable, true),
    ] {
        let fixture = AtomicRecoveryFixture::new(&format!("external-v2-create-{label}"), false);
        let payload = b"new-config";
        let operation_id = format!("wal-external-v2-create-{label}");
        let (coordinator, plan, record) =
            fixture.prepare_external_config_create_new(&operation_id, payload);
        let mut guard = coordinator.begin(record).unwrap();

        let checkpoint_identity = if target_present {
            fs::write(&fixture.target, payload).unwrap();
            fs::set_permissions(
                &fixture.target,
                fs::Permissions::from_mode(plan.evidence.target_new_mode_bits),
            )
            .unwrap();
            capability::external_stage_identity_digest_for_test(&fixture.target, "target").unwrap()
        } else {
            "a".repeat(32)
        };
        guard
            .mark_external_auxiliary_durable(
                super::super::WalExternalStageCheckpoint::new(checkpoint_identity, None).unwrap(),
            )
            .unwrap();
        if matches!(phase, WalPhase::EffectVisible | WalPhase::TargetDurable) {
            guard.mark_effect_visible().unwrap();
        }
        if phase == WalPhase::TargetDurable {
            guard.mark_target_durable().unwrap();
        }
        if target_present {
            fs::File::open(&fixture.target).unwrap().sync_all().unwrap();
        }
        fs::File::open(&fixture.parent).unwrap().sync_all().unwrap();
        drop(guard);
        drop(coordinator);

        let restarted = fixture.restart_coordinator();
        let first_scan = restarted.snapshot().unwrap();
        assert!(!first_scan.blocked, "{label}: {first_scan:?}");
        if target_present {
            assert_eq!(fs::read(&fixture.target).unwrap(), payload, "{label}");
        } else {
            assert!(!fixture.target.exists(), "{label}");
        }
        let target_temp = fixture
            .parent
            .join(super::super::decode_component_hex(&plan.evidence.target.temp_leaf_hex).unwrap());
        assert!(!target_temp.exists(), "{label}");
        drop(restarted);

        let second_restart = fixture.restart_coordinator();
        let second_scan = second_restart.snapshot().unwrap();
        assert!(!second_scan.blocked, "{label}: {second_scan:?}");
        if target_present {
            assert_eq!(fs::read(&fixture.target).unwrap(), payload, "{label}");
        } else {
            assert!(!fixture.target.exists(), "{label}");
        }
        drop(second_restart);
        fixture.cleanup();
    }
}

#[test]
fn external_config_maximum_payload_restart_stays_within_recovery_budget() {
    let fixture = AtomicRecoveryFixture::new("external-v2-max-budget", true);
    let size = super::super::MAX_WAL_EXTERNAL_CONFIG_BYTES as usize;
    let previous = vec![b'o'; size];
    let payload = vec![b'n'; size];
    fs::write(&fixture.target, &previous).unwrap();
    let backup = fixture.parent.join("target.txt.pana-studio-max.bak");
    let (coordinator, plan, record) = fixture.prepare_external_config_with_previous(
        "wal-external-v2-max",
        &payload,
        &backup,
        &previous,
    );
    let mut guard = coordinator.begin(record).unwrap();
    fixture.materialize_external_committed_pair(
        &plan,
        &mut guard,
        &payload,
        &backup,
        WalPhase::EffectVisible,
    );
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();

    assert!(!scan.blocked, "{scan:?}");
    assert_eq!(fs::read(&fixture.target).unwrap(), payload);
    assert_eq!(fs::read(&backup).unwrap(), previous);
    drop(restarted);
    fixture.cleanup();
}

#[test]
fn external_config_action_rejects_oversized_competitor_after_classification() {
    let fixture = AtomicRecoveryFixture::new("external-v2-oversized-cas", true);
    let payload = b"new-config";
    let backup = fixture.parent.join("target.txt.pana-studio-oversized.bak");
    let (coordinator, plan, record) =
        fixture.prepare_external_config("wal-external-v2-oversized", payload, &backup);
    let recovery_record = record.clone();
    let mut guard = coordinator.begin(record).unwrap();
    fixture.materialize_external_committed_pair(
        &plan,
        &mut guard,
        payload,
        &backup,
        WalPhase::EffectVisible,
    );
    let checkpoint = super::super::WalExternalStageCheckpoint::new(
        capability::external_stage_identity_digest_for_test(&fixture.target, "target").unwrap(),
        None,
    )
    .unwrap();
    let mut classify_budget = RecoveryReadBudget::new();
    let assessment = capability::classify_external_config_recovery(
        &recovery_record,
        WalPhase::EffectVisible,
        Some(&checkpoint),
        None,
        &mut classify_budget,
    )
    .unwrap();
    assert!(assessment.automatic_action.is_some());

    fs::remove_file(&fixture.target).unwrap();
    let oversized = vec![b'x'; super::super::MAX_WAL_EXTERNAL_CONFIG_BYTES as usize + 1];
    fs::write(&fixture.target, &oversized).unwrap();
    fs::set_permissions(
        &fixture.target,
        fs::Permissions::from_mode(plan.evidence.target_new_mode_bits),
    )
    .unwrap();
    let mut action_budget = RecoveryReadBudget::new();
    let error = capability::execute_external_config_recovery(
        &recovery_record,
        WalPhase::EffectVisible,
        Some(&checkpoint),
        None,
        &mut action_budget,
    )
    .unwrap_err();

    assert!(error.contains("limita ExternalConfig"), "{error}");
    assert_eq!(
        fs::metadata(&fixture.target).unwrap().len(),
        oversized.len() as u64
    );
    assert_eq!(fs::read(&backup).unwrap(), b"baseline");
    drop(guard);
    drop(coordinator);
    fixture.cleanup();
}

#[test]
fn external_config_byte_identical_wrong_backup_inode_is_preserved_and_blocked() {
    let fixture = AtomicRecoveryFixture::new("external-v2-wrong-backup-inode", true);
    let payload = b"new-config";
    let backup = fixture
        .parent
        .join("target.txt.pana-studio-wrong-inode.bak");
    let (coordinator, _plan, record) =
        fixture.prepare_external_config("wal-external-v2-wrong-inode", payload, &backup);
    let mut guard = coordinator.begin(record).unwrap();
    guard
        .mark_external_auxiliary_durable(
            super::super::WalExternalStageCheckpoint::new("a".repeat(32), None).unwrap(),
        )
        .unwrap();
    fs::remove_file(&fixture.target).unwrap();
    fs::write(&backup, b"baseline").unwrap();
    fs::set_permissions(&backup, fs::Permissions::from_mode(0o644)).unwrap();
    guard.mark_effect_visible().unwrap();
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();

    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::Conflict
    );
    assert_eq!(fs::read(&backup).unwrap(), b"baseline");
    assert!(!fixture.target.exists());
    drop(restarted);
    fixture.cleanup();
}

#[test]
fn directory_v2_project_initializer_owner_contract_commits() {
    let fixture = AtomicRecoveryFixture::new("mkdir-v2-project-initializer", false);
    let target_path = fixture.parent.join("created");
    let authority = capability::capture_directory_authority(
        &fixture.boundary,
        "test/mkdir-v2-project-initializer",
        DirectoryAuthorityScope::ProjectCreation { authority_id: 42 },
    )
    .unwrap();
    let target = WriteTarget::new(
        &target_path,
        &fixture.boundary,
        "test/mkdir-v2-project-initializer",
    )
    .bind_authority(authority)
    .unwrap();
    let intent = WriteIntent::new(
        WriteCategory::ProjectSourceWrite,
        WriteOwner::ProjectInitializer,
        WriteOperationKind::CreateDirectory,
        target,
        WritePolicy::project_creation_lifecycle(),
        "Directory direct ProjectInitializer contract fixture.",
    );
    let plan = capability::plan_directory(&intent.target).unwrap();
    let record =
        build_directory_wal_record("wal-mkdir-v2-project-initializer", 1, &intent, &plan).unwrap();
    let coordinator = fixture.restart_coordinator();
    let mut guard = coordinator.begin(record).unwrap();
    let effect = capability::create_directory_all_wal(&intent.target, &plan, &mut guard).unwrap();
    assert!(effect.changed);
    assert!(!effect.recovery_required);
    guard.commit().unwrap();
    assert!(target_path.is_dir());
    drop(coordinator);
    fixture.cleanup();
}

#[test]
fn directory_v2_owner_scope_is_bound() {
    let fixture = AtomicRecoveryFixture::new("mkdir-v2-owner-binding", false);
    let target = fixture.parent.join("created");
    fs::create_dir(&target).unwrap();
    let (_coordinator, _intent, _plan, record) =
        fixture.prepare_directory_v2("wal-mkdir-v2-owner-binding", &target);

    let mut body = record.body;
    body.owner = "kernel".into();
    let error = super::super::WalRecord::seal(body).unwrap_err();
    assert!(
        error.contains("owner/category/scope/policy Directory v2"),
        "{error}"
    );
    fixture.cleanup();
}

#[test]
fn symlink_v2_planner_requires_existing_parent_and_preview_cache_scope() {
    let fixture = AtomicRecoveryFixture::new("symlink-v2-planner-binding", false);
    let missing_target = fixture.parent.join("missing/link");
    let preview_authority = capability::capture_directory_authority(
        &fixture.boundary,
        "test/symlink-v2-missing-parent",
        DirectoryAuthorityScope::ApplicationPreviewCache,
    )
    .unwrap();
    let missing = WriteTarget::new(
        &missing_target,
        &fixture.boundary,
        "test/symlink-v2-missing-parent",
    )
    .bind_authority(preview_authority)
    .unwrap();
    let error = capability::plan_symlink(&missing, Path::new("desired")).unwrap_err();
    assert!(error.contains("parent final existent"), "{error}");
    assert!(!fixture.parent.join("missing").exists());

    let project_authority = capability::capture_directory_authority(
        &fixture.boundary,
        "test/symlink-v2-wrong-scope",
        DirectoryAuthorityScope::ProjectRoot,
    )
    .unwrap();
    let wrong_scope = WriteTarget::new(
        fixture.parent.join("link"),
        &fixture.boundary,
        "test/symlink-v2-wrong-scope",
    )
    .bind_authority(project_authority)
    .unwrap();
    let error = capability::plan_symlink(&wrong_scope, Path::new("desired")).unwrap_err();
    assert!(error.contains("application_preview_cache"), "{error}");
    fixture.cleanup();
}

#[test]
fn wal_begin_is_exclusive_across_independent_coordinators() {
    let fixture = AtomicRecoveryFixture::new("exclusive-coordinators", false);
    let first_target = fixture.parent.join("first");
    let second_target = fixture.parent.join("second");
    let (first, _intent, _plan, first_record) =
        fixture.prepare_directory("wal-exclusive-first", &first_target);
    let (second, _intent, _plan, second_record) =
        fixture.prepare_directory("wal-exclusive-second", &second_target);
    let first_guard = first.begin(first_record).unwrap();
    let (ready_tx, ready_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        ready_tx.send(()).unwrap();
        let result = second
            .begin(second_record)
            .and_then(|guard| guard.abort_no_effect());
        done_tx.send(result).unwrap();
    });
    ready_rx.recv().unwrap();
    assert!(done_rx.recv_timeout(Duration::from_millis(200)).is_err());
    first_guard.abort_no_effect().unwrap();
    done_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap();
    worker.join().unwrap();
    fixture.cleanup();
}

#[test]
fn copy_io_gate_serializes_planning_and_transfer_within_the_process() {
    let fixture = AtomicRecoveryFixture::new("copy-io-gate", false);
    let coordinator = Arc::new(fixture.restart_coordinator());
    let first = coordinator.acquire_copy_io().unwrap();
    let worker_coordinator = Arc::clone(&coordinator);
    let (ready_tx, ready_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        ready_tx.send(()).unwrap();
        let lease = worker_coordinator.acquire_copy_io();
        done_tx.send(lease.is_ok()).unwrap();
    });
    ready_rx.recv().unwrap();
    assert!(done_rx.recv_timeout(Duration::from_millis(200)).is_err());
    drop(first);
    assert!(done_rx.recv_timeout(Duration::from_secs(2)).unwrap());
    worker.join().unwrap();
    fixture.cleanup();
}

#[test]
fn require_clean_checks_disk_instead_of_trusting_a_stale_clean_snapshot() {
    let fixture = AtomicRecoveryFixture::new("require-clean-disk", false);
    let target = fixture.parent.join("created");
    let (writer, _intent, _plan, record) =
        fixture.prepare_directory("wal-require-clean-disk", &target);
    let stale = fixture.restart_coordinator();
    assert!(!stale.snapshot().unwrap().blocked);
    let guard = writer.begin(record).unwrap();
    drop(guard);
    assert!(!stale.snapshot().unwrap().blocked);

    let error = stale.require_clean().unwrap_err();
    assert!(error.contains("RECOVERY_BLOCKED"), "{error}");
    fixture.cleanup();
}

#[test]
fn explicit_rescan_recovers_prepared_no_effect_without_process_restart() {
    let fixture = AtomicRecoveryFixture::new("runtime-rescan-no-effect", false);
    let source = fixture.root.join("source.bin");
    fs::write(&source, b"payload").unwrap();
    let (coordinator, _intent, plan, record) = fixture.prepare_copy(
        "wal-runtime-rescan-no-effect",
        &fixture.target,
        &source,
        CapabilityReplacePolicy::Replace,
    );
    let guard = coordinator.begin(record).unwrap();
    drop(plan);
    drop(guard);
    assert!(coordinator.snapshot().unwrap().blocked);

    let scan = coordinator.rescan_and_recover_exclusive().unwrap();
    assert!(!scan.blocked, "{scan:?}");
    assert!(!fixture.target.exists());
    fixture.cleanup();
}

#[test]
fn copy_v2_preview_replace_uses_atomic_overwrite_without_cleanup_leaf() {
    let fixture = AtomicRecoveryFixture::new("copy-replace", true);
    fs::set_permissions(&fixture.target, fs::Permissions::from_mode(0o600)).unwrap();
    let source = fixture.root.join("source.bin");
    fs::write(&source, b"replacement through WAL").unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o640)).unwrap();
    let (coordinator, intent, plan, record) = fixture.prepare_copy(
        "wal-copy-replace-runtime",
        &fixture.target,
        &source,
        CapabilityReplacePolicy::Replace,
    );
    let temp = fixture.parent.join(plan.temp_leaf().unwrap());
    let mut guard = coordinator.begin(record).unwrap();
    let effect = capability::copy_file_wal(
        &intent.target,
        &source,
        CapabilityReplacePolicy::Replace,
        plan,
        &mut guard,
    )
    .unwrap();
    assert!(effect.changed);
    assert!(!effect.recovery_required, "{effect:?}");
    guard.commit().unwrap();
    assert_eq!(
        fs::read(&fixture.target).unwrap(),
        b"replacement through WAL"
    );
    assert_eq!(
        fs::metadata(&fixture.target).unwrap().permissions().mode() & 0o7777,
        0o640
    );
    assert!(!temp.exists());
    assert!(!coordinator.snapshot().unwrap().blocked);
    fixture.cleanup();
}

#[test]
fn copy_v2_replace_is_rejected_outside_application_preview_cache() {
    let fixture = AtomicRecoveryFixture::new("copy-v2-scope-reject", true);
    let source = fixture.root.join("source.bin");
    fs::write(&source, b"replacement").unwrap();
    let authority = capability::capture_directory_authority(
        &fixture.boundary,
        "test/copy-v2-scope-reject",
        DirectoryAuthorityScope::ProjectRoot,
    )
    .unwrap();
    let target = WriteTarget::new(
        &fixture.target,
        &fixture.boundary,
        "test/copy-v2-scope-reject",
    )
    .bind_authority(authority)
    .unwrap();
    let error = capability::plan_copy(
        &target,
        &source,
        CapabilityReplacePolicy::Replace,
        "wal-copy-v2-scope-reject",
    )
    .unwrap_err();
    assert!(error.contains("ApplicationPreviewCache"), "{error}");
    assert_eq!(fs::read(&fixture.target).unwrap(), b"baseline");
    fixture.cleanup();
}

#[test]
fn copy_v2_preview_overwrite_window_is_confined_to_rebuildable_cache() {
    let fixture = AtomicRecoveryFixture::new("copy-v2-cache-overwrite", true);
    let source = fixture.root.join("source.bin");
    let displaced_baseline = fixture.parent.join("displaced-baseline.txt");
    fs::write(&source, b"planned preview payload").unwrap();
    let (coordinator, intent, plan, record) = fixture.prepare_copy(
        "wal-copy-v2-cache-overwrite",
        &fixture.target,
        &source,
        CapabilityReplacePolicy::Replace,
    );
    let temp = fixture.parent.join(plan.temp_leaf().unwrap());
    let mut guard = coordinator.begin(record).unwrap();
    let hook_target = fixture.target.clone();
    let hook_displaced = displaced_baseline.clone();
    let effect = capability::with_before_copy_preview_overwrite_rename_hook_for_test(
        move || {
            fs::rename(&hook_target, &hook_displaced).unwrap();
            fs::write(&hook_target, b"concurrent preview cache entry").unwrap();
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
    assert!(effect.changed && !effect.recovery_required, "{effect:?}");
    guard.commit().unwrap();
    assert_eq!(
        fs::read(&fixture.target).unwrap(),
        b"planned preview payload"
    );
    assert_eq!(fs::read(&displaced_baseline).unwrap(), b"baseline");
    assert!(!temp.exists());
    assert!(!coordinator.snapshot().unwrap().blocked);
    fixture.cleanup();
}

#[test]
fn preview_copy_v2_detects_authority_root_swap_after_target_fsync() {
    let fixture = AtomicRecoveryFixture::new("copy-preview-authority-root-swap", true);
    let source = fixture.root.join("source.bin");
    let displaced_boundary = fixture.root.join("boundary-displaced");
    fs::write(&source, b"preview replacement").unwrap();
    let (coordinator, intent, plan, record) = fixture.prepare_copy_for_owner(
        "wal-copy-preview-authority-root-swap",
        &fixture.target,
        &source,
        WriteOwner::Preview,
        CapabilityReplacePolicy::Replace,
    );
    let temp_leaf = plan.temp_leaf().unwrap();
    let displaced_target = displaced_boundary.join("nested/target.txt");
    let displaced_temp = displaced_boundary.join("nested").join(&temp_leaf);
    let mut guard = coordinator.begin(record).unwrap();
    let hook_boundary = fixture.boundary.clone();
    let hook_displaced = displaced_boundary.clone();
    let hook_parent = fixture.parent.clone();
    let hook_target = fixture.target.clone();
    let effect = capability::with_after_copy_target_fsync_hook_for_test(
        move || {
            fs::rename(&hook_boundary, &hook_displaced).unwrap();
            fs::create_dir_all(&hook_parent).unwrap();
            fs::write(&hook_target, b"public competitor").unwrap();
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
    assert!(
        effect
            .diagnostic
            .as_deref()
            .is_some_and(|value| value.contains("Recordul copy WAL rămâne hot")),
        "{effect:?}"
    );
    assert_eq!(guard.phase(), WalPhase::EffectVisible);
    assert_eq!(fs::read(&fixture.target).unwrap(), b"public competitor");
    assert_eq!(fs::read(&displaced_target).unwrap(), b"preview replacement");
    assert!(!displaced_temp.exists());
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::Conflict
    );
    assert_eq!(fs::read(&fixture.target).unwrap(), b"public competitor");
    assert_eq!(fs::read(&displaced_target).unwrap(), b"preview replacement");
    assert!(!displaced_temp.exists());
    drop(restarted);
    fixture.cleanup();
}

#[test]
fn project_initializer_copy_create_new_has_terminal_authority_root_postflight() {
    let fixture = AtomicRecoveryFixture::new("copy-initializer-authority-root-swap", false);
    let source = fixture.root.join("source.bin");
    let displaced_boundary = fixture.root.join("boundary-displaced");
    fs::write(&source, b"initializer payload").unwrap();
    let (coordinator, intent, plan, record) = fixture.prepare_copy_for_owner(
        "wal-copy-initializer-authority-root-swap",
        &fixture.target,
        &source,
        WriteOwner::ProjectInitializer,
        CapabilityReplacePolicy::CreateNew,
    );
    let mut guard = coordinator.begin(record).unwrap();
    let hook_boundary = fixture.boundary.clone();
    let hook_displaced = displaced_boundary.clone();
    let hook_parent = fixture.parent.clone();
    let hook_target = fixture.target.clone();
    let effect = capability::with_after_copy_target_durable_hook_for_test(
        move || {
            fs::rename(&hook_boundary, &hook_displaced).unwrap();
            fs::create_dir_all(&hook_parent).unwrap();
            fs::write(&hook_target, b"public competitor").unwrap();
        },
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
    assert!(
        effect
            .diagnostic
            .as_deref()
            .is_some_and(|value| value.contains("Recordul copy WAL rămâne hot")),
        "{effect:?}"
    );
    assert_eq!(guard.phase(), WalPhase::TargetDurable);
    assert_eq!(fs::read(&fixture.target).unwrap(), b"public competitor");
    assert_eq!(
        fs::read(displaced_boundary.join("nested/target.txt")).unwrap(),
        b"initializer payload"
    );
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    let scan = restarted.snapshot().unwrap();
    assert!(scan.blocked, "{scan:?}");
    assert_eq!(
        scan.items[0].classification,
        super::super::WriteAuthorityRecoveryClassification::Conflict
    );
    assert_eq!(fs::read(&fixture.target).unwrap(), b"public competitor");
    assert_eq!(
        fs::read(displaced_boundary.join("nested/target.txt")).unwrap(),
        b"initializer payload"
    );
    drop(restarted);
    fixture.cleanup();
}

#[test]
fn restart_rename_recovery_recaptures_cross_authority_destination() {
    let fixture = AtomicRecoveryFixture::new("rename-cross-authority", true);
    let destination_boundary = fixture.root.join("app-data");
    let destination_path = destination_boundary.join("sessions/runtime/trash/target.txt");
    fs::create_dir_all(&destination_boundary).unwrap();
    let source_authority = capability::capture_directory_authority(
        &fixture.boundary,
        "test/rename-cross-source",
        DirectoryAuthorityScope::ProjectRoot,
    )
    .unwrap();
    let destination_authority = capability::capture_directory_authority(
        &destination_boundary,
        "test/rename-cross-destination",
        DirectoryAuthorityScope::ApplicationData,
    )
    .unwrap();
    let metadata = fs::symlink_metadata(&fixture.target).unwrap();
    let source = WriteTarget::new(
        &fixture.target,
        &fixture.boundary,
        "test/rename-cross-source-leaf",
    )
    .with_expected_present(
        project_disk_metadata_version_token(&metadata),
        Some(hash_bytes(&fs::read(&fixture.target).unwrap())),
    )
    .bind_authority(source_authority)
    .unwrap();
    let destination = WriteTarget::new(
        &destination_path,
        &destination_boundary,
        "test/rename-cross-destination-leaf",
    )
    .with_expected_absent()
    .bind_authority(destination_authority)
    .unwrap();
    let intent = WriteIntent::new(
        WriteCategory::ProjectSourceWrite,
        WriteOwner::ProjectWorkspace,
        WriteOperationKind::Rename,
        source,
        WritePolicy::project_entry_rename(),
        "Cross-authority rename recovery fixture.",
    );
    let plan = capability::plan_rename(&intent.target, &destination).unwrap();
    let record = build_rename_wal_record("wal-rename-cross-authority", 1, &intent, &plan).unwrap();
    let coordinator = fixture.restart_coordinator();
    let mut guard = coordinator.begin(record).unwrap();
    let effect =
        capability::rename_entry_wal(&intent.target, &destination, plan, &mut guard).unwrap();
    assert!(effect.changed, "{effect:?}");
    assert_eq!(guard.phase(), WalPhase::TargetDurable);
    drop(guard);
    drop(coordinator);

    let restarted = fixture.restart_coordinator();
    assert!(!restarted.snapshot().unwrap().blocked);
    assert!(!fixture.target.exists());
    assert_eq!(fs::read(&destination_path).unwrap(), b"baseline");
    fixture.cleanup();
}

#[test]
fn project_initializer_copy_v2_crash_matrix_is_restart_idempotent() {
    for (label, checkpoint, expected) in [
        (
            "anonymous-checkpoint",
            CopyV2CrashCheckpoint::AnonymousStageCheckpoint,
            CopyV2ExpectedTarget::Absent,
        ),
        (
            "target-link",
            CopyV2CrashCheckpoint::TargetLinkBeforePhase,
            CopyV2ExpectedTarget::Payload,
        ),
        (
            "target-fsync",
            CopyV2CrashCheckpoint::TargetFsync,
            CopyV2ExpectedTarget::Payload,
        ),
        (
            "target-durable",
            CopyV2CrashCheckpoint::TargetDurable,
            CopyV2ExpectedTarget::Payload,
        ),
    ] {
        run_copy_v2_crash_restart_case(
            &format!("copy-v2-initializer-{label}"),
            WriteOwner::ProjectInitializer,
            CapabilityReplacePolicy::CreateNew,
            false,
            checkpoint,
            expected,
        );
    }
}

#[test]
fn preview_copy_v2_crash_matrix_is_restart_idempotent() {
    for (label, checkpoint, expected) in [
        (
            "anonymous-checkpoint",
            CopyV2CrashCheckpoint::AnonymousStageCheckpoint,
            CopyV2ExpectedTarget::Baseline,
        ),
        (
            "temporary-link",
            CopyV2CrashCheckpoint::TemporaryLinkBeforePhase,
            CopyV2ExpectedTarget::Payload,
        ),
        (
            "rename",
            CopyV2CrashCheckpoint::RenameBeforePhase,
            CopyV2ExpectedTarget::Payload,
        ),
        (
            "target-fsync",
            CopyV2CrashCheckpoint::TargetFsync,
            CopyV2ExpectedTarget::Payload,
        ),
        (
            "target-durable",
            CopyV2CrashCheckpoint::TargetDurable,
            CopyV2ExpectedTarget::Payload,
        ),
    ] {
        run_copy_v2_crash_restart_case(
            &format!("copy-v2-preview-{label}"),
            WriteOwner::Preview,
            CapabilityReplacePolicy::Replace,
            true,
            checkpoint,
            expected,
        );
    }
}
