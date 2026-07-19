use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use super::{
    reject_external_driver_attributes,
    repository::{require_ready, validate_commit_message, validate_oid, validate_path, zero_oid},
    PreparedVersionIntegration, VersionIntegrationInput, VersionIntegrationKind,
    VersionIntegrationMode, VersionIntegrationPlan, VersionIntegrationRelationship,
    VersionIntegrationTargetInput, VersionRepository, VersionSwitchBranchInput, VersionTree,
    VersioningSnapshot, VERSIONING_SCHEMA_VERSION,
};

const MAX_INTEGRATION_MARKERS: usize = 8;
const MAX_INTEGRATION_CONFLICTS: usize = 5_000;
const MAX_MARKER_COMMIT_BYTES: usize = 256 * 1024;
static INTEGRATION_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct MergeTreeResult {
    tree_oid: String,
    conflict_paths: Vec<String>,
}

impl VersionRepository {
    pub(crate) fn integration_plan(
        &self,
        input: &VersionIntegrationTargetInput,
    ) -> Result<VersionIntegrationPlan, String> {
        let snapshot = self.snapshot()?;
        require_ready(&snapshot)?;
        let head_oid = snapshot
            .head_oid
            .as_deref()
            .ok_or_else(|| "Integrarea cere cel puțin un commit local.".to_string())?;
        let (target_ref, target_oid) = self.resolve_integration_target(
            &snapshot,
            &input.target_ref,
            &input.expected_target_oid,
        )?;
        let relationship = self.integration_relationship(head_oid, &target_oid)?;
        let (ahead, behind) = self.ahead_behind(head_oid, &target_oid)?;
        let local_range = format!("{target_oid}..{head_oid}");
        let target_range = format!("{head_oid}..{target_oid}");
        let diagnostic = match relationship {
            VersionIntegrationRelationship::Same => {
                "Branch-ul local și ținta indică același commit.".to_string()
            }
            VersionIntegrationRelationship::FastForward => {
                "Ținta este descendentă din HEAD și poate fi aplicată fast-forward fără commit suplimentar."
                    .to_string()
            }
            VersionIntegrationRelationship::LocalAhead => {
                "Ținta este deja strămoșul HEAD; nu există versiuni remote de integrat."
                    .to_string()
            }
            VersionIntegrationRelationship::Diverged => {
                "Istoricul local și ținta au divergat; integrarea cere un merge explicit."
                    .to_string()
            }
        };
        Ok(VersionIntegrationPlan {
            schema_version: VERSIONING_SCHEMA_VERSION,
            head_oid: head_oid.to_string(),
            target_ref,
            target_oid,
            relationship,
            ahead,
            behind,
            local_only: self.history_for_range(&local_range)?,
            target_only: self.history_for_range(&target_range)?,
            fast_forward_allowed: relationship == VersionIntegrationRelationship::FastForward,
            merge_allowed: matches!(
                relationship,
                VersionIntegrationRelationship::FastForward
                    | VersionIntegrationRelationship::Diverged
            ),
            repository_clean: snapshot.clean,
            diagnostic,
        })
    }

    pub(crate) fn prepare_integration(
        &self,
        input: &VersionIntegrationInput,
    ) -> Result<PreparedVersionIntegration, String> {
        let snapshot = self.snapshot()?;
        require_ready(&snapshot)?;
        self.require_clean_integration_start(&snapshot)?;
        let previous_head_oid = snapshot
            .head_oid
            .clone()
            .ok_or_else(|| "Integrarea cere cel puțin un commit local.".to_string())?;
        let full_head_ref = self.current_full_head_ref()?;
        let (target_ref, target_oid) = self.resolve_integration_target(
            &snapshot,
            &input.target_ref,
            &input.expected_target_oid,
        )?;
        let relationship = self.integration_relationship(&previous_head_oid, &target_oid)?;
        match input.mode {
            VersionIntegrationMode::FastForward
                if relationship != VersionIntegrationRelationship::FastForward =>
            {
                return Err(
                    "Fast-forward a fost blocat deoarece ținta nu mai este un descendent direct al HEAD. Actualizează planul de integrare."
                        .to_string(),
                )
            }
            VersionIntegrationMode::Merge
                if !matches!(
                    relationship,
                    VersionIntegrationRelationship::FastForward
                        | VersionIntegrationRelationship::Diverged
                ) =>
            {
                return Err("Nu există o divergență care să necesite merge.".to_string())
            }
            _ => {}
        }

        let message = validate_commit_message(&input.message)?;
        let previous_tree_oid = self.tree_oid(&previous_head_oid)?;
        let target_tree = self.read_tree(&target_oid)?;
        reject_external_driver_attributes(&target_tree)?;
        let (kind, target_tree_oid, conflict_paths) = match input.mode {
            VersionIntegrationMode::FastForward => (
                VersionIntegrationKind::FastForward,
                target_tree.tree_oid.clone(),
                Vec::new(),
            ),
            VersionIntegrationMode::Merge => {
                if snapshot.user_name.is_none() || snapshot.user_email.is_none() {
                    return Err("Merge-ul cere identitatea Git locală configurată.".to_string());
                }
                let merged = self.merge_tree(&previous_head_oid, &target_oid)?;
                let kind = if merged.conflict_paths.is_empty() {
                    VersionIntegrationKind::MergeClean
                } else {
                    VersionIntegrationKind::MergeConflict
                };
                (kind, merged.tree_oid, merged.conflict_paths)
            }
        };
        self.create_integration_marker(
            kind,
            &previous_head_oid,
            &previous_tree_oid,
            &full_head_ref,
            &target_ref,
            &target_oid,
            &target_tree_oid,
            None,
            &conflict_paths,
            &message,
        )
    }

    pub(crate) fn prepare_branch_switch(
        &self,
        input: &VersionSwitchBranchInput,
    ) -> Result<PreparedVersionIntegration, String> {
        let snapshot = self.snapshot()?;
        require_ready(&snapshot)?;
        self.require_clean_integration_start(&snapshot)?;
        let previous_head_oid = snapshot
            .head_oid
            .clone()
            .ok_or_else(|| "Schimbarea branch-ului cere un commit local.".to_string())?;
        let branch = self.validate_branch_name(&input.branch)?;
        if snapshot.branch.as_deref() == Some(branch.as_str()) {
            return Err("Branch-ul ales este deja activ.".to_string());
        }
        let target_ref = format!("refs/heads/{branch}");
        let target_oid = self.resolve_ref_oid(&target_ref)?;
        let expected = self.resolve_commit_oid(&input.expected_target_oid)?;
        if target_oid != expected {
            return Err("Branch-ul țintă s-a schimbat; actualizează panoul Versiuni.".to_string());
        }
        let target_tree = self.read_tree(&target_oid)?;
        reject_external_driver_attributes(&target_tree)?;
        self.create_integration_marker(
            VersionIntegrationKind::SwitchBranch,
            &previous_head_oid,
            &self.tree_oid(&previous_head_oid)?,
            &self.current_full_head_ref()?,
            &target_ref,
            &target_oid,
            &target_tree.tree_oid,
            Some(&branch),
            &[],
            &format!("Schimbare branch la {branch}"),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn create_integration_marker(
        &self,
        kind: VersionIntegrationKind,
        previous_head_oid: &str,
        previous_tree_oid: &str,
        full_head_ref: &str,
        target_ref: &str,
        target_oid: &str,
        target_tree_oid: &str,
        target_branch: Option<&str>,
        conflict_paths: &[String],
        message: &str,
    ) -> Result<PreparedVersionIntegration, String> {
        validate_oid(previous_head_oid)?;
        validate_oid(previous_tree_oid)?;
        validate_oid(target_oid)?;
        validate_oid(target_tree_oid)?;
        if !full_head_ref.starts_with("refs/heads/") {
            return Err("HEAD nu indică un branch local suportat.".to_string());
        }
        let transaction_id = integration_transaction_id();
        let recovery_ref = format!("refs/pana-studio/integrations/{transaction_id}");
        let marker_message = integration_commit_message(
            message,
            &transaction_id,
            kind,
            full_head_ref,
            target_ref,
            target_oid,
            target_tree_oid,
            target_branch,
            conflict_paths,
        )?;
        let marker_commit_oid = self
            .runner
            .run_with_input(
                [
                    "commit-tree",
                    target_tree_oid,
                    "-p",
                    previous_head_oid,
                    "-p",
                    target_oid,
                ],
                marker_message.as_bytes(),
            )?
            .require_success("Crearea marker-ului durabil de integrare")?
            .stdout_text()?
            .trim()
            .to_string();
        validate_oid(&marker_commit_oid)?;
        let result_commit_oid = match kind {
            VersionIntegrationKind::FastForward | VersionIntegrationKind::SwitchBranch => {
                Some(target_oid.to_string())
            }
            VersionIntegrationKind::MergeClean | VersionIntegrationKind::MergeResolved => {
                Some(marker_commit_oid.clone())
            }
            VersionIntegrationKind::MergeConflict => None,
        };
        let snapshot = self.snapshot()?;
        let zero = zero_oid(snapshot.object_format.as_deref());
        self.runner
            .run(["update-ref", &recovery_ref, &marker_commit_oid, &zero])?
            .require_success("Publicarea marker-ului durabil de integrare")?;
        Ok(PreparedVersionIntegration {
            transaction_id,
            recovery_ref,
            kind,
            previous_head_oid: previous_head_oid.to_string(),
            previous_tree_oid: previous_tree_oid.to_string(),
            full_head_ref: full_head_ref.to_string(),
            target_ref: target_ref.to_string(),
            target_oid: target_oid.to_string(),
            target_tree_oid: target_tree_oid.to_string(),
            marker_commit_oid,
            result_commit_oid,
            target_branch: target_branch.map(str::to_string),
            conflict_paths: conflict_paths.to_vec(),
            message: message.to_string(),
        })
    }

    pub(crate) fn read_integration_markers(
        &self,
    ) -> Result<Vec<PreparedVersionIntegration>, String> {
        let snapshot = self.snapshot()?;
        require_ready(&snapshot)?;
        let output = self
            .runner
            .run([
                "for-each-ref",
                "--format=%(refname)%00%(objectname)",
                "refs/pana-studio/integrations",
            ])?
            .require_success("Citirea marker-elor de integrare")?;
        if output.stdout_truncated {
            return Err("Lista marker-elor de integrare a fost trunchiată.".to_string());
        }
        let source = output.stdout_text()?;
        let mut markers = Vec::new();
        for line in source.lines().filter(|line| !line.is_empty()) {
            let (recovery_ref, marker_commit_oid) = line.split_once('\0').ok_or_else(|| {
                "Git a returnat un marker de integrare cu format invalid.".to_string()
            })?;
            let transaction_id = recovery_ref
                .strip_prefix("refs/pana-studio/integrations/")
                .filter(|value| !value.is_empty() && !value.contains('/'))
                .ok_or_else(|| format!("Marker de integrare invalid: {recovery_ref}."))?;
            validate_oid(marker_commit_oid)?;
            let output = self
                .runner
                .run_with_limit(
                    ["cat-file", "commit", marker_commit_oid],
                    MAX_MARKER_COMMIT_BYTES,
                )?
                .require_success("Citirea commit-ului marker de integrare")?;
            if output.stdout_truncated {
                return Err(format!(
                    "Marker-ul de integrare {marker_commit_oid} depășește limita sigură."
                ));
            }
            let commit = output.stdout_text()?;
            let (headers, message) = commit.split_once("\n\n").ok_or_else(|| {
                format!("Marker-ul de integrare {marker_commit_oid} nu are mesaj valid.")
            })?;
            let (tree_oid, parents) = parse_commit_headers(headers)?;
            if parents.is_empty() || parents.len() > 2 {
                return Err(format!(
                    "Marker-ul de integrare {marker_commit_oid} are un număr invalid de părinți."
                ));
            }
            let previous_head_oid = parents[0].clone();
            let recorded_transaction = unique_trailer(
                message,
                "Pana-Studio-Integration-Transaction: ",
                marker_commit_oid,
            )?;
            if recorded_transaction != transaction_id {
                return Err(format!(
                    "Marker-ul {recovery_ref} nu corespunde tranzacției declarate."
                ));
            }
            let kind = parse_integration_kind(&unique_trailer(
                message,
                "Pana-Studio-Integration-Kind: ",
                marker_commit_oid,
            )?)?;
            let full_head_ref = unique_trailer(
                message,
                "Pana-Studio-Integration-Head-Ref: ",
                marker_commit_oid,
            )?;
            if !full_head_ref.starts_with("refs/heads/") {
                return Err("Marker-ul de integrare declară un HEAD ref invalid.".to_string());
            }
            let target_ref = unique_trailer(
                message,
                "Pana-Studio-Integration-Target-Ref: ",
                marker_commit_oid,
            )?;
            let target_ref_check = self.runner.run(["check-ref-format", &target_ref])?;
            if !target_ref_check.success()
                || !(target_ref.starts_with("refs/heads/")
                    || target_ref.starts_with("refs/remotes/"))
            {
                return Err("Marker-ul de integrare declară o țintă ref invalidă.".to_string());
            }
            let recorded_target_oid = unique_trailer(
                message,
                "Pana-Studio-Integration-Target-Oid: ",
                marker_commit_oid,
            )?;
            validate_oid(&recorded_target_oid)?;
            if recorded_target_oid == previous_head_oid {
                if parents.iter().any(|parent| parent != &recorded_target_oid) {
                    return Err(
                        "Marker-ul schimbării între branch-uri echivalente are părinți inconsistenți."
                            .to_string(),
                    );
                }
            } else if parents.len() != 2 || parents[1] != recorded_target_oid {
                return Err("Marker-ul de integrare are target OID inconsistent.".to_string());
            }
            let target_oid = recorded_target_oid;
            let target_tree_oid =
                unique_trailer(message, "Pana-Studio-Integration-Tree: ", marker_commit_oid)?;
            validate_oid(&target_tree_oid)?;
            if target_tree_oid != tree_oid {
                return Err("Marker-ul de integrare are tree OID inconsistent.".to_string());
            }
            let previous_tree_oid = self.tree_oid(&previous_head_oid)?;
            let target_branch = optional_unique_trailer(
                message,
                "Pana-Studio-Integration-Target-Branch: ",
                marker_commit_oid,
            )?;
            if let Some(branch) = target_branch.as_deref() {
                self.validate_branch_name(branch)?;
            }
            let encoded_message = unique_trailer(
                message,
                "Pana-Studio-Integration-Message-Hex: ",
                marker_commit_oid,
            )?;
            let desired_message = decode_hex_utf8(&encoded_message)?;
            let conflict_paths = message
                .lines()
                .filter_map(|line| line.strip_prefix("Pana-Studio-Integration-Conflict-Hex: "))
                .map(decode_hex_utf8)
                .map(|path| path.and_then(|path| validate_path(&path)))
                .collect::<Result<Vec<_>, _>>()?;
            if conflict_paths.len() > MAX_INTEGRATION_CONFLICTS {
                return Err("Marker-ul declară prea multe conflicte.".to_string());
            }
            let result_commit_oid = match kind {
                VersionIntegrationKind::FastForward | VersionIntegrationKind::SwitchBranch => {
                    Some(target_oid.clone())
                }
                VersionIntegrationKind::MergeClean | VersionIntegrationKind::MergeResolved => {
                    Some(marker_commit_oid.to_string())
                }
                VersionIntegrationKind::MergeConflict => None,
            };
            markers.push(PreparedVersionIntegration {
                transaction_id: transaction_id.to_string(),
                recovery_ref: recovery_ref.to_string(),
                kind,
                previous_head_oid,
                previous_tree_oid,
                full_head_ref,
                target_ref,
                target_oid,
                target_tree_oid,
                marker_commit_oid: marker_commit_oid.to_string(),
                result_commit_oid,
                target_branch,
                conflict_paths,
                message: desired_message,
            });
        }
        if markers.len() > MAX_INTEGRATION_MARKERS {
            return Err(format!(
                "Există {} markere de integrare, peste limita sigură de {MAX_INTEGRATION_MARKERS}.",
                markers.len()
            ));
        }
        markers.sort_by(|left, right| left.recovery_ref.cmp(&right.recovery_ref));
        Ok(markers)
    }

    pub(crate) fn integration_tree(
        &self,
        prepared: &PreparedVersionIntegration,
    ) -> Result<VersionTree, String> {
        self.read_tree(&prepared.marker_commit_oid)
    }

    pub(crate) fn previous_integration_tree(
        &self,
        prepared: &PreparedVersionIntegration,
    ) -> Result<VersionTree, String> {
        self.read_tree(&prepared.previous_head_oid)
    }

    pub(crate) fn finalize_integration(
        &self,
        prepared: &PreparedVersionIntegration,
    ) -> Result<VersioningSnapshot, String> {
        let marker_oid = self.resolve_ref_oid(&prepared.recovery_ref)?;
        if marker_oid != prepared.marker_commit_oid {
            return Err("Marker-ul integrării s-a schimbat neașteptat.".to_string());
        }
        let live_head = self.resolve_commit_oid("HEAD").map_err(|error| {
            format!("HEAD nu poate fi citit la finalizarea integrării: {error}")
        })?;
        match prepared.kind {
            VersionIntegrationKind::MergeConflict => {
                return Err(
                    "Integrarea cu conflicte trebuie continuată după rezolvarea fișierelor."
                        .to_string(),
                )
            }
            VersionIntegrationKind::SwitchBranch => {
                let target_branch = prepared.target_branch.as_deref().ok_or_else(|| {
                    "Marker-ul schimbării de branch nu declară branch-ul țintă.".to_string()
                })?;
                let target_full_ref = format!("refs/heads/{target_branch}");
                let target_oid = self.resolve_ref_oid(&target_full_ref)?;
                if target_oid != prepared.target_oid {
                    return Err(
                        "Branch-ul țintă s-a schimbat în timpul tranzacției; marker-ul a fost păstrat."
                            .to_string(),
                    );
                }
                let live_full_ref = self.current_full_head_ref()?;
                if live_full_ref == prepared.full_head_ref
                    && live_head == prepared.previous_head_oid
                {
                    self.runner
                        .run(["read-tree", &prepared.target_tree_oid])?
                        .require_success("Alinierea indexului pentru schimbarea branch-ului")?;
                    self.runner
                        .run(["symbolic-ref", "HEAD", &target_full_ref])?
                        .require_success("Schimbarea referinței HEAD")?;
                } else if live_full_ref != target_full_ref || live_head != prepared.target_oid {
                    return Err(
                        "HEAD a divergat în timpul schimbării branch-ului; marker-ul a fost păstrat."
                            .to_string(),
                    );
                }
            }
            _ => {
                let result_oid = prepared.result_commit_oid.as_deref().ok_or_else(|| {
                    "Marker-ul integrării nu declară commit-ul rezultat.".to_string()
                })?;
                let live_full_ref = self.current_full_head_ref()?;
                if live_full_ref != prepared.full_head_ref {
                    return Err(
                        "Branch-ul activ s-a schimbat în timpul integrării; marker-ul a fost păstrat."
                            .to_string(),
                    );
                }
                self.runner
                    .run(["read-tree", &prepared.target_tree_oid])?
                    .require_success("Alinierea indexului la integrarea Git")?;
                if live_head == prepared.previous_head_oid {
                    self.runner
                        .run([
                            "update-ref",
                            &prepared.full_head_ref,
                            result_oid,
                            &prepared.previous_head_oid,
                        ])?
                        .require_success("Publicarea integrării Git")?;
                } else if live_head != result_oid {
                    return Err(
                        "HEAD a divergat în timpul integrării; marker-ul a fost păstrat."
                            .to_string(),
                    );
                }
            }
        }
        self.delete_integration_marker(prepared)?;
        self.snapshot()
    }

    pub(crate) fn promote_conflict_resolution(
        &self,
        prepared: &PreparedVersionIntegration,
    ) -> Result<PreparedVersionIntegration, String> {
        if prepared.kind != VersionIntegrationKind::MergeConflict {
            return Err("Numai un merge cu conflicte poate fi continuat.".to_string());
        }
        let snapshot = self.snapshot()?;
        require_ready(&snapshot)?;
        if snapshot.head_oid.as_deref() != Some(prepared.previous_head_oid.as_str())
            || snapshot.branch.as_deref() != prepared.full_head_ref.strip_prefix("refs/heads/")
        {
            return Err("HEAD nu mai corespunde începutului merge-ului.".to_string());
        }
        self.runner
            .run(["add", "-A", "--", "."])?
            .require_success("Pregătirea rezoluției merge")?;
        let resolved_tree_oid = self
            .runner
            .run(["write-tree"])?
            .require_success("Construirea arborelui rezolvat")?
            .stdout_text()?
            .trim()
            .to_string();
        validate_oid(&resolved_tree_oid)?;
        let marker_message = integration_commit_message(
            &prepared.message,
            &prepared.transaction_id,
            VersionIntegrationKind::MergeResolved,
            &prepared.full_head_ref,
            &prepared.target_ref,
            &prepared.target_oid,
            &resolved_tree_oid,
            None,
            &[],
        )?;
        let final_oid = self
            .runner
            .run_with_input(
                [
                    "commit-tree",
                    &resolved_tree_oid,
                    "-p",
                    &prepared.previous_head_oid,
                    "-p",
                    &prepared.target_oid,
                ],
                marker_message.as_bytes(),
            )?
            .require_success("Crearea commit-ului merge rezolvat")?
            .stdout_text()?
            .trim()
            .to_string();
        validate_oid(&final_oid)?;
        let resolved_tree = self.read_tree(&final_oid)?;
        reject_external_driver_attributes(&resolved_tree)?;
        self.runner
            .run([
                "update-ref",
                &prepared.recovery_ref,
                &final_oid,
                &prepared.marker_commit_oid,
            ])?
            .require_success("Promovarea marker-ului merge rezolvat")?;
        Ok(PreparedVersionIntegration {
            kind: VersionIntegrationKind::MergeResolved,
            target_tree_oid: resolved_tree_oid,
            marker_commit_oid: final_oid.clone(),
            result_commit_oid: Some(final_oid),
            conflict_paths: Vec::new(),
            ..prepared.clone()
        })
    }

    pub(crate) fn abort_integration_metadata(
        &self,
        prepared: &PreparedVersionIntegration,
    ) -> Result<VersioningSnapshot, String> {
        let live_head = self.resolve_commit_oid("HEAD")?;
        if live_head != prepared.previous_head_oid {
            return Err(
                "Rollback-ul integrării cere HEAD-ul inițial; marker-ul a fost păstrat."
                    .to_string(),
            );
        }
        let live_ref = self.current_full_head_ref()?;
        if live_ref != prepared.full_head_ref {
            return Err(
                "Rollback-ul integrării cere branch-ul inițial; marker-ul a fost păstrat."
                    .to_string(),
            );
        }
        self.runner
            .run(["read-tree", &prepared.previous_tree_oid])?
            .require_success("Restaurarea indexului anterior integrării")?;
        self.delete_integration_marker(prepared)?;
        self.snapshot()
    }

    pub(crate) fn delete_integration_marker(
        &self,
        prepared: &PreparedVersionIntegration,
    ) -> Result<(), String> {
        self.runner
            .run([
                "update-ref",
                "-d",
                &prepared.recovery_ref,
                &prepared.marker_commit_oid,
            ])?
            .require_success("Eliminarea marker-ului de integrare")?;
        Ok(())
    }

    fn require_clean_integration_start(&self, snapshot: &VersioningSnapshot) -> Result<(), String> {
        if snapshot.detached_head {
            return Err("Integrarea este blocată pe detached HEAD.".to_string());
        }
        if !snapshot.clean {
            return Err(
                "Integrarea cere un repository complet curat, fără staged, unstaged sau untracked."
                    .to_string(),
            );
        }
        if !self.read_integration_markers()?.is_empty() {
            return Err(
                "Există deja o integrare activă sau întreruptă; rezolvă Recovery înainte de alta."
                    .to_string(),
            );
        }
        Ok(())
    }

    pub(super) fn resolve_integration_target(
        &self,
        snapshot: &VersioningSnapshot,
        target_ref: &str,
        expected_target_oid: &str,
    ) -> Result<(String, String), String> {
        let target_ref = target_ref.trim();
        let allowed = snapshot
            .branches
            .iter()
            .any(|branch| format!("refs/heads/{}", branch.name) == target_ref)
            || snapshot
                .remote_branches
                .iter()
                .any(|branch| branch.ref_name == target_ref);
        if !allowed {
            return Err(
                "Ținta integrării nu este un branch local sau remote-tracking inventariat."
                    .to_string(),
            );
        }
        let live_oid = self.resolve_ref_oid(target_ref)?;
        let expected_oid = self.resolve_commit_oid(expected_target_oid)?;
        if live_oid != expected_oid {
            return Err(
                "Ținta integrării s-a schimbat; actualizează starea înainte de operație."
                    .to_string(),
            );
        }
        Ok((target_ref.to_string(), live_oid))
    }

    fn integration_relationship(
        &self,
        head_oid: &str,
        target_oid: &str,
    ) -> Result<VersionIntegrationRelationship, String> {
        if head_oid == target_oid {
            return Ok(VersionIntegrationRelationship::Same);
        }
        if self.is_ancestor(head_oid, target_oid)? {
            return Ok(VersionIntegrationRelationship::FastForward);
        }
        if self.is_ancestor(target_oid, head_oid)? {
            return Ok(VersionIntegrationRelationship::LocalAhead);
        }
        Ok(VersionIntegrationRelationship::Diverged)
    }

    fn is_ancestor(&self, ancestor: &str, descendant: &str) -> Result<bool, String> {
        let output = self
            .runner
            .run(["merge-base", "--is-ancestor", ancestor, descendant])?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(format!(
                "Git nu a putut calcula relația dintre versiuni: {}",
                output.stderr_lossy()
            )),
        }
    }

    fn merge_tree(&self, ours: &str, theirs: &str) -> Result<MergeTreeResult, String> {
        let output = self.runner.run([
            "merge-tree",
            "--write-tree",
            "--name-only",
            "-z",
            "--no-messages",
            ours,
            theirs,
        ])?;
        if !matches!(output.status.code(), Some(0 | 1)) {
            return Err(format!(
                "Calcularea merge-ului a eșuat: {}",
                output.stderr_lossy()
            ));
        }
        if output.stdout_truncated {
            return Err("Rezultatul merge-tree a fost trunchiat.".to_string());
        }
        parse_merge_tree_output(&output.stdout)
    }

    fn current_full_head_ref(&self) -> Result<String, String> {
        let reference = self
            .runner
            .run(["symbolic-ref", "--quiet", "HEAD"])?
            .require_success("Citirea branch-ului activ")?
            .stdout_text()?
            .trim()
            .to_string();
        if !reference.starts_with("refs/heads/") {
            return Err("HEAD nu indică un branch local suportat.".to_string());
        }
        Ok(reference)
    }

    fn tree_oid(&self, commit_oid: &str) -> Result<String, String> {
        let oid = self
            .runner
            .run(["rev-parse", "--verify", &format!("{commit_oid}^{{tree}}")])?
            .require_success("Citirea arborelui commit-ului Git")?
            .stdout_text()?
            .trim()
            .to_string();
        validate_oid(&oid)?;
        Ok(oid)
    }
}

fn parse_merge_tree_output(bytes: &[u8]) -> Result<MergeTreeResult, String> {
    let mut fields = bytes
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty());
    let first = fields
        .next()
        .ok_or_else(|| "merge-tree nu a returnat arborele rezultat.".to_string())?;
    let tree_oid = std::str::from_utf8(first)
        .map_err(|_| "merge-tree a returnat un OID non-UTF-8.".to_string())?
        .trim()
        .to_string();
    validate_oid(&tree_oid)?;
    let mut conflict_paths = fields
        .map(|field| {
            let path = std::str::from_utf8(field)
                .map_err(|_| "merge-tree a returnat un path non-UTF-8.".to_string())?;
            validate_path(path.trim_matches('\n'))
        })
        .collect::<Result<Vec<_>, _>>()?;
    conflict_paths.sort();
    conflict_paths.dedup();
    if conflict_paths.len() > MAX_INTEGRATION_CONFLICTS {
        return Err("Merge-ul produce prea multe conflicte.".to_string());
    }
    Ok(MergeTreeResult {
        tree_oid,
        conflict_paths,
    })
}

#[allow(clippy::too_many_arguments)]
fn integration_commit_message(
    message: &str,
    transaction_id: &str,
    kind: VersionIntegrationKind,
    full_head_ref: &str,
    target_ref: &str,
    target_oid: &str,
    target_tree_oid: &str,
    target_branch: Option<&str>,
    conflict_paths: &[String],
) -> Result<String, String> {
    let message = validate_commit_message(message)?;
    if message
        .lines()
        .any(|line| line.starts_with("Pana-Studio-Integration-"))
    {
        return Err(
            "Mesajul merge-ului folosește un prefix intern rezervat Pană Studio.".to_string(),
        );
    }
    let mut result = format!(
        "{message}\n\nPana-Studio-Integration-Transaction: {transaction_id}\nPana-Studio-Integration-Kind: {}\nPana-Studio-Integration-Head-Ref: {full_head_ref}\nPana-Studio-Integration-Target-Ref: {target_ref}\nPana-Studio-Integration-Target-Oid: {target_oid}\nPana-Studio-Integration-Tree: {target_tree_oid}\nPana-Studio-Integration-Message-Hex: {}\n",
        integration_kind_name(kind),
        encode_hex(message.as_bytes()),
    );
    if let Some(branch) = target_branch {
        result.push_str(&format!(
            "Pana-Studio-Integration-Target-Branch: {branch}\n"
        ));
    }
    for path in conflict_paths {
        validate_path(path)?;
        result.push_str(&format!(
            "Pana-Studio-Integration-Conflict-Hex: {}\n",
            encode_hex(path.as_bytes())
        ));
    }
    if result.len() > MAX_MARKER_COMMIT_BYTES {
        return Err("Marker-ul de integrare depășește limita sigură.".to_string());
    }
    Ok(result)
}

fn parse_commit_headers(headers: &str) -> Result<(String, Vec<String>), String> {
    let mut tree = None;
    let mut parents = Vec::new();
    for line in headers.lines() {
        if let Some(value) = line.strip_prefix("tree ") {
            validate_oid(value)?;
            tree = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("parent ") {
            validate_oid(value)?;
            parents.push(value.to_string());
        }
    }
    Ok((
        tree.ok_or_else(|| "Commit-ul marker nu declară tree.".to_string())?,
        parents,
    ))
}

fn unique_trailer(message: &str, prefix: &str, marker_oid: &str) -> Result<String, String> {
    let values = message
        .lines()
        .filter_map(|line| line.strip_prefix(prefix))
        .collect::<Vec<_>>();
    if values.len() != 1 || values[0].is_empty() {
        return Err(format!(
            "Marker-ul {marker_oid} trebuie să declare exact o dată trailer-ul {prefix}."
        ));
    }
    Ok(values[0].to_string())
}

fn optional_unique_trailer(
    message: &str,
    prefix: &str,
    marker_oid: &str,
) -> Result<Option<String>, String> {
    let values = message
        .lines()
        .filter_map(|line| line.strip_prefix(prefix))
        .collect::<Vec<_>>();
    if values.len() > 1 || values.first().is_some_and(|value| value.is_empty()) {
        return Err(format!(
            "Marker-ul {marker_oid} declară invalid trailer-ul {prefix}."
        ));
    }
    Ok(values.first().map(|value| value.to_string()))
}

fn integration_kind_name(kind: VersionIntegrationKind) -> &'static str {
    match kind {
        VersionIntegrationKind::FastForward => "fast_forward",
        VersionIntegrationKind::MergeClean => "merge_clean",
        VersionIntegrationKind::MergeConflict => "merge_conflict",
        VersionIntegrationKind::MergeResolved => "merge_resolved",
        VersionIntegrationKind::SwitchBranch => "switch_branch",
    }
}

fn parse_integration_kind(value: &str) -> Result<VersionIntegrationKind, String> {
    match value {
        "fast_forward" => Ok(VersionIntegrationKind::FastForward),
        "merge_clean" => Ok(VersionIntegrationKind::MergeClean),
        "merge_conflict" => Ok(VersionIntegrationKind::MergeConflict),
        "merge_resolved" => Ok(VersionIntegrationKind::MergeResolved),
        "switch_branch" => Ok(VersionIntegrationKind::SwitchBranch),
        _ => Err(format!("Tip de integrare necunoscut: {value}.")),
    }
}

fn integration_transaction_id() -> String {
    let sequence = INTEGRATION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("integration-{timestamp}-{}-{sequence}", std::process::id())
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn decode_hex_utf8(value: &str) -> Result<String, String> {
    if value.len() % 2 != 0 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Trailer-ul hex al integrării este invalid.".to_string());
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let digit = |byte: u8| match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => 0,
        };
        bytes.push((digit(pair[0]) << 4) | digit(pair[1]));
    }
    String::from_utf8(bytes).map_err(|_| "Trailer-ul hex nu conține UTF-8 valid.".to_string())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "pana-versioning-integration-{label}-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn repository(root: &Path) -> VersionRepository {
        VersionRepository::new(
            root.to_string_lossy().to_string(),
            root.to_path_buf(),
            root.to_path_buf(),
        )
    }

    fn initialized_repository(label: &str) -> (TestDirectory, VersionRepository) {
        let directory = TestDirectory::new(label);
        let repository = repository(&directory.0);
        repository.initialize().unwrap();
        repository
            .configure_identity("Pană Test", "pana@example.test")
            .unwrap();
        (directory, repository)
    }

    fn commit_file(
        repository: &VersionRepository,
        root: &Path,
        path: &str,
        contents: &str,
        message: &str,
    ) -> String {
        fs::write(root.join(path), contents).unwrap();
        let snapshot = repository.stage_all().unwrap();
        repository
            .commit(message, snapshot.head_oid.as_deref())
            .unwrap()
            .commit_oid
    }

    fn create_branch(repository: &VersionRepository, name: &str, oid: &str) {
        repository
            .create_branch(&super::super::VersionBranchInput {
                name: name.to_string(),
                start_oid: Some(oid.to_string()),
            })
            .unwrap();
    }

    fn force_test_switch(repository: &VersionRepository, branch: &str) {
        let full_ref = format!("refs/heads/{branch}");
        repository
            .runner
            .run(["read-tree", "--reset", "-u", &full_ref])
            .unwrap()
            .require_success("test read-tree switch")
            .unwrap();
        repository
            .runner
            .run(["symbolic-ref", "HEAD", &full_ref])
            .unwrap()
            .require_success("test symbolic-ref switch")
            .unwrap();
    }

    fn materialize_marker_tree(
        repository: &VersionRepository,
        prepared: &PreparedVersionIntegration,
    ) {
        repository
            .runner
            .run(["read-tree", "--reset", "-u", &prepared.marker_commit_oid])
            .unwrap()
            .require_success("test materialize integration tree")
            .unwrap();
    }

    #[test]
    fn hex_roundtrip_preserves_paths_and_messages() {
        let source = "templates/pagină conflict.html\nmesaj";
        assert_eq!(
            decode_hex_utf8(&encode_hex(source.as_bytes())).unwrap(),
            source
        );
    }

    #[test]
    fn parses_clean_merge_tree_output() {
        let oid = "a".repeat(40);
        let result = parse_merge_tree_output(format!("{oid}\0").as_bytes()).unwrap();
        assert_eq!(result.tree_oid, oid);
        assert!(result.conflict_paths.is_empty());
    }

    #[test]
    fn parses_nul_delimited_conflict_paths() {
        let oid = "b".repeat(40);
        let source = format!("{oid}\0templates/a b.html\0content/x.md\0");
        let result = parse_merge_tree_output(source.as_bytes()).unwrap();
        assert_eq!(
            result.conflict_paths,
            vec!["content/x.md", "templates/a b.html"]
        );
    }

    #[test]
    fn fast_forward_is_prepared_durably_and_published_with_cas() {
        let (directory, repository) = initialized_repository("fast-forward");
        let base = commit_file(&repository, &directory.0, "index.html", "base\n", "Base");
        create_branch(&repository, "remote-main", &base);
        force_test_switch(&repository, "remote-main");
        let target = commit_file(
            &repository,
            &directory.0,
            "index.html",
            "remote\n",
            "Remote",
        );
        force_test_switch(&repository, "main");

        let plan = repository
            .integration_plan(&VersionIntegrationTargetInput {
                target_ref: "refs/heads/remote-main".to_string(),
                expected_target_oid: target.clone(),
            })
            .unwrap();
        assert_eq!(
            plan.relationship,
            VersionIntegrationRelationship::FastForward
        );
        assert_eq!((plan.ahead, plan.behind), (0, 1));
        assert!(plan.local_only.is_empty());
        assert_eq!(plan.target_only.len(), 1);
        let preview = repository
            .diff(&super::super::VersionDiffInput {
                kind: super::super::VersionDiffKind::Integration,
                path: None,
                commit_oid: None,
                target_ref: Some(plan.target_ref.clone()),
                expected_target_oid: Some(target.clone()),
            })
            .unwrap();
        assert!(preview.patch.contains("remote"), "{}", preview.patch);
        let prepared = repository
            .prepare_integration(&VersionIntegrationInput {
                target_ref: plan.target_ref,
                expected_target_oid: target.clone(),
                mode: VersionIntegrationMode::FastForward,
                message: "Actualizare fast-forward".to_string(),
            })
            .unwrap();
        assert_eq!(prepared.kind, VersionIntegrationKind::FastForward);
        assert_eq!(repository.read_integration_markers().unwrap().len(), 1);
        materialize_marker_tree(&repository, &prepared);
        let snapshot = repository.finalize_integration(&prepared).unwrap();
        assert_eq!(snapshot.branch.as_deref(), Some("main"));
        assert_eq!(snapshot.head_oid.as_deref(), Some(target.as_str()));
        assert!(snapshot.clean);
        assert!(repository.read_integration_markers().unwrap().is_empty());
    }

    #[test]
    fn interrupted_fast_forward_is_reloaded_from_its_durable_marker() {
        let (directory, repository) = initialized_repository("fast-forward-recovery");
        let base = commit_file(&repository, &directory.0, "index.html", "base\n", "Base");
        create_branch(&repository, "remote-main", &base);
        force_test_switch(&repository, "remote-main");
        let target = commit_file(
            &repository,
            &directory.0,
            "index.html",
            "remote\n",
            "Remote",
        );
        force_test_switch(&repository, "main");
        let prepared = repository
            .prepare_integration(&VersionIntegrationInput {
                target_ref: "refs/heads/remote-main".to_string(),
                expected_target_oid: target.clone(),
                mode: VersionIntegrationMode::FastForward,
                message: "Fast-forward recuperabil".to_string(),
            })
            .unwrap();
        materialize_marker_tree(&repository, &prepared);

        let reopened = VersionRepository::new(
            directory.0.to_string_lossy().to_string(),
            directory.0.clone(),
            directory.0.clone(),
        );
        let markers = reopened.read_integration_markers().unwrap();
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].transaction_id, prepared.transaction_id);
        assert_eq!(markers[0].target_oid, target);
        let snapshot = reopened.finalize_integration(&markers[0]).unwrap();
        assert_eq!(
            snapshot.head_oid.as_deref(),
            Some(markers[0].target_oid.as_str())
        );
        assert!(reopened.read_integration_markers().unwrap().is_empty());
    }

    #[test]
    fn divergent_head_blocks_finalization_and_preserves_recovery_marker() {
        let (directory, repository) = initialized_repository("divergent-recovery");
        let base = commit_file(&repository, &directory.0, "base.txt", "base\n", "Base");
        create_branch(&repository, "remote-main", &base);
        force_test_switch(&repository, "remote-main");
        let target = commit_file(
            &repository,
            &directory.0,
            "remote.txt",
            "remote\n",
            "Remote",
        );
        force_test_switch(&repository, "main");
        create_branch(&repository, "other", &base);
        force_test_switch(&repository, "other");
        let other = commit_file(&repository, &directory.0, "other.txt", "other\n", "Other");
        force_test_switch(&repository, "main");
        let prepared = repository
            .prepare_integration(&VersionIntegrationInput {
                target_ref: "refs/heads/remote-main".to_string(),
                expected_target_oid: target,
                mode: VersionIntegrationMode::FastForward,
                message: "Fast-forward întrerupt".to_string(),
            })
            .unwrap();
        materialize_marker_tree(&repository, &prepared);
        repository
            .runner
            .run(["update-ref", "refs/heads/main", &other, &base])
            .unwrap()
            .require_success("test divergent HEAD")
            .unwrap();

        let error = repository.finalize_integration(&prepared).unwrap_err();
        assert!(error.contains("HEAD a divergat"), "{error}");
        let markers = repository.read_integration_markers().unwrap();
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].transaction_id, prepared.transaction_id);
    }

    #[test]
    fn clean_merge_publishes_a_two_parent_commit() {
        let (directory, repository) = initialized_repository("clean-merge");
        fs::write(directory.0.join("base.txt"), "base\n").unwrap();
        repository.stage_all().unwrap();
        let base = repository.commit("Base", None).unwrap().commit_oid;
        create_branch(&repository, "remote-main", &base);
        let ours = commit_file(&repository, &directory.0, "local.txt", "local\n", "Local");
        force_test_switch(&repository, "remote-main");
        let theirs = commit_file(
            &repository,
            &directory.0,
            "remote.txt",
            "remote\n",
            "Remote",
        );
        force_test_switch(&repository, "main");
        assert_eq!(
            repository.snapshot().unwrap().head_oid.as_deref(),
            Some(ours.as_str())
        );

        let plan = repository
            .integration_plan(&VersionIntegrationTargetInput {
                target_ref: "refs/heads/remote-main".to_string(),
                expected_target_oid: theirs.clone(),
            })
            .unwrap();
        assert_eq!(plan.relationship, VersionIntegrationRelationship::Diverged);
        assert_eq!((plan.ahead, plan.behind), (1, 1));
        assert_eq!(plan.local_only.len(), 1);
        assert_eq!(plan.target_only.len(), 1);

        let prepared = repository
            .prepare_integration(&VersionIntegrationInput {
                target_ref: "refs/heads/remote-main".to_string(),
                expected_target_oid: theirs.clone(),
                mode: VersionIntegrationMode::Merge,
                message: "Integrare remote-main".to_string(),
            })
            .unwrap();
        assert_eq!(prepared.kind, VersionIntegrationKind::MergeClean);
        materialize_marker_tree(&repository, &prepared);
        let snapshot = repository.finalize_integration(&prepared).unwrap();
        assert_eq!(snapshot.head_oid, prepared.result_commit_oid);
        let history = repository.history(0, 1).unwrap();
        assert_eq!(history.entries[0].parent_oids, vec![ours, theirs]);
        assert!(snapshot.clean);
    }

    #[test]
    fn conflicted_merge_requires_resolution_then_publishes_two_parents() {
        let (directory, repository) = initialized_repository("conflict-merge");
        let base = commit_file(&repository, &directory.0, "index.html", "base\n", "Base");
        create_branch(&repository, "remote-main", &base);
        let ours = commit_file(&repository, &directory.0, "index.html", "local\n", "Local");
        force_test_switch(&repository, "remote-main");
        let theirs = commit_file(
            &repository,
            &directory.0,
            "index.html",
            "remote\n",
            "Remote",
        );
        force_test_switch(&repository, "main");

        let prepared = repository
            .prepare_integration(&VersionIntegrationInput {
                target_ref: "refs/heads/remote-main".to_string(),
                expected_target_oid: theirs.clone(),
                mode: VersionIntegrationMode::Merge,
                message: "Rezolvare remote-main".to_string(),
            })
            .unwrap();
        assert_eq!(prepared.kind, VersionIntegrationKind::MergeConflict);
        assert_eq!(prepared.conflict_paths, vec!["index.html"]);
        materialize_marker_tree(&repository, &prepared);
        let conflicted = fs::read_to_string(directory.0.join("index.html")).unwrap();
        assert!(conflicted.contains("<<<<<<<"), "{conflicted}");
        fs::write(directory.0.join("index.html"), "rezolvat\n").unwrap();
        let resolved = repository.promote_conflict_resolution(&prepared).unwrap();
        assert_eq!(resolved.kind, VersionIntegrationKind::MergeResolved);
        let snapshot = repository.finalize_integration(&resolved).unwrap();
        assert!(snapshot.clean);
        assert_eq!(
            fs::read_to_string(directory.0.join("index.html")).unwrap(),
            "rezolvat\n"
        );
        let history = repository.history(0, 1).unwrap();
        assert_eq!(history.entries[0].parent_oids, vec![ours, theirs]);
    }

    #[test]
    fn branch_switch_changes_symbolic_head_only_after_tree_is_materialized() {
        let (directory, repository) = initialized_repository("switch");
        let main = commit_file(&repository, &directory.0, "index.html", "main\n", "Main");
        create_branch(&repository, "feature", &main);
        force_test_switch(&repository, "feature");
        let feature = commit_file(
            &repository,
            &directory.0,
            "index.html",
            "feature\n",
            "Feature",
        );
        force_test_switch(&repository, "main");
        let prepared = repository
            .prepare_branch_switch(&VersionSwitchBranchInput {
                branch: "feature".to_string(),
                expected_target_oid: feature.clone(),
            })
            .unwrap();
        materialize_marker_tree(&repository, &prepared);
        assert_eq!(
            repository.snapshot().unwrap().branch.as_deref(),
            Some("main")
        );
        let snapshot = repository.finalize_integration(&prepared).unwrap();
        assert_eq!(snapshot.branch.as_deref(), Some("feature"));
        assert_eq!(snapshot.head_oid.as_deref(), Some(feature.as_str()));
        assert!(snapshot.clean);
    }

    #[test]
    fn branch_switch_supports_two_branches_at_the_same_commit() {
        let (directory, repository) = initialized_repository("switch-same-oid");
        let main = commit_file(&repository, &directory.0, "index.html", "same\n", "Main");
        create_branch(&repository, "feature", &main);

        let same_plan = repository
            .integration_plan(&VersionIntegrationTargetInput {
                target_ref: "refs/heads/feature".to_string(),
                expected_target_oid: main.clone(),
            })
            .unwrap();
        assert_eq!(same_plan.relationship, VersionIntegrationRelationship::Same);
        assert_eq!((same_plan.ahead, same_plan.behind), (0, 0));

        let prepared = repository
            .prepare_branch_switch(&VersionSwitchBranchInput {
                branch: "feature".to_string(),
                expected_target_oid: main.clone(),
            })
            .unwrap();
        materialize_marker_tree(&repository, &prepared);
        let snapshot = repository.finalize_integration(&prepared).unwrap();
        assert_eq!(snapshot.branch.as_deref(), Some("feature"));
        assert_eq!(snapshot.head_oid.as_deref(), Some(main.as_str()));
        assert!(snapshot.clean);
        assert!(repository.read_integration_markers().unwrap().is_empty());

        force_test_switch(&repository, "main");
        let advanced = commit_file(
            &repository,
            &directory.0,
            "index.html",
            "main advanced\n",
            "Main advanced",
        );
        let local_ahead = repository
            .integration_plan(&VersionIntegrationTargetInput {
                target_ref: "refs/heads/feature".to_string(),
                expected_target_oid: main,
            })
            .unwrap();
        assert_eq!(
            local_ahead.relationship,
            VersionIntegrationRelationship::LocalAhead
        );
        assert_eq!((local_ahead.ahead, local_ahead.behind), (1, 0));
        assert_eq!(local_ahead.head_oid, advanced);
    }
}
