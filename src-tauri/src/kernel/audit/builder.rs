use std::collections::{HashMap, HashSet};

use sha2::{Digest, Sha256};

use crate::{
    kernel::file_buffer_store::{FileBufferDiagnostic, FileBufferDiagnosticSeverity},
    localization::LocalizedDiagnostic,
    project_model::model::{
        ProjectModel, ProjectModelDiagnosticSeverity, ProjectModelFile, ProjectModelFileKind,
    },
    project_model::structural_edit::audit_html_indentation,
    source_graph::{
        mixed_cst::{parse_mixed_cst, MixedCstKind},
        model::{
            SourceDataNode, SourceDataPathSegment, SourceDiagnosticSeverity, SourceOrigin,
            SourceRange,
        },
    },
};

use super::model::{
    AuditBuildEvidence, AuditCategory, AuditCompleteness, AuditCoverage, AuditEvidence,
    AuditEvidenceKind, AuditFinding, AuditFix, AuditFixApplicability, AuditImpact, AuditLocation,
    AuditOutcome, AuditPolicy, AuditProviderKind, AuditProviderReceipt, AuditProviderStatus,
    AuditPublishCoverageRequirement, AuditRequest, AuditRunReceipt, AuditSourceOrigin,
    AuditSummary, AuditSuppression, AuditSuppressionScope, AuditTextEdit, AUDIT_RULESET_VERSION,
    AUDIT_RUN_SCHEMA_VERSION,
};

struct AuditProviderDefinition {
    id: &'static str,
    kind: AuditProviderKind,
    publish_coverage_requirement: AuditPublishCoverageRequirement,
    run: fn(&mut AuditContext<'_>) -> ProviderExecution,
}

struct ProviderExecution {
    status: AuditProviderStatus,
    coverage: AuditCoverage,
    evidence: Vec<AuditEvidence>,
}

impl ProviderExecution {
    fn complete(eligible: usize, analyzed: usize) -> Self {
        Self {
            status: AuditProviderStatus::Complete,
            coverage: AuditCoverage {
                eligible,
                analyzed,
                limitations: Vec::new(),
            },
            evidence: Vec::new(),
        }
    }

    fn partial(eligible: usize, analyzed: usize, limitations: Vec<LocalizedDiagnostic>) -> Self {
        Self {
            status: AuditProviderStatus::Partial,
            coverage: AuditCoverage {
                eligible,
                analyzed,
                limitations,
            },
            evidence: Vec::new(),
        }
    }
}

const AUDIT_PROVIDER_REGISTRY: &[AuditProviderDefinition] = &[
    AuditProviderDefinition {
        id: "workspace_integrity",
        kind: AuditProviderKind::WorkspaceIntegrity,
        publish_coverage_requirement: AuditPublishCoverageRequirement::Required,
        run: run_workspace_provider,
    },
    AuditProviderDefinition {
        id: "project_model",
        kind: AuditProviderKind::ProjectModel,
        publish_coverage_requirement: AuditPublishCoverageRequirement::Required,
        run: run_project_model_provider,
    },
    AuditProviderDefinition {
        id: "source_conformance",
        kind: AuditProviderKind::SourceConformance,
        publish_coverage_requirement: AuditPublishCoverageRequirement::Required,
        run: run_source_conformance_provider,
    },
    AuditProviderDefinition {
        id: "project_graph",
        kind: AuditProviderKind::ProjectGraph,
        publish_coverage_requirement: AuditPublishCoverageRequirement::Required,
        run: run_project_graph_provider,
    },
    AuditProviderDefinition {
        id: "component_graph",
        kind: AuditProviderKind::ComponentGraph,
        publish_coverage_requirement: AuditPublishCoverageRequirement::Required,
        run: run_component_graph_provider,
    },
    AuditProviderDefinition {
        id: "block_graph",
        kind: AuditProviderKind::BlockGraph,
        publish_coverage_requirement: AuditPublishCoverageRequirement::Required,
        run: run_block_graph_provider,
    },
    AuditProviderDefinition {
        id: "content_models",
        kind: AuditProviderKind::ContentModels,
        publish_coverage_requirement: AuditPublishCoverageRequirement::Required,
        run: run_content_models_provider,
    },
    AuditProviderDefinition {
        id: "listing_items",
        kind: AuditProviderKind::ListingItems,
        publish_coverage_requirement: AuditPublishCoverageRequirement::Required,
        run: run_listing_items_provider,
    },
    AuditProviderDefinition {
        id: "dynamic_widgets",
        kind: AuditProviderKind::DynamicWidgets,
        publish_coverage_requirement: AuditPublishCoverageRequirement::Required,
        run: run_dynamic_widgets_provider,
    },
    AuditProviderDefinition {
        id: "template_semantics",
        kind: AuditProviderKind::TemplateSemantics,
        publish_coverage_requirement: AuditPublishCoverageRequirement::Required,
        run: run_template_semantics_provider,
    },
    AuditProviderDefinition {
        id: "content_semantics",
        kind: AuditProviderKind::ContentSemantics,
        publish_coverage_requirement: AuditPublishCoverageRequirement::Required,
        run: run_content_semantics_provider,
    },
    AuditProviderDefinition {
        id: "asset_usage",
        kind: AuditProviderKind::AssetUsage,
        publish_coverage_requirement: AuditPublishCoverageRequirement::Advisory,
        run: run_asset_usage_provider,
    },
    AuditProviderDefinition {
        id: "build_zola",
        kind: AuditProviderKind::BuildZola,
        publish_coverage_requirement: AuditPublishCoverageRequirement::Advisory,
        run: run_build_provider,
    },
];

pub fn build_audit_run(
    model: &ProjectModel,
    file_buffer_diagnostics: &[FileBufferDiagnostic],
    runtime_session_id: String,
    workspace_revision: u64,
    request: AuditRequest,
    build_evidence: AuditBuildEvidence,
) -> Result<AuditRunReceipt, String> {
    validate_request(&request)?;
    let receipt_mode = request.mode;
    let receipt_scope = request.scope.clone();
    let mut context = AuditContext {
        model,
        file_buffer_diagnostics,
        request: &request,
        build_evidence: &build_evidence,
        findings: Vec::new(),
        seen: HashSet::new(),
        active_provider: "",
    };
    let mut providers = Vec::with_capacity(AUDIT_PROVIDER_REGISTRY.len());
    for provider in AUDIT_PROVIDER_REGISTRY {
        context.active_provider = provider.id;
        let before = context.findings.len();
        let execution = (provider.run)(&mut context);
        providers.push(AuditProviderReceipt {
            id: provider.id.to_string(),
            kind: provider.kind,
            status: execution.status,
            publish_coverage_requirement: provider.publish_coverage_requirement,
            finding_count: context.findings.len().saturating_sub(before),
            coverage: execution.coverage,
            evidence: execution.evidence,
        });
    }

    context.findings.sort_by(compare_findings);
    let summary = summarize(&context.findings);
    let completeness = if providers
        .iter()
        .all(|provider| provider.status == AuditProviderStatus::Complete)
    {
        AuditCompleteness::Complete
    } else {
        AuditCompleteness::Partial
    };

    Ok(AuditRunReceipt {
        schema_version: AUDIT_RUN_SCHEMA_VERSION,
        ruleset_version: AUDIT_RULESET_VERSION,
        project_root: model.project_root.to_string_lossy().to_string(),
        runtime_session_id,
        workspace_revision,
        project_model_revision: model.revision.clone(),
        mode: receipt_mode,
        scope: receipt_scope,
        completeness,
        summary,
        providers,
        findings: context.findings,
    })
}

fn validate_request(request: &AuditRequest) -> Result<(), String> {
    let mut rules = HashSet::new();
    for item in &request.policy_overrides {
        let code = item.rule_code.trim();
        if code.is_empty() || !rules.insert(code) {
            return Err("Audit a refuzat un override de policy gol sau duplicat.".to_string());
        }
    }
    for suppression in &request.suppressions {
        if suppression.rule_code.trim().is_empty() || suppression.reason.trim().is_empty() {
            return Err("Audit a refuzat o suprimare fără rule code sau justificare.".to_string());
        }
        if suppression.scope == AuditSuppressionScope::File
            && suppression.file.as_deref().is_none_or(str::is_empty)
        {
            return Err("Audit a refuzat o suprimare de fișier fără fișier.".to_string());
        }
        if suppression.scope == AuditSuppressionScope::Finding
            && !suppression
                .fingerprint
                .as_deref()
                .is_some_and(valid_fingerprint)
        {
            return Err(
                "Audit a refuzat o suprimare de constatare fără fingerprint valid.".to_string(),
            );
        }
    }
    if let super::model::AuditScope::File { path } = &request.scope {
        if path.trim().is_empty() {
            return Err("Audit a refuzat un scope de fișier gol.".to_string());
        }
    }
    Ok(())
}

struct AuditContext<'a> {
    model: &'a ProjectModel,
    file_buffer_diagnostics: &'a [FileBufferDiagnostic],
    request: &'a AuditRequest,
    build_evidence: &'a AuditBuildEvidence,
    findings: Vec<AuditFinding>,
    seen: HashSet<String>,
    active_provider: &'static str,
}

impl AuditContext<'_> {
    fn push(&mut self, mut candidate: AuditCandidate) {
        if !self.in_scope(candidate.primary_location.as_ref()) {
            return;
        }
        if let Some(policy) = self
            .request
            .policy_overrides
            .iter()
            .find(|item| item.rule_code == candidate.rule_code)
            .map(|item| item.policy)
        {
            candidate.policy = policy;
        }
        let fingerprint = finding_fingerprint(self.active_provider, &candidate);
        let suppression = self.matching_suppression(&candidate, &fingerprint);
        if suppression.is_some()
            && matches!(
                candidate.outcome,
                AuditOutcome::Violation | AuditOutcome::NeedsReview
            )
        {
            candidate.outcome = AuditOutcome::Suppressed;
        }
        if !self.seen.insert(fingerprint.clone()) {
            return;
        }
        self.findings.push(AuditFinding {
            id: format!("audit:{}", &fingerprint["sha256:".len()..][..24]),
            fingerprint,
            provider_id: self.active_provider.to_string(),
            rule_code: candidate.rule_code,
            category: candidate.category,
            outcome: candidate.outcome,
            impact: candidate.impact,
            policy: candidate.policy,
            title_diagnostic: candidate.title_diagnostic,
            message_diagnostic: candidate.message_diagnostic,
            primary_location: candidate.primary_location,
            related_locations: candidate.related_locations,
            evidence: candidate.evidence,
            fixes: candidate.fixes,
            suppression,
        });
    }

    fn in_scope(&self, location: Option<&AuditLocation>) -> bool {
        match &self.request.scope {
            super::model::AuditScope::Project => true,
            super::model::AuditScope::File { path } => {
                location.is_some_and(|location| location.file == *path)
            }
        }
    }

    fn matching_suppression(
        &self,
        candidate: &AuditCandidate,
        fingerprint: &str,
    ) -> Option<AuditSuppression> {
        self.request
            .suppressions
            .iter()
            .find(|suppression| {
                suppression.rule_code == candidate.rule_code
                    && match suppression.scope {
                        AuditSuppressionScope::Rule => true,
                        AuditSuppressionScope::File => candidate
                            .primary_location
                            .as_ref()
                            .and_then(|location| {
                                suppression.file.as_ref().map(|file| file == &location.file)
                            })
                            .unwrap_or(false),
                        AuditSuppressionScope::Finding => {
                            suppression.fingerprint.as_deref() == Some(fingerprint)
                        }
                    }
            })
            .cloned()
    }

    fn project_location(
        &self,
        file: impl Into<String>,
        range: Option<SourceRange>,
        source_node_id: Option<String>,
    ) -> AuditLocation {
        let file = file.into();
        AuditLocation {
            origin: origin_for_file(self.model, &file),
            file,
            range,
            source_node_id,
        }
    }

    fn node_location(
        &self,
        file: Option<&str>,
        source_node_id: Option<&str>,
    ) -> Option<AuditLocation> {
        if let Some(node_id) = source_node_id {
            if let Some(node) = self.model.source_graph.node_by_id(node_id) {
                return Some(AuditLocation {
                    file: node.file.clone(),
                    range: node.range.clone(),
                    origin: match node.origin {
                        SourceOrigin::Local => AuditSourceOrigin::Project,
                        SourceOrigin::Theme => AuditSourceOrigin::Theme,
                    },
                    source_node_id: Some(node.id.clone()),
                });
            }
        }
        file.map(|file| self.project_location(file, None, source_node_id.map(str::to_string)))
    }
}

fn valid_fingerprint(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

struct AuditCandidate {
    rule_code: String,
    category: AuditCategory,
    outcome: AuditOutcome,
    impact: AuditImpact,
    policy: AuditPolicy,
    title_diagnostic: LocalizedDiagnostic,
    message_diagnostic: LocalizedDiagnostic,
    primary_location: Option<AuditLocation>,
    related_locations: Vec<AuditLocation>,
    evidence: Vec<AuditEvidence>,
    fixes: Vec<AuditFix>,
}

fn diagnostic_candidate(
    code: impl Into<String>,
    category: AuditCategory,
    impact: AuditImpact,
    policy: AuditPolicy,
    title_code: &'static str,
    message: LocalizedDiagnostic,
    location: Option<AuditLocation>,
) -> AuditCandidate {
    AuditCandidate {
        rule_code: code.into(),
        category,
        outcome: AuditOutcome::Violation,
        impact,
        policy,
        title_diagnostic: LocalizedDiagnostic::new(title_code),
        message_diagnostic: message,
        primary_location: location,
        related_locations: Vec::new(),
        evidence: Vec::new(),
        fixes: Vec::new(),
    }
}

fn pass_candidate(rule_code: &str, category: AuditCategory) -> AuditCandidate {
    AuditCandidate {
        rule_code: rule_code.to_string(),
        category,
        outcome: AuditOutcome::Pass,
        impact: AuditImpact::Info,
        policy: AuditPolicy::Off,
        title_diagnostic: LocalizedDiagnostic::new("audit-provider-passed-title"),
        message_diagnostic: LocalizedDiagnostic::new("audit-provider-passed-message")
            .with_argument("provider", rule_code),
        primary_location: None,
        related_locations: Vec::new(),
        evidence: Vec::new(),
        fixes: Vec::new(),
    }
}

fn not_applicable_candidate(rule_code: &str, category: AuditCategory) -> AuditCandidate {
    AuditCandidate {
        rule_code: rule_code.to_string(),
        category,
        outcome: AuditOutcome::NotApplicable,
        impact: AuditImpact::Info,
        policy: AuditPolicy::Off,
        title_diagnostic: LocalizedDiagnostic::new("audit-provider-not-applicable-title"),
        message_diagnostic: LocalizedDiagnostic::new("audit-provider-not-applicable-message")
            .with_argument("provider", rule_code),
        primary_location: None,
        related_locations: Vec::new(),
        evidence: Vec::new(),
        fixes: Vec::new(),
    }
}

fn run_workspace_provider(context: &mut AuditContext<'_>) -> ProviderExecution {
    for diagnostic in context.file_buffer_diagnostics {
        context.push(diagnostic_candidate(
            diagnostic.code.clone(),
            AuditCategory::Workspace,
            match diagnostic.severity {
                FileBufferDiagnosticSeverity::Warning => AuditImpact::Moderate,
                FileBufferDiagnosticSeverity::Error => AuditImpact::Serious,
            },
            AuditPolicy::Blocking,
            "audit-title-workspace-file",
            diagnostic.message_diagnostic.clone(),
            diagnostic
                .relative_path
                .as_deref()
                .map(|file| context.project_location(file, None, None)),
        ));
    }
    if context.file_buffer_diagnostics.is_empty() {
        context.push(pass_candidate(
            "workspace_integrity",
            AuditCategory::Workspace,
        ));
    }
    ProviderExecution::complete(
        context.file_buffer_diagnostics.len(),
        context.file_buffer_diagnostics.len(),
    )
}

fn run_project_model_provider(context: &mut AuditContext<'_>) -> ProviderExecution {
    for diagnostic in &context.model.diagnostics {
        context.push(diagnostic_candidate(
            diagnostic.diagnostic.code.clone(),
            AuditCategory::Build,
            match diagnostic.severity {
                ProjectModelDiagnosticSeverity::Warning => AuditImpact::Moderate,
                ProjectModelDiagnosticSeverity::Error => AuditImpact::Serious,
            },
            AuditPolicy::Advisory,
            "audit-title-project-model",
            diagnostic.diagnostic.clone(),
            diagnostic
                .file
                .as_deref()
                .map(|file| context.project_location(file, diagnostic.range.clone(), None)),
        ));
    }
    if context.model.diagnostics.is_empty() {
        context.push(pass_candidate("project_model", AuditCategory::Build));
    }
    ProviderExecution::complete(context.model.files.len(), context.model.files.len())
}

fn run_source_conformance_provider(context: &mut AuditContext<'_>) -> ProviderExecution {
    let mut has_findings = !context.model.source_graph.diagnostics.is_empty();
    for diagnostic in &context.model.source_graph.diagnostics {
        context.push(diagnostic_candidate(
            diagnostic.diagnostic.code.clone(),
            AuditCategory::References,
            source_impact(&diagnostic.severity),
            if diagnostic.severity == SourceDiagnosticSeverity::Error {
                AuditPolicy::Blocking
            } else {
                AuditPolicy::Advisory
            },
            "audit-title-project-reference",
            diagnostic.diagnostic.clone(),
            diagnostic
                .file
                .as_deref()
                .map(|file| context.project_location(file, diagnostic.range.clone(), None)),
        ));
    }
    let indentation_audits = context
        .model
        .files
        .iter()
        .filter(|file| {
            matches!(
                file.kind,
                ProjectModelFileKind::Template | ProjectModelFileKind::StaticText
            ) && file.relative_path.to_ascii_lowercase().ends_with(".html")
        })
        .map(|file| {
            (
                file.relative_path.clone(),
                file.contents.clone(),
                audit_html_indentation(&file.contents, &file.relative_path),
            )
        })
        .collect::<Vec<_>>();
    for (file, contents, audit) in indentation_audits {
        match audit {
            Ok(audit) if !audit.issues.is_empty() => {
                has_findings = true;
                let location = context.project_location(&file, None, None);
                let count = audit.issues.len();
                let fixes = audit
                    .repaired_contents
                    .map(|_| {
                        vec![AuditFix {
                            id: format!("structural-indentation:{file}"),
                            title_diagnostic: LocalizedDiagnostic::new(
                                "audit-structural-indentation-fix-title",
                            ),
                            applicability: AuditFixApplicability::Safe,
                            edits: audit
                                .issues
                                .iter()
                                .map(|issue| AuditTextEdit {
                                    location: context.project_location(
                                        &file,
                                        Some(source_range(&contents, issue.start, issue.end)),
                                        None,
                                    ),
                                    replacement: issue.expected.clone(),
                                })
                                .collect(),
                        }]
                    })
                    .unwrap_or_default();
                context.push(AuditCandidate {
                    rule_code: "structural_indentation_drift".to_string(),
                    category: AuditCategory::Build,
                    outcome: AuditOutcome::Violation,
                    impact: AuditImpact::Moderate,
                    policy: AuditPolicy::Advisory,
                    title_diagnostic: LocalizedDiagnostic::new(
                        "audit-structural-indentation-title",
                    ),
                    message_diagnostic: LocalizedDiagnostic::new(
                        "audit-structural-indentation-message",
                    )
                    .with_argument("count", count.to_string())
                    .with_argument("file", file),
                    primary_location: Some(location),
                    related_locations: Vec::new(),
                    evidence: vec![AuditEvidence {
                        kind: AuditEvidenceKind::Parser,
                        diagnostic: LocalizedDiagnostic::new("audit-semantic-parser-evidence"),
                        value: Some(format!("issues={count}")),
                    }],
                    fixes,
                });
            }
            Ok(_) => {}
            Err(error) => {
                has_findings = true;
                context.push(AuditCandidate {
                    rule_code: "structural_indentation_unavailable".to_string(),
                    category: AuditCategory::Build,
                    outcome: AuditOutcome::NeedsReview,
                    impact: AuditImpact::Moderate,
                    policy: AuditPolicy::Advisory,
                    title_diagnostic: LocalizedDiagnostic::new(
                        "audit-structural-indentation-title",
                    ),
                    message_diagnostic: LocalizedDiagnostic::new(
                        "audit-provider-diagnostic-message",
                    )
                    .with_argument("details", error),
                    primary_location: Some(context.project_location(file, None, None)),
                    related_locations: Vec::new(),
                    evidence: Vec::new(),
                    fixes: Vec::new(),
                });
            }
        }
    }
    if !has_findings {
        context.push(pass_candidate(
            "source_conformance",
            AuditCategory::References,
        ));
    }
    ProviderExecution::complete(context.model.files.len(), context.model.files.len())
}

fn run_project_graph_provider(context: &mut AuditContext<'_>) -> ProviderExecution {
    context.push(pass_candidate("project_graph", AuditCategory::References));
    let graph = &context.model.source_graph;
    let analyzed = graph.nodes.len().saturating_add(graph.relations.len());
    ProviderExecution {
        status: AuditProviderStatus::Complete,
        coverage: AuditCoverage {
            eligible: analyzed,
            analyzed,
            limitations: Vec::new(),
        },
        evidence: vec![AuditEvidence {
            kind: AuditEvidenceKind::Graph,
            diagnostic: LocalizedDiagnostic::new("audit-graph-evidence"),
            value: Some(format!(
                "nodes={},relations={}",
                graph.nodes.len(),
                graph.relations.len()
            )),
        }],
    }
}

fn run_component_graph_provider(context: &mut AuditContext<'_>) -> ProviderExecution {
    let graph = &context.model.source_graph.component_graph;
    for diagnostic in graph
        .diagnostics
        .iter()
        .chain(
            graph
                .definitions
                .iter()
                .flat_map(|item| item.diagnostics.iter()),
        )
        .chain(
            graph
                .invocations
                .iter()
                .flat_map(|item| item.diagnostics.iter()),
        )
    {
        context.push(diagnostic_candidate(
            diagnostic.code.clone(),
            AuditCategory::Components,
            source_impact(&diagnostic.severity),
            AuditPolicy::Advisory,
            "audit-title-component-graph",
            diagnostic.diagnostic.clone(),
            context.node_location(
                diagnostic.file.as_deref(),
                diagnostic.source_node_id.as_deref(),
            ),
        ));
    }
    ProviderExecution::complete(
        graph.definitions.len() + graph.invocations.len(),
        graph.definitions.len() + graph.invocations.len(),
    )
}

fn run_block_graph_provider(context: &mut AuditContext<'_>) -> ProviderExecution {
    let graph = &context.model.source_graph.block_graph;
    for diagnostic in graph.diagnostics.iter().chain(
        graph
            .source_instances
            .iter()
            .flat_map(|item| item.diagnostics.iter()),
    ) {
        context.push(diagnostic_candidate(
            diagnostic.code.clone(),
            AuditCategory::Components,
            source_impact(&diagnostic.severity),
            AuditPolicy::Advisory,
            "audit-title-block-graph",
            diagnostic.diagnostic.clone(),
            context.node_location(
                diagnostic.file.as_deref(),
                diagnostic.source_node_id.as_deref(),
            ),
        ));
    }
    ProviderExecution::complete(
        graph.definitions.len() + graph.source_instances.len(),
        graph.definitions.len() + graph.source_instances.len(),
    )
}

fn run_content_models_provider(context: &mut AuditContext<'_>) -> ProviderExecution {
    let graph = &context.model.source_graph.content_models;
    for diagnostic in &graph.diagnostics {
        context.push(diagnostic_candidate(
            diagnostic.code.clone(),
            AuditCategory::Content,
            string_severity_impact(&diagnostic.severity),
            AuditPolicy::Advisory,
            "audit-title-content-model",
            generic_message(&diagnostic.message),
            diagnostic
                .file
                .as_deref()
                .map(|file| context.project_location(file, None, None)),
        ));
    }
    ProviderExecution::complete(graph.models.len(), graph.models.len())
}

fn run_listing_items_provider(context: &mut AuditContext<'_>) -> ProviderExecution {
    let graph = &context.model.source_graph.listing_items;
    for diagnostic in graph
        .diagnostics
        .iter()
        .chain(graph.items.iter().flat_map(|item| item.diagnostics.iter()))
    {
        context.push(diagnostic_candidate(
            diagnostic.code.clone(),
            AuditCategory::Components,
            AuditImpact::Moderate,
            AuditPolicy::Advisory,
            "audit-title-listing-item",
            generic_message(&diagnostic.message),
            diagnostic
                .file
                .as_deref()
                .map(|file| context.project_location(file, None, None)),
        ));
    }
    ProviderExecution::complete(graph.items.len(), graph.items.len())
}

fn run_dynamic_widgets_provider(context: &mut AuditContext<'_>) -> ProviderExecution {
    let graph = &context.model.source_graph.dynamic_widget_graph;
    let ranges = graph
        .source_instances
        .iter()
        .map(|item| (item.instance_id.as_str(), (&item.file, &item.range)))
        .collect::<HashMap<_, _>>();
    for diagnostic in graph.diagnostics.iter().chain(
        graph
            .source_instances
            .iter()
            .flat_map(|item| item.diagnostics.iter()),
    ) {
        let located = diagnostic
            .instance_id
            .as_deref()
            .and_then(|id| ranges.get(id))
            .map(|(file, range)| {
                context.project_location((*file).clone(), Some((*range).clone()), None)
            })
            .or_else(|| {
                diagnostic
                    .file
                    .as_deref()
                    .map(|file| context.project_location(file, None, None))
            });
        context.push(diagnostic_candidate(
            diagnostic.code.clone(),
            AuditCategory::Components,
            AuditImpact::Moderate,
            AuditPolicy::Advisory,
            "audit-title-dynamic-widget",
            generic_message(&diagnostic.message),
            located,
        ));
    }
    ProviderExecution::complete(graph.source_instances.len(), graph.source_instances.len())
}

fn run_template_semantics_provider(context: &mut AuditContext<'_>) -> ProviderExecution {
    let files = context
        .model
        .files
        .iter()
        .filter(|file| file.kind == ProjectModelFileKind::Template)
        .collect::<Vec<_>>();
    if files.is_empty() {
        context.push(not_applicable_candidate(
            "template_semantics",
            AuditCategory::Accessibility,
        ));
        return ProviderExecution::complete(0, 0);
    }
    let mut analyzed = 0;
    let mut incomplete = false;
    for file in &files {
        let document = parse_mixed_cst(&file.contents, &file.relative_path);
        if !document.is_lossless() || !document.tera.is_valid_tera() {
            incomplete = true;
            continue;
        }
        analyzed += 1;
        audit_template_cst(context, file, &document);
    }
    if incomplete {
        ProviderExecution::partial(
            files.len(),
            analyzed,
            vec![LocalizedDiagnostic::new(
                "audit-template-coverage-limitation",
            )],
        )
    } else {
        ProviderExecution::complete(files.len(), analyzed)
    }
}

fn audit_template_cst(
    context: &mut AuditContext<'_>,
    file: &ProjectModelFile,
    document: &crate::source_graph::mixed_cst::MixedCstDocument,
) {
    let mut html = None;
    let mut head = None;
    let mut has_title = false;
    for node in &document.nodes {
        let MixedCstKind::StartTag(tag) = &node.kind else {
            continue;
        };
        match tag.name.as_str() {
            "img" if !has_attribute(tag, "alt") => {
                let dynamic = !tag.embedded_tera.is_empty();
                context.push(semantic_candidate(
                    "image_missing_alt",
                    AuditCategory::Accessibility,
                    if dynamic {
                        AuditOutcome::NeedsReview
                    } else {
                        AuditOutcome::Violation
                    },
                    AuditImpact::Serious,
                    "audit-image-missing-alt-title",
                    "audit-image-missing-alt-message",
                    context.project_location(
                        &file.relative_path,
                        Some(source_range(&file.contents, node.start, node.end)),
                        None,
                    ),
                ));
            }
            "html" => html = Some((node, tag)),
            "head" => head = Some(node),
            "title" => has_title = true,
            _ => {}
        }
    }
    if let Some((node, tag)) = html {
        if !has_attribute(tag, "lang") {
            let dynamic = !tag.embedded_tera.is_empty();
            context.push(semantic_candidate(
                "html_missing_lang",
                AuditCategory::Accessibility,
                if dynamic {
                    AuditOutcome::NeedsReview
                } else {
                    AuditOutcome::Violation
                },
                AuditImpact::Moderate,
                "audit-html-missing-lang-title",
                "audit-html-missing-lang-message",
                context.project_location(
                    &file.relative_path,
                    Some(source_range(&file.contents, node.start, node.end)),
                    None,
                ),
            ));
        }
    }
    if let Some(head) = head {
        if !has_title {
            let template = context
                .model
                .source_graph
                .templates
                .iter()
                .find(|template| template.file == file.relative_path);
            let dynamic = template.is_some_and(|template| {
                template.is_partial
                    || template.extends.is_some()
                    || !template.includes.is_empty()
                    || !template.blocks.is_empty()
            });
            context.push(semantic_candidate(
                "document_missing_title",
                AuditCategory::Seo,
                if dynamic {
                    AuditOutcome::NeedsReview
                } else {
                    AuditOutcome::Violation
                },
                AuditImpact::Moderate,
                "audit-document-missing-title-title",
                "audit-document-missing-title-message",
                context.project_location(
                    &file.relative_path,
                    Some(source_range(&file.contents, head.start, head.end)),
                    None,
                ),
            ));
        }
    }
}

fn run_content_semantics_provider(context: &mut AuditContext<'_>) -> ProviderExecution {
    let pages = context.model.source_graph.pages.clone();
    if pages.is_empty() {
        context.push(not_applicable_candidate(
            "content_semantics",
            AuditCategory::Content,
        ));
        return ProviderExecution::complete(0, 0);
    }
    let mut analyzed = 0;
    let mut incomplete = false;
    for page in &pages {
        if page.frontmatter_parse_error.is_some() {
            incomplete = true;
            continue;
        }
        analyzed += 1;
        let location = context.project_location(
            &page.file,
            frontmatter_document_range(&page.frontmatter_nodes),
            Some(page.content_node_id.clone()),
        );
        if !frontmatter_has_key(&page.frontmatter_nodes, "title") {
            context.push(semantic_candidate(
                "content_missing_title",
                AuditCategory::Seo,
                AuditOutcome::Violation,
                AuditImpact::Moderate,
                "audit-content-missing-title-title",
                "audit-content-missing-title-message",
                location.clone(),
            ));
        }
        if !frontmatter_has_key(&page.frontmatter_nodes, "description") {
            context.push(semantic_candidate(
                "content_missing_description",
                AuditCategory::Seo,
                AuditOutcome::NeedsReview,
                AuditImpact::Minor,
                "audit-content-missing-description-title",
                "audit-content-missing-description-message",
                location,
            ));
        }
    }
    if incomplete {
        ProviderExecution::partial(
            pages.len(),
            analyzed,
            vec![LocalizedDiagnostic::new(
                "audit-content-coverage-limitation",
            )],
        )
    } else {
        ProviderExecution::complete(pages.len(), analyzed)
    }
}

fn run_asset_usage_provider(context: &mut AuditContext<'_>) -> ProviderExecution {
    let graph = &context.model.source_graph;
    let referenced = graph
        .relations
        .iter()
        .map(|relation| relation.to.as_str())
        .collect::<HashSet<_>>();
    let assets = graph.assets.clone();
    if assets.is_empty() {
        context.push(not_applicable_candidate(
            "asset_usage",
            AuditCategory::Assets,
        ));
        return ProviderExecution::complete(0, 0);
    }
    for asset in &assets {
        if asset.origin != SourceOrigin::Local || referenced.contains(asset.node_id.as_str()) {
            continue;
        }
        let mut candidate = semantic_candidate(
            "asset_without_known_usage",
            AuditCategory::Assets,
            AuditOutcome::NeedsReview,
            AuditImpact::Info,
            "audit-asset-without-usage-title",
            "audit-asset-without-usage-message",
            context.project_location(&asset.file, None, Some(asset.node_id.clone())),
        );
        candidate.message_diagnostic =
            LocalizedDiagnostic::new("audit-asset-without-usage-message")
                .with_argument("path", asset.logical_path.clone());
        candidate.evidence.push(AuditEvidence {
            kind: AuditEvidenceKind::Coverage,
            diagnostic: LocalizedDiagnostic::new("audit-asset-coverage-evidence"),
            value: Some(
                "html_asset_attributes,css_url,get_url,get_hash,image,conventional_style,conventional_script"
                    .to_string(),
            ),
        });
        context.push(candidate);
    }
    let coverage = &graph.asset_reference_coverage;
    let eligible = assets.len().saturating_add(coverage.eligible);
    let analyzed = assets.len().saturating_add(coverage.analyzed);
    if coverage.unanalysable > 0 {
        ProviderExecution::partial(
            eligible,
            analyzed,
            vec![LocalizedDiagnostic::new("audit-asset-coverage-limitation")
                .with_argument("count", coverage.unanalysable as u64)],
        )
    } else {
        ProviderExecution::complete(eligible, analyzed)
    }
}

fn run_build_provider(context: &mut AuditContext<'_>) -> ProviderExecution {
    match context.build_evidence {
        AuditBuildEvidence::Complete { message } => {
            let mut candidate = pass_candidate("build_zola", AuditCategory::Build);
            candidate.evidence.push(AuditEvidence {
                kind: AuditEvidenceKind::Build,
                diagnostic: LocalizedDiagnostic::new("audit-build-evidence"),
                value: Some(message.clone()),
            });
            context.push(candidate);
            ProviderExecution::complete(1, 1)
        }
        AuditBuildEvidence::Failed { message } => {
            let mut candidate = diagnostic_candidate(
                "build_zola_engine_error",
                AuditCategory::Build,
                AuditImpact::Serious,
                AuditPolicy::Blocking,
                "audit-build-provider-failed-title",
                LocalizedDiagnostic::new("audit-build-provider-failed-message")
                    .with_argument("details", message.clone()),
                None,
            );
            candidate.outcome = AuditOutcome::EngineError;
            context.push(candidate);
            ProviderExecution {
                status: AuditProviderStatus::Failed,
                coverage: AuditCoverage {
                    eligible: 1,
                    analyzed: 0,
                    limitations: vec![LocalizedDiagnostic::new("audit-build-coverage-failed")],
                },
                evidence: Vec::new(),
            }
        }
        AuditBuildEvidence::Skipped { message } => {
            let mut candidate = diagnostic_candidate(
                "build_zola_skipped",
                AuditCategory::Build,
                AuditImpact::Info,
                AuditPolicy::Advisory,
                "audit-build-provider-skipped-title",
                LocalizedDiagnostic::new("audit-build-provider-skipped-message")
                    .with_argument("details", message.clone()),
                None,
            );
            candidate.outcome = AuditOutcome::Skipped;
            context.push(candidate);
            ProviderExecution {
                status: AuditProviderStatus::Skipped,
                coverage: AuditCoverage {
                    eligible: 1,
                    analyzed: 0,
                    limitations: vec![LocalizedDiagnostic::new("audit-build-coverage-skipped")],
                },
                evidence: Vec::new(),
            }
        }
    }
}

fn semantic_candidate(
    rule_code: &str,
    category: AuditCategory,
    outcome: AuditOutcome,
    impact: AuditImpact,
    title_code: &'static str,
    message_code: &'static str,
    location: AuditLocation,
) -> AuditCandidate {
    AuditCandidate {
        rule_code: rule_code.to_string(),
        category,
        outcome,
        impact,
        policy: AuditPolicy::Advisory,
        title_diagnostic: LocalizedDiagnostic::new(title_code),
        message_diagnostic: LocalizedDiagnostic::new(message_code),
        primary_location: Some(location),
        related_locations: Vec::new(),
        evidence: vec![AuditEvidence {
            kind: AuditEvidenceKind::Parser,
            diagnostic: LocalizedDiagnostic::new("audit-semantic-parser-evidence"),
            value: None,
        }],
        fixes: Vec::new(),
    }
}

fn has_attribute(tag: &crate::source_graph::mixed_cst::HtmlStartTagCst, name: &str) -> bool {
    tag.attributes
        .iter()
        .any(|attribute| attribute.name.eq_ignore_ascii_case(name))
}

fn frontmatter_has_key(nodes: &[SourceDataNode], key: &str) -> bool {
    nodes.iter().any(|node| {
        node.path.as_slice() == [SourceDataPathSegment::Key(key.to_string())]
            || node.key.as_deref() == Some(key) && node.path.len() == 1
    })
}

fn frontmatter_document_range(nodes: &[SourceDataNode]) -> Option<SourceRange> {
    nodes
        .iter()
        .find(|node| node.path.is_empty())
        .and_then(|node| node.range.clone())
        .or_else(|| nodes.iter().find_map(|node| node.range.clone()))
}

fn generic_message(message: &str) -> LocalizedDiagnostic {
    LocalizedDiagnostic::new("audit-provider-diagnostic-message")
        .with_argument("details", message.to_string())
}

fn source_impact(severity: &SourceDiagnosticSeverity) -> AuditImpact {
    match severity {
        SourceDiagnosticSeverity::Warning => AuditImpact::Moderate,
        SourceDiagnosticSeverity::Error => AuditImpact::Serious,
    }
}

fn string_severity_impact(severity: &str) -> AuditImpact {
    if severity.eq_ignore_ascii_case("error") {
        AuditImpact::Serious
    } else {
        AuditImpact::Moderate
    }
}

fn origin_for_file(model: &ProjectModel, file: &str) -> AuditSourceOrigin {
    model
        .source_graph
        .nodes
        .iter()
        .find(|node| node.file == file)
        .map(|node| match node.origin {
            SourceOrigin::Local => AuditSourceOrigin::Project,
            SourceOrigin::Theme => AuditSourceOrigin::Theme,
        })
        .unwrap_or(AuditSourceOrigin::Workspace)
}

fn finding_fingerprint(provider_id: &str, candidate: &AuditCandidate) -> String {
    let location = candidate.primary_location.as_ref();
    let origin = location
        .map(|location| format!("{:?}", location.origin))
        .unwrap_or_default();
    let file = location
        .map(|location| location.file.as_str())
        .unwrap_or("");
    let start = location
        .and_then(|location| location.range.as_ref())
        .map(|range| range.start)
        .unwrap_or(0);
    let end = location
        .and_then(|location| location.range.as_ref())
        .map(|range| range.end)
        .unwrap_or(0);
    let canonical = format!(
        "audit-ruleset-{AUDIT_RULESET_VERSION}\0{provider_id}\0{}\0{origin}\0{file}\0{start}\0{end}",
        candidate.rule_code
    );
    format!("sha256:{:x}", Sha256::digest(canonical.as_bytes()))
}

fn source_range(source: &str, start: usize, end: usize) -> SourceRange {
    let start = start.min(source.len());
    let end = end.max(start).min(source.len());
    let (line, column) = line_column(source, start);
    let (end_line, end_column) = line_column(source, end);
    SourceRange {
        start,
        end,
        line,
        column,
        end_line,
        end_column,
    }
}

fn line_column(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;
    for (index, character) in source.char_indices() {
        if index >= offset {
            break;
        }
        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

fn summarize(findings: &[AuditFinding]) -> AuditSummary {
    AuditSummary {
        total: findings.len(),
        violations: count_outcome(findings, AuditOutcome::Violation),
        needs_review: count_outcome(findings, AuditOutcome::NeedsReview),
        engine_errors: count_outcome(findings, AuditOutcome::EngineError),
        passed: count_outcome(findings, AuditOutcome::Pass),
        not_applicable: count_outcome(findings, AuditOutcome::NotApplicable),
        skipped: count_outcome(findings, AuditOutcome::Skipped),
        suppressed: count_outcome(findings, AuditOutcome::Suppressed),
        blocking: findings
            .iter()
            .filter(|finding| {
                finding.policy == AuditPolicy::Blocking
                    && matches!(
                        finding.outcome,
                        AuditOutcome::Violation | AuditOutcome::EngineError
                    )
            })
            .count(),
        affected_files: findings
            .iter()
            .filter_map(|finding| {
                finding
                    .primary_location
                    .as_ref()
                    .map(|location| &location.file)
            })
            .collect::<HashSet<_>>()
            .len(),
    }
}

fn count_outcome(findings: &[AuditFinding], outcome: AuditOutcome) -> usize {
    findings
        .iter()
        .filter(|finding| finding.outcome == outcome)
        .count()
}

fn compare_findings(left: &AuditFinding, right: &AuditFinding) -> std::cmp::Ordering {
    outcome_rank(left.outcome)
        .cmp(&outcome_rank(right.outcome))
        .then_with(|| policy_rank(left.policy).cmp(&policy_rank(right.policy)))
        .then_with(|| right.impact.cmp(&left.impact))
        .then_with(|| {
            left.primary_location
                .as_ref()
                .map(|location| &location.file)
                .cmp(
                    &right
                        .primary_location
                        .as_ref()
                        .map(|location| &location.file),
                )
        })
        .then_with(|| left.rule_code.cmp(&right.rule_code))
        .then_with(|| left.fingerprint.cmp(&right.fingerprint))
}

fn outcome_rank(outcome: AuditOutcome) -> u8 {
    match outcome {
        AuditOutcome::EngineError => 0,
        AuditOutcome::Violation => 1,
        AuditOutcome::NeedsReview => 2,
        AuditOutcome::Suppressed => 3,
        AuditOutcome::Skipped => 4,
        AuditOutcome::Pass => 5,
        AuditOutcome::NotApplicable => 6,
    }
}

fn policy_rank(policy: AuditPolicy) -> u8 {
    match policy {
        AuditPolicy::Blocking => 0,
        AuditPolicy::Budget => 1,
        AuditPolicy::Advisory => 2,
        AuditPolicy::Off => 3,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        kernel::{
            content_models::ContentModelDiagnostic, dynamic_widgets::DynamicWidgetDiagnostic,
            listing_items::ListingItemDiagnostic, project_workspace::WorkspaceProjectionSnapshot,
        },
        project::{AcceptedProjectDiskManifest, ProjectDiskManifest},
        project_model::build_project_model_for_audit_from_workspace_projection,
        source_graph::model::{BlockDiagnostic, ComponentDiagnostic},
    };

    use super::*;

    #[test]
    fn multiline_ranges_keep_exact_end_position() {
        let source = "<div>\n  text\n</div>";
        let range = source_range(source, 0, source.len());
        assert_eq!((range.line, range.column), (1, 1));
        assert_eq!((range.end_line, range.end_column), (3, 7));
    }

    #[test]
    fn mixed_cst_ignores_tags_inside_html_comments() {
        let document = parse_mixed_cst("<!-- <img src='x'> --><img alt='' src='x'>", "test.html");
        let images = document
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                MixedCstKind::StartTag(tag) if tag.name == "img" => Some(tag),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(images.len(), 1);
        assert!(has_attribute(images[0], "alt"));
    }

    #[test]
    fn fingerprint_uses_rule_origin_and_full_range() {
        let base = AuditCandidate {
            rule_code: "rule".to_string(),
            category: AuditCategory::Seo,
            outcome: AuditOutcome::Violation,
            impact: AuditImpact::Moderate,
            policy: AuditPolicy::Advisory,
            title_diagnostic: LocalizedDiagnostic::new("title"),
            message_diagnostic: LocalizedDiagnostic::new("message"),
            primary_location: Some(AuditLocation {
                file: "templates/index.html".to_string(),
                range: Some(source_range("<head></head>", 0, 6)),
                origin: AuditSourceOrigin::Project,
                source_node_id: None,
            }),
            related_locations: Vec::new(),
            evidence: Vec::new(),
            fixes: Vec::new(),
        };
        let first = finding_fingerprint("provider", &base);
        let mut changed = base;
        changed
            .primary_location
            .as_mut()
            .unwrap()
            .range
            .as_mut()
            .unwrap()
            .end += 1;
        assert_ne!(first, finding_fingerprint("provider", &changed));
    }

    #[test]
    fn request_rejects_unjustified_suppression() {
        let request = AuditRequest {
            suppressions: vec![AuditSuppression {
                rule_code: "rule".to_string(),
                file: None,
                fingerprint: None,
                scope: AuditSuppressionScope::Rule,
                reason: String::new(),
            }],
            ..Default::default()
        };
        assert!(validate_request(&request).is_err());
    }

    #[test]
    fn finding_suppression_requires_an_exact_fingerprint() {
        let request = AuditRequest {
            suppressions: vec![AuditSuppression {
                rule_code: "rule".to_string(),
                file: None,
                fingerprint: None,
                scope: AuditSuppressionScope::Finding,
                reason: "accepted exception".to_string(),
            }],
            ..Default::default()
        };
        assert!(validate_request(&request).is_err());
        assert!(valid_fingerprint(
            "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        ));
    }

    #[test]
    fn exact_suppression_and_policy_override_preserve_the_fact_separately() {
        let (root, projection) = projected_model(HashMap::from([
            ("zola.toml".to_string(), "base_url = '/'\n".to_string()),
            (
                "templates/index.html".to_string(),
                "<html lang='ro'><head></head></html>".to_string(),
            ),
        ]));
        let model =
            build_project_model_for_audit_from_workspace_projection(&root, &projection).unwrap();
        let first = audit(
            &model,
            AuditBuildEvidence::Skipped {
                message: "quick".to_string(),
            },
        );
        let original = first
            .findings
            .iter()
            .find(|finding| finding.rule_code == "document_missing_title")
            .unwrap();
        let request = AuditRequest {
            policy_overrides: vec![crate::kernel::audit::model::AuditPolicyOverride {
                rule_code: original.rule_code.clone(),
                policy: AuditPolicy::Blocking,
            }],
            suppressions: vec![AuditSuppression {
                rule_code: original.rule_code.clone(),
                file: None,
                fingerprint: Some(original.fingerprint.clone()),
                scope: AuditSuppressionScope::Finding,
                reason: "accepted exception".to_string(),
            }],
            ..Default::default()
        };
        let second = build_audit_run(
            &model,
            &[],
            "audit-test-session".to_string(),
            7,
            request,
            AuditBuildEvidence::Skipped {
                message: "quick".to_string(),
            },
        )
        .unwrap();
        fs::remove_dir_all(root).unwrap();
        let suppressed = second
            .findings
            .iter()
            .find(|finding| finding.fingerprint == original.fingerprint)
            .unwrap();

        assert_eq!(suppressed.outcome, AuditOutcome::Suppressed);
        assert_eq!(suppressed.policy, AuditPolicy::Blocking);
        assert_eq!(
            suppressed.suppression.as_ref().unwrap().reason,
            "accepted exception"
        );
        assert_eq!(second.summary.blocking, 0);
    }

    #[test]
    fn invalid_project_source_becomes_finding_instead_of_aborting_audit() {
        let (root, projection) = projected_model(HashMap::from([
            ("zola.toml".to_string(), "base_url = '/'\n".to_string()),
            (
                "templates/index.html".to_string(),
                "{% if broken %}<main>{% endif".to_string(),
            ),
        ]));
        let model = build_project_model_for_audit_from_workspace_projection(&root, &projection)
            .expect("tolerant audit model");
        let receipt = audit(
            &model,
            AuditBuildEvidence::Skipped {
                message: "quick".to_string(),
            },
        );
        fs::remove_dir_all(root).unwrap();

        assert!(receipt.findings.iter().any(|finding| {
            finding.provider_id == "source_conformance"
                && finding.rule_code == "source-graph-tera-syntax-invalid"
                && finding.outcome == AuditOutcome::Violation
        }));
        assert!(receipt
            .providers
            .iter()
            .any(|provider| provider.id == "project_graph"));
    }

    #[test]
    fn structural_indentation_audit_offers_one_safe_lossless_repair() {
        let source = concat!(
            "  <section>\n",
            "    <div>\n",
            "            <article>\n",
            "        <p>Text</p>\n",
            "            </article>\n",
            "    </div>\n",
            "  </section>\n",
        );
        let (root, projection) = projected_model(HashMap::from([
            ("zola.toml".to_string(), "base_url = '/'\n".to_string()),
            ("templates/index.html".to_string(), source.to_string()),
        ]));
        let model =
            build_project_model_for_audit_from_workspace_projection(&root, &projection).unwrap();
        let receipt = audit(
            &model,
            AuditBuildEvidence::Skipped {
                message: "quick".to_string(),
            },
        );
        fs::remove_dir_all(root).unwrap();

        let finding = receipt
            .findings
            .iter()
            .find(|finding| finding.rule_code == "structural_indentation_drift")
            .expect("structural indentation finding");
        assert_eq!(finding.fixes.len(), 1);
        assert_eq!(finding.fixes[0].applicability, AuditFixApplicability::Safe);
        assert_eq!(finding.fixes[0].edits.len(), 2);
        let mut repaired = source.to_string();
        let mut edits = finding.fixes[0]
            .edits
            .iter()
            .map(|edit| {
                let range = edit.location.range.as_ref().expect("prefix edit range");
                (range.start, range.end, edit.replacement.as_str())
            })
            .collect::<Vec<_>>();
        edits.sort_unstable_by_key(|(start, _, _)| std::cmp::Reverse(*start));
        for (start, end, replacement) in edits {
            repaired.replace_range(start..end, replacement);
        }
        assert_eq!(
            repaired,
            concat!(
                "  <section>\n",
                "    <div>\n",
                "      <article>\n",
                "        <p>Text</p>\n",
                "      </article>\n",
                "    </div>\n",
                "  </section>\n",
            )
        );
        assert!(audit_html_indentation(&repaired, "templates/index.html")
            .unwrap()
            .issues
            .is_empty());
    }

    #[test]
    fn nested_graph_diagnostics_are_aggregated_and_deduplicated() {
        let (root, projection) = projected_model(HashMap::from([
            ("zola.toml".to_string(), "base_url = '/'\n".to_string()),
            (
                "templates/index.html".to_string(),
                "<main></main>".to_string(),
            ),
        ]));
        let mut model =
            build_project_model_for_audit_from_workspace_projection(&root, &projection).unwrap();
        let component = ComponentDiagnostic {
            code: "component_nested".to_string(),
            diagnostic: LocalizedDiagnostic::new("component-nested"),
            severity: SourceDiagnosticSeverity::Warning,
            file: Some("templates/index.html".to_string()),
            source_node_id: None,
        };
        model
            .source_graph
            .component_graph
            .diagnostics
            .push(component.clone());
        model
            .source_graph
            .component_graph
            .diagnostics
            .push(component);
        model
            .source_graph
            .block_graph
            .diagnostics
            .push(BlockDiagnostic {
                code: "block_nested".to_string(),
                diagnostic: LocalizedDiagnostic::new("block-nested"),
                severity: SourceDiagnosticSeverity::Error,
                file: Some("templates/index.html".to_string()),
                source_node_id: None,
            });
        model
            .source_graph
            .content_models
            .diagnostics
            .push(ContentModelDiagnostic {
                severity: "error".to_string(),
                code: "content_nested".to_string(),
                message: "content".to_string(),
                file: Some(".panastudio/project.toml".to_string()),
            });
        model
            .source_graph
            .listing_items
            .diagnostics
            .push(ListingItemDiagnostic {
                code: "listing_nested".to_string(),
                message: "listing".to_string(),
                file: Some(".panastudio/listing-items.toml".to_string()),
                item_id: None,
            });
        model
            .source_graph
            .dynamic_widget_graph
            .diagnostics
            .push(DynamicWidgetDiagnostic {
                code: "widget_nested".to_string(),
                message: "widget".to_string(),
                file: Some("templates/index.html".to_string()),
                instance_id: None,
            });
        let receipt = audit(
            &model,
            AuditBuildEvidence::Skipped {
                message: "quick".to_string(),
            },
        );
        fs::remove_dir_all(root).unwrap();
        let codes = receipt
            .findings
            .iter()
            .map(|finding| finding.rule_code.as_str())
            .collect::<Vec<_>>();
        for code in [
            "component_nested",
            "block_nested",
            "content_nested",
            "listing_nested",
            "widget_nested",
        ] {
            assert!(codes.contains(&code), "missing {code}");
        }
        assert_eq!(
            codes
                .iter()
                .filter(|code| **code == "component_nested")
                .count(),
            1
        );
    }

    #[test]
    fn provider_failure_is_isolated_and_receipt_contract_is_versioned() {
        let (root, projection) = projected_model(HashMap::from([
            ("zola.toml".to_string(), "base_url = '/'\n".to_string()),
            (
                "templates/index.html".to_string(),
                "<html><head></head></html>".to_string(),
            ),
        ]));
        let model =
            build_project_model_for_audit_from_workspace_projection(&root, &projection).unwrap();
        let receipt = audit(
            &model,
            AuditBuildEvidence::Failed {
                message: "zola failed".to_string(),
            },
        );
        fs::remove_dir_all(root).unwrap();
        let json = serde_json::to_value(&receipt).unwrap();

        assert_eq!(json["schemaVersion"], AUDIT_RUN_SCHEMA_VERSION);
        assert_eq!(json["rulesetVersion"], AUDIT_RULESET_VERSION);
        assert_eq!(json["completeness"], "partial");
        assert!(receipt.providers.iter().any(|provider| {
            provider.id == "build_zola" && provider.status == AuditProviderStatus::Failed
        }));
        assert!(receipt.providers.iter().any(|provider| {
            provider.id == "template_semantics" && provider.status == AuditProviderStatus::Complete
        }));
        assert!(receipt
            .findings
            .iter()
            .any(|finding| finding.outcome == AuditOutcome::EngineError));
    }

    #[test]
    fn semantic_frontmatter_and_analyzable_asset_coverage_are_exact() {
        let (root, projection) = projected_model(HashMap::from([
            ("zola.toml".to_string(), "base_url = '/'\n".to_string()),
            (
                "content/page.md".to_string(),
                "+++\n# title = 'comment only'\n+++\nBody".to_string(),
            ),
            (
                "static/site.css".to_string(),
                "body { color: red; }".to_string(),
            ),
        ]));
        let model =
            build_project_model_for_audit_from_workspace_projection(&root, &projection).unwrap();
        let first = audit(
            &model,
            AuditBuildEvidence::Skipped {
                message: "quick".to_string(),
            },
        );
        let second = audit(
            &model,
            AuditBuildEvidence::Skipped {
                message: "quick".to_string(),
            },
        );
        fs::remove_dir_all(root).unwrap();

        assert!(first.findings.iter().any(|finding| {
            finding.rule_code == "content_missing_title"
                && finding.primary_location.as_ref().is_some_and(|location| {
                    location.file == "content/page.md" && location.range.is_some()
                })
        }));
        assert!(first.findings.iter().any(|finding| {
            finding.rule_code == "asset_without_known_usage"
                && finding.outcome == AuditOutcome::NeedsReview
        }));
        assert!(first.providers.iter().any(|provider| {
            provider.id == "asset_usage"
                && provider.status == AuditProviderStatus::Complete
                && provider.coverage.limitations.is_empty()
        }));
        let first_fingerprints = first
            .findings
            .iter()
            .map(|finding| &finding.fingerprint)
            .collect::<Vec<_>>();
        let second_fingerprints = second
            .findings
            .iter()
            .map(|finding| &finding.fingerprint)
            .collect::<Vec<_>>();
        assert_eq!(first_fingerprints, second_fingerprints);
    }

    #[test]
    fn literal_unicode_html_asset_reference_prevents_false_unused_finding() {
        let asset_path = "static/images/Captură de ecran de la 2026-07-30 17-20-08.png";
        let (root, projection) = projected_model(HashMap::from([
            ("zola.toml".to_string(), "base_url = '/'\n".to_string()),
            (
                "templates/index.html".to_string(),
                "<img src='/images/Captur%C4%83%20de%20ecran%20de%20la%202026-07-30%2017-20-08.png?v=1#preview' alt='Imagine'>".to_string(),
            ),
            (asset_path.to_string(), "test-fixture".to_string()),
        ]));
        let model =
            build_project_model_for_audit_from_workspace_projection(&root, &projection).unwrap();
        let receipt = audit(
            &model,
            AuditBuildEvidence::Skipped {
                message: "quick".to_string(),
            },
        );
        fs::remove_dir_all(root).unwrap();

        let asset = model
            .source_graph
            .assets
            .iter()
            .find(|asset| asset.file == asset_path)
            .expect("asset node");
        assert!(model
            .source_graph
            .relations
            .iter()
            .any(|relation| relation.to == asset.node_id));
        assert!(!receipt.findings.iter().any(|finding| {
            finding.rule_code == "asset_without_known_usage"
                && finding
                    .primary_location
                    .as_ref()
                    .is_some_and(|location| location.file == asset_path)
        }));
        assert!(receipt.providers.iter().any(|provider| {
            provider.id == "asset_usage" && provider.status == AuditProviderStatus::Complete
        }));
    }

    #[test]
    fn dynamic_asset_reference_is_the_only_reason_for_partial_coverage() {
        let (root, projection) = projected_model(HashMap::from([
            ("zola.toml".to_string(), "base_url = '/'\n".to_string()),
            (
                "templates/index.html".to_string(),
                "<img src='{{ image }}' alt='Imagine'>".to_string(),
            ),
            (
                "static/images/unused.png".to_string(),
                "fixture".to_string(),
            ),
        ]));
        let model =
            build_project_model_for_audit_from_workspace_projection(&root, &projection).unwrap();
        let receipt = audit(
            &model,
            AuditBuildEvidence::Skipped {
                message: "quick".to_string(),
            },
        );
        fs::remove_dir_all(root).unwrap();

        assert!(receipt.providers.iter().any(|provider| {
            provider.id == "asset_usage"
                && provider.status == AuditProviderStatus::Partial
                && provider.coverage.eligible == provider.coverage.analyzed + 1
        }));
    }

    #[test]
    fn inheritance_limits_title_rule_and_empty_domains_are_not_applicable() {
        let (root, projection) = projected_model(HashMap::from([
            ("zola.toml".to_string(), "base_url = '/'\n".to_string()),
            (
                "templates/base.html".to_string(),
                "<html lang='ro'><head>{% block title %}{% endblock title %}</head></html>"
                    .to_string(),
            ),
        ]));
        let model =
            build_project_model_for_audit_from_workspace_projection(&root, &projection).unwrap();
        let receipt = audit(
            &model,
            AuditBuildEvidence::Skipped {
                message: "quick".to_string(),
            },
        );
        fs::remove_dir_all(root).unwrap();

        assert!(receipt.findings.iter().any(|finding| {
            finding.rule_code == "document_missing_title"
                && finding.outcome == AuditOutcome::NeedsReview
        }));
        assert!(receipt.findings.iter().any(|finding| {
            finding.rule_code == "content_semantics"
                && finding.outcome == AuditOutcome::NotApplicable
        }));
        assert!(receipt.findings.iter().any(|finding| {
            finding.rule_code == "asset_usage" && finding.outcome == AuditOutcome::NotApplicable
        }));
    }

    #[test]
    fn exact_audit_projection_does_not_import_nested_metadata_from_disk() {
        let (root, projection) = projected_model(HashMap::from([
            (
                "zola.toml".to_string(),
                "base_url = '/'\noutput_dir = 'public'\n".to_string(),
            ),
            (
                "templates/index.html".to_string(),
                "{% set external = load_data(path='external.json') %}<main>Workspace</main>"
                    .to_string(),
            ),
        ]));
        fs::create_dir_all(root.join(".panastudio/models")).unwrap();
        fs::create_dir_all(root.join("public")).unwrap();
        fs::write(
            root.join(".panastudio/listing-items.toml"),
            "this is invalid disk metadata",
        )
        .unwrap();
        fs::write(
            root.join(".panastudio/project.toml"),
            "this is invalid disk metadata",
        )
        .unwrap();
        fs::write(
            root.join(".panastudio/models/external.toml"),
            "this is invalid disk metadata",
        )
        .unwrap();
        fs::write(root.join("public/external.json"), "{\"disk\":true}").unwrap();

        let model =
            build_project_model_for_audit_from_workspace_projection(&root, &projection).unwrap();
        fs::remove_dir_all(root).unwrap();

        assert!(!model.source_graph.listing_items.metadata_present);
        assert!(model.source_graph.listing_items.diagnostics.is_empty());
        assert!(!model.source_graph.content_models.metadata_present);
        assert!(model.source_graph.content_models.models.is_empty());
        assert!(model.source_graph.content_models.diagnostics.is_empty());
        assert!(!model
            .source_graph
            .data_files
            .iter()
            .any(|file| file.logical_path.contains("external.json")));
    }

    fn audit(model: &ProjectModel, build: AuditBuildEvidence) -> AuditRunReceipt {
        build_audit_run(
            model,
            &[],
            "audit-test-session".to_string(),
            7,
            AuditRequest::default(),
            build,
        )
        .unwrap()
    }

    fn projected_model(
        source_texts: HashMap<String, String>,
    ) -> (PathBuf, WorkspaceProjectionSnapshot) {
        let root = unique_test_dir();
        fs::create_dir_all(&root).unwrap();
        let canonical = root.canonicalize().unwrap().to_string_lossy().to_string();
        let session = "audit-test-session".to_string();
        let projection = WorkspaceProjectionSnapshot {
            project_root: canonical.clone(),
            runtime_session_id: session.clone(),
            revision: 7,
            workspace_transaction_id: None,
            changed_paths: source_texts.keys().cloned().collect(),
            source_texts,
            resource_bytes: HashMap::new(),
            deleted_sources: HashSet::new(),
            accepted_disk: AcceptedProjectDiskManifest::new(
                session,
                canonical.clone(),
                ProjectDiskManifest {
                    root: canonical,
                    files: Vec::new(),
                    truncated: false,
                    max_files: 10_000,
                },
            )
            .unwrap(),
        };
        (root, projection)
    }

    fn unique_test_dir() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pana-audit-engine-{stamp}"))
    }
}
