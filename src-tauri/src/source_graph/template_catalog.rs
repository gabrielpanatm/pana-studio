use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use serde::Serialize;

use crate::localization::LocalizedDiagnostic;

use super::model::{
    SourceGraph, SourceGraphPage, SourceGraphTemplate, SourceOrigin, SourcePageKind,
    SourceRelationKind,
};
use super::taxonomy_catalog::TaxonomyCatalogSnapshot;

pub const TEMPLATE_CATALOG_SCHEMA_VERSION: u32 = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateCatalogRole {
    Page,
    Layout,
    Partial,
    ListingItem,
    MacroLibrary,
    Shortcode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateSemanticCategory {
    Layout,
    Page,
    Archive,
    Element,
    ListingItem,
    Taxonomy,
    System,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateCatalogContext {
    Page,
    Section,
    System,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateAssignmentSource {
    Explicit,
    Inherited,
    Default,
    Convention,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateSemanticRole {
    Layout,
    Homepage,
    DefaultPage,
    SpecificPage,
    SectionArchive,
    SectionElement,
    ListingItem,
    TaxonomyList,
    TaxonomyTerm,
    NotFound,
    Custom,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateSemanticTargetKind {
    Resource,
    Site,
    Page,
    Section,
    Taxonomy,
    System,
    Custom,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateCatalogReferenceKind {
    Extends,
    Includes,
    Imports,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateCatalogTemplateUsage {
    pub file: String,
    pub name: String,
    pub kind: TemplateCatalogReferenceKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateCatalogPageUsage {
    pub file: String,
    pub title: String,
    pub url: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateCatalogAssignment {
    pub file: String,
    pub title: String,
    pub url: String,
    pub context: TemplateCatalogContext,
    pub source: TemplateAssignmentSource,
    pub declared_in: Option<String>,
    pub frontmatter_key: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateResource {
    pub id: String,
    pub file: String,
    pub name: String,
    pub origin: SourceOrigin,
    pub theme_name: Option<String>,
    pub roles: Vec<TemplateCatalogRole>,
    pub editable: bool,
    pub effective: bool,
    pub local_override_path: String,
    pub extends: Option<String>,
    pub includes: Vec<String>,
    pub imports: Vec<String>,
    pub blocks: Vec<String>,
    pub macros: Vec<String>,
    pub used_by_templates: Vec<TemplateCatalogTemplateUsage>,
    pub affected_pages: Vec<TemplateCatalogPageUsage>,
    pub can_delete: bool,
    pub delete_blocked_diagnostic: Option<LocalizedDiagnostic>,
    pub node_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateSemanticTarget {
    pub id: String,
    pub kind: TemplateSemanticTargetKind,
    pub label: Option<String>,
    pub label_diagnostic: Option<LocalizedDiagnostic>,
    pub file: Option<String>,
    pub url: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateAssignment {
    pub key: Option<String>,
    pub source: TemplateAssignmentSource,
    pub declared_in: Option<String>,
    pub resource_id: Option<String>,
    pub resource_name: String,
    pub fallback_name: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePreviewContext {
    pub kind: TemplateCatalogContext,
    pub page_file: Option<String>,
    pub title: Option<String>,
    pub title_diagnostic: Option<LocalizedDiagnostic>,
    pub url: String,
    pub exact: bool,
    pub available: bool,
    pub unavailable_diagnostic: Option<LocalizedDiagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateSemanticEntry {
    pub id: String,
    pub category: TemplateSemanticCategory,
    pub role: TemplateSemanticRole,
    pub label: Option<String>,
    pub label_diagnostic: Option<LocalizedDiagnostic>,
    pub target: TemplateSemanticTarget,
    pub assignment: TemplateAssignment,
    pub preview_context: Option<TemplatePreviewContext>,
    pub affected_pages: Vec<TemplateCatalogPageUsage>,
    #[serde(skip)]
    pub sort_label: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateCatalogSnapshot {
    pub schema_version: u32,
    pub active_theme: Option<String>,
    pub resources: Vec<TemplateResource>,
    pub semantic_entries: Vec<TemplateSemanticEntry>,
}

pub fn build_template_catalog(graph: &SourceGraph) -> TemplateCatalogSnapshot {
    build_template_catalog_with_taxonomies(graph, None)
}

pub fn build_template_catalog_with_taxonomies(
    graph: &SourceGraph,
    taxonomies: Option<&TaxonomyCatalogSnapshot>,
) -> TemplateCatalogSnapshot {
    let templates_by_node = graph
        .templates
        .iter()
        .map(|template| (template.node_id.as_str(), template))
        .collect::<HashMap<_, _>>();
    let pages_by_node = graph
        .pages
        .iter()
        .map(|page| (page.content_node_id.as_str(), page))
        .collect::<HashMap<_, _>>();
    let local_template_names = graph
        .templates
        .iter()
        .filter(|template| template.origin == SourceOrigin::Local)
        .map(|template| template.name.as_str())
        .collect::<HashSet<_>>();

    let mut resources = graph
        .templates
        .iter()
        .map(|template| {
            let used_by_templates = direct_template_usages(graph, template, &templates_by_node);
            let affected_pages =
                affected_pages(graph, template, &templates_by_node, &pages_by_node);
            let roles = template_roles(
                template,
                &used_by_templates,
                directly_serves_page(graph, template),
            );
            let editable = template.origin == SourceOrigin::Local;
            let effective = editable || !local_template_names.contains(template.name.as_str());

            TemplateResource {
                id: template.id.clone(),
                file: template.file.clone(),
                name: template.name.clone(),
                origin: template.origin.clone(),
                theme_name: template.theme_name.clone(),
                roles,
                editable,
                effective,
                local_override_path: format!("templates/{}", template.name),
                extends: template.extends.clone(),
                includes: template.includes.clone(),
                imports: template.imports.clone(),
                blocks: template.blocks.clone(),
                macros: template.macros.clone(),
                used_by_templates,
                affected_pages,
                can_delete: false,
                delete_blocked_diagnostic: None,
                node_id: template.node_id.clone(),
            }
        })
        .collect::<Vec<_>>();

    let assignment_projections = semantic_template_assignments(graph);
    let effective_resource_by_name = resources
        .iter()
        .filter(|entry| entry.effective)
        .map(|entry| (normalize_template_name(&entry.name), entry.node_id.clone()))
        .collect::<HashMap<_, _>>();

    for (template_name, assignment) in &assignment_projections {
        let Some(direct_node_id) = effective_resource_by_name.get(template_name) else {
            continue;
        };
        for impacted_node_id in template_dependency_closure(graph, direct_node_id) {
            let Some(resource) = resources
                .iter_mut()
                .find(|resource| resource.node_id == impacted_node_id)
            else {
                continue;
            };
            let usage = TemplateCatalogPageUsage {
                file: assignment.file.clone(),
                title: assignment.title.clone(),
                url: assignment.url.clone(),
            };
            if !resource.affected_pages.contains(&usage) {
                resource.affected_pages.push(usage);
            }
        }
    }

    for resource in &mut resources {
        resource.affected_pages.sort_by(|left, right| {
            left.url
                .cmp(&right.url)
                .then_with(|| left.file.cmp(&right.file))
        });
        let assignment_count = assignment_projections
            .iter()
            .filter(|(name, _)| {
                effective_resource_by_name
                    .get(name)
                    .is_some_and(|node_id| node_id == &resource.node_id)
            })
            .count();
        let incoming_count = resource.used_by_templates.len() + assignment_count;
        resource.delete_blocked_diagnostic = if !resource.editable {
            Some(LocalizedDiagnostic::new("templates-delete-theme-readonly"))
        } else if incoming_count > 0 {
            Some(
                LocalizedDiagnostic::new("templates-delete-referenced")
                    .with_argument("count", incoming_count as u64),
            )
        } else {
            None
        };
        resource.can_delete = resource.delete_blocked_diagnostic.is_none();
    }

    resources.sort_by(|left, right| {
        right
            .effective
            .cmp(&left.effective)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.file.cmp(&right.file))
    });
    let mut semantic_entries =
        build_semantic_entries(graph, &resources, &assignment_projections, taxonomies);
    semantic_entries.sort_by(|left, right| {
        semantic_category_rank(left.category)
            .cmp(&semantic_category_rank(right.category))
            .then_with(|| left.sort_label.cmp(&right.sort_label))
            .then_with(|| left.id.cmp(&right.id))
    });

    TemplateCatalogSnapshot {
        schema_version: TEMPLATE_CATALOG_SCHEMA_VERSION,
        active_theme: graph.active_theme.clone(),
        resources,
        semantic_entries,
    }
}

fn build_semantic_entries(
    graph: &SourceGraph,
    resources: &[TemplateResource],
    assignments: &[(String, TemplateCatalogAssignment)],
    taxonomies: Option<&TaxonomyCatalogSnapshot>,
) -> Vec<TemplateSemanticEntry> {
    let effective_by_name = resources
        .iter()
        .filter(|resource| resource.effective)
        .map(|resource| (normalize_template_name(&resource.name), resource))
        .collect::<HashMap<_, _>>();
    let sections = graph
        .pages
        .iter()
        .filter(|page| {
            matches!(
                page.page_kind,
                SourcePageKind::Home | SourcePageKind::Section
            )
        })
        .collect::<Vec<_>>();
    let mut entries = Vec::new();
    let mut represented_resources = HashSet::new();

    for resource in resources.iter().filter(|resource| {
        resource.effective
            && resource.roles.contains(&TemplateCatalogRole::Layout)
            && !resource.roles.iter().any(|role| {
                matches!(
                    role,
                    TemplateCatalogRole::Partial
                        | TemplateCatalogRole::ListingItem
                        | TemplateCatalogRole::MacroLibrary
                        | TemplateCatalogRole::Shortcode
                )
            })
    }) {
        represented_resources.insert(resource.id.clone());
        entries.push(semantic_entry(
            format!("semantic:layout:{}", resource.id),
            TemplateSemanticCategory::Layout,
            TemplateSemanticRole::Layout,
            semantic_resource_label(&resource.name),
            None,
            TemplateSemanticTarget {
                id: resource.id.clone(),
                kind: TemplateSemanticTargetKind::Resource,
                label: Some(resource.name.clone()),
                label_diagnostic: None,
                file: Some(resource.file.clone()),
                url: None,
            },
            TemplateAssignment {
                key: Some("extends".to_string()),
                source: TemplateAssignmentSource::Convention,
                declared_in: Some(resource.file.clone()),
                resource_id: Some(resource.id.clone()),
                resource_name: resource.name.clone(),
                fallback_name: None,
            },
            preview_from_usage(
                resource.affected_pages.first(),
                TemplateCatalogContext::Page,
            ),
            resource.affected_pages.clone(),
        ));
    }

    if let Some(home) = graph
        .pages
        .iter()
        .find(|page| matches!(page.page_kind, SourcePageKind::Home))
    {
        let resource_name = home
            .frontmatter_template
            .as_deref()
            .or(home.resolved_template.as_deref())
            .map(normalize_template_name)
            .unwrap_or_else(|| "index.html".to_string());
        let source = if home.frontmatter_template.is_some() {
            TemplateAssignmentSource::Explicit
        } else {
            TemplateAssignmentSource::Default
        };
        let resource = effective_by_name.get(&resource_name).copied();
        if let Some(resource) = resource {
            represented_resources.insert(resource.id.clone());
        }
        entries.push(semantic_entry(
            "semantic:homepage".to_string(),
            TemplateSemanticCategory::Page,
            TemplateSemanticRole::Homepage,
            "Pagina principală".to_string(),
            Some(LocalizedDiagnostic::new("templates-semantic-homepage")),
            page_target(home, TemplateSemanticTargetKind::Site),
            assignment_projection(
                resource,
                &resource_name,
                "template",
                source,
                source
                    .eq(&TemplateAssignmentSource::Explicit)
                    .then(|| home.file.clone()),
                "index.html",
            ),
            Some(preview_for_page(home, TemplateCatalogContext::Section)),
            vec![page_usage(home)],
        ));
    }

    for section in sections
        .iter()
        .copied()
        .filter(|page| matches!(page.page_kind, SourcePageKind::Section))
    {
        let list_name = section
            .frontmatter_template
            .as_deref()
            .or(section.resolved_template.as_deref())
            .map(normalize_template_name)
            .unwrap_or_else(|| "section.html".to_string());
        let list_source = if section.frontmatter_template.is_some() {
            TemplateAssignmentSource::Explicit
        } else {
            TemplateAssignmentSource::Default
        };
        let list_resource = effective_by_name.get(&list_name).copied();
        if let Some(resource) = list_resource {
            represented_resources.insert(resource.id.clone());
        }
        entries.push(semantic_entry(
            format!("semantic:section-archive:{}", section.file),
            TemplateSemanticCategory::Archive,
            TemplateSemanticRole::SectionArchive,
            format!("Arhivă {}", section.title),
            Some(
                LocalizedDiagnostic::new("templates-semantic-section-archive")
                    .with_argument("title", section.title.clone()),
            ),
            page_target(section, TemplateSemanticTargetKind::Section),
            assignment_projection(
                list_resource,
                &list_name,
                "template",
                list_source,
                list_source
                    .eq(&TemplateAssignmentSource::Explicit)
                    .then(|| section.file.clone()),
                "section.html",
            ),
            Some(preview_for_page(section, TemplateCatalogContext::Section)),
            vec![page_usage(section)],
        ));

        let (item_name, item_source, item_declared_in) =
            effective_section_page_template(section, &sections);
        let item_resource = effective_by_name.get(&item_name).copied();
        if let Some(resource) = item_resource {
            represented_resources.insert(resource.id.clone());
        }
        let mut item_pages = graph
            .pages
            .iter()
            .filter(|page| matches!(page.page_kind, SourcePageKind::Page))
            .filter(|page| {
                owning_section(page, &sections).is_some_and(|owner| owner.file == section.file)
            })
            .filter(|page| page.frontmatter_template.is_none())
            .map(page_usage)
            .collect::<Vec<_>>();
        item_pages.sort_by(|left, right| {
            left.url
                .cmp(&right.url)
                .then_with(|| left.file.cmp(&right.file))
        });
        let item_preview = item_pages.first().map(|page| TemplatePreviewContext {
            kind: TemplateCatalogContext::Page,
            page_file: Some(page.file.clone()),
            title: Some(page.title.clone()),
            title_diagnostic: None,
            url: page.url.clone(),
            exact: true,
            available: true,
            unavailable_diagnostic: None,
        });
        entries.push(semantic_entry(
            format!("semantic:section-element:{}", section.file),
            TemplateSemanticCategory::Element,
            TemplateSemanticRole::SectionElement,
            format!("Articol {}", section.title),
            Some(
                LocalizedDiagnostic::new("templates-semantic-section-element")
                    .with_argument("title", section.title.clone()),
            ),
            page_target(section, TemplateSemanticTargetKind::Section),
            assignment_projection(
                item_resource,
                &item_name,
                "page_template",
                item_source,
                item_declared_in,
                "page.html",
            ),
            item_preview.or_else(|| {
                Some(TemplatePreviewContext {
                    kind: TemplateCatalogContext::Page,
                    page_file: None,
                    title: Some(section.title.clone()),
                    title_diagnostic: None,
                    url: section.url.clone(),
                    exact: true,
                    available: false,
                    unavailable_diagnostic: Some(LocalizedDiagnostic::new(
                        "templates-preview-section-empty",
                    )),
                })
            }),
            item_pages,
        ));
    }

    let mut default_pages_by_template = HashMap::<String, Vec<&SourceGraphPage>>::new();
    for page in graph
        .pages
        .iter()
        .filter(|page| matches!(page.page_kind, SourcePageKind::Page))
    {
        if page.frontmatter_template.is_some() {
            let name = page
                .frontmatter_template
                .as_deref()
                .map(normalize_template_name)
                .unwrap_or_else(|| "page.html".to_string());
            let resource = effective_by_name.get(&name).copied();
            if let Some(resource) = resource {
                represented_resources.insert(resource.id.clone());
            }
            entries.push(semantic_entry(
                format!("semantic:specific-page:{}", page.file),
                TemplateSemanticCategory::Page,
                TemplateSemanticRole::SpecificPage,
                page.title.clone(),
                None,
                page_target(page, TemplateSemanticTargetKind::Page),
                assignment_projection(
                    resource,
                    &name,
                    "template",
                    TemplateAssignmentSource::Explicit,
                    Some(page.file.clone()),
                    "page.html",
                ),
                Some(preview_for_page(page, TemplateCatalogContext::Page)),
                vec![page_usage(page)],
            ));
            continue;
        }
        if owning_section(page, &sections).is_none() {
            let name = assignments
                .iter()
                .find(|(_, assignment)| assignment.file == page.file)
                .map(|(name, _)| name.clone())
                .unwrap_or_else(|| "page.html".to_string());
            default_pages_by_template
                .entry(name)
                .or_default()
                .push(page);
        }
    }
    // The default-page role is a semantic Zola capability, not merely a
    // projection of current consumers. Keep the slot visible even in a new
    // project with no ordinary pages or no `page.html` resource yet.
    default_pages_by_template
        .entry("page.html".to_string())
        .or_default();
    for (resource_name, mut pages) in default_pages_by_template {
        pages.sort_by(|left, right| left.url.cmp(&right.url));
        let resource = effective_by_name.get(&resource_name).copied();
        if let Some(resource) = resource {
            represented_resources.insert(resource.id.clone());
        }
        let affected_pages = pages
            .iter()
            .map(|page| page_usage(page))
            .collect::<Vec<_>>();
        entries.push(semantic_entry(
            format!("semantic:default-page:{resource_name}"),
            TemplateSemanticCategory::Page,
            TemplateSemanticRole::DefaultPage,
            "Pagini implicite".to_string(),
            Some(LocalizedDiagnostic::new("templates-semantic-default-pages")),
            TemplateSemanticTarget {
                id: "site:default-pages".to_string(),
                kind: TemplateSemanticTargetKind::Site,
                label: None,
                label_diagnostic: Some(LocalizedDiagnostic::new(
                    "templates-target-unassigned-pages",
                )),
                file: None,
                url: None,
            },
            assignment_projection(
                resource,
                &resource_name,
                "template",
                TemplateAssignmentSource::Default,
                None,
                "page.html",
            ),
            pages
                .first()
                .map(|page| preview_for_page(page, TemplateCatalogContext::Page)),
            affected_pages,
        ));
    }

    let not_found_resource = effective_by_name.get("404.html").copied();
    if let Some(resource) = not_found_resource {
        represented_resources.insert(resource.id.clone());
    }
    entries.push(semantic_entry(
        "semantic:system:404".to_string(),
        TemplateSemanticCategory::System,
        TemplateSemanticRole::NotFound,
        "Pagina 404".to_string(),
        Some(LocalizedDiagnostic::new("templates-semantic-not-found")),
        TemplateSemanticTarget {
            id: "system:404".to_string(),
            kind: TemplateSemanticTargetKind::System,
            label: None,
            label_diagnostic: Some(LocalizedDiagnostic::new("templates-target-not-found")),
            file: None,
            url: Some("/404.html".to_string()),
        },
        assignment_projection(
            not_found_resource,
            "404.html",
            "convention",
            TemplateAssignmentSource::Convention,
            None,
            "404.html",
        ),
        Some(TemplatePreviewContext {
            kind: TemplateCatalogContext::System,
            page_file: None,
            title: None,
            title_diagnostic: Some(LocalizedDiagnostic::new("templates-semantic-not-found")),
            url: "/404.html".to_string(),
            exact: true,
            available: not_found_resource.is_some(),
            unavailable_diagnostic: not_found_resource
                .is_none()
                .then(|| LocalizedDiagnostic::new("templates-preview-404-missing")),
        }),
        Vec::new(),
    ));

    if let Some(taxonomies) = taxonomies {
        for taxonomy in taxonomies.entries.iter().filter(|entry| entry.render) {
            let list_name = normalize_template_name(&taxonomy.list_template.logical_name);
            let list_resource = effective_by_name.get(&list_name).copied();
            if let Some(resource) = list_resource {
                represented_resources.insert(resource.id.clone());
            }
            entries.push(semantic_entry(
                format!("semantic:taxonomy-list:{}", taxonomy.id),
                TemplateSemanticCategory::Taxonomy,
                TemplateSemanticRole::TaxonomyList,
                format!("Listă {}", taxonomy.name),
                Some(
                    LocalizedDiagnostic::new("templates-semantic-taxonomy-list")
                        .with_argument("name", taxonomy.name.clone()),
                ),
                TemplateSemanticTarget {
                    id: taxonomy.id.clone(),
                    kind: TemplateSemanticTargetKind::Taxonomy,
                    label: Some(taxonomy.name.clone()),
                    label_diagnostic: None,
                    file: Some(taxonomies.config_path.clone()),
                    url: Some(taxonomy.path.clone()),
                },
                assignment_projection(
                    list_resource,
                    &list_name,
                    "convention",
                    TemplateAssignmentSource::Convention,
                    Some(taxonomies.config_path.clone()),
                    "taxonomy_list.html",
                ),
                Some(TemplatePreviewContext {
                    kind: TemplateCatalogContext::System,
                    page_file: None,
                    title: None,
                    title_diagnostic: Some(
                        LocalizedDiagnostic::new("templates-semantic-taxonomy-list")
                            .with_argument("name", taxonomy.name.clone()),
                    ),
                    url: taxonomy.path.clone(),
                    exact: true,
                    available: true,
                    unavailable_diagnostic: None,
                }),
                taxonomy
                    .pages
                    .iter()
                    .map(|page| TemplateCatalogPageUsage {
                        file: page.file.clone(),
                        title: page.title.clone(),
                        url: page.url.clone(),
                    })
                    .collect(),
            ));

            let term_name = normalize_template_name(&taxonomy.term_template.logical_name);
            let term_resource = effective_by_name.get(&term_name).copied();
            if let Some(resource) = term_resource {
                represented_resources.insert(resource.id.clone());
            }
            let first_term = taxonomy.terms.first();
            entries.push(semantic_entry(
                format!("semantic:taxonomy-term:{}", taxonomy.id),
                TemplateSemanticCategory::Taxonomy,
                TemplateSemanticRole::TaxonomyTerm,
                format!("Termen {}", taxonomy.name),
                Some(
                    LocalizedDiagnostic::new("templates-semantic-taxonomy-term")
                        .with_argument("name", taxonomy.name.clone()),
                ),
                TemplateSemanticTarget {
                    id: taxonomy.id.clone(),
                    kind: TemplateSemanticTargetKind::Taxonomy,
                    label: Some(taxonomy.name.clone()),
                    label_diagnostic: None,
                    file: Some(taxonomies.config_path.clone()),
                    url: first_term
                        .map(|term| term.path.clone())
                        .or_else(|| Some(taxonomy.path.clone())),
                },
                assignment_projection(
                    term_resource,
                    &term_name,
                    "convention",
                    TemplateAssignmentSource::Convention,
                    Some(taxonomies.config_path.clone()),
                    "taxonomy_single.html",
                ),
                Some(TemplatePreviewContext {
                    kind: TemplateCatalogContext::System,
                    page_file: None,
                    title: first_term.map(|term| term.name.clone()),
                    title_diagnostic: first_term.is_none().then(|| {
                        LocalizedDiagnostic::new("templates-semantic-taxonomy-term")
                            .with_argument("name", taxonomy.name.clone())
                    }),
                    url: first_term
                        .map(|term| term.path.clone())
                        .unwrap_or_else(|| taxonomy.path.clone()),
                    exact: true,
                    available: first_term.is_some(),
                    unavailable_diagnostic: first_term
                        .is_none()
                        .then(|| LocalizedDiagnostic::new("templates-preview-taxonomy-term-empty")),
                }),
                taxonomy
                    .pages
                    .iter()
                    .map(|page| TemplateCatalogPageUsage {
                        file: page.file.clone(),
                        title: page.title.clone(),
                        url: page.url.clone(),
                    })
                    .collect(),
            ));
        }
    }

    for listing_item in &graph.listing_items.items {
        let resource = effective_by_name
            .get(&normalize_template_name(&listing_item.template_name))
            .copied();
        if let Some(resource) = resource {
            represented_resources.insert(resource.id.clone());
        }
        let preview_page = listing_item
            .preview_page_file
            .as_deref()
            .and_then(|file| graph.pages.iter().find(|page| page.file == file));
        let affected_pages = preview_page
            .map(|page| vec![page_usage(page)])
            .unwrap_or_default();
        entries.push(semantic_entry(
            format!("semantic:listing-item:{}", listing_item.id),
            TemplateSemanticCategory::ListingItem,
            TemplateSemanticRole::ListingItem,
            listing_item.label.clone(),
            None,
            TemplateSemanticTarget {
                id: listing_item.id.clone(),
                kind: TemplateSemanticTargetKind::Resource,
                label: Some(listing_item.label.clone()),
                label_diagnostic: None,
                file: Some(listing_item.file.clone()),
                url: listing_item.preview_url.clone(),
            },
            TemplateAssignment {
                key: Some("include".to_string()),
                source: TemplateAssignmentSource::Convention,
                declared_in: Some(
                    crate::kernel::listing_items::LISTING_ITEM_METADATA_PATH.to_string(),
                ),
                resource_id: resource.map(|resource| resource.id.clone()),
                resource_name: listing_item.template_name.clone(),
                fallback_name: None,
            },
            Some(TemplatePreviewContext {
                kind: TemplateCatalogContext::Page,
                page_file: listing_item.preview_page_file.clone(),
                title: preview_page.map(|page| page.title.clone()),
                title_diagnostic: None,
                url: listing_item.preview_url.clone().unwrap_or_default(),
                exact: true,
                available: preview_page.is_some() && resource.is_some(),
                unavailable_diagnostic: (preview_page.is_none() || resource.is_none()).then(|| {
                    LocalizedDiagnostic::new("templates-preview-listing-item-unavailable")
                }),
            }),
            affected_pages,
        ));
    }

    for resource in resources.iter().filter(|resource| {
        resource.effective
            && resource.roles.contains(&TemplateCatalogRole::Page)
            && !represented_resources.contains(&resource.id)
            && !resource.roles.iter().any(|role| {
                matches!(
                    role,
                    TemplateCatalogRole::Partial
                        | TemplateCatalogRole::ListingItem
                        | TemplateCatalogRole::MacroLibrary
                        | TemplateCatalogRole::Shortcode
                )
            })
    }) {
        entries.push(semantic_entry(
            format!("semantic:custom:{}", resource.id),
            TemplateSemanticCategory::Page,
            TemplateSemanticRole::Custom,
            semantic_resource_label(&resource.name),
            None,
            TemplateSemanticTarget {
                id: resource.id.clone(),
                kind: TemplateSemanticTargetKind::Custom,
                label: Some(resource.name.clone()),
                label_diagnostic: None,
                file: Some(resource.file.clone()),
                url: None,
            },
            TemplateAssignment {
                key: None,
                source: TemplateAssignmentSource::Convention,
                declared_in: None,
                resource_id: Some(resource.id.clone()),
                resource_name: resource.name.clone(),
                fallback_name: None,
            },
            preview_from_usage(
                resource.affected_pages.first(),
                TemplateCatalogContext::Page,
            ),
            resource.affected_pages.clone(),
        ));
    }

    entries
}

// Semantic entries mirror the immutable catalog schema and its distinct evidence projections.
#[allow(clippy::too_many_arguments)]
fn semantic_entry(
    id: String,
    category: TemplateSemanticCategory,
    role: TemplateSemanticRole,
    label: String,
    label_diagnostic: Option<LocalizedDiagnostic>,
    target: TemplateSemanticTarget,
    assignment: TemplateAssignment,
    preview_context: Option<TemplatePreviewContext>,
    affected_pages: Vec<TemplateCatalogPageUsage>,
) -> TemplateSemanticEntry {
    let sort_label = label;
    TemplateSemanticEntry {
        id,
        category,
        role,
        label: label_diagnostic.is_none().then(|| sort_label.clone()),
        label_diagnostic,
        target,
        assignment,
        preview_context,
        affected_pages,
        sort_label,
    }
}

fn assignment_projection(
    resource: Option<&TemplateResource>,
    resource_name: &str,
    key: &str,
    source: TemplateAssignmentSource,
    declared_in: Option<String>,
    fallback_name: &str,
) -> TemplateAssignment {
    TemplateAssignment {
        key: Some(key.to_string()),
        source,
        declared_in,
        resource_id: resource.map(|resource| resource.id.clone()),
        resource_name: resource_name.to_string(),
        fallback_name: (resource_name != fallback_name).then(|| fallback_name.to_string()),
    }
}

fn page_target(page: &SourceGraphPage, kind: TemplateSemanticTargetKind) -> TemplateSemanticTarget {
    TemplateSemanticTarget {
        id: page.id.clone(),
        kind,
        label: Some(page.title.clone()),
        label_diagnostic: None,
        file: Some(page.file.clone()),
        url: Some(page.url.clone()),
    }
}

fn page_usage(page: &SourceGraphPage) -> TemplateCatalogPageUsage {
    TemplateCatalogPageUsage {
        file: page.file.clone(),
        title: page.title.clone(),
        url: page.url.clone(),
    }
}

fn preview_for_page(
    page: &SourceGraphPage,
    kind: TemplateCatalogContext,
) -> TemplatePreviewContext {
    TemplatePreviewContext {
        kind,
        page_file: Some(page.file.clone()),
        title: Some(page.title.clone()),
        title_diagnostic: None,
        url: page.url.clone(),
        exact: true,
        available: true,
        unavailable_diagnostic: None,
    }
}

fn preview_from_usage(
    usage: Option<&TemplateCatalogPageUsage>,
    kind: TemplateCatalogContext,
) -> Option<TemplatePreviewContext> {
    usage.map(|usage| TemplatePreviewContext {
        kind,
        page_file: Some(usage.file.clone()),
        title: Some(usage.title.clone()),
        title_diagnostic: None,
        url: usage.url.clone(),
        exact: true,
        available: true,
        unavailable_diagnostic: None,
    })
}

fn semantic_resource_label(name: &str) -> String {
    name.trim_end_matches(".html")
        .rsplit('/')
        .next()
        .unwrap_or(name)
        .replace(['-', '_'], " ")
}

fn semantic_category_rank(category: TemplateSemanticCategory) -> u8 {
    match category {
        TemplateSemanticCategory::Layout => 0,
        TemplateSemanticCategory::Page => 1,
        TemplateSemanticCategory::Archive => 2,
        TemplateSemanticCategory::Element => 3,
        TemplateSemanticCategory::ListingItem => 4,
        TemplateSemanticCategory::Taxonomy => 5,
        TemplateSemanticCategory::System => 6,
    }
}

fn normalize_template_name(name: &str) -> String {
    super::zola::normalize_zola_template_reference(name)
}

fn template_dependency_closure(graph: &SourceGraph, start_node_id: &str) -> Vec<String> {
    let mut queue = VecDeque::from([start_node_id.to_string()]);
    let mut visited = BTreeSet::new();
    while let Some(node_id) = queue.pop_front() {
        if !visited.insert(node_id.clone()) {
            continue;
        }
        for relation in graph
            .relations
            .iter()
            .filter(|relation| relation.from == node_id)
            .filter(|relation| reference_kind(&relation.kind).is_some())
        {
            queue.push_back(relation.to.clone());
        }
    }
    visited.into_iter().collect()
}

fn semantic_template_assignments(graph: &SourceGraph) -> Vec<(String, TemplateCatalogAssignment)> {
    let sections = graph
        .pages
        .iter()
        .filter(|page| {
            matches!(
                page.page_kind,
                SourcePageKind::Home | SourcePageKind::Section
            )
        })
        .collect::<Vec<_>>();
    let mut assignments = graph
        .pages
        .iter()
        .filter_map(|page| {
            let (template_name, source, declared_in, frontmatter_key) = match page.page_kind {
                SourcePageKind::Home | SourcePageKind::Section => {
                    if let Some(template) = page.frontmatter_template.as_ref() {
                        (
                            normalize_template_name(template),
                            TemplateAssignmentSource::Explicit,
                            Some(page.file.clone()),
                            Some("template".to_string()),
                        )
                    } else {
                        (
                            page.resolved_template
                                .as_deref()
                                .map(normalize_template_name)
                                .unwrap_or_else(|| {
                                    if matches!(page.page_kind, SourcePageKind::Home) {
                                        "index.html".to_string()
                                    } else {
                                        "section.html".to_string()
                                    }
                                }),
                            TemplateAssignmentSource::Default,
                            None,
                            None,
                        )
                    }
                }
                SourcePageKind::Page => {
                    if let Some(template) = page.frontmatter_template.as_ref() {
                        (
                            normalize_template_name(template),
                            TemplateAssignmentSource::Explicit,
                            Some(page.file.clone()),
                            Some("template".to_string()),
                        )
                    } else if let Some(section) = owning_section(page, &sections) {
                        let (name, _source, declared_in) =
                            effective_section_page_template(section, &sections);
                        if let Some(declared_in) = declared_in {
                            (
                                name,
                                TemplateAssignmentSource::Inherited,
                                Some(declared_in),
                                Some("page_template".to_string()),
                            )
                        } else {
                            (name, TemplateAssignmentSource::Default, None, None)
                        }
                    } else {
                        (
                            "page.html".to_string(),
                            TemplateAssignmentSource::Default,
                            None,
                            None,
                        )
                    }
                }
            };
            if template_name.is_empty() {
                return None;
            }
            Some((
                template_name,
                TemplateCatalogAssignment {
                    file: page.file.clone(),
                    title: page.title.clone(),
                    url: page.url.clone(),
                    context: if matches!(page.page_kind, SourcePageKind::Page) {
                        TemplateCatalogContext::Page
                    } else {
                        TemplateCatalogContext::Section
                    },
                    source,
                    declared_in,
                    frontmatter_key,
                },
            ))
        })
        .collect::<Vec<_>>();
    assignments.sort_by(|left, right| {
        left.1
            .url
            .cmp(&right.1.url)
            .then_with(|| left.1.file.cmp(&right.1.file))
    });
    assignments
}

fn effective_section_page_template(
    section: &SourceGraphPage,
    sections: &[&SourceGraphPage],
) -> (String, TemplateAssignmentSource, Option<String>) {
    if let Some(template) = section.frontmatter_page_template.as_ref() {
        return (
            normalize_template_name(template),
            TemplateAssignmentSource::Explicit,
            Some(section.file.clone()),
        );
    }
    let current_directory = section_directory(&section.file);
    let inherited = sections
        .iter()
        .filter(|candidate| candidate.file != section.file)
        .filter_map(|candidate| {
            let candidate_directory = section_directory(&candidate.file);
            let template = candidate.frontmatter_page_template.as_ref()?;
            is_directory_ancestor(&candidate_directory, &current_directory).then_some((
                candidate_directory.len(),
                normalize_template_name(template),
                candidate.file.clone(),
            ))
        })
        .max_by_key(|(length, _, _)| *length);
    if let Some((_, template, file)) = inherited {
        return (template, TemplateAssignmentSource::Inherited, Some(file));
    }
    (
        "page.html".to_string(),
        TemplateAssignmentSource::Default,
        None,
    )
}

fn owning_section<'a>(
    page: &SourceGraphPage,
    sections: &'a [&SourceGraphPage],
) -> Option<&'a SourceGraphPage> {
    let page_directory = page_directory(&page.file);
    sections
        .iter()
        .copied()
        .filter(|section| {
            let directory = section_directory(&section.file);
            directory == page_directory || is_directory_ancestor(&directory, &page_directory)
        })
        .max_by_key(|section| section_directory(&section.file).len())
}

fn section_directory(file: &str) -> String {
    file.strip_suffix("/_index.md")
        .unwrap_or_else(|| file.strip_suffix("_index.md").unwrap_or(file))
        .trim_end_matches('/')
        .to_string()
}

fn page_directory(file: &str) -> String {
    file.rsplit_once('/')
        .map(|(directory, _)| directory.to_string())
        .unwrap_or_default()
}

fn is_directory_ancestor(candidate: &str, descendant: &str) -> bool {
    candidate == descendant
        || descendant
            .strip_prefix(candidate)
            .is_some_and(|remainder| remainder.starts_with('/'))
}

fn direct_template_usages(
    graph: &SourceGraph,
    template: &SourceGraphTemplate,
    templates_by_node: &HashMap<&str, &SourceGraphTemplate>,
) -> Vec<TemplateCatalogTemplateUsage> {
    let mut usages = graph
        .relations
        .iter()
        .filter(|relation| relation.to == template.node_id)
        .filter_map(|relation| {
            let kind = reference_kind(&relation.kind)?;
            let source = templates_by_node.get(relation.from.as_str())?;
            Some(TemplateCatalogTemplateUsage {
                file: source.file.clone(),
                name: source.name.clone(),
                kind,
            })
        })
        .collect::<Vec<_>>();
    usages.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.file.cmp(&right.file))
    });
    usages.dedup();
    usages
}

fn affected_pages(
    graph: &SourceGraph,
    template: &SourceGraphTemplate,
    templates_by_node: &HashMap<&str, &SourceGraphTemplate>,
    pages_by_node: &HashMap<&str, &SourceGraphPage>,
) -> Vec<TemplateCatalogPageUsage> {
    let mut queue = VecDeque::from([template.node_id.as_str()]);
    let mut visited_templates = HashSet::new();
    let mut pages = BTreeSet::new();

    while let Some(node_id) = queue.pop_front() {
        if !visited_templates.insert(node_id) {
            continue;
        }
        for relation in graph
            .relations
            .iter()
            .filter(|relation| relation.to == node_id)
        {
            if reference_kind(&relation.kind).is_some() {
                if let Some(source) = templates_by_node.get(relation.from.as_str()) {
                    queue.push_back(source.node_id.as_str());
                }
                continue;
            }
            if matches!(
                relation.kind,
                SourceRelationKind::PageTemplate | SourceRelationKind::SectionPageTemplate
            ) {
                if let Some(page) = pages_by_node.get(relation.from.as_str()) {
                    pages.insert((page.file.clone(), page.title.clone(), page.url.clone()));
                }
            }
        }
        for page in &graph.pages {
            if page.template_node_id.as_deref() == Some(node_id)
                || page.page_template_node_id.as_deref() == Some(node_id)
            {
                pages.insert((page.file.clone(), page.title.clone(), page.url.clone()));
            }
        }
    }

    pages
        .into_iter()
        .map(|(file, title, url)| TemplateCatalogPageUsage { file, title, url })
        .collect()
}

fn template_roles(
    template: &SourceGraphTemplate,
    used_by_templates: &[TemplateCatalogTemplateUsage],
    directly_serves_page: bool,
) -> Vec<TemplateCatalogRole> {
    let mut roles = Vec::new();
    let is_macro_library = template.name.starts_with("macros/") || !template.macros.is_empty();
    let is_partial = template.name.starts_with("partials/");
    let is_listing_item = template.name.starts_with("listing-items/");
    let is_shortcode = template.name.starts_with("shortcodes/");
    let is_layout = !template.blocks.is_empty()
        && (used_by_templates
            .iter()
            .any(|usage| usage.kind == TemplateCatalogReferenceKind::Extends)
            || template.name.contains("base")
            || template.name.contains("layout"));

    if directly_serves_page {
        roles.push(TemplateCatalogRole::Page);
    }
    if is_layout {
        roles.push(TemplateCatalogRole::Layout);
    }
    if is_listing_item {
        roles.push(TemplateCatalogRole::ListingItem);
    } else if is_partial {
        roles.push(TemplateCatalogRole::Partial);
    }
    if is_macro_library {
        roles.push(TemplateCatalogRole::MacroLibrary);
    }
    if is_shortcode {
        roles.push(TemplateCatalogRole::Shortcode);
    }
    if roles.is_empty() && !is_partial && !is_listing_item && !is_macro_library && !is_shortcode {
        roles.push(TemplateCatalogRole::Page);
    }
    roles
}

fn directly_serves_page(graph: &SourceGraph, template: &SourceGraphTemplate) -> bool {
    graph.pages.iter().any(|page| {
        page.template_node_id.as_deref() == Some(template.node_id.as_str())
            || page.page_template_node_id.as_deref() == Some(template.node_id.as_str())
    })
}

fn reference_kind(kind: &SourceRelationKind) -> Option<TemplateCatalogReferenceKind> {
    match kind {
        SourceRelationKind::Extends => Some(TemplateCatalogReferenceKind::Extends),
        SourceRelationKind::Includes => Some(TemplateCatalogReferenceKind::Includes),
        SourceRelationKind::Imports => Some(TemplateCatalogReferenceKind::Imports),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_graph::model::{
        SourceGraphDiagnostic, SourceGraphPage, SourceGraphTemplate, SourceNode, SourcePageKind,
        SourceRelation,
    };
    use crate::source_graph::taxonomy_catalog::{
        TaxonomyCatalogCapabilities, TaxonomyCatalogEntry, TaxonomyCatalogSnapshot,
        TaxonomyCatalogTemplate, TaxonomyCatalogTerm, TAXONOMY_CATALOG_SCHEMA_VERSION,
    };

    fn template(
        id: &str,
        file: &str,
        name: &str,
        origin: SourceOrigin,
        extends: Option<&str>,
        blocks: &[&str],
    ) -> SourceGraphTemplate {
        SourceGraphTemplate {
            id: id.to_string(),
            file: file.to_string(),
            name: name.to_string(),
            origin,
            theme_name: None,
            is_partial: name.starts_with("partials/") || name.starts_with("macros/"),
            extends: extends.map(str::to_string),
            includes: Vec::new(),
            include_groups: Vec::new(),
            imports: Vec::new(),
            get_pages: Vec::new(),
            get_sections: Vec::new(),
            internal_links: Vec::new(),
            asset_urls: Vec::new(),
            asset_hashes: Vec::new(),
            literal_asset_references: Vec::new(),
            asset_reference_eligible: 0,
            asset_reference_unanalysable: 0,
            data_loads: Vec::new(),
            image_metadata: Vec::new(),
            image_resizes: Vec::new(),
            blocks: blocks.iter().map(|value| (*value).to_string()).collect(),
            macros: Vec::new(),
            semantics: None,
            markdown_projections: Vec::new(),
            node_id: id.to_string(),
        }
    }

    fn relation(from: &str, to: &str, kind: SourceRelationKind) -> SourceRelation {
        SourceRelation {
            id: format!("{from}-{to}"),
            from: from.to_string(),
            to: to.to_string(),
            kind,
            label: String::new(),
        }
    }

    // Test fixtures expose every independent assignment identity used by the scenario matrix.
    #[allow(clippy::too_many_arguments)]
    fn page(
        id: &str,
        file: &str,
        title: &str,
        url: &str,
        page_kind: SourcePageKind,
        template: Option<&str>,
        page_template: Option<&str>,
        resolved_template: Option<&str>,
        template_node_id: Option<&str>,
        page_template_node_id: Option<&str>,
    ) -> SourceGraphPage {
        SourceGraphPage {
            id: id.to_string(),
            file: file.to_string(),
            title: title.to_string(),
            url: url.to_string(),
            page_kind,
            frontmatter_template: template.map(str::to_string),
            frontmatter_page_template: page_template.map(str::to_string),
            resolved_template: resolved_template.map(str::to_string),
            content_node_id: format!("{id}-node"),
            template_node_id: template_node_id.map(str::to_string),
            page_template_node_id: page_template_node_id.map(str::to_string),
            frontmatter_format: None,
            frontmatter_parse_error: None,
            frontmatter_nodes: Vec::new(),
            taxonomies: Default::default(),
            shortcode_parse_error: None,
            shortcodes: Vec::new(),
        }
    }

    #[test]
    fn catalog_projects_roles_impact_and_theme_shadowing_from_the_rust_graph() {
        let page = SourceGraphPage {
            id: "page".to_string(),
            file: "content/_index.md".to_string(),
            title: "Acasă".to_string(),
            url: "/".to_string(),
            page_kind: SourcePageKind::Home,
            frontmatter_template: None,
            frontmatter_page_template: None,
            resolved_template: Some("index.html".to_string()),
            content_node_id: "page-node".to_string(),
            template_node_id: Some("index".to_string()),
            page_template_node_id: None,
            frontmatter_format: None,
            frontmatter_parse_error: None,
            frontmatter_nodes: Vec::new(),
            taxonomies: Default::default(),
            shortcode_parse_error: None,
            shortcodes: Vec::new(),
        };
        let graph = SourceGraph {
            node_index: Default::default(),
            project_root: "/project".to_string(),
            zola_root: "/project".to_string(),
            active_theme: Some("theme".to_string()),
            pages: vec![page],
            templates: vec![
                template(
                    "base-local",
                    "templates/base.html",
                    "base.html",
                    SourceOrigin::Local,
                    None,
                    &["content"],
                ),
                template(
                    "base-theme",
                    "themes/theme/templates/base.html",
                    "base.html",
                    SourceOrigin::Theme,
                    None,
                    &["content"],
                ),
                template(
                    "index",
                    "templates/index.html",
                    "index.html",
                    SourceOrigin::Local,
                    Some("base.html"),
                    &["content"],
                ),
                template(
                    "footer",
                    "templates/partials/footer.html",
                    "partials/footer.html",
                    SourceOrigin::Local,
                    None,
                    &[],
                ),
            ],
            styles: Vec::new(),
            scripts: Vec::new(),
            assets: Vec::new(),
            data_files: Vec::new(),
            structured_documents: Vec::new(),
            component_graph: Default::default(),
            block_graph: Default::default(),
            content_models: Default::default(),
            listing_items: Default::default(),
            dynamic_widget_graph: Default::default(),
            markdown_projections: Vec::new(),
            nodes: Vec::<SourceNode>::new(),
            relations: vec![
                relation("index", "base-local", SourceRelationKind::Extends),
                relation("index", "footer", SourceRelationKind::Includes),
                relation("page-node", "index", SourceRelationKind::PageTemplate),
            ],
            asset_reference_coverage: Default::default(),
            diagnostics: Vec::<SourceGraphDiagnostic>::new(),
        };

        let catalog = build_template_catalog(&graph);
        let local_base = catalog
            .resources
            .iter()
            .find(|entry| entry.file == "templates/base.html")
            .unwrap();
        let theme_base = catalog
            .resources
            .iter()
            .find(|entry| entry.file == "themes/theme/templates/base.html")
            .unwrap();
        let footer = catalog
            .resources
            .iter()
            .find(|entry| entry.file == "templates/partials/footer.html")
            .unwrap();

        assert!(local_base.roles.contains(&TemplateCatalogRole::Layout));
        assert!(!local_base.roles.contains(&TemplateCatalogRole::Page));
        assert_eq!(local_base.affected_pages[0].file, "content/_index.md");
        assert!(!local_base.can_delete);
        assert!(local_base.effective);
        assert!(!theme_base.effective);
        assert!(!theme_base.editable);
        assert_eq!(theme_base.local_override_path, "templates/base.html");
        assert_eq!(footer.roles, vec![TemplateCatalogRole::Partial]);
        assert_eq!(footer.affected_pages[0].file, "content/_index.md");
    }

    #[test]
    fn catalog_projects_independent_archive_and_element_semantic_roles() {
        let section = page(
            "blog",
            "content/blog/_index.md",
            "Blog",
            "/blog/",
            SourcePageKind::Section,
            Some("blog/list.html"),
            Some("blog/single.html"),
            Some("blog/list.html"),
            Some("blog-list"),
            Some("blog-single"),
        );
        let inherited = page(
            "article",
            "content/blog/primul.md",
            "Primul articol",
            "/blog/primul/",
            SourcePageKind::Page,
            None,
            None,
            Some("blog/single.html"),
            Some("blog-single"),
            None,
        );
        let overridden = page(
            "special",
            "content/blog/special.md",
            "Articol special",
            "/blog/special/",
            SourcePageKind::Page,
            Some("special.html"),
            None,
            Some("special.html"),
            Some("special"),
            None,
        );
        let graph = SourceGraph {
            node_index: Default::default(),
            project_root: "/project".to_string(),
            zola_root: "/project".to_string(),
            active_theme: None,
            pages: vec![section, inherited, overridden],
            templates: vec![
                template(
                    "base",
                    "templates/base.html",
                    "base.html",
                    SourceOrigin::Local,
                    None,
                    &["content"],
                ),
                template(
                    "blog-list",
                    "templates/blog/list.html",
                    "blog/list.html",
                    SourceOrigin::Local,
                    Some("base.html"),
                    &["content"],
                ),
                template(
                    "blog-single",
                    "templates/blog/single.html",
                    "blog/single.html",
                    SourceOrigin::Local,
                    Some("base.html"),
                    &["content"],
                ),
                template(
                    "special",
                    "templates/special.html",
                    "special.html",
                    SourceOrigin::Local,
                    Some("base.html"),
                    &["content"],
                ),
                template(
                    "shortcode",
                    "templates/shortcodes/galerie.html",
                    "shortcodes/galerie.html",
                    SourceOrigin::Local,
                    None,
                    &[],
                ),
            ],
            styles: Vec::new(),
            scripts: Vec::new(),
            assets: Vec::new(),
            data_files: Vec::new(),
            structured_documents: Vec::new(),
            component_graph: Default::default(),
            block_graph: Default::default(),
            content_models: Default::default(),
            listing_items: Default::default(),
            dynamic_widget_graph: Default::default(),
            markdown_projections: Vec::new(),
            nodes: Vec::new(),
            relations: vec![
                relation("blog-list", "base", SourceRelationKind::Extends),
                relation("blog-single", "base", SourceRelationKind::Extends),
                relation("special", "base", SourceRelationKind::Extends),
                relation("blog-node", "blog-list", SourceRelationKind::PageTemplate),
                relation(
                    "article-node",
                    "blog-single",
                    SourceRelationKind::PageTemplate,
                ),
                relation("special-node", "special", SourceRelationKind::PageTemplate),
            ],
            asset_reference_coverage: Default::default(),
            diagnostics: Vec::new(),
        };

        let catalog = build_template_catalog(&graph);
        assert_eq!(catalog.schema_version, TEMPLATE_CATALOG_SCHEMA_VERSION);
        let archive = catalog
            .semantic_entries
            .iter()
            .find(|entry| entry.role == TemplateSemanticRole::SectionArchive)
            .unwrap();
        let element = catalog
            .semantic_entries
            .iter()
            .find(|entry| entry.role == TemplateSemanticRole::SectionElement)
            .unwrap();
        assert_eq!(
            archive
                .label_diagnostic
                .as_ref()
                .map(|diagnostic| diagnostic.code.as_str()),
            Some("templates-semantic-section-archive")
        );
        assert_eq!(
            archive.target.file.as_deref(),
            Some("content/blog/_index.md")
        );
        assert_eq!(archive.assignment.resource_name, "blog/list.html");
        assert_eq!(
            archive.assignment.source,
            TemplateAssignmentSource::Explicit
        );
        assert_eq!(
            element
                .label_diagnostic
                .as_ref()
                .map(|diagnostic| diagnostic.code.as_str()),
            Some("templates-semantic-section-element")
        );
        assert_eq!(element.assignment.resource_name, "blog/single.html");
        assert_eq!(
            element.assignment.source,
            TemplateAssignmentSource::Explicit
        );
        assert_eq!(element.affected_pages.len(), 1);
        assert_eq!(
            element.assignment.declared_in.as_deref(),
            Some("content/blog/_index.md")
        );
        assert_eq!(
            element
                .preview_context
                .as_ref()
                .unwrap()
                .page_file
                .as_deref(),
            Some("content/blog/primul.md")
        );

        let shortcode = catalog
            .resources
            .iter()
            .find(|entry| entry.name == "shortcodes/galerie.html")
            .unwrap();
        assert_eq!(shortcode.roles, vec![TemplateCatalogRole::Shortcode]);
        assert!(!catalog.semantic_entries.iter().any(|entry| entry
            .assignment
            .resource_id
            .as_deref()
            == Some(shortcode.id.as_str())));
    }

    #[test]
    fn catalog_keeps_default_page_and_not_found_slots_without_resources_or_consumers() {
        let graph = SourceGraph {
            node_index: Default::default(),
            project_root: "/project".to_string(),
            zola_root: "/project".to_string(),
            active_theme: None,
            pages: Vec::new(),
            templates: Vec::new(),
            styles: Vec::new(),
            scripts: Vec::new(),
            assets: Vec::new(),
            data_files: Vec::new(),
            structured_documents: Vec::new(),
            component_graph: Default::default(),
            block_graph: Default::default(),
            content_models: Default::default(),
            listing_items: Default::default(),
            dynamic_widget_graph: Default::default(),
            markdown_projections: Vec::new(),
            nodes: Vec::new(),
            relations: Vec::new(),
            asset_reference_coverage: Default::default(),
            diagnostics: Vec::new(),
        };

        let catalog = build_template_catalog(&graph);
        let default_page = catalog
            .semantic_entries
            .iter()
            .find(|entry| entry.role == TemplateSemanticRole::DefaultPage)
            .unwrap();
        assert_eq!(default_page.assignment.resource_name, "page.html");
        assert!(default_page.assignment.resource_id.is_none());
        assert!(default_page.affected_pages.is_empty());

        let not_found = catalog
            .semantic_entries
            .iter()
            .find(|entry| entry.role == TemplateSemanticRole::NotFound)
            .unwrap();
        assert_eq!(not_found.assignment.resource_name, "404.html");
        assert!(not_found.assignment.resource_id.is_none());
        assert!(!not_found.preview_context.as_ref().unwrap().available);
    }

    #[test]
    fn catalog_projects_taxonomy_list_and_term_as_independent_semantic_uses() {
        let graph = SourceGraph {
            node_index: Default::default(),
            project_root: "/project".to_string(),
            zola_root: "/project".to_string(),
            active_theme: None,
            pages: Vec::new(),
            templates: vec![
                template(
                    "taxonomy-list",
                    "templates/taxonomy_list.html",
                    "taxonomy_list.html",
                    SourceOrigin::Local,
                    None,
                    &[],
                ),
                template(
                    "taxonomy-single",
                    "templates/taxonomy_single.html",
                    "taxonomy_single.html",
                    SourceOrigin::Local,
                    None,
                    &[],
                ),
            ],
            styles: Vec::new(),
            scripts: Vec::new(),
            assets: Vec::new(),
            data_files: Vec::new(),
            structured_documents: Vec::new(),
            component_graph: Default::default(),
            block_graph: Default::default(),
            content_models: Default::default(),
            listing_items: Default::default(),
            dynamic_widget_graph: Default::default(),
            markdown_projections: Vec::new(),
            nodes: Vec::new(),
            relations: Vec::new(),
            asset_reference_coverage: Default::default(),
            diagnostics: Vec::new(),
        };
        let taxonomies = TaxonomyCatalogSnapshot {
            schema_version: TAXONOMY_CATALOG_SCHEMA_VERSION,
            config_path: "zola.toml".to_string(),
            taxonomy_root: None,
            default_language: "ro".to_string(),
            slugify_strategy: "on".to_string(),
            entries: vec![TaxonomyCatalogEntry {
                id: "taxonomy:ro:categorii".to_string(),
                name: "categorii".to_string(),
                slug: "categorii".to_string(),
                language: "ro".to_string(),
                declared: true,
                render: true,
                feed: false,
                paginate_by: None,
                paginate_path: None,
                path: "/categorii/".to_string(),
                permalink: "https://example.test/categorii/".to_string(),
                terms: vec![TaxonomyCatalogTerm {
                    id: "taxonomy-term:ro:categorii:rust".to_string(),
                    name: "Rust".to_string(),
                    aliases: vec!["Rust".to_string()],
                    slug: "rust".to_string(),
                    path: "/categorii/rust/".to_string(),
                    permalink: "https://example.test/categorii/rust/".to_string(),
                    pages: Vec::new(),
                }],
                pages: Vec::new(),
                list_template: TaxonomyCatalogTemplate {
                    logical_name: "taxonomy_list.html".to_string(),
                    file: Some("templates/taxonomy_list.html".to_string()),
                    origin: Some(SourceOrigin::Local),
                    theme_name: None,
                    fallback: true,
                    missing: false,
                },
                term_template: TaxonomyCatalogTemplate {
                    logical_name: "taxonomy_single.html".to_string(),
                    file: Some("templates/taxonomy_single.html".to_string()),
                    origin: Some(SourceOrigin::Local),
                    theme_name: None,
                    fallback: true,
                    missing: false,
                },
                capabilities: TaxonomyCatalogCapabilities {
                    can_edit_definition: true,
                    can_delete_definition: true,
                    can_assign_terms: true,
                },
            }],
            diagnostics: Vec::new(),
        };

        let catalog = build_template_catalog_with_taxonomies(&graph, Some(&taxonomies));
        let list = catalog
            .semantic_entries
            .iter()
            .find(|entry| entry.role == TemplateSemanticRole::TaxonomyList)
            .unwrap();
        let term = catalog
            .semantic_entries
            .iter()
            .find(|entry| entry.role == TemplateSemanticRole::TaxonomyTerm)
            .unwrap();

        assert_eq!(
            list.label_diagnostic
                .as_ref()
                .map(|diagnostic| diagnostic.code.as_str()),
            Some("templates-semantic-taxonomy-list")
        );
        assert_eq!(list.assignment.resource_name, "taxonomy_list.html");
        assert_eq!(list.preview_context.as_ref().unwrap().url, "/categorii/");
        assert_eq!(
            term.label_diagnostic
                .as_ref()
                .map(|diagnostic| diagnostic.code.as_str()),
            Some("templates-semantic-taxonomy-term")
        );
        assert_eq!(term.assignment.resource_name, "taxonomy_single.html");
        assert_eq!(
            term.preview_context.as_ref().unwrap().url,
            "/categorii/rust/"
        );
        assert_ne!(list.id, term.id);
    }
}
