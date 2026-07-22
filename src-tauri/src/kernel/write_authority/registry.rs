use std::path::{Component, Path};

use serde::Serialize;

use super::model::{
    ConflictPolicy, RecoveryPolicy, WriteAtomicity, WriteCategory, WriteIntent, WriteOperationKind,
    WriteOwner,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteDeclaration {
    pub category: WriteCategory,
    pub owner: WriteOwner,
    pub operations: Vec<WriteOperationKind>,
    pub path_authority: &'static str,
    pub atomicity: WriteAtomicity,
    pub conflict: ConflictPolicy,
    pub recovery: RecoveryPolicy,
    pub validation: &'static str,
}

impl WriteDeclaration {
    pub fn matches_intent(&self, intent: &WriteIntent) -> bool {
        self.category == intent.category
            && self.owner == intent.owner
            && self.operations.contains(&intent.operation)
            && self.atomicity == intent.policy.atomicity
            && self.conflict == intent.policy.conflict
            && self.recovery == intent.policy.recovery
            && intent.policy.log_required
    }
}

pub fn matching_write_declaration(intent: &WriteIntent) -> Option<WriteDeclaration> {
    known_write_declarations()
        .into_iter()
        .find(|declaration| declaration.matches_intent(intent))
}

pub(super) fn validate_authority_path(intent: &WriteIntent) -> Result<(), String> {
    let relative = intent
        .target
        .path
        .strip_prefix(&intent.target.boundary_root)
        .map_err(|_| {
            format!(
                "Write Registry a refuzat {}: target-ul nu aparține root-ului autoritar.",
                intent.target.public_label
            )
        })?;
    let segments = utf8_segments(relative)?;
    let valid = match intent.category {
        WriteCategory::InternalAppWrite => validate_internal_path(intent.owner, &segments),
        WriteCategory::PreviewWorkspaceWrite => validate_preview_path(&segments),
        WriteCategory::BuildOutputWrite => validate_build_output_path(&segments),
        WriteCategory::ExternalIntegrationWrite => {
            intent.owner == WriteOwner::CodexMcp && segments.as_slice() == ["config.toml"]
        }
        WriteCategory::ProjectSourceWrite | WriteCategory::ProjectDesignWrite => {
            validate_project_path(intent, &segments)
        }
    };
    if valid {
        return Ok(());
    }
    Err(format!(
        "Write Registry a refuzat ruta concretă {} pentru {:?}/{:?}/{:?}; path_authority nu este satisfăcută.",
        intent.target.path.display(), intent.category, intent.owner, intent.operation
    ))
}

pub(super) fn validate_companion_authority_path(
    intent: &WriteIntent,
    companion: &super::model::WriteTarget,
) -> Result<(), String> {
    let relative = companion
        .path
        .strip_prefix(&companion.boundary_root)
        .map_err(|_| "Write Registry companion target este în afara authority root.".to_string())?;
    let segments = utf8_segments(relative)?;
    let valid = match (intent.category, intent.owner, intent.operation) {
        (
            WriteCategory::ExternalIntegrationWrite,
            WriteOwner::CodexMcp,
            WriteOperationKind::ExternalConfigUpdate,
        ) => {
            segments.len() == 1
                && segments[0].starts_with("config.toml.pana-studio-")
                && segments[0].ends_with(".bak")
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(format!(
            "Write Registry a refuzat companion target {} pentru {:?}/{:?}/{:?}.",
            companion.path.display(),
            intent.category,
            intent.owner,
            intent.operation
        ))
    }
}

fn utf8_segments(path: &Path) -> Result<Vec<&str>, String> {
    path.components()
        .map(|component| match component {
            Component::Normal(value) => value.to_str().ok_or_else(|| {
                "Write Registry cere segmente UTF-8 pentru rutele declarate.".to_string()
            }),
            _ => Err("Write Registry a detectat traversal/non-normal path component.".to_string()),
        })
        .collect()
}

fn validate_internal_path(owner: WriteOwner, segments: &[&str]) -> bool {
    match owner {
        WriteOwner::AppConfig => {
            segments == ["config.json"]
                || (segments.len() == 2
                    && segments[0] == "projects"
                    && is_hex_json_name(segments[1]))
        }
        WriteOwner::McpContext => {
            matches!(
                segments,
                ["mcp", "current-context.json"] | ["mcp", "mcp.json"]
            )
        }
        WriteOwner::ScratchState => {
            segments.len() == 3
                && segments[0] == "scratch"
                && !segments[1].is_empty()
                && segments[2].ends_with(".json")
        }
        WriteOwner::ProjectSession => session_tail(segments) == Some(&["manifest.json"][..]),
        WriteOwner::Workbench => session_tail(segments) == Some(&["workbench.json"][..]),
        WriteOwner::ProjectWorkspace => {
            session_tail(segments) == Some(&["project-workspace.json"][..])
                || session_tail(segments) == Some(&["project-open-recovery-decision.json"][..])
                || matches!(session_tail(segments), Some(["project-workspace-save", file]) if file.ends_with(".json"))
        }
        WriteOwner::Kernel => validate_kernel_session_tail(session_tail(segments)),
        _ => false,
    }
}

fn session_tail<'a>(segments: &'a [&str]) -> Option<&'a [&'a str]> {
    if segments.len() < 3 || segments.first().copied() != Some("sessions") {
        return None;
    }
    if segments[1].is_empty() {
        return None;
    }
    Some(&segments[2..])
}

fn validate_kernel_session_tail(tail: Option<&[&str]>) -> bool {
    matches!(
        tail,
        Some(["project-transition-decisions.jsonl"])
            | Some(["project-transition-decision-recovery-acknowledgements.jsonl"])
    ) || matches!(
        tail,
        Some(["project-transition-decision-retention", file]) if file.ends_with(".json")
    ) || matches!(
        tail,
        Some(["project-transition-decision-retention", "archives", file]) if file.ends_with(".jsonl")
    )
}

fn validate_preview_path(segments: &[&str]) -> bool {
    let Some(workspace) = segments.first() else {
        return false;
    };
    workspace.starts_with("project-") || workspace.starts_with("template-sandbox-")
}

fn validate_build_output_path(segments: &[&str]) -> bool {
    segments.len() >= 2 && segments[0] == "sursa"
}

fn validate_project_path(intent: &WriteIntent, segments: &[&str]) -> bool {
    match intent.owner {
        WriteOwner::ProjectInitializer => match intent.category {
            WriteCategory::ProjectDesignWrite => segments.first() == Some(&"design"),
            WriteCategory::ProjectSourceWrite => !segments.is_empty(),
            _ => false,
        },
        WriteOwner::MoodBoard => match intent.operation {
            WriteOperationKind::WriteText => segments == ["design", "mood-board.json"],
            WriteOperationKind::WriteBytes => {
                segments.len() >= 3
                    && matches!(
                        &segments[0..2],
                        ["design", "imagini"] | ["resurse", "imagini"]
                    )
            }
            _ => false,
        },
        WriteOwner::ProjectWorkspace => {
            !segments.is_empty()
                && match intent.category {
                    WriteCategory::ProjectDesignWrite => segments.first() == Some(&"design"),
                    WriteCategory::ProjectSourceWrite => segments.first() != Some(&"design"),
                    _ => false,
                }
        }
        _ => false,
    }
}

fn is_hex_json_name(value: &str) -> bool {
    value
        .strip_suffix(".json")
        .is_some_and(|stem| stem.len() == 16 && stem.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

pub fn known_write_declarations() -> Vec<WriteDeclaration> {
    vec![
        WriteDeclaration {
            category: WriteCategory::InternalAppWrite,
            owner: WriteOwner::ProjectSession,
            operations: vec![WriteOperationKind::WriteText],
            path_authority: "ApplicationHome.data/sessions/[project-id]/manifest.json",
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::LoggedAtomicFile,
            validation:
                "Project session manifests stay inside the session directory and are logged.",
        },
        WriteDeclaration {
            category: WriteCategory::InternalAppWrite,
            owner: WriteOwner::Workbench,
            operations: vec![WriteOperationKind::WriteText],
            path_authority: "ApplicationHome.data/sessions/[project-id]/workbench.json",
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::EphemeralRebuildable,
            validation:
                "Workbench projections are bounded, project-scoped navigation state written atomically under the stable ProjectSession directory; they can be rebuilt without touching project content.",
        },
        WriteDeclaration {
            category: WriteCategory::InternalAppWrite,
            owner: WriteOwner::ScratchState,
            operations: vec![WriteOperationKind::WriteText],
            path_authority: "ApplicationHome.cache/scratch/[namespace]/[key].json",
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::EphemeralRebuildable,
            validation:
                "Scratch state entries are bounded, normalized JSON/text records under Application Home cache and can be rebuilt instead of recovered as source of truth.",
        },
        WriteDeclaration {
            category: WriteCategory::InternalAppWrite,
            owner: WriteOwner::ScratchState,
            operations: vec![WriteOperationKind::RemoveFile],
            path_authority: "ApplicationHome.cache/scratch/[namespace]/[key].json",
            atomicity: WriteAtomicity::FileLifecycle,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::EphemeralRebuildable,
            validation:
                "Scratch state cleanup removes only normalized scratch entries inside Application Home cache after symlink/directory preflight.",
        },
        WriteDeclaration {
            category: WriteCategory::InternalAppWrite,
            owner: WriteOwner::Kernel,
            operations: vec![WriteOperationKind::AppendText],
            path_authority:
                "ApplicationHome.data/sessions/[project-id]/project-transition-decisions.jsonl",
            atomicity: WriteAtomicity::AppendOnly,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::AppendOnlyJournal,
            validation:
                "ProjectTransition operator decisions append evidence-hashed metadata records under ProjectSession and remain consumable only on exact evidence match.",
        },
        WriteDeclaration {
            category: WriteCategory::InternalAppWrite,
            owner: WriteOwner::Kernel,
            operations: vec![WriteOperationKind::AppendText],
            path_authority:
                "ApplicationHome.data/sessions/[project-id]/project-transition-decision-recovery-acknowledgements.jsonl",
            atomicity: WriteAtomicity::AppendOnly,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::AppendOnlyJournal,
            validation:
                "ProjectTransition Decision recovery acknowledgements append metadata-only records tied to recoveryPlan.evidenceHash; they never mutate the original Decision Journal or execute retention.",
        },
        WriteDeclaration {
            category: WriteCategory::InternalAppWrite,
            owner: WriteOwner::Kernel,
            operations: vec![WriteOperationKind::WriteText],
            path_authority:
                "ApplicationHome.data/sessions/[project-id]/project-transition-decision-retention/[retention-id].json",
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::HotRollbackJournal,
            validation:
                "ProjectTransition Decision retention writes a hot rollback journal with before/after/archive hashes and full before text before mutating the active Decision Journal.",
        },
        WriteDeclaration {
            category: WriteCategory::InternalAppWrite,
            owner: WriteOwner::Kernel,
            operations: vec![WriteOperationKind::WriteText],
            path_authority:
                "ApplicationHome.data/sessions/[project-id]/project-transition-decision-retention/archives/[retention-id].jsonl",
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::HotRollbackJournal,
            validation:
                "Superseded ProjectTransition Decision records are archived under ProjectSession before the active journal is compacted.",
        },
        WriteDeclaration {
            category: WriteCategory::InternalAppWrite,
            owner: WriteOwner::Kernel,
            operations: vec![WriteOperationKind::WriteText],
            path_authority:
                "ApplicationHome.data/sessions/[project-id]/project-transition-decisions.jsonl",
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::HotRollbackJournal,
            validation:
                "ProjectTransition Decision retention may rewrite the active Decision Journal only after a matching retention acknowledgement, archive write and hot rollback journal.",
        },
        WriteDeclaration {
            category: WriteCategory::InternalAppWrite,
            owner: WriteOwner::Kernel,
            operations: vec![WriteOperationKind::RemoveFile],
            path_authority:
                "ApplicationHome.data/sessions/[project-id]/project-transition-decision-retention/[retention-id].json",
            atomicity: WriteAtomicity::FileLifecycle,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::HotRollbackJournal,
            validation:
                "ProjectTransition Decision retention clears its hot journal only after committed retention or explicit recovery classification.",
        },
        WriteDeclaration {
            category: WriteCategory::InternalAppWrite,
            owner: WriteOwner::ProjectWorkspace,
            operations: vec![WriteOperationKind::WriteText],
            path_authority:
                "ApplicationHome.data/sessions/[project-id]/{project-workspace.json|project-open-recovery-decision.json}",
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::LoggedAtomicFile,
            validation:
                "ProjectWorkspace persists its bounded recovery snapshot and explicit open-recovery decisions atomically.",
        },
        WriteDeclaration {
            category: WriteCategory::InternalAppWrite,
            owner: WriteOwner::ProjectWorkspace,
            operations: vec![WriteOperationKind::RemoveFile],
            path_authority:
                "ApplicationHome.data/sessions/[project-id]/{project-workspace.json|project-open-recovery-decision.json}",
            atomicity: WriteAtomicity::FileLifecycle,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::LoggedAtomicFile,
            validation:
                "ProjectWorkspace clears recovery state only after the session baseline is accepted or explicitly discarded.",
        },
        WriteDeclaration {
            category: WriteCategory::InternalAppWrite,
            owner: WriteOwner::ProjectWorkspace,
            operations: vec![WriteOperationKind::WriteText],
            path_authority:
                "ApplicationHome.data/sessions/[project-id]/project-workspace-save/[save-id].json",
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::HotRollbackJournal,
            validation:
                "ProjectWorkspace writes a durable Save rollback journal before the first project-disk mutation.",
        },
        WriteDeclaration {
            category: WriteCategory::InternalAppWrite,
            owner: WriteOwner::ProjectWorkspace,
            operations: vec![WriteOperationKind::RemoveFile],
            path_authority:
                "ApplicationHome.data/sessions/[project-id]/project-workspace-save/[save-id].json",
            atomicity: WriteAtomicity::FileLifecycle,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::HotRollbackJournal,
            validation:
                "ProjectWorkspace removes a Save rollback journal only after commit or complete controlled rollback.",
        },
        WriteDeclaration {
            category: WriteCategory::InternalAppWrite,
            owner: WriteOwner::AppConfig,
            operations: vec![WriteOperationKind::WriteText],
            path_authority: "ApplicationHome.config/projects",
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::LoggedAtomicFile,
            validation: "App config writes stay inside Application Home config and are logged.",
        },
        WriteDeclaration {
            category: WriteCategory::InternalAppWrite,
            owner: WriteOwner::McpContext,
            operations: vec![WriteOperationKind::WriteText],
            path_authority: "ApplicationHome.config/mcp",
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::EphemeralRebuildable,
            validation: "MCP context and discovery files stay under Application Home MCP and are regenerated instead of blocking project writes after a crash.",
        },
        WriteDeclaration {
            category: WriteCategory::ExternalIntegrationWrite,
            owner: WriteOwner::CodexMcp,
            operations: vec![WriteOperationKind::ExternalConfigUpdate],
            path_authority: "External tool home: ~/.codex/config.toml",
            atomicity: WriteAtomicity::ExternalToolWrite,
            conflict: ConflictPolicy::ExternalBackupRequired,
            recovery: RecoveryPolicy::BackupBeforeWrite,
            validation: "External config writes create a backup and log the operation.",
        },
        WriteDeclaration {
            category: WriteCategory::ProjectSourceWrite,
            owner: WriteOwner::ProjectWorkspace,
            operations: vec![WriteOperationKind::WriteText, WriteOperationKind::WriteBytes],
            path_authority: "ProjectSession project root",
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::RequireDiskBaseline,
            recovery: RecoveryPolicy::HotRollbackJournal,
            validation:
                "ProjectWorkspace writes source text or binary resources only from a validated active Save journal with an exact live disk baseline.",
        },
        WriteDeclaration {
            category: WriteCategory::ProjectSourceWrite,
            owner: WriteOwner::ProjectWorkspace,
            operations: vec![WriteOperationKind::RemoveFile],
            path_authority: "ProjectSession project root",
            atomicity: WriteAtomicity::FileLifecycle,
            conflict: ConflictPolicy::RequireDiskBaseline,
            recovery: RecoveryPolicy::HotRollbackJournal,
            validation:
                "ProjectWorkspace removes a file created by an interrupted Save only while its active journal and planned hash still match.",
        },
        WriteDeclaration {
            category: WriteCategory::ProjectDesignWrite,
            owner: WriteOwner::ProjectWorkspace,
            operations: vec![WriteOperationKind::WriteText, WriteOperationKind::WriteBytes],
            path_authority: "ProjectSession project root / design",
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::RequireDiskBaseline,
            recovery: RecoveryPolicy::HotRollbackJournal,
            validation:
                "ProjectWorkspace writes design text or binary resources only from a validated active Save journal with an exact live disk baseline.",
        },
        WriteDeclaration {
            category: WriteCategory::ProjectDesignWrite,
            owner: WriteOwner::ProjectWorkspace,
            operations: vec![WriteOperationKind::RemoveFile],
            path_authority: "ProjectSession project root / design",
            atomicity: WriteAtomicity::FileLifecycle,
            conflict: ConflictPolicy::RequireDiskBaseline,
            recovery: RecoveryPolicy::HotRollbackJournal,
            validation:
                "ProjectWorkspace removes a design file created by an interrupted Save only while its journal and planned hash match.",
        },
        WriteDeclaration {
            category: WriteCategory::ProjectSourceWrite,
            owner: WriteOwner::ProjectInitializer,
            operations: vec![
                WriteOperationKind::CreateDirectory,
                WriteOperationKind::Copy,
                WriteOperationKind::RemoveFile,
                WriteOperationKind::RemoveDirectoryTree,
            ],
            path_authority:
                "new empty project root and project/sursa during Project Initializer bootstrap",
            atomicity: WriteAtomicity::FileLifecycle,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::LoggedAtomicFile,
            validation:
                "Project initializer writes only after empty-folder preflight, stays inside the selected project root, blocks file overwrite during resource copy, and logs lifecycle mutations through WriteAuthority.",
        },
        WriteDeclaration {
            category: WriteCategory::ProjectDesignWrite,
            owner: WriteOwner::ProjectInitializer,
            operations: vec![WriteOperationKind::CreateDirectory, WriteOperationKind::Copy],
            path_authority: "new empty project root / design during Project Initializer bootstrap",
            atomicity: WriteAtomicity::FileLifecycle,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::LoggedAtomicFile,
            validation:
                "Design scaffold files copied by the project initializer stay inside project/design, are never copied over an existing destination file, and are logged through WriteAuthority.",
        },
        WriteDeclaration {
            category: WriteCategory::ProjectDesignWrite,
            owner: WriteOwner::MoodBoard,
            operations: vec![WriteOperationKind::WriteText],
            path_authority: "project/design/mood-board.json",
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::LoggedAtomicFile,
            validation:
                "Mood Board state is a single-owner JSON design document, written atomically through WriteAuthority inside project/design.",
        },
        WriteDeclaration {
            category: WriteCategory::ProjectDesignWrite,
            owner: WriteOwner::MoodBoard,
            operations: vec![WriteOperationKind::WriteBytes],
            path_authority: "project/design/imagini and project/resurse/imagini Mood Board exports",
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::RequireExplicitOverride,
            recovery: RecoveryPolicy::LoggedAtomicFile,
            validation:
                "Mood Board binary image exports are create-only, limited to design/imagini or resurse/imagini, blocked on existing target/symlink/directory, and logged through WriteAuthority.",
        },
        WriteDeclaration {
            category: WriteCategory::PreviewWorkspaceWrite,
            owner: WriteOwner::Preview,
            operations: vec![WriteOperationKind::WriteText, WriteOperationKind::WriteBytes],
            path_authority: "ApplicationHome.temp/cache preview workspace",
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::EphemeralRebuildable,
            validation:
                "Preview workspace generated text and binary overlays are written atomically inside rebuildable preview directories and never become source of truth.",
        },
        WriteDeclaration {
            category: WriteCategory::PreviewWorkspaceWrite,
            owner: WriteOwner::Preview,
            operations: vec![
                WriteOperationKind::CreateDirectory,
                WriteOperationKind::Copy,
                WriteOperationKind::Symlink,
                WriteOperationKind::RemoveFile,
                WriteOperationKind::RemoveDirectoryTree,
            ],
            path_authority: "ApplicationHome.temp/cache preview workspace",
            atomicity: WriteAtomicity::FileLifecycle,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::EphemeralRebuildable,
            validation: "Preview workspaces are rebuildable and never become source of truth.",
        },
        WriteDeclaration {
            category: WriteCategory::BuildOutputWrite,
            owner: WriteOwner::ImageOptimizer,
            operations: vec![WriteOperationKind::WriteBytes, WriteOperationKind::WriteText],
            path_authority: "Zola output_dir after build",
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::EphemeralRebuildable,
            validation:
                "Image optimizer writes WebP assets and rewritten output references only inside validated Zola output_dir; output is rebuildable from project source.",
        },
        WriteDeclaration {
            category: WriteCategory::BuildOutputWrite,
            owner: WriteOwner::ImageOptimizer,
            operations: vec![WriteOperationKind::RemoveFile],
            path_authority: "Zola output_dir after build",
            atomicity: WriteAtomicity::FileLifecycle,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::EphemeralRebuildable,
            validation:
                "Image optimizer removes replaced originals only inside validated Zola output_dir after the replacement asset has been committed.",
        },
    ]
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::kernel::write_authority::{WritePolicy, WriteTarget};

    #[test]
    fn scratch_state_write_and_cleanup_are_declared_as_rebuildable_cache() {
        let target = WriteTarget::new(
            PathBuf::from("/app/cache/scratch/preview/session.json"),
            PathBuf::from("/app/cache/scratch"),
            "scratch:preview/session.json",
        );
        let write = WriteIntent::new(
            WriteCategory::InternalAppWrite,
            WriteOwner::ScratchState,
            WriteOperationKind::WriteText,
            target.clone(),
            WritePolicy::scratch_state_atomic(),
            "declared scratch write",
        );
        let remove = WriteIntent::new(
            WriteCategory::InternalAppWrite,
            WriteOwner::ScratchState,
            WriteOperationKind::RemoveFile,
            target,
            WritePolicy::scratch_state_lifecycle(),
            "declared scratch cleanup",
        );

        assert!(matching_write_declaration(&write).is_some());
        assert!(matching_write_declaration(&remove).is_some());
    }

    #[test]
    fn mismatched_policy_is_not_declared() {
        let intent = WriteIntent::new(
            WriteCategory::InternalAppWrite,
            WriteOwner::AppConfig,
            WriteOperationKind::WriteText,
            WriteTarget::new(
                PathBuf::from("/app/config/config.json"),
                PathBuf::from("/app/config"),
                "config/config.json",
            ),
            WritePolicy::internal_lifecycle(),
            "bad app config policy",
        );

        assert!(matching_write_declaration(&intent).is_none());
    }

    #[test]
    fn declarations_have_documented_path_contract() {
        for declaration in known_write_declarations() {
            assert!(!declaration.operations.is_empty());
            assert!(!declaration.path_authority.trim().is_empty());
            assert!(!declaration.validation.trim().is_empty());
        }
    }

    #[test]
    fn project_write_declarations_have_only_current_explicit_owners() {
        for declaration in known_write_declarations() {
            match declaration.category {
                WriteCategory::ProjectSourceWrite => assert!(matches!(
                    declaration.owner,
                    WriteOwner::ProjectWorkspace | WriteOwner::ProjectInitializer
                )),
                WriteCategory::ProjectDesignWrite => assert!(matches!(
                    declaration.owner,
                    WriteOwner::ProjectWorkspace
                        | WriteOwner::ProjectInitializer
                        | WriteOwner::MoodBoard
                )),
                _ => {}
            }
        }
    }
}
