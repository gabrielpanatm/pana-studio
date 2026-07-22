use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::{
    project_model::model::ProjectModel,
    source_graph::model::{SourceGraphPage, SourceGraphTemplate, SourceOrigin, SourceRelationKind},
};

pub const TEMPLATE_WORKBENCH_PLAN_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateWorkbenchPlanInput {
    pub template_path: String,
    #[serde(default)]
    pub preferred_page_path: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateWorkbenchPlan {
    pub schema_version: u32,
    pub project_model_revision: String,
    pub active_template: TemplateWorkbenchTemplate,
    pub direct_parent: Option<TemplateWorkbenchTemplate>,
    pub navigator: Vec<TemplateWorkbenchNavigatorEntry>,
    pub consumers: Vec<TemplateWorkbenchConsumer>,
    pub selected_context: Option<TemplateWorkbenchConsumer>,
    pub render_mode: TemplateWorkbenchRenderMode,
    pub render_context: TemplateWorkbenchRenderContext,
    pub diagnostics: Vec<TemplateWorkbenchDiagnostic>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateWorkbenchTemplate {
    pub source_id: String,
    pub file: String,
    pub name: String,
    pub origin: SourceOrigin,
    pub theme_name: Option<String>,
    pub is_partial: bool,
    pub defines_macros: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateWorkbenchNavigatorEntry {
    pub role: TemplateWorkbenchNavigatorRole,
    pub template: TemplateWorkbenchTemplate,
    pub expanded: bool,
    pub editable: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TemplateWorkbenchNavigatorRole {
    DirectParent,
    Active,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateWorkbenchConsumer {
    pub page_id: String,
    pub page_file: String,
    pub page_title: String,
    pub page_url: String,
    pub root_template_source_id: String,
    pub root_template_file: String,
    pub dependency_path: Vec<TemplateWorkbenchDependencyStep>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateWorkbenchDependencyStep {
    pub from_source_id: String,
    pub from_file: String,
    pub to_source_id: String,
    pub to_file: String,
    pub kind: TemplateWorkbenchDependencyKind,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TemplateWorkbenchDependencyKind {
    Extends,
    Includes,
    Imports,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TemplateWorkbenchRenderMode {
    Page,
    IncludedTemplate,
    MacroScenario,
    OrphanTemplate,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TemplateWorkbenchRenderContext {
    pub kind: TemplateWorkbenchRenderContextKind,
    pub canonical_truth: bool,
    pub label: String,
    pub explanation: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TemplateWorkbenchRenderContextKind {
    RealZolaPage,
    RealZolaConsumer,
    ControlledMacroScenario,
    ControlledTemplateFixture,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateWorkbenchDiagnostic {
    pub code: String,
    pub message: String,
}

#[derive(Clone)]
struct DependencyEdge {
    to: String,
    kind: TemplateWorkbenchDependencyKind,
}

pub fn resolve_template_workbench_plan(
    model: &ProjectModel,
    input: &TemplateWorkbenchPlanInput,
) -> Result<TemplateWorkbenchPlan, String> {
    let graph = &model.source_graph;
    let active =
        find_template(graph.templates.as_slice(), &input.template_path).ok_or_else(|| {
            format!(
                "Context de template nu a găsit template-ul «{}» în ProjectModel revizia {}.",
                input.template_path, model.revision
            )
        })?;
    let templates_by_node = graph
        .templates
        .iter()
        .map(|template| (template.node_id.clone(), template))
        .collect::<HashMap<_, _>>();
    let dependencies = dependency_edges(model);

    let direct_parent = dependencies
        .get(&active.node_id)
        .into_iter()
        .flatten()
        .filter(|edge| edge.kind == TemplateWorkbenchDependencyKind::Extends)
        .filter_map(|edge| templates_by_node.get(&edge.to).copied())
        .min_by(|left, right| left.file.cmp(&right.file));

    let mut consumers = graph
        .pages
        .iter()
        .filter_map(|page| consumer_for_page(page, active, &templates_by_node, &dependencies))
        .collect::<Vec<_>>();
    consumers.sort_by(|left, right| {
        left.page_url
            .cmp(&right.page_url)
            .then(left.page_file.cmp(&right.page_file))
    });

    let preferred_page = input
        .preferred_page_path
        .as_deref()
        .map(normalize_project_path)
        .filter(|value| !value.is_empty());
    let selected_context = preferred_page
        .as_deref()
        .and_then(|preferred| {
            consumers.iter().find(|consumer| {
                normalize_project_path(&consumer.page_file) == preferred
                    || normalize_url(&consumer.page_url) == normalize_url(preferred)
            })
        })
        .or_else(|| consumers.first())
        .cloned();

    let render_mode = render_mode(active, selected_context.as_ref());
    let render_context = render_context(&render_mode, selected_context.as_ref());
    let mut navigator = Vec::with_capacity(if direct_parent.is_some() { 2 } else { 1 });
    if let Some(parent) = direct_parent {
        navigator.push(TemplateWorkbenchNavigatorEntry {
            role: TemplateWorkbenchNavigatorRole::DirectParent,
            template: workbench_template(parent),
            expanded: true,
            editable: false,
        });
    }
    navigator.push(TemplateWorkbenchNavigatorEntry {
        role: TemplateWorkbenchNavigatorRole::Active,
        template: workbench_template(active),
        expanded: true,
        editable: true,
    });

    let mut diagnostics = Vec::new();
    if selected_context.is_none() {
        diagnostics.push(TemplateWorkbenchDiagnostic {
            code: "template_without_consumer".to_string(),
            message: "Template-ul nu este consumat de nicio pagină; Workbench-ul trebuie să folosească un context controlat explicit.".to_string(),
        });
    }
    if render_mode == TemplateWorkbenchRenderMode::MacroScenario {
        diagnostics.push(TemplateWorkbenchDiagnostic {
            code: "controlled_macro_scenario".to_string(),
            message: "Macro-ul este randat printr-un import și un apel controlat, cu argumente demonstrative declarate ca fixture.".to_string(),
        });
    }

    Ok(TemplateWorkbenchPlan {
        schema_version: TEMPLATE_WORKBENCH_PLAN_SCHEMA_VERSION,
        project_model_revision: model.revision.clone(),
        active_template: workbench_template(active),
        direct_parent: direct_parent.map(workbench_template),
        navigator,
        consumers,
        selected_context,
        render_mode,
        render_context,
        diagnostics,
    })
}

fn render_context(
    mode: &TemplateWorkbenchRenderMode,
    selected_context: Option<&TemplateWorkbenchConsumer>,
) -> TemplateWorkbenchRenderContext {
    match mode {
        TemplateWorkbenchRenderMode::Page => TemplateWorkbenchRenderContext {
            kind: TemplateWorkbenchRenderContextKind::RealZolaPage,
            canonical_truth: true,
            label: "Pagină Zola reală".to_string(),
            explanation: "Template-ul este randat cu datele paginii/section-ului real din biblioteca Zola a reviziei curente.".to_string(),
        },
        TemplateWorkbenchRenderMode::IncludedTemplate => TemplateWorkbenchRenderContext {
            kind: TemplateWorkbenchRenderContextKind::RealZolaConsumer,
            canonical_truth: true,
            label: "Context consumator real".to_string(),
            explanation: selected_context
                .map(|context| {
                    format!(
                        "Template-ul este evaluat în contextul real al paginii «{}» ({}).",
                        context.page_title, context.page_url
                    )
                })
                .unwrap_or_else(|| {
                    "Template-ul este evaluat în contextul consumatorului Zola selectat.".to_string()
                }),
        },
        TemplateWorkbenchRenderMode::MacroScenario => TemplateWorkbenchRenderContext {
            kind: TemplateWorkbenchRenderContextKind::ControlledMacroScenario,
            canonical_truth: false,
            label: "Scenariu macro controlat".to_string(),
            explanation: "Workbench-ul importă macro-ul real și îl apelează cu argumente demonstrative; rezultatul este o previzualizare de scenariu, nu o pagină publicată.".to_string(),
        },
        TemplateWorkbenchRenderMode::OrphanTemplate => TemplateWorkbenchRenderContext {
            kind: TemplateWorkbenchRenderContextKind::ControlledTemplateFixture,
            canonical_truth: false,
            label: "Fixture izolat".to_string(),
            explanation: "Template-ul nu are consumator Zola; Workbench-ul folosește date demonstrative explicite și nu prezintă rezultatul drept pagină canonică.".to_string(),
        },
    }
}

fn consumer_for_page(
    page: &SourceGraphPage,
    active: &SourceGraphTemplate,
    templates_by_node: &HashMap<String, &SourceGraphTemplate>,
    dependencies: &HashMap<String, Vec<DependencyEdge>>,
) -> Option<TemplateWorkbenchConsumer> {
    let root_node_id = page.template_node_id.as_ref()?;
    let root_template = templates_by_node.get(root_node_id).copied()?;
    let dependency_path = dependency_path(
        root_node_id,
        &active.node_id,
        templates_by_node,
        dependencies,
    )?;

    Some(TemplateWorkbenchConsumer {
        page_id: page.id.clone(),
        page_file: page.file.clone(),
        page_title: page.title.clone(),
        page_url: page.url.clone(),
        root_template_source_id: root_template.node_id.clone(),
        root_template_file: root_template.file.clone(),
        dependency_path,
    })
}

fn dependency_path(
    start: &str,
    target: &str,
    templates_by_node: &HashMap<String, &SourceGraphTemplate>,
    dependencies: &HashMap<String, Vec<DependencyEdge>>,
) -> Option<Vec<TemplateWorkbenchDependencyStep>> {
    if start == target {
        return Some(Vec::new());
    }

    let mut queue = VecDeque::from([(start.to_string(), Vec::new())]);
    let mut visited = HashSet::from([start.to_string()]);
    while let Some((current, path)) = queue.pop_front() {
        for edge in dependencies.get(&current).into_iter().flatten() {
            if !visited.insert(edge.to.clone()) {
                continue;
            }
            let from_template = templates_by_node.get(&current).copied()?;
            let to_template = templates_by_node.get(&edge.to).copied()?;
            let mut next_path = path.clone();
            next_path.push(TemplateWorkbenchDependencyStep {
                from_source_id: current.clone(),
                from_file: from_template.file.clone(),
                to_source_id: edge.to.clone(),
                to_file: to_template.file.clone(),
                kind: edge.kind,
            });
            if edge.to == target {
                return Some(next_path);
            }
            queue.push_back((edge.to.clone(), next_path));
        }
    }
    None
}

fn dependency_edges(model: &ProjectModel) -> HashMap<String, Vec<DependencyEdge>> {
    let mut dependencies: HashMap<String, Vec<DependencyEdge>> = HashMap::new();
    for relation in &model.source_graph.relations {
        let kind = match relation.kind {
            SourceRelationKind::Extends => TemplateWorkbenchDependencyKind::Extends,
            SourceRelationKind::Includes => TemplateWorkbenchDependencyKind::Includes,
            SourceRelationKind::Imports => TemplateWorkbenchDependencyKind::Imports,
            _ => continue,
        };
        dependencies
            .entry(relation.from.clone())
            .or_default()
            .push(DependencyEdge {
                to: relation.to.clone(),
                kind,
            });
    }
    for edges in dependencies.values_mut() {
        edges.sort_by(|left, right| {
            dependency_kind_rank(left.kind)
                .cmp(&dependency_kind_rank(right.kind))
                .then(left.to.cmp(&right.to))
        });
        edges.dedup_by(|left, right| left.to == right.to && left.kind == right.kind);
    }
    dependencies
}

fn dependency_kind_rank(kind: TemplateWorkbenchDependencyKind) -> u8 {
    match kind {
        TemplateWorkbenchDependencyKind::Extends => 0,
        TemplateWorkbenchDependencyKind::Includes => 1,
        TemplateWorkbenchDependencyKind::Imports => 2,
    }
}

fn render_mode(
    active: &SourceGraphTemplate,
    selected_context: Option<&TemplateWorkbenchConsumer>,
) -> TemplateWorkbenchRenderMode {
    if active.is_partial && !active.macros.is_empty() {
        return TemplateWorkbenchRenderMode::MacroScenario;
    }
    let Some(context) = selected_context else {
        return TemplateWorkbenchRenderMode::OrphanTemplate;
    };
    if context.dependency_path.is_empty() {
        TemplateWorkbenchRenderMode::Page
    } else {
        TemplateWorkbenchRenderMode::IncludedTemplate
    }
}

fn workbench_template(template: &SourceGraphTemplate) -> TemplateWorkbenchTemplate {
    TemplateWorkbenchTemplate {
        source_id: template.node_id.clone(),
        file: template.file.clone(),
        name: template.name.clone(),
        origin: template.origin.clone(),
        theme_name: template.theme_name.clone(),
        is_partial: template.is_partial,
        defines_macros: !template.macros.is_empty(),
    }
}

fn find_template<'a>(
    templates: &'a [SourceGraphTemplate],
    requested_path: &str,
) -> Option<&'a SourceGraphTemplate> {
    let requested = normalize_project_path(requested_path);
    templates.iter().find(|template| {
        let file = normalize_project_path(&template.file);
        let name = normalize_project_path(&template.name);
        file == requested
            || name == requested
            || file
                .strip_prefix("templates/")
                .is_some_and(|relative| relative == requested)
            || file
                .split_once("/templates/")
                .is_some_and(|(_, relative)| relative == requested)
    })
}

fn normalize_project_path(value: &str) -> String {
    let mut normalized = value.trim().replace('\\', "/");
    while normalized.starts_with('/') {
        normalized.remove(0);
    }
    while normalized.contains("//") {
        normalized = normalized.replace("//", "/");
    }
    normalized
}

fn normalize_url(value: &str) -> String {
    let value = value.trim();
    if value == "/" {
        return value.to_string();
    }
    value.trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use crate::project_model::build_project_model;

    #[test]
    fn resolves_transitive_consumers_and_only_the_direct_parent() {
        let root = fixture_root("transitive");
        write_fixture(
            &root,
            "{% extends \"layout.html\" %}{% block content %}{% include \"partials/header.html\" %}<main></main>{% endblock %}",
        );
        fs::write(
            root.join("templates/layout.html"),
            "{% extends \"base.html\" %}{% block body %}{% block content %}{% endblock %}{% endblock %}",
        )
        .unwrap();
        fs::write(
            root.join("templates/base.html"),
            "<!doctype html><body>{% block body %}{% endblock %}</body>",
        )
        .unwrap();
        fs::write(
            root.join("templates/partials/header.html"),
            "<header><h1>Brand</h1></header>",
        )
        .unwrap();

        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let index = resolve_template_workbench_plan(
            &model,
            &TemplateWorkbenchPlanInput {
                template_path: "index.html".to_string(),
                preferred_page_path: None,
            },
        )
        .unwrap();
        assert_eq!(index.direct_parent.as_ref().unwrap().name, "layout.html");
        assert_eq!(index.navigator.len(), 2);
        assert!(index.navigator.iter().all(|entry| entry.expanded));
        assert_eq!(
            index.navigator[0].role,
            TemplateWorkbenchNavigatorRole::DirectParent
        );
        assert_eq!(index.navigator[0].template.name, "layout.html");
        assert!(!index.navigator[0].editable);
        assert_eq!(
            index.navigator[1].role,
            TemplateWorkbenchNavigatorRole::Active
        );
        assert_eq!(index.navigator[1].template.name, "index.html");
        assert!(index.navigator[1].editable);
        assert_eq!(index.render_mode, TemplateWorkbenchRenderMode::Page);

        let layout = resolve_template_workbench_plan(
            &model,
            &TemplateWorkbenchPlanInput {
                template_path: "layout.html".to_string(),
                preferred_page_path: None,
            },
        )
        .unwrap();
        assert_eq!(
            layout
                .navigator
                .iter()
                .map(|entry| entry.template.name.as_str())
                .collect::<Vec<_>>(),
            vec!["base.html", "layout.html"]
        );

        let header = resolve_template_workbench_plan(
            &model,
            &TemplateWorkbenchPlanInput {
                template_path: "templates/partials/header.html".to_string(),
                preferred_page_path: Some("content/_index.md".to_string()),
            },
        )
        .unwrap();
        assert!(header.direct_parent.is_none());
        assert_eq!(header.consumers.len(), 1);
        assert_eq!(header.consumers[0].dependency_path.len(), 1);
        assert_eq!(
            header.consumers[0].dependency_path[0].kind,
            TemplateWorkbenchDependencyKind::Includes
        );
        assert_eq!(
            header.render_mode,
            TemplateWorkbenchRenderMode::IncludedTemplate
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn detects_nested_includes_and_survives_dependency_cycles() {
        let root = fixture_root("nested-cycle");
        write_fixture(&root, "{% include \"partials/shell.html\" %}<main></main>");
        fs::write(
            root.join("templates/partials/shell.html"),
            "<section>{% include \"partials/cta.html\" %}</section>",
        )
        .unwrap();
        fs::write(
            root.join("templates/partials/cta.html"),
            "<aside>{% include \"partials/shell.html\" %}</aside>",
        )
        .unwrap();

        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let plan = resolve_template_workbench_plan(
            &model,
            &TemplateWorkbenchPlanInput {
                template_path: "partials/cta.html".to_string(),
                preferred_page_path: None,
            },
        )
        .unwrap();
        assert_eq!(plan.consumers.len(), 1);
        assert_eq!(plan.consumers[0].dependency_path.len(), 2);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reports_orphan_and_macro_render_modes_explicitly() {
        let root = fixture_root("orphan-macro");
        write_fixture(&root, "<main></main>");
        fs::write(
            root.join("templates/partials/card.html"),
            "<article>Orphan</article>",
        )
        .unwrap();
        fs::write(
            root.join("templates/partials/macros.html"),
            "{% macro card(title) %}<article>{{ title }}</article>{% endmacro %}",
        )
        .unwrap();

        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let orphan = resolve_template_workbench_plan(
            &model,
            &TemplateWorkbenchPlanInput {
                template_path: "partials/card.html".to_string(),
                preferred_page_path: None,
            },
        )
        .unwrap();
        assert_eq!(
            orphan.render_mode,
            TemplateWorkbenchRenderMode::OrphanTemplate
        );

        let macros = resolve_template_workbench_plan(
            &model,
            &TemplateWorkbenchPlanInput {
                template_path: "partials/macros.html".to_string(),
                preferred_page_path: None,
            },
        )
        .unwrap();
        assert_eq!(
            macros.render_mode,
            TemplateWorkbenchRenderMode::MacroScenario
        );

        fs::remove_dir_all(root).unwrap();
    }

    fn fixture_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "pana-template-workbench-{name}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates/partials")).unwrap();
        fs::write(
            root.join("zola.toml"),
            "base_url = \"http://example.test\"\n",
        )
        .unwrap();
        root
    }

    fn write_fixture(root: &PathBuf, index_template: &str) {
        fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = \"Acasă\"\ntemplate = \"index.html\"\n+++\n",
        )
        .unwrap();
        fs::write(root.join("templates/index.html"), index_template).unwrap();
    }
}
