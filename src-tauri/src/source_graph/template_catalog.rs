use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use serde::Serialize;

use super::model::{
    SourceGraph, SourceGraphPage, SourceGraphTemplate, SourceOrigin, SourceRelationKind,
};

pub const TEMPLATE_CATALOG_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateCatalogRole {
    Page,
    Layout,
    Partial,
    MacroLibrary,
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
pub struct TemplateCatalogEntry {
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
    pub delete_blocked_reason: Option<String>,
    pub node_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateCatalogSnapshot {
    pub schema_version: u32,
    pub active_theme: Option<String>,
    pub entries: Vec<TemplateCatalogEntry>,
}

pub fn build_template_catalog(graph: &SourceGraph) -> TemplateCatalogSnapshot {
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

    let mut entries = graph
        .templates
        .iter()
        .map(|template| {
            let used_by_templates =
                direct_template_usages(graph, template, &templates_by_node);
            let affected_pages =
                affected_pages(graph, template, &templates_by_node, &pages_by_node);
            let roles = template_roles(
                template,
                &used_by_templates,
                directly_serves_page(graph, template),
            );
            let editable = template.origin == SourceOrigin::Local;
            let effective = editable || !local_template_names.contains(template.name.as_str());
            let incoming_count = used_by_templates.len() + affected_pages.len();
            let delete_blocked_reason = if !editable {
                Some(
                    "Șabloanele temei sunt read-only. Creează mai întâi o suprascriere locală."
                        .to_string(),
                )
            } else if incoming_count > 0 {
                Some(format!(
                    "Șablonul este folosit de {} surse. Elimină sau mută referințele înainte de ștergere.",
                    incoming_count
                ))
            } else {
                None
            };

            TemplateCatalogEntry {
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
                can_delete: delete_blocked_reason.is_none(),
                delete_blocked_reason,
                node_id: template.node_id.clone(),
            }
        })
        .collect::<Vec<_>>();

    entries.sort_by(|left, right| {
        right
            .effective
            .cmp(&left.effective)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.file.cmp(&right.file))
    });

    TemplateCatalogSnapshot {
        schema_version: TEMPLATE_CATALOG_SCHEMA_VERSION,
        active_theme: graph.active_theme.clone(),
        entries,
    }
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
    if is_partial {
        roles.push(TemplateCatalogRole::Partial);
    }
    if is_macro_library {
        roles.push(TemplateCatalogRole::MacroLibrary);
    }
    if roles.is_empty() && !is_partial && !is_macro_library {
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
            data_loads: Vec::new(),
            image_metadata: Vec::new(),
            image_resizes: Vec::new(),
            blocks: blocks.iter().map(|value| (*value).to_string()).collect(),
            macros: Vec::new(),
            semantics: None,
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
            shortcode_parse_error: None,
            shortcodes: Vec::new(),
        };
        let graph = SourceGraph {
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
            nodes: Vec::<SourceNode>::new(),
            relations: vec![
                relation("index", "base-local", SourceRelationKind::Extends),
                relation("index", "footer", SourceRelationKind::Includes),
                relation("page-node", "index", SourceRelationKind::PageTemplate),
            ],
            diagnostics: Vec::<SourceGraphDiagnostic>::new(),
        };

        let catalog = build_template_catalog(&graph);
        let local_base = catalog
            .entries
            .iter()
            .find(|entry| entry.file == "templates/base.html")
            .unwrap();
        let theme_base = catalog
            .entries
            .iter()
            .find(|entry| entry.file == "themes/theme/templates/base.html")
            .unwrap();
        let footer = catalog
            .entries
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
}
