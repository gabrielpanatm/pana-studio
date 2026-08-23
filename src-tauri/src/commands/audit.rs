use std::{collections::BTreeMap, path::Path, time::Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::{
    kernel::audit::{
        build_audit_run, AuditBuildEvidence, AuditFix, AuditFixApplicability, AuditOutcome,
        AuditRequest, AuditRunMode, AuditRunReceipt, AUDIT_RULESET_VERSION,
        AUDIT_RUN_SCHEMA_VERSION,
    },
    kernel::file_buffer_store::now_ms,
    kernel::project_workspace::{
        publish_prepared_project_workspace_candidate, ProjectWorkspace, ProjectWorkspaceIdentity,
        ProjectWorkspaceMutationReceipt, ProjectWorkspacePreviewProjection,
        ProjectWorkspaceSnapshot, WorkspaceMutationMetadata, WorkspaceResourceMutation,
    },
    preview::PersistentPreviewOwner,
    project_model::{
        build_project_model_for_audit_from_workspace_projection,
        cache::{
            capture_project_model_build_context, validate_project_model_build_context_current,
        },
    },
    state::{AppState, ProjectAuditAuthority},
};

pub const AUDIT_FIX_APPLY_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditFixApplyInput {
    pub schema_version: u32,
    pub expected_audit_schema_version: u32,
    pub expected_ruleset_version: u32,
    pub expected_project_root: String,
    pub expected_session_id: String,
    pub expected_workspace_revision: u64,
    pub expected_project_model_revision: String,
    pub finding_fingerprint: String,
    pub fix_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditFixApplyReceipt {
    pub schema_version: u32,
    pub finding_fingerprint: String,
    pub fix_id: String,
    pub mutation: ProjectWorkspaceMutationReceipt,
    pub workspace: ProjectWorkspaceSnapshot,
    pub audit: AuditRunReceipt,
}

/// Produces a versioned, provider-oriented receipt from the exact immutable
/// ProjectWorkspace revision. Project defects are findings; only authority,
/// identity and coherence failures reject the command.
#[tauri::command]
pub fn read_project_audit(
    state: State<AppState>,
    request: Option<AuditRequest>,
) -> Result<AuditRunReceipt, String> {
    let request = request.unwrap_or_default();
    let (root, session, context) = capture_project_model_build_context(&state)?;
    let file_buffer_diagnostics = {
        let workspace = state
            .project_workspace
            .lock()
            .map_err(|_| "Nu am putut captura ProjectWorkspace pentru Audit.".to_string())?;
        let workspace = workspace
            .as_ref()
            .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru Audit.".to_string())?;
        if workspace.session.project_root != context.projection().project_root
            || workspace.runtime_session_id() != context.projection().runtime_session_id
            || workspace.revision != context.projection().revision
        {
            return Err(
                "Audit a refuzat o proiecție stale: sesiunea sau revizia workspace s-a schimbat."
                    .to_string(),
            );
        }
        workspace.documents.diagnostics.clone()
    };
    let model =
        build_project_model_for_audit_from_workspace_projection(&root, context.projection())?;
    let build_evidence = capture_build_evidence(
        &state,
        &root,
        &session.runtime_instance_id(),
        context.projection().revision,
        request.mode,
    )?;
    let receipt = build_audit_run(
        &model,
        &file_buffer_diagnostics,
        session.runtime_instance_id(),
        context.projection().revision,
        request.clone(),
        build_evidence,
    )?;
    validate_project_model_build_context_current(&state, &context)?;
    *state
        .project_audit_authority
        .lock()
        .map_err(|_| "Audit nu a putut publica autoritatea rulării curente.".to_string())? =
        Some(ProjectAuditAuthority {
            request,
            receipt: receipt.clone(),
        });
    Ok(receipt)
}

#[tauri::command]
pub fn apply_audit_fix(
    input: AuditFixApplyInput,
    app: AppHandle,
    state: State<AppState>,
) -> Result<AuditFixApplyReceipt, String> {
    validate_audit_fix_input(&input)?;
    state
        .ai_coordination
        .require_user_source_mutation()
        .map_err(|error| error.to_string())?;
    let audit_request = require_authoritative_audit_request(&state, &input)?;
    let total_started = Instant::now();
    let clone_started = Instant::now();
    let (root, mut candidate, base_diagnostics) = {
        let root = state
            .current_root
            .lock()
            .map_err(|_| "Audit Fix nu a putut bloca ProjectRoot.".to_string())?
            .clone()
            .ok_or_else(|| "Audit Fix cere un proiect deschis.".to_string())?;
        let workspace = state
            .project_workspace
            .lock()
            .map_err(|_| "Audit Fix nu a putut captura ProjectWorkspace.".to_string())?;
        let workspace = workspace
            .as_ref()
            .ok_or_else(|| "Audit Fix cere un ProjectWorkspace activ.".to_string())?;
        require_audit_fix_workspace_identity(workspace, &input)?;
        workspace.accepted_disk.require_live_complete(
            &workspace.runtime_session_id(),
            &workspace.session.project_root,
            &root,
        )?;
        (
            root,
            workspace.fork_candidate(),
            workspace.documents.diagnostics.clone(),
        )
    };
    let candidate_clone_ms = clone_started.elapsed().as_millis().min(u64::MAX as u128) as u64;

    let base_projection = candidate.capture_projection_snapshot()?;
    let base_model =
        build_project_model_for_audit_from_workspace_projection(&root, &base_projection)?;
    if base_model.revision != input.expected_project_model_revision {
        return Err("Audit Fix a refuzat ProjectModel-ul stale.".to_string());
    }
    let base_audit = build_audit_run(
        &base_model,
        &base_diagnostics,
        input.expected_session_id.clone(),
        input.expected_workspace_revision,
        audit_request.clone(),
        AuditBuildEvidence::Skipped {
            message: "Audit Fix revalidează sursele fără a forța o generație Preview.".to_string(),
        },
    )?;
    let finding = base_audit
        .findings
        .iter()
        .find(|finding| finding.fingerprint == input.finding_fingerprint)
        .ok_or_else(|| "Audit Fix nu mai găsește constatarea în revizia indicată.".to_string())?;
    if finding.outcome == AuditOutcome::Suppressed || finding.suppression.is_some() {
        return Err("Audit Fix a refuzat o constatare suprimată.".to_string());
    }
    let target_rule_code = finding.rule_code.clone();
    let target_file = finding
        .primary_location
        .as_ref()
        .map(|location| location.file.clone());
    let fix = finding
        .fixes
        .iter()
        .find(|fix| fix.id == input.fix_id)
        .ok_or_else(|| "Audit Fix nu mai găsește remedierea solicitată.".to_string())?;
    if fix.applicability != AuditFixApplicability::Safe {
        return Err("Audit Fix aplică automat numai remedieri marcate safe de Rust.".to_string());
    }
    let mutations = materialize_audit_fix(&candidate, fix)?;
    if mutations.is_empty() {
        return Err("Audit Fix a refuzat o remediere fără modificări.".to_string());
    }

    let mutation_started = Instant::now();
    let identity = ProjectWorkspaceIdentity {
        expected_project_root: input.expected_project_root.clone(),
        expected_session_id: input.expected_session_id.clone(),
        expected_revision: input.expected_workspace_revision,
    };
    let mutation = candidate.stage_resource_texts(
        &identity,
        WorkspaceMutationMetadata {
            label: "Aplicare remediere sigură Audit".to_string(),
            source: "audit.apply_safe_fix".to_string(),
            coalesce_key: None,
            transaction_id: Some(format!("audit-fix:{}", input.fix_id)),
        },
        mutations,
        now_ms(),
    )?;
    if !mutation.changed || mutation.revision_after != input.expected_workspace_revision + 1 {
        return Err("Audit Fix nu a produs exact o mutație ProjectWorkspace.".to_string());
    }
    let post_projection = candidate.capture_projection_snapshot()?;
    let post_model =
        build_project_model_for_audit_from_workspace_projection(&root, &post_projection)?;
    let mut post_request = audit_request;
    post_request.mode = AuditRunMode::Quick;
    let post_audit = build_audit_run(
        &post_model,
        &candidate.documents.diagnostics,
        input.expected_session_id.clone(),
        mutation.revision_after,
        post_request.clone(),
        AuditBuildEvidence::Skipped {
            message: "Audit Fix a revalidat modelul rezultat; buildul rămâne separat.".to_string(),
        },
    )?;
    if post_audit.findings.iter().any(|candidate| {
        candidate.fingerprint == input.finding_fingerprint
            || candidate.rule_code == target_rule_code
                && candidate
                    .primary_location
                    .as_ref()
                    .map(|location| &location.file)
                    == target_file.as_ref()
                && matches!(
                    candidate.outcome,
                    AuditOutcome::Violation | AuditOutcome::NeedsReview | AuditOutcome::EngineError
                )
    }) {
        return Err("Audit Fix a refuzat candidatul: constatarea țintă persistă.".to_string());
    }
    let transaction_id = mutation
        .transaction_id
        .as_deref()
        .ok_or_else(|| "Audit Fix nu a primit transaction ID canonic.".to_string())?;
    candidate.publish_project_model_for_transaction(
        &input.expected_project_root,
        &input.expected_session_id,
        mutation.revision_after,
        transaction_id,
        post_model,
    )?;
    let mutation_ms = mutation_started.elapsed().as_millis().min(u64::MAX as u128) as u64;

    let workspace_snapshot = {
        let mut workspace = state
            .project_workspace
            .lock()
            .map_err(|_| "Audit Fix nu a putut publica ProjectWorkspace.".to_string())?;
        let live = workspace
            .as_mut()
            .ok_or_else(|| "Audit Fix a devenit stale: proiectul a fost închis.".to_string())?;
        require_audit_fix_workspace_identity(live, &input)?;
        publish_prepared_project_workspace_candidate(
            &app,
            live,
            input.expected_workspace_revision,
            candidate,
            ProjectWorkspacePreviewProjection::Required,
            candidate_clone_ms,
            mutation_ms,
            total_started,
        )?;
        live.snapshot()
    };
    // The workspace commit is already authoritative at this point. A poisoned
    // derived Publish cache must never turn a committed source mutation into a
    // false failure receipt; currency checks will reject it on the next read.
    let _ = state.clear_publish_authorization();
    if let Ok(mut authority) = state.project_audit_authority.lock() {
        *authority = Some(ProjectAuditAuthority {
            request: post_request,
            receipt: post_audit.clone(),
        });
    }
    Ok(AuditFixApplyReceipt {
        schema_version: AUDIT_FIX_APPLY_SCHEMA_VERSION,
        finding_fingerprint: input.finding_fingerprint,
        fix_id: input.fix_id,
        mutation,
        workspace: workspace_snapshot,
        audit: post_audit,
    })
}

fn require_authoritative_audit_request(
    state: &AppState,
    input: &AuditFixApplyInput,
) -> Result<AuditRequest, String> {
    let authority = state
        .project_audit_authority
        .lock()
        .map_err(|_| "Audit Fix nu a putut verifica autoritatea Audit.".to_string())?;
    let authority = authority.as_ref().ok_or_else(|| {
        "Audit Fix cere o rulare Audit curentă înainte de aplicarea remedierii.".to_string()
    })?;
    let receipt = &authority.receipt;
    if receipt.schema_version != input.expected_audit_schema_version
        || receipt.ruleset_version != input.expected_ruleset_version
        || receipt.project_root != input.expected_project_root
        || receipt.runtime_session_id != input.expected_session_id
        || receipt.workspace_revision != input.expected_workspace_revision
        || receipt.project_model_revision != input.expected_project_model_revision
    {
        return Err("Audit Fix a refuzat o rulare Audit stale sau străină.".to_string());
    }
    let finding = receipt
        .findings
        .iter()
        .find(|finding| finding.fingerprint == input.finding_fingerprint)
        .ok_or_else(|| "Audit Fix nu găsește constatarea în rularea autorizată.".to_string())?;
    if finding.outcome == AuditOutcome::Suppressed || finding.suppression.is_some() {
        return Err("Audit Fix a refuzat o constatare suprimată.".to_string());
    }
    if !finding
        .fixes
        .iter()
        .any(|fix| fix.id == input.fix_id && fix.applicability == AuditFixApplicability::Safe)
    {
        return Err("Audit Fix nu găsește o remediere safe în rularea autorizată.".to_string());
    }
    Ok(authority.request.clone())
}

fn validate_audit_fix_input(input: &AuditFixApplyInput) -> Result<(), String> {
    if input.schema_version != AUDIT_FIX_APPLY_SCHEMA_VERSION
        || input.expected_audit_schema_version != AUDIT_RUN_SCHEMA_VERSION
        || input.expected_ruleset_version != AUDIT_RULESET_VERSION
    {
        return Err("Audit Fix a refuzat un contract sau ruleset incompatibil.".to_string());
    }
    if input.expected_project_root.trim().is_empty()
        || input.expected_session_id.trim().is_empty()
        || input.expected_project_model_revision.trim().is_empty()
        || input.fix_id.trim().is_empty()
        || !input
            .finding_fingerprint
            .strip_prefix("sha256:")
            .is_some_and(|digest| {
                digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
    {
        return Err("Audit Fix a refuzat identități sau fingerprint invalide.".to_string());
    }
    Ok(())
}

fn require_audit_fix_workspace_identity(
    workspace: &ProjectWorkspace,
    input: &AuditFixApplyInput,
) -> Result<(), String> {
    if workspace.session.project_root != input.expected_project_root
        || workspace.runtime_session_id() != input.expected_session_id
        || workspace.revision != input.expected_workspace_revision
    {
        return Err("Audit Fix a refuzat o sesiune sau revizie stale.".to_string());
    }
    Ok(())
}

fn materialize_audit_fix(
    workspace: &ProjectWorkspace,
    fix: &AuditFix,
) -> Result<Vec<WorkspaceResourceMutation>, String> {
    let mut edits_by_file = BTreeMap::<String, Vec<_>>::new();
    for edit in &fix.edits {
        let range = edit
            .location
            .range
            .as_ref()
            .ok_or_else(|| "Audit Fix a refuzat un edit fără range exact.".to_string())?;
        if edit.location.file.trim().is_empty()
            || range.start > range.end
            || matches!(
                edit.location.origin,
                crate::kernel::audit::AuditSourceOrigin::Generated
            )
        {
            return Err("Audit Fix a refuzat o locație nemutabilă sau invalidă.".to_string());
        }
        edits_by_file
            .entry(edit.location.file.clone())
            .or_default()
            .push((range.start, range.end, edit.replacement.as_str()));
    }
    let mut mutations = Vec::with_capacity(edits_by_file.len());
    for (relative_path, mut edits) in edits_by_file {
        let source = workspace
            .documents
            .text_for(&relative_path)
            .ok_or_else(|| {
                format!("Audit Fix nu poate edita fișierul neurmărit {relative_path}.")
            })?;
        let contents = apply_validated_text_edits(&source, &relative_path, &mut edits)?;
        mutations.push(WorkspaceResourceMutation {
            relative_path,
            contents,
            create_only: false,
        });
    }
    Ok(mutations)
}

fn apply_validated_text_edits(
    source: &str,
    relative_path: &str,
    edits: &mut Vec<(usize, usize, &str)>,
) -> Result<String, String> {
    edits.sort_by_key(|(start, end, _)| (*start, *end));
    for pair in edits.windows(2) {
        if pair[0].1 > pair[1].0 || pair[0].0 == pair[1].0 && pair[0].1 == pair[1].1 {
            return Err(format!(
                "Audit Fix a refuzat edituri suprapuse în {relative_path}."
            ));
        }
    }
    for (start, end, _) in edits.iter() {
        if *end > source.len() || !source.is_char_boundary(*start) || !source.is_char_boundary(*end)
        {
            return Err(format!(
                "Audit Fix a refuzat un range UTF-8 invalid în {relative_path}."
            ));
        }
    }
    let mut contents = source.to_string();
    for (start, end, replacement) in edits.iter().rev() {
        contents.replace_range(*start..*end, replacement);
    }
    Ok(contents)
}

fn capture_build_evidence(
    state: &AppState,
    root: &Path,
    runtime_session_id: &str,
    workspace_revision: u64,
    mode: AuditRunMode,
) -> Result<AuditBuildEvidence, String> {
    let engine = state
        .preview_engine
        .lock()
        .map_err(|_| "Motorul Preview nu a putut fi verificat pentru Audit.".to_string())?;
    let Some(engine) = engine.as_ref() else {
        return Ok(missing_build_evidence(
            mode,
            "Nu există încă o generație Preview Zola pentru sesiunea curentă.",
        ));
    };
    let owner = PersistentPreviewOwner::new(root.to_string_lossy().as_ref(), runtime_session_id);
    if !engine.owner_matches(&owner) {
        return Ok(missing_build_evidence(
            mode,
            "Generația Preview aparține altei sesiuni de proiect.",
        ));
    }
    match engine.active_matches_revision(workspace_revision) {
        Ok(true) => Ok(AuditBuildEvidence::Complete {
            message: format!(
                "Zola embedded a confirmat ProjectWorkspace revizia {workspace_revision}."
            ),
        }),
        Ok(false) => Ok(missing_build_evidence(
            mode,
            &format!(
                "Preview Zola nu confirmă încă ProjectWorkspace revizia {workspace_revision}."
            ),
        )),
        Err(error) => Ok(AuditBuildEvidence::Failed { message: error }),
    }
}

fn missing_build_evidence(mode: AuditRunMode, message: &str) -> AuditBuildEvidence {
    if mode == AuditRunMode::Full {
        AuditBuildEvidence::Failed {
            message: message.to_string(),
        }
    } else {
        AuditBuildEvidence::Skipped {
            message: message.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        js::PageJsDraftStore,
        kernel::{
            audit::{
                AuditCategory, AuditCompleteness, AuditFinding, AuditImpact, AuditLocation,
                AuditOutcome, AuditPolicy, AuditRequest, AuditRunMode, AuditRunReceipt, AuditScope,
                AuditSourceOrigin, AuditSummary, AuditSuppression, AuditSuppressionScope,
                AuditTextEdit,
            },
            file_buffer_store::{
                hash_text, FileBufferBaseline, FileBufferEntry, FileBufferStore,
                FileBufferStoreLimits, TextBufferLanguage, TextBufferRole,
            },
            project_session::{
                ProjectRootFingerprint, ProjectSessionScanSummary, ProjectSessionSnapshot,
            },
            project_workspace::{ProjectWorkspaceIdentity, WorkspaceMutationMetadata},
        },
        localization::LocalizedDiagnostic,
        project::{AcceptedProjectDiskManifest, ProjectDiskManifest},
        source_graph::model::SourceRange,
        state::{AppState, ProjectAuditAuthority},
    };

    use super::{
        apply_validated_text_edits, materialize_audit_fix, require_audit_fix_workspace_identity,
        require_authoritative_audit_request, AuditFix, AuditFixApplicability, AuditFixApplyInput,
        ProjectWorkspace, AUDIT_FIX_APPLY_SCHEMA_VERSION, AUDIT_RULESET_VERSION,
        AUDIT_RUN_SCHEMA_VERSION,
    };

    #[test]
    fn audit_fix_edits_are_descending_utf8_safe_and_lossless_elsewhere() {
        let source = "<div>Ș</div>\n<span>text</span>\n";
        let second_line = source.find("<span>").unwrap();
        let mut edits = vec![(0, 0, "  "), (second_line, second_line, "  ")];
        let result = apply_validated_text_edits(source, "templates/index.html", &mut edits)
            .expect("valid edits");
        assert_eq!(result, "  <div>Ș</div>\n  <span>text</span>\n");
    }

    #[test]
    fn audit_fix_rejects_overlap_and_non_utf8_boundary() {
        let mut overlap = vec![(0, 2, ""), (1, 3, "")];
        assert!(apply_validated_text_edits("abcd", "a.html", &mut overlap).is_err());
        let mut invalid_utf8 = vec![(6, 7, "")];
        assert!(apply_validated_text_edits("text Ș", "a.html", &mut invalid_utf8).is_err());
    }

    #[test]
    fn audit_fix_identity_rejects_stale_revision_and_foreign_session_without_mutation() {
        let workspace = test_workspace("<main>Original</main>\n");
        let original_revision = workspace.revision;
        let original_text = workspace.documents.text_for("templates/index.html");
        let input = audit_fix_input(&workspace);
        require_audit_fix_workspace_identity(&workspace, &input).expect("current identity");

        let mut stale = input.clone();
        stale.expected_workspace_revision += 1;
        assert!(require_audit_fix_workspace_identity(&workspace, &stale).is_err());

        let mut foreign = input;
        foreign.expected_session_id.push_str(":foreign");
        assert!(require_audit_fix_workspace_identity(&workspace, &foreign).is_err());
        assert_eq!(workspace.revision, original_revision);
        assert_eq!(
            workspace.documents.text_for("templates/index.html"),
            original_text
        );
    }

    #[test]
    fn audit_fix_requires_current_authoritative_unsuppressed_safe_finding() {
        let workspace = test_workspace("<main>Original</main>\n");
        let input = audit_fix_input(&workspace);
        let state = AppState::default();
        *state.project_audit_authority.lock().unwrap() =
            Some(test_audit_authority(&workspace, false));
        require_authoritative_audit_request(&state, &input).expect("authorized current audit");

        let mut stale = input.clone();
        stale.expected_workspace_revision += 1;
        assert!(require_authoritative_audit_request(&state, &stale).is_err());

        *state.project_audit_authority.lock().unwrap() =
            Some(test_audit_authority(&workspace, true));
        assert!(require_authoritative_audit_request(&state, &input).is_err());
    }

    #[test]
    fn audit_fix_is_one_history_entry_and_round_trips_exactly_through_undo_redo() {
        let source = "<main>\n<article>Ș</article>\n</main>\n";
        let expected = "<main>\n  <article>Ș</article>\n</main>\n";
        let mut workspace = test_workspace(source);
        let start = source.find("<article>").expect("article prefix");
        let fix = AuditFix {
            id: "structural-indentation:templates/index.html".to_string(),
            title_diagnostic: LocalizedDiagnostic::new("audit-structural-indentation-fix-title"),
            applicability: AuditFixApplicability::Safe,
            edits: vec![AuditTextEdit {
                location: AuditLocation {
                    file: "templates/index.html".to_string(),
                    range: Some(SourceRange {
                        start,
                        end: start,
                        line: 2,
                        column: 1,
                        end_line: 2,
                        end_column: 1,
                    }),
                    origin: AuditSourceOrigin::Project,
                    source_node_id: None,
                },
                replacement: "  ".to_string(),
            }],
        };
        let mutations = materialize_audit_fix(&workspace, &fix).expect("server fix");
        let identity = workspace_identity(&workspace);
        let receipt = workspace
            .stage_resource_texts(
                &identity,
                WorkspaceMutationMetadata {
                    label: "Aplicare remediere sigură Audit".to_string(),
                    source: "audit.apply_safe_fix".to_string(),
                    coalesce_key: None,
                    transaction_id: Some(format!("audit-fix:{}", fix.id)),
                },
                mutations,
                2,
            )
            .expect("single atomic mutation");
        assert!(receipt.changed);
        assert_eq!(receipt.history.undo_count, 1);
        assert_eq!(
            workspace
                .documents
                .text_for("templates/index.html")
                .as_deref(),
            Some(expected)
        );

        workspace
            .undo(&workspace_identity(&workspace), 3)
            .expect("undo fix");
        assert_eq!(
            workspace
                .documents
                .text_for("templates/index.html")
                .as_deref(),
            Some(source)
        );
        workspace
            .redo(&workspace_identity(&workspace), 4)
            .expect("redo fix");
        assert_eq!(
            workspace
                .documents
                .text_for("templates/index.html")
                .as_deref(),
            Some(expected)
        );
    }

    fn audit_fix_input(workspace: &ProjectWorkspace) -> AuditFixApplyInput {
        AuditFixApplyInput {
            schema_version: AUDIT_FIX_APPLY_SCHEMA_VERSION,
            expected_audit_schema_version: AUDIT_RUN_SCHEMA_VERSION,
            expected_ruleset_version: AUDIT_RULESET_VERSION,
            expected_project_root: workspace.session.project_root.clone(),
            expected_session_id: workspace.runtime_session_id(),
            expected_workspace_revision: workspace.revision,
            expected_project_model_revision: "model-test".to_string(),
            finding_fingerprint: format!("sha256:{}", "0".repeat(64)),
            fix_id: "fix-test".to_string(),
        }
    }

    fn test_audit_authority(
        workspace: &ProjectWorkspace,
        suppressed: bool,
    ) -> ProjectAuditAuthority {
        let suppression = suppressed.then(|| AuditSuppression {
            rule_code: "structural_indentation_drift".to_string(),
            file: None,
            fingerprint: None,
            scope: AuditSuppressionScope::Rule,
            reason: "test".to_string(),
        });
        let finding = AuditFinding {
            id: "audit:test".to_string(),
            fingerprint: format!("sha256:{}", "0".repeat(64)),
            provider_id: "source_conformance".to_string(),
            rule_code: "structural_indentation_drift".to_string(),
            category: AuditCategory::Build,
            outcome: if suppressed {
                AuditOutcome::Suppressed
            } else {
                AuditOutcome::Violation
            },
            impact: AuditImpact::Moderate,
            policy: AuditPolicy::Advisory,
            title_diagnostic: LocalizedDiagnostic::new("audit-structural-indentation-title"),
            message_diagnostic: LocalizedDiagnostic::new("audit-structural-indentation-message"),
            primary_location: None,
            related_locations: Vec::new(),
            evidence: Vec::new(),
            fixes: vec![AuditFix {
                id: "fix-test".to_string(),
                title_diagnostic: LocalizedDiagnostic::new(
                    "audit-structural-indentation-fix-title",
                ),
                applicability: AuditFixApplicability::Safe,
                edits: Vec::new(),
            }],
            suppression,
        };
        ProjectAuditAuthority {
            request: AuditRequest::default(),
            receipt: AuditRunReceipt {
                schema_version: AUDIT_RUN_SCHEMA_VERSION,
                ruleset_version: AUDIT_RULESET_VERSION,
                project_root: workspace.session.project_root.clone(),
                runtime_session_id: workspace.runtime_session_id(),
                workspace_revision: workspace.revision,
                project_model_revision: "model-test".to_string(),
                mode: AuditRunMode::Quick,
                scope: AuditScope::Project,
                completeness: AuditCompleteness::Complete,
                summary: AuditSummary::default(),
                providers: Vec::new(),
                findings: vec![finding],
            },
        }
    }

    fn workspace_identity(workspace: &ProjectWorkspace) -> ProjectWorkspaceIdentity {
        ProjectWorkspaceIdentity {
            expected_project_root: workspace.session.project_root.clone(),
            expected_session_id: workspace.runtime_session_id(),
            expected_revision: workspace.revision,
        }
    }

    fn test_workspace(source: &str) -> ProjectWorkspace {
        let root = format!("/tmp/pana-audit-fix-test-{}", std::process::id());
        let session = ProjectSessionSnapshot {
            schema_version: 1,
            id: "audit-fix-test".to_string(),
            project_root: root.clone(),
            zola_root: root.clone(),
            session_dir: format!("{root}/session"),
            manifest_path: format!("{root}/session.json"),
            opened_at_ms: 1,
            last_seen_at_ms: 1,
            root_fingerprint: ProjectRootFingerprint {
                canonical_path: root.clone(),
                modified_ms: 1,
                size: 0,
                readonly: false,
                unix_device: None,
                unix_inode: None,
            },
            scan_summary: ProjectSessionScanSummary {
                active_theme: None,
                file_count: 1,
                directory_count: 1,
            },
        };
        let mut documents = FileBufferStore::for_project_session(
            &session,
            1,
            FileBufferStoreLimits {
                max_files: 8,
                max_file_bytes: 1024 * 1024,
                max_total_bytes: 2 * 1024 * 1024,
            },
        );
        documents.insert_loaded_file(FileBufferEntry {
            relative_path: "templates/index.html".to_string(),
            absolute_path: format!("{root}/templates/index.html"),
            language: TextBufferLanguage::Html,
            role: TextBufferRole::Template,
            baseline: FileBufferBaseline {
                hash: hash_text(source),
                modified_ms: 1,
                size: source.len() as u64,
                readonly: false,
            },
            baseline_text: source.to_string().into(),
            draft: None,
            revision: 1,
        });
        let accepted = AcceptedProjectDiskManifest::new(
            session.runtime_instance_id(),
            root.clone(),
            ProjectDiskManifest {
                root,
                files: Vec::new(),
                truncated: false,
                max_files: 8,
            },
        )
        .expect("accepted manifest");
        let page_js = PageJsDraftStore::new(&session);
        ProjectWorkspace::new(session, accepted, documents, page_js).expect("workspace")
    }
}
