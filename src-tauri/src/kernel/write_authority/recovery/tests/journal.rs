use super::{fixtures::*, *};

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
fn symlink_wal_round_trips_non_utf8_literal() {
    let fixture = AtomicRecoveryFixture::new("symlink-non-utf8", false);
    let target = fixture.parent.join("link");
    let source = PathBuf::from(OsString::from_vec(b"../\xff-target".to_vec()));
    let (coordinator, intent, plan, record) =
        fixture.prepare_symlink("wal-symlink-non-utf8", &target, &source);
    let mut guard = coordinator.begin(record).unwrap();
    let effect = capability::symlink_entry_wal(&intent.target, &source, &plan, &mut guard).unwrap();
    assert!(effect.changed);
    assert!(!effect.recovery_required);
    guard.commit().unwrap();
    assert_eq!(fs::read_link(&target).unwrap(), source);
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
fn symlink_v2_round_trips_non_utf8_literal_direct() {
    let fixture = AtomicRecoveryFixture::new("symlink-v2-non-utf8", false);
    let target = fixture.parent.join("link");
    let source = PathBuf::from(OsString::from_vec(b"../\xff-target".to_vec()));
    let (coordinator, intent, plan, record) =
        fixture.prepare_symlink_v2("wal-symlink-v2-non-utf8", &target, &source, true);
    let mut guard = coordinator.begin(record).unwrap();
    let effect = capability::symlink_entry_wal(&intent.target, &source, &plan, &mut guard).unwrap();
    assert!(effect.changed);
    assert!(!effect.recovery_required, "{effect:?}");
    guard.commit().unwrap();
    assert_eq!(fs::read_link(&target).unwrap(), source);
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
    let WalOperationEvidence::Symlink(evidence) = &mut unknown_protocol.operation_evidence else {
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
    let WalOperationEvidence::Symlink(evidence) = &mut inconsistent_exact.operation_evidence else {
        unreachable!()
    };
    evidence.desired_link_target_hex =
        super::super::encode_path_hex(Path::new("different-desired"));
    let error = super::super::WalRecord::seal(inconsistent_exact).unwrap_err();
    assert!(error.contains("literal diferit"), "{error}");
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

    let error = super::super::wal_io::with_record_remove_failure_before_unlink(|| guard.commit())
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
fn copy_checkpoint_filename_is_rejected_for_non_copy_record_family() {
    let fixture = AtomicRecoveryFixture::new("copy-v2-family-binding", false);
    let target = fixture.parent.join("directory-target");
    let operation_id = "wal-copy-v2-family-binding";
    let (coordinator, _intent, _plan, record) = fixture.prepare_directory(operation_id, &target);
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
    let super::super::WalOperationEvidence::Copy(evidence) = &mut body.operation_evidence else {
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
    let super::super::WalOperationEvidence::Copy(evidence) = &mut body.operation_evidence else {
        unreachable!();
    };
    evidence.protocol_version = 99;
    let error = super::super::WalRecord::seal(body).unwrap_err();
    assert!(error.contains("evidence copy invalidă"), "{error}");
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
    let super::super::WalOperationEvidence::Append(evidence) = &mut wrong_tail.operation_evidence
    else {
        unreachable!()
    };
    evidence.before_tail_size = evidence.before_tail_size.saturating_sub(1);
    assert!(super::super::WalRecord::seal(wrong_tail).is_err());

    let mut wrong_parent = record.body.clone();
    let super::super::WalOperationEvidence::Append(evidence) = &mut wrong_parent.operation_evidence
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
