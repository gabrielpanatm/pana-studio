use super::*;
use super::{observability::now_ms, session::*};

pub(super) enum IntegrationTreePublication {
    Applied {
        changed_paths: Vec<String>,
    },
    RecoveryRequired {
        changed_paths: Vec<String>,
        diagnostic: String,
    },
}

#[allow(clippy::too_many_arguments)]
pub(super) fn publish_integration_tree(
    app: &AppHandle,
    root: &Path,
    workspace: &mut ProjectWorkspace,
    captured: &CapturedVersioningSession,
    repository: &VersionRepository,
    session_lease: ActiveProjectReadLease<'_>,
    prepared: &PreparedVersionIntegration,
    current_tree: &VersionTree,
    target_tree: &VersionTree,
    allowed_baseline_divergence: &BTreeSet<String>,
    label: String,
    source: &str,
) -> Result<IntegrationTreePublication, String> {
    let mut plan = build_version_restore_plan(workspace, current_tree, target_tree)?;
    for change in &mut plan.binary_changes {
        let live = session_lease.read_bounded_regular_file(
            Path::new(&change.relative_path),
            32 * 1024 * 1024,
            "versioning/integration-binary-baseline",
        )?;
        let live_bytes = live.map(|snapshot| snapshot.bytes);
        let source_relative = change.relative_path.as_str();
        if live_bytes != change.before && !allowed_baseline_divergence.contains(source_relative) {
            return Err(format!(
                "Integrarea a fost blocată: baseline-ul live pentru {} nu corespunde arborelui HEAD Git.",
                change.relative_path
            ));
        }
        change.before = live_bytes;
    }
    let changed_paths = plan.changed_paths.clone();
    let expected_files = plan.expected_files.clone();
    let mut candidate = workspace.fork_candidate();
    let workspace_identity = ProjectWorkspaceIdentity {
        expected_project_root: captured.session.project_root.clone(),
        expected_session_id: captured.runtime_session_id.clone(),
        expected_revision: candidate.revision,
    };
    let metadata = WorkspaceMutationMetadata {
        label,
        source: source.to_string(),
        coalesce_key: None,
        transaction_id: Some(prepared.transaction_id.clone()),
    };
    if let Err(error) = candidate.stage_version_tree_restore(
        &workspace_identity,
        metadata,
        plan.text_changes,
        plan.text_deletes,
        plan.binary_changes,
        now_ms(),
    ) {
        let cleanup = repository.delete_integration_marker(prepared);
        return Err(match cleanup {
            Ok(()) => error,
            Err(cleanup_error) => format!(
                "{error} Marker-ul durabil {} nu a putut fi eliminat: {cleanup_error}",
                prepared.recovery_ref
            ),
        });
    }

    drop(session_lease);
    match save_project_workspace_with_recovery(app, root, &mut candidate, &workspace_identity) {
        Ok(_) => {}
        Err(ProjectWorkspaceSaveError::Rejected { diagnostic }) => {
            let cleanup = repository.delete_integration_marker(prepared);
            return Err(match cleanup {
                Ok(()) => diagnostic,
                Err(cleanup_error) => format!(
                    "{diagnostic} Marker-ul durabil {} a fost păstrat deoarece cleanup-ul a eșuat: {cleanup_error}",
                    prepared.recovery_ref
                ),
            });
        }
        Err(ProjectWorkspaceSaveError::RecoveryRequired { diagnostic, .. }) => {
            return Ok(IntegrationTreePublication::RecoveryRequired {
                changed_paths,
                diagnostic: format!(
                    "Save-ul integrării are nevoie de recovery: {diagnostic} Marker-ul Git durabil a fost păstrat. Nu repeta operația automat."
                ),
            });
        }
    }
    workspace.adopt_candidate(candidate);
    emit_project_workspace_mutated(app, workspace, ProjectWorkspacePreviewProjection::Required);

    let authority = app.state::<WriteAuthorityRuntime>();
    let verify_lease = authority.acquire_active_project_read_lease_for_session(
        &captured.root,
        &captured.runtime_session_id,
    )?;
    if let Err(error) = verify_restored_files(&verify_lease, &expected_files) {
        return Ok(IntegrationTreePublication::RecoveryRequired {
            changed_paths,
            diagnostic: format!(
                "Fișierele integrării nu au trecut verificarea byte-cu-byte: {error} Marker-ul Git durabil a fost păstrat."
            ),
        });
    }
    Ok(IntegrationTreePublication::Applied { changed_paths })
}

pub(super) fn changed_tree_paths<'a>(
    current: &'a VersionTree,
    target: &'a VersionTree,
) -> BTreeSet<&'a str> {
    let current_files = current
        .files
        .iter()
        .map(|file| (file.path.as_str(), (&file.oid, file.executable)))
        .collect::<std::collections::BTreeMap<_, _>>();
    let target_files = target
        .files
        .iter()
        .map(|file| (file.path.as_str(), (&file.oid, file.executable)))
        .collect::<std::collections::BTreeMap<_, _>>();
    current_files
        .keys()
        .chain(target_files.keys())
        .copied()
        .filter(|path| current_files.get(path) != target_files.get(path))
        .collect()
}

pub(super) fn expected_tree_files(
    current: &VersionTree,
    target: &VersionTree,
) -> Vec<VersionRestoreExpectedFile> {
    let target_paths = target
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<BTreeSet<_>>();
    let mut expected = target
        .files
        .iter()
        .map(|file| VersionRestoreExpectedFile {
            project_relative_path: file.path.clone(),
            expected_bytes: Some(file.bytes.clone()),
        })
        .collect::<Vec<_>>();
    expected.extend(
        current
            .files
            .iter()
            .filter(|file| !target_paths.contains(file.path.as_str()))
            .map(|file| VersionRestoreExpectedFile {
                project_relative_path: file.path.clone(),
                expected_bytes: None,
            }),
    );
    expected.sort_by(|left, right| left.project_relative_path.cmp(&right.project_relative_path));
    expected
}

pub(super) fn verify_restored_files(
    lease: &ActiveProjectReadLease<'_>,
    expected_files: &[VersionRestoreExpectedFile],
) -> Result<(), String> {
    for expected in expected_files {
        let expected_size = expected
            .expected_bytes
            .as_ref()
            .map(|bytes| bytes.len() as u64)
            .unwrap_or(0);
        let live = lease.read_bounded_regular_file(
            Path::new(&expected.project_relative_path),
            expected_size.saturating_add(1),
            "versioning/restore-byte-verification",
        )?;
        let live_bytes = live.map(|snapshot| snapshot.bytes);
        if live_bytes != expected.expected_bytes {
            return Err(format!(
                "{} diferă de versiunea țintă.",
                expected.project_relative_path
            ));
        }
    }
    Ok(())
}
