use std::{
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Barrier},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use super::*;

#[test]
fn sealed_authority_rejects_root_replacement_before_effect() {
    let root = unique_test_dir("sealed-authority-preflight-swap");
    let authority_path = root.join("project");
    let held_path = root.join("project-held");
    let target_path = authority_path.join("document.txt");
    fs::create_dir_all(target_path.parent().unwrap()).unwrap();
    fs::write(&target_path, "original-before").unwrap();

    let authority = capture_directory_authority(
        &authority_path,
        "test/sealed-authority-preflight",
        DirectoryAuthorityScope::ProjectRoot,
    )
    .unwrap();
    let target = WriteTarget::new(
        target_path.clone(),
        authority_path.clone(),
        "test/sealed-authority-preflight/document.txt",
    )
    .bind_authority(authority)
    .unwrap();

    fs::rename(&authority_path, &held_path).unwrap();
    fs::create_dir_all(target_path.parent().unwrap()).unwrap();
    fs::write(&target_path, "replacement-sentinel").unwrap();

    let error = crate::kernel::write_authority::capability::atomic_write(
        &target,
        b"must-not-write",
        CapabilityReplacePolicy::Replace,
    )
    .unwrap_err();

    assert!(error.contains("înlocuit"));
    assert_eq!(
        fs::read_to_string(held_path.join("document.txt")).unwrap(),
        "original-before"
    );
    assert_eq!(
        fs::read_to_string(&target_path).unwrap(),
        "replacement-sentinel"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn sealed_authority_stays_on_held_inode_during_root_replacement() {
    let root = unique_test_dir("sealed-authority-live-swap");
    let authority_path = root.join("project");
    let held_path = root.join("project-held");
    let replacement_path = root.join("project-replacement");
    let target_path = authority_path.join("document.txt");
    fs::create_dir_all(target_path.parent().unwrap()).unwrap();
    fs::write(&target_path, "original-before").unwrap();
    fs::create_dir_all(&replacement_path).unwrap();
    fs::write(
        replacement_path.join("document.txt"),
        "replacement-sentinel",
    )
    .unwrap();

    let authority = capture_directory_authority(
        &authority_path,
        "test/sealed-authority-live",
        DirectoryAuthorityScope::ProjectRoot,
    )
    .unwrap();
    let target = WriteTarget::new(
        target_path.clone(),
        authority_path.clone(),
        "test/sealed-authority-live/document.txt",
    )
    .bind_authority(authority)
    .unwrap();

    let hook_authority_path = authority_path.clone();
    let hook_held_path = held_path.clone();
    let hook_replacement_path = replacement_path.clone();
    let effect = with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::AfterAuthorityPathVerified {
                fs::rename(&hook_authority_path, &hook_held_path).unwrap();
                fs::rename(&hook_replacement_path, &hook_authority_path).unwrap();
            }
        },
        || {
            crate::kernel::write_authority::capability::atomic_write(
                &target,
                b"original-after",
                CapabilityReplacePolicy::Replace,
            )
        },
    )
    .unwrap();

    assert!(effect.changed);
    assert!(effect.recovery_required);
    assert!(effect
        .diagnostic
        .as_deref()
        .is_some_and(|diagnostic| diagnostic.contains("Replacement-ul nu a fost folosit")));
    assert_eq!(
        fs::read_to_string(held_path.join("document.txt")).unwrap(),
        "original-after"
    );
    assert_eq!(
        fs::read_to_string(&target_path).unwrap(),
        "replacement-sentinel"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn sealed_authority_noop_still_detects_live_root_replacement() {
    let root = unique_test_dir("sealed-authority-noop-swap");
    let authority_path = root.join("project");
    let held_path = root.join("project-held");
    let replacement_path = root.join("project-replacement");
    let target_path = authority_path.join("missing.txt");
    fs::create_dir_all(&authority_path).unwrap();
    fs::create_dir_all(&replacement_path).unwrap();
    fs::write(replacement_path.join("missing.txt"), "replacement-sentinel").unwrap();

    let authority = capture_directory_authority(
        &authority_path,
        "test/sealed-authority-noop",
        DirectoryAuthorityScope::ProjectRoot,
    )
    .unwrap();
    let target = WriteTarget::new(
        target_path.clone(),
        authority_path.clone(),
        "test/sealed-authority-noop/missing.txt",
    )
    .bind_authority(authority)
    .unwrap();

    let hook_authority_path = authority_path.clone();
    let hook_held_path = held_path.clone();
    let hook_replacement_path = replacement_path.clone();
    let effect = with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::AfterAuthorityPathVerified {
                fs::rename(&hook_authority_path, &hook_held_path).unwrap();
                fs::rename(&hook_replacement_path, &hook_authority_path).unwrap();
            }
        },
        || crate::kernel::write_authority::capability::remove_file_if_exists_maintenance(&target),
    )
    .unwrap();

    assert!(!effect.changed);
    assert!(effect.recovery_required);
    assert!(held_path.to_path_buf().is_dir());
    assert_eq!(
        fs::read_to_string(&target_path).unwrap(),
        "replacement-sentinel"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn subprocess_directory_lease_survives_path_replacement() {
    let root = unique_test_dir("subprocess-directory-lease");
    let original = root.join("project");
    let moved = root.join("project-held");
    fs::create_dir_all(&original).unwrap();

    let lease = capture_directory_lease(&original, "test/subprocess-directory").unwrap();
    lease.require_empty().unwrap();
    fs::rename(&original, &moved).unwrap();
    fs::create_dir_all(&original).unwrap();

    let status = Command::new("/bin/sh")
        .arg("-c")
        .arg("printf original > child-marker.txt")
        .current_dir(lease.current_dir_path())
        .status()
        .unwrap();

    assert!(status.success());
    assert_eq!(
        fs::read_to_string(moved.join("child-marker.txt")).unwrap(),
        "original"
    );
    assert!(!original.join("child-marker.txt").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn parent_creation_sync_failure_is_recovery_not_rejection() {
    let root = unique_test_dir("parent-creation-recovery");
    fs::create_dir_all(&root).unwrap();
    let boundary = root.join("session/new-project");
    let target_path = boundary.join("recovery.json");
    let target = WriteTarget::new(&target_path, &boundary, "test/parent-creation-recovery");

    let effect = with_directory_sync_failure(|| {
        atomic_write(&target, b"{}", CapabilityReplacePolicy::CreateNew)
    })
    .expect("a visible parent must return a recovery effect");

    assert!(effect.changed);
    assert!(effect.recovery_required);
    assert!(effect
        .diagnostic
        .as_deref()
        .is_some_and(|diagnostic| diagnostic.contains("namespace")));
    assert!(root.join("session").is_dir());
    assert!(!target_path.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn append_leaf_created_before_validation_failure_is_recovery() {
    let root = unique_test_dir("append-create-recovery");
    fs::create_dir_all(&root).unwrap();
    let target_path = root.join("transactions.jsonl");
    let alias_path = root.join("transactions-alias.jsonl");
    let target = WriteTarget::new(&target_path, &root, "test/append-create-recovery");
    let hook_target = target_path.clone();
    let hook_alias = alias_path.clone();

    let effect = with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::AfterAppendLeafOpened {
                fs::hard_link(&hook_target, &hook_alias).unwrap();
            }
        },
        || append(&target, b"record\n"),
    )
    .expect("a newly visible append leaf must return a recovery effect");

    assert!(effect.changed);
    assert!(effect.recovery_required);
    assert_eq!(fs::metadata(&target_path).unwrap().len(), 0);
    assert_eq!(fs::metadata(&alias_path).unwrap().len(), 0);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn atomic_write_remains_anchored_after_ancestor_swap() {
    let root = unique_test_dir("capability-ancestor-swap");
    let safe = root.join("safe");
    let held = root.join("safe-held");
    let outside = root.join("outside");
    let boundary = safe.join("boundary");
    let target_path = boundary.join("nested/document.txt");
    let outside_target = outside.join("boundary/nested/document.txt");
    fs::create_dir_all(target_path.parent().unwrap()).unwrap();
    fs::create_dir_all(outside_target.parent().unwrap()).unwrap();
    fs::write(&target_path, "inside-before").unwrap();
    fs::write(&outside_target, "outside-sentinel").unwrap();

    let entered = Arc::new(Barrier::new(2));
    let swapped = Arc::new(Barrier::new(2));
    let attacker_entered = Arc::clone(&entered);
    let attacker_swapped = Arc::clone(&swapped);
    let attacker_root = root.clone();
    let attacker = thread::spawn(move || {
        attacker_entered.wait();
        fs::rename(attacker_root.join("safe"), attacker_root.join("safe-held")).unwrap();
        symlink(attacker_root.join("outside"), attacker_root.join("safe")).unwrap();
        attacker_swapped.wait();
    });

    let operation_entered = Arc::clone(&entered);
    let operation_swapped = Arc::clone(&swapped);
    let target = WriteTarget::new(&target_path, &boundary, "test/ancestor-swap");
    let effect = with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::AfterTargetParentCaptured {
                operation_entered.wait();
                operation_swapped.wait();
            }
        },
        || atomic_write(&target, b"inside-after", CapabilityReplacePolicy::Replace),
    )
    .unwrap();
    attacker.join().unwrap();

    assert!(effect.changed);
    assert_eq!(
        fs::read_to_string(&outside_target).unwrap(),
        "outside-sentinel"
    );
    assert_eq!(
        fs::read_to_string(held.join("boundary/nested/document.txt")).unwrap(),
        "inside-after"
    );
    assert_no_temp_files(&held.join("boundary/nested"));

    fs::remove_file(root.join("safe")).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn atomic_create_new_cleans_temp_when_collision_arrives_before_commit() {
    let root = unique_test_dir("capability-atomic-collision");
    let boundary = root.join("boundary");
    let target_path = boundary.join("document.txt");
    fs::create_dir_all(&boundary).unwrap();

    let entered = Arc::new(Barrier::new(2));
    let collided = Arc::new(Barrier::new(2));
    let attacker_entered = Arc::clone(&entered);
    let attacker_collided = Arc::clone(&collided);
    let attacker_target = target_path.clone();
    let attacker = thread::spawn(move || {
        attacker_entered.wait();
        fs::write(attacker_target, "competitor").unwrap();
        attacker_collided.wait();
    });

    let operation_entered = Arc::clone(&entered);
    let operation_collided = Arc::clone(&collided);
    let target = WriteTarget::new(&target_path, &boundary, "test/atomic-collision");
    let error = with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::BeforeAtomicCommit {
                operation_entered.wait();
                operation_collided.wait();
            }
        },
        || atomic_write(&target, b"ours", CapabilityReplacePolicy::CreateNew),
    )
    .unwrap_err();
    attacker.join().unwrap();

    assert!(error.contains("Commit-ul atomic"));
    assert_eq!(fs::read_to_string(&target_path).unwrap(), "competitor");
    assert_no_temp_files(&boundary);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn atomic_post_commit_sync_failure_returns_terminal_recovery_effect() {
    let root = unique_test_dir("capability-atomic-recovery");
    let boundary = root.join("boundary");
    let target_path = boundary.join("document.txt");
    fs::create_dir_all(&boundary).unwrap();
    fs::write(&target_path, "before").unwrap();
    let target = WriteTarget::new(&target_path, &boundary, "test/atomic-recovery");

    let effect = with_directory_sync_failure(|| {
        atomic_write(&target, b"after", CapabilityReplacePolicy::Replace)
    })
    .unwrap();

    assert!(effect.changed);
    assert!(effect.recovery_required);
    assert!(effect
        .diagnostic
        .as_deref()
        .is_some_and(|value| value.to_lowercase().contains("nu repeta")));
    assert_eq!(fs::read_to_string(&target_path).unwrap(), "after");
    assert_no_temp_files(&boundary);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn atomic_replace_preserves_old_leaf_when_target_is_substituted_after_exchange() {
    let root = unique_test_dir("capability-atomic-replace-post-exchange");
    let boundary = root.join("boundary");
    let target_path = boundary.join("document.txt");
    let displaced_new = boundary.join("our-displaced-new.txt");
    fs::create_dir_all(&boundary).unwrap();
    fs::write(&target_path, "previous").unwrap();
    let target = WriteTarget::new(&target_path, &boundary, "test/atomic-replace-race");
    let racing_target = target_path.clone();
    let racing_displaced = displaced_new.clone();

    let effect = with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::AfterAtomicExchange {
                fs::rename(&racing_target, &racing_displaced).unwrap();
                fs::write(&racing_target, "competitor").unwrap();
            }
        },
        || atomic_write(&target, b"ours", CapabilityReplacePolicy::Replace),
    )
    .unwrap();

    assert!(effect.recovery_required);
    assert_eq!(fs::read_to_string(&target_path).unwrap(), "competitor");
    assert_eq!(fs::read_to_string(&displaced_new).unwrap(), "ours");
    let preserved_old = fs::read_dir(&boundary)
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
        .expect("previous leaf should remain for recovery")
        .path();
    assert_eq!(fs::read_to_string(preserved_old).unwrap(), "previous");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn atomic_commit_rejects_substituted_temp_inode_without_touching_target() {
    let root = unique_test_dir("capability-temp-substitution");
    let boundary = root.join("boundary");
    let target_path = boundary.join("document.txt");
    let outside = root.join("outside.txt");
    fs::create_dir_all(&boundary).unwrap();
    fs::write(&target_path, "original").unwrap();
    fs::write(&outside, "outside-sentinel").unwrap();

    let entered = Arc::new(Barrier::new(2));
    let substituted = Arc::new(Barrier::new(2));
    let attacker_entered = Arc::clone(&entered);
    let attacker_substituted = Arc::clone(&substituted);
    let attacker_boundary = boundary.clone();
    let attacker_outside = outside.clone();
    let attacker = thread::spawn(move || {
        attacker_entered.wait();
        let temp = fs::read_dir(&attacker_boundary)
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".pana-capability-")
            })
            .expect("atomic temp should be visible")
            .path();
        fs::remove_file(&temp).unwrap();
        fs::hard_link(attacker_outside, temp).unwrap();
        attacker_substituted.wait();
    });

    let operation_entered = Arc::clone(&entered);
    let operation_substituted = Arc::clone(&substituted);
    let target = WriteTarget::new(&target_path, &boundary, "test/temp-substitution");
    let error = with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::BeforeAtomicCommit {
                operation_entered.wait();
                operation_substituted.wait();
            }
        },
        || atomic_write(&target, b"ours", CapabilityReplacePolicy::Replace),
    )
    .unwrap_err();
    attacker.join().unwrap();

    assert!(error.contains("inode-ul temporar"));
    assert_eq!(fs::read_to_string(&target_path).unwrap(), "original");
    assert_eq!(fs::read_to_string(&outside).unwrap(), "outside-sentinel");
    assert_no_temp_files(&boundary);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn conditional_atomic_replace_restores_racing_leaf_without_overwrite() {
    let root = unique_test_dir("capability-leaf-cas-replace");
    let boundary = root.join("boundary");
    let target_path = boundary.join("document.txt");
    let displaced = boundary.join("captured-original.txt");
    fs::create_dir_all(&boundary).unwrap();
    fs::write(&target_path, "expected-original").unwrap();
    let version =
        crate::project::project_disk_metadata_version_token(&fs::metadata(&target_path).unwrap());
    let target = WriteTarget::new(&target_path, &boundary, "test/leaf-cas-replace")
        .with_expected_present(version, Some(hash_bytes(b"expected-original")));
    let racing_target = target_path.clone();
    let racing_displaced = displaced.clone();

    let error = with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::AfterExpectedLeafCaptured {
                fs::rename(&racing_target, &racing_displaced).unwrap();
                fs::write(&racing_target, "competitor").unwrap();
            }
        },
        || atomic_write(&target, b"ours", CapabilityReplacePolicy::Replace),
    )
    .unwrap_err();

    assert!(error.contains("restaurată"));
    assert_eq!(fs::read_to_string(&target_path).unwrap(), "competitor");
    assert_eq!(fs::read_to_string(&displaced).unwrap(), "expected-original");
    assert_no_temp_files(&boundary);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn conditional_atomic_replace_preserves_old_leaf_when_target_is_substituted_after_exchange() {
    let root = unique_test_dir("capability-leaf-cas-post-exchange");
    let boundary = root.join("boundary");
    let target_path = boundary.join("document.txt");
    let displaced_new = boundary.join("our-displaced-new.txt");
    fs::create_dir_all(&boundary).unwrap();
    fs::write(&target_path, "expected-original").unwrap();
    let version =
        crate::project::project_disk_metadata_version_token(&fs::metadata(&target_path).unwrap());
    let target = WriteTarget::new(&target_path, &boundary, "test/leaf-cas-post-exchange")
        .with_expected_present(version, Some(hash_bytes(b"expected-original")));
    let racing_target = target_path.clone();
    let racing_displaced = displaced_new.clone();

    let effect = with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::AfterAtomicExchange {
                fs::rename(&racing_target, &racing_displaced).unwrap();
                fs::write(&racing_target, "competitor").unwrap();
            }
        },
        || atomic_write(&target, b"ours", CapabilityReplacePolicy::Replace),
    )
    .unwrap();

    assert!(effect.recovery_required);
    assert_eq!(fs::read_to_string(&target_path).unwrap(), "competitor");
    assert_eq!(fs::read_to_string(&displaced_new).unwrap(), "ours");
    let preserved_old = fs::read_dir(&boundary)
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
        .expect("old leaf should remain in temp for recovery")
        .path();
    assert_eq!(
        fs::read_to_string(preserved_old).unwrap(),
        "expected-original"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn conditional_remove_restores_racing_leaf_without_deletion() {
    let root = unique_test_dir("capability-leaf-cas-remove");
    let boundary = root.join("boundary");
    let target_path = boundary.join("document.txt");
    let displaced = boundary.join("captured-original.txt");
    fs::create_dir_all(&boundary).unwrap();
    fs::write(&target_path, "expected-original").unwrap();
    let version =
        crate::project::project_disk_metadata_version_token(&fs::metadata(&target_path).unwrap());
    let target = WriteTarget::new(&target_path, &boundary, "test/leaf-cas-remove")
        .with_expected_present(version, Some(hash_bytes(b"expected-original")));
    let racing_target = target_path.clone();
    let racing_displaced = displaced.clone();

    let error = with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::AfterExpectedLeafCaptured {
                fs::rename(&racing_target, &racing_displaced).unwrap();
                fs::write(&racing_target, "competitor").unwrap();
            }
        },
        || remove_file_if_exists(&target),
    )
    .unwrap_err();

    assert!(error.contains("restaurată"));
    assert_eq!(fs::read_to_string(&target_path).unwrap(), "competitor");
    assert_eq!(fs::read_to_string(&displaced).unwrap(), "expected-original");
    assert_no_temp_files(&boundary);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rename_noreplace_preserves_racing_destination_and_source() {
    let root = unique_test_dir("capability-rename-collision");
    let source_boundary = root.join("source");
    let destination_boundary = root.join("destination");
    let source_path = source_boundary.join("entry.txt");
    let destination_path = destination_boundary.join("entry.txt");
    fs::create_dir_all(&source_boundary).unwrap();
    fs::create_dir_all(&destination_boundary).unwrap();
    fs::write(&source_path, "source").unwrap();

    let entered = Arc::new(Barrier::new(2));
    let collided = Arc::new(Barrier::new(2));
    let attacker_entered = Arc::clone(&entered);
    let attacker_collided = Arc::clone(&collided);
    let attacker_destination = destination_path.clone();
    let attacker = thread::spawn(move || {
        attacker_entered.wait();
        fs::write(attacker_destination, "competitor").unwrap();
        attacker_collided.wait();
    });

    let operation_entered = Arc::clone(&entered);
    let operation_collided = Arc::clone(&collided);
    let source = WriteTarget::new(&source_path, &source_boundary, "test/rename-source");
    let destination = WriteTarget::new(
        &destination_path,
        &destination_boundary,
        "test/rename-destination",
    );
    let error = with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::BeforeRename {
                operation_entered.wait();
                operation_collided.wait();
            }
        },
        || rename_noreplace(&source, &destination),
    )
    .unwrap_err();
    attacker.join().unwrap();

    assert!(error.contains("fără suprascriere"));
    assert_eq!(fs::read_to_string(&source_path).unwrap(), "source");
    assert_eq!(fs::read_to_string(&destination_path).unwrap(), "competitor");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn conditional_rename_restores_racing_source_and_keeps_destination_absent() {
    let root = unique_test_dir("capability-leaf-cas-rename");
    let boundary = root.join("boundary");
    let source_path = boundary.join("source.txt");
    let destination_path = boundary.join("destination.txt");
    let displaced = boundary.join("captured-original.txt");
    fs::create_dir_all(&boundary).unwrap();
    fs::write(&source_path, "expected-original").unwrap();
    let version =
        crate::project::project_disk_metadata_version_token(&fs::metadata(&source_path).unwrap());
    let source = WriteTarget::new(&source_path, &boundary, "test/leaf-cas-source")
        .with_expected_present(version, Some(hash_bytes(b"expected-original")));
    let destination = WriteTarget::new(&destination_path, &boundary, "test/leaf-cas-destination")
        .with_expected_absent();
    let racing_source = source_path.clone();
    let racing_displaced = displaced.clone();

    let error = with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::BeforeRename {
                fs::rename(&racing_source, &racing_displaced).unwrap();
                fs::write(&racing_source, "competitor").unwrap();
            }
        },
        || rename_noreplace(&source, &destination),
    )
    .unwrap_err();

    assert!(error.contains("restaurată"));
    assert_eq!(fs::read_to_string(&source_path).unwrap(), "competitor");
    assert_eq!(fs::read_to_string(&displaced).unwrap(), "expected-original");
    assert!(!destination_path.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn conditional_directory_rename_rolls_back_child_edit_after_tree_preflight() {
    let root = unique_test_dir("capability-tree-cas-rename");
    let boundary = root.join("boundary");
    let source_path = boundary.join("source");
    let destination_path = boundary.join("destination");
    let child_path = source_path.join("child.txt");
    fs::create_dir_all(&source_path).unwrap();
    fs::write(&child_path, "accepted").unwrap();
    let source_version =
        crate::project::project_disk_metadata_version_token(&fs::metadata(&source_path).unwrap());
    let tree_fingerprint = tree_fingerprint_from_records(vec![TreeFingerprintRecord {
        relative_path: "child.txt".to_string(),
        kind: b'f',
        version_token: crate::project::project_disk_metadata_version_token(
            &fs::metadata(&child_path).unwrap(),
        ),
    }]);
    let source = WriteTarget::new(&source_path, &boundary, "test/tree-cas-source")
        .with_expected_present_tree(source_version, tree_fingerprint);
    let destination = WriteTarget::new(&destination_path, &boundary, "test/tree-cas-destination")
        .with_expected_absent();
    let racing_child = child_path.clone();

    let error = with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::BeforeRename {
                fs::write(&racing_child, "external-after-preflight").unwrap();
            }
        },
        || rename_noreplace(&source, &destination),
    )
    .unwrap_err();

    assert!(error.contains("descendenții sursei s-au schimbat"));
    assert!(source_path.is_dir());
    assert!(!destination_path.exists());
    assert_eq!(
        fs::read_to_string(child_path).unwrap(),
        "external-after-preflight"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn same_boundary_rename_uses_one_capture_across_ancestor_swap() {
    let root = unique_test_dir("capability-rename-one-boundary");
    let boundary = root.join("project");
    let held = root.join("project-held");
    let outside = root.join("outside");
    let source_path = boundary.join("source.txt");
    let destination_path = boundary.join("nested/destination.txt");
    fs::create_dir_all(destination_path.parent().unwrap()).unwrap();
    fs::create_dir_all(outside.join("nested")).unwrap();
    fs::write(&source_path, "inside").unwrap();
    fs::write(outside.join("source.txt"), "outside-source").unwrap();
    fs::write(
        outside.join("nested/destination.txt"),
        "outside-destination",
    )
    .unwrap();

    let entered = Arc::new(Barrier::new(2));
    let swapped = Arc::new(Barrier::new(2));
    let attacker_entered = Arc::clone(&entered);
    let attacker_swapped = Arc::clone(&swapped);
    let attacker_boundary = boundary.clone();
    let attacker_held = held.clone();
    let attacker_outside = outside.clone();
    let attacker_root = root.clone();
    let attacker = thread::spawn(move || {
        attacker_entered.wait();
        fs::rename(attacker_boundary, attacker_held).unwrap();
        symlink(attacker_outside, attacker_root.join("project")).unwrap();
        attacker_swapped.wait();
    });

    let operation_entered = Arc::clone(&entered);
    let operation_swapped = Arc::clone(&swapped);
    let source = WriteTarget::new(&source_path, &boundary, "test/rename-source");
    let destination = WriteTarget::new(&destination_path, &boundary, "test/rename-destination");
    let effect = with_test_hook(
        move |stage| {
            if stage == CapabilityTestStage::AfterRenameSourceParentCaptured {
                operation_entered.wait();
                operation_swapped.wait();
            }
        },
        || rename_noreplace(&source, &destination),
    )
    .unwrap();
    attacker.join().unwrap();

    assert!(effect.changed);
    assert!(!held.join("source.txt").exists());
    assert_eq!(
        fs::read_to_string(held.join("nested/destination.txt")).unwrap(),
        "inside"
    );
    assert_eq!(
        fs::read_to_string(outside.join("source.txt")).unwrap(),
        "outside-source"
    );
    assert_eq!(
        fs::read_to_string(outside.join("nested/destination.txt")).unwrap(),
        "outside-destination"
    );

    fs::remove_file(root.join("project")).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn append_rejects_hardlinked_leaf_before_mutation() {
    let root = unique_test_dir("capability-append-hardlink");
    let boundary = root.join("boundary");
    let outside = root.join("outside.txt");
    let target_path = boundary.join("journal.jsonl");
    fs::create_dir_all(&boundary).unwrap();
    fs::write(&outside, "sentinel").unwrap();
    fs::hard_link(&outside, &target_path).unwrap();

    let target = WriteTarget::new(&target_path, &boundary, "test/append-hardlink");
    let error = append(&target, b"mutated").unwrap_err();

    assert!(error.contains("hardlink"));
    assert_eq!(fs::read_to_string(&outside).unwrap(), "sentinel");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn stable_lock_file_excludes_a_second_writer_across_open_descriptors() {
    let root = unique_test_dir("capability-stable-lock");
    let boundary = root.join("logs");
    let lock_path = boundary.join(".kernel-log.lock");
    fs::create_dir_all(&boundary).unwrap();
    let target = WriteTarget::new(&lock_path, &boundary, "test/stable-lock");

    let first = lock_file(&target, CapabilityLockMode::Exclusive).unwrap();
    let lexical = lexical_target(&target, false).unwrap();
    let parent = capture_existing_target_parent(&lexical)
        .unwrap()
        .expect("lock parent should exist");
    let second_descriptor = rustix::fs::openat(
        &parent.directory,
        &parent.leaf,
        OFlags::RDWR | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .unwrap();
    let error = rustix::fs::flock(&second_descriptor, FlockOperation::NonBlockingLockExclusive)
        .unwrap_err();
    assert_eq!(error, Errno::WOULDBLOCK);

    drop(first);
    rustix::fs::flock(&second_descriptor, FlockOperation::NonBlockingLockExclusive).unwrap();
    drop(second_descriptor);
    fs::remove_dir_all(root).unwrap();
}

fn assert_no_temp_files(directory: &Path) {
    let leftovers = fs::read_dir(directory)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".pana-capability-")
        })
        .count();
    assert_eq!(leftovers, 0, "capability temp files must be cleaned");
}

fn unique_test_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "pana-studio-{label}-{}-{nanos}",
        std::process::id()
    ))
}
