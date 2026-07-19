#[cfg(target_os = "linux")]
mod linux {
    use std::{
        ffi::OsString,
        fs::{self, OpenOptions},
        io::Write,
        os::unix::ffi::OsStringExt,
        os::unix::fs::{symlink, MetadataExt, PermissionsExt},
        panic::{catch_unwind, AssertUnwindSafe},
        path::{Path, PathBuf},
        sync::{mpsc, Arc},
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use crate::kernel::write_authority::{
        capability::{self, CapabilityReplacePolicy},
        model::{
            WriteCategory, WriteIntent, WriteOperationKind, WriteOwner, WritePolicy, WriteTarget,
        },
        operation::{
            build_append_wal_record, build_atomic_wal_record, build_copy_wal_record,
            build_directory_wal_record, build_external_config_wal_record,
            build_remove_leaf_wal_record, build_remove_tree_wal_record, build_rename_wal_record,
            build_symlink_wal_record,
        },
        root_authority::DirectoryAuthorityScope,
    };

    use super::super::{
        model::{
            WalOperationEvidence, WalPhase, WriteAuthorityRecoveryResolutionAction,
            WriteAuthorityRecoveryResolutionInput,
            WRITE_AUTHORITY_RECOVERY_RESOLUTION_SCHEMA_VERSION,
        },
        paths::{
            WalAppendStageCheckpoint, WalAppendStageRole, WalCopyStageCheckpoint, WalCopyStageRole,
            WalRecordName,
        },
        RecoveryCoordinator, RecoveryReadBudget,
    };
    use crate::{
        kernel::file_buffer_store::hash_bytes, project::project_disk_metadata_version_token,
    };

    #[test]
    fn wal_phase_order_is_strict() {
        assert_eq!(WalPhase::Preparing.next(), Some(WalPhase::Prepared));
        assert_eq!(WalPhase::Prepared.next(), Some(WalPhase::AuxiliaryDurable));
        assert_eq!(
            WalPhase::AuxiliaryDurable.next(),
            Some(WalPhase::EffectVisible)
        );
        assert_eq!(
            WalPhase::EffectVisible.next(),
            Some(WalPhase::TargetDurable)
        );
        assert_eq!(WalPhase::TargetDurable.next(), None);
    }

    #[test]
    fn wal_rejects_operation_label_from_another_evidence_family() {
        let fixture = AtomicRecoveryFixture::new("family-mismatch", false);
        let (_coordinator, _plan, record) = fixture.prepare("wal-family-mismatch", b"payload");
        let mut body = record.body;
        body.operation = "append_text".into();
        let error = super::super::WalRecord::seal(body).unwrap_err();
        assert!(error.contains("familia incompatibilă"), "{error}");
        fixture.cleanup();
    }

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
    fn prepared_atomic_exact_no_effect_is_the_only_automatic_legacy_action() {
        let fixture = AtomicRecoveryFixture::new("prepared-atomic-no-effect", false);
        let (coordinator, _plan, record) =
            fixture.prepare("wal-prepared-atomic-no-effect", b"payload");
        let guard = coordinator.begin(record).unwrap();
        drop(guard);
        drop(coordinator);

        let restarted = fixture.restart_coordinator();
        assert!(!restarted.snapshot().unwrap().blocked);
        assert!(!fixture.target.exists());
        fixture.cleanup();
    }

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
                capability::external_stage_identity_digest_for_test(&fixture.target, "target")
                    .unwrap()
            } else {
                "a".repeat(32)
            };
            guard
                .mark_external_auxiliary_durable(
                    super::super::WalExternalStageCheckpoint::new(checkpoint_identity, None)
                        .unwrap(),
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
            let target_temp = fixture.parent.join(
                super::super::decode_component_hex(&plan.evidence.target.temp_leaf_hex).unwrap(),
            );
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
        let (coordinator, _plan, record) = fixture.prepare_external_config(
            "wal-external-v2-rollback-rename-crash",
            payload,
            &backup,
        );
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
        let effect =
            capability::create_directory_all_wal(&intent.target, &plan, &mut guard).unwrap();
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
    fn directory_v3_operator_accepts_only_the_bound_current_empty_directory() {
        let fixture = AtomicRecoveryFixture::new("mkdir-v3-accept-current", false);
        let target = fixture.parent.join("created");
        let operation_id = "wal-mkdir-v3-accept-current";
        let (coordinator, _intent, _plan, record) =
            fixture.prepare_directory_v2(operation_id, &target);
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
        let (coordinator, _intent, _plan, record) =
            fixture.prepare_directory_v2(operation_id, &target);
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
        let (coordinator, _intent, _plan, record) =
            fixture.prepare_directory_v2(operation_id, &target);
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
        let (coordinator, _intent, _plan, record) =
            fixture.prepare_directory_v2(operation_id, &target);
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
            serde_json::to_string(&WriteAuthorityRecoveryResolutionAction::AcceptCurrentState)
                .unwrap(),
            r#""accept_current_state""#
        );
        let decoded: WriteAuthorityRecoveryResolutionAction =
            serde_json::from_str(r#""accept_current_state""#).unwrap();
        assert_eq!(
            decoded,
            WriteAuthorityRecoveryResolutionAction::AcceptCurrentState
        );
        assert_eq!(WRITE_AUTHORITY_RECOVERY_RESOLUTION_SCHEMA_VERSION, 6);
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
        let (coordinator, _intent, _plan, record) =
            fixture.prepare_directory_v2(operation_id, &target);
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
        let (coordinator, _intent, _plan, record) =
            fixture.prepare_directory_v2(operation_id, &target);
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
    fn directory_v2_project_initializer_owner_contract_commits() {
        let fixture = AtomicRecoveryFixture::new("mkdir-v2-project-initializer", false);
        let target_path = fixture.parent.join("created");
        let authority = capability::capture_directory_authority(
            &fixture.boundary,
            "test/mkdir-v2-project-initializer",
            DirectoryAuthorityScope::ProjectBootstrap { lease_id: 42 },
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
            build_directory_wal_record("wal-mkdir-v2-project-initializer", 1, &intent, &plan)
                .unwrap();
        let coordinator = fixture.restart_coordinator();
        let mut guard = coordinator.begin(record).unwrap();
        let effect =
            capability::create_directory_all_wal(&intent.target, &plan, &mut guard).unwrap();
        assert!(effect.changed);
        assert!(!effect.recovery_required);
        guard.commit().unwrap();
        assert!(target_path.is_dir());
        drop(coordinator);
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
    fn directory_v2_record_contract_binds_protocol_mode_leaf_and_ancestor() {
        let fixture = AtomicRecoveryFixture::new("mkdir-v2-record-contract", false);
        let target = fixture.parent.join("created");
        let (_coordinator, _intent, _plan, record) =
            fixture.prepare_directory_v2("wal-mkdir-v2-record-contract", &target);

        let mut wrong_mode = record.body.clone();
        let super::super::WalOperationEvidence::Directory(evidence) =
            &mut wrong_mode.operation_evidence
        else {
            unreachable!()
        };
        evidence.desired_mode_bits = Some(0o700);
        let error = super::super::WalRecord::seal(wrong_mode).unwrap_err();
        assert!(
            error.contains("evidence Directory v2 incompletă"),
            "{error}"
        );

        let mut old_temp_protocol = record.body.clone();
        let super::super::WalOperationEvidence::Directory(evidence) =
            &mut old_temp_protocol.operation_evidence
        else {
            unreachable!()
        };
        evidence.protocol_version = 2;
        let error = super::super::WalRecord::seal(old_temp_protocol).unwrap_err();
        assert!(
            error.contains("protocolul Directory necunoscut 2"),
            "{error}"
        );

        let mut wrong_leaf = record.body.clone();
        let super::super::WalOperationEvidence::Directory(evidence) =
            &mut wrong_leaf.operation_evidence
        else {
            unreachable!()
        };
        evidence.target_leaf_hex = Some("666f6f".into());
        let error = super::super::WalRecord::seal(wrong_leaf).unwrap_err();
        assert!(
            error.contains("leaf-ul Directory direct inconsistent"),
            "{error}"
        );

        let mut wrong_ancestor = record.body;
        let super::super::WalOperationEvidence::Directory(evidence) =
            &mut wrong_ancestor.operation_evidence
        else {
            unreachable!()
        };
        evidence.existing_ancestor_identity.inode =
            evidence.existing_ancestor_identity.inode.saturating_add(1);
        let error = super::super::WalRecord::seal(wrong_ancestor).unwrap_err();
        assert!(error.contains("ancestor identity Directory v2"), "{error}");
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
    fn symlink_wal_round_trips_non_utf8_literal() {
        let fixture = AtomicRecoveryFixture::new("symlink-non-utf8", false);
        let target = fixture.parent.join("link");
        let source = PathBuf::from(OsString::from_vec(b"../\xff-target".to_vec()));
        let (coordinator, intent, plan, record) =
            fixture.prepare_symlink("wal-symlink-non-utf8", &target, &source);
        let mut guard = coordinator.begin(record).unwrap();
        let effect =
            capability::symlink_entry_wal(&intent.target, &source, &plan, &mut guard).unwrap();
        assert!(effect.changed);
        assert!(!effect.recovery_required);
        guard.commit().unwrap();
        assert_eq!(fs::read_link(&target).unwrap(), source);
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
        let effect =
            capability::symlink_entry_wal(&intent.target, &source, &plan, &mut guard).unwrap();
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
    fn symlink_v2_expected_leaf_contract_is_exact() {
        let fixture = AtomicRecoveryFixture::new("symlink-v2-expected-leaf", false);
        let target_path = fixture.parent.join("link");
        let source = PathBuf::from("desired");
        let authority = capability::capture_directory_authority(
            &fixture.boundary,
            "test/symlink-v2-expected-leaf",
            DirectoryAuthorityScope::ApplicationPreviewCache,
        )
        .unwrap();
        let unspecified_absent = WriteTarget::new(
            &target_path,
            &fixture.boundary,
            "test/symlink-v2-unspecified-absent",
        )
        .bind_authority(authority.clone())
        .unwrap();
        let absent_plan = capability::plan_symlink(&unspecified_absent, &source).unwrap();
        assert!(matches!(
            absent_plan.evidence.before,
            super::super::WalSymlinkBefore::Absent
        ));
        let present = WriteTarget::new(&target_path, &fixture.boundary, "test/symlink-v2-present")
            .with_expected_present("ignored", None)
            .bind_authority(authority.clone())
            .unwrap();
        let error = capability::plan_symlink(&present, &source).unwrap_err();
        assert!(error.contains("ExpectedLeaf::Present"), "{error}");

        symlink(&source, &target_path).unwrap();
        let absent = WriteTarget::new(&target_path, &fixture.boundary, "test/symlink-v2-absent")
            .with_expected_absent()
            .bind_authority(authority.clone())
            .unwrap();
        let error = capability::plan_symlink(&absent, &source).unwrap_err();
        assert!(error.contains("ExpectedLeaf::Absent"), "{error}");

        let unspecified = WriteTarget::new(
            &target_path,
            &fixture.boundary,
            "test/symlink-v2-unspecified",
        )
        .bind_authority(authority)
        .unwrap();
        let plan = capability::plan_symlink(&unspecified, &source).unwrap();
        assert!(matches!(
            plan.evidence.before,
            super::super::WalSymlinkBefore::Exact { .. }
        ));
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
        let effect =
            capability::symlink_entry_wal(&intent.target, &source, &plan, &mut guard).unwrap();
        assert!(!effect.changed, "{effect:?}");
        assert!(!effect.recovery_required, "{effect:?}");
        guard.abort_no_effect().unwrap();
        let after = fs::symlink_metadata(&target).unwrap();
        assert_eq!((after.dev(), after.ino()), (before.dev(), before.ino()));
        assert_eq!(fs::read_link(&target).unwrap(), source);
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
    fn symlink_v2_round_trips_non_utf8_literal_direct() {
        let fixture = AtomicRecoveryFixture::new("symlink-v2-non-utf8", false);
        let target = fixture.parent.join("link");
        let source = PathBuf::from(OsString::from_vec(b"../\xff-target".to_vec()));
        let (coordinator, intent, plan, record) =
            fixture.prepare_symlink_v2("wal-symlink-v2-non-utf8", &target, &source, true);
        let mut guard = coordinator.begin(record).unwrap();
        let effect =
            capability::symlink_entry_wal(&intent.target, &source, &plan, &mut guard).unwrap();
        assert!(effect.changed);
        assert!(!effect.recovery_required, "{effect:?}");
        guard.commit().unwrap();
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
    fn symlink_v2_checkpointed_crash_auto_finalizes_idempotently() {
        let fixture = AtomicRecoveryFixture::new("symlink-v2-checkpoint-finalize", false);
        let target = fixture.parent.join("link");
        let source = PathBuf::from("desired");
        let (coordinator, intent, plan, record) = fixture.prepare_symlink_v2(
            "wal-symlink-v2-checkpoint-finalize",
            &target,
            &source,
            true,
        );
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
    fn symlink_v2_record_rejects_wrong_owner_and_unknown_protocol() {
        let fixture = AtomicRecoveryFixture::new("symlink-v2-record-contract", false);
        let target = fixture.parent.join("link");
        let source = PathBuf::from("desired");
        let (_coordinator, _intent, _plan, record) =
            fixture.prepare_symlink_v2("wal-symlink-v2-record-owner", &target, &source, true);
        let mut wrong_owner = record.body;
        wrong_owner.owner = "kernel".into();
        let error = super::super::WalRecord::seal(wrong_owner).unwrap_err();
        assert!(error.contains("owner/category/scope/policy"), "{error}");

        let (_coordinator, _intent, _plan, record) =
            fixture.prepare_symlink_v2("wal-symlink-v2-record-protocol", &target, &source, true);
        let mut unknown_protocol = record.body;
        let WalOperationEvidence::Symlink(evidence) = &mut unknown_protocol.operation_evidence
        else {
            unreachable!()
        };
        evidence.protocol_version = 99;
        let error = super::super::WalRecord::seal(unknown_protocol).unwrap_err();
        assert!(
            error.contains("protocolul Symlink necunoscut 99"),
            "{error}"
        );

        symlink(&source, &target).unwrap();
        let (_coordinator, _intent, _plan, record) = fixture.prepare_symlink_v2(
            "wal-symlink-v2-record-exact-literal",
            &target,
            &source,
            false,
        );
        let mut inconsistent_exact = record.body;
        let WalOperationEvidence::Symlink(evidence) = &mut inconsistent_exact.operation_evidence
        else {
            unreachable!()
        };
        evidence.desired_link_target_hex =
            super::super::encode_path_hex(Path::new("different-desired"));
        let error = super::super::WalRecord::seal(inconsistent_exact).unwrap_err();
        assert!(error.contains("literal diferit"), "{error}");
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
    fn copy_auxiliary_checkpoint_is_published_durably_in_the_wal_name() {
        let fixture = AtomicRecoveryFixture::new("copy-checkpoint-name", false);
        let source = fixture.root.join("source.bin");
        fs::write(&source, b"checkpoint payload").unwrap();
        let operation_id = "wal-copy-checkpoint-name";
        let (coordinator, _intent, plan, record) = fixture.prepare_copy(
            operation_id,
            &fixture.target,
            &source,
            CapabilityReplacePolicy::Replace,
        );
        let checkpoint = super::super::WalCopyStageCheckpoint::new(
            "b".repeat(32),
            &plan.evidence.file.new_content_hash,
            plan.evidence.file.new_size,
            plan.evidence.new_mode_bits,
            super::super::WalCopyStageRole::ReplaceTemporary,
        )
        .unwrap();
        let mut guard = coordinator.begin(record).unwrap();
        guard
            .mark_copy_auxiliary_durable(checkpoint.clone())
            .unwrap();
        assert_eq!(guard.phase(), WalPhase::AuxiliaryDurable);

        let file_name = fs::read_dir(&fixture.wal)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .find(|name| name.starts_with(operation_id))
            .unwrap();
        let parsed = super::super::paths::WalRecordName::parse(&file_name).unwrap();
        assert_eq!(parsed.copy_stage_checkpoint, Some(checkpoint));
        drop(plan);
        drop(guard);
        fixture.cleanup();
    }

    #[test]
    fn copy_v2_checkpoint_hook_scaffolding_is_scoped_and_runtime_inert() {
        assert_eq!(
            capability::with_after_copy_anonymous_stage_checkpoint_hook_for_test(
                || panic!("checkpointul nu trebuie publicat fără un apel explicit din Copy v2"),
                || 1,
            ),
            1
        );
        assert_eq!(
            capability::with_after_copy_temporary_link_before_phase_hook_for_test(
                || panic!("checkpointul nu trebuie publicat fără un apel explicit din Copy v2"),
                || 2,
            ),
            2
        );
        assert_eq!(
            capability::with_after_copy_target_link_before_phase_hook_for_test(
                || panic!("checkpointul nu trebuie publicat fără un apel explicit din Copy v2"),
                || 3,
            ),
            3
        );
        assert_eq!(
            capability::with_after_copy_rename_before_phase_hook_for_test(
                || panic!("checkpointul nu trebuie publicat fără un apel explicit din Copy v2"),
                || 4,
            ),
            4
        );
        assert_eq!(
            capability::with_after_copy_target_fsync_hook_for_test(
                || panic!("checkpointul nu trebuie publicat fără un apel explicit din Copy v2"),
                || 5,
            ),
            5
        );
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
    fn atomic_recovery_read_budget_is_aggregate_and_fail_closed() {
        let fixture = AtomicRecoveryFixture::new("atomic-recovery-budget", true);
        let (_coordinator, _plan, record) =
            fixture.prepare("wal-atomic-recovery-budget", b"replacement");
        let mut budget = RecoveryReadBudget::with_limit(1);
        let error = capability::classify_atomic_recovery(&record, WalPhase::Prepared, &mut budget)
            .unwrap_err();
        assert!(error.contains("bugetul agregat de citire"), "{error}");
        assert_eq!(fs::read(&fixture.target).unwrap(), b"baseline");
        fixture.cleanup();
    }

    #[test]
    fn recovery_treats_no_effect_shape_as_conflict_after_effect_visible_phase() {
        let fixture = AtomicRecoveryFixture::new("atomic-phase-no-effect", false);
        let (coordinator, plan, record) =
            fixture.prepare("wal-atomic-phase-no-effect", b"replacement");
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
    fn copy_wal_create_preserves_payload_mode_and_clears_record() {
        let fixture = AtomicRecoveryFixture::new("copy-create", false);
        let source = fixture.root.join("source.bin");
        fs::write(&source, b"copy payload").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o640)).unwrap();
        let (coordinator, intent, plan, record) = fixture.prepare_copy(
            "wal-copy-create",
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
        assert!(effect.changed);
        assert!(!effect.recovery_required, "{effect:?}");
        guard.commit().unwrap();
        assert!(!coordinator.snapshot().unwrap().blocked);
        assert_eq!(fs::read(&fixture.target).unwrap(), b"copy payload");
        assert_eq!(
            fs::metadata(&fixture.target).unwrap().permissions().mode() & 0o7777,
            0o640
        );
        fixture.cleanup();
    }

    #[test]
    fn copy_wal_clear_failure_before_unlink_keeps_terminal_record_hot() {
        let fixture = AtomicRecoveryFixture::new("copy-wal-clear-failure", false);
        let source = fixture.root.join("source.bin");
        fs::write(&source, b"copy terminal payload").unwrap();
        let (coordinator, intent, plan, record) = fixture.prepare_copy(
            "wal-copy-clear-failure",
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

        let error =
            super::super::wal_io::with_record_remove_failure_before_unlink(|| guard.commit())
                .unwrap_err();
        assert!(
            error.contains("failure injection înainte de unlink"),
            "{error}"
        );
        assert!(coordinator.snapshot().unwrap().blocked);
        assert_eq!(fs::read(&fixture.target).unwrap(), b"copy terminal payload");
        let hot_records = fs::read_dir(&fixture.wal)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .contains("wal-copy-clear-failure")
            })
            .count();
        assert_eq!(hot_records, 1);
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
    fn copy_checkpoint_filename_is_rejected_for_non_copy_record_family() {
        let fixture = AtomicRecoveryFixture::new("copy-v2-family-binding", false);
        let target = fixture.parent.join("directory-target");
        let operation_id = "wal-copy-v2-family-binding";
        let (coordinator, _intent, _plan, record) =
            fixture.prepare_directory(operation_id, &target);
        let guard = coordinator.begin(record).unwrap();
        drop(guard);
        drop(coordinator);

        let forged = WalRecordName::with_copy_stage_checkpoint(
            operation_id,
            WalPhase::AuxiliaryDurable,
            WalCopyStageCheckpoint::new(
                "a".repeat(32),
                &"b".repeat(64),
                1,
                0o600,
                WalCopyStageRole::CreateTarget,
            )
            .unwrap(),
        )
        .unwrap();
        fs::rename(
            fixture.wal.join(format!("{operation_id}.prepared.json")),
            fixture.wal.join(&forged.file_name),
        )
        .unwrap();

        let restarted = fixture.restart_coordinator();
        let scan = restarted.snapshot().unwrap();
        assert!(scan.blocked, "{scan:?}");
        assert_eq!(scan.record_count, 1);
        assert_eq!(
            scan.items[0].classification,
            super::super::WriteAuthorityRecoveryClassification::Conflict
        );
        assert!(!scan.items[0].automatic_recovery_available);
        assert!(scan.items[0].diagnostic.contains("familia recordului"));
        assert!(!target.exists());
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
    fn wal_copy_record_rejects_invalid_mode_contract() {
        let fixture = AtomicRecoveryFixture::new("copy-invalid-record", false);
        let source = fixture.root.join("source.bin");
        fs::write(&source, b"payload").unwrap();
        let (_coordinator, _intent, _plan, record) = fixture.prepare_copy(
            "wal-copy-invalid-record",
            &fixture.target,
            &source,
            CapabilityReplacePolicy::Replace,
        );
        let mut body = record.body;
        let super::super::WalOperationEvidence::Copy(evidence) = &mut body.operation_evidence
        else {
            unreachable!();
        };
        evidence.new_mode_bits = 0o10_000;
        let error = super::super::WalRecord::seal(body).unwrap_err();
        assert!(error.contains("evidence copy invalidă"), "{error}");
        fixture.cleanup();
    }

    #[test]
    fn wal_copy_record_rejects_unknown_protocol_version() {
        let fixture = AtomicRecoveryFixture::new("copy-unknown-protocol", false);
        let source = fixture.root.join("source.bin");
        fs::write(&source, b"payload").unwrap();
        let (_coordinator, _intent, _plan, record) = fixture.prepare_copy(
            "wal-copy-unknown-protocol",
            &fixture.target,
            &source,
            CapabilityReplacePolicy::Replace,
        );
        let mut body = record.body;
        let super::super::WalOperationEvidence::Copy(evidence) = &mut body.operation_evidence
        else {
            unreachable!();
        };
        evidence.protocol_version = 99;
        let error = super::super::WalRecord::seal(body).unwrap_err();
        assert!(error.contains("evidence copy invalidă"), "{error}");
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
        let record =
            build_rename_wal_record("wal-rename-cross-authority", 1, &intent, &plan).unwrap();
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
        let (coordinator, intent, plan, record) =
            fixture.prepare_remove_leaf("wal-remove-parent-swap");
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

    struct AtomicRecoveryFixture {
        root: PathBuf,
        boundary: PathBuf,
        parent: PathBuf,
        target: PathBuf,
        wal: PathBuf,
    }

    impl AtomicRecoveryFixture {
        fn new(label: &str, with_target: bool) -> Self {
            let root = unique_test_dir(label);
            let boundary = root.join("boundary");
            let parent = boundary.join("nested");
            let target = parent.join("target.txt");
            let wal = root.join("app-data/kernel/write-authority-wal");
            fs::create_dir_all(&parent).unwrap();
            fs::create_dir_all(&wal).unwrap();
            if with_target {
                fs::write(&target, b"baseline").unwrap();
            }
            Self {
                root,
                boundary,
                parent,
                target,
                wal,
            }
        }

        fn prepare(
            &self,
            operation_id: &str,
            payload: &[u8],
        ) -> (
            RecoveryCoordinator,
            crate::kernel::write_authority::operation::AtomicOperationPlan,
            super::super::WalRecord,
        ) {
            let target_authority = capability::capture_directory_authority(
                &self.boundary,
                "test/recovery-target",
                DirectoryAuthorityScope::ProjectRoot,
            )
            .unwrap();
            let target = WriteTarget::new(&self.target, &self.boundary, "test/recovery-atomic")
                .bind_authority(target_authority)
                .unwrap();
            let intent = WriteIntent::new(
                WriteCategory::InternalAppWrite,
                WriteOwner::Kernel,
                WriteOperationKind::WriteBytes,
                target,
                WritePolicy::internal_atomic(),
                "Recovery crash fixture.",
            );
            let plan = capability::plan_atomic_write(
                &intent.target,
                payload,
                CapabilityReplacePolicy::Replace,
                operation_id,
            )
            .unwrap();
            let record = build_atomic_wal_record(operation_id, 1, &intent, &plan).unwrap();
            (self.restart_coordinator(), plan, record)
        }

        fn prepare_append(
            &self,
            operation_id: &str,
            payload: &[u8],
        ) -> (RecoveryCoordinator, super::super::WalRecord) {
            let target_authority = capability::capture_directory_authority(
                &self.boundary,
                "test/append-recovery-target",
                DirectoryAuthorityScope::ProjectRoot,
            )
            .unwrap();
            let target = WriteTarget::new(&self.target, &self.boundary, "test/recovery-append")
                .bind_authority(target_authority)
                .unwrap();
            let intent = WriteIntent::new(
                WriteCategory::InternalAppWrite,
                WriteOwner::Kernel,
                WriteOperationKind::AppendText,
                target,
                WritePolicy::internal_append(),
                "Append recovery crash fixture.",
            );
            let plan = capability::plan_legacy_append_for_test(&intent.target, payload).unwrap();
            let record = build_append_wal_record(operation_id, 1, &intent, &plan).unwrap();
            drop(plan);
            (self.restart_coordinator(), record)
        }

        fn prepare_external_config(
            &self,
            operation_id: &str,
            payload: &[u8],
            backup_path: &Path,
        ) -> (
            RecoveryCoordinator,
            crate::kernel::write_authority::operation::ExternalConfigOperationPlan,
            super::super::WalRecord,
        ) {
            self.prepare_external_config_with_previous(
                operation_id,
                payload,
                backup_path,
                b"baseline",
            )
        }

        fn prepare_external_config_create_new(
            &self,
            operation_id: &str,
            payload: &[u8],
        ) -> (
            RecoveryCoordinator,
            crate::kernel::write_authority::operation::ExternalConfigOperationPlan,
            super::super::WalRecord,
        ) {
            let authority = capability::capture_directory_authority(
                &self.boundary,
                "test/external-config-create-recovery-target",
                DirectoryAuthorityScope::ExternalCodex { lease_id: 1 },
            )
            .unwrap();
            let target = WriteTarget::new(
                &self.target,
                &self.boundary,
                "test/recovery-external-config-create",
            )
            .bind_authority(authority)
            .unwrap();
            let intent = WriteIntent::new(
                WriteCategory::ExternalIntegrationWrite,
                WriteOwner::CodexMcp,
                WriteOperationKind::ExternalConfigUpdate,
                target,
                WritePolicy::external_config_update(),
                "External config create-new recovery crash fixture.",
            );
            let plan =
                capability::plan_external_config(&intent.target, payload, None, operation_id)
                    .unwrap();
            let record = build_external_config_wal_record(operation_id, 1, &intent, &plan).unwrap();
            (self.restart_coordinator(), plan, record)
        }

        fn prepare_external_config_with_previous(
            &self,
            operation_id: &str,
            payload: &[u8],
            backup_path: &Path,
            previous: &[u8],
        ) -> (
            RecoveryCoordinator,
            crate::kernel::write_authority::operation::ExternalConfigOperationPlan,
            super::super::WalRecord,
        ) {
            let authority = capability::capture_directory_authority(
                &self.boundary,
                "test/external-config-recovery-target",
                DirectoryAuthorityScope::ExternalCodex { lease_id: 1 },
            )
            .unwrap();
            let target = WriteTarget::new(
                &self.target,
                &self.boundary,
                "test/recovery-external-config",
            )
            .bind_authority(authority.clone())
            .unwrap();
            let backup = WriteTarget::new(
                backup_path,
                &self.boundary,
                "test/recovery-external-config-backup",
            )
            .bind_authority(authority)
            .unwrap();
            let intent = WriteIntent::new(
                WriteCategory::ExternalIntegrationWrite,
                WriteOwner::CodexMcp,
                WriteOperationKind::ExternalConfigUpdate,
                target,
                WritePolicy::external_config_update(),
                "External config recovery crash fixture.",
            );
            let plan = capability::plan_external_config(
                &intent.target,
                payload,
                Some((&backup, previous)),
                operation_id,
            )
            .unwrap();
            let record = build_external_config_wal_record(operation_id, 1, &intent, &plan).unwrap();
            (self.restart_coordinator(), plan, record)
        }

        fn materialize_external_relocated_baseline(
            &self,
            plan: &crate::kernel::write_authority::operation::ExternalConfigOperationPlan,
            guard: &mut super::super::DurableWalGuard<'_>,
            backup_path: &Path,
            phase: WalPhase,
        ) {
            assert_eq!(
                plan.evidence.protocol_version,
                super::super::WAL_EXTERNAL_CONFIG_PROTOCOL_VERSION
            );
            guard
                .mark_external_auxiliary_durable(
                    super::super::WalExternalStageCheckpoint::new("a".repeat(32), None).unwrap(),
                )
                .unwrap();
            fs::rename(&self.target, backup_path).unwrap();
            if phase >= WalPhase::EffectVisible {
                guard.mark_effect_visible().unwrap();
            }
            fs::File::open(&self.parent).unwrap().sync_all().unwrap();
        }

        fn materialize_external_committed_pair(
            &self,
            plan: &crate::kernel::write_authority::operation::ExternalConfigOperationPlan,
            guard: &mut super::super::DurableWalGuard<'_>,
            payload: &[u8],
            backup_path: &Path,
            phase: WalPhase,
        ) {
            assert_eq!(
                plan.evidence.protocol_version,
                super::super::WAL_EXTERNAL_CONFIG_PROTOCOL_VERSION
            );
            fs::rename(&self.target, backup_path).unwrap();
            fs::write(&self.target, payload).unwrap();
            fs::set_permissions(
                &self.target,
                fs::Permissions::from_mode(plan.evidence.target_new_mode_bits),
            )
            .unwrap();
            let checkpoint = super::super::WalExternalStageCheckpoint::new(
                capability::external_stage_identity_digest_for_test(&self.target, "target")
                    .unwrap(),
                None,
            )
            .unwrap();
            guard.mark_external_auxiliary_durable(checkpoint).unwrap();
            if phase >= WalPhase::EffectVisible {
                guard.mark_effect_visible().unwrap();
            }
            if phase >= WalPhase::TargetDurable {
                guard.mark_target_durable().unwrap();
            }
            fs::File::open(&self.target).unwrap().sync_all().unwrap();
            fs::File::open(backup_path).unwrap().sync_all().unwrap();
            fs::File::open(&self.parent).unwrap().sync_all().unwrap();
        }

        fn prepare_copy(
            &self,
            operation_id: &str,
            target_path: &Path,
            source: &Path,
            replace_policy: CapabilityReplacePolicy,
        ) -> (
            RecoveryCoordinator,
            WriteIntent,
            crate::kernel::write_authority::operation::CopyOperationPlan,
            super::super::WalRecord,
        ) {
            self.prepare_copy_for_owner(
                operation_id,
                target_path,
                source,
                WriteOwner::Preview,
                replace_policy,
            )
        }

        fn prepare_legacy_copy(
            &self,
            operation_id: &str,
            target_path: &Path,
            source: &Path,
            replace_policy: CapabilityReplacePolicy,
        ) -> (
            RecoveryCoordinator,
            WriteIntent,
            crate::kernel::write_authority::operation::CopyOperationPlan,
            super::super::WalRecord,
        ) {
            let (coordinator, intent, mut plan, _) =
                self.prepare_copy(operation_id, target_path, source, replace_policy);
            plan.evidence.protocol_version = 0;
            let record = build_copy_wal_record(operation_id, 1, &intent, &plan).unwrap();
            (coordinator, intent, plan, record)
        }

        fn prepare_copy_for_owner(
            &self,
            operation_id: &str,
            target_path: &Path,
            source: &Path,
            owner: WriteOwner,
            replace_policy: CapabilityReplacePolicy,
        ) -> (
            RecoveryCoordinator,
            WriteIntent,
            crate::kernel::write_authority::operation::CopyOperationPlan,
            super::super::WalRecord,
        ) {
            let (scope, category, policy, description) = match owner {
                WriteOwner::Preview => (
                    DirectoryAuthorityScope::ApplicationPreviewCache,
                    WriteCategory::PreviewWorkspaceWrite,
                    WritePolicy::preview_workspace_lifecycle(),
                    "Preview Copy recovery crash fixture.",
                ),
                WriteOwner::ProjectInitializer => (
                    DirectoryAuthorityScope::ProjectBootstrap { lease_id: 1 },
                    WriteCategory::ProjectSourceWrite,
                    WritePolicy::project_creation_lifecycle(),
                    "Project Initializer Copy recovery crash fixture.",
                ),
                _ => panic!("Copy recovery fixture accepts only the two authorized owners"),
            };
            let target_authority = capability::capture_directory_authority(
                &self.boundary,
                "test/copy-recovery-target",
                scope,
            )
            .unwrap();
            let target = WriteTarget::new(target_path, &self.boundary, "test/recovery-copy")
                .bind_authority(target_authority)
                .unwrap();
            let intent = WriteIntent::new(
                category,
                owner,
                WriteOperationKind::Copy,
                target,
                policy,
                description,
            );
            let plan = capability::plan_copy(&intent.target, source, replace_policy, operation_id)
                .unwrap();
            let record = build_copy_wal_record(operation_id, 1, &intent, &plan).unwrap();
            (self.restart_coordinator(), intent, plan, record)
        }

        fn prepare_rename(
            &self,
            operation_id: &str,
            destination_path: &Path,
        ) -> (
            RecoveryCoordinator,
            WriteIntent,
            WriteTarget,
            crate::kernel::write_authority::operation::RenameOperationPlan,
            super::super::WalRecord,
        ) {
            let authority = capability::capture_directory_authority(
                &self.boundary,
                "test/rename-recovery-target",
                DirectoryAuthorityScope::ProjectRoot,
            )
            .unwrap();
            let metadata = fs::symlink_metadata(&self.target).unwrap();
            let source =
                WriteTarget::new(&self.target, &self.boundary, "test/recovery-rename-source")
                    .with_expected_present(
                        project_disk_metadata_version_token(&metadata),
                        Some(hash_bytes(&fs::read(&self.target).unwrap())),
                    )
                    .bind_authority(authority.clone())
                    .unwrap();
            let destination = WriteTarget::new(
                destination_path,
                &self.boundary,
                "test/recovery-rename-destination",
            )
            .with_expected_absent()
            .bind_authority(authority)
            .unwrap();
            let intent = WriteIntent::new(
                WriteCategory::ProjectSourceWrite,
                WriteOwner::ProjectWorkspace,
                WriteOperationKind::Rename,
                source,
                WritePolicy::project_entry_rename(),
                "Rename recovery crash fixture.",
            );
            let plan = capability::plan_rename(&intent.target, &destination).unwrap();
            let record = build_rename_wal_record(operation_id, 1, &intent, &plan).unwrap();
            (
                self.restart_coordinator(),
                intent,
                destination,
                plan,
                record,
            )
        }

        fn prepare_remove_leaf(
            &self,
            operation_id: &str,
        ) -> (
            RecoveryCoordinator,
            WriteIntent,
            crate::kernel::write_authority::operation::RemoveLeafOperationPlan,
            super::super::WalRecord,
        ) {
            let authority = capability::capture_directory_authority(
                &self.boundary,
                "test/remove-leaf-recovery-target",
                DirectoryAuthorityScope::ProjectRoot,
            )
            .unwrap();
            let metadata = fs::symlink_metadata(&self.target).unwrap();
            let target =
                WriteTarget::new(&self.target, &self.boundary, "test/recovery-remove-leaf")
                    .with_expected_present(
                        project_disk_metadata_version_token(&metadata),
                        Some(hash_bytes(&fs::read(&self.target).unwrap())),
                    )
                    .bind_authority(authority)
                    .unwrap();
            let intent = WriteIntent::new(
                WriteCategory::ProjectSourceWrite,
                WriteOwner::ProjectWorkspace,
                WriteOperationKind::RemoveFile,
                target,
                WritePolicy::project_workspace_remove(),
                "Remove leaf recovery crash fixture.",
            );
            let plan = capability::plan_remove_leaf(&intent.target, operation_id)
                .unwrap()
                .unwrap();
            let record = build_remove_leaf_wal_record(operation_id, 1, &intent, &plan).unwrap();
            (self.restart_coordinator(), intent, plan, record)
        }

        fn prepare_remove_leaf_unchecked(
            &self,
            operation_id: &str,
        ) -> (
            RecoveryCoordinator,
            WriteIntent,
            crate::kernel::write_authority::operation::RemoveLeafOperationPlan,
            super::super::WalRecord,
        ) {
            let authority = capability::capture_directory_authority(
                &self.boundary,
                "test/remove-leaf-unchecked-target",
                DirectoryAuthorityScope::ProjectRoot,
            )
            .unwrap();
            let target = WriteTarget::new(
                &self.target,
                &self.boundary,
                "test/recovery-remove-leaf-unchecked",
            )
            .bind_authority(authority)
            .unwrap();
            let intent = WriteIntent::new(
                WriteCategory::InternalAppWrite,
                WriteOwner::Kernel,
                WriteOperationKind::RemoveFile,
                target,
                WritePolicy::internal_lifecycle(),
                "Remove leaf unchecked recovery fixture.",
            );
            let plan = capability::plan_remove_leaf(&intent.target, operation_id)
                .unwrap()
                .unwrap();
            let record = build_remove_leaf_wal_record(operation_id, 1, &intent, &plan).unwrap();
            (self.restart_coordinator(), intent, plan, record)
        }

        fn create_tree(&self) {
            fs::create_dir(&self.target).unwrap();
            fs::write(self.target.join("a.txt"), b"a").unwrap();
            fs::create_dir(self.target.join("nested")).unwrap();
            fs::write(self.target.join("nested/b.txt"), b"b").unwrap();
        }

        fn prepare_remove_tree(
            &self,
            operation_id: &str,
        ) -> (
            RecoveryCoordinator,
            WriteIntent,
            crate::kernel::write_authority::operation::RemoveTreeOperationPlan,
            super::super::WalRecord,
        ) {
            let authority = capability::capture_directory_authority(
                &self.boundary,
                "test/remove-tree-recovery-target",
                DirectoryAuthorityScope::ProjectRoot,
            )
            .unwrap();
            let target =
                WriteTarget::new(&self.target, &self.boundary, "test/recovery-remove-tree")
                    .bind_authority(authority)
                    .unwrap();
            let intent = WriteIntent::new(
                WriteCategory::PreviewWorkspaceWrite,
                WriteOwner::Preview,
                WriteOperationKind::RemoveDirectoryTree,
                target,
                WritePolicy::preview_workspace_lifecycle(),
                "Remove tree recovery crash fixture.",
            );
            let plan = capability::plan_remove_tree(&intent.target, operation_id)
                .unwrap()
                .unwrap();
            let record = build_remove_tree_wal_record(operation_id, 1, &intent, &plan).unwrap();
            (self.restart_coordinator(), intent, plan, record)
        }

        fn prepare_directory(
            &self,
            operation_id: &str,
            directory_path: &Path,
        ) -> (
            RecoveryCoordinator,
            WriteIntent,
            crate::kernel::write_authority::operation::DirectoryOperationPlan,
            super::super::WalRecord,
        ) {
            let target_authority = capability::capture_directory_authority(
                &self.boundary,
                "test/mkdir-recovery-target",
                DirectoryAuthorityScope::ProjectRoot,
            )
            .unwrap();
            let target =
                WriteTarget::new(directory_path, &self.boundary, "test/recovery-directory")
                    .bind_authority(target_authority)
                    .unwrap();
            let intent = WriteIntent::new(
                WriteCategory::InternalAppWrite,
                WriteOwner::Kernel,
                WriteOperationKind::CreateDirectory,
                target,
                WritePolicy::internal_lifecycle(),
                "Directory recovery crash fixture.",
            );
            // Aceste fixture-uri apără explicit compatibilitatea/fail-safe-ul
            // recordurilor mkdir legacy multi-component. Producția folosește
            // Directory v2 single-leaf și are teste separate.
            let plan = capability::plan_legacy_directory_for_test(&intent.target).unwrap();
            let record = build_directory_wal_record(operation_id, 1, &intent, &plan).unwrap();
            (self.restart_coordinator(), intent, plan, record)
        }

        fn prepare_directory_v2(
            &self,
            operation_id: &str,
            directory_path: &Path,
        ) -> (
            RecoveryCoordinator,
            WriteIntent,
            crate::kernel::write_authority::operation::DirectoryOperationPlan,
            super::super::WalRecord,
        ) {
            let target_authority = capability::capture_directory_authority(
                &self.boundary,
                "test/mkdir-v2-recovery-target",
                DirectoryAuthorityScope::ApplicationPreviewCache,
            )
            .unwrap();
            let target =
                WriteTarget::new(directory_path, &self.boundary, "test/recovery-directory-v2")
                    .bind_authority(target_authority)
                    .unwrap();
            let intent = WriteIntent::new(
                WriteCategory::PreviewWorkspaceWrite,
                WriteOwner::Preview,
                WriteOperationKind::CreateDirectory,
                target,
                WritePolicy::preview_workspace_lifecycle(),
                "Directory v2 recovery crash fixture.",
            );
            let plan = capability::plan_directory(&intent.target).unwrap();
            let record = build_directory_wal_record(operation_id, 1, &intent, &plan).unwrap();
            (self.restart_coordinator(), intent, plan, record)
        }

        fn prepare_symlink(
            &self,
            operation_id: &str,
            target_path: &Path,
            source: &Path,
        ) -> (
            RecoveryCoordinator,
            WriteIntent,
            crate::kernel::write_authority::operation::SymlinkOperationPlan,
            super::super::WalRecord,
        ) {
            let target_authority = capability::capture_directory_authority(
                &self.boundary,
                "test/symlink-recovery-target",
                DirectoryAuthorityScope::ProjectRoot,
            )
            .unwrap();
            let target = WriteTarget::new(target_path, &self.boundary, "test/recovery-symlink")
                .bind_authority(target_authority)
                .unwrap();
            let intent = WriteIntent::new(
                WriteCategory::InternalAppWrite,
                WriteOwner::Kernel,
                WriteOperationKind::Symlink,
                target,
                WritePolicy::internal_lifecycle(),
                "Symlink recovery crash fixture.",
            );
            // Aceste fixture-uri apără explicit recovery-ul protocolului
            // lifecycle legacy. Producția Preview folosește Symlink v2 direct
            // și are fixture-uri/teste separate.
            let plan = capability::plan_legacy_symlink_for_test(&intent.target, source).unwrap();
            let record = build_symlink_wal_record(operation_id, 1, &intent, &plan).unwrap();
            (self.restart_coordinator(), intent, plan, record)
        }

        fn prepare_symlink_v2(
            &self,
            operation_id: &str,
            target_path: &Path,
            source: &Path,
            expected_absent: bool,
        ) -> (
            RecoveryCoordinator,
            WriteIntent,
            crate::kernel::write_authority::operation::SymlinkOperationPlan,
            super::super::WalRecord,
        ) {
            let target_authority = capability::capture_directory_authority(
                &self.boundary,
                "test/symlink-v2-recovery-target",
                DirectoryAuthorityScope::ApplicationPreviewCache,
            )
            .unwrap();
            let target = WriteTarget::new(target_path, &self.boundary, "test/recovery-symlink-v2");
            let target = if expected_absent {
                target.with_expected_absent()
            } else {
                target
            }
            .bind_authority(target_authority)
            .unwrap();
            let intent = WriteIntent::new(
                WriteCategory::PreviewWorkspaceWrite,
                WriteOwner::Preview,
                WriteOperationKind::Symlink,
                target,
                WritePolicy::preview_workspace_lifecycle(),
                "Symlink v2 direct recovery crash fixture.",
            );
            let plan = capability::plan_symlink(&intent.target, source).unwrap();
            let record = build_symlink_wal_record(operation_id, 1, &intent, &plan).unwrap();
            (self.restart_coordinator(), intent, plan, record)
        }

        fn restart_coordinator(&self) -> RecoveryCoordinator {
            let wal_authority = capability::capture_directory_authority(
                &self.wal,
                "test/write-authority-wal",
                DirectoryAuthorityScope::ApplicationWriteAuthorityWal,
            )
            .unwrap();
            RecoveryCoordinator::bootstrap(wal_authority).unwrap()
        }

        fn cleanup(&self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    struct AppendV2Fixture {
        root: PathBuf,
        boundary: PathBuf,
        target: PathBuf,
        wal: PathBuf,
    }

    impl AppendV2Fixture {
        fn new(label: &str, with_target: bool) -> Self {
            let root = unique_test_dir(label);
            let boundary = root.join("application-data");
            let session = boundary.join("sessions/session-append-v2");
            let target = session.join("project-transition-decisions.jsonl");
            let wal = root.join("write-authority-wal");
            fs::create_dir_all(&session).unwrap();
            fs::create_dir_all(&wal).unwrap();
            if with_target {
                fs::write(&target, b"{\"baseline\":true}\n").unwrap();
            }
            Self {
                root,
                boundary,
                target,
                wal,
            }
        }

        fn prepare(
            &self,
            operation_id: &str,
            payload: &[u8],
        ) -> (
            RecoveryCoordinator,
            WriteIntent,
            crate::kernel::write_authority::operation::AppendOperationPlan,
            super::super::WalRecord,
        ) {
            let authority = capability::capture_directory_authority(
                &self.boundary,
                "test/append-v2-application-data",
                DirectoryAuthorityScope::ApplicationData,
            )
            .unwrap();
            let target = WriteTarget::new(
                &self.target,
                &self.boundary,
                "session/append-v2/project-transition-decisions.jsonl",
            )
            .bind_authority(authority)
            .unwrap();
            let intent = WriteIntent::new(
                WriteCategory::InternalAppWrite,
                WriteOwner::Kernel,
                WriteOperationKind::AppendText,
                target,
                WritePolicy::internal_append(),
                "Append v2 crash fixture.",
            );
            let plan = capability::plan_append(&intent.target, payload).unwrap();
            let record = build_append_wal_record(operation_id, 1, &intent, &plan).unwrap();
            (self.restart_coordinator(), intent, plan, record)
        }

        fn restart_coordinator(&self) -> RecoveryCoordinator {
            let authority = capability::capture_directory_authority(
                &self.wal,
                "test/append-v2-wal",
                DirectoryAuthorityScope::ApplicationWriteAuthorityWal,
            )
            .unwrap();
            RecoveryCoordinator::bootstrap(authority).unwrap()
        }

        fn cleanup(&self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[derive(Clone, Copy, Debug)]
    enum AppendV2CrashCheckpoint {
        Checkpoint,
        EffectBeforePhase,
        TargetFsync,
        TargetDurable,
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

    #[test]
    fn append_v2_checkpoint_is_family_and_protocol_bound() {
        let fixture = AppendV2Fixture::new("append-v2-family-binding", true);
        let payload = b"{\"binding\":true}\n";
        let (_coordinator, _intent, plan, record) =
            fixture.prepare("wal-append-v2-family-binding", payload);
        let before_size = match &plan.evidence.before {
            super::super::WalAppendBefore::Present { size, .. } => *size,
            super::super::WalAppendBefore::Absent => panic!("fixture must be Present"),
        };
        let checkpoint = WalAppendStageCheckpoint::new(
            plan.evidence
                .before_identity_digest
                .clone()
                .expect("Append v2 Present identity"),
            &plan.evidence.payload_hash,
            plan.evidence.payload_size,
            before_size,
            WalAppendStageRole::ExistingTarget,
        )
        .unwrap();
        let name = WalRecordName::with_append_stage_checkpoint(
            "wal-append-v2-family-binding",
            WalPhase::AuxiliaryDurable,
            checkpoint,
        )
        .unwrap();
        name.validate_family_metadata(&record.body.operation_evidence)
            .unwrap();
        let mut legacy = record.body.operation_evidence.clone();
        let super::super::WalOperationEvidence::Append(evidence) = &mut legacy else {
            unreachable!()
        };
        evidence.protocol_version = 0;
        assert!(name.validate_family_metadata(&legacy).is_err());
        fixture.cleanup();
    }

    #[test]
    fn append_v2_well_formed_but_wrong_checkpoint_contract_never_clears_wal() {
        let fixture = AppendV2Fixture::new("append-v2-wrong-checkpoint", true);
        let payload = b"{\"checkpoint\":true}\n";
        let operation_id = "wal-append-v2-wrong-checkpoint";
        let (coordinator, intent, plan, record) = fixture.prepare(operation_id, payload);
        let mut guard = coordinator.begin(record).unwrap();
        let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            capability::with_after_append_v2_checkpoint_hook_for_test(
                || panic!("checkpoint crash"),
                || capability::append_wal(&intent.target, payload, plan, &mut guard),
            )
        }));
        assert!(crashed.is_err());
        drop(guard);
        drop(coordinator);

        let old_entry = fs::read_dir(&fixture.wal)
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| entry.file_name().to_string_lossy().contains(".ape."))
            .expect("Append v2 checkpoint WAL");
        let parsed = WalRecordName::parse(&old_entry.file_name().to_string_lossy()).unwrap();
        let checkpoint = parsed.append_stage_checkpoint.unwrap();
        let forged = WalAppendStageCheckpoint::new(
            checkpoint.target_identity_digest,
            &"0".repeat(64),
            payload.len() as u64,
            b"{\"baseline\":true}\n".len() as u64,
            WalAppendStageRole::ExistingTarget,
        )
        .unwrap();
        let forged_name = WalRecordName::with_append_stage_checkpoint(
            operation_id,
            WalPhase::AuxiliaryDurable,
            forged,
        )
        .unwrap();
        fs::rename(old_entry.path(), fixture.wal.join(&forged_name.file_name)).unwrap();

        let restarted = fixture.restart_coordinator();
        let scan = restarted.snapshot().unwrap();
        assert!(scan.blocked, "{scan:?}");
        assert_eq!(
            scan.items[0].classification,
            super::super::WriteAuthorityRecoveryClassification::Conflict
        );
        assert_eq!(fs::read(&fixture.target).unwrap(), b"{\"baseline\":true}\n");
        assert!(fixture.wal.join(forged_name.file_name).exists());
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
            scan.items[0].diagnostic.contains("hash")
                || scan.items[0].diagnostic.contains("schimbat")
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
    fn append_v2_body_rejects_internal_contract_mutations() {
        let fixture = AppendV2Fixture::new("append-v2-body-contract", true);
        let payload = b"{\"body_contract\":true}\n";
        let (coordinator, _intent, _plan, record) =
            fixture.prepare("wal-append-v2-body-contract", payload);
        drop(coordinator);

        let mut wrong_complete = record.body.clone();
        let super::super::WalOperationEvidence::Append(evidence) =
            &mut wrong_complete.operation_evidence
        else {
            unreachable!()
        };
        evidence.payload_complete_in_record = !evidence.payload_complete_in_record;
        assert!(super::super::WalRecord::seal(wrong_complete).is_err());

        let mut wrong_tail = record.body.clone();
        let super::super::WalOperationEvidence::Append(evidence) =
            &mut wrong_tail.operation_evidence
        else {
            unreachable!()
        };
        evidence.before_tail_size = evidence.before_tail_size.saturating_sub(1);
        assert!(super::super::WalRecord::seal(wrong_tail).is_err());

        let mut wrong_parent = record.body.clone();
        let super::super::WalOperationEvidence::Append(evidence) =
            &mut wrong_parent.operation_evidence
        else {
            unreachable!()
        };
        evidence.parent.existing_prefix_len = 1;
        assert!(super::super::WalRecord::seal(wrong_parent).is_err());
        fixture.cleanup();
    }

    #[test]
    fn append_v2_payload_bound_accepts_256_kib_and_rejects_next_byte() {
        let fixture = AppendV2Fixture::new("append-v2-payload-bound", true);
        let max = super::super::MAX_WAL_APPEND_PAYLOAD_BYTES;
        let payload = format!("{{\"x\":\"{}\"}}\n", "a".repeat(max - 9)).into_bytes();
        assert_eq!(payload.len(), max);
        let (coordinator, _intent, _plan, record) =
            fixture.prepare("wal-append-v2-payload-bound", &payload);
        assert!(record.to_bytes().unwrap().len() <= 640 * 1024);
        drop(coordinator);

        let oversized = format!("{{\"x\":\"{}\"}}\n", "a".repeat(max - 8)).into_bytes();
        assert_eq!(oversized.len(), max + 1);
        let authority = capability::capture_directory_authority(
            &fixture.boundary,
            "test/append-v2-payload-bound",
            DirectoryAuthorityScope::ApplicationData,
        )
        .unwrap();
        let target = WriteTarget::new(
            &fixture.target,
            &fixture.boundary,
            "session/append-v2/transactions.jsonl",
        )
        .bind_authority(authority)
        .unwrap();
        assert!(capability::plan_append(&target, &oversized).is_err());
        fixture.cleanup();
    }

    fn run_append_v2_crash_case(with_target: bool, checkpoint: AppendV2CrashCheckpoint) {
        let label = format!("append-v2-{with_target}-{checkpoint:?}");
        let fixture = AppendV2Fixture::new(&label, with_target);
        let payload = b"{\"append_v2\":true}\n";
        let operation_id = format!("wal-{label}");
        let (coordinator, intent, plan, record) = fixture.prepare(&operation_id, payload);
        let mut guard = coordinator.begin(record).unwrap();
        let mut plan = Some(plan);
        let mut execute = || {
            capability::append_wal(
                &intent.target,
                payload,
                plan.take().expect("Append v2 plan consumed once"),
                &mut guard,
            )
        };
        let crash = || panic!("simulated Append v2 crash");
        let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match checkpoint {
            AppendV2CrashCheckpoint::Checkpoint => {
                capability::with_after_append_v2_checkpoint_hook_for_test(crash, &mut execute)
            }
            AppendV2CrashCheckpoint::EffectBeforePhase if with_target => {
                capability::with_after_append_v2_write_before_phase_hook_for_test(
                    crash,
                    &mut execute,
                )
            }
            AppendV2CrashCheckpoint::EffectBeforePhase => {
                capability::with_after_append_v2_link_before_phase_hook_for_test(
                    crash,
                    &mut execute,
                )
            }
            AppendV2CrashCheckpoint::TargetFsync => {
                capability::with_after_append_v2_target_fsync_hook_for_test(crash, &mut execute)
            }
            AppendV2CrashCheckpoint::TargetDurable => {
                capability::with_after_append_v2_target_durable_hook_for_test(crash, &mut execute)
            }
        }));
        assert!(crashed.is_err(), "{label}");
        drop(execute);
        let expected_phase = match checkpoint {
            AppendV2CrashCheckpoint::Checkpoint | AppendV2CrashCheckpoint::EffectBeforePhase => {
                WalPhase::AuxiliaryDurable
            }
            AppendV2CrashCheckpoint::TargetFsync => WalPhase::EffectVisible,
            AppendV2CrashCheckpoint::TargetDurable => WalPhase::TargetDurable,
        };
        assert_eq!(guard.phase(), expected_phase, "{label}");
        drop(guard);
        drop(coordinator);

        let restarted = fixture.restart_coordinator();
        let first_scan = restarted.snapshot().unwrap();
        if matches!(checkpoint, AppendV2CrashCheckpoint::Checkpoint) {
            assert!(first_scan.blocked, "{label}: {first_scan:?}");
            assert!(first_scan.items.iter().any(|item| {
                item.operation_id.as_deref() == Some(operation_id.as_str())
                    && item.classification
                        == super::super::WriteAuthorityRecoveryClassification::Conflict
                    && !item.automatic_recovery_available
            }));
        } else {
            assert!(!first_scan.blocked, "{label}: {first_scan:?}");
        }
        let expected = match (with_target, checkpoint) {
            (true, AppendV2CrashCheckpoint::Checkpoint) => b"{\"baseline\":true}\n".to_vec(),
            (false, AppendV2CrashCheckpoint::Checkpoint) => Vec::new(),
            (true, _) => [b"{\"baseline\":true}\n".as_slice(), payload.as_slice()].concat(),
            (false, _) => payload.to_vec(),
        };
        if expected.is_empty() {
            assert!(!fixture.target.exists(), "{label}");
        } else {
            assert_eq!(fs::read(&fixture.target).unwrap(), expected, "{label}");
        }
        drop(restarted);
        let second = fixture.restart_coordinator();
        assert_eq!(
            second.snapshot().unwrap().blocked,
            matches!(checkpoint, AppendV2CrashCheckpoint::Checkpoint),
            "{label}"
        );
        drop(second);
        fixture.cleanup();
    }

    #[derive(Clone, Copy, Debug)]
    enum CopyV2CrashCheckpoint {
        AnonymousStageCheckpoint,
        TemporaryLinkBeforePhase,
        TargetLinkBeforePhase,
        RenameBeforePhase,
        TargetFsync,
        TargetDurable,
    }

    #[derive(Clone, Copy, Debug)]
    enum CopyV2ExpectedTarget {
        Absent,
        Baseline,
        Payload,
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
        assert!(first_scan.items.iter().any(|item| {
            item.diagnostic.contains("hash") || item.diagnostic.contains("payload")
        }));
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

    fn run_copy_v2_crash_restart_case(
        label: &str,
        owner: WriteOwner,
        replace_policy: CapabilityReplacePolicy,
        with_target: bool,
        checkpoint: CopyV2CrashCheckpoint,
        expected_target: CopyV2ExpectedTarget,
    ) {
        let fixture = AtomicRecoveryFixture::new(label, with_target);
        let source = fixture.root.join("source.bin");
        let payload = format!("payload-{label}").into_bytes();
        fs::write(&source, &payload).unwrap();
        let operation_id = format!("wal-{label}");
        let (coordinator, intent, plan, record) = fixture.prepare_copy_for_owner(
            &operation_id,
            &fixture.target,
            &source,
            owner,
            replace_policy,
        );
        let temp = fixture.parent.join(plan.temp_leaf().unwrap());
        let mut guard = coordinator.begin(record).unwrap();
        let mut plan = Some(plan);
        let mut execute = || {
            capability::copy_file_wal(
                &intent.target,
                &source,
                replace_policy,
                plan.take().expect("Copy v2 plan is consumed once"),
                &mut guard,
            )
        };
        let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match checkpoint {
            CopyV2CrashCheckpoint::AnonymousStageCheckpoint => {
                capability::with_after_copy_anonymous_stage_checkpoint_hook_for_test(
                    copy_v2_crash_now,
                    &mut execute,
                )
            }
            CopyV2CrashCheckpoint::TemporaryLinkBeforePhase => {
                capability::with_after_copy_temporary_link_before_phase_hook_for_test(
                    copy_v2_crash_now,
                    &mut execute,
                )
            }
            CopyV2CrashCheckpoint::TargetLinkBeforePhase => {
                capability::with_after_copy_target_link_before_phase_hook_for_test(
                    copy_v2_crash_now,
                    &mut execute,
                )
            }
            CopyV2CrashCheckpoint::RenameBeforePhase => {
                capability::with_after_copy_rename_before_phase_hook_for_test(
                    copy_v2_crash_now,
                    &mut execute,
                )
            }
            CopyV2CrashCheckpoint::TargetFsync => {
                capability::with_after_copy_target_fsync_hook_for_test(
                    copy_v2_crash_now,
                    &mut execute,
                )
            }
            CopyV2CrashCheckpoint::TargetDurable => {
                capability::with_after_copy_target_durable_hook_for_test(
                    copy_v2_crash_now,
                    &mut execute,
                )
            }
        }));
        assert!(crashed.is_err(), "{label}: hookul nu a simulat crash-ul");
        let expected_phase = match checkpoint {
            CopyV2CrashCheckpoint::AnonymousStageCheckpoint
            | CopyV2CrashCheckpoint::TemporaryLinkBeforePhase
            | CopyV2CrashCheckpoint::TargetLinkBeforePhase
            | CopyV2CrashCheckpoint::RenameBeforePhase => WalPhase::AuxiliaryDurable,
            CopyV2CrashCheckpoint::TargetFsync => WalPhase::EffectVisible,
            CopyV2CrashCheckpoint::TargetDurable => WalPhase::TargetDurable,
        };
        drop(execute);
        assert_eq!(guard.phase(), expected_phase, "{label}");
        drop(guard);
        drop(coordinator);

        let restarted = fixture.restart_coordinator();
        let first_scan = restarted.snapshot().unwrap();
        if matches!(checkpoint, CopyV2CrashCheckpoint::AnonymousStageCheckpoint) {
            assert!(first_scan.blocked, "{label}: {first_scan:?}");
            let item = first_scan
                .items
                .iter()
                .find(|item| item.operation_id.as_deref() == Some(operation_id.as_str()))
                .expect("Copy v2 baseline item");
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
                    operation_id: operation_id.clone(),
                    expected_phase,
                    evidence_hash: item.evidence_hash.clone().expect("Copy v2 binding hash"),
                    action: WriteAuthorityRecoveryResolutionAction::AcceptRestoredState,
                })
                .unwrap();
            assert!(!receipt.recovery_scan.blocked, "{label}: {receipt:?}");
        } else {
            assert!(!first_scan.blocked, "{label}: {first_scan:?}");
        }
        assert_copy_v2_expected_target(&fixture, expected_target, &payload, label);
        assert!(
            !temp.exists(),
            "{label}: temp-ul trebuie consumat sau absent"
        );
        drop(restarted);

        let second_restart = fixture.restart_coordinator();
        let second_scan = second_restart.snapshot().unwrap();
        assert!(!second_scan.blocked, "{label}: {second_scan:?}");
        assert_copy_v2_expected_target(&fixture, expected_target, &payload, label);
        assert!(
            !temp.exists(),
            "{label}: al doilea restart a recreat temp-ul"
        );
        drop(second_restart);
        fixture.cleanup();
    }

    fn copy_v2_crash_now() {
        panic!("simulated Copy v2 crash checkpoint");
    }

    fn assert_copy_v2_expected_target(
        fixture: &AtomicRecoveryFixture,
        expected: CopyV2ExpectedTarget,
        payload: &[u8],
        label: &str,
    ) {
        match expected {
            CopyV2ExpectedTarget::Absent => assert!(!fixture.target.exists(), "{label}"),
            CopyV2ExpectedTarget::Baseline => {
                assert_eq!(fs::read(&fixture.target).unwrap(), b"baseline", "{label}")
            }
            CopyV2ExpectedTarget::Payload => {
                assert_eq!(fs::read(&fixture.target).unwrap(), payload, "{label}")
            }
        }
    }

    fn copy_v2_wal_record_name(fixture: &AtomicRecoveryFixture, operation_id: &str) -> String {
        let mut names = fs::read_dir(&fixture.wal)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(operation_id))
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(
            names.len(),
            1,
            "Copy v2 trebuie să păstreze un singur record WAL"
        );
        names.pop().unwrap()
    }

    fn unique_test_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "pana-wa-recovery-{label}-{}-{nanos}",
            std::process::id()
        ))
    }

    #[allow(dead_code)]
    fn assert_path(_path: &Path) {}
}
