#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CapabilityTestStage {
    AfterAuthorityPathVerified,
    AfterTargetParentCaptured,
    AfterBoundedReadLeafOpened,
    AfterAppendLeafOpened,
    AfterAppendV2Checkpoint,
    AfterAppendV2WriteBeforePhase,
    AfterAppendV2LinkBeforePhase,
    AfterAppendV2TargetFsync,
    AfterAppendV2TargetDurable,
    AfterAppendV2RecoveryHash,
    AfterDirectoryCreateBeforePhase,
    BeforeDirectoryV2CheckpointCapture,
    AfterDirectoryV2Checkpoint,
    BeforeDirectoryV2NoopFullPath,
    BeforeDirectoryCurrentStateFreshCapture,
    AfterRenameSourceParentCaptured,
    AfterExpectedLeafCaptured,
    AfterExternalBaselineRelocated,
    AfterExternalBackupCommitted,
    AfterExternalPublication,
    AfterAtomicExchange,
    AfterCopyAnonymousStageCheckpoint,
    AfterCopyTemporaryLinkBeforePhase,
    AfterCopyTargetLinkBeforePhase,
    AfterCopyRenameBeforePhase,
    AfterCopyRecoveryHash,
    AfterCopyTargetFsync,
    AfterCopyTargetDurable,
    BeforeExternalTargetDurable,
    BeforeDirectoryTargetDurable,
    BeforeCopyPreviewOverwriteRename,
    BeforeCopyStream,
    BeforeCopyTargetDurable,
    BeforeRemoveLeafQuarantine,
    BeforeRemoveLeafTargetDurable,
    BeforeRemoveLeafUnlink,
    BeforeRemoveTreeQuarantine,
    BeforeRemoveTreeTargetDurable,
    BeforeRemoveTreeTraversal,
    BeforeSymlinkTargetDurable,
    AfterSymlinkCreateBeforePhase,
    AfterSymlinkV2FirstOpenBeforeCapture,
    BeforeSymlinkV2CheckpointCapture,
    AfterSymlinkV2Checkpoint,
    BeforeSymlinkV2NoopFullPath,
    BeforeSymlinkCurrentStateFreshCapture,
    BeforeAtomicCommit,
    BeforeRename,
}

#[cfg(test)]
type CapabilityTestHook = Box<dyn Fn(CapabilityTestStage)>;

#[cfg(test)]
thread_local! {
    static TEST_HOOK: std::cell::RefCell<Option<CapabilityTestHook>> =
        std::cell::RefCell::new(None);
    pub(super) static TEST_FAIL_DIRECTORY_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static TEST_FORCE_EXTERNAL_LINKAT_PROC_FALLBACK: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static TEST_FAIL_EXTERNAL_LINKAT: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static TEST_APPEND_V2_SHORT_WRITE: std::cell::Cell<Option<usize>> =
        const { std::cell::Cell::new(None) };
}

#[cfg(test)]
pub(super) fn run_test_hook(stage: CapabilityTestStage) {
    TEST_HOOK.with(|hook| {
        if let Some(hook) = hook.borrow().as_ref() {
            hook(stage);
        }
    });
}

#[cfg(not(test))]
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub(super) enum CapabilityTestStage {
    AfterAuthorityPathVerified,
    AfterTargetParentCaptured,
    AfterBoundedReadLeafOpened,
    AfterAppendLeafOpened,
    AfterAppendV2Checkpoint,
    AfterAppendV2WriteBeforePhase,
    AfterAppendV2LinkBeforePhase,
    AfterAppendV2TargetFsync,
    AfterAppendV2TargetDurable,
    AfterAppendV2RecoveryHash,
    AfterDirectoryCreateBeforePhase,
    BeforeDirectoryV2CheckpointCapture,
    AfterDirectoryV2Checkpoint,
    BeforeDirectoryV2NoopFullPath,
    BeforeDirectoryCurrentStateFreshCapture,
    AfterRenameSourceParentCaptured,
    AfterExpectedLeafCaptured,
    AfterExternalBaselineRelocated,
    AfterExternalBackupCommitted,
    AfterExternalPublication,
    AfterAtomicExchange,
    AfterCopyAnonymousStageCheckpoint,
    AfterCopyTemporaryLinkBeforePhase,
    AfterCopyTargetLinkBeforePhase,
    AfterCopyRenameBeforePhase,
    AfterCopyRecoveryHash,
    AfterCopyTargetFsync,
    AfterCopyTargetDurable,
    BeforeExternalTargetDurable,
    BeforeDirectoryTargetDurable,
    BeforeCopyPreviewOverwriteRename,
    BeforeCopyStream,
    BeforeCopyTargetDurable,
    BeforeRemoveLeafQuarantine,
    BeforeRemoveLeafTargetDurable,
    BeforeRemoveLeafUnlink,
    BeforeRemoveTreeQuarantine,
    BeforeRemoveTreeTargetDurable,
    BeforeRemoveTreeTraversal,
    BeforeSymlinkTargetDurable,
    AfterSymlinkCreateBeforePhase,
    AfterSymlinkV2FirstOpenBeforeCapture,
    BeforeSymlinkV2CheckpointCapture,
    AfterSymlinkV2Checkpoint,
    BeforeSymlinkV2NoopFullPath,
    BeforeSymlinkCurrentStateFreshCapture,
    BeforeAtomicCommit,
    BeforeRename,
}

#[cfg(not(test))]
pub(super) fn run_test_hook(_stage: CapabilityTestStage) {}

#[cfg(test)]
pub(super) fn force_external_linkat_proc_fallback() -> bool {
    TEST_FORCE_EXTERNAL_LINKAT_PROC_FALLBACK.with(std::cell::Cell::get)
}

#[cfg(not(test))]
pub(super) fn force_external_linkat_proc_fallback() -> bool {
    false
}

#[cfg(test)]
pub(super) fn fail_external_linkat() -> bool {
    TEST_FAIL_EXTERNAL_LINKAT.with(std::cell::Cell::get)
}

#[cfg(not(test))]
pub(super) fn fail_external_linkat() -> bool {
    false
}

#[cfg(test)]
pub(super) fn with_test_hook<T>(
    hook: impl Fn(CapabilityTestStage) + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    struct ResetHook;
    impl Drop for ResetHook {
        fn drop(&mut self) {
            TEST_HOOK.with(|slot| {
                *slot.borrow_mut() = None;
            });
        }
    }

    TEST_HOOK.with(|slot| {
        let previous = slot.borrow_mut().replace(Box::new(hook));
        assert!(previous.is_none(), "capability test hook already installed");
    });
    let _reset = ResetHook;
    operation()
}

#[cfg(test)]
fn with_append_v2_stage_hook_for_test<T>(
    expected: CapabilityTestStage,
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == expected {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_after_bounded_read_leaf_opened_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::AfterBoundedReadLeafOpened {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_append_v2_short_write_for_test<T>(
    bytes: usize,
    operation: impl FnOnce() -> T,
) -> T {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            TEST_APPEND_V2_SHORT_WRITE.with(|slot| slot.set(None));
        }
    }
    TEST_APPEND_V2_SHORT_WRITE.with(|slot| {
        assert!(slot.replace(Some(bytes)).is_none());
    });
    let _reset = Reset;
    operation()
}

#[cfg(test)]
pub(super) fn append_v2_short_write_limit() -> Option<usize> {
    TEST_APPEND_V2_SHORT_WRITE.with(std::cell::Cell::get)
}

#[cfg(not(test))]
pub(super) fn append_v2_short_write_limit() -> Option<usize> {
    None
}

#[cfg(test)]
macro_rules! append_v2_stage_hook {
    ($name:ident, $stage:ident) => {
        pub(in crate::kernel::write_authority::capability) fn $name<T>(
            hook: impl Fn() + 'static,
            operation: impl FnOnce() -> T,
        ) -> T {
            with_append_v2_stage_hook_for_test(CapabilityTestStage::$stage, hook, operation)
        }
    };
}

#[cfg(test)]
append_v2_stage_hook!(
    with_after_append_v2_checkpoint_hook_for_test,
    AfterAppendV2Checkpoint
);
#[cfg(test)]
append_v2_stage_hook!(
    with_after_append_v2_write_before_phase_hook_for_test,
    AfterAppendV2WriteBeforePhase
);
#[cfg(test)]
append_v2_stage_hook!(
    with_after_append_v2_link_before_phase_hook_for_test,
    AfterAppendV2LinkBeforePhase
);
#[cfg(test)]
append_v2_stage_hook!(
    with_after_append_v2_target_fsync_hook_for_test,
    AfterAppendV2TargetFsync
);
#[cfg(test)]
append_v2_stage_hook!(
    with_after_append_v2_target_durable_hook_for_test,
    AfterAppendV2TargetDurable
);
#[cfg(test)]
append_v2_stage_hook!(
    with_after_append_v2_recovery_hash_hook_for_test,
    AfterAppendV2RecoveryHash
);

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_external_backup_committed_test_hook<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::AfterExternalBackupCommitted {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_external_baseline_relocated_test_hook<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::AfterExternalBaselineRelocated {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_external_post_publication_test_hook<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::AfterExternalPublication {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_before_external_target_durable_test_hook<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::BeforeExternalTargetDurable {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_external_linkat_proc_fallback_test_hook<
    T,
>(
    operation: impl FnOnce() -> T,
) -> T {
    struct ResetFallback;
    impl Drop for ResetFallback {
        fn drop(&mut self) {
            TEST_FORCE_EXTERNAL_LINKAT_PROC_FALLBACK.with(|flag| flag.set(false));
        }
    }

    TEST_FORCE_EXTERNAL_LINKAT_PROC_FALLBACK.with(|flag| {
        assert!(
            !flag.replace(true),
            "external linkat proc fallback test hook already installed"
        );
    });
    let _reset = ResetFallback;
    operation()
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_external_linkat_failure_test_hook<T>(
    operation: impl FnOnce() -> T,
) -> T {
    struct ResetFailure;
    impl Drop for ResetFailure {
        fn drop(&mut self) {
            TEST_FAIL_EXTERNAL_LINKAT.with(|flag| flag.set(false));
        }
    }

    TEST_FAIL_EXTERNAL_LINKAT.with(|flag| {
        assert!(
            !flag.replace(true),
            "external linkat failure test hook already installed"
        );
    });
    let _reset = ResetFailure;
    operation()
}

#[cfg(test)]
pub(super) fn with_directory_sync_failure<T>(operation: impl FnOnce() -> T) -> T {
    struct ResetFailure;
    impl Drop for ResetFailure {
        fn drop(&mut self) {
            TEST_FAIL_DIRECTORY_SYNC.with(|flag| flag.set(false));
        }
    }
    TEST_FAIL_DIRECTORY_SYNC.with(|flag| {
        assert!(
            !flag.replace(true),
            "directory sync failure already installed"
        );
    });
    let _reset = ResetFailure;
    operation()
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_directory_sync_failure_for_test<T>(
    operation: impl FnOnce() -> T,
) -> T {
    with_directory_sync_failure(operation)
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_before_directory_target_durable_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::BeforeDirectoryTargetDurable {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_after_directory_create_before_phase_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::AfterDirectoryCreateBeforePhase {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_before_directory_v2_checkpoint_capture_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::BeforeDirectoryV2CheckpointCapture {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_after_directory_v2_checkpoint_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::AfterDirectoryV2Checkpoint {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_before_directory_v2_noop_full_path_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::BeforeDirectoryV2NoopFullPath {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_before_directory_current_state_fresh_capture_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::BeforeDirectoryCurrentStateFreshCapture {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_before_symlink_target_durable_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::BeforeSymlinkTargetDurable {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_after_symlink_create_before_phase_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::AfterSymlinkCreateBeforePhase {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_after_symlink_v2_first_open_before_capture_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::AfterSymlinkV2FirstOpenBeforeCapture {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_before_symlink_v2_checkpoint_capture_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::BeforeSymlinkV2CheckpointCapture {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_after_symlink_v2_checkpoint_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::AfterSymlinkV2Checkpoint {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_before_symlink_v2_noop_full_path_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::BeforeSymlinkV2NoopFullPath {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_before_symlink_current_state_fresh_capture_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::BeforeSymlinkCurrentStateFreshCapture {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_before_copy_stream_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::BeforeCopyStream {
                hook();
            }
        },
        operation,
    )
}

// Checkpointurile de mai jos sunt infrastructură pentru matricea Copy v2.
// Ele nu sunt apelate de protocolul v1 și nu modifică producția; Copy v2
// le va publica exact lângă syscall-ul/faza pe care o denumește fiecare.
#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_after_copy_anonymous_stage_checkpoint_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::AfterCopyAnonymousStageCheckpoint {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_after_copy_temporary_link_before_phase_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::AfterCopyTemporaryLinkBeforePhase {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_after_copy_target_link_before_phase_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::AfterCopyTargetLinkBeforePhase {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_after_copy_rename_before_phase_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::AfterCopyRenameBeforePhase {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_after_copy_target_fsync_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::AfterCopyTargetFsync {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_before_copy_preview_overwrite_rename_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::BeforeCopyPreviewOverwriteRename {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_after_copy_recovery_hash_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::AfterCopyRecoveryHash {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_before_rename_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::BeforeRename {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_before_remove_leaf_quarantine_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::BeforeRemoveLeafQuarantine {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_before_remove_leaf_unlink_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::BeforeRemoveLeafUnlink {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_before_remove_leaf_target_durable_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::BeforeRemoveLeafTargetDurable {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_before_remove_tree_quarantine_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::BeforeRemoveTreeQuarantine {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_before_remove_tree_traversal_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::BeforeRemoveTreeTraversal {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_before_remove_tree_target_durable_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::BeforeRemoveTreeTargetDurable {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_before_copy_target_durable_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::BeforeCopyTargetDurable {
                hook();
            }
        },
        operation,
    )
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_after_copy_target_durable_hook_for_test<
    T,
>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::AfterCopyTargetDurable {
                hook();
            }
        },
        operation,
    )
}
