use std::{
    ffi::OsString,
    sync::{atomic::AtomicBool, Arc},
};

use super::{
    git::{GitCommandOutput, ProgressCallback, NETWORK_CANCELLED_ERROR},
    repository::{parse_history, require_ready, validate_oid, zero_oid},
    VersionBranch, VersionBranchInput, VersionHistoryEntry, VersionNetworkOperationKind,
    VersionNetworkReceipt, VersionPushInput, VersionRemote, VersionRemoteBranch,
    VersionRemoteInput, VersionRepository, VersionSyncComparison, VersionSyncState,
    VersionUpstream, VersionUpstreamInput, VersioningSnapshot, VERSIONING_SCHEMA_VERSION,
};

const MAX_REMOTE_NAME_BYTES: usize = 64;
const MAX_BRANCH_NAME_BYTES: usize = 256;
const MAX_REMOTE_URL_BYTES: usize = 2_048;
const MAX_SYNC_COMPARISON_ENTRIES: usize = 50;
const REDACTED_REMOTE_URL: &str = "[URL ascunsă — reconfigurează remote-ul]";

pub(super) struct VersionRemoteSnapshotParts {
    pub remotes: Vec<VersionRemote>,
    pub branches: Vec<VersionBranch>,
    pub remote_branches: Vec<VersionRemoteBranch>,
    pub upstream: Option<VersionUpstream>,
    pub sync_state: VersionSyncState,
}

impl VersionRepository {
    pub(super) fn unsupported_partial_clone_config(&self) -> Result<Option<String>, String> {
        let keys = self.local_config_keys_matching(
            r"^(extensions\.partialclone|remote\..*\.(promisor|partialclonefilter))$",
        )?;
        Ok((!keys.is_empty()).then(|| {
            "Repository-urile partial clone/promisor nu sunt suportate: obiectele tuturor versiunilor trebuie să existe local înainte de versionare sau restaurare."
                .to_string()
        }))
    }

    pub(super) fn remote_state_fingerprint(&self) -> Result<Vec<u8>, String> {
        let refs = self
            .runner
            .run([
                "for-each-ref",
                "--format=%(refname)%00%(objectname)%00%(symref)%00",
                "refs/heads",
                "refs/remotes",
                "refs/pana-studio",
            ])?
            .require_success("Citirea referințelor Git pentru status")?;
        if refs.stdout_truncated {
            return Err("Referințele Git au depășit limita sigură de status.".to_string());
        }
        // Întregul scope local intră în token. Astfel, o modificare a unui
        // helper, include, rewrite URL sau transport invalidează identitatea
        // operației, chiar dacă acea cheie nu este afișată în snapshot.
        let config = self.runner.run(["config", "--local", "--null", "--list"])?;
        let config_bytes = if config.success() {
            if config.stdout_truncated {
                return Err("Configurația Git remote a depășit limita sigură.".to_string());
            }
            config.stdout
        } else if config.status.code() == Some(1) {
            Vec::new()
        } else {
            return Err(network_safe_error(
                "Citirea configurației Git remote",
                &config,
            ));
        };
        let mut fingerprint = Vec::with_capacity(refs.stdout.len() + config_bytes.len() + 16);
        fingerprint.extend_from_slice(&(refs.stdout.len() as u64).to_le_bytes());
        fingerprint.extend_from_slice(&refs.stdout);
        fingerprint.extend_from_slice(&(config_bytes.len() as u64).to_le_bytes());
        fingerprint.extend_from_slice(&config_bytes);
        Ok(fingerprint)
    }

    pub(super) fn remote_snapshot_parts(
        &self,
        current_branch: Option<&str>,
        head_oid: Option<&str>,
    ) -> Result<VersionRemoteSnapshotParts, String> {
        let remotes = self.read_remotes()?;
        let remote_branches = self.read_remote_branches(&remotes)?;
        let mut branches = self.read_local_branches(current_branch, head_oid)?;
        if let Some(branch_name) = current_branch {
            if !branches.iter().any(|branch| branch.name == branch_name) {
                let upstream = self.configured_upstream(branch_name, &remotes)?;
                branches.push(VersionBranch {
                    name: branch_name.to_string(),
                    oid: None,
                    current: true,
                    upstream_ref: upstream.as_ref().map(|value| value.ref_name.clone()),
                    upstream_oid: upstream.as_ref().and_then(|value| value.oid.clone()),
                    ahead: 0,
                    behind: 0,
                    sync_state: VersionSyncState::Unborn,
                });
            }
        }
        branches.sort_by(|left, right| {
            right
                .current
                .cmp(&left.current)
                .then_with(|| left.name.cmp(&right.name))
        });
        let upstream = current_branch
            .map(|branch| self.configured_upstream(branch, &remotes))
            .transpose()?
            .flatten();
        let sync_state = if head_oid.is_none() {
            VersionSyncState::Unborn
        } else {
            upstream
                .as_ref()
                .map(|value| value.sync_state)
                .unwrap_or(VersionSyncState::NoUpstream)
        };
        Ok(VersionRemoteSnapshotParts {
            remotes,
            branches,
            remote_branches,
            upstream,
            sync_state,
        })
    }

    pub(crate) fn configure_remote(
        &self,
        input: &VersionRemoteInput,
    ) -> Result<VersioningSnapshot, String> {
        require_ready(&self.snapshot()?)?;
        let name = validate_remote_name(&input.name)?;
        let fetch_url = validate_remote_url(&input.fetch_url)?;
        let push_url = input
            .push_url
            .as_deref()
            .map(validate_remote_url)
            .transpose()?;
        let url_key = format!("remote.{name}.url");
        let push_url_key = format!("remote.{name}.pushurl");
        let fetch_key = format!("remote.{name}.fetch");
        let mirror_key = format!("remote.{name}.mirror");
        let fetch_refspec = format!("+refs/heads/*:refs/remotes/{name}/*");
        self.runner
            .run(["config", "--local", "--replace-all", &url_key, &fetch_url])?
            .require_success("Salvarea URL-ului remote Git")?;
        match push_url.as_deref() {
            Some(url) => {
                self.runner
                    .run(["config", "--local", "--replace-all", &push_url_key, url])?
                    .require_success("Salvarea URL-ului de push Git")?;
            }
            None => self.unset_all_config(&push_url_key)?,
        }
        self.runner
            .run([
                "config",
                "--local",
                "--replace-all",
                &fetch_key,
                &fetch_refspec,
            ])?
            .require_success("Fixarea refspec-ului remote Git")?;
        self.unset_all_config(&mirror_key)?;
        for key in [
            "uploadpack",
            "receivepack",
            "vcs",
            "proxy",
            "promisor",
            "partialCloneFilter",
        ] {
            self.unset_all_config(&format!("remote.{name}.{key}"))?;
        }
        self.snapshot()
    }

    pub(crate) fn remove_remote(&self, remote: &str) -> Result<VersioningSnapshot, String> {
        require_ready(&self.snapshot()?)?;
        let remote = validate_remote_name(remote)?;
        self.require_remote(&remote)?;
        self.runner
            .run(["remote", "remove", &remote])?
            .require_success("Eliminarea remote-ului Git")?;
        self.snapshot()
    }

    pub(crate) fn configure_upstream(
        &self,
        input: &VersionUpstreamInput,
    ) -> Result<VersioningSnapshot, String> {
        let snapshot = self.snapshot()?;
        require_ready(&snapshot)?;
        let local_branch = self.validate_branch_name(&input.local_branch)?;
        let remote = validate_remote_name(&input.remote)?;
        let remote_branch = self.validate_branch_name(&input.remote_branch)?;
        self.require_local_branch(&local_branch)?;
        self.require_usable_remote(&remote)?;
        let tracking_ref = format!("refs/remotes/{remote}/{remote_branch}");
        self.resolve_ref_oid(&tracking_ref).map_err(|_| {
            format!(
                "Branch-ul remote {remote}/{remote_branch} nu există local. Rulează Fetch înainte de configurarea upstream-ului."
            )
        })?;
        let remote_key = format!("branch.{local_branch}.remote");
        let merge_key = format!("branch.{local_branch}.merge");
        let merge_ref = format!("refs/heads/{remote_branch}");
        self.runner
            .run(["config", "--local", "--replace-all", &remote_key, &remote])?
            .require_success("Configurarea remote-ului upstream")?;
        self.runner
            .run(["config", "--local", "--replace-all", &merge_key, &merge_ref])?
            .require_success("Configurarea branch-ului upstream")?;
        self.snapshot()
    }

    pub(crate) fn clear_upstream(&self, local_branch: &str) -> Result<VersioningSnapshot, String> {
        require_ready(&self.snapshot()?)?;
        let local_branch = self.validate_branch_name(local_branch)?;
        self.require_local_branch(&local_branch)?;
        self.unset_all_config(&format!("branch.{local_branch}.remote"))?;
        self.unset_all_config(&format!("branch.{local_branch}.merge"))?;
        self.snapshot()
    }

    pub(crate) fn create_branch(
        &self,
        input: &VersionBranchInput,
    ) -> Result<VersioningSnapshot, String> {
        let snapshot = self.snapshot()?;
        require_ready(&snapshot)?;
        let name = self.validate_branch_name(&input.name)?;
        let start_oid = match input.start_oid.as_deref() {
            Some(value) => self.resolve_commit_oid(value)?,
            None => snapshot
                .head_oid
                .clone()
                .ok_or_else(|| "Primul branch cere mai întâi un commit Git.".to_string())?,
        };
        let full_ref = format!("refs/heads/{name}");
        let zero = zero_oid(snapshot.object_format.as_deref());
        self.runner
            .run(["update-ref", &full_ref, &start_oid, &zero])?
            .require_success("Crearea branch-ului Git")?;
        self.snapshot()
    }

    pub(crate) fn delete_branch(&self, branch: &str) -> Result<VersioningSnapshot, String> {
        let snapshot = self.snapshot()?;
        require_ready(&snapshot)?;
        let branch = self.validate_branch_name(branch)?;
        if snapshot.branch.as_deref() == Some(branch.as_str()) {
            return Err("Branch-ul activ nu poate fi șters.".to_string());
        }
        let full_ref = format!("refs/heads/{branch}");
        let oid = self.resolve_ref_oid(&full_ref)?;
        let head = snapshot
            .head_oid
            .as_deref()
            .ok_or_else(|| "Ștergerea branch-ului cere un HEAD publicat.".to_string())?;
        let merged = self
            .runner
            .run(["merge-base", "--is-ancestor", &oid, head])?;
        if !merged.success() {
            if merged.status.code() == Some(1) {
                return Err(
                    "Branch-ul conține commit-uri care nu sunt integrate în HEAD și nu a fost șters."
                        .to_string(),
                );
            }
            return Err(network_safe_error(
                "Verificarea branch-ului înainte de ștergere",
                &merged,
            ));
        }
        self.runner
            .run(["update-ref", "-d", &full_ref, &oid])?
            .require_success("Ștergerea branch-ului Git")?;
        self.snapshot()
    }

    pub(crate) fn fetch_remote(
        &self,
        remote: &str,
        prune: bool,
        operation_id: &str,
        cancellation: Arc<AtomicBool>,
        progress: ProgressCallback,
    ) -> Result<VersionNetworkReceipt, String> {
        validate_operation_id(operation_id)?;
        let before = self.snapshot()?;
        require_ready(&before)?;
        let remote = validate_remote_name(remote)?;
        let configured = self.require_usable_remote(&remote)?;
        let refspec = format!("+refs/heads/*:refs/remotes/{remote}/*");
        let mut args = vec![
            OsString::from("fetch"),
            OsString::from("--atomic"),
            OsString::from("--progress"),
            OsString::from("--no-tags"),
            OsString::from("--no-recurse-submodules"),
        ];
        if prune {
            args.push(OsString::from("--prune"));
        }
        args.push(OsString::from("--"));
        args.push(OsString::from(configured.fetch_url));
        args.push(OsString::from(refspec));
        let output = self
            .runner
            .run_network(args, cancellation, progress)
            .map_err(|error| {
                classify_network_runtime_error(VersionNetworkOperationKind::Fetch, error)
            })?;
        if !output.success() {
            return Err(classify_network_output_error(
                VersionNetworkOperationKind::Fetch,
                &output,
            ));
        }
        if output.stdout_truncated || output.stderr_truncated {
            return Err(
                "Fetch a reușit posibil, dar outputul Git a depășit limita sigură. Actualizează starea înainte de a repeta operația."
                    .to_string(),
            );
        }
        let after = self.snapshot()?;
        Ok(VersionNetworkReceipt {
            schema_version: VERSIONING_SCHEMA_VERSION,
            operation_id: operation_id.to_string(),
            kind: VersionNetworkOperationKind::Fetch,
            remote,
            branch: after.branch.clone(),
            changed: before.status_token != after.status_token,
            diagnostic: None,
            snapshot: after,
        })
    }

    pub(crate) fn push_branch(
        &self,
        input: &VersionPushInput,
        cancellation: Arc<AtomicBool>,
        progress: ProgressCallback,
    ) -> Result<VersionNetworkReceipt, String> {
        validate_operation_id(&input.operation_id)?;
        let before = self.snapshot()?;
        require_ready(&before)?;
        if before.detached_head {
            return Err("Push este blocat pe detached HEAD.".to_string());
        }
        if before.head_oid.is_none() {
            return Err("Push cere cel puțin un commit local.".to_string());
        }
        let local_branch = before
            .branch
            .clone()
            .ok_or_else(|| "Push cere un branch local activ.".to_string())?;
        let remote = validate_remote_name(&input.remote)?;
        let remote_branch = self.validate_branch_name(&input.remote_branch)?;
        let configured = self.require_usable_remote(&remote)?;
        let refspec = format!("refs/heads/{local_branch}:refs/heads/{remote_branch}");
        let output = self
            .runner
            .run_network(
                [
                    "push",
                    "--porcelain",
                    "--progress",
                    "--",
                    &configured.push_url,
                    &refspec,
                ],
                cancellation,
                progress,
            )
            .map_err(|error| {
                classify_network_runtime_error(VersionNetworkOperationKind::Push, error)
            })?;
        if !output.success() {
            return Err(classify_network_output_error(
                VersionNetworkOperationKind::Push,
                &output,
            ));
        }
        if output.stdout_truncated || output.stderr_truncated {
            return Err(
                "Push a fost publicat posibil, dar outputul Git a depășit limita sigură. Nu repeta automat; execută Fetch și verifică starea remote."
                    .to_string(),
            );
        }
        // Push-ul cu URL explicit nu actualizează automat ref-ul de tracking.
        // Îl publicăm local prin CAS numai după confirmarea serverului, astfel
        // încât primul push poate seta upstream fără un fetch intermediar.
        let head_oid = before.head_oid.as_deref().ok_or_else(|| {
            "Push-ul a reușit, dar HEAD-ul local nu mai poate fi determinat.".to_string()
        })?;
        let tracking_ref = format!("refs/remotes/{remote}/{remote_branch}");
        let previous_tracking = self
            .resolve_ref_oid(&tracking_ref)
            .unwrap_or_else(|_| zero_oid(before.object_format.as_deref()));
        self.runner
            .run(["update-ref", &tracking_ref, head_oid, &previous_tracking])
            .and_then(|output| {
                output.require_success(
                    "Actualizarea ref-ului local de tracking după push; push-ul remote a reușit, nu îl repeta automat",
                )
            })
            .map_err(|error| {
                format!(
                    "Push-ul remote a reușit, dar tracking-ul local nu a putut fi actualizat. Nu repeta Push; rulează Fetch și verifică starea. {}",
                    redact_network_text(&error)
                )
            })?;
        let snapshot_result = if input.set_upstream {
            self.configure_upstream(&VersionUpstreamInput {
                local_branch: local_branch.clone(),
                remote: remote.clone(),
                remote_branch: remote_branch.clone(),
            })
        } else {
            self.snapshot()
        };
        let snapshot = snapshot_result.map_err(|error| {
            format!(
                "Push-ul remote a reușit, dar starea/upstream-ul local nu a putut fi finalizat. Nu repeta Push; rulează Fetch și verifică starea. {}",
                redact_network_text(&error)
            )
        })?;
        Ok(VersionNetworkReceipt {
            schema_version: VERSIONING_SCHEMA_VERSION,
            operation_id: input.operation_id.clone(),
            kind: VersionNetworkOperationKind::Push,
            remote,
            branch: Some(local_branch),
            changed: before.status_token != snapshot.status_token,
            diagnostic: None,
            snapshot,
        })
    }

    pub(crate) fn sync_comparison(&self) -> Result<VersionSyncComparison, String> {
        let snapshot = self.snapshot()?;
        require_ready(&snapshot)?;
        let head_oid = snapshot
            .head_oid
            .as_deref()
            .ok_or_else(|| "Compararea cere un commit local.".to_string())?;
        let upstream = snapshot
            .upstream
            .as_ref()
            .ok_or_else(|| "Branch-ul activ nu are upstream configurat.".to_string())?;
        let upstream_oid = upstream.oid.as_deref().ok_or_else(|| {
            "Referința upstream lipsește local. Rulează Fetch înainte de comparare.".to_string()
        })?;
        let local_range = format!("{upstream_oid}..{head_oid}");
        let remote_range = format!("{head_oid}..{upstream_oid}");
        Ok(VersionSyncComparison {
            schema_version: VERSIONING_SCHEMA_VERSION,
            local_ref: snapshot
                .branch
                .clone()
                .unwrap_or_else(|| "HEAD".to_string()),
            upstream_ref: upstream.ref_name.clone(),
            ahead: upstream.ahead,
            behind: upstream.behind,
            local_only: self.history_for_range(&local_range)?,
            remote_only: self.history_for_range(&remote_range)?,
        })
    }

    fn read_remotes(&self) -> Result<Vec<VersionRemote>, String> {
        let output = self
            .runner
            .run(["remote"])?
            .require_success("Citirea remote-urilor Git")?;
        if output.stdout_truncated {
            return Err("Lista remote-urilor Git a fost trunchiată.".to_string());
        }
        let source = output.stdout_text()?;
        let unsafe_transport_keys = self.local_config_keys_matching(
            r"^(credential(\..*)?\.helper|core\.(sshcommand|gitproxy|askpass)|url\..*\.(insteadof|pushinsteadof)|include(if)?\..*|http(\..*)?\.(extraheader|cookiefile|savecookies|sslverify))$",
        )?;
        let mut remotes = Vec::new();
        for raw_name in source.lines().filter(|line| !line.trim().is_empty()) {
            let name = raw_name.trim().to_string();
            let mut diagnostics = Vec::new();
            if let Err(error) = validate_remote_name(&name) {
                diagnostics.push(error);
                remotes.push(VersionRemote {
                    name,
                    fetch_url: REDACTED_REMOTE_URL.to_string(),
                    push_url: REDACTED_REMOTE_URL.to_string(),
                    usable: false,
                    diagnostic: Some(diagnostics.join(" ")),
                });
                continue;
            }
            let fetch_urls = self.remote_urls(&name, false)?;
            let push_urls = self.remote_urls(&name, true)?;
            let fetch_url = single_remote_url(&fetch_urls, "fetch", &mut diagnostics);
            let push_url = single_remote_url(&push_urls, "push", &mut diagnostics);
            let expected_refspec = format!("+refs/heads/*:refs/remotes/{name}/*");
            let refspecs = self.config_values(&format!("remote.{name}.fetch"))?;
            if refspecs != [expected_refspec] {
                diagnostics.push(
                    "Refspec-ul fetch nu este cel izolat în refs/remotes/<remote>/*; reconfigurează remote-ul."
                        .to_string(),
                );
            }
            if self
                .optional_local_config(&format!("remote.{name}.mirror"))?
                .is_some_and(|value| value.eq_ignore_ascii_case("true"))
            {
                diagnostics.push("Remote-urile mirror nu sunt suportate.".to_string());
            }
            for key in ["uploadpack", "receivepack", "vcs", "proxy"] {
                if self
                    .optional_local_config(&format!("remote.{name}.{key}"))?
                    .is_some()
                {
                    diagnostics.push(format!(
                        "Configurația remote.{name}.{key} nu este acceptată; transportul este controlat exclusiv de Pană Studio."
                    ));
                }
            }
            if !unsafe_transport_keys.is_empty() {
                diagnostics.push(format!(
                    "Configurația Git locală poate executa sau redirecționa autentificarea/transportul și trebuie eliminată înainte de operații remote: {}.",
                    unsafe_transport_keys.join(", ")
                ));
            }
            let fetch_display = safe_remote_display(fetch_url.as_deref(), &mut diagnostics);
            let push_display = safe_remote_display(push_url.as_deref(), &mut diagnostics);
            remotes.push(VersionRemote {
                name,
                fetch_url: fetch_display,
                push_url: push_display,
                usable: diagnostics.is_empty(),
                diagnostic: (!diagnostics.is_empty()).then(|| diagnostics.join(" ")),
            });
        }
        remotes.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(remotes)
    }

    fn read_local_branches(
        &self,
        current_branch: Option<&str>,
        head_oid: Option<&str>,
    ) -> Result<Vec<VersionBranch>, String> {
        let output = self
            .runner
            .run([
                "for-each-ref",
                "--format=%(refname)%00%(objectname)%00%(upstream)%00",
                "refs/heads",
            ])?
            .require_success("Citirea branch-urilor locale")?;
        let records = parse_ref_records(&output, 3, "branch-urilor locale")?;
        let remotes = self.read_remotes()?;
        records
            .into_iter()
            .map(|record| {
                let full_ref = &record[0];
                let name = full_ref
                    .strip_prefix("refs/heads/")
                    .ok_or_else(|| format!("Referință locală neașteptată: {full_ref}."))?
                    .to_string();
                let oid = record[1].clone();
                validate_oid(&oid)?;
                let configured = self.configured_upstream(&name, &remotes)?;
                let (upstream_ref, upstream_oid, ahead, behind, sync_state) = configured
                    .map(|upstream| {
                        (
                            Some(upstream.ref_name),
                            upstream.oid,
                            upstream.ahead,
                            upstream.behind,
                            upstream.sync_state,
                        )
                    })
                    .unwrap_or((None, None, 0, 0, VersionSyncState::NoUpstream));
                Ok(VersionBranch {
                    current: current_branch == Some(name.as_str())
                        && head_oid == Some(oid.as_str()),
                    name,
                    oid: Some(oid),
                    upstream_ref,
                    upstream_oid,
                    ahead,
                    behind,
                    sync_state,
                })
            })
            .collect()
    }

    fn read_remote_branches(
        &self,
        remotes: &[VersionRemote],
    ) -> Result<Vec<VersionRemoteBranch>, String> {
        let output = self
            .runner
            .run([
                "for-each-ref",
                "--format=%(refname)%00%(objectname)%00%(symref)%00",
                "refs/remotes",
            ])?
            .require_success("Citirea branch-urilor remote")?;
        let records = parse_ref_records(&output, 3, "branch-urilor remote")?;
        let mut branches = Vec::new();
        for record in records {
            if !record[2].is_empty() {
                continue;
            }
            let full_ref = record[0].clone();
            let relative = full_ref
                .strip_prefix("refs/remotes/")
                .ok_or_else(|| format!("Referință remote neașteptată: {full_ref}."))?;
            let Some(remote) = remotes
                .iter()
                .filter(|remote| relative.starts_with(&format!("{}/", remote.name)))
                .max_by_key(|remote| remote.name.len())
            else {
                continue;
            };
            let name = relative[remote.name.len() + 1..].to_string();
            if name == "HEAD" || name.is_empty() {
                continue;
            }
            let oid = record[1].clone();
            validate_oid(&oid)?;
            branches.push(VersionRemoteBranch {
                remote: remote.name.clone(),
                name,
                ref_name: full_ref,
                oid,
            });
        }
        branches.sort_by(|left, right| {
            left.remote
                .cmp(&right.remote)
                .then_with(|| left.name.cmp(&right.name))
        });
        Ok(branches)
    }

    fn configured_upstream(
        &self,
        local_branch: &str,
        remotes: &[VersionRemote],
    ) -> Result<Option<VersionUpstream>, String> {
        let remote = self.optional_local_config(&format!("branch.{local_branch}.remote"))?;
        let merge_ref = self.optional_local_config(&format!("branch.{local_branch}.merge"))?;
        let (Some(remote), Some(merge_ref)) = (remote, merge_ref) else {
            return Ok(None);
        };
        if !remotes.iter().any(|candidate| candidate.name == remote) {
            return Ok(Some(VersionUpstream {
                local_branch: local_branch.to_string(),
                remote: remote.clone(),
                remote_branch: merge_ref
                    .strip_prefix("refs/heads/")
                    .unwrap_or(&merge_ref)
                    .to_string(),
                ref_name: format!("refs/remotes/{remote}/<missing>"),
                oid: None,
                ahead: 0,
                behind: 0,
                sync_state: VersionSyncState::UpstreamMissing,
            }));
        }
        let remote_branch = merge_ref
            .strip_prefix("refs/heads/")
            .ok_or_else(|| {
                format!(
                    "Upstream-ul branch-ului {local_branch} are merge ref nesuportat: {merge_ref}."
                )
            })?
            .to_string();
        self.validate_branch_name(&remote_branch)?;
        let ref_name = format!("refs/remotes/{remote}/{remote_branch}");
        let oid = self.resolve_ref_oid(&ref_name).ok();
        let local_oid = self
            .resolve_ref_oid(&format!("refs/heads/{local_branch}"))
            .ok();
        let (ahead, behind, sync_state) = match (local_oid.as_deref(), oid.as_deref()) {
            (Some(local), Some(upstream)) => {
                let (ahead, behind) = self.ahead_behind(local, upstream)?;
                (ahead, behind, sync_state(ahead, behind))
            }
            (None, _) => (0, 0, VersionSyncState::Unborn),
            (_, None) => (0, 0, VersionSyncState::UpstreamMissing),
        };
        Ok(Some(VersionUpstream {
            local_branch: local_branch.to_string(),
            remote,
            remote_branch,
            ref_name,
            oid,
            ahead,
            behind,
            sync_state,
        }))
    }

    pub(super) fn ahead_behind(&self, local: &str, upstream: &str) -> Result<(u64, u64), String> {
        let range = format!("{local}...{upstream}");
        let output = self
            .runner
            .run(["rev-list", "--left-right", "--count", &range])?
            .require_success("Calcularea stării ahead/behind")?;
        let source = output.stdout_text()?;
        let mut values = source.split_whitespace();
        let ahead = values
            .next()
            .ok_or_else(|| "Git nu a returnat contorul ahead.".to_string())?
            .parse::<u64>()
            .map_err(|_| "Contorul ahead Git este invalid.".to_string())?;
        let behind = values
            .next()
            .ok_or_else(|| "Git nu a returnat contorul behind.".to_string())?
            .parse::<u64>()
            .map_err(|_| "Contorul behind Git este invalid.".to_string())?;
        if values.next().is_some() {
            return Err("Git a returnat contoare ahead/behind suplimentare.".to_string());
        }
        Ok((ahead, behind))
    }

    pub(super) fn history_for_range(
        &self,
        range: &str,
    ) -> Result<Vec<VersionHistoryEntry>, String> {
        let output = self
            .runner
            .run([
                "log",
                "-z",
                "--format=%H%x00%h%x00%P%x00%an%x00%ae%x00%aI%x00%s",
                &format!("--max-count={MAX_SYNC_COMPARISON_ENTRIES}"),
                range,
            ])?
            .require_success("Citirea istoricului comparativ Git")?;
        if output.stdout_truncated {
            return Err("Istoricul comparativ Git a fost trunchiat.".to_string());
        }
        parse_history(&output.stdout)
    }

    fn require_remote(&self, name: &str) -> Result<VersionRemote, String> {
        self.read_remotes()?
            .into_iter()
            .find(|remote| remote.name == name)
            .ok_or_else(|| format!("Remote-ul {name} nu există."))
    }

    fn require_usable_remote(&self, name: &str) -> Result<VersionRemote, String> {
        let remote = self.require_remote(name)?;
        if remote.usable {
            Ok(remote)
        } else {
            Err(remote
                .diagnostic
                .unwrap_or_else(|| format!("Remote-ul {name} nu poate fi folosit în siguranță.")))
        }
    }

    fn require_local_branch(&self, name: &str) -> Result<String, String> {
        self.resolve_ref_oid(&format!("refs/heads/{name}"))
            .map_err(|_| format!("Branch-ul local {name} nu există."))
    }

    pub(super) fn resolve_ref_oid(&self, reference: &str) -> Result<String, String> {
        let output = self.runner.run(["rev-parse", "--verify", reference])?;
        if !output.success() {
            return Err(network_safe_error("Rezolvarea referinței Git", &output));
        }
        let oid = output.stdout_text()?.trim().to_string();
        validate_oid(&oid)?;
        Ok(oid)
    }

    pub(super) fn validate_branch_name(&self, value: &str) -> Result<String, String> {
        let name = value.trim();
        if name.is_empty()
            || name.len() > MAX_BRANCH_NAME_BYTES
            || name.starts_with('-')
            || name.contains(['\0', '\n', '\r'])
        {
            return Err("Numele branch-ului Git este gol sau invalid.".to_string());
        }
        let full_ref = format!("refs/heads/{name}");
        let output = self.runner.run(["check-ref-format", &full_ref])?;
        if !output.success() {
            return Err(format!("Numele branch-ului Git {name:?} nu este valid."));
        }
        Ok(name.to_string())
    }

    fn remote_urls(&self, name: &str, push: bool) -> Result<Vec<String>, String> {
        let mut args = vec!["remote", "get-url", "--all"];
        if push {
            args.push("--push");
        }
        args.push(name);
        let output = self.runner.run(args)?;
        if !output.success() {
            return Ok(Vec::new());
        }
        if output.stdout_truncated {
            return Err(format!("URL-urile remote-ului {name} au fost trunchiate."));
        }
        Ok(output
            .stdout_text()?
            .lines()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect())
    }

    fn config_values(&self, key: &str) -> Result<Vec<String>, String> {
        let output = self.runner.run(["config", "--local", "--get-all", key])?;
        if output.success() {
            return Ok(output
                .stdout_text()?
                .lines()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect());
        }
        if output.status.code() == Some(1) {
            return Ok(Vec::new());
        }
        Err(network_safe_error("Citirea configurației Git", &output))
    }

    fn local_config_keys_matching(&self, pattern: &str) -> Result<Vec<String>, String> {
        let output = self.runner.run([
            "config",
            "--local",
            "--null",
            "--name-only",
            "--get-regexp",
            pattern,
        ])?;
        if !output.success() {
            if output.status.code() == Some(1) {
                return Ok(Vec::new());
            }
            return Err(network_safe_error(
                "Auditarea configurației Git locale",
                &output,
            ));
        }
        if output.stdout_truncated {
            return Err("Lista cheilor configurației Git locale a fost trunchiată.".to_string());
        }
        let mut keys = output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|record| !record.is_empty())
            .map(|record| {
                String::from_utf8(record.to_vec())
                    .map_err(|_| "O cheie din configurația Git locală nu este UTF-8.".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        keys.sort();
        keys.dedup();
        if keys.len() > 32 {
            let omitted = keys.len() - 32;
            keys.truncate(32);
            keys.push(format!("<încă {omitted} chei>"));
        }
        Ok(keys)
    }

    fn optional_local_config(&self, key: &str) -> Result<Option<String>, String> {
        Ok(self.config_values(key)?.into_iter().next())
    }

    fn unset_all_config(&self, key: &str) -> Result<(), String> {
        let output = self.runner.run(["config", "--local", "--unset-all", key])?;
        if output.success() || output.status.code() == Some(5) || output.status.code() == Some(1) {
            return Ok(());
        }
        Err(network_safe_error("Eliminarea configurației Git", &output))
    }
}

pub(crate) fn validate_operation_id(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.len() < 8
        || value.len() > 96
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("Identificatorul operației Git de rețea este invalid.".to_string());
    }
    Ok(value.to_string())
}

pub(crate) fn redact_network_text(value: &str) -> String {
    if value.contains("PRIVATE KEY-----") {
        return "[conținut sensibil ascuns]".to_string();
    }
    let mut sanitized = value.replace('\0', "").replace('\r', "\n");
    for scheme in ["https://", "http://", "ssh://"] {
        let mut cursor = 0;
        while let Some(relative) = sanitized[cursor..].find(scheme) {
            let start = cursor + relative + scheme.len();
            let end = sanitized[start..]
                .find(|character: char| {
                    character.is_ascii_whitespace() || matches!(character, '\'' | '"' | ')' | ']')
                })
                .map(|offset| start + offset)
                .unwrap_or(sanitized.len());
            let authority_end = sanitized[start..end]
                .find('/')
                .map(|offset| start + offset)
                .unwrap_or(end);
            if let Some(at) = sanitized[start..authority_end].rfind('@') {
                let at = start + at;
                sanitized.replace_range(start..=at, "[credenciale-ascunse]@");
                cursor = start + "[credenciale-ascunse]@".len();
            } else {
                cursor = authority_end.max(start + 1);
            }
        }
    }
    for marker in [
        "access_token=",
        "refresh_token=",
        "authorization:",
        "authorization=",
        "password=",
        "passwd=",
        "token=",
        "oauth=",
        "bearer ",
        "basic ",
    ] {
        redact_secret_values(&mut sanitized, marker);
    }
    sanitized.trim().chars().take(2_000).collect()
}

pub(crate) fn network_progress_text(value: &str) -> String {
    let sanitized = redact_network_text(value);
    let mut lines = Vec::new();
    for line in sanitized.lines() {
        let line = line.trim();
        let line = line.strip_prefix("remote: ").unwrap_or(line);
        if [
            "Enumerating objects:",
            "Counting objects:",
            "Compressing objects:",
            "Receiving objects:",
            "Resolving deltas:",
            "Writing objects:",
            "Total ",
        ]
        .iter()
        .any(|prefix| line.starts_with(prefix))
        {
            lines.push(line.chars().take(240).collect::<String>());
        }
    }
    if lines.is_empty() {
        "Transfer Git în curs…".to_string()
    } else {
        lines
            .into_iter()
            .rev()
            .take(2)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn redact_secret_values(value: &mut String, marker: &str) {
    let mut cursor = 0;
    loop {
        let lower = value.to_ascii_lowercase();
        let Some(relative_start) = lower[cursor..].find(marker) else {
            break;
        };
        let start = cursor + relative_start;
        let secret_start = start + marker.len();
        if value[secret_start..].starts_with("[secret-ascuns]") {
            cursor = secret_start + "[secret-ascuns]".len();
            continue;
        }
        let secret_end = value[secret_start..]
            .find(|character: char| {
                character.is_ascii_whitespace()
                    || matches!(character, '&' | ';' | '\'' | '"' | ')' | ']' | '}')
            })
            .map(|offset| secret_start + offset)
            .unwrap_or(value.len());
        if secret_start == secret_end {
            cursor = secret_start;
            continue;
        }
        value.replace_range(secret_start..secret_end, "[secret-ascuns]");
        cursor = secret_start + "[secret-ascuns]".len();
    }
}

fn validate_remote_name(value: &str) -> Result<String, String> {
    let name = value.trim();
    if name.is_empty()
        || name.len() > MAX_REMOTE_NAME_BYTES
        || matches!(name, "." | "..")
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        || name.starts_with('-')
    {
        return Err(
            "Numele remote-ului poate conține numai litere ASCII, cifre, punct, _ și -."
                .to_string(),
        );
    }
    Ok(name.to_string())
}

fn validate_remote_url(value: &str) -> Result<String, String> {
    let url = value.trim();
    if url.is_empty()
        || url.len() > MAX_REMOTE_URL_BYTES
        || url.contains(['\0', '\n', '\r', '\t', ' '])
        || url.contains(['?', '#'])
    {
        return Err(
            "URL-ul remote este gol, prea lung sau conține caractere/query nesuportate."
                .to_string(),
        );
    }
    if let Some(rest) = url.strip_prefix("https://") {
        validate_url_authority(rest, false)?;
        return Ok(url.to_string());
    }
    if let Some(rest) = url.strip_prefix("ssh://") {
        validate_url_authority(rest, true)?;
        return Ok(url.to_string());
    }
    if let Some(rest) = url.strip_prefix("git://") {
        validate_url_authority(rest, false)?;
        return Ok(url.to_string());
    }
    if url.contains("://")
        || url.starts_with('/')
        || url.starts_with('.')
        || url.starts_with('~')
        || url.contains('\\')
    {
        return Err(
            "Sunt acceptate numai remote-uri HTTPS, SSH, git:// sau forma SSH user@host:repo. Remote-urile locale și helper-ele externe sunt blocate."
                .to_string(),
        );
    }
    let (authority, path) = url.split_once(':').ok_or_else(|| {
        "Forma SSH a remote-ului trebuie să fie user@host:cale/repository.".to_string()
    })?;
    if authority.is_empty()
        || path.is_empty()
        || path.starts_with('/')
        || authority.contains('/')
        || authority.contains(':')
    {
        return Err("Forma SSH a URL-ului remote este invalidă.".to_string());
    }
    validate_ssh_authority(authority)?;
    validate_remote_path(path)?;
    Ok(url.to_string())
}

fn validate_url_authority(rest: &str, allow_ssh_user: bool) -> Result<(), String> {
    let (authority, path) = rest.split_once('/').ok_or_else(|| {
        "URL-ul remote trebuie să includă host și calea repository-ului.".to_string()
    })?;
    if authority.is_empty() || path.is_empty() || authority.contains(['[', ']']) {
        return Err("Host-ul sau calea URL-ului remote este invalidă.".to_string());
    }
    if allow_ssh_user {
        validate_ssh_authority(authority)?;
    } else if authority.contains('@') {
        return Err(
            "Credențialele în URL sunt interzise; folosește credential helper sau SSH agent."
                .to_string(),
        );
    }
    let host_port = authority
        .rsplit_once('@')
        .map(|(_, value)| value)
        .unwrap_or(authority);
    let host = host_port
        .split_once(':')
        .map(|(host, _)| host)
        .unwrap_or(host_port);
    if host.is_empty()
        || !host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err("Host-ul URL-ului remote conține caractere nesuportate.".to_string());
    }
    if let Some((_, port)) = host_port.split_once(':') {
        if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err("Portul URL-ului remote este invalid.".to_string());
        }
    }
    validate_remote_path(path)
}

fn validate_ssh_authority(authority: &str) -> Result<(), String> {
    if authority.matches('@').count() > 1 {
        return Err("Autoritatea SSH a URL-ului remote este invalidă.".to_string());
    }
    if let Some((user, host)) = authority.split_once('@') {
        if user.is_empty()
            || user.contains(':')
            || !user
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
            || host.is_empty()
        {
            return Err(
                "Credențialele în URL sunt interzise; SSH poate include numai un nume de utilizator."
                    .to_string(),
            );
        }
    }
    let host_port = authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority);
    let (host, port) = host_port
        .split_once(':')
        .map(|(host, port)| (host, Some(port)))
        .unwrap_or((host_port, None));
    if host.is_empty()
        || !host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err("Host-ul SSH al URL-ului remote este invalid.".to_string());
    }
    if port.is_some_and(|port| port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit())) {
        return Err("Portul SSH al URL-ului remote este invalid.".to_string());
    }
    Ok(())
}

fn validate_remote_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.starts_with(':')
        || path.contains(':')
        || path
            .split('/')
            .any(|segment| matches!(segment, "" | "." | ".."))
        || path.contains(['\0', '\n', '\r'])
    {
        return Err("Calea repository-ului remote este invalidă.".to_string());
    }
    Ok(())
}

fn single_remote_url(
    values: &[String],
    kind: &str,
    diagnostics: &mut Vec<String>,
) -> Option<String> {
    if values.len() != 1 {
        diagnostics.push(format!(
            "Remote-ul trebuie să aibă exact un URL de {kind}; au fost găsite {}.",
            values.len()
        ));
        None
    } else {
        Some(values[0].clone())
    }
}

fn safe_remote_display(value: Option<&str>, diagnostics: &mut Vec<String>) -> String {
    let Some(value) = value else {
        return REDACTED_REMOTE_URL.to_string();
    };
    match validate_remote_url(value) {
        Ok(value) => value,
        Err(error) => {
            diagnostics.push(error);
            REDACTED_REMOTE_URL.to_string()
        }
    }
}

fn parse_ref_records(
    output: &GitCommandOutput,
    field_count: usize,
    label: &str,
) -> Result<Vec<Vec<String>>, String> {
    if output.stdout_truncated {
        return Err(format!("Lista {label} a fost trunchiată."));
    }
    output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|record| !record.is_empty())
        .map(|record| {
            let mut fields = record
                .split(|byte| *byte == 0)
                .map(|field| {
                    String::from_utf8(field.to_vec())
                        .map_err(|_| format!("Lista {label} conține metadata non-UTF-8."))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if fields.last().is_some_and(String::is_empty) {
                fields.pop();
            }
            if fields.len() != field_count {
                return Err(format!("Lista {label} are un format neașteptat."));
            }
            Ok(fields)
        })
        .collect()
}

fn sync_state(ahead: u64, behind: u64) -> VersionSyncState {
    match (ahead, behind) {
        (0, 0) => VersionSyncState::UpToDate,
        (_, 0) => VersionSyncState::Ahead,
        (0, _) => VersionSyncState::Behind,
        _ => VersionSyncState::Diverged,
    }
}

fn classify_network_runtime_error(kind: VersionNetworkOperationKind, error: String) -> String {
    if error == NETWORK_CANCELLED_ERROR {
        return if kind == VersionNetworkOperationKind::Push {
            format!(
                "{error} Serverul poate să fi primit deja commit-ul. Nu repeta Push automat; rulează Fetch și verifică branch-ul remote."
            )
        } else {
            error
        };
    }
    let operation = match kind {
        VersionNetworkOperationKind::Fetch => "Fetch",
        VersionNetworkOperationKind::Push => "Push",
    };
    let diagnostic = redact_network_text(&error);
    if kind == VersionNetworkOperationKind::Push && error.contains("a depășit limita") {
        format!(
            "Push a depășit timeout-ul, iar rezultatul remote este necunoscut. Nu repeta Push automat; rulează Fetch și verifică branch-ul remote. {diagnostic}"
        )
    } else {
        format!("{operation} nu a putut rula: {diagnostic}")
    }
}

fn classify_network_output_error(
    kind: VersionNetworkOperationKind,
    output: &GitCommandOutput,
) -> String {
    let source = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let lower = source.to_ascii_lowercase();
    let operation = match kind {
        VersionNetworkOperationKind::Fetch => "Fetch",
        VersionNetworkOperationKind::Push => "Push",
    };
    if lower.contains("non-fast-forward") || lower.contains("fetch first") {
        return "Push a fost refuzat deoarece branch-ul remote conține versiuni absente local. Rulează Fetch, inspectează divergența și integrează explicit înainte de un nou Push."
            .to_string();
    }
    if lower.contains("authentication failed")
        || lower.contains("permission denied")
        || lower.contains("could not read username")
        || lower.contains("publickey")
        || lower.contains("credential")
    {
        return format!(
            "{operation} nu s-a putut autentifica. Configurează credential helper-ul Git sau cheia/agentul SSH în sistem; Pană Studio nu stochează secrete."
        );
    }
    if lower.contains("host key verification failed") {
        return "Verificarea cheii host SSH a eșuat. Confirmă host-ul în configurația SSH a sistemului înainte de a relua operația."
            .to_string();
    }
    if lower.contains("could not resolve host")
        || lower.contains("failed to connect")
        || lower.contains("connection timed out")
        || lower.contains("network is unreachable")
    {
        return format!(
            "{operation} nu a putut contacta serverul remote. Verifică rețeaua, host-ul și configurația proxy."
        );
    }
    if kind == VersionNetworkOperationKind::Push {
        format!(
            "Push a eșuat cu statusul {}, iar rezultatul remote nu poate fi demonstrat. Nu repeta Push automat; rulează Fetch și verifică branch-ul remote. Outputul necunoscut nu este afișat pentru a evita expunerea datelor sensibile.",
            output.status
        )
    } else {
        format!(
            "Fetch a eșuat cu statusul {}. Verifică remote-ul și configurația de transport; outputul necunoscut nu este afișat pentru a evita expunerea datelor sensibile.",
            output.status
        )
    }
}

fn network_safe_error(operation: &str, output: &GitCommandOutput) -> String {
    let diagnostic = redact_network_text(&String::from_utf8_lossy(&output.stderr));
    if diagnostic.is_empty() {
        format!("{operation} a eșuat cu statusul {}.", output.status)
    } else {
        format!("{operation} a eșuat: {diagnostic}")
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        net::{TcpListener, TcpStream},
        path::{Path, PathBuf},
        process::{Child, Command, Stdio},
        sync::atomic::{AtomicU64, Ordering},
        thread,
        time::Duration,
    };

    use super::*;

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "pana-versioning-remote-{label}-{}-{sequence}",
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

    struct DaemonGuard(Child);

    impl Drop for DaemonGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn repository(root: &Path) -> VersionRepository {
        VersionRepository::new(
            root.to_string_lossy().to_string(),
            root.to_path_buf(),
            root.to_path_buf(),
        )
    }

    #[test]
    fn accepts_secret_free_https_and_ssh_urls() {
        for value in [
            "https://github.com/example/site.git",
            "ssh://git@github.com/example/site.git",
            "git@github.com:example/site.git",
            "git://example.test/example/site.git",
        ] {
            assert_eq!(validate_remote_url(value).unwrap(), value);
        }
    }

    #[test]
    fn rejects_credentials_local_paths_and_external_helpers() {
        for value in [
            "https://token@github.com/example/site.git",
            "https://github.com/example/site.git?token=secret",
            "file:///tmp/repository.git",
            "/tmp/repository.git",
            "../repository.git",
            "ext::danger",
        ] {
            assert!(validate_remote_url(value).is_err(), "{value}");
        }
    }

    #[test]
    fn redacts_userinfo_from_network_diagnostics() {
        let value = redact_network_text(
            "fatal: unable to access 'https://token@example.test/site.git': denied token=secret Authorization: Bearer abc123",
        );
        assert!(!value.contains("token@example"), "{value}");
        assert!(!value.contains("token=secret"), "{value}");
        assert!(!value.contains("abc123"), "{value}");
        assert!(value.contains("credenciale-ascunse"), "{value}");
    }

    #[test]
    fn network_progress_exposes_only_known_transfer_lines() {
        let progress = network_progress_text(
            "helper said token=secret\nReceiving objects: 42% (42/100)\nhttps://example.test/private",
        );
        assert_eq!(progress, "Receiving objects: 42% (42/100)");
        assert!(!progress.contains("secret"));
        assert!(!progress.contains("example.test"));
    }

    #[test]
    fn cancelled_push_is_reported_as_remote_outcome_unknown() {
        let push = classify_network_runtime_error(
            VersionNetworkOperationKind::Push,
            NETWORK_CANCELLED_ERROR.to_string(),
        );
        assert!(push.contains("poate să fi primit"), "{push}");
        assert!(push.contains("Nu repeta Push"), "{push}");
        let fetch = classify_network_runtime_error(
            VersionNetworkOperationKind::Fetch,
            NETWORK_CANCELLED_ERROR.to_string(),
        );
        assert_eq!(fetch, NETWORK_CANCELLED_ERROR);
    }

    #[test]
    fn validates_remote_and_operation_identifiers() {
        assert_eq!(validate_remote_name("origin-1").unwrap(), "origin-1");
        assert!(validate_remote_name("../origin").is_err());
        assert!(validate_remote_name("--upload-pack").is_err());
        assert!(validate_operation_id("network-12345678").is_ok());
        assert!(validate_operation_id("short").is_err());
    }

    #[test]
    fn configures_remote_branch_and_upstream_with_ahead_behind_state() {
        let directory = TestDirectory::new("inventory");
        let repository = repository(&directory.0);
        repository.initialize().unwrap();
        repository
            .configure_identity("Pană Test", "pana@example.test")
            .unwrap();
        fs::write(directory.0.join("index.html"), "prima\n").unwrap();
        repository.stage_all().unwrap();
        let commit = repository.commit("Prima", None).unwrap();
        let before_remote = repository.snapshot().unwrap();

        let configured = repository
            .configure_remote(&VersionRemoteInput {
                name: "origin".to_string(),
                fetch_url: "https://example.test/client/site.git".to_string(),
                push_url: None,
            })
            .unwrap();
        assert_ne!(configured.status_token, before_remote.status_token);
        assert_eq!(configured.remotes.len(), 1);
        assert!(configured.remotes[0].usable);
        assert_eq!(configured.remotes[0].name, "origin");

        repository
            .runner
            .run(["update-ref", "refs/remotes/origin/main", &commit.commit_oid])
            .unwrap()
            .require_success("test remote tracking ref")
            .unwrap();
        let upstream = repository
            .configure_upstream(&VersionUpstreamInput {
                local_branch: "main".to_string(),
                remote: "origin".to_string(),
                remote_branch: "main".to_string(),
            })
            .unwrap();
        assert_eq!(upstream.sync_state, VersionSyncState::UpToDate);
        assert_eq!(upstream.upstream.as_ref().unwrap().ahead, 0);
        assert_eq!(upstream.upstream.as_ref().unwrap().behind, 0);

        let branched = repository
            .create_branch(&VersionBranchInput {
                name: "feature/pagina".to_string(),
                start_oid: None,
            })
            .unwrap();
        assert!(branched
            .branches
            .iter()
            .any(|branch| branch.name == "feature/pagina"));
        let deleted = repository.delete_branch("feature/pagina").unwrap();
        assert!(!deleted
            .branches
            .iter()
            .any(|branch| branch.name == "feature/pagina"));
    }

    #[test]
    fn existing_secret_url_and_unsafe_fetch_refspec_are_never_exposed_or_used() {
        let directory = TestDirectory::new("unsafe-remote");
        let repository = repository(&directory.0);
        repository.initialize().unwrap();
        repository
            .runner
            .run([
                "config",
                "--local",
                "remote.origin.url",
                "https://secret-token@example.test/site.git",
            ])
            .unwrap()
            .require_success("test unsafe url")
            .unwrap();
        repository
            .runner
            .run([
                "config",
                "--local",
                "remote.origin.fetch",
                "+refs/heads/*:refs/heads/*",
            ])
            .unwrap()
            .require_success("test unsafe refspec")
            .unwrap();

        let snapshot = repository.snapshot().unwrap();
        let remote = &snapshot.remotes[0];
        assert!(!remote.usable);
        assert_eq!(remote.fetch_url, REDACTED_REMOTE_URL);
        assert!(!remote.fetch_url.contains("secret-token"));
        assert!(!remote
            .diagnostic
            .as_deref()
            .unwrap()
            .contains("secret-token"));
    }

    #[test]
    fn blocks_local_transport_helpers_and_partial_clone_configuration() {
        let directory = TestDirectory::new("unsafe-config");
        let repository = repository(&directory.0);
        repository.initialize().unwrap();
        let configured = repository
            .configure_remote(&VersionRemoteInput {
                name: "origin".to_string(),
                fetch_url: "https://example.test/client/site.git".to_string(),
                push_url: None,
            })
            .unwrap();
        repository
            .runner
            .run(["config", "--local", "credential.helper", "!danger"])
            .unwrap()
            .require_success("test credential helper")
            .unwrap();
        repository
            .runner
            .run(["config", "--local", "core.sshCommand", "danger"])
            .unwrap()
            .require_success("test ssh command")
            .unwrap();
        let blocked = repository.snapshot().unwrap();
        assert_ne!(blocked.status_token, configured.status_token);
        assert!(!blocked.remotes[0].usable);
        let diagnostic = blocked.remotes[0].diagnostic.as_deref().unwrap();
        assert!(diagnostic.contains("credential.helper"), "{diagnostic}");
        assert!(diagnostic.contains("core.sshcommand"), "{diagnostic}");

        repository
            .runner
            .run(["config", "--local", "extensions.partialClone", "origin"])
            .unwrap()
            .require_success("test partial clone")
            .unwrap();
        let unsupported = repository.snapshot().unwrap();
        assert_eq!(
            unsupported.repository_state,
            super::super::VersionRepositoryState::Unsupported
        );
        assert!(unsupported
            .diagnostic
            .as_deref()
            .unwrap()
            .contains("partial clone"));
    }

    #[test]
    #[ignore = "cere un socket loopback pentru git daemon"]
    fn first_push_sets_tracking_and_later_non_fast_forward_is_rejected() {
        let directory = TestDirectory::new("git-daemon");
        let server_root = directory.0.join("server");
        let client_root = directory.0.join("client");
        let peer_root = directory.0.join("peer");
        fs::create_dir_all(&server_root).unwrap();
        fs::create_dir_all(&client_root).unwrap();
        fs::create_dir_all(&peer_root).unwrap();
        let remote_root = server_root.join("site.git");
        fs::create_dir_all(&remote_root).unwrap();
        super::super::git::GitRunner::new(&remote_root)
            .run(["init", "--bare", "--initial-branch=main", "."])
            .unwrap()
            .require_success("test bare init")
            .unwrap();

        let port = TcpListener::bind(("127.0.0.1", 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let child = Command::new("git")
            .arg("daemon")
            .arg("--reuseaddr")
            .arg("--export-all")
            .arg("--enable=receive-pack")
            .arg("--listen=127.0.0.1")
            .arg(format!("--port={port}"))
            .arg(format!("--base-path={}", server_root.display()))
            .arg(&server_root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let _daemon = DaemonGuard(child);
        let mut ready = false;
        for _ in 0..100 {
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                ready = true;
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(ready, "git daemon nu a pornit");
        let remote_url = format!("git://127.0.0.1:{port}/site.git");

        let client = repository(&client_root);
        client.initialize().unwrap();
        client
            .configure_identity("Pană Client", "client@example.test")
            .unwrap();
        fs::write(client_root.join("index.html"), "client\n").unwrap();
        client.stage_all().unwrap();
        client.commit("Client", None).unwrap();
        client
            .configure_remote(&VersionRemoteInput {
                name: "origin".to_string(),
                fetch_url: remote_url.clone(),
                push_url: None,
            })
            .unwrap();
        let first = client
            .push_branch(
                &VersionPushInput {
                    operation_id: "push-first-12345678".to_string(),
                    remote: "origin".to_string(),
                    remote_branch: "main".to_string(),
                    set_upstream: true,
                },
                Arc::new(AtomicBool::new(false)),
                Arc::new(|_| {}),
            )
            .unwrap();
        assert_eq!(first.snapshot.sync_state, VersionSyncState::UpToDate);
        assert_eq!(
            first.snapshot.upstream.as_ref().unwrap().ref_name,
            "refs/remotes/origin/main"
        );

        super::super::git::GitRunner::new(&directory.0)
            .run([
                "clone",
                "--branch",
                "main",
                "--single-branch",
                &remote_url,
                peer_root.to_str().unwrap(),
            ])
            .unwrap()
            .require_success("test peer clone")
            .unwrap();
        let peer = repository(&peer_root);
        peer.configure_identity("Pană Peer", "peer@example.test")
            .unwrap();
        fs::write(peer_root.join("index.html"), "peer\n").unwrap();
        peer.stage_all().unwrap();
        let peer_head = peer.snapshot().unwrap().head_oid;
        peer.commit("Peer", peer_head.as_deref()).unwrap();
        peer.runner
            .run(["update-ref", "refs/heads/side", "HEAD"])
            .unwrap()
            .require_success("test peer side branch")
            .unwrap();
        peer.runner
            .run(["tag", "remote-tag", "HEAD"])
            .unwrap()
            .require_success("test peer tag")
            .unwrap();
        peer.runner
            .run([
                "push",
                "origin",
                "refs/heads/main:refs/heads/main",
                "refs/heads/side:refs/heads/side",
                "refs/tags/remote-tag:refs/tags/remote-tag",
            ])
            .unwrap()
            .require_success("test peer push")
            .unwrap();

        client
            .fetch_remote(
                "origin",
                true,
                "fetch-prune-12345678",
                Arc::new(AtomicBool::new(false)),
                Arc::new(|_| {}),
            )
            .unwrap();
        assert!(client.resolve_ref_oid("refs/remotes/origin/side").is_ok());
        assert!(client.resolve_ref_oid("refs/tags/remote-tag").is_err());
        peer.runner
            .run(["push", "origin", ":refs/heads/side"])
            .unwrap()
            .require_success("test peer delete side")
            .unwrap();
        client
            .fetch_remote(
                "origin",
                true,
                "fetch-prune-87654321",
                Arc::new(AtomicBool::new(false)),
                Arc::new(|_| {}),
            )
            .unwrap();
        assert!(client.resolve_ref_oid("refs/remotes/origin/side").is_err());

        fs::write(client_root.join("index.html"), "client two\n").unwrap();
        client.stage_all().unwrap();
        let client_head = client.snapshot().unwrap().head_oid;
        client.commit("Client two", client_head.as_deref()).unwrap();
        let error = client
            .push_branch(
                &VersionPushInput {
                    operation_id: "push-reject-12345678".to_string(),
                    remote: "origin".to_string(),
                    remote_branch: "main".to_string(),
                    set_upstream: false,
                },
                Arc::new(AtomicBool::new(false)),
                Arc::new(|_| {}),
            )
            .unwrap_err();
        assert!(error.contains("conține versiuni absente local"), "{error}");
    }
}
