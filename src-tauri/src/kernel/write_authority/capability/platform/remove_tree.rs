use super::*;

#[path = "remove_tree/recovery.rs"]
mod recovery;
#[path = "remove_tree/snapshot.rs"]
mod snapshot;
#[path = "remove_tree/traversal.rs"]
mod traversal;

pub(in crate::kernel::write_authority::capability) use recovery::{
    classify_remove_tree_recovery, execute_remove_tree_recovery, resolve_remove_tree_operator,
};
use recovery::{
    observe_tree, observed_matches_before, observed_matches_intact_moved, observed_root_from_stat,
};
use snapshot::{capture_tree_snapshot, mount_id_for_fd, mount_id_for_name, TreeSnapshot};
use traversal::{records_by_key, remove_planned_tree_contents};

#[derive(Clone, Copy)]
enum RemoveTreePublicState {
    Before,
    QuarantinedIntact,
    Removed,
}

pub(in crate::kernel::write_authority::capability) fn plan_remove_tree(
    target: &WriteTarget,
    operation_id: &str,
) -> Result<Option<RemoveTreeOperationPlan>, String> {
    let lexical = lexical_target(target, false)?;
    if lexical.authority.is_none() {
        return Err(capability_error(
            &lexical.public_label,
            "planul RemoveDirectoryTree WAL cere authority sigilată",
        ));
    }
    let Some(parent) = capture_existing_target_parent(&lexical)? else {
        return absent_plan_result(target, &lexical.public_label);
    };
    let Some(source_before) =
        leaf_metadata(&parent.directory, &parent.leaf, &lexical.public_label)?
    else {
        return absent_plan_result(target, &lexical.public_label);
    };
    match FileType::from_raw_mode(source_before.st_mode) {
        FileType::Symlink => {
            return Err(capability_error(
                &lexical.public_label,
                "RemoveDirectoryTree refuză un symlink leaf",
            ));
        }
        FileType::Directory => {}
        _ => {
            return Err(capability_error(
                &lexical.public_label,
                "RemoveDirectoryTree a primit un leaf care nu este director",
            ));
        }
    }
    if target.expected_leaf == ExpectedLeaf::Absent {
        return Err(capability_error(
            &lexical.public_label,
            "RemoveDirectoryTree a primit expected leaf Absent pentru un target existent",
        ));
    }

    let parent_mount_id = mount_id_for_fd(
        &parent.directory,
        &lexical.public_label,
        "RemoveDirectoryTree parent",
    )?;
    let named_mount_id = mount_id_for_name(
        &parent.directory,
        &parent.leaf,
        &lexical.public_label,
        "RemoveDirectoryTree source",
    )?;
    if parent_mount_id != named_mount_id {
        return Err(capability_error(
            &lexical.public_label,
            "RemoveDirectoryTree refuză targetul care este mount/bind-mount root",
        ));
    }

    let source_directory =
        open_directory_strict(&parent.directory, &parent.leaf).map_err(|error| {
            capability_error(
                &lexical.public_label,
                &format!("RemoveDirectoryTree WAL nu poate captura directorul: {error}"),
            )
        })?;
    validate_open_directory_identity(
        &source_directory,
        &source_before,
        &lexical.public_label,
        "RemoveDirectoryTree plan source",
    )?;
    let captured = fs::fstat(&source_directory).map_err(|error| {
        capability_error(
            &lexical.public_label,
            &format!("RemoveDirectoryTree WAL nu poate citi root metadata: {error}"),
        )
    })?;
    if mount_id_for_fd(
        &source_directory,
        &lexical.public_label,
        "RemoveDirectoryTree captured source",
    )? != named_mount_id
    {
        return Err(capability_error(
            &lexical.public_label,
            "RemoveDirectoryTree source și descriptorul capturat au mount ID diferit",
        ));
    }
    if !same_stable_leaf_version(&source_before, &captured) {
        return Err(capability_error(
            &lexical.public_label,
            "RemoveDirectoryTree source s-a schimbat în timpul capturii",
        ));
    }

    let snapshot = capture_tree_snapshot(
        &source_directory,
        named_mount_id,
        &lexical.public_label,
        "RemoveDirectoryTree plan",
    )?;
    match &target.expected_leaf {
        ExpectedLeaf::Present(expected) => {
            if expected.content_hash.is_some() {
                return Err(capability_error(
                    &lexical.public_label,
                    "RemoveDirectoryTree refuză content hash; directoarele cer tree fingerprint",
                ));
            }
            let expected_tree = expected.tree_fingerprint.as_deref().ok_or_else(|| {
                capability_error(
                    &lexical.public_label,
                    "RemoveDirectoryTree expected Present cere tree fingerprint",
                )
            })?;
            if version_token_for_stat(&captured) != expected.version_token
                || snapshot.fingerprint != expected_tree
            {
                return Err(capability_error(
                    &lexical.public_label,
                    "RemoveDirectoryTree source diferă de disk baseline înainte de WAL prepare",
                ));
            }
        }
        ExpectedLeaf::Unspecified => {}
        ExpectedLeaf::Absent => unreachable!(),
    }

    let captured_after = fs::fstat(&source_directory).map_err(|error| {
        capability_error(
            &lexical.public_label,
            &format!("RemoveDirectoryTree source nu poate fi reverificat: {error}"),
        )
    })?;
    if version_token_for_stat(&captured) != version_token_for_stat(&captured_after) {
        return Err(capability_error(
            &lexical.public_label,
            "RemoveDirectoryTree root s-a schimbat în timpul fingerprint-ului",
        ));
    }
    validate_named_directory_identity(
        &parent.directory,
        &parent.leaf,
        &source_directory,
        &lexical.public_label,
        "RemoveDirectoryTree plan source final",
    )?;

    let quarantine_leaf = remove_tree_quarantine_leaf(operation_id);
    if quarantine_leaf == parent.leaf
        || leaf_metadata(&parent.directory, &quarantine_leaf, &lexical.public_label)?.is_some()
    {
        return Err(capability_error(
            &lexical.public_label,
            "numele determinist RemoveDirectoryTree quarantine nu este disponibil",
        ));
    }
    let parent_identity = wal_identity_from_fd(&parent.directory, &lexical.public_label)?;
    let (_, parents) = lexical.relative_components.split_last().ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "RemoveDirectoryTree WAL cere un leaf",
        )
    })?;
    let source = source_evidence(
        &captured_after,
        named_mount_id,
        &snapshot,
        &lexical.public_label,
    )?;

    Ok(Some(RemoveTreeOperationPlan {
        evidence: WalRemoveTreeEvidence {
            parent: WalParentEvidence {
                relative_components_hex: parents
                    .iter()
                    .map(|component| encode_component_hex(component))
                    .collect(),
                existing_prefix_len: parents.len(),
                existing_ancestor_identity: parent_identity.clone(),
                parent_identity: Some(parent_identity),
            },
            target_leaf_hex: encode_component_hex(&parent.leaf),
            quarantine_leaf_hex: encode_component_hex(&quarantine_leaf),
            source,
        },
        source_directory,
        source_records: snapshot.records,
    }))
}

pub(in crate::kernel::write_authority::capability) fn remove_tree_wal(
    target: &WriteTarget,
    plan: RemoveTreeOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    let lexical = lexical_target(target, false)?;
    validate_plan_shape(&lexical, target, &plan, guard.operation_id())?;
    let parent = match capture_parent_from_wal_evidence(&lexical, &plan.evidence.parent) {
        Ok(parent) => parent,
        Err(error) => return error.into_operation_result(),
    };
    if parent.created_ancestors {
        return Err(capability_error(
            &lexical.public_label,
            "RemoveDirectoryTree WAL nu poate crea namespace părinte",
        ));
    }
    let quarantine_leaf = plan.quarantine_leaf()?;
    validate_source_before(
        &parent.directory,
        &parent.leaf,
        &plan,
        &lexical.public_label,
    )?;
    validate_leaf_absent(
        &parent.directory,
        &quarantine_leaf,
        &lexical.public_label,
        "RemoveDirectoryTree quarantine",
    )?;
    validate_public_state(&lexical, &plan.evidence, RemoveTreePublicState::Before)?;

    let recovery = |diagnostic: String| {
        wal_recovery_effect(
            0,
            &lexical.public_label,
            format!(
                "{diagnostic} RemoveDirectoryTree WAL rămâne pentru recovery; nu repeta operația automat."
            ),
        )
    };
    if let Err(error) = guard.mark_auxiliary_durable() {
        return Ok(recovery(error));
    }

    run_test_hook(CapabilityTestStage::BeforeRemoveTreeQuarantine);
    if let Err(error) = fs::renameat_with(
        &parent.directory,
        &parent.leaf,
        &parent.directory,
        &quarantine_leaf,
        RenameFlags::NOREPLACE,
    ) {
        return Ok(recovery(capability_error(
            &lexical.public_label,
            &format!("RemoveDirectoryTree quarantine rename a eșuat: {error}"),
        )));
    }
    if let Err(error) = guard.mark_effect_visible() {
        return Ok(recovery(error));
    }
    if let Err(error) = validate_quarantine_intact(
        &parent.directory,
        &parent.leaf,
        &quarantine_leaf,
        &plan,
        &lexical.public_label,
    ) {
        return Ok(recovery(error));
    }
    if let Err(error) = sync_directory(&parent.directory, &lexical.public_label) {
        return Ok(recovery(error));
    }
    if let Err(error) = validate_public_state(
        &lexical,
        &plan.evidence,
        RemoveTreePublicState::QuarantinedIntact,
    ) {
        return Ok(recovery(error));
    }

    run_test_hook(CapabilityTestStage::BeforeRemoveTreeTraversal);
    let planned = records_by_key(&plan.source_records);
    let mut removed = 0_u64;
    if let Err(error) = remove_planned_tree_contents(
        &plan.source_directory,
        "",
        0,
        &mut removed,
        plan.evidence.source.mount_id,
        &planned,
        &lexical.public_label,
    ) {
        return Ok(recovery(format!(
            "{error} Arborele parțial rămâne izolat în quarantine {}.",
            quarantine_leaf.to_string_lossy()
        )));
    }
    if removed != plan.evidence.source.entry_count {
        return Ok(recovery(capability_error(
            &lexical.public_label,
            &format!(
                "RemoveDirectoryTree a eliminat {removed} intrări din {} planificate",
                plan.evidence.source.entry_count
            ),
        )));
    }
    if let Err(error) = validate_named_directory_identity(
        &parent.directory,
        &quarantine_leaf,
        &plan.source_directory,
        &lexical.public_label,
        "RemoveDirectoryTree quarantine root",
    ) {
        return Ok(recovery(error));
    }
    if let Err(error) = fs::unlinkat(&parent.directory, &quarantine_leaf, AtFlags::REMOVEDIR) {
        return Ok(recovery(capability_error(
            &lexical.public_label,
            &format!("RemoveDirectoryTree quarantine root nu poate fi eliminat: {error}"),
        )));
    }
    if let Err(error) = sync_directory(&parent.directory, &lexical.public_label) {
        return Ok(recovery(error));
    }
    if let Err(error) =
        validate_public_state(&lexical, &plan.evidence, RemoveTreePublicState::Removed)
    {
        return Ok(recovery(error));
    }
    run_test_hook(CapabilityTestStage::BeforeRemoveTreeTargetDurable);
    if let Err(error) = guard.mark_target_durable() {
        return Ok(recovery(error));
    }
    if let Err(error) =
        validate_public_state(&lexical, &plan.evidence, RemoveTreePublicState::Removed)
    {
        return Ok(recovery(error));
    }
    Ok(CapabilityEffect::changed(0))
}

/// Removes an ephemeral, rebuildable directory through the same
/// descriptor-bound snapshot/quarantine traversal as the journaled project
/// operation. This deliberately has no global WAL: callers may only use it
/// for private generations which can be regenerated, never source of truth.
pub(in crate::kernel::write_authority::capability) fn remove_rebuildable_tree(
    target: &WriteTarget,
    operation_id: &str,
) -> Result<CapabilityEffect, String> {
    let lexical = lexical_target(target, false)?;
    let Some(plan) = plan_remove_tree(target, operation_id)? else {
        return Ok(CapabilityEffect::unchanged());
    };
    validate_plan_shape(&lexical, target, &plan, operation_id)?;
    let parent = match capture_parent_from_wal_evidence(&lexical, &plan.evidence.parent) {
        Ok(parent) => parent,
        Err(error) => return error.into_operation_result(),
    };
    if parent.created_ancestors {
        return Err(capability_error(
            &lexical.public_label,
            "cleanup-ul rebuildable nu poate crea namespace părinte",
        ));
    }
    let quarantine_leaf = plan.quarantine_leaf()?;
    validate_source_before(
        &parent.directory,
        &parent.leaf,
        &plan,
        &lexical.public_label,
    )?;
    validate_leaf_absent(
        &parent.directory,
        &quarantine_leaf,
        &lexical.public_label,
        "cleanup rebuildable quarantine",
    )?;
    validate_public_state(&lexical, &plan.evidence, RemoveTreePublicState::Before)?;

    let recovery = |diagnostic: String| {
        CapabilityEffect::recovery_required(
            0,
            format!(
                "{diagnostic} Generația rebuildable poate rămâne izolată în quarantine {}; nu repeta cleanup-ul automat.",
                quarantine_leaf.to_string_lossy()
            ),
        )
    };
    if let Err(error) = fs::renameat_with(
        &parent.directory,
        &parent.leaf,
        &parent.directory,
        &quarantine_leaf,
        RenameFlags::NOREPLACE,
    ) {
        return Err(capability_error(
            &lexical.public_label,
            &format!("cleanup-ul rebuildable nu a putut izola generația: {error}"),
        ));
    }
    if let Err(error) = validate_quarantine_intact(
        &parent.directory,
        &parent.leaf,
        &quarantine_leaf,
        &plan,
        &lexical.public_label,
    ) {
        return Ok(recovery(error));
    }
    if let Err(error) = sync_directory(&parent.directory, &lexical.public_label) {
        return Ok(recovery(error));
    }

    let planned = records_by_key(&plan.source_records);
    let mut removed = 0_u64;
    if let Err(error) = remove_planned_tree_contents(
        &plan.source_directory,
        "",
        0,
        &mut removed,
        plan.evidence.source.mount_id,
        &planned,
        &lexical.public_label,
    ) {
        return Ok(recovery(error));
    }
    if removed != plan.evidence.source.entry_count {
        return Ok(recovery(capability_error(
            &lexical.public_label,
            &format!(
                "cleanup-ul rebuildable a eliminat {removed} intrări din {} planificate",
                plan.evidence.source.entry_count
            ),
        )));
    }
    if let Err(error) = validate_named_directory_identity(
        &parent.directory,
        &quarantine_leaf,
        &plan.source_directory,
        &lexical.public_label,
        "cleanup rebuildable quarantine root",
    ) {
        return Ok(recovery(error));
    }
    if let Err(error) = fs::unlinkat(&parent.directory, &quarantine_leaf, AtFlags::REMOVEDIR) {
        return Ok(recovery(capability_error(
            &lexical.public_label,
            &format!("cleanup-ul rebuildable nu a putut elimina rădăcina: {error}"),
        )));
    }
    if let Err(error) = sync_directory(&parent.directory, &lexical.public_label) {
        return Ok(recovery(error));
    }
    validate_public_state(&lexical, &plan.evidence, RemoveTreePublicState::Removed)?;
    Ok(CapabilityEffect::changed(0))
}

impl RemoveTreeOperationPlan {
    fn quarantine_leaf(&self) -> Result<OsString, String> {
        decode_component_hex(&self.evidence.quarantine_leaf_hex)
    }
}

fn absent_plan_result(
    target: &WriteTarget,
    public_label: &str,
) -> Result<Option<RemoveTreeOperationPlan>, String> {
    if matches!(target.expected_leaf, ExpectedLeaf::Present(_)) {
        Err(capability_error(
            public_label,
            "RemoveDirectoryTree expected Present, dar directorul lipsește înainte de WAL",
        ))
    } else {
        Ok(None)
    }
}

fn source_evidence(
    stat: &fs::Stat,
    mount_id: u64,
    snapshot: &TreeSnapshot,
    public_label: &str,
) -> Result<WalRemoveTreeSourceEvidence, String> {
    Ok(WalRemoveTreeSourceEvidence {
        identity: WalFilesystemIdentity {
            device: stat.st_dev,
            inode: stat.st_ino,
        },
        size: u64::try_from(stat.st_size).map_err(|_| {
            capability_error(
                public_label,
                "RemoveDirectoryTree root are dimensiune negativă",
            )
        })?,
        mtime_seconds: stat.st_mtime,
        mtime_nanoseconds: stat.st_mtime_nsec,
        ctime_seconds: stat.st_ctime,
        ctime_nanoseconds: stat.st_ctime_nsec,
        raw_mode: stat.st_mode,
        link_count: stat.st_nlink,
        owner_uid: stat.st_uid,
        owner_gid: stat.st_gid,
        raw_device: stat.st_rdev,
        version_token: version_token_for_stat(stat),
        tree_fingerprint: snapshot.fingerprint.clone(),
        entry_count: snapshot.entry_count,
        mount_id,
    })
}

fn validate_plan_shape(
    lexical: &LexicalTarget,
    target: &WriteTarget,
    plan: &RemoveTreeOperationPlan,
    operation_id: &str,
) -> Result<(), String> {
    let (leaf, parents) = lexical.relative_components.split_last().ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "RemoveDirectoryTree WAL cere un leaf",
        )
    })?;
    let planned_parents = plan
        .evidence
        .parent
        .relative_components_hex
        .iter()
        .map(|component| decode_component_hex(component))
        .collect::<Result<Vec<_>, _>>()?;
    if planned_parents != parents
        || decode_component_hex(&plan.evidence.target_leaf_hex)? != *leaf
        || plan.quarantine_leaf()? != remove_tree_quarantine_leaf(operation_id)
        || plan.evidence.source.entry_count != plan.source_records.len() as u64
        || plan.evidence.source.tree_fingerprint
            != tree_fingerprint_from_records(plan.source_records.clone())
    {
        return Err(capability_error(
            &lexical.public_label,
            "planul WAL RemoveDirectoryTree nu corespunde targetului sau inventarului executat",
        ));
    }
    match &target.expected_leaf {
        ExpectedLeaf::Unspecified => Ok(()),
        ExpectedLeaf::Present(expected)
            if expected.content_hash.is_none()
                && expected.version_token == plan.evidence.source.version_token
                && expected.tree_fingerprint.as_deref()
                    == Some(plan.evidence.source.tree_fingerprint.as_str()) =>
        {
            Ok(())
        }
        _ => Err(capability_error(
            &lexical.public_label,
            "expected-state RemoveDirectoryTree diferă de planul WAL",
        )),
    }
}

fn validate_source_before(
    parent: &OwnedFd,
    leaf: &OsStr,
    plan: &RemoveTreeOperationPlan,
    public_label: &str,
) -> Result<(), String> {
    let stat = fs::statat(parent, leaf, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
        capability_error(
            public_label,
            &format!("RemoveDirectoryTree source nu mai poate fi verificat: {error}"),
        )
    })?;
    validate_named_directory_identity(
        parent,
        leaf,
        &plan.source_directory,
        public_label,
        "RemoveDirectoryTree source",
    )?;
    let mount_id = mount_id_for_name(parent, leaf, public_label, "RemoveDirectoryTree source")?;
    let snapshot = capture_tree_snapshot(
        &plan.source_directory,
        plan.evidence.source.mount_id,
        public_label,
        "RemoveDirectoryTree pre-rename",
    )?;
    let observed =
        observed_root_from_stat(&stat, Some(mount_id), Some(snapshot.clone()), public_label)?;
    if !observed_matches_before(&observed, &plan.evidence.source) {
        return Err(capability_error(
            public_label,
            "RemoveDirectoryTree root diferă de baseline înainte de quarantine",
        ));
    }
    if snapshot.records != plan.source_records
        || snapshot.fingerprint != plan.evidence.source.tree_fingerprint
    {
        return Err(capability_error(
            public_label,
            "RemoveDirectoryTree descendenții diferă de plan înainte de quarantine",
        ));
    }
    Ok(())
}

fn validate_quarantine_intact(
    parent: &OwnedFd,
    target_leaf: &OsStr,
    quarantine_leaf: &OsStr,
    plan: &RemoveTreeOperationPlan,
    public_label: &str,
) -> Result<(), String> {
    validate_leaf_absent(
        parent,
        target_leaf,
        public_label,
        "RemoveDirectoryTree target",
    )?;
    validate_named_directory_identity(
        parent,
        quarantine_leaf,
        &plan.source_directory,
        public_label,
        "RemoveDirectoryTree quarantine",
    )?;
    let stat = fs::fstat(&plan.source_directory).map_err(|error| {
        capability_error(
            public_label,
            &format!("RemoveDirectoryTree quarantine metadata nu poate fi citită: {error}"),
        )
    })?;
    let mount_id = mount_id_for_fd(
        &plan.source_directory,
        public_label,
        "RemoveDirectoryTree quarantine",
    )?;
    let snapshot = capture_tree_snapshot(
        &plan.source_directory,
        mount_id,
        public_label,
        "RemoveDirectoryTree post-rename",
    )?;
    let observed = observed_root_from_stat(&stat, Some(mount_id), Some(snapshot), public_label)?;
    if !observed_matches_intact_moved(&observed, &plan.evidence.source)
        || observed.snapshot.as_ref().map(|value| &value.records) != Some(&plan.source_records)
    {
        return Err(capability_error(
            public_label,
            "RemoveDirectoryTree quarantine nu mai este arborele planificat intact",
        ));
    }
    Ok(())
}

fn validate_public_state(
    lexical: &LexicalTarget,
    evidence: &WalRemoveTreeEvidence,
    state: RemoveTreePublicState,
) -> Result<(), String> {
    let parent = capture_existing_target_parent(lexical)?.ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "RemoveDirectoryTree full-path CAS nu mai poate captura parentul",
        )
    })?;
    let observed_parent = wal_identity_from_fd(&parent.directory, &lexical.public_label)?;
    if evidence.parent.parent_identity.as_ref() != Some(&observed_parent) {
        return Err(capability_error(
            &lexical.public_label,
            "RemoveDirectoryTree full-path CAS a observat alt parent",
        ));
    }
    let target = observe_tree(&parent.directory, &parent.leaf, &lexical.public_label)?;
    let quarantine_leaf = decode_component_hex(&evidence.quarantine_leaf_hex)?;
    let quarantine = observe_tree(&parent.directory, &quarantine_leaf, &lexical.public_label)?;
    let valid = match state {
        RemoveTreePublicState::Before => {
            target
                .as_ref()
                .is_some_and(|value| observed_matches_before(value, &evidence.source))
                && quarantine.is_none()
        }
        RemoveTreePublicState::QuarantinedIntact => {
            target.is_none()
                && quarantine
                    .as_ref()
                    .is_some_and(|value| observed_matches_intact_moved(value, &evidence.source))
        }
        RemoveTreePublicState::Removed => target.is_none() && quarantine.is_none(),
    };
    if !valid {
        return Err(capability_error(
            &lexical.public_label,
            "RemoveDirectoryTree full-path CAS diferă de starea WAL așteptată",
        ));
    }
    Ok(())
}

fn validate_leaf_absent(
    parent: &OwnedFd,
    leaf: &OsStr,
    public_label: &str,
    role: &str,
) -> Result<(), String> {
    if leaf_metadata(parent, leaf, public_label)?.is_some() {
        return Err(capability_error(
            public_label,
            &format!("{role} trebuia să fie absent"),
        ));
    }
    Ok(())
}
