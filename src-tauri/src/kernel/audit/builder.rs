use std::{
    collections::{hash_map::DefaultHasher, HashSet},
    hash::{Hash, Hasher},
};

use crate::{
    kernel::file_buffer_store::{FileBufferDiagnostic, FileBufferDiagnosticSeverity},
    localization::LocalizedDiagnostic,
    project_model::model::{
        ProjectModel, ProjectModelDiagnosticSeverity, ProjectModelFile, ProjectModelFileKind,
    },
    source_graph::model::{SourceDiagnosticSeverity, SourceOrigin, SourceRange},
};

use super::model::{
    AuditCategory, AuditDiagnostic, AuditSeverity, AuditSummary, ProjectAuditSnapshot,
    PROJECT_AUDIT_SCHEMA_VERSION,
};

pub fn build_project_audit(
    model: &ProjectModel,
    file_buffer_diagnostics: &[FileBufferDiagnostic],
    runtime_session_id: String,
    workspace_revision: u64,
) -> ProjectAuditSnapshot {
    let mut diagnostics = Vec::new();
    let mut seen = HashSet::new();

    for diagnostic in &model.diagnostics {
        push_unique(
            &mut diagnostics,
            &mut seen,
            AuditCandidate {
                severity: match diagnostic.severity {
                    ProjectModelDiagnosticSeverity::Warning => AuditSeverity::Warning,
                    ProjectModelDiagnosticSeverity::Error => AuditSeverity::Error,
                },
                category: AuditCategory::Build,
                code: "project_model".to_string(),
                title_diagnostic: LocalizedDiagnostic::new("audit-title-project-model"),
                message_diagnostic: diagnostic.diagnostic.clone(),
                file: diagnostic.file.clone(),
                range: diagnostic.range.clone(),
            },
        );
    }

    for diagnostic in &model.source_graph.diagnostics {
        push_unique(
            &mut diagnostics,
            &mut seen,
            AuditCandidate {
                severity: match diagnostic.severity {
                    SourceDiagnosticSeverity::Warning => AuditSeverity::Warning,
                    SourceDiagnosticSeverity::Error => AuditSeverity::Error,
                },
                category: AuditCategory::References,
                code: "source_graph".to_string(),
                title_diagnostic: LocalizedDiagnostic::new("audit-title-project-reference"),
                message_diagnostic: diagnostic.diagnostic.clone(),
                file: diagnostic.file.clone(),
                range: diagnostic.range.clone(),
            },
        );
    }

    for diagnostic in file_buffer_diagnostics {
        push_unique(
            &mut diagnostics,
            &mut seen,
            AuditCandidate {
                severity: match diagnostic.severity {
                    FileBufferDiagnosticSeverity::Warning => AuditSeverity::Warning,
                    FileBufferDiagnosticSeverity::Error => AuditSeverity::Error,
                },
                category: AuditCategory::Workspace,
                code: diagnostic.code.clone(),
                title_diagnostic: LocalizedDiagnostic::new("audit-title-workspace-file"),
                message_diagnostic: diagnostic.message_diagnostic.clone(),
                file: diagnostic.relative_path.clone(),
                range: None,
            },
        );
    }

    for file in &model.files {
        match file.kind {
            ProjectModelFileKind::Template => audit_template(file, &mut diagnostics, &mut seen),
            ProjectModelFileKind::Content => audit_content(file, &mut diagnostics, &mut seen),
            _ => {}
        }
    }
    audit_unused_assets(model, &mut diagnostics, &mut seen);

    diagnostics.sort_by(|left, right| {
        severity_rank(left.severity)
            .cmp(&severity_rank(right.severity))
            .then_with(|| format!("{:?}", left.category).cmp(&format!("{:?}", right.category)))
            .then_with(|| left.file.cmp(&right.file))
            .then_with(|| {
                left.range
                    .as_ref()
                    .map(|range| range.line)
                    .cmp(&right.range.as_ref().map(|range| range.line))
            })
            .then_with(|| {
                left.message_diagnostic
                    .code
                    .cmp(&right.message_diagnostic.code)
            })
    });

    let affected_files = diagnostics
        .iter()
        .filter_map(|diagnostic| diagnostic.file.as_deref())
        .collect::<HashSet<_>>()
        .len();
    let summary = AuditSummary {
        total: diagnostics.len(),
        errors: diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == AuditSeverity::Error)
            .count(),
        warnings: diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == AuditSeverity::Warning)
            .count(),
        info: diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == AuditSeverity::Info)
            .count(),
        affected_files,
    };

    ProjectAuditSnapshot {
        schema_version: PROJECT_AUDIT_SCHEMA_VERSION,
        project_root: model.project_root.to_string_lossy().to_string(),
        runtime_session_id,
        workspace_revision,
        project_model_revision: model.revision.clone(),
        summary,
        diagnostics,
    }
}

fn audit_template(
    file: &ProjectModelFile,
    diagnostics: &mut Vec<AuditDiagnostic>,
    seen: &mut HashSet<String>,
) {
    // ASCII case folding keeps byte offsets aligned with the original source,
    // which is required for exact editor ranges in Unicode templates.
    let lower = file.contents.to_ascii_lowercase();
    for start in tag_offsets(&lower, "<img") {
        let end = lower[start..]
            .find('>')
            .map(|offset| start + offset + 1)
            .unwrap_or(lower.len());
        if !tag_has_attribute(&lower[start..end], "alt") {
            push_unique(
                diagnostics,
                seen,
                candidate_at(
                    AuditSeverity::Warning,
                    AuditCategory::Accessibility,
                    "image_missing_alt",
                    "audit-image-missing-alt-title",
                    "audit-image-missing-alt-message",
                    file,
                    start,
                    end,
                ),
            );
        }
    }

    if let Some(start) = lower.find("<html") {
        let end = lower[start..]
            .find('>')
            .map(|offset| start + offset + 1)
            .unwrap_or(lower.len());
        if !tag_has_attribute(&lower[start..end], "lang") {
            push_unique(
                diagnostics,
                seen,
                candidate_at(
                    AuditSeverity::Warning,
                    AuditCategory::Accessibility,
                    "html_missing_lang",
                    "audit-html-missing-lang-title",
                    "audit-html-missing-lang-message",
                    file,
                    start,
                    end,
                ),
            );
        }
    }

    if lower.contains("<head") && !lower.contains("<title") {
        let start = lower.find("<head").unwrap_or(0);
        push_unique(
            diagnostics,
            seen,
            candidate_at(
                AuditSeverity::Warning,
                AuditCategory::Seo,
                "document_missing_title",
                "audit-document-missing-title-title",
                "audit-document-missing-title-message",
                file,
                start,
                start.saturating_add(5),
            ),
        );
    }
}

fn audit_content(
    file: &ProjectModelFile,
    diagnostics: &mut Vec<AuditDiagnostic>,
    seen: &mut HashSet<String>,
) {
    let frontmatter = toml_frontmatter(&file.contents).unwrap_or("");
    let lower = frontmatter.to_ascii_lowercase();
    if !has_toml_key(&lower, "title") {
        push_unique(
            diagnostics,
            seen,
            AuditCandidate {
                severity: AuditSeverity::Warning,
                category: AuditCategory::Seo,
                code: "content_missing_title".to_string(),
                title_diagnostic: LocalizedDiagnostic::new("audit-content-missing-title-title"),
                message_diagnostic: LocalizedDiagnostic::new("audit-content-missing-title-message"),
                file: Some(file.relative_path.clone()),
                range: Some(range_at(&file.contents, 0, 0)),
            },
        );
    }
    if !has_toml_key(&lower, "description") {
        push_unique(
            diagnostics,
            seen,
            AuditCandidate {
                severity: AuditSeverity::Info,
                category: AuditCategory::Seo,
                code: "content_missing_description".to_string(),
                title_diagnostic: LocalizedDiagnostic::new(
                    "audit-content-missing-description-title",
                ),
                message_diagnostic: LocalizedDiagnostic::new(
                    "audit-content-missing-description-message",
                ),
                file: Some(file.relative_path.clone()),
                range: Some(range_at(&file.contents, 0, 0)),
            },
        );
    }
}

fn audit_unused_assets(
    model: &ProjectModel,
    diagnostics: &mut Vec<AuditDiagnostic>,
    seen: &mut HashSet<String>,
) {
    let referenced = model
        .source_graph
        .relations
        .iter()
        .map(|relation| relation.to.as_str())
        .collect::<HashSet<_>>();
    for asset in &model.source_graph.assets {
        if asset.origin != SourceOrigin::Local || referenced.contains(asset.node_id.as_str()) {
            continue;
        }
        push_unique(
            diagnostics,
            seen,
            AuditCandidate {
                severity: AuditSeverity::Info,
                category: AuditCategory::Assets,
                code: "asset_without_usage".to_string(),
                title_diagnostic: LocalizedDiagnostic::new("audit-asset-without-usage-title"),
                message_diagnostic: LocalizedDiagnostic::new("audit-asset-without-usage-message")
                    .with_argument("path", asset.logical_path.clone()),
                file: Some(asset.file.clone()),
                range: None,
            },
        );
    }
}

struct AuditCandidate {
    severity: AuditSeverity,
    category: AuditCategory,
    code: String,
    title_diagnostic: LocalizedDiagnostic,
    message_diagnostic: LocalizedDiagnostic,
    file: Option<String>,
    range: Option<SourceRange>,
}

#[allow(clippy::too_many_arguments)]
fn candidate_at(
    severity: AuditSeverity,
    category: AuditCategory,
    code: &str,
    title_code: &'static str,
    message_code: &'static str,
    file: &ProjectModelFile,
    start: usize,
    end: usize,
) -> AuditCandidate {
    AuditCandidate {
        severity,
        category,
        code: code.to_string(),
        title_diagnostic: LocalizedDiagnostic::new(title_code),
        message_diagnostic: LocalizedDiagnostic::new(message_code),
        file: Some(file.relative_path.clone()),
        range: Some(range_at(&file.contents, start, end)),
    }
}

fn push_unique(
    diagnostics: &mut Vec<AuditDiagnostic>,
    seen: &mut HashSet<String>,
    candidate: AuditCandidate,
) {
    let line = candidate
        .range
        .as_ref()
        .map(|range| range.line)
        .unwrap_or(0);
    let message_key = serde_json::to_string(&candidate.message_diagnostic)
        .unwrap_or_else(|_| candidate.message_diagnostic.code.clone());
    let key = format!(
        "{}\u{0}{}\u{0}{}",
        candidate.file.as_deref().unwrap_or("project"),
        line,
        message_key
    );
    if !seen.insert(key) {
        return;
    }
    let mut hasher = DefaultHasher::new();
    candidate.code.hash(&mut hasher);
    candidate.file.hash(&mut hasher);
    line.hash(&mut hasher);
    message_key.hash(&mut hasher);
    diagnostics.push(AuditDiagnostic {
        id: format!("audit:{:016x}", hasher.finish()),
        severity: candidate.severity,
        category: candidate.category,
        code: candidate.code,
        title_diagnostic: candidate.title_diagnostic,
        message_diagnostic: candidate.message_diagnostic,
        file: candidate.file,
        range: candidate.range,
    });
}

fn tag_offsets(source: &str, needle: &str) -> Vec<usize> {
    let mut offsets = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = source[cursor..].find(needle) {
        let absolute = cursor + offset;
        let boundary = source[absolute + needle.len()..].chars().next();
        if boundary.is_none_or(|character| {
            character.is_ascii_whitespace() || character == '>' || character == '/'
        }) {
            offsets.push(absolute);
        }
        cursor = absolute + needle.len();
    }
    offsets
}

fn tag_has_attribute(tag: &str, attribute: &str) -> bool {
    tag.match_indices(attribute).any(|(index, _)| {
        let before = tag[..index].chars().next_back();
        let after = tag[index + attribute.len()..].trim_start().chars().next();
        before.is_some_and(char::is_whitespace) && after == Some('=')
    })
}

fn toml_frontmatter(source: &str) -> Option<&str> {
    let trimmed = source.strip_prefix("\u{feff}").unwrap_or(source);
    let body = trimmed.strip_prefix("+++")?;
    let end = body.find("\n+++")?;
    Some(&body[..end])
}

fn has_toml_key(source: &str, key: &str) -> bool {
    source.lines().any(|line| {
        let line = line.trim_start();
        line.strip_prefix(key)
            .is_some_and(|rest| rest.trim_start().starts_with('='))
    })
}

fn range_at(source: &str, start: usize, end: usize) -> SourceRange {
    let safe_start = start.min(source.len());
    let safe_end = end.max(safe_start).min(source.len());
    let prefix = &source[..safe_start];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit_once('\n')
        .map(|(_, tail)| tail.chars().count() + 1)
        .unwrap_or_else(|| prefix.chars().count() + 1);
    SourceRange {
        start: safe_start,
        end: safe_end,
        line,
        column,
        end_line: line,
        end_column: column + source[safe_start..safe_end].chars().count(),
    }
}

fn severity_rank(severity: AuditSeverity) -> u8 {
    match severity {
        AuditSeverity::Error => 0,
        AuditSeverity::Warning => 1,
        AuditSeverity::Info => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::{has_toml_key, range_at, tag_has_attribute, tag_offsets, toml_frontmatter};

    #[test]
    fn html_attribute_detection_respects_attribute_boundaries() {
        assert!(tag_has_attribute(r#"<img src="a.webp" alt="">"#, "alt"));
        assert!(!tag_has_attribute(
            r#"<img src="a.webp" data-alt="decorativ">"#,
            "alt"
        ));
    }

    #[test]
    fn html_tag_detection_does_not_match_longer_tag_names() {
        assert!(
            tag_offsets("<image src=\"x.svg\"><img src=\"x.webp\">", "<img")
                .iter()
                .eq([19].iter())
        );
    }

    #[test]
    fn template_offsets_remain_valid_for_unicode_source() {
        let source = "<p>Pană</p>\n<IMG src=\"x.webp\">";
        let folded = source.to_ascii_lowercase();
        let offset = tag_offsets(&folded, "<img")[0];
        let range = range_at(source, offset, offset + 4);
        assert_eq!(range.line, 2);
        assert_eq!(range.column, 1);
    }

    #[test]
    fn zola_frontmatter_keys_are_detected() {
        let source = "+++\ntitle = \"Acasă\"\n[extra]\ndescription = \"Test\"\n+++\n";
        let frontmatter = toml_frontmatter(source).expect("frontmatter");
        assert!(has_toml_key(frontmatter, "title"));
        assert!(has_toml_key(frontmatter, "description"));
        assert!(!has_toml_key(frontmatter, "draft"));
    }
}
