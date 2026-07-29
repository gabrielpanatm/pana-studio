use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use zola_config::Config;
use zola_utils::slugs::slugify_paths;

use crate::localization::LocalizedDiagnostic;

use super::model::{SourceGraph, SourceGraphPage, SourceGraphTemplate, SourceOrigin};

pub const TAXONOMY_CATALOG_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaxonomyCatalogDiagnosticSeverity {
    Warning,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxonomyCatalogDiagnostic {
    pub code: String,
    pub severity: TaxonomyCatalogDiagnosticSeverity,
    pub diagnostic: LocalizedDiagnostic,
    pub file: Option<String>,
    pub taxonomy_name: Option<String>,
    pub term: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxonomyCatalogPageUsage {
    pub file: String,
    pub title: String,
    pub url: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxonomyCatalogTemplate {
    pub logical_name: String,
    pub file: Option<String>,
    pub origin: Option<SourceOrigin>,
    pub theme_name: Option<String>,
    pub fallback: bool,
    pub missing: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxonomyCatalogTerm {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub slug: String,
    pub path: String,
    pub permalink: String,
    pub pages: Vec<TaxonomyCatalogPageUsage>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxonomyCatalogCapabilities {
    pub can_edit_definition: bool,
    pub can_delete_definition: bool,
    pub can_assign_terms: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxonomyCatalogEntry {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub language: String,
    pub declared: bool,
    pub render: bool,
    pub feed: bool,
    pub paginate_by: Option<usize>,
    pub paginate_path: Option<String>,
    pub path: String,
    pub permalink: String,
    pub terms: Vec<TaxonomyCatalogTerm>,
    pub pages: Vec<TaxonomyCatalogPageUsage>,
    pub list_template: TaxonomyCatalogTemplate,
    pub term_template: TaxonomyCatalogTemplate,
    pub capabilities: TaxonomyCatalogCapabilities,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxonomyCatalogSnapshot {
    pub schema_version: u32,
    pub config_path: String,
    pub taxonomy_root: Option<String>,
    pub default_language: String,
    pub slugify_strategy: String,
    pub entries: Vec<TaxonomyCatalogEntry>,
    pub diagnostics: Vec<TaxonomyCatalogDiagnostic>,
}

#[derive(Default)]
struct TermAccumulator {
    names: BTreeSet<String>,
    pages: BTreeMap<String, TaxonomyCatalogPageUsage>,
}

struct EntryAccumulator {
    entry: TaxonomyCatalogEntry,
    terms: BTreeMap<String, TermAccumulator>,
    pages: BTreeMap<String, TaxonomyCatalogPageUsage>,
}

pub fn build_taxonomy_catalog(
    graph: &SourceGraph,
    config_path: &str,
    config_source: &str,
) -> TaxonomyCatalogSnapshot {
    let config = match Config::parse(config_source) {
        Ok(config) => config,
        Err(error) => {
            return TaxonomyCatalogSnapshot {
                schema_version: TAXONOMY_CATALOG_SCHEMA_VERSION,
                config_path: config_path.to_string(),
                taxonomy_root: None,
                default_language: "en".to_string(),
                slugify_strategy: "on".to_string(),
                entries: Vec::new(),
                diagnostics: vec![TaxonomyCatalogDiagnostic {
                    code: "taxonomy_config_invalid".to_string(),
                    severity: TaxonomyCatalogDiagnosticSeverity::Error,
                    diagnostic: LocalizedDiagnostic::new("taxonomies-diagnostic-config-invalid")
                        .with_argument("details", error.to_string()),
                    file: Some(config_path.to_string()),
                    taxonomy_name: None,
                    term: None,
                }],
            };
        }
    };

    let mut diagnostics = Vec::new();
    let mut entries = BTreeMap::<(String, String), EntryAccumulator>::new();
    let mut definition_slugs = BTreeMap::<(String, String), Vec<String>>::new();
    for (language, options) in &config.languages {
        for definition in &options.taxonomies {
            definition_slugs
                .entry((language.clone(), definition.slug.clone()))
                .or_default()
                .push(definition.name.clone());
            let key = (language.clone(), definition.name.clone());
            entries.insert(
                key,
                EntryAccumulator {
                    entry: catalog_entry(
                        graph,
                        &config,
                        language,
                        &definition.name,
                        &definition.slug,
                        true,
                        definition.render,
                        definition.feed,
                        definition.paginate_by,
                        definition.paginate_path.clone(),
                    ),
                    terms: BTreeMap::new(),
                    pages: BTreeMap::new(),
                },
            );
        }
    }
    for ((language, slug), mut names) in definition_slugs {
        names.sort();
        names.dedup();
        if names.len() > 1 {
            diagnostics.push(TaxonomyCatalogDiagnostic {
                code: "taxonomy_definition_slug_collision".to_string(),
                severity: TaxonomyCatalogDiagnosticSeverity::Error,
                diagnostic: LocalizedDiagnostic::new(
                    "taxonomies-diagnostic-definition-slug-collision",
                )
                .with_argument("names", names.join(", "))
                .with_argument("language", language.clone())
                .with_argument("slug", slug.clone()),
                file: Some(config_path.to_string()),
                taxonomy_name: None,
                term: None,
            });
        }
    }

    let languages = config.languages.keys().cloned().collect::<BTreeSet<_>>();
    for page in &graph.pages {
        let language = page_language(page, &languages, &config.default_language);
        for (taxonomy_name, terms) in &page.taxonomies {
            let key = (language.clone(), taxonomy_name.clone());
            let accumulator = entries.entry(key).or_insert_with(|| {
                diagnostics.push(TaxonomyCatalogDiagnostic {
                    code: "taxonomy_undeclared".to_string(),
                    severity: TaxonomyCatalogDiagnosticSeverity::Error,
                    diagnostic: LocalizedDiagnostic::new("taxonomies-diagnostic-undeclared")
                        .with_argument("path", page.file.clone())
                        .with_argument("name", taxonomy_name.clone())
                        .with_argument("language", language.clone()),
                    file: Some(page.file.clone()),
                    taxonomy_name: Some(taxonomy_name.clone()),
                    term: None,
                });
                let slug = slugify_paths(taxonomy_name, config.slugify.taxonomies);
                EntryAccumulator {
                    entry: catalog_entry(
                        graph,
                        &config,
                        &language,
                        taxonomy_name,
                        &slug,
                        false,
                        false,
                        false,
                        None,
                        None,
                    ),
                    terms: BTreeMap::new(),
                    pages: BTreeMap::new(),
                }
            });
            let page_usage = page_usage(page);
            accumulator
                .pages
                .insert(page.file.clone(), page_usage.clone());
            for term in terms
                .iter()
                .map(|term| term.trim())
                .filter(|term| !term.is_empty())
            {
                let slug = slugify_paths(term, config.slugify.taxonomies);
                let term_accumulator = accumulator.terms.entry(slug.clone()).or_default();
                term_accumulator.names.insert(term.to_string());
                term_accumulator
                    .pages
                    .insert(page.file.clone(), page_usage.clone());
            }
        }
    }

    let mut projected_entries = Vec::new();
    for ((_language, _name), mut accumulator) in entries {
        let mut terms = Vec::new();
        for (slug, term_accumulator) in accumulator.terms {
            let aliases = term_accumulator.names.into_iter().collect::<Vec<_>>();
            let name = aliases.first().cloned().unwrap_or_else(|| slug.clone());
            if aliases.len() > 1 {
                diagnostics.push(TaxonomyCatalogDiagnostic {
                    code: "taxonomy_term_slug_collision".to_string(),
                    severity: TaxonomyCatalogDiagnosticSeverity::Warning,
                    diagnostic: LocalizedDiagnostic::new(
                        "taxonomies-diagnostic-term-slug-collision",
                    )
                    .with_argument("aliases", aliases.join(", "))
                    .with_argument("slug", slug.clone()),
                    file: None,
                    taxonomy_name: Some(accumulator.entry.name.clone()),
                    term: Some(name.clone()),
                });
            }
            let path = term_path(
                &config,
                &accumulator.entry.language,
                &accumulator.entry.slug,
                &slug,
            );
            terms.push(TaxonomyCatalogTerm {
                id: format!(
                    "taxonomy-term:{}:{}:{}",
                    accumulator.entry.language, accumulator.entry.name, slug
                ),
                name,
                aliases,
                slug,
                permalink: config.make_permalink(&path),
                path,
                pages: term_accumulator.pages.into_values().collect(),
            });
        }
        terms.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
        accumulator.entry.terms = terms;
        accumulator.entry.pages = accumulator.pages.into_values().collect();
        if accumulator.entry.declared && accumulator.entry.render {
            for (kind, template) in [
                ("list", &accumulator.entry.list_template),
                ("term", &accumulator.entry.term_template),
            ] {
                if template.missing {
                    diagnostics.push(TaxonomyCatalogDiagnostic {
                        code: "taxonomy_template_missing".to_string(),
                        severity: TaxonomyCatalogDiagnosticSeverity::Error,
                        diagnostic: LocalizedDiagnostic::new(
                            "taxonomies-diagnostic-template-missing",
                        )
                        .with_argument("name", accumulator.entry.name.clone())
                        .with_argument("kind", kind),
                        file: Some(config_path.to_string()),
                        taxonomy_name: Some(accumulator.entry.name.clone()),
                        term: None,
                    });
                }
            }
        }
        projected_entries.push(accumulator.entry);
    }
    projected_entries.sort_by(|left, right| {
        left.language
            .cmp(&right.language)
            .then_with(|| left.name.cmp(&right.name))
    });
    diagnostics.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then_with(|| left.file.cmp(&right.file))
            .then_with(|| left.taxonomy_name.cmp(&right.taxonomy_name))
    });

    TaxonomyCatalogSnapshot {
        schema_version: TAXONOMY_CATALOG_SCHEMA_VERSION,
        config_path: config_path.to_string(),
        taxonomy_root: config.taxonomy_root,
        default_language: config.default_language,
        slugify_strategy: format!("{:?}", config.slugify.taxonomies).to_lowercase(),
        entries: projected_entries,
        diagnostics,
    }
}

fn catalog_entry(
    graph: &SourceGraph,
    config: &Config,
    language: &str,
    name: &str,
    slug: &str,
    declared: bool,
    render: bool,
    feed: bool,
    paginate_by: Option<usize>,
    paginate_path: Option<String>,
) -> TaxonomyCatalogEntry {
    let path = taxonomy_path(config, language, slug);
    TaxonomyCatalogEntry {
        id: format!("taxonomy:{language}:{name}"),
        name: name.to_string(),
        slug: slug.to_string(),
        language: language.to_string(),
        declared,
        render,
        feed,
        paginate_by,
        paginate_path,
        permalink: config.make_permalink(&path),
        path,
        terms: Vec::new(),
        pages: Vec::new(),
        list_template: resolve_template(graph, &format!("{name}/list.html"), "taxonomy_list.html"),
        term_template: resolve_template(
            graph,
            &format!("{name}/single.html"),
            "taxonomy_single.html",
        ),
        capabilities: TaxonomyCatalogCapabilities {
            can_edit_definition: true,
            can_delete_definition: declared,
            can_assign_terms: declared,
        },
    }
}

fn resolve_template(
    graph: &SourceGraph,
    specific_name: &str,
    fallback_name: &str,
) -> TaxonomyCatalogTemplate {
    if let Some(template) = effective_template(graph, specific_name) {
        return template_snapshot(specific_name, template, false);
    }
    if let Some(template) = effective_template(graph, fallback_name) {
        return template_snapshot(fallback_name, template, true);
    }
    TaxonomyCatalogTemplate {
        logical_name: specific_name.to_string(),
        file: None,
        origin: None,
        theme_name: None,
        fallback: false,
        missing: true,
    }
}

fn effective_template<'a>(graph: &'a SourceGraph, name: &str) -> Option<&'a SourceGraphTemplate> {
    graph
        .templates
        .iter()
        .filter(|template| template.name == name)
        .min_by_key(|template| match template.origin {
            SourceOrigin::Local => 0,
            SourceOrigin::Theme => 1,
        })
}

fn template_snapshot(
    logical_name: &str,
    template: &SourceGraphTemplate,
    fallback: bool,
) -> TaxonomyCatalogTemplate {
    TaxonomyCatalogTemplate {
        logical_name: logical_name.to_string(),
        file: Some(template.file.clone()),
        origin: Some(template.origin.clone()),
        theme_name: template.theme_name.clone(),
        fallback,
        missing: false,
    }
}

fn page_language(
    page: &SourceGraphPage,
    languages: &BTreeSet<String>,
    default_language: &str,
) -> String {
    let filename = page.file.rsplit('/').next().unwrap_or(&page.file);
    let stem = filename.strip_suffix(".md").unwrap_or(filename);
    stem.rsplit_once('.')
        .map(|(_, suffix)| suffix)
        .filter(|suffix| languages.contains(*suffix))
        .unwrap_or(default_language)
        .to_string()
}

fn page_usage(page: &SourceGraphPage) -> TaxonomyCatalogPageUsage {
    TaxonomyCatalogPageUsage {
        file: page.file.clone(),
        title: page.title.clone(),
        url: page.url.clone(),
    }
}

fn taxonomy_path(config: &Config, language: &str, taxonomy_slug: &str) -> String {
    let mut segments = Vec::new();
    if language != config.default_language {
        segments.push(language);
    }
    if let Some(root) = config
        .taxonomy_root
        .as_deref()
        .filter(|root| !root.is_empty())
    {
        segments.push(root);
    }
    segments.push(taxonomy_slug);
    format!("/{}/", segments.join("/"))
}

fn term_path(config: &Config, language: &str, taxonomy_slug: &str, term_slug: &str) -> String {
    format!(
        "{}{}/",
        taxonomy_path(config, language, taxonomy_slug),
        term_slug
    )
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use crate::source_graph::build_source_graph;

    fn fixture_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pana-taxonomy-catalog-{name}-{nonce}"))
    }

    #[test]
    fn projects_multilingual_terms_routes_templates_and_collisions() {
        let root = fixture_root("semantic");
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates/tags")).unwrap();
        let config = r#"
base_url = "https://example.com"
default_language = "en"
taxonomy_root = "blog"
taxonomies = [{ name = "tags", feed = true }]

[languages.ro]
taxonomies = [{ name = "etichete", paginate_by = 10 }]
"#;
        fs::write(root.join("zola.toml"), config).unwrap();
        fs::write(root.join("templates/tags/list.html"), "list").unwrap();
        fs::write(root.join("templates/taxonomy_single.html"), "single").unwrap();
        fs::write(
            root.join("content/one.md"),
            "+++\ntitle = \"One\"\n[taxonomies]\ntags = [\"École\"]\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("content/two.md"),
            "---\ntitle: Two\ntaxonomies:\n  tags:\n    - Ecole\n---\n",
        )
        .unwrap();
        fs::write(
            root.join("content/trei.ro.md"),
            "+++\ntitle = \"Trei\"\n[taxonomies]\netichete = [\"Rust\"]\n+++\n",
        )
        .unwrap();

        let graph = build_source_graph(&root).unwrap();
        let catalog = build_taxonomy_catalog(&graph, "zola.toml", config);
        let tags = catalog
            .entries
            .iter()
            .find(|entry| entry.name == "tags")
            .unwrap();
        assert_eq!(tags.path, "/blog/tags/");
        assert_eq!(tags.terms.len(), 1);
        assert_eq!(tags.terms[0].pages.len(), 2);
        assert_eq!(tags.list_template.logical_name, "tags/list.html");
        assert_eq!(tags.term_template.logical_name, "taxonomy_single.html");
        assert!(catalog
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "taxonomy_term_slug_collision"));
        let romanian = catalog
            .entries
            .iter()
            .find(|entry| entry.name == "etichete")
            .unwrap();
        assert_eq!(romanian.path, "/ro/blog/etichete/");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reports_invalid_config_instead_of_inventing_a_catalog() {
        let root = fixture_root("invalid");
        fs::create_dir_all(root.join("content")).unwrap();
        fs::write(root.join("zola.toml"), "taxonomies = []\n").unwrap();
        let graph = build_source_graph(&root).unwrap();
        let catalog = build_taxonomy_catalog(&graph, "zola.toml", "taxonomies = []\n");
        assert!(catalog.entries.is_empty());
        assert_eq!(catalog.diagnostics[0].code, "taxonomy_config_invalid");
        let _ = fs::remove_dir_all(root);
    }
}
