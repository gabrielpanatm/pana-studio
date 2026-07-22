use std::{
    ffi::OsString,
    fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use sha2::{Digest, Sha256};

use super::{
    git::{canonical_git_root, GitRunner},
    PreparedVersionRestore, VersionDiffInput, VersionDiffKind, VersionDiffReceipt, VersionFileKind,
    VersionFileStatus, VersionHistoryEntry, VersionHistoryPage, VersionPublicationStatus,
    VersionRepositoryState, VersionRestoreFinalization, VersionTree, VersionTreeFile,
    VersioningCommitReceipt, VersioningSnapshot, VERSIONING_SCHEMA_VERSION,
};

const MAX_COMMIT_MESSAGE_BYTES: usize = 16 * 1024;
const MAX_IDENTITY_BYTES: usize = 256;
const MAX_DIFF_BYTES: usize = 768 * 1024;
const MAX_HISTORY_PAGE: usize = 100;
const MAX_TREE_FILES: usize = 5_000;
const MAX_TREE_FILE_BYTES: usize = 32 * 1024 * 1024;
const MAX_TREE_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
static RESTORE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(crate) struct VersionRepository {
    pub(super) project_root: String,
    pub(super) repository_root: PathBuf,
    pub(super) runner: GitRunner,
}

impl VersionRepository {
    pub(crate) fn new(
        project_root: impl Into<String>,
        repository_root: impl Into<PathBuf>,
        subprocess_cwd: impl Into<PathBuf>,
    ) -> Self {
        Self {
            project_root: project_root.into(),
            repository_root: repository_root.into(),
            runner: GitRunner::new(subprocess_cwd),
        }
    }

    pub(crate) fn snapshot(&self) -> Result<VersioningSnapshot, String> {
        let repository_root = self.repository_root_string();
        let git_version = match self.runner.run(["--version"]) {
            Ok(output) if output.success() => Some(output.stdout_text()?.trim().to_string()),
            Ok(output) => {
                return Ok(self.terminal_snapshot(
                    VersionRepositoryState::GitUnavailable,
                    Some(output.stderr_lossy()),
                    None,
                ))
            }
            Err(error) => {
                return Ok(self.terminal_snapshot(
                    VersionRepositoryState::GitUnavailable,
                    Some(error),
                    None,
                ))
            }
        };

        let metadata_path = self.repository_root.join(".git");
        let metadata = match fs::symlink_metadata(&metadata_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Ok(self.terminal_snapshot(
                    VersionRepositoryState::Uninitialized,
                    None,
                    git_version,
                ))
            }
            Err(error) => {
                return Ok(self.terminal_snapshot(
                    VersionRepositoryState::Invalid,
                    Some(format!("Metadata .git nu poate fi citită: {error}")),
                    git_version,
                ))
            }
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Ok(self.terminal_snapshot(
                VersionRepositoryState::Unsupported,
                Some(
                    "Pană Studio acceptă numai un director .git real aflat direct în rădăcina Zola. Worktree-urile, gitdir-urile externe și symlink-urile nu sunt suportate."
                        .to_string(),
                ),
                git_version,
            ));
        }
        if let Some(diagnostic) = unsupported_git_metadata(&metadata_path)? {
            return Ok(self.terminal_snapshot(
                VersionRepositoryState::Unsupported,
                Some(diagnostic),
                git_version,
            ));
        }

        let reported_git_dir =
            match self
                .runner
                .run(["rev-parse", "--path-format=absolute", "--absolute-git-dir"])
            {
                Ok(output) if output.success() => canonical_git_root(&output.stdout_text()?)?,
                Ok(output) => {
                    return Ok(self.terminal_snapshot(
                        VersionRepositoryState::Invalid,
                        Some(nonempty_or(
                            output.stderr_lossy(),
                            "Directorul metadata Git nu poate fi rezolvat.",
                        )),
                        git_version,
                    ))
                }
                Err(error) => {
                    return Ok(self.terminal_snapshot(
                        VersionRepositoryState::Invalid,
                        Some(error),
                        git_version,
                    ))
                }
            };
        let expected_git_dir = metadata_path
            .canonicalize()
            .map_err(|error| format!("Directorul .git autorizat nu poate fi canonizat: {error}"))?;
        if reported_git_dir != expected_git_dir {
            return Ok(self.terminal_snapshot(
                VersionRepositoryState::Unsupported,
                Some(format!(
                    "Git folosește metadata din {}, nu directorul direct autorizat {}.",
                    reported_git_dir.display(),
                    expected_git_dir.display()
                )),
                git_version,
            ));
        }

        let reported_root =
            match self
                .runner
                .run(["rev-parse", "--path-format=absolute", "--show-toplevel"])
            {
                Ok(output) if output.success() => canonical_git_root(&output.stdout_text()?)?,
                Ok(output) => {
                    return Ok(self.terminal_snapshot(
                        VersionRepositoryState::Invalid,
                        Some(nonempty_or(
                            output.stderr_lossy(),
                            "Directorul .git nu descrie un repository valid.",
                        )),
                        git_version,
                    ))
                }
                Err(error) => {
                    return Ok(self.terminal_snapshot(
                        VersionRepositoryState::Invalid,
                        Some(error),
                        git_version,
                    ))
                }
            };
        let expected_root = self
            .repository_root
            .canonicalize()
            .map_err(|error| format!("Rădăcina Zola nu poate fi canonizată pentru Git: {error}"))?;
        if reported_root != expected_root {
            return Ok(self.terminal_snapshot(
                VersionRepositoryState::Unsupported,
                Some(format!(
                    "Repository-ul Git are root-ul {}, nu root-ul autorizat {}.",
                    reported_root.display(),
                    expected_root.display()
                )),
                git_version,
            ));
        }

        let branch_output = self
            .runner
            .run(["symbolic-ref", "--quiet", "--short", "HEAD"])?;
        let branch = if branch_output.success() {
            nonempty_trimmed(branch_output.stdout_text()?)
        } else {
            None
        };
        let head_output = self
            .runner
            .run(["rev-parse", "--verify", "HEAD^{commit}"])?;
        let head_oid = if head_output.success() {
            nonempty_trimmed(head_output.stdout_text()?)
        } else {
            None
        };
        let unborn_head = head_oid.is_none() && branch.is_some();
        let detached_head = head_oid.is_some() && branch.is_none();
        let object_format = self
            .runner
            .run(["rev-parse", "--show-object-format"])?
            .require_success("Citirea formatului de obiecte Git")?
            .stdout_text()?
            .trim()
            .to_string();
        if !matches!(object_format.as_str(), "sha1" | "sha256") {
            return Ok(self.terminal_snapshot(
                VersionRepositoryState::Unsupported,
                Some(format!(
                    "Formatul de obiecte Git {object_format:?} nu este suportat."
                )),
                git_version,
            ));
        }

        if let Some(diagnostic) = self.unsupported_partial_clone_config()? {
            return Ok(self.terminal_snapshot(
                VersionRepositoryState::Unsupported,
                Some(diagnostic),
                git_version,
            ));
        }

        if let Err(diagnostic) = self.require_no_tracked_filter_attributes() {
            return Ok(self.terminal_snapshot(
                VersionRepositoryState::Unsupported,
                Some(diagnostic),
                git_version,
            ));
        }

        let output_relative = self.configured_output_relative_path();
        if let Some(output) = output_relative.as_deref() {
            let tracked_output = self
                .runner
                .run(["ls-files", "-z", "--", output])?
                .require_success("Verificarea output-ului Zola în indexul Git")?;
            if !tracked_output.stdout.is_empty() {
                return Ok(self.terminal_snapshot(
                    VersionRepositoryState::Unsupported,
                    Some(format!(
                        "Output-ul Zola configurat `{output}` este deja urmărit de Git. Pană Studio nu migrează și nu versionează artefacte generate; elimină-l manual din index înainte de a continua."
                    )),
                    git_version,
                ));
            }
        }

        let mut status_args = vec![
            OsString::from("status"),
            OsString::from("--porcelain=v1"),
            OsString::from("-z"),
            OsString::from("--untracked-files=all"),
        ];
        append_source_pathspecs(&mut status_args, output_relative.as_deref());
        let status_output = self
            .runner
            .run(status_args)?
            .require_success("Citirea statusului Git")?;
        if status_output.stdout_truncated {
            return Err(
                "Statusul Git depășește limita sigură de output; operațiile mutabile sunt blocate."
                    .to_string(),
            );
        }
        let files = parse_porcelain_v1_z(&status_output.stdout)?;
        let staged_count = files.iter().filter(|file| file.staged).count();
        let unstaged_count = files.iter().filter(|file| file.unstaged).count();
        let conflicted_count = files.iter().filter(|file| file.conflicted).count();
        let user_name = self.optional_config("user.name")?;
        let user_email = self.optional_config("user.email")?;
        let remote_fingerprint = self.remote_state_fingerprint()?;
        let remote = self.remote_snapshot_parts(branch.as_deref(), head_oid.as_deref())?;
        let status_token = status_token(&[
            self.project_root.as_bytes(),
            repository_root.as_bytes(),
            branch.as_deref().unwrap_or("<detached>").as_bytes(),
            head_oid.as_deref().unwrap_or("<unborn>").as_bytes(),
            object_format.as_bytes(),
            user_name.as_deref().unwrap_or("").as_bytes(),
            user_email.as_deref().unwrap_or("").as_bytes(),
            &status_output.stdout,
            &remote_fingerprint,
        ]);

        Ok(VersioningSnapshot {
            schema_version: VERSIONING_SCHEMA_VERSION,
            project_root: self.project_root.clone(),
            repository_root,
            repository_state: VersionRepositoryState::Ready,
            diagnostic: None,
            git_version,
            object_format: Some(object_format),
            branch,
            detached_head,
            unborn_head,
            head_oid,
            status_token,
            clean: files.is_empty(),
            staged_count,
            unstaged_count,
            conflicted_count,
            files,
            user_name,
            user_email,
            remotes: remote.remotes,
            branches: remote.branches,
            remote_branches: remote.remote_branches,
            upstream: remote.upstream,
            sync_state: remote.sync_state,
        })
    }

    pub(crate) fn initialize(&self) -> Result<VersioningSnapshot, String> {
        let before = self.snapshot()?;
        match before.repository_state {
            VersionRepositoryState::Uninitialized => {}
            VersionRepositoryState::Ready => return Ok(before),
            VersionRepositoryState::GitUnavailable => {
                return Err(before
                    .diagnostic
                    .unwrap_or_else(|| "Git nu este disponibil pentru inițializare.".to_string()))
            }
            VersionRepositoryState::Invalid | VersionRepositoryState::Unsupported => {
                return Err(before.diagnostic.unwrap_or_else(|| {
                    "Repository-ul existent nu poate fi inițializat în siguranță.".to_string()
                }))
            }
        }
        self.runner
            .run(["init", "--initial-branch=main", "."])?
            .require_success("Inițializarea repository-ului Git")?;
        let after = self.snapshot()?;
        require_ready(&after)?;
        Ok(after)
    }

    pub(crate) fn configure_identity(
        &self,
        name: &str,
        email: &str,
    ) -> Result<VersioningSnapshot, String> {
        validate_identity(name, "Numele Git")?;
        validate_identity(email, "Emailul Git")?;
        if !email.contains('@') {
            return Err("Emailul Git trebuie să conțină caracterul @.".to_string());
        }
        require_ready(&self.snapshot()?)?;
        self.runner
            .run(["config", "--local", "--replace-all", "user.name", name])?
            .require_success("Configurarea numelui Git")?;
        self.runner
            .run(["config", "--local", "--replace-all", "user.email", email])?
            .require_success("Configurarea emailului Git")?;
        self.snapshot()
    }

    pub(crate) fn stage_paths(&self, paths: &[String]) -> Result<VersioningSnapshot, String> {
        let paths = validate_paths(paths)?;
        if paths.is_empty() {
            return Err("Stage cere cel puțin un fișier.".to_string());
        }
        if let Some(output) = self.configured_output_relative_path() {
            if let Some(path) = paths.iter().find(|path| is_output_path(path, &output)) {
                return Err(format!(
                    "Stage a refuzat artefactul Zola generat `{path}` din output_dir `{output}`."
                ));
            }
        }
        self.require_no_external_clean_filters(&paths)?;
        let mut args = vec![OsString::from("add"), OsString::from("--")];
        args.extend(paths.iter().map(OsString::from));
        self.runner
            .run(args)?
            .require_success("Pregătirea fișierelor Git")?;
        self.snapshot()
    }

    pub(crate) fn stage_all(&self) -> Result<VersioningSnapshot, String> {
        let before = self.snapshot()?;
        require_ready(&before)?;
        let paths = before
            .files
            .iter()
            .flat_map(|file| [Some(file.path.clone()), file.original_path.clone()])
            .flatten()
            .collect::<Vec<_>>();
        self.require_no_external_clean_filters(&paths)?;
        let mut args = vec![OsString::from("add"), OsString::from("-A")];
        append_source_pathspecs(&mut args, self.configured_output_relative_path().as_deref());
        self.runner
            .run(args)?
            .require_success("Pregătirea tuturor fișierelor Git")?;
        self.snapshot()
    }

    pub(crate) fn unstage_paths(&self, paths: &[String]) -> Result<VersioningSnapshot, String> {
        let paths = validate_paths(paths)?;
        if paths.is_empty() {
            return Err("Unstage cere cel puțin un fișier.".to_string());
        }
        let snapshot = self.snapshot()?;
        require_ready(&snapshot)?;
        let mut args = if snapshot.head_oid.is_some() {
            vec![
                OsString::from("restore"),
                OsString::from("--staged"),
                OsString::from("--"),
            ]
        } else {
            vec![
                OsString::from("rm"),
                OsString::from("--cached"),
                OsString::from("--ignore-unmatch"),
                OsString::from("--"),
            ]
        };
        args.extend(paths.iter().map(OsString::from));
        self.runner
            .run(args)?
            .require_success("Eliminarea fișierelor din indexul Git")?;
        self.snapshot()
    }

    fn configured_output_relative_path(&self) -> Option<String> {
        let project_root = Path::new(&self.project_root);
        let output =
            crate::deploy::resolve_artifact_root(project_root, &self.repository_root).ok()?;
        let relative = output.strip_prefix(&self.repository_root).ok()?;
        let value = relative.to_string_lossy().replace('\\', "/");
        (!value.is_empty()).then_some(value)
    }

    pub(crate) fn unstage_all(&self) -> Result<VersioningSnapshot, String> {
        let snapshot = self.snapshot()?;
        require_ready(&snapshot)?;
        if snapshot.staged_count == 0 {
            return Ok(snapshot);
        }
        if snapshot.head_oid.is_some() {
            self.runner
                .run(["restore", "--staged", "--", "."])?
                .require_success("Golirea indexului Git")?;
        } else {
            self.runner
                .run(["rm", "-r", "--cached", "--ignore-unmatch", "--", "."])?
                .require_success("Golirea indexului Git unborn")?;
        }
        self.snapshot()
    }

    pub(crate) fn commit(
        &self,
        message: &str,
        expected_head_oid: Option<&str>,
    ) -> Result<VersioningCommitReceipt, String> {
        let message = validate_commit_message(message)?;
        let before = self.snapshot()?;
        require_ready(&before)?;
        if before.detached_head {
            return Err("Commit-ul este blocat pe detached HEAD.".to_string());
        }
        if before.conflicted_count > 0 {
            return Err("Commit-ul este blocat cât timp repository-ul are conflicte.".to_string());
        }
        if before.staged_count == 0 {
            return Err("Nu există modificări pregătite pentru commit.".to_string());
        }
        if before.user_name.is_none() || before.user_email.is_none() {
            return Err(
                "Identitatea Git lipsește. Configurează numele și emailul repository-ului."
                    .to_string(),
            );
        }
        if before.head_oid.as_deref() != expected_head_oid {
            return Err(
                "HEAD s-a schimbat înainte de commit; actualizează panoul Versiuni.".to_string(),
            );
        }

        let tree_oid = self
            .runner
            .run(["write-tree"])?
            .require_success("Construirea arborelui Git")?
            .stdout_text()?
            .trim()
            .to_string();
        validate_oid(&tree_oid)?;

        let mut commit_args = vec![OsString::from("commit-tree"), OsString::from(&tree_oid)];
        if let Some(parent) = before.head_oid.as_deref() {
            validate_oid(parent)?;
            commit_args.push(OsString::from("-p"));
            commit_args.push(OsString::from(parent));
        }
        let commit_oid = self
            .runner
            .run_with_input(commit_args, format!("{message}\n").as_bytes())?
            .require_success("Crearea obiectului commit Git")?
            .stdout_text()?
            .trim()
            .to_string();
        validate_oid(&commit_oid)?;

        let full_ref = self
            .runner
            .run(["symbolic-ref", "--quiet", "HEAD"])?
            .require_success("Citirea referinței HEAD")?
            .stdout_text()?
            .trim()
            .to_string();
        if !full_ref.starts_with("refs/heads/") {
            return Err("HEAD nu indică un branch local suportat.".to_string());
        }
        let old_oid_storage = before
            .head_oid
            .clone()
            .unwrap_or_else(|| zero_oid(before.object_format.as_deref()));
        self.runner
            .run(["update-ref", &full_ref, &commit_oid, &old_oid_storage])?
            .require_success("Publicarea commit-ului Git")?;

        match self.snapshot() {
            Ok(after) if after.head_oid.as_deref() == Some(commit_oid.as_str()) => {
                Ok(VersioningCommitReceipt {
                    schema_version: VERSIONING_SCHEMA_VERSION,
                    commit_oid,
                    parent_oid: before.head_oid,
                    message,
                    publication_status: VersionPublicationStatus::Published,
                    diagnostic: None,
                    snapshot: Some(after),
                })
            }
            Ok(after) => Ok(VersioningCommitReceipt {
                schema_version: VERSIONING_SCHEMA_VERSION,
                commit_oid: commit_oid.clone(),
                parent_oid: before.head_oid,
                message,
                publication_status: VersionPublicationStatus::PublishedRefreshRequired,
                diagnostic: Some(format!(
                    "Commit-ul {commit_oid} a fost publicat, dar HEAD observat ulterior este {:?}. Actualizează starea; nu repeta commit-ul automat.",
                    after.head_oid
                )),
                snapshot: Some(after),
            }),
            Err(error) => Ok(VersioningCommitReceipt {
                schema_version: VERSIONING_SCHEMA_VERSION,
                commit_oid: commit_oid.clone(),
                parent_oid: before.head_oid,
                message,
                publication_status: VersionPublicationStatus::PublishedRefreshRequired,
                diagnostic: Some(format!(
                    "Commit-ul {commit_oid} a fost publicat, dar starea rezultată nu a putut fi citită: {error} Nu repeta commit-ul automat."
                )),
                snapshot: None,
            }),
        }
    }

    pub(crate) fn history(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<VersionHistoryPage, String> {
        let snapshot = self.snapshot()?;
        require_ready(&snapshot)?;
        let limit = limit.clamp(1, MAX_HISTORY_PAGE);
        if snapshot.head_oid.is_none() {
            return Ok(VersionHistoryPage {
                schema_version: VERSIONING_SCHEMA_VERSION,
                offset,
                limit,
                has_more: false,
                entries: Vec::new(),
            });
        }
        let requested = limit.saturating_add(1);
        let output = self
            .runner
            .run([
                "log",
                "-z",
                "--format=%H%x00%h%x00%P%x00%an%x00%ae%x00%aI%x00%s",
                &format!("--skip={offset}"),
                &format!("--max-count={requested}"),
                "HEAD",
            ])?
            .require_success("Citirea istoricului Git")?;
        if output.stdout_truncated {
            return Err("Istoricul Git depășește limita sigură de output.".to_string());
        }
        let mut entries = parse_history(&output.stdout)?;
        let has_more = entries.len() > limit;
        entries.truncate(limit);
        Ok(VersionHistoryPage {
            schema_version: VERSIONING_SCHEMA_VERSION,
            offset,
            limit,
            has_more,
            entries,
        })
    }

    pub(crate) fn diff(&self, input: &VersionDiffInput) -> Result<VersionDiffReceipt, String> {
        let snapshot = self.snapshot()?;
        require_ready(&snapshot)?;
        let path = input.path.as_deref().map(validate_path).transpose()?;
        let mut args = match input.kind {
            VersionDiffKind::Unstaged => vec![
                OsString::from("diff"),
                OsString::from("--no-ext-diff"),
                OsString::from("--no-textconv"),
                OsString::from("--no-color"),
                OsString::from("--unified=3"),
            ],
            VersionDiffKind::Staged => vec![
                OsString::from("diff"),
                OsString::from("--cached"),
                OsString::from("--no-ext-diff"),
                OsString::from("--no-textconv"),
                OsString::from("--no-color"),
                OsString::from("--unified=3"),
            ],
            VersionDiffKind::Commit => {
                let commit_oid = input
                    .commit_oid
                    .as_deref()
                    .ok_or_else(|| "Diff-ul unei versiuni cere commitOid.".to_string())?;
                let commit_oid = self.resolve_commit_oid(commit_oid)?;
                vec![
                    OsString::from("show"),
                    OsString::from("--format="),
                    OsString::from("--no-ext-diff"),
                    OsString::from("--no-textconv"),
                    OsString::from("--no-color"),
                    OsString::from("--unified=3"),
                    OsString::from(commit_oid),
                ]
            }
            VersionDiffKind::Integration => {
                let target_ref = input
                    .target_ref
                    .as_deref()
                    .ok_or_else(|| "Preview-ul integrării cere targetRef.".to_string())?;
                let expected_target_oid = input
                    .expected_target_oid
                    .as_deref()
                    .ok_or_else(|| "Preview-ul integrării cere expectedTargetOid.".to_string())?;
                let (_, target_oid) =
                    self.resolve_integration_target(&snapshot, target_ref, expected_target_oid)?;
                vec![
                    OsString::from("diff"),
                    OsString::from("--no-ext-diff"),
                    OsString::from("--no-textconv"),
                    OsString::from("--no-color"),
                    OsString::from("--unified=3"),
                    OsString::from(format!("HEAD...{target_oid}")),
                ]
            }
        };
        if let Some(path) = path.as_ref() {
            args.push(OsString::from("--"));
            args.push(OsString::from(path));
        }
        let output = self
            .runner
            .run_with_limit(args, MAX_DIFF_BYTES)?
            .require_success("Citirea diff-ului Git")?;
        let patch = String::from_utf8_lossy(&output.stdout).to_string();
        let binary = patch.contains("Binary files ")
            || patch.contains("GIT binary patch")
            || patch.contains("Binary file ");
        Ok(VersionDiffReceipt {
            schema_version: VERSIONING_SCHEMA_VERSION,
            kind: input.kind,
            path,
            commit_oid: match input.kind {
                VersionDiffKind::Integration => input.expected_target_oid.clone(),
                _ => input.commit_oid.clone(),
            },
            binary,
            truncated: output.stdout_truncated,
            patch,
        })
    }

    pub(crate) fn read_tree(&self, commit_oid: &str) -> Result<VersionTree, String> {
        let snapshot = self.snapshot()?;
        require_ready(&snapshot)?;
        let commit_oid = self.resolve_commit_oid(commit_oid)?;
        let tree_spec = format!("{commit_oid}^{{tree}}");
        let tree_oid = self
            .runner
            .run(["rev-parse", "--verify", &tree_spec])?
            .require_success("Rezolvarea arborelui versiunii Git")?
            .stdout_text()?
            .trim()
            .to_string();
        validate_oid(&tree_oid)?;

        let output = self
            .runner
            .run(["ls-tree", "-r", "-z", "--full-tree", &commit_oid])?
            .require_success("Citirea arborelui versiunii Git")?;
        if output.stdout_truncated {
            return Err("Lista de fișiere a versiunii Git depășește limita sigură.".to_string());
        }
        let records = output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|record| !record.is_empty())
            .collect::<Vec<_>>();
        if records.len() > MAX_TREE_FILES {
            return Err(format!(
                "Versiunea conține {} fișiere, peste limita sigură de {MAX_TREE_FILES}.",
                records.len()
            ));
        }

        let output_relative = self.configured_output_relative_path();
        let mut descriptors = Vec::with_capacity(records.len());
        for record in records {
            let tab = record
                .iter()
                .position(|byte| *byte == b'\t')
                .ok_or_else(|| "ls-tree a returnat un record fără path.".to_string())?;
            let header = std::str::from_utf8(&record[..tab])
                .map_err(|_| "ls-tree a returnat metadata non-UTF-8.".to_string())?;
            let mut fields = header.split_whitespace();
            let mode = fields
                .next()
                .ok_or_else(|| "ls-tree nu a returnat mode.".to_string())?;
            let object_type = fields
                .next()
                .ok_or_else(|| "ls-tree nu a returnat tipul obiectului.".to_string())?;
            let oid = fields
                .next()
                .ok_or_else(|| "ls-tree nu a returnat OID-ul obiectului.".to_string())?;
            if fields.next().is_some() {
                return Err("ls-tree a returnat metadata suplimentară neașteptată.".to_string());
            }
            if object_type != "blob" || !matches!(mode, "100644" | "100755") {
                return Err(format!(
                    "Versiunea conține un obiect nesuportat ({mode} {object_type}). Symlink-urile, submodulele și tipurile speciale nu pot fi previzualizate sau restaurate."
                ));
            }
            validate_oid(oid)?;
            let path = utf8_git_path(&record[tab + 1..])?;
            if matches!(path.as_str(), "sursa/zola.toml" | "sursa/config.toml") {
                return Err(
                    "Versiunea folosește structura veche cu rădăcina Zola în `sursa/`. Pană Studio nu migrează și nu restaurează acest format."
                        .to_string(),
                );
            }
            if output_relative
                .as_deref()
                .is_some_and(|output| is_output_path(&path, output))
            {
                return Err(format!(
                    "Versiunea conține output-ul Zola generat `{path}`. Pană Studio nu îl previzualizează, integrează sau restaurează."
                ));
            }
            descriptors.push((path, oid.to_string(), mode == "100755"));
        }
        descriptors.sort_by(|left, right| left.0.cmp(&right.0));
        if descriptors.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err("Arborele Git conține path-uri duplicate.".to_string());
        }

        let mut total_bytes = 0_u64;
        let mut files = Vec::with_capacity(descriptors.len());
        for (path, oid, executable) in descriptors {
            let output = self.runner.run_with_limit(
                ["cat-file", "blob", &oid],
                MAX_TREE_FILE_BYTES.saturating_add(1),
            )?;
            let output = output.require_success("Citirea unui blob Git")?;
            if output.stdout_truncated || output.stdout.len() > MAX_TREE_FILE_BYTES {
                return Err(format!(
                    "Fișierul {path} depășește limita de {MAX_TREE_FILE_BYTES} bytes."
                ));
            }
            total_bytes = total_bytes
                .checked_add(output.stdout.len() as u64)
                .ok_or_else(|| "Arborele Git a depășit contorul de bytes.".to_string())?;
            if total_bytes > MAX_TREE_TOTAL_BYTES {
                return Err(format!(
                    "Versiunea depășește limita totală de {MAX_TREE_TOTAL_BYTES} bytes."
                ));
            }
            files.push(VersionTreeFile {
                path,
                oid,
                bytes: output.stdout,
                executable,
            });
        }

        Ok(VersionTree {
            commit_oid,
            tree_oid,
            files,
            total_bytes,
        })
    }

    pub(crate) fn prepare_restore(
        &self,
        target: &VersionTree,
        message: &str,
        expected_head_oid: &str,
    ) -> Result<PreparedVersionRestore, String> {
        let snapshot = self.snapshot()?;
        require_ready(&snapshot)?;
        if !snapshot.clean {
            return Err(
                "Restaurarea cere un repository Git complet curat, fără staged, unstaged sau untracked."
                    .to_string(),
            );
        }
        if snapshot.detached_head {
            return Err("Restaurarea este blocată pe detached HEAD.".to_string());
        }
        if snapshot.conflicted_count > 0 {
            return Err("Restaurarea este blocată de conflicte Git.".to_string());
        }
        if snapshot.head_oid.as_deref() != Some(expected_head_oid) {
            return Err("HEAD s-a schimbat înainte de pregătirea restaurării.".to_string());
        }
        if target.commit_oid == expected_head_oid {
            return Err("Versiunea aleasă este deja HEAD.".to_string());
        }
        validate_oid(expected_head_oid)?;
        validate_oid(&target.commit_oid)?;
        validate_oid(&target.tree_oid)?;
        let message = validate_commit_message(message)?;
        let full_head_ref = self
            .runner
            .run(["symbolic-ref", "--quiet", "HEAD"])?
            .require_success("Citirea referinței HEAD pentru restaurare")?
            .stdout_text()?
            .trim()
            .to_string();
        if !full_head_ref.starts_with("refs/heads/") {
            return Err("Restaurarea cere un branch local simbolic.".to_string());
        }
        let sequence = RESTORE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default();
        let transaction_id = format!("restore-{timestamp}-{}-{sequence}", std::process::id());
        let recovery_ref = format!("refs/pana-studio/restores/{transaction_id}");
        let commit_message = format!(
            "{message}\n\nPana-Studio-Restore-From: {}\nPana-Studio-Restore-Transaction: {transaction_id}\nPana-Studio-Restore-Head-Ref: {full_head_ref}\n",
            target.commit_oid,
        );
        let restore_commit_oid = self
            .runner
            .run_with_input(
                [
                    "commit-tree",
                    target.tree_oid.as_str(),
                    "-p",
                    expected_head_oid,
                ],
                commit_message.as_bytes(),
            )?
            .require_success("Pregătirea commit-ului de restaurare")?
            .stdout_text()?
            .trim()
            .to_string();
        validate_oid(&restore_commit_oid)?;
        let zero = zero_oid(snapshot.object_format.as_deref());
        self.runner
            .run(["update-ref", &recovery_ref, &restore_commit_oid, &zero])?
            .require_success("Publicarea marker-ului durabil de restaurare")?;
        let marker_oid = self
            .runner
            .run(["rev-parse", "--verify", &recovery_ref])?
            .require_success("Verificarea marker-ului durabil de restaurare")?
            .stdout_text()?
            .trim()
            .to_string();
        if marker_oid != restore_commit_oid {
            return Err(format!(
                "Marker-ul restaurării nu confirmă commit-ul pregătit {restore_commit_oid}."
            ));
        }
        Ok(PreparedVersionRestore {
            transaction_id,
            recovery_ref,
            target_commit_oid: target.commit_oid.clone(),
            target_tree_oid: target.tree_oid.clone(),
            previous_head_oid: expected_head_oid.to_string(),
            restore_commit_oid,
            full_head_ref,
        })
    }

    pub(crate) fn cancel_prepared_restore(
        &self,
        prepared: &PreparedVersionRestore,
    ) -> Result<(), String> {
        self.runner
            .run([
                "update-ref",
                "-d",
                &prepared.recovery_ref,
                &prepared.restore_commit_oid,
            ])?
            .require_success("Eliminarea marker-ului restaurării fără efect")?;
        Ok(())
    }

    pub(crate) fn read_restore_markers(&self) -> Result<Vec<PreparedVersionRestore>, String> {
        let snapshot = self.snapshot()?;
        require_ready(&snapshot)?;
        let output = self
            .runner
            .run([
                "for-each-ref",
                "--format=%(refname)%00%(objectname)",
                "refs/pana-studio/restores",
            ])?
            .require_success("Citirea marker-elor de restaurare")?;
        if output.stdout_truncated {
            return Err("Lista marker-elor de restaurare a fost trunchiată.".to_string());
        }
        let source = output.stdout_text()?;
        let mut markers = Vec::new();
        for line in source.lines().filter(|line| !line.is_empty()) {
            let (recovery_ref, restore_commit_oid) = line.split_once('\0').ok_or_else(|| {
                "Git a returnat un marker de restaurare cu format invalid.".to_string()
            })?;
            if !recovery_ref.starts_with("refs/pana-studio/restores/") {
                return Err(format!(
                    "Marker de restaurare în afara namespace-ului intern: {recovery_ref}."
                ));
            }
            validate_oid(restore_commit_oid)?;
            let transaction_id = recovery_ref
                .strip_prefix("refs/pana-studio/restores/")
                .filter(|value| !value.is_empty() && !value.contains('/'))
                .ok_or_else(|| format!("Marker de restaurare invalid: {recovery_ref}."))?;
            let commit = self
                .runner
                .run_with_limit(
                    ["cat-file", "commit", restore_commit_oid],
                    MAX_COMMIT_MESSAGE_BYTES.saturating_add(8 * 1024),
                )?
                .require_success("Citirea commit-ului intern de restaurare")?;
            if commit.stdout_truncated {
                return Err(format!(
                    "Commit-ul intern {restore_commit_oid} depășește limita sigură."
                ));
            }
            let commit = commit.stdout_text()?;
            let (headers, message) = commit.split_once("\n\n").ok_or_else(|| {
                format!("Commit-ul intern {restore_commit_oid} nu are mesaj valid.")
            })?;
            let mut tree_oid = None;
            let mut parent_oids = Vec::new();
            for header in headers.lines() {
                if let Some(value) = header.strip_prefix("tree ") {
                    tree_oid = Some(value.to_string());
                } else if let Some(value) = header.strip_prefix("parent ") {
                    parent_oids.push(value.to_string());
                }
            }
            let tree_oid = tree_oid
                .ok_or_else(|| format!("Commit-ul intern {restore_commit_oid} nu declară tree."))?;
            validate_oid(&tree_oid)?;
            if parent_oids.len() != 1 {
                return Err(format!(
                    "Commit-ul intern {restore_commit_oid} trebuie să aibă exact un părinte."
                ));
            }
            let previous_head_oid = parent_oids.remove(0);
            validate_oid(&previous_head_oid)?;
            let target_commit_oid =
                unique_restore_trailer(message, "Pana-Studio-Restore-From: ", restore_commit_oid)?;
            validate_oid(&target_commit_oid)?;
            let recorded_transaction = unique_restore_trailer(
                message,
                "Pana-Studio-Restore-Transaction: ",
                restore_commit_oid,
            )?;
            if recorded_transaction != transaction_id {
                return Err(format!(
                    "Marker-ul {recovery_ref} nu corespunde tranzacției declarate în commit."
                ));
            }
            let full_head_ref = unique_restore_trailer(
                message,
                "Pana-Studio-Restore-Head-Ref: ",
                restore_commit_oid,
            )?;
            if !full_head_ref.starts_with("refs/heads/") {
                return Err(format!(
                    "Commit-ul intern {restore_commit_oid} declară un branch invalid."
                ));
            }
            let target_tree_oid = self
                .runner
                .run([
                    "rev-parse",
                    "--verify",
                    &format!("{target_commit_oid}^{{tree}}"),
                ])?
                .require_success("Validarea versiunii țintă din marker")?
                .stdout_text()?
                .trim()
                .to_string();
            if target_tree_oid != tree_oid {
                return Err(format!(
                    "Commit-ul intern {restore_commit_oid} nu reproduce arborele versiunii țintă {target_commit_oid}."
                ));
            }
            markers.push(PreparedVersionRestore {
                transaction_id: transaction_id.to_string(),
                recovery_ref: recovery_ref.to_string(),
                target_commit_oid,
                target_tree_oid,
                previous_head_oid,
                restore_commit_oid: restore_commit_oid.to_string(),
                full_head_ref,
            });
        }
        markers.sort_by(|left, right| left.recovery_ref.cmp(&right.recovery_ref));
        Ok(markers)
    }

    pub(crate) fn abort_prepared_restore(
        &self,
        prepared: &PreparedVersionRestore,
    ) -> Result<VersioningSnapshot, String> {
        let live_head = self
            .runner
            .run(["rev-parse", "--verify", "HEAD^{commit}"])?
            .require_success("Citirea HEAD la anularea restaurării")?
            .stdout_text()?
            .trim()
            .to_string();
        if live_head != prepared.previous_head_oid {
            return Err(format!(
                "Rollback-ul marker-ului {} cere HEAD {}, dar HEAD este {live_head}.",
                prepared.recovery_ref, prepared.previous_head_oid
            ));
        }
        let previous_tree_oid = self
            .runner
            .run([
                "rev-parse",
                "--verify",
                &format!("{}^{{tree}}", prepared.previous_head_oid),
            ])?
            .require_success("Citirea arborelui anterior restaurării")?
            .stdout_text()?
            .trim()
            .to_string();
        self.runner
            .run(["read-tree", &previous_tree_oid])?
            .require_success("Revenirea indexului Git la arborele anterior")?;
        let index_tree = self
            .runner
            .run(["write-tree"])?
            .require_success("Verificarea indexului Git după rollback")?
            .stdout_text()?
            .trim()
            .to_string();
        if index_tree != previous_tree_oid {
            return Err("Indexul Git nu a revenit exact la arborele anterior.".to_string());
        }
        self.cancel_prepared_restore(prepared)?;
        self.snapshot()
    }

    pub(crate) fn finalize_restore(
        &self,
        prepared: &PreparedVersionRestore,
    ) -> Result<VersionRestoreFinalization, String> {
        let marker_oid = self
            .runner
            .run(["rev-parse", "--verify", &prepared.recovery_ref])?
            .require_success("Citirea marker-ului restaurării")?
            .stdout_text()?
            .trim()
            .to_string();
        if marker_oid != prepared.restore_commit_oid {
            return Err("Marker-ul restaurării a devenit stale sau a fost înlocuit.".to_string());
        }
        let live_head = self
            .runner
            .run(["rev-parse", "--verify", "HEAD^{commit}"])?
            .require_success("Citirea HEAD la finalizarea restaurării")?
            .stdout_text()?
            .trim()
            .to_string();
        if live_head != prepared.previous_head_oid && live_head != prepared.restore_commit_oid {
            return Err(format!(
                "HEAD a divergat în timpul restaurării: {live_head}. Marker-ul durabil a fost păstrat pentru recovery."
            ));
        }

        self.runner
            .run(["read-tree", &prepared.target_tree_oid])?
            .require_success("Alinierea indexului la arborele restaurat")?;
        let index_tree = self
            .runner
            .run(["write-tree"])?
            .require_success("Verificarea indexului restaurat")?
            .stdout_text()?
            .trim()
            .to_string();
        if index_tree != prepared.target_tree_oid {
            return Err(format!(
                "Indexul Git nu corespunde arborelui restaurat {}. Marker-ul durabil a fost păstrat.",
                prepared.target_tree_oid
            ));
        }
        if live_head == prepared.previous_head_oid {
            self.runner
                .run([
                    "update-ref",
                    &prepared.full_head_ref,
                    &prepared.restore_commit_oid,
                    &prepared.previous_head_oid,
                ])?
                .require_success("Publicarea commit-ului de restaurare")?;
        }

        let cleanup = self.cancel_prepared_restore(prepared);
        let snapshot = self.snapshot();
        match (cleanup, snapshot) {
            (Ok(()), Ok(snapshot)) => Ok(VersionRestoreFinalization {
                snapshot: Some(snapshot),
                diagnostic: None,
                cleanup_required: false,
            }),
            (cleanup, snapshot) => {
                let mut diagnostics = Vec::new();
                if let Err(error) = cleanup {
                    diagnostics.push(format!(
                        "Commit-ul restaurării a fost publicat, dar marker-ul durabil nu a putut fi eliminat: {error}"
                    ));
                }
                let snapshot = match snapshot {
                    Ok(snapshot) => Some(snapshot),
                    Err(error) => {
                        diagnostics.push(format!(
                            "Commit-ul restaurării a fost publicat, dar snapshotul Git nu a putut fi citit: {error}"
                        ));
                        None
                    }
                };
                Ok(VersionRestoreFinalization {
                    snapshot,
                    diagnostic: Some(format!(
                        "{} Nu repeta restaurarea automat.",
                        diagnostics.join(" ")
                    )),
                    cleanup_required: true,
                })
            }
        }
    }

    pub(crate) fn require_status_token(
        &self,
        expected_status_token: &str,
        expected_head_oid: Option<&str>,
    ) -> Result<VersioningSnapshot, String> {
        let snapshot = self.snapshot()?;
        require_ready(&snapshot)?;
        if snapshot.status_token != expected_status_token {
            return Err(
                "Starea Git s-a schimbat; actualizează panoul Versiuni înainte de operație."
                    .to_string(),
            );
        }
        if snapshot.head_oid.as_deref() != expected_head_oid {
            return Err("HEAD s-a schimbat; operația Git a fost blocată.".to_string());
        }
        Ok(snapshot)
    }

    pub(crate) fn resolve_commit_oid(&self, input: &str) -> Result<String, String> {
        if input.trim().is_empty() || input.starts_with('-') {
            return Err("Commit OID invalid.".to_string());
        }
        let spec = format!("{}^{{commit}}", input.trim());
        let oid = self
            .runner
            .run(["rev-parse", "--verify", &spec])?
            .require_success("Rezolvarea commit-ului Git")?
            .stdout_text()?
            .trim()
            .to_string();
        validate_oid(&oid)?;
        Ok(oid)
    }

    fn optional_config(&self, key: &str) -> Result<Option<String>, String> {
        let output = self.runner.run(["config", "--local", "--get", key])?;
        if output.success() {
            return Ok(nonempty_trimmed(output.stdout_text()?));
        }
        if output.status.code() == Some(1) {
            return Ok(None);
        }
        Err(nonempty_or(
            output.stderr_lossy(),
            &format!("Configurația Git {key} nu poate fi citită."),
        ))
    }

    fn require_no_tracked_filter_attributes(&self) -> Result<(), String> {
        let output = self
            .runner
            .run([
                "ls-files",
                "-z",
                "--cached",
                "--",
                ".gitattributes",
                ":(glob)**/.gitattributes",
            ])?
            .require_success("Inventarierea fișierelor .gitattributes urmărite")?;
        if output.stdout_truncated {
            return Err("Lista .gitattributes urmărite a fost trunchiată.".to_string());
        }
        let paths = output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
            .map(utf8_git_path)
            .collect::<Result<Vec<_>, _>>()?;
        if paths.len() > 256 {
            return Err(
                "Repository-ul conține peste 256 de fișiere .gitattributes; driverele filter/merge nu pot fi auditate în limita sigură."
                    .to_string(),
            );
        }
        for path in paths {
            let object = format!(":{path}");
            let output = self
                .runner
                .run_with_limit(["show", &object], 1024 * 1024)?
                .require_success("Citirea unui fișier .gitattributes urmărit")?;
            if output.stdout_truncated {
                return Err(format!(
                    "{path} depășește limita sigură de audit pentru atribute Git."
                ));
            }
            let source = std::str::from_utf8(&output.stdout)
                .map_err(|_| format!("{path} nu este UTF-8."))?;
            if attributes_define_filter(source) {
                return Err(format!(
                    "Repository-ul este nesuportat: {path} definește atribute filter/merge. Pană Studio nu execută drivere clean/smudge/merge externe."
                ));
            }
        }
        let info_attributes = self.repository_root.join(".git/info/attributes");
        match fs::read(&info_attributes) {
            Ok(bytes) => {
                let source = std::str::from_utf8(&bytes)
                    .map_err(|_| ".git/info/attributes nu este UTF-8.".to_string())?;
                if attributes_define_filter(source) {
                    return Err(
                        "Repository-ul este nesuportat: .git/info/attributes definește drivere Git filter/merge externe."
                            .to_string(),
                    );
                }
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(format!(".git/info/attributes nu poate fi auditat: {error}")),
        }
        Ok(())
    }

    fn require_no_external_clean_filters(&self, paths: &[String]) -> Result<(), String> {
        if paths.is_empty() {
            return Ok(());
        }
        let mut args = vec![
            OsString::from("check-attr"),
            OsString::from("-z"),
            OsString::from("filter"),
            OsString::from("--"),
        ];
        args.extend(paths.iter().map(OsString::from));
        let output = self
            .runner
            .run(args)?
            .require_success("Verificarea filtrelor Git")?;
        if output.stdout_truncated {
            return Err("Verificarea filtrelor Git a fost trunchiată.".to_string());
        }
        let mut fields = output.stdout.split(|byte| *byte == 0);
        loop {
            let Some(path) = fields.next() else { break };
            if path.is_empty() {
                break;
            }
            let attribute = fields
                .next()
                .ok_or_else(|| "Răspuns check-attr incomplet.".to_string())?;
            let value = fields
                .next()
                .ok_or_else(|| "Răspuns check-attr incomplet.".to_string())?;
            if attribute != b"filter" {
                return Err("Răspuns check-attr invalid.".to_string());
            }
            if !matches!(value, b"unspecified" | b"unset") {
                return Err(format!(
                    "Stage a fost blocat pentru {}: atributul Git filter={}. Pană Studio nu execută filtre clean externe.",
                    String::from_utf8_lossy(path),
                    String::from_utf8_lossy(value)
                ));
            }
        }
        Ok(())
    }

    fn terminal_snapshot(
        &self,
        state: VersionRepositoryState,
        diagnostic: Option<String>,
        git_version: Option<String>,
    ) -> VersioningSnapshot {
        let repository_root = self.repository_root_string();
        let state_label = format!("{state:?}");
        let token = status_token(&[
            self.project_root.as_bytes(),
            repository_root.as_bytes(),
            state_label.as_bytes(),
            diagnostic.as_deref().unwrap_or("").as_bytes(),
        ]);
        VersioningSnapshot::terminal(
            self.project_root.clone(),
            repository_root,
            state,
            diagnostic,
            git_version,
            token,
        )
    }

    fn repository_root_string(&self) -> String {
        self.repository_root.to_string_lossy().to_string()
    }
}

fn unsupported_git_metadata(metadata_path: &Path) -> Result<Option<String>, String> {
    for relative in [
        "commondir",
        "objects/info/alternates",
        "objects/info/http-alternates",
    ] {
        match fs::symlink_metadata(metadata_path.join(relative)) {
            Ok(_) => {
                return Ok(Some(format!(
                    "Repository-ul Git folosește metadata externă prin .git/{relative}; această configurație nu este suportată."
                )))
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Metadata critică .git/{relative} nu poate fi verificată: {error}"
                ))
            }
        }
    }
    for relative in [
        "objects",
        "objects/info",
        "refs",
        "info",
        "info/attributes",
        "HEAD",
        "config",
        "index",
        "packed-refs",
    ] {
        match fs::symlink_metadata(metadata_path.join(relative)) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Ok(Some(format!(
                    "Repository-ul Git este nesuportat: .git/{relative} este symlink."
                )))
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Metadata critică .git/{relative} nu poate fi verificată: {error}"
                ))
            }
        }
    }
    let config_path = metadata_path.join("config");
    let config_metadata = fs::metadata(&config_path)
        .map_err(|error| format!("Configurația locală Git nu poate fi inspectată: {error}"))?;
    if config_metadata.len() > 1024 * 1024 {
        return Ok(Some(
            "Configurația locală .git/config depășește limita sigură de 1 MiB.".to_string(),
        ));
    }
    let config = fs::read_to_string(&config_path)
        .map_err(|error| format!("Configurația locală Git nu este UTF-8 valid: {error}"))?;
    if config.lines().any(|line| {
        let section = line
            .trim()
            .chars()
            .filter(|character| !character.is_ascii_whitespace())
            .collect::<String>()
            .to_ascii_lowercase();
        section.starts_with("[include")
    }) {
        return Ok(Some(
            "Repository-ul Git este nesuportat: .git/config include configurație din afara rădăcinii autorizate."
                .to_string(),
        ));
    }
    Ok(None)
}

pub(super) fn attributes_define_filter(source: &str) -> bool {
    attributes_define_external_driver(source)
}

pub(super) fn attributes_define_external_driver(source: &str) -> bool {
    source.lines().any(|line| {
        let line = line.trim_start();
        if line.is_empty() || line.starts_with('#') {
            return false;
        }
        line.split_ascii_whitespace().any(|token| {
            token == "filter"
                || token == "-filter"
                || token.starts_with("filter=")
                || token.starts_with("!filter")
                || token == "merge"
                || token == "-merge"
                || token.starts_with("merge=")
                || token.starts_with("!merge")
        })
    })
}

fn unique_restore_trailer(
    message: &str,
    prefix: &str,
    restore_commit_oid: &str,
) -> Result<String, String> {
    let values = message
        .lines()
        .filter_map(|line| line.strip_prefix(prefix))
        .collect::<Vec<_>>();
    if values.len() != 1 || values[0].is_empty() {
        return Err(format!(
            "Commit-ul intern {restore_commit_oid} trebuie să declare exact o dată trailer-ul {prefix}."
        ));
    }
    Ok(values[0].to_string())
}

fn parse_porcelain_v1_z(bytes: &[u8]) -> Result<Vec<VersionFileStatus>, String> {
    let mut files = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes.len().saturating_sub(cursor) < 4 || bytes[cursor + 2] != b' ' {
            return Err("Statusul porcelain Git are un record invalid.".to_string());
        }
        let index = bytes[cursor] as char;
        let worktree = bytes[cursor + 1] as char;
        cursor += 3;
        let (path_bytes, next) = nul_field(bytes, cursor)?;
        cursor = next;
        let path = utf8_git_path(path_bytes)?;
        let renamed_or_copied = matches!(index, 'R' | 'C') || matches!(worktree, 'R' | 'C');
        let original_path = if renamed_or_copied {
            let (original, next) = nul_field(bytes, cursor)?;
            cursor = next;
            Some(utf8_git_path(original)?)
        } else {
            None
        };
        let conflicted = is_conflicted(index, worktree);
        let staged = !matches!(index, ' ' | '?' | '!');
        let unstaged = !matches!(worktree, ' ' | '!') || index == '?';
        files.push(VersionFileStatus {
            path,
            original_path,
            kind: file_kind(index, worktree, conflicted),
            index_status: index.to_string(),
            worktree_status: worktree.to_string(),
            staged,
            unstaged,
            conflicted,
        });
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

pub(super) fn parse_history(bytes: &[u8]) -> Result<Vec<VersionHistoryEntry>, String> {
    let mut fields = bytes.split(|byte| *byte == 0).collect::<Vec<_>>();
    if fields.last().is_some_and(|field| field.is_empty()) {
        fields.pop();
    }
    if fields.is_empty() {
        return Ok(Vec::new());
    }
    if fields.len() % 7 != 0 {
        return Err("Istoricul Git are un format neașteptat.".to_string());
    }
    fields
        .chunks_exact(7)
        .map(|chunk| {
            let value = |index: usize| {
                String::from_utf8(chunk[index].to_vec())
                    .map_err(|_| "Istoricul Git conține metadata non-UTF-8.".to_string())
            };
            let oid = value(0)?;
            validate_oid(&oid)?;
            Ok(VersionHistoryEntry {
                oid,
                short_oid: value(1)?,
                parent_oids: value(2)?.split_whitespace().map(str::to_string).collect(),
                author_name: value(3)?,
                author_email: value(4)?,
                authored_at: value(5)?,
                subject: value(6)?,
            })
        })
        .collect()
}

fn nul_field(bytes: &[u8], start: usize) -> Result<(&[u8], usize), String> {
    let end = bytes[start..]
        .iter()
        .position(|byte| *byte == 0)
        .map(|offset| start + offset)
        .ok_or_else(|| "Recordul porcelain Git nu este terminat cu NUL.".to_string())?;
    Ok((&bytes[start..end], end + 1))
}

fn utf8_git_path(bytes: &[u8]) -> Result<String, String> {
    let path = String::from_utf8(bytes.to_vec())
        .map_err(|_| "Repository-ul conține un path non-UTF-8 nesuportat.".to_string())?;
    validate_path(&path)
}

fn append_source_pathspecs(args: &mut Vec<OsString>, output_relative: Option<&str>) {
    args.push(OsString::from("--"));
    args.push(OsString::from("."));
    if let Some(output) = output_relative {
        args.push(OsString::from(format!(":(exclude){output}")));
        args.push(OsString::from(format!(":(exclude){output}/**")));
    }
}

fn is_output_path(path: &str, output_relative: &str) -> bool {
    path == output_relative
        || path
            .strip_prefix(output_relative)
            .is_some_and(|tail| tail.starts_with('/'))
}

fn validate_paths(paths: &[String]) -> Result<Vec<String>, String> {
    let mut normalized = paths
        .iter()
        .map(|path| validate_path(path))
        .collect::<Result<Vec<_>, _>>()?;
    normalized.sort();
    normalized.dedup();
    Ok(normalized)
}

pub(super) fn validate_path(path: &str) -> Result<String, String> {
    let normalized = path.trim().replace('\\', "/");
    if normalized.is_empty()
        || normalized.chars().any(char::is_control)
        || normalized.starts_with('-')
    {
        return Err(format!("Path Git invalid: {path:?}."));
    }
    let parsed = Path::new(&normalized);
    if parsed.is_absolute()
        || parsed
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || normalized.split('/').any(|segment| segment == ".git")
    {
        return Err(format!("Path Git în afara  sau rezervat: {path}."));
    }
    Ok(normalized)
}

fn validate_identity(value: &str, label: &str) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() || value.len() > MAX_IDENTITY_BYTES || value.contains(['\n', '\r', '\0']) {
        return Err(format!("{label} este gol sau invalid."));
    }
    Ok(())
}

pub(super) fn validate_commit_message(message: &str) -> Result<String, String> {
    let message = message.trim();
    if message.is_empty() {
        return Err("Mesajul commit-ului este obligatoriu.".to_string());
    }
    if message.len() > MAX_COMMIT_MESSAGE_BYTES || message.contains('\0') {
        return Err("Mesajul commit-ului depășește limita sau conține NUL.".to_string());
    }
    Ok(message.to_string())
}

pub(super) fn validate_oid(oid: &str) -> Result<(), String> {
    if !matches!(oid.len(), 40 | 64) || !oid.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("Git a returnat un OID invalid: {oid:?}."));
    }
    Ok(())
}

pub(super) fn zero_oid(object_format: Option<&str>) -> String {
    "0".repeat(if object_format == Some("sha256") {
        64
    } else {
        40
    })
}

pub(super) fn require_ready(snapshot: &VersioningSnapshot) -> Result<(), String> {
    if snapshot.repository_state == VersionRepositoryState::Ready {
        return Ok(());
    }
    Err(snapshot
        .diagnostic
        .clone()
        .unwrap_or_else(|| "Repository-ul Git nu este pregătit.".to_string()))
}

fn status_token(parts: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update((part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    format!("{:x}", hasher.finalize())
}

fn file_kind(index: char, worktree: char, conflicted: bool) -> VersionFileKind {
    if conflicted {
        return VersionFileKind::Conflicted;
    }
    let status = if worktree != ' ' { worktree } else { index };
    match status {
        'A' => VersionFileKind::Added,
        'M' => VersionFileKind::Modified,
        'D' => VersionFileKind::Deleted,
        'R' => VersionFileKind::Renamed,
        'C' => VersionFileKind::Copied,
        'T' => VersionFileKind::TypeChanged,
        '?' => VersionFileKind::Untracked,
        _ => VersionFileKind::Unknown,
    }
}

fn is_conflicted(index: char, worktree: char) -> bool {
    matches!(
        (index, worktree),
        ('D', 'D') | ('A', 'U') | ('U', 'D') | ('U', 'A') | ('D', 'U') | ('A', 'A') | ('U', 'U')
    )
}

fn nonempty_trimmed(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn nonempty_or(value: String, fallback: &str) -> String {
    nonempty_trimmed(value).unwrap_or_else(|| fallback.to_string())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "pana-versioning-{label}-{}-{sequence}",
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

    #[test]
    fn parses_staged_unstaged_untracked_and_conflicted_porcelain() {
        let source =
            b"M  templates/a.html\0 M sass/a.scss\0?? static/new.png\0UU content/conflict.md\0";
        let files = parse_porcelain_v1_z(source).unwrap();
        assert_eq!(files.len(), 4);
        assert!(files
            .iter()
            .any(|file| file.path == "templates/a.html" && file.staged));
        assert!(files
            .iter()
            .any(|file| file.path == "sass/a.scss" && file.unstaged));
        assert!(files
            .iter()
            .any(|file| file.path == "static/new.png" && file.kind == VersionFileKind::Untracked));
        assert!(files
            .iter()
            .any(|file| file.path == "content/conflict.md" && file.conflicted));
    }

    #[test]
    fn parses_porcelain_rename_extra_nul_field() {
        let files = parse_porcelain_v1_z(b"R  templates/new.html\0templates/old.html\0").unwrap();
        assert_eq!(files[0].path, "templates/new.html");
        assert_eq!(
            files[0].original_path.as_deref(),
            Some("templates/old.html")
        );
        assert_eq!(files[0].kind, VersionFileKind::Renamed);
    }

    #[test]
    fn paths_cannot_escape_or_address_git_metadata() {
        for path in ["../secret", "/absolute", ".git/config", "templates/.git/x"] {
            assert!(validate_path(path).is_err(), "{path}");
        }
        assert_eq!(
            validate_path("templates/index.html").unwrap(),
            "templates/index.html"
        );
    }

    #[test]
    fn status_token_is_length_delimited() {
        assert_ne!(status_token(&[b"ab", b"c"]), status_token(&[b"a", b"bc"]));
    }

    #[test]
    fn repository_lifecycle_commit_history_and_diff_use_the_manual_pipeline() {
        let directory = TestDirectory::new("lifecycle");
        let repository = repository(&directory.0);
        assert_eq!(
            repository.snapshot().unwrap().repository_state,
            VersionRepositoryState::Uninitialized
        );

        let initialized = repository.initialize().unwrap();
        assert_eq!(initialized.repository_state, VersionRepositoryState::Ready);
        assert!(initialized.unborn_head);
        repository
            .configure_identity("Pană Test", "pana@example.test")
            .unwrap();

        fs::create_dir_all(directory.0.join("templates")).unwrap();
        fs::write(directory.0.join("templates/index.html"), "<h1>Prima</h1>\n").unwrap();
        let untracked = repository.snapshot().unwrap();
        assert_eq!(untracked.unstaged_count, 1);
        assert!(repository
            .require_status_token(&untracked.status_token, None)
            .is_ok());

        let staged = repository.stage_all().unwrap();
        assert_eq!(staged.staged_count, 1);
        let commit = repository.commit("Versiunea inițială", None).unwrap();
        assert_eq!(
            commit.publication_status,
            VersionPublicationStatus::Published
        );
        let head = commit.commit_oid.clone();
        assert_eq!(
            commit
                .snapshot
                .as_ref()
                .and_then(|item| item.head_oid.as_ref()),
            Some(&head)
        );
        let tree = repository.read_tree(&head).unwrap();
        assert_eq!(tree.commit_oid, head);
        assert_eq!(tree.files.len(), 1);
        assert_eq!(tree.files[0].path, "templates/index.html");
        assert_eq!(tree.files[0].bytes, b"<h1>Prima</h1>\n");

        fs::write(
            directory.0.join("templates/index.html"),
            "<h1>A doua</h1>\n",
        )
        .unwrap();
        let changed = repository.snapshot().unwrap();
        let diff = repository
            .diff(&VersionDiffInput {
                kind: VersionDiffKind::Unstaged,
                path: Some("templates/index.html".to_string()),
                commit_oid: None,
                target_ref: None,
                expected_target_oid: None,
            })
            .unwrap();
        assert!(diff.patch.contains("Prima"));
        assert!(diff.patch.contains("A doua"));
        assert!(repository
            .require_status_token(&staged.status_token, Some(&head))
            .is_err());
        assert!(repository
            .require_status_token(&changed.status_token, Some(&head))
            .is_ok());

        let history = repository.history(0, 20).unwrap();
        assert_eq!(history.entries.len(), 1);
        assert_eq!(history.entries[0].oid, head);
        assert_eq!(history.entries[0].subject, "Versiunea inițială");
    }

    #[test]
    fn restore_is_published_as_a_new_descendant_after_durable_preparation() {
        let directory = TestDirectory::new("restore-descendant");
        let repository = repository(&directory.0);
        repository.initialize().unwrap();
        repository
            .configure_identity("Pană Test", "pana@example.test")
            .unwrap();

        fs::write(directory.0.join("index.html"), "prima\n").unwrap();
        repository.stage_all().unwrap();
        let first = repository.commit("Prima", None).unwrap();
        fs::write(directory.0.join("index.html"), "a doua\n").unwrap();
        repository.stage_all().unwrap();
        let second = repository
            .commit("A doua", Some(&first.commit_oid))
            .unwrap();
        let target = repository.read_tree(&first.commit_oid).unwrap();

        let prepared = repository
            .prepare_restore(&target, "Restaurare prima", &second.commit_oid)
            .unwrap();
        let markers = repository.read_restore_markers().unwrap();
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].recovery_ref, prepared.recovery_ref);
        assert_eq!(markers[0].target_commit_oid, first.commit_oid);
        assert_eq!(markers[0].previous_head_oid, second.commit_oid);

        // This write models the already verified ProjectWorkspace Save. Git
        // only aligns its index and branch after source publication succeeded.
        fs::write(directory.0.join("index.html"), "prima\n").unwrap();
        let finalization = repository.finalize_restore(&prepared).unwrap();
        assert!(!finalization.cleanup_required);
        let snapshot = finalization.snapshot.unwrap();
        assert!(snapshot.clean);
        assert_eq!(
            snapshot.head_oid.as_deref(),
            Some(prepared.restore_commit_oid.as_str())
        );
        assert!(repository.read_restore_markers().unwrap().is_empty());
        let restored_tree = repository
            .read_tree(snapshot.head_oid.as_deref().unwrap())
            .unwrap();
        assert_eq!(restored_tree.tree_oid, target.tree_oid);
        let history = repository.history(0, 10).unwrap();
        assert_eq!(history.entries.len(), 3);
        assert_eq!(history.entries[0].parent_oids, vec![second.commit_oid]);
    }

    #[test]
    fn prepared_restore_can_be_rolled_back_without_rewriting_history() {
        let directory = TestDirectory::new("restore-abort");
        let repository = repository(&directory.0);
        repository.initialize().unwrap();
        repository
            .configure_identity("Pană Test", "pana@example.test")
            .unwrap();
        fs::write(directory.0.join("index.html"), "prima\n").unwrap();
        repository.stage_all().unwrap();
        let first = repository.commit("Prima", None).unwrap();
        fs::write(directory.0.join("index.html"), "a doua\n").unwrap();
        repository.stage_all().unwrap();
        let second = repository
            .commit("A doua", Some(&first.commit_oid))
            .unwrap();
        let target = repository.read_tree(&first.commit_oid).unwrap();
        let prepared = repository
            .prepare_restore(&target, "Restaurare întreruptă", &second.commit_oid)
            .unwrap();

        let snapshot = repository.abort_prepared_restore(&prepared).unwrap();
        assert!(snapshot.clean);
        assert_eq!(
            snapshot.head_oid.as_deref(),
            Some(second.commit_oid.as_str())
        );
        assert!(repository.read_restore_markers().unwrap().is_empty());
        assert_eq!(repository.history(0, 10).unwrap().entries.len(), 2);
    }

    #[test]
    fn recovery_finalization_cleans_marker_when_restore_commit_is_already_head() {
        let directory = TestDirectory::new("restore-published-cleanup");
        let repository = repository(&directory.0);
        repository.initialize().unwrap();
        repository
            .configure_identity("Pană Test", "pana@example.test")
            .unwrap();
        fs::write(directory.0.join("index.html"), "prima\n").unwrap();
        repository.stage_all().unwrap();
        let first = repository.commit("Prima", None).unwrap();
        fs::write(directory.0.join("index.html"), "a doua\n").unwrap();
        repository.stage_all().unwrap();
        let second = repository
            .commit("A doua", Some(&first.commit_oid))
            .unwrap();
        let target = repository.read_tree(&first.commit_oid).unwrap();
        let prepared = repository
            .prepare_restore(&target, "Restaurare publicată", &second.commit_oid)
            .unwrap();

        fs::write(directory.0.join("index.html"), "prima\n").unwrap();
        repository
            .runner
            .run(["read-tree", &target.tree_oid])
            .unwrap()
            .require_success("test read-tree")
            .unwrap();
        repository
            .runner
            .run([
                "update-ref",
                &prepared.full_head_ref,
                &prepared.restore_commit_oid,
                &prepared.previous_head_oid,
            ])
            .unwrap()
            .require_success("test update-ref")
            .unwrap();

        let finalization = repository.finalize_restore(&prepared).unwrap();
        assert!(!finalization.cleanup_required);
        assert!(finalization.snapshot.unwrap().clean);
        assert!(repository.read_restore_markers().unwrap().is_empty());
    }

    #[test]
    fn divergent_head_preserves_restore_marker_for_manual_recovery() {
        let directory = TestDirectory::new("restore-divergent-head");
        let repository = repository(&directory.0);
        repository.initialize().unwrap();
        repository
            .configure_identity("Pană Test", "pana@example.test")
            .unwrap();
        fs::write(directory.0.join("index.html"), "prima\n").unwrap();
        repository.stage_all().unwrap();
        let first = repository.commit("Prima", None).unwrap();
        fs::write(directory.0.join("index.html"), "a doua\n").unwrap();
        repository.stage_all().unwrap();
        let second = repository
            .commit("A doua", Some(&first.commit_oid))
            .unwrap();
        let target = repository.read_tree(&first.commit_oid).unwrap();
        let prepared = repository
            .prepare_restore(&target, "Restaurare concurentă", &second.commit_oid)
            .unwrap();

        fs::write(directory.0.join("index.html"), "a treia\n").unwrap();
        repository.stage_all().unwrap();
        repository
            .commit("A treia", Some(&second.commit_oid))
            .unwrap();
        let error = repository.finalize_restore(&prepared).unwrap_err();
        assert!(error.contains("HEAD a divergat"), "{error}");
        assert_eq!(repository.read_restore_markers().unwrap().len(), 1);
    }

    #[test]
    fn parent_repository_is_not_adopted_for_a_source_child() {
        let directory = TestDirectory::new("parent-root");
        let parent = repository(&directory.0);
        parent.initialize().unwrap();
        let child = directory.0.join("site");
        fs::create_dir_all(&child).unwrap();

        let snapshot = repository(&child).snapshot().unwrap();
        assert_eq!(
            snapshot.repository_state,
            VersionRepositoryState::Uninitialized
        );
    }

    #[test]
    fn generated_output_is_excluded_from_status_and_stage_all() {
        for (label, config, output) in [
            ("git-default-output", "base_url = '/'\n", "public"),
            (
                "git-custom-output",
                "base_url = '/'\noutput_dir = 'generated/site'\n",
                "generated/site",
            ),
        ] {
            let directory = TestDirectory::new(label);
            fs::create_dir_all(directory.0.join("templates")).unwrap();
            fs::create_dir_all(directory.0.join(output)).unwrap();
            fs::write(directory.0.join("zola.toml"), config).unwrap();
            fs::write(directory.0.join("templates/index.html"), "source").unwrap();
            fs::write(directory.0.join(output).join("index.html"), "generated").unwrap();
            let repository = repository(&directory.0);
            repository.initialize().unwrap();

            let before = repository.snapshot().unwrap();
            assert!(before
                .files
                .iter()
                .all(|file| !is_output_path(&file.path, output)));
            let after = repository.stage_all().unwrap();
            assert!(after
                .files
                .iter()
                .all(|file| !is_output_path(&file.path, output)));
            let tracked = repository
                .runner
                .run(["ls-files", "-z", "--", output])
                .unwrap()
                .require_success("test ls-files")
                .unwrap();
            assert!(tracked.stdout.is_empty());
        }
    }

    #[test]
    fn already_tracked_generated_output_is_refused_without_migration() {
        let directory = TestDirectory::new("git-tracked-output");
        fs::create_dir_all(directory.0.join("public")).unwrap();
        fs::write(directory.0.join("zola.toml"), "base_url = '/'\n").unwrap();
        fs::write(directory.0.join("public/index.html"), "generated").unwrap();
        let repository = repository(&directory.0);
        repository.initialize().unwrap();
        repository
            .runner
            .run(["add", "-f", "--", "public/index.html"])
            .unwrap()
            .require_success("test force-add output")
            .unwrap();

        let snapshot = repository.snapshot().unwrap();

        assert_eq!(
            snapshot.repository_state,
            VersionRepositoryState::Unsupported
        );
        assert!(snapshot
            .diagnostic
            .as_deref()
            .is_some_and(|message| message.contains("nu migrează")));
    }

    #[test]
    fn historical_legacy_layout_is_refused_instead_of_restored() {
        let directory = TestDirectory::new("git-legacy-history");
        let repository = repository(&directory.0);
        repository.initialize().unwrap();
        repository
            .configure_identity("Pană Test", "pana@example.test")
            .unwrap();
        fs::create_dir_all(directory.0.join("sursa/content")).unwrap();
        fs::write(directory.0.join("sursa/zola.toml"), "base_url = '/'\n").unwrap();
        repository.stage_all().unwrap();
        let legacy = repository.commit("Legacy", None).unwrap();

        fs::remove_dir_all(directory.0.join("sursa")).unwrap();
        fs::create_dir_all(directory.0.join("content")).unwrap();
        fs::write(directory.0.join("zola.toml"), "base_url = '/'\n").unwrap();
        repository.stage_all().unwrap();
        repository
            .commit("Direct root", Some(&legacy.commit_oid))
            .unwrap();

        let error = repository.read_tree(&legacy.commit_oid).unwrap_err();
        assert!(error.contains("structura veche"), "{error}");
        assert!(error.contains("nu migrează"), "{error}");
    }

    #[test]
    fn historical_generated_output_is_refused_instead_of_restored() {
        let directory = TestDirectory::new("git-output-history");
        let repository = repository(&directory.0);
        repository.initialize().unwrap();
        repository
            .configure_identity("Pană Test", "pana@example.test")
            .unwrap();
        fs::create_dir_all(directory.0.join("public")).unwrap();
        fs::write(directory.0.join("zola.toml"), "base_url = '/'\n").unwrap();
        fs::write(directory.0.join("public/index.html"), "generated").unwrap();
        repository
            .runner
            .run(["add", "-f", "--", "zola.toml", "public/index.html"])
            .unwrap()
            .require_success("test force-add historical output")
            .unwrap();
        repository
            .runner
            .run(["commit", "-m", "Historical output"])
            .unwrap()
            .require_success("test commit historical output")
            .unwrap();
        let historical_oid = repository
            .runner
            .run(["rev-parse", "HEAD"])
            .unwrap()
            .require_success("test read historical oid")
            .unwrap()
            .stdout_text()
            .unwrap()
            .trim()
            .to_string();
        fs::remove_file(directory.0.join("public/index.html")).unwrap();
        repository
            .runner
            .run(["add", "-A", "--", "."])
            .unwrap()
            .require_success("test remove historical output")
            .unwrap();
        repository
            .runner
            .run(["commit", "-m", "Remove generated output"])
            .unwrap()
            .require_success("test commit output removal")
            .unwrap();

        let error = repository.read_tree(&historical_oid).unwrap_err();
        assert!(error.contains("output-ul Zola generat"), "{error}");
        assert!(error.contains("nu îl previzualizează"), "{error}");
    }

    #[test]
    fn gitdir_file_is_rejected_instead_of_followed() {
        let directory = TestDirectory::new("gitdir-file");
        fs::write(directory.0.join(".git"), "gitdir: /tmp/not-authorized\n").unwrap();
        let snapshot = repository(&directory.0).snapshot().unwrap();
        assert_eq!(
            snapshot.repository_state,
            VersionRepositoryState::Unsupported
        );
    }

    #[test]
    fn external_object_alternates_are_rejected() {
        let directory = TestDirectory::new("object-alternates");
        let repository = repository(&directory.0);
        repository.initialize().unwrap();
        fs::write(
            directory.0.join(".git/objects/info/alternates"),
            "/tmp/external-objects\n",
        )
        .unwrap();
        let snapshot = repository.snapshot().unwrap();
        assert_eq!(
            snapshot.repository_state,
            VersionRepositoryState::Unsupported
        );
        assert!(snapshot.diagnostic.unwrap().contains("alternates"));
    }

    #[test]
    fn local_config_includes_are_rejected() {
        let directory = TestDirectory::new("config-include");
        let repository = repository(&directory.0);
        repository.initialize().unwrap();
        let config_path = directory.0.join(".git/config");
        let mut config = fs::read_to_string(&config_path).unwrap();
        config.push_str("\n[include]\n\tpath = /tmp/external-git-config\n");
        fs::write(config_path, config).unwrap();
        let snapshot = repository.snapshot().unwrap();
        assert_eq!(
            snapshot.repository_state,
            VersionRepositoryState::Unsupported
        );
        assert!(snapshot.diagnostic.unwrap().contains("include"));
    }

    #[test]
    fn local_identity_is_part_of_the_status_token() {
        let directory = TestDirectory::new("identity-token");
        let repository = repository(&directory.0);
        let before = repository.initialize().unwrap();
        let after = repository
            .configure_identity("Pană Test", "pana@example.test")
            .unwrap();
        assert_ne!(before.status_token, after.status_token);
    }

    #[test]
    fn stage_refuses_repository_clean_filters() {
        let directory = TestDirectory::new("clean-filter");
        let repository = repository(&directory.0);
        repository.initialize().unwrap();
        fs::write(directory.0.join(".gitattributes"), "*.txt filter=danger\n").unwrap();
        fs::write(directory.0.join("payload.txt"), "payload\n").unwrap();

        let error = repository.stage_all().unwrap_err();
        assert!(error.contains("filter=danger"), "{error}");
        assert_eq!(repository.snapshot().unwrap().staged_count, 0);
    }

    #[cfg(unix)]
    #[test]
    fn historical_tree_rejects_symlinks_before_materialization() {
        use std::os::unix::fs::symlink;

        let directory = TestDirectory::new("tree-symlink");
        let repository = repository(&directory.0);
        repository.initialize().unwrap();
        repository
            .configure_identity("Pană Test", "pana@example.test")
            .unwrap();
        fs::write(directory.0.join("target.txt"), "target\n").unwrap();
        symlink("target.txt", directory.0.join("link.txt")).unwrap();
        let staged = repository.stage_all().unwrap();
        let commit = repository.commit("Versiune cu symlink", None).unwrap();
        assert_eq!(staged.staged_count, 2);

        let error = repository.read_tree(&commit.commit_oid).unwrap_err();
        assert!(error.contains("Symlink-urile"), "{error}");
    }
}
