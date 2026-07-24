use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::Path,
};

mod replacements;
mod targets;

use crate::{
    kernel::{
        file_buffer_store::FileBufferStore,
        project_path::normalize_project_relative_path,
        project_workspace::{WorkspaceTextChange, WorkspaceTextMutationInput},
    },
    source_graph::{
        build_source_graph,
        model::{SourceDiagnosticSeverity, SourceGraph, SourceRelationKind},
    },
};

use replacements::{
    plan_frontmatter_template_replacements, plan_tera_template_replacements,
    plan_zola_asset_function_replacements, plan_zola_content_function_replacements,
    plan_zola_content_load_function_replacements, plan_zola_data_file_function_replacements,
    relation_kind_label, TextReplacement,
};
use targets::rewrite_targets_for_entry;

use super::model::{
    SourceGraphReferenceRewritePlan, SourceGraphRewriteDiagnostic, SourceGraphRewriteOperation,
    SourceGraphRewriteSeverity, SourceGraphRewriteStatus, SOURCE_GRAPH_REWRITE_SCHEMA_VERSION,
    SOURCE_GRAPH_REWRITE_WORKSPACE_TARGET,
};

pub fn plan_template_reference_workspace_mutation(
    project_root: &Path,
    store: &FileBufferStore,
    operation: SourceGraphRewriteOperation,
    source_relative_path: &str,
    destination_relative_path: &str,
) -> Result<SourceGraphReferenceRewritePlan, String> {
    validate_store_identity(project_root, store)?;
    let source_relative_path = normalize_project_relative_path(source_relative_path)?;
    let destination_relative_path = normalize_project_relative_path(destination_relative_path)?;
    if source_relative_path == destination_relative_path {
        return Err("SourceGraphRewrite blocat: sursa și destinația sunt identice.".to_string());
    }

    let graph = build_source_graph(project_root).map_err(|error| {
        format!("SourceGraphRewrite blocat: Source Graph nu a putut fi construit: {error}.")
    })?;
    plan_template_reference_workspace_mutation_with_graph(
        store,
        &graph,
        operation,
        source_relative_path,
        destination_relative_path,
        false,
    )
}

/// Plans a reference rewrite against the current ProjectWorkspace projection.
///
/// Unlike the disk-only planner above, this variant deliberately reads the
/// current text of draft files. The supplied graph and the FileBufferStore
/// therefore describe the same in-memory revision and can be staged as one
/// atomic ProjectWorkspace mutation.
pub fn plan_template_reference_workspace_mutation_from_graph(
    project_root: &Path,
    store: &FileBufferStore,
    graph: &SourceGraph,
    operation: SourceGraphRewriteOperation,
    source_relative_path: &str,
    destination_relative_path: &str,
) -> Result<SourceGraphReferenceRewritePlan, String> {
    validate_store_identity(project_root, store)?;
    let source_relative_path = normalize_project_relative_path(source_relative_path)?;
    let destination_relative_path = normalize_project_relative_path(destination_relative_path)?;
    if source_relative_path == destination_relative_path {
        return Err("SourceGraphRewrite blocat: sursa și destinația sunt identice.".to_string());
    }

    plan_template_reference_workspace_mutation_with_graph(
        store,
        graph,
        operation,
        source_relative_path,
        destination_relative_path,
        true,
    )
}

fn plan_template_reference_workspace_mutation_with_graph(
    store: &FileBufferStore,
    graph: &SourceGraph,
    operation: SourceGraphRewriteOperation,
    source_relative_path: String,
    destination_relative_path: String,
    allow_current_drafts: bool,
) -> Result<SourceGraphReferenceRewritePlan, String> {
    let projected_store = allow_current_drafts.then(|| {
        let mut projected = store.clone();
        for entry in projected.files.values_mut() {
            if entry.draft.is_some() {
                let current_text = entry.current_text().to_string();
                entry.baseline_text = current_text;
                entry.draft = None;
            }
        }
        projected
    });
    let store = projected_store.as_ref().unwrap_or(store);

    let mut diagnostics = source_graph_diagnostics(&graph);
    if let Some(blocker) = first_blocker(&diagnostics) {
        return Err(blocker.message.clone());
    }

    let rewrite_targets =
        rewrite_targets_for_entry(&graph, &source_relative_path, &destination_relative_path)?;
    if rewrite_targets.is_empty() {
        diagnostics.push(SourceGraphRewriteDiagnostic::info(
            "no_template_targets",
            Some(source_relative_path.clone()),
            "SourceGraphRewrite nu a găsit template-uri locale afectate de această intrare.",
        ));
        return Ok(noop_plan(
            operation,
            source_relative_path,
            destination_relative_path,
            diagnostics,
        ));
    }

    let node_by_id = graph
        .nodes
        .iter()
        .map(|node| (node.id.clone(), node))
        .collect::<HashMap<_, _>>();
    let mut replacements_by_file: BTreeMap<String, Vec<TextReplacement>> = BTreeMap::new();

    for relation in &graph.relations {
        let Some(target) = rewrite_targets.get(&relation.to) else {
            continue;
        };
        if !is_rewriteable_relation_kind(&relation.kind) {
            continue;
        }
        let Some(from_node) = node_by_id.get(&relation.from).copied() else {
            diagnostics.push(SourceGraphRewriteDiagnostic::blocked(
                "missing_relation_source_node",
                None,
                format!(
                    "SourceGraphRewrite a blocat relația {}: nodul sursă lipsește din graph.",
                    relation_kind_label(&relation.kind)
                ),
            ));
            continue;
        };

        let replacements = match relation.kind {
            SourceRelationKind::PageTemplate => plan_frontmatter_template_replacements(
                &graph,
                store,
                from_node,
                target,
                relation_kind_label(&relation.kind),
                "template",
                "implicit_page_template",
                &mut diagnostics,
            )?,
            SourceRelationKind::SectionPageTemplate => plan_frontmatter_template_replacements(
                &graph,
                store,
                from_node,
                target,
                relation_kind_label(&relation.kind),
                "page_template",
                "missing_section_page_template",
                &mut diagnostics,
            )?,
            SourceRelationKind::Extends
            | SourceRelationKind::Includes
            | SourceRelationKind::Imports => plan_tera_template_replacements(
                store,
                from_node,
                &relation.kind,
                target,
                &mut diagnostics,
            )?,
            SourceRelationKind::GetsPage
            | SourceRelationKind::GetsSection
            | SourceRelationKind::InternalContentLink => plan_zola_content_function_replacements(
                store,
                from_node,
                &relation.kind,
                target,
                &mut diagnostics,
            )?,
            SourceRelationKind::AssetUrl
            | SourceRelationKind::AssetHash
            | SourceRelationKind::DataLoad
            | SourceRelationKind::ImageMetadata
            | SourceRelationKind::ImageResize => plan_zola_asset_function_replacements(
                store,
                from_node,
                &relation.kind,
                target,
                &mut diagnostics,
            )?,
            SourceRelationKind::DataFileLoad => plan_zola_data_file_function_replacements(
                store,
                from_node,
                &relation.kind,
                target,
                &mut diagnostics,
            )?,
            SourceRelationKind::ContentDataLoad => plan_zola_content_load_function_replacements(
                store,
                from_node,
                &relation.kind,
                target,
                &mut diagnostics,
            )?,
            SourceRelationKind::DefinesBlock
            | SourceRelationKind::OverridesBlock
            | SourceRelationKind::UsesStyle
            | SourceRelationKind::UsesScript => Vec::new(),
        };

        for replacement in replacements {
            replacements_by_file
                .entry(replacement.rewrite.relative_path.clone())
                .or_default()
                .push(replacement);
        }
    }

    if let Some(blocker) = first_blocker(&diagnostics) {
        return Err(blocker.message.clone());
    }

    let mut rewritten_references = Vec::new();
    let mut changes = Vec::new();
    for (relative_path, mut replacements) in replacements_by_file {
        let entry = store.files.get(&relative_path).ok_or_else(|| {
            format!(
                "SourceGraphRewrite blocat pentru {relative_path}: FileBufferStore nu are baseline urmărit."
            )
        })?;
        if entry.draft.is_some() && !allow_current_drafts {
            return Err(format!(
                "SourceGraphRewrite blocat pentru {relative_path}: fișierul are draft nesalvat în FileBufferStore."
            ));
        }
        replacements.sort_by(|left, right| {
            right
                .range_start
                .cmp(&left.range_start)
                .then_with(|| right.range_end.cmp(&left.range_end))
        });
        validate_replacements_do_not_overlap(&relative_path, &replacements)?;

        let mut next_text = if allow_current_drafts {
            entry.current_text().to_string()
        } else {
            entry.baseline_text.clone()
        };
        for replacement in &replacements {
            next_text.replace_range(
                replacement.range_start..replacement.range_end,
                &replacement.new_text,
            );
        }

        for replacement in replacements.into_iter().rev() {
            rewritten_references.push(replacement.rewrite);
        }
        changes.push(WorkspaceTextChange {
            relative_path,
            new_text: next_text,
        });
    }

    if changes.is_empty() {
        diagnostics.push(SourceGraphRewriteDiagnostic::info(
            "no_static_references_to_rewrite",
            Some(source_relative_path.clone()),
            "SourceGraphRewrite nu a găsit referințe statice care trebuie rescrise.",
        ));
        return Ok(noop_plan(
            operation,
            source_relative_path,
            destination_relative_path,
            diagnostics,
        ));
    }

    let touched_files = changes
        .iter()
        .map(|change| change.relative_path.clone())
        .collect::<Vec<_>>();

    Ok(SourceGraphReferenceRewritePlan {
        schema_version: SOURCE_GRAPH_REWRITE_SCHEMA_VERSION,
        operation,
        status: SourceGraphRewriteStatus::Planned,
        source_relative_path,
        destination_relative_path,
        rewritten_references,
        touched_files,
        diagnostics,
        workspace_mutation: Some(WorkspaceTextMutationInput {
            label: "Rewrite Source Graph references".to_string(),
            target: SOURCE_GRAPH_REWRITE_WORKSPACE_TARGET.to_string(),
            changes,
        }),
    })
}

fn source_graph_diagnostics(graph: &SourceGraph) -> Vec<SourceGraphRewriteDiagnostic> {
    graph
        .diagnostics
        .iter()
        .map(|diagnostic| match diagnostic.severity {
            SourceDiagnosticSeverity::Warning => SourceGraphRewriteDiagnostic::warning(
                "source_graph_warning",
                diagnostic.file.clone(),
                diagnostic.message.clone(),
            ),
            SourceDiagnosticSeverity::Error => SourceGraphRewriteDiagnostic::blocked(
                "source_graph_error",
                diagnostic.file.clone(),
                format!(
                    "SourceGraphRewrite blocat: Source Graph are diagnostic de eroare: {}",
                    diagnostic.message
                ),
            ),
        })
        .collect()
}

fn first_blocker(
    diagnostics: &[SourceGraphRewriteDiagnostic],
) -> Option<&SourceGraphRewriteDiagnostic> {
    diagnostics
        .iter()
        .find(|diagnostic| diagnostic.severity == SourceGraphRewriteSeverity::Blocked)
}

fn noop_plan(
    operation: SourceGraphRewriteOperation,
    source_relative_path: String,
    destination_relative_path: String,
    diagnostics: Vec<SourceGraphRewriteDiagnostic>,
) -> SourceGraphReferenceRewritePlan {
    SourceGraphReferenceRewritePlan {
        schema_version: SOURCE_GRAPH_REWRITE_SCHEMA_VERSION,
        operation,
        status: SourceGraphRewriteStatus::NoOp,
        source_relative_path,
        destination_relative_path,
        rewritten_references: Vec::new(),
        touched_files: Vec::new(),
        diagnostics,
        workspace_mutation: None,
    }
}

fn validate_store_identity(project_root: &Path, store: &FileBufferStore) -> Result<(), String> {
    let expected = project_root.to_string_lossy();
    if store.project_root != expected {
        return Err(format!(
            "SourceGraphRewrite blocat: FileBufferStore aparține proiectului {}, dar proiectul curent este {}.",
            store.project_root, expected
        ));
    }
    Ok(())
}

fn is_rewriteable_relation_kind(kind: &SourceRelationKind) -> bool {
    matches!(
        kind,
        SourceRelationKind::PageTemplate
            | SourceRelationKind::SectionPageTemplate
            | SourceRelationKind::GetsPage
            | SourceRelationKind::GetsSection
            | SourceRelationKind::InternalContentLink
            | SourceRelationKind::AssetUrl
            | SourceRelationKind::AssetHash
            | SourceRelationKind::DataLoad
            | SourceRelationKind::DataFileLoad
            | SourceRelationKind::ContentDataLoad
            | SourceRelationKind::ImageMetadata
            | SourceRelationKind::ImageResize
            | SourceRelationKind::Extends
            | SourceRelationKind::Includes
            | SourceRelationKind::Imports
    )
}

fn validate_replacements_do_not_overlap(
    relative_path: &str,
    replacements: &[TextReplacement],
) -> Result<(), String> {
    let mut seen_ranges = BTreeSet::new();
    let mut previous_start = None;
    for replacement in replacements {
        if replacement.range_start > replacement.range_end {
            return Err(format!(
                "SourceGraphRewrite blocat pentru {relative_path}: range invalid."
            ));
        }
        if !seen_ranges.insert((replacement.range_start, replacement.range_end)) {
            return Err(format!(
                "SourceGraphRewrite blocat pentru {relative_path}: range duplicat."
            ));
        }
        if let Some(previous_start) = previous_start {
            if replacement.range_end > previous_start {
                return Err(format!(
                    "SourceGraphRewrite blocat pentru {relative_path}: range-uri suprapuse."
                ));
            }
        }
        previous_start = Some(replacement.range_start);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::kernel::file_buffer_store::{
        hash_text, FileBufferBaseline, FileBufferEntry, FileBufferStore, FileBufferStoreLimits,
        TextBufferLanguage, TextBufferRole,
    };
    use crate::source_graph::build_source_graph_with_projection;

    use super::{
        plan_template_reference_workspace_mutation,
        plan_template_reference_workspace_mutation_from_graph, SourceGraphRewriteOperation,
        SourceGraphRewriteStatus,
    };

    #[test]
    fn planner_rewrites_tera_references_for_renamed_partial() {
        let root = zola_project("rewrite-tera-rename");
        write_text(
            &root,
            "templates/base.html",
            r#"{% include "partials/header.html" %}
<main>{% block content %}{% endblock content %}</main>
"#,
        );
        write_text(
            &root,
            "templates/partials/header.html",
            "<header>Header</header>",
        );
        let store = store_with_files(
            &root,
            &[
                (
                    "templates/base.html",
                    r#"{% include "partials/header.html" %}
<main>{% block content %}{% endblock content %}</main>
"#,
                ),
                ("templates/partials/header.html", "<header>Header</header>"),
            ],
        );

        let plan = plan_template_reference_workspace_mutation(
            &root,
            &store,
            SourceGraphRewriteOperation::Rename,
            "templates/partials/header.html",
            "templates/partials/site-header.html",
        )
        .unwrap();

        assert_eq!(plan.status, SourceGraphRewriteStatus::Planned);
        let workspace_mutation = plan.workspace_mutation.unwrap();
        assert_eq!(workspace_mutation.changes.len(), 1);
        assert_eq!(
            workspace_mutation.changes[0].relative_path,
            "templates/base.html"
        );
        assert!(workspace_mutation.changes[0]
            .new_text
            .contains(r#"{% include "partials/site-header.html" %}"#));
    }

    #[test]
    fn planner_rewrites_frontmatter_template_reference() {
        let root = zola_project("rewrite-frontmatter");
        write_text(
            &root,
            "content/despre.md",
            "+++\ntitle = \"Despre\"\ntemplate = \"custom/despre.html\"\n+++\n",
        );
        write_text(&root, "templates/custom/despre.html", "<h1>Despre</h1>");
        let store = store_with_files(
            &root,
            &[
                (
                    "content/despre.md",
                    "+++\ntitle = \"Despre\"\ntemplate = \"custom/despre.html\"\n+++\n",
                ),
                ("templates/custom/despre.html", "<h1>Despre</h1>"),
            ],
        );

        let plan = plan_template_reference_workspace_mutation(
            &root,
            &store,
            SourceGraphRewriteOperation::Rename,
            "templates/custom/despre.html",
            "templates/custom/prezentare.html",
        )
        .unwrap();

        let workspace_mutation = plan.workspace_mutation.unwrap();
        assert_eq!(workspace_mutation.changes.len(), 1);
        assert!(workspace_mutation.changes[0]
            .new_text
            .contains("template = \"custom/prezentare.html\""));
    }

    #[test]
    fn planner_rewrites_section_page_template_reference() {
        let root = zola_project("rewrite-section-page-template");
        write_text(
            &root,
            "content/blog/_index.md",
            "+++\ntitle = \"Blog\"\npage_template = \"blog/card.html\"\n+++\n",
        );
        write_text(
            &root,
            "templates/blog/card.html",
            "<article>{{ page.title }}</article>",
        );
        let store = store_with_files(
            &root,
            &[
                (
                    "content/blog/_index.md",
                    "+++\ntitle = \"Blog\"\npage_template = \"blog/card.html\"\n+++\n",
                ),
                (
                    "templates/blog/card.html",
                    "<article>{{ page.title }}</article>",
                ),
            ],
        );

        let plan = plan_template_reference_workspace_mutation(
            &root,
            &store,
            SourceGraphRewriteOperation::Rename,
            "templates/blog/card.html",
            "templates/blog/item.html",
        )
        .unwrap();

        let workspace_mutation = plan.workspace_mutation.unwrap();
        assert_eq!(workspace_mutation.changes.len(), 1);
        assert!(workspace_mutation.changes[0]
            .new_text
            .contains("page_template = \"blog/item.html\""));
        assert!(plan.rewritten_references.iter().any(|rewrite| {
            rewrite.relative_path == "content/blog/_index.md"
                && rewrite.target_relative_path == "templates/blog/card.html"
                && rewrite.relation_kind == "section_page_template"
                && rewrite.old_reference == "blog/card.html"
                && rewrite.new_reference == "blog/item.html"
        }));
    }

    #[test]
    fn planner_rewrites_get_page_reference() {
        let root = zola_project("rewrite-get-page");
        write_text(
            &root,
            "content/blog/post.md",
            "+++\ntitle = \"Post\"\n+++\n",
        );
        write_text(
            &root,
            "templates/index.html",
            r#"{% set featured = get_page(path="blog/post.md") %}"#,
        );
        let store = store_with_files(
            &root,
            &[
                ("content/blog/post.md", "+++\ntitle = \"Post\"\n+++\n"),
                (
                    "templates/index.html",
                    r#"{% set featured = get_page(path="blog/post.md") %}"#,
                ),
            ],
        );

        let plan = plan_template_reference_workspace_mutation(
            &root,
            &store,
            SourceGraphRewriteOperation::Rename,
            "content/blog/post.md",
            "content/blog/articol.md",
        )
        .unwrap();

        let workspace_mutation = plan.workspace_mutation.unwrap();
        assert_eq!(workspace_mutation.changes.len(), 1);
        assert!(workspace_mutation.changes[0]
            .new_text
            .contains(r#"get_page(path="blog/articol.md")"#));
        assert!(plan.rewritten_references.iter().any(|rewrite| {
            rewrite.relative_path == "templates/index.html"
                && rewrite.target_relative_path == "content/blog/post.md"
                && rewrite.relation_kind == "gets_page"
                && rewrite.old_reference == "blog/post.md"
                && rewrite.new_reference == "blog/articol.md"
        }));
    }

    #[test]
    fn planner_rewrites_get_section_reference_for_renamed_section_directory() {
        let root = zola_project("rewrite-get-section");
        write_text(
            &root,
            "content/blog/_index.md",
            "+++\ntitle = \"Blog\"\n+++\n",
        );
        write_text(
            &root,
            "templates/index.html",
            r#"{% set blog = get_section(path="blog/_index.md", metadata_only=true) %}"#,
        );
        let store = store_with_files(
            &root,
            &[
                ("content/blog/_index.md", "+++\ntitle = \"Blog\"\n+++\n"),
                (
                    "templates/index.html",
                    r#"{% set blog = get_section(path="blog/_index.md", metadata_only=true) %}"#,
                ),
            ],
        );

        let plan = plan_template_reference_workspace_mutation(
            &root,
            &store,
            SourceGraphRewriteOperation::Rename,
            "content/blog",
            "content/jurnal",
        )
        .unwrap();

        let workspace_mutation = plan.workspace_mutation.unwrap();
        assert_eq!(workspace_mutation.changes.len(), 1);
        assert!(workspace_mutation.changes[0]
            .new_text
            .contains(r#"get_section(path="jurnal/_index.md", metadata_only=true)"#));
        assert!(plan.rewritten_references.iter().any(|rewrite| {
            rewrite.relative_path == "templates/index.html"
                && rewrite.target_relative_path == "content/blog/_index.md"
                && rewrite.relation_kind == "gets_section"
                && rewrite.old_reference == "blog/_index.md"
                && rewrite.new_reference == "jurnal/_index.md"
        }));
    }

    #[test]
    fn planner_rewrites_get_url_internal_content_reference() {
        let root = zola_project("rewrite-get-url-internal");
        write_text(
            &root,
            "content/blog/post.md",
            "+++\ntitle = \"Post\"\n+++\n",
        );
        write_text(
            &root,
            "templates/index.html",
            r#"<a href="{{ get_url(path="@/blog/post.md") }}">Post</a>"#,
        );
        let store = store_with_files(
            &root,
            &[
                ("content/blog/post.md", "+++\ntitle = \"Post\"\n+++\n"),
                (
                    "templates/index.html",
                    r#"<a href="{{ get_url(path="@/blog/post.md") }}">Post</a>"#,
                ),
            ],
        );

        let plan = plan_template_reference_workspace_mutation(
            &root,
            &store,
            SourceGraphRewriteOperation::Rename,
            "content/blog/post.md",
            "content/blog/articol.md",
        )
        .unwrap();

        let workspace_mutation = plan.workspace_mutation.unwrap();
        assert_eq!(workspace_mutation.changes.len(), 1);
        assert!(workspace_mutation.changes[0]
            .new_text
            .contains(r#"get_url(path="@/blog/articol.md")"#));
        assert!(plan.rewritten_references.iter().any(|rewrite| {
            rewrite.relative_path == "templates/index.html"
                && rewrite.target_relative_path == "content/blog/post.md"
                && rewrite.relation_kind == "internal_content_link"
                && rewrite.old_reference == "@/blog/post.md"
                && rewrite.new_reference == "@/blog/articol.md"
        }));
    }

    #[test]
    fn planner_rewrites_load_data_content_reference() {
        let root = zola_project("rewrite-load-data-content");
        write_text(
            &root,
            "content/blog/post.md",
            "+++\ntitle = \"Post\"\n+++\n",
        );
        write_text(
            &root,
            "templates/index.html",
            r#"{% set post = load_data(path="@/blog/post.md") %}
{% set post_copy = load_data(path="content/blog/post.md") %}"#,
        );
        let store = store_with_files(
            &root,
            &[
                ("content/blog/post.md", "+++\ntitle = \"Post\"\n+++\n"),
                (
                    "templates/index.html",
                    r#"{% set post = load_data(path="@/blog/post.md") %}
{% set post_copy = load_data(path="content/blog/post.md") %}"#,
                ),
            ],
        );

        let plan = plan_template_reference_workspace_mutation(
            &root,
            &store,
            SourceGraphRewriteOperation::Rename,
            "content/blog/post.md",
            "content/blog/articol.md",
        )
        .unwrap();

        let workspace_mutation = plan.workspace_mutation.unwrap();
        assert_eq!(workspace_mutation.changes.len(), 1);
        assert!(workspace_mutation.changes[0]
            .new_text
            .contains(r#"load_data(path="@/blog/articol.md")"#));
        assert!(workspace_mutation.changes[0]
            .new_text
            .contains(r#"load_data(path="content/blog/articol.md")"#));
        assert!(plan.rewritten_references.iter().any(|rewrite| {
            rewrite.relative_path == "templates/index.html"
                && rewrite.target_relative_path == "content/blog/post.md"
                && rewrite.relation_kind == "content_data_load"
                && rewrite.old_reference == "@/blog/post.md"
                && rewrite.new_reference == "@/blog/articol.md"
        }));
        assert!(plan.rewritten_references.iter().any(|rewrite| {
            rewrite.relative_path == "templates/index.html"
                && rewrite.target_relative_path == "content/blog/post.md"
                && rewrite.relation_kind == "content_data_load"
                && rewrite.old_reference == "content/blog/post.md"
                && rewrite.new_reference == "content/blog/articol.md"
        }));
    }

    #[test]
    fn planner_rewrites_static_asset_url_and_hash_references() {
        let root = zola_project("rewrite-static-asset");
        write_text(&root, "static/js/app.js", "console.log('ok');");
        write_text(
            &root,
            "templates/index.html",
            r#"<script src="{{ get_url(path="js/app.js") }}" integrity="{{ get_hash(path="static/js/app.js") }}"></script>"#,
        );
        let store = store_with_files(
            &root,
            &[(
                "templates/index.html",
                r#"<script src="{{ get_url(path="js/app.js") }}" integrity="{{ get_hash(path="static/js/app.js") }}"></script>"#,
            )],
        );

        let plan = plan_template_reference_workspace_mutation(
            &root,
            &store,
            SourceGraphRewriteOperation::Rename,
            "static/js/app.js",
            "static/js/site.js",
        )
        .unwrap();

        let workspace_mutation = plan.workspace_mutation.unwrap();
        assert_eq!(workspace_mutation.changes.len(), 1);
        assert!(workspace_mutation.changes[0]
            .new_text
            .contains(r#"get_url(path="js/site.js")"#));
        assert!(workspace_mutation.changes[0]
            .new_text
            .contains(r#"get_hash(path="static/js/site.js")"#));
        assert!(plan.rewritten_references.iter().any(|rewrite| {
            rewrite.relative_path == "templates/index.html"
                && rewrite.target_relative_path == "static/js/app.js"
                && rewrite.relation_kind == "asset_url"
                && rewrite.old_reference == "js/app.js"
                && rewrite.new_reference == "js/site.js"
        }));
        assert!(plan.rewritten_references.iter().any(|rewrite| {
            rewrite.relative_path == "templates/index.html"
                && rewrite.target_relative_path == "static/js/app.js"
                && rewrite.relation_kind == "asset_hash"
                && rewrite.old_reference == "static/js/app.js"
                && rewrite.new_reference == "static/js/site.js"
        }));
    }

    #[test]
    fn planner_rewrites_load_data_static_asset_reference() {
        let root = zola_project("rewrite-load-data");
        write_text(&root, "static/data/catalog.json", "{}");
        write_text(
            &root,
            "templates/index.html",
            r#"{% set catalog = load_data(path="static/data/catalog.json") %}"#,
        );
        let store = store_with_files(
            &root,
            &[(
                "templates/index.html",
                r#"{% set catalog = load_data(path="static/data/catalog.json") %}"#,
            )],
        );

        let plan = plan_template_reference_workspace_mutation(
            &root,
            &store,
            SourceGraphRewriteOperation::Rename,
            "static/data/catalog.json",
            "static/data/products.json",
        )
        .unwrap();

        let workspace_mutation = plan.workspace_mutation.unwrap();
        assert_eq!(workspace_mutation.changes.len(), 1);
        assert!(workspace_mutation.changes[0]
            .new_text
            .contains(r#"load_data(path="static/data/products.json")"#));
        assert!(plan.rewritten_references.iter().any(|rewrite| {
            rewrite.relative_path == "templates/index.html"
                && rewrite.target_relative_path == "static/data/catalog.json"
                && rewrite.relation_kind == "data_load"
                && rewrite.old_reference == "static/data/catalog.json"
                && rewrite.new_reference == "static/data/products.json"
        }));
    }

    #[test]
    fn planner_rewrites_load_data_zola_data_file_reference() {
        let root = zola_project("rewrite-load-data-file");
        write_text(&root, "date/meniu.toml", "[[item]]\nlabel = \"Acasă\"\n");
        write_text(
            &root,
            "templates/index.html",
            r#"{% set meniu = load_data(path="date/meniu.toml") %}"#,
        );
        let store = store_with_files(
            &root,
            &[(
                "templates/index.html",
                r#"{% set meniu = load_data(path="date/meniu.toml") %}"#,
            )],
        );

        let plan = plan_template_reference_workspace_mutation(
            &root,
            &store,
            SourceGraphRewriteOperation::Rename,
            "date/meniu.toml",
            "date/navigatie.toml",
        )
        .unwrap();

        let workspace_mutation = plan.workspace_mutation.unwrap();
        assert_eq!(workspace_mutation.changes.len(), 1);
        assert!(workspace_mutation.changes[0]
            .new_text
            .contains(r#"load_data(path="date/navigatie.toml")"#));
        assert!(plan.rewritten_references.iter().any(|rewrite| {
            rewrite.relative_path == "templates/index.html"
                && rewrite.target_relative_path == "date/meniu.toml"
                && rewrite.relation_kind == "data_file_load"
                && rewrite.old_reference == "date/meniu.toml"
                && rewrite.new_reference == "date/navigatie.toml"
        }));
    }

    #[test]
    fn planner_rewrites_get_image_metadata_static_asset_reference() {
        let root = zola_project("rewrite-image-metadata");
        write_text(&root, "static/img/hero.png", "png");
        write_text(
            &root,
            "templates/index.html",
            r#"{% set meta = get_image_metadata(path="static/img/hero.png") %}"#,
        );
        let store = store_with_files(
            &root,
            &[(
                "templates/index.html",
                r#"{% set meta = get_image_metadata(path="static/img/hero.png") %}"#,
            )],
        );

        let plan = plan_template_reference_workspace_mutation(
            &root,
            &store,
            SourceGraphRewriteOperation::Rename,
            "static/img/hero.png",
            "static/img/cover.png",
        )
        .unwrap();

        let workspace_mutation = plan.workspace_mutation.unwrap();
        assert_eq!(workspace_mutation.changes.len(), 1);
        assert!(workspace_mutation.changes[0]
            .new_text
            .contains(r#"get_image_metadata(path="static/img/cover.png")"#));
        assert!(plan.rewritten_references.iter().any(|rewrite| {
            rewrite.relative_path == "templates/index.html"
                && rewrite.target_relative_path == "static/img/hero.png"
                && rewrite.relation_kind == "image_metadata"
                && rewrite.old_reference == "static/img/hero.png"
                && rewrite.new_reference == "static/img/cover.png"
        }));
    }

    #[test]
    fn planner_rewrites_resize_image_static_asset_reference() {
        let root = zola_project("rewrite-resize-image");
        write_text(&root, "static/img/hero.png", "png");
        write_text(
            &root,
            "templates/index.html",
            r#"{% set image = resize_image(path="static/img/hero.png", width=640, op="fit_width") %}"#,
        );
        let store = store_with_files(
            &root,
            &[(
                "templates/index.html",
                r#"{% set image = resize_image(path="static/img/hero.png", width=640, op="fit_width") %}"#,
            )],
        );

        let plan = plan_template_reference_workspace_mutation(
            &root,
            &store,
            SourceGraphRewriteOperation::Rename,
            "static/img/hero.png",
            "static/img/cover.png",
        )
        .unwrap();

        let workspace_mutation = plan.workspace_mutation.unwrap();
        assert_eq!(workspace_mutation.changes.len(), 1);
        assert!(workspace_mutation.changes[0]
            .new_text
            .contains(r#"resize_image(path="static/img/cover.png""#));
        assert!(plan.rewritten_references.iter().any(|rewrite| {
            rewrite.relative_path == "templates/index.html"
                && rewrite.target_relative_path == "static/img/hero.png"
                && rewrite.relation_kind == "image_resize"
                && rewrite.old_reference == "static/img/hero.png"
                && rewrite.new_reference == "static/img/cover.png"
        }));
    }

    #[test]
    fn planner_blocks_implicit_page_template_reference() {
        let root = zola_project("rewrite-implicit");
        write_text(&root, "content/despre.md", "+++\ntitle = \"Despre\"\n+++\n");
        write_text(&root, "templates/page.html", "<h1>{{ page.title }}</h1>");
        let store = store_with_files(
            &root,
            &[
                ("content/despre.md", "+++\ntitle = \"Despre\"\n+++\n"),
                ("templates/page.html", "<h1>{{ page.title }}</h1>"),
            ],
        );

        let error = plan_template_reference_workspace_mutation(
            &root,
            &store,
            SourceGraphRewriteOperation::Rename,
            "templates/page.html",
            "templates/layouts/page.html",
        )
        .unwrap_err();

        assert!(error.contains("template-ul implicit"));
    }

    #[test]
    fn planner_blocks_dirty_rewrite_target_file() {
        let root = zola_project("rewrite-dirty");
        write_text(
            &root,
            "templates/base.html",
            r#"{% include "partials/header.html" %}"#,
        );
        write_text(
            &root,
            "templates/partials/header.html",
            "<header>Header</header>",
        );
        let mut store = store_with_files(
            &root,
            &[
                (
                    "templates/base.html",
                    r#"{% include "partials/header.html" %}"#,
                ),
                ("templates/partials/header.html", "<header>Header</header>"),
            ],
        );
        store
            .set_draft(
                "templates/base.html",
                r#"{% include "partials/header.html" %}<p>draft</p>"#.to_string(),
                1,
            )
            .unwrap();

        let error = plan_template_reference_workspace_mutation(
            &root,
            &store,
            SourceGraphRewriteOperation::Rename,
            "templates/partials/header.html",
            "templates/partials/site-header.html",
        )
        .unwrap_err();

        assert!(error.contains("draft nesalvat"));
    }

    #[test]
    fn projected_planner_rewrites_the_current_draft_instead_of_the_disk_baseline() {
        let root = zola_project("rewrite-projected-draft");
        write_text(
            &root,
            "templates/base.html",
            r#"{% include "partials/header.html" %}"#,
        );
        write_text(
            &root,
            "templates/partials/header.html",
            "<header>Header</header>",
        );
        let mut store = store_with_files(
            &root,
            &[
                (
                    "templates/base.html",
                    r#"{% include "partials/header.html" %}"#,
                ),
                ("templates/partials/header.html", "<header>Header</header>"),
            ],
        );
        let draft = r#"{% include "partials/header.html" %}<p>draft păstrat</p>"#;
        store
            .set_draft("templates/base.html", draft.to_string(), 2)
            .unwrap();
        let graph = build_source_graph_with_projection(
            &root,
            &HashMap::from([("templates/base.html".to_string(), draft.to_string())]),
            &HashSet::new(),
        )
        .unwrap();

        let plan = plan_template_reference_workspace_mutation_from_graph(
            &root,
            &store,
            &graph,
            SourceGraphRewriteOperation::Rename,
            "templates/partials/header.html",
            "templates/partials/site-header.html",
        )
        .unwrap();

        let text = &plan.workspace_mutation.unwrap().changes[0].new_text;
        assert!(text.contains(r#"{% include "partials/site-header.html" %}"#));
        assert!(text.contains("<p>draft păstrat</p>"));
    }

    fn zola_project(label: &str) -> PathBuf {
        let root = unique_test_dir(label);
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates/custom")).unwrap();
        fs::create_dir_all(root.join("templates/partials")).unwrap();
        fs::write(
            root.join("zola.toml"),
            "base_url = \"https://example.test\"\n",
        )
        .unwrap();
        root
    }

    fn store_with_files(root: &Path, files: &[(&str, &str)]) -> FileBufferStore {
        let mut store = FileBufferStore::new(
            "session-1",
            root.to_string_lossy(),
            1,
            FileBufferStoreLimits {
                max_files: 64,
                max_file_bytes: 1024 * 1024,
                max_total_bytes: 4 * 1024 * 1024,
            },
        );
        for (relative_path, text) in files {
            store.insert_loaded_file(FileBufferEntry {
                relative_path: (*relative_path).to_string(),
                absolute_path: root.join(relative_path).to_string_lossy().to_string(),
                language: language_for_path(relative_path),
                role: role_for_path(relative_path),
                baseline: baseline(text),
                baseline_text: (*text).to_string(),
                draft: None,
                revision: 1,
            });
        }
        store
    }

    fn baseline(text: &str) -> FileBufferBaseline {
        FileBufferBaseline {
            hash: hash_text(text),
            modified_ms: 1,
            size: text.len() as u64,
            readonly: false,
        }
    }

    fn language_for_path(relative_path: &str) -> TextBufferLanguage {
        if relative_path.ends_with(".md") {
            TextBufferLanguage::Markdown
        } else if relative_path.ends_with(".toml") {
            TextBufferLanguage::Toml
        } else if relative_path.ends_with(".json") {
            TextBufferLanguage::Json
        } else if relative_path.ends_with(".yaml") || relative_path.ends_with(".yml") {
            TextBufferLanguage::Yaml
        } else {
            TextBufferLanguage::Html
        }
    }

    fn role_for_path(relative_path: &str) -> TextBufferRole {
        if relative_path.contains("/content/") {
            TextBufferRole::Page
        } else if relative_path.contains("/date/") {
            TextBufferRole::Data
        } else {
            TextBufferRole::Template
        }
    }

    fn write_text(root: &Path, relative_path: &str, text: &str) {
        let path = root.join(relative_path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn unique_test_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pana-studio-{label}-{nanos}"))
    }
}
