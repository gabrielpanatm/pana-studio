use super::*;

pub(super) struct AtomicRecoveryFixture {
    pub(super) root: PathBuf,
    pub(super) boundary: PathBuf,
    pub(super) parent: PathBuf,
    pub(super) target: PathBuf,
    pub(super) wal: PathBuf,
}

impl AtomicRecoveryFixture {
    pub(super) fn new(label: &str, with_target: bool) -> Self {
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

    pub(super) fn prepare(
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

    pub(super) fn prepare_append(
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

    pub(super) fn prepare_external_config(
        &self,
        operation_id: &str,
        payload: &[u8],
        backup_path: &Path,
    ) -> (
        RecoveryCoordinator,
        crate::kernel::write_authority::operation::ExternalConfigOperationPlan,
        super::super::WalRecord,
    ) {
        self.prepare_external_config_with_previous(operation_id, payload, backup_path, b"baseline")
    }

    pub(super) fn prepare_external_config_create_new(
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
            capability::plan_external_config(&intent.target, payload, None, operation_id).unwrap();
        let record = build_external_config_wal_record(operation_id, 1, &intent, &plan).unwrap();
        (self.restart_coordinator(), plan, record)
    }

    pub(super) fn prepare_external_config_with_previous(
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

    pub(super) fn materialize_external_relocated_baseline(
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

    pub(super) fn materialize_external_committed_pair(
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
            capability::external_stage_identity_digest_for_test(&self.target, "target").unwrap(),
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

    pub(super) fn prepare_copy(
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

    pub(super) fn prepare_legacy_copy(
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

    pub(super) fn prepare_copy_for_owner(
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
                DirectoryAuthorityScope::ProjectCreation { authority_id: 1 },
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
        let plan =
            capability::plan_copy(&intent.target, source, replace_policy, operation_id).unwrap();
        let record = build_copy_wal_record(operation_id, 1, &intent, &plan).unwrap();
        (self.restart_coordinator(), intent, plan, record)
    }

    pub(super) fn prepare_rename(
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
        let source = WriteTarget::new(&self.target, &self.boundary, "test/recovery-rename-source")
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

    pub(super) fn prepare_remove_leaf(
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
        let target = WriteTarget::new(&self.target, &self.boundary, "test/recovery-remove-leaf")
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

    pub(super) fn prepare_remove_leaf_unchecked(
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

    pub(super) fn create_tree(&self) {
        fs::create_dir(&self.target).unwrap();
        fs::write(self.target.join("a.txt"), b"a").unwrap();
        fs::create_dir(self.target.join("nested")).unwrap();
        fs::write(self.target.join("nested/b.txt"), b"b").unwrap();
    }

    pub(super) fn prepare_remove_tree(
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
        let target = WriteTarget::new(&self.target, &self.boundary, "test/recovery-remove-tree")
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

    pub(super) fn prepare_directory(
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
        let target = WriteTarget::new(directory_path, &self.boundary, "test/recovery-directory")
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

    pub(super) fn prepare_directory_v2(
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
        let target = WriteTarget::new(directory_path, &self.boundary, "test/recovery-directory-v2")
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

    pub(super) fn prepare_symlink(
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

    pub(super) fn prepare_symlink_v2(
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

    pub(super) fn restart_coordinator(&self) -> RecoveryCoordinator {
        let wal_authority = capability::capture_directory_authority(
            &self.wal,
            "test/write-authority-wal",
            DirectoryAuthorityScope::ApplicationWriteAuthorityWal,
        )
        .unwrap();
        RecoveryCoordinator::bootstrap(wal_authority).unwrap()
    }

    pub(super) fn cleanup(&self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

pub(super) struct AppendV2Fixture {
    pub(super) root: PathBuf,
    pub(super) boundary: PathBuf,
    pub(super) target: PathBuf,
    pub(super) wal: PathBuf,
}

impl AppendV2Fixture {
    pub(super) fn new(label: &str, with_target: bool) -> Self {
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

    pub(super) fn prepare(
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

    pub(super) fn restart_coordinator(&self) -> RecoveryCoordinator {
        let authority = capability::capture_directory_authority(
            &self.wal,
            "test/append-v2-wal",
            DirectoryAuthorityScope::ApplicationWriteAuthorityWal,
        )
        .unwrap();
        RecoveryCoordinator::bootstrap(authority).unwrap()
    }

    pub(super) fn cleanup(&self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum AppendV2CrashCheckpoint {
    Checkpoint,
    EffectBeforePhase,
    TargetFsync,
    TargetDurable,
}

pub(super) fn run_append_v2_crash_case(with_target: bool, checkpoint: AppendV2CrashCheckpoint) {
    let label = format!("append-v2-{with_target}-{checkpoint:?}");
    let fixture = AppendV2Fixture::new(&label, with_target);
    let payload = b"{\"append_v2\":true}\n";
    let operation_id = format!("wal-{label}");
    let (coordinator, intent, plan, record) = fixture.prepare(&operation_id, payload);
    let mut guard = coordinator.begin(record).unwrap();
    let mut plan = Some(plan);
    let crash = || panic!("simulated Append v2 crash");
    let crashed = {
        let mut execute = || {
            capability::append_wal(
                &intent.target,
                payload,
                plan.take().expect("Append v2 plan consumed once"),
                &mut guard,
            )
        };
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match checkpoint {
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
        }))
    };
    assert!(crashed.is_err(), "{label}");
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
pub(super) enum CopyV2CrashCheckpoint {
    AnonymousStageCheckpoint,
    TemporaryLinkBeforePhase,
    TargetLinkBeforePhase,
    RenameBeforePhase,
    TargetFsync,
    TargetDurable,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum CopyV2ExpectedTarget {
    Absent,
    Baseline,
    Payload,
}

pub(super) fn run_copy_v2_crash_restart_case(
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
    let crashed = {
        let mut execute = || {
            capability::copy_file_wal(
                &intent.target,
                &source,
                replace_policy,
                plan.take().expect("Copy v2 plan is consumed once"),
                &mut guard,
            )
        };
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match checkpoint {
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
        }))
    };
    assert!(crashed.is_err(), "{label}: hookul nu a simulat crash-ul");
    let expected_phase = match checkpoint {
        CopyV2CrashCheckpoint::AnonymousStageCheckpoint
        | CopyV2CrashCheckpoint::TemporaryLinkBeforePhase
        | CopyV2CrashCheckpoint::TargetLinkBeforePhase
        | CopyV2CrashCheckpoint::RenameBeforePhase => WalPhase::AuxiliaryDurable,
        CopyV2CrashCheckpoint::TargetFsync => WalPhase::EffectVisible,
        CopyV2CrashCheckpoint::TargetDurable => WalPhase::TargetDurable,
    };
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

pub(super) fn copy_v2_crash_now() {
    panic!("simulated Copy v2 crash checkpoint");
}

pub(super) fn assert_copy_v2_expected_target(
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

pub(super) fn copy_v2_wal_record_name(
    fixture: &AtomicRecoveryFixture,
    operation_id: &str,
) -> String {
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

pub(super) fn unique_test_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "pana-wa-recovery-{label}-{}-{nanos}",
        std::process::id()
    ))
}
