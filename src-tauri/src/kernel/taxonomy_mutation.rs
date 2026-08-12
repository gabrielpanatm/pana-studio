use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use toml_edit::{value, Array, DocumentMut, InlineTable, Item, Table, TableLike, Value};
use zola_config::Config;
use zola_utils::slugs::{slugify_paths, SlugifyStrategy};

use crate::{
    kernel::project_workspace::{
        ProjectWorkspace, ProjectWorkspaceIdentity, ProjectWorkspaceMutationReceipt,
        WorkspaceMutationMetadata, WorkspaceResourceMutation,
    },
    source_graph::{
        model::{SourceGraph, SourceGraphPage},
        taxonomy_catalog::TaxonomyCatalogSnapshot,
        zola::zola_frontmatter_range,
    },
};

pub const TAXONOMY_MUTATION_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxonomyDefinitionInput {
    pub name: String,
    pub language: String,
    #[serde(default = "default_true")]
    pub render: bool,
    #[serde(default)]
    pub feed: bool,
    pub paginate_by: Option<usize>,
    pub paginate_path: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum TaxonomyMutationOperation {
    SetTaxonomyRoot {
        taxonomy_root: Option<String>,
    },
    UpsertDefinition {
        original_name: Option<String>,
        original_language: Option<String>,
        definition: TaxonomyDefinitionInput,
    },
    SetPageTerms {
        page_file: String,
        taxonomy_name: String,
        terms: Vec<String>,
    },
    RenameTerm {
        taxonomy_name: String,
        language: String,
        old_term: String,
        new_term: String,
    },
    RemoveDefinition {
        name: String,
        language: String,
        remove_assignments: bool,
        expected_usage_count: usize,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxonomyMutationInput {
    pub operation: TaxonomyMutationOperation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxonomyMutationPlan {
    pub schema_version: u32,
    pub plan_id: String,
    pub operation: String,
    pub label: String,
    pub config_path: String,
    pub touched_files: Vec<String>,
    pub affected_pages: Vec<String>,
    pub usage_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct PlannedTaxonomyMutation {
    pub plan: TaxonomyMutationPlan,
    pub changes: Vec<WorkspaceResourceMutation>,
}

pub fn plan_taxonomy_mutation(
    graph: &SourceGraph,
    catalog: &TaxonomyCatalogSnapshot,
    source_texts: &HashMap<String, String>,
    input: &TaxonomyMutationInput,
) -> Result<PlannedTaxonomyMutation, String> {
    let config_source = source_texts.get(&catalog.config_path).ok_or_else(|| {
        format!(
            "ProjectWorkspace nu urmărește configurația {}.",
            catalog.config_path
        )
    })?;
    let config = Config::parse(config_source)
        .map_err(|error| format!("Configurația Zola este invalidă: {error}"))?;
    let mut changes = HashMap::<String, String>::new();
    let mut affected_pages = BTreeSet::new();
    let mut warnings = Vec::new();
    let (operation, label, usage_count) = match &input.operation {
        TaxonomyMutationOperation::SetTaxonomyRoot { taxonomy_root } => {
            let mut document = parse_config(config_source)?;
            set_taxonomy_root(&mut document, taxonomy_root.as_deref())?;
            stage_config_change(&mut changes, &catalog.config_path, document)?;
            (
                "set_taxonomy_root".to_string(),
                "Actualizare rădăcină taxonomii".to_string(),
                0,
            )
        }
        TaxonomyMutationOperation::UpsertDefinition {
            original_name,
            original_language,
            definition,
        } => {
            validate_definition(definition, &config)?;
            let original = match (original_name.as_deref(), original_language.as_deref()) {
                (None, None) => None,
                (Some(name), Some(language)) => Some((name, language)),
                _ => {
                    return Err(
                        "Editarea taxonomiei cere împreună originalName și originalLanguage."
                            .to_string(),
                    )
                }
            };
            if catalog.entries.iter().any(|entry| {
                entry.declared
                    && entry.name == definition.name
                    && entry.language == definition.language
                    && original
                        .map(|(name, language)| {
                            name != definition.name || language != definition.language
                        })
                        .unwrap_or(true)
            }) {
                return Err(format!(
                    "Taxonomia „{}” există deja pentru limba {}.",
                    definition.name, definition.language
                ));
            }
            let definition_slug = slugify_paths(definition.name.trim(), config.slugify.taxonomies);
            if let Some(collision) = catalog.entries.iter().find(|entry| {
                entry.declared
                    && entry.language == definition.language
                    && entry.slug == definition_slug
                    && original
                        .map(|(name, language)| entry.name != name || entry.language != language)
                        .unwrap_or(true)
            }) {
                return Err(format!(
                    "Taxonomia „{}” ar produce aceeași rută Zola ca „{}” pentru limba {}.",
                    definition.name, collision.name, definition.language
                ));
            }
            if let Some((name, language)) = original {
                if language != definition.language {
                    let usage_count = catalog
                        .entries
                        .iter()
                        .find(|entry| {
                            entry.declared && entry.name == name && entry.language == language
                        })
                        .map(|entry| entry.pages.len())
                        .unwrap_or(0);
                    if usage_count > 0 {
                        return Err(format!(
                            "Taxonomia „{}” nu poate fi mutată din limba {} în {} cât timp este atribuită în {} pagini.",
                            name, language, definition.language, usage_count
                        ));
                    }
                }
            }

            let mut document = parse_config(config_source)?;
            if let Some((name, language)) = original {
                if !remove_definition_from_config(
                    &mut document,
                    name,
                    language,
                    &config.default_language,
                )? {
                    return Err(format!(
                        "Configurația nu mai conține taxonomia „{}” pentru limba {}.",
                        name, language
                    ));
                }
            }
            insert_definition_into_config(&mut document, definition, &config.default_language)?;
            stage_config_change(&mut changes, &catalog.config_path, document)?;

            if let Some((old_name, old_language)) = original {
                if old_name != definition.name || old_language != definition.language {
                    for page in pages_for_language(graph, &config, old_language) {
                        if !page.taxonomies.contains_key(old_name) {
                            continue;
                        }
                        let source = required_source(source_texts, &page.file)?;
                        let next = rewrite_page_frontmatter(
                            source,
                            PageTaxonomyEdit::RenameTaxonomy {
                                old_name,
                                new_name: &definition.name,
                            },
                            config.slugify.taxonomies,
                        )?;
                        if next != source {
                            changes.insert(page.file.clone(), next);
                            affected_pages.insert(page.file.clone());
                        }
                    }
                }
            }
            let action = if original.is_some() { "edit" } else { "create" };
            (
                format!("{action}_definition"),
                if original.is_some() {
                    "Editare definiție taxonomie".to_string()
                } else {
                    "Creare definiție taxonomie".to_string()
                },
                affected_pages.len(),
            )
        }
        TaxonomyMutationOperation::SetPageTerms {
            page_file,
            taxonomy_name,
            terms,
        } => {
            let page = graph
                .pages
                .iter()
                .find(|page| page.file == *page_file)
                .ok_or_else(|| format!("SourceGraph nu conține pagina {page_file}."))?;
            let language = page_language(page, &config);
            let entry = require_declared(catalog, taxonomy_name, &language)?;
            let normalized_terms = normalize_terms(terms)?;
            validate_term_assignments(entry, &normalized_terms, config.slugify.taxonomies)?;
            let source = required_source(source_texts, page_file)?;
            let next = rewrite_page_frontmatter(
                source,
                PageTaxonomyEdit::SetTerms {
                    taxonomy_name,
                    terms: &normalized_terms,
                },
                config.slugify.taxonomies,
            )?;
            changes.insert(page_file.clone(), next);
            affected_pages.insert(page_file.clone());
            (
                "set_page_terms".to_string(),
                "Atribuire termeni taxonomie".to_string(),
                normalized_terms.len(),
            )
        }
        TaxonomyMutationOperation::RenameTerm {
            taxonomy_name,
            language,
            old_term,
            new_term,
        } => {
            let entry = require_declared(catalog, taxonomy_name, language)?;
            let old_term = required_term(old_term, "Termenul actual")?;
            let new_term = required_term(new_term, "Termenul nou")?;
            let old_slug = slugify_paths(old_term, config.slugify.taxonomies);
            let new_slug = slugify_paths(new_term, config.slugify.taxonomies);
            if old_slug == new_slug && !old_term.eq_ignore_ascii_case(new_term) {
                warnings.push(format!(
                    "„{}” și „{}” au aceeași rută Zola; redenumirea schimbă doar eticheta.",
                    old_term, new_term
                ));
            } else if entry
                .terms
                .iter()
                .any(|term| term.slug == new_slug && term.slug != old_slug)
            {
                return Err(format!(
                    "Termenul „{}” ar intra în coliziune cu un termen existent pe ruta „{}”.",
                    new_term, new_slug
                ));
            }
            for page in pages_for_language(graph, &config, language) {
                let Some(terms) = page.taxonomies.get(taxonomy_name) else {
                    continue;
                };
                if !terms
                    .iter()
                    .any(|term| slugify_paths(term, config.slugify.taxonomies) == old_slug)
                {
                    continue;
                }
                let source = required_source(source_texts, &page.file)?;
                let next = rewrite_page_frontmatter(
                    source,
                    PageTaxonomyEdit::RenameTerm {
                        taxonomy_name,
                        old_slug: &old_slug,
                        new_term,
                    },
                    config.slugify.taxonomies,
                )?;
                if next != source {
                    changes.insert(page.file.clone(), next);
                    affected_pages.insert(page.file.clone());
                }
            }
            (
                "rename_term".to_string(),
                "Redenumire termen taxonomie".to_string(),
                affected_pages.len(),
            )
        }
        TaxonomyMutationOperation::RemoveDefinition {
            name,
            language,
            remove_assignments,
            expected_usage_count,
        } => {
            let entry = require_declared(catalog, name, language)?;
            if entry.pages.len() != *expected_usage_count {
                return Err(format!(
                    "Impactul taxonomiei s-a schimbat: UI a confirmat {} pagini, Rust găsește {}.",
                    expected_usage_count,
                    entry.pages.len()
                ));
            }
            let mut document = parse_config(config_source)?;
            if !remove_definition_from_config(
                &mut document,
                name,
                language,
                &config.default_language,
            )? {
                return Err(format!("Taxonomia „{name}” nu mai există în configurație."));
            }
            stage_config_change(&mut changes, &catalog.config_path, document)?;
            if *remove_assignments {
                for page in pages_for_language(graph, &config, language) {
                    if !page.taxonomies.contains_key(name) {
                        continue;
                    }
                    let source = required_source(source_texts, &page.file)?;
                    let next = rewrite_page_frontmatter(
                        source,
                        PageTaxonomyEdit::RemoveTaxonomy {
                            taxonomy_name: name,
                        },
                        config.slugify.taxonomies,
                    )?;
                    if next != source {
                        changes.insert(page.file.clone(), next);
                        affected_pages.insert(page.file.clone());
                    }
                }
            } else if !entry.pages.is_empty() {
                warnings.push(format!(
                    "{} pagini vor păstra o taxonomie nedeclarată.",
                    entry.pages.len()
                ));
            }
            (
                "remove_definition".to_string(),
                "Eliminare definiție taxonomie".to_string(),
                entry.pages.len(),
            )
        }
    };

    let mut changes = changes
        .into_iter()
        .filter_map(|(relative_path, contents)| {
            (source_texts.get(&relative_path) != Some(&contents)).then_some(
                WorkspaceResourceMutation {
                    relative_path,
                    contents,
                    create_only: false,
                },
            )
        })
        .collect::<Vec<_>>();
    changes.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let touched_files = changes
        .iter()
        .map(|change| change.relative_path.clone())
        .collect::<Vec<_>>();
    let affected_pages = affected_pages.into_iter().collect::<Vec<_>>();
    let plan_id = plan_id(input, &changes);
    Ok(PlannedTaxonomyMutation {
        plan: TaxonomyMutationPlan {
            schema_version: TAXONOMY_MUTATION_SCHEMA_VERSION,
            plan_id,
            operation,
            label,
            config_path: catalog.config_path.clone(),
            touched_files,
            affected_pages,
            usage_count,
            warnings,
        },
        changes,
    })
}

pub fn stage_taxonomy_mutation(
    workspace: &mut ProjectWorkspace,
    planned: PlannedTaxonomyMutation,
    now_ms: u128,
) -> Result<(TaxonomyMutationPlan, ProjectWorkspaceMutationReceipt), String> {
    let identity = ProjectWorkspaceIdentity {
        expected_project_root: workspace.session.project_root.clone(),
        expected_session_id: workspace.runtime_session_id(),
        expected_revision: workspace.revision,
    };
    let plan = planned.plan;
    let mutation = workspace.stage_composite_changes(
        &identity,
        WorkspaceMutationMetadata {
            label: plan.label.clone(),
            source: "taxonomies.semantic".to_string(),
            coalesce_key: None,
            transaction_id: Some(format!("taxonomy-{}", plan.plan_id)),
        },
        planned.changes,
        Vec::new(),
        None,
        now_ms,
    )?;
    Ok((plan, mutation))
}

fn default_true() -> bool {
    true
}

fn parse_config(source: &str) -> Result<DocumentMut, String> {
    source
        .parse::<DocumentMut>()
        .map_err(|error| format!("Configurația TOML nu poate fi editată lossless: {error}"))
}

fn stage_config_change(
    changes: &mut HashMap<String, String>,
    config_path: &str,
    document: DocumentMut,
) -> Result<(), String> {
    let rendered = document.to_string();
    Config::parse(&rendered).map_err(|error| {
        format!("Configurația rezultată pentru taxonomii nu este validă Zola: {error}")
    })?;
    changes.insert(config_path.to_string(), rendered);
    Ok(())
}

fn validate_definition(
    definition: &TaxonomyDefinitionInput,
    config: &Config,
) -> Result<(), String> {
    let name = definition.name.trim();
    if name.is_empty() {
        return Err("Numele taxonomiei este obligatoriu.".to_string());
    }
    if name.chars().any(char::is_control) {
        return Err("Numele taxonomiei conține caractere de control.".to_string());
    }
    if !config.languages.contains_key(definition.language.trim()) {
        return Err(format!(
            "Limba „{}” nu există în configurația Zola.",
            definition.language
        ));
    }
    if definition.paginate_by == Some(0) {
        return Err("paginateBy trebuie să fie mai mare decât zero.".to_string());
    }
    if definition
        .paginate_path
        .as_deref()
        .is_some_and(|path| path.trim().is_empty())
    {
        return Err("paginatePath nu poate fi gol când este prezent.".to_string());
    }
    Ok(())
}

fn set_taxonomy_root(document: &mut DocumentMut, root: Option<&str>) -> Result<(), String> {
    match root.map(str::trim).filter(|root| !root.is_empty()) {
        Some(root) => {
            if root
                .split('/')
                .any(|segment| segment.is_empty() || segment == "." || segment == "..")
            {
                return Err(
                    "Rădăcina taxonomiilor trebuie să fie un segment URL sigur.".to_string()
                );
            }
            document["taxonomy_root"] = value(root);
        }
        None => {
            document.as_table_mut().remove("taxonomy_root");
        }
    }
    Ok(())
}

fn insert_definition_into_config(
    document: &mut DocumentMut,
    definition: &TaxonomyDefinitionInput,
    default_language: &str,
) -> Result<(), String> {
    let table = target_taxonomy_table_mut(document, &definition.language, default_language)?;
    if !table.contains_key("taxonomies") {
        table.insert("taxonomies", value(Array::new()));
    }
    let item = table
        .get_mut("taxonomies")
        .expect("taxonomies was inserted above");
    if let Some(array) = item.as_array_mut() {
        array.push(Value::InlineTable(inline_definition(definition)));
        return Ok(());
    }
    if let Some(array) = item.as_array_of_tables_mut() {
        array.push(table_definition(definition));
        return Ok(());
    }
    Err(
        "Configurația taxonomiilor trebuie să fie o listă TOML inline sau o listă de tabele."
            .to_string(),
    )
}

fn inline_definition(definition: &TaxonomyDefinitionInput) -> InlineTable {
    let mut table = InlineTable::new();
    table.insert("name", Value::from(definition.name.trim()));
    if let Some(paginate_by) = definition.paginate_by {
        table.insert("paginate_by", Value::from(paginate_by as i64));
    }
    if let Some(path) = definition
        .paginate_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        table.insert("paginate_path", Value::from(path));
    }
    table.insert("feed", Value::from(definition.feed));
    table.insert("render", Value::from(definition.render));
    table
}

fn table_definition(definition: &TaxonomyDefinitionInput) -> Table {
    let mut table = Table::new();
    table["name"] = value(definition.name.trim());
    if let Some(paginate_by) = definition.paginate_by {
        table["paginate_by"] = value(paginate_by as i64);
    }
    if let Some(path) = definition
        .paginate_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        table["paginate_path"] = value(path);
    }
    table["feed"] = value(definition.feed);
    table["render"] = value(definition.render);
    table
}

fn remove_definition_from_config(
    document: &mut DocumentMut,
    name: &str,
    language: &str,
    default_language: &str,
) -> Result<bool, String> {
    if language == default_language && remove_definition_from_table(document.as_table_mut(), name)?
    {
        return Ok(true);
    }
    let Some(language_table) = existing_language_table_mut(document, language)? else {
        return Ok(false);
    };
    remove_definition_from_table(language_table, name)
}

fn remove_definition_from_table(table: &mut Table, name: &str) -> Result<bool, String> {
    let Some(item) = table.get_mut("taxonomies") else {
        return Ok(false);
    };
    if let Some(array) = item.as_array_mut() {
        let index = array.iter().position(|value| {
            value
                .as_inline_table()
                .and_then(|table| table.get("name"))
                .and_then(Value::as_str)
                == Some(name)
        });
        if let Some(index) = index {
            array.remove(index);
            return Ok(true);
        }
        return Ok(false);
    }
    if let Some(array) = item.as_array_of_tables_mut() {
        let index = array
            .iter()
            .position(|table| table.get("name").and_then(Item::as_str) == Some(name));
        if let Some(index) = index {
            array.remove(index);
            return Ok(true);
        }
        return Ok(false);
    }
    Err(
        "Configurația taxonomiilor trebuie să fie o listă TOML inline sau o listă de tabele."
            .to_string(),
    )
}

fn target_taxonomy_table_mut<'a>(
    document: &'a mut DocumentMut,
    language: &str,
    default_language: &str,
) -> Result<&'a mut Table, String> {
    if language == default_language {
        let use_root = document.as_table().contains_key("taxonomies")
            || !language_has_taxonomies(document, language)?;
        if use_root {
            return Ok(document.as_table_mut());
        }
    }
    language_table_mut(document, language)
}

fn language_has_taxonomies(document: &DocumentMut, language: &str) -> Result<bool, String> {
    let Some(languages) = document.get("languages") else {
        return Ok(false);
    };
    let languages = languages
        .as_table()
        .ok_or_else(|| "Secțiunea languages din config trebuie să fie un tabel.".to_string())?;
    let Some(language) = languages.get(language) else {
        return Ok(false);
    };
    let language = language
        .as_table()
        .ok_or_else(|| "Configurația limbii trebuie să fie un tabel.".to_string())?;
    Ok(language.contains_key("taxonomies"))
}

fn language_table_mut<'a>(
    document: &'a mut DocumentMut,
    language: &str,
) -> Result<&'a mut Table, String> {
    if !document.as_table().contains_key("languages") {
        document
            .as_table_mut()
            .insert("languages", Item::Table(Table::new()));
    }
    let languages = document
        .get_mut("languages")
        .and_then(Item::as_table_mut)
        .ok_or_else(|| "Secțiunea languages din config trebuie să fie un tabel.".to_string())?;
    if !languages.contains_key(language) {
        languages.insert(language, Item::Table(Table::new()));
    }
    languages
        .get_mut(language)
        .and_then(Item::as_table_mut)
        .ok_or_else(|| "Configurația limbii trebuie să fie un tabel.".to_string())
}

fn existing_language_table_mut<'a>(
    document: &'a mut DocumentMut,
    language: &str,
) -> Result<Option<&'a mut Table>, String> {
    let Some(languages) = document.get_mut("languages") else {
        return Ok(None);
    };
    let languages = languages
        .as_table_mut()
        .ok_or_else(|| "Secțiunea languages din config trebuie să fie un tabel.".to_string())?;
    let Some(language) = languages.get_mut(language) else {
        return Ok(None);
    };
    language
        .as_table_mut()
        .map(Some)
        .ok_or_else(|| "Configurația limbii trebuie să fie un tabel.".to_string())
}

enum PageTaxonomyEdit<'a> {
    SetTerms {
        taxonomy_name: &'a str,
        terms: &'a [String],
    },
    RenameTaxonomy {
        old_name: &'a str,
        new_name: &'a str,
    },
    RenameTerm {
        taxonomy_name: &'a str,
        old_slug: &'a str,
        new_term: &'a str,
    },
    RemoveTaxonomy {
        taxonomy_name: &'a str,
    },
}

fn rewrite_page_frontmatter(
    source: &str,
    edit: PageTaxonomyEdit<'_>,
    slugify_strategy: SlugifyStrategy,
) -> Result<String, String> {
    let (start, end) = zola_frontmatter_range(source)
        .ok_or_else(|| "Pagina nu are frontmatter Zola delimitat valid.".to_string())?;
    let frontmatter = &source[start..end];
    let is_toml = source.trim_start_matches('\u{feff}').starts_with("+++");
    let rendered = if is_toml {
        rewrite_toml_frontmatter(frontmatter, edit, slugify_strategy)?
    } else {
        rewrite_yaml_frontmatter(frontmatter, edit, slugify_strategy)?
    };
    let mut next = source.to_string();
    next.replace_range(start..end, &with_leading_newline(rendered));
    Ok(next)
}

fn rewrite_toml_frontmatter(
    frontmatter: &str,
    edit: PageTaxonomyEdit<'_>,
    slugify_strategy: SlugifyStrategy,
) -> Result<String, String> {
    let mut document = frontmatter
        .parse::<DocumentMut>()
        .map_err(|error| format!("Frontmatter TOML invalid: {error}"))?;
    if !document.as_table().contains_key("taxonomies") {
        document
            .as_table_mut()
            .insert("taxonomies", Item::Table(Table::new()));
    }
    let table = document
        .get_mut("taxonomies")
        .and_then(Item::as_table_like_mut)
        .ok_or_else(|| "Câmpul taxonomies trebuie să fie un tabel TOML.".to_string())?;
    apply_toml_page_edit(table, edit, slugify_strategy)?;
    Ok(document.to_string())
}

fn apply_toml_page_edit(
    table: &mut dyn TableLike,
    edit: PageTaxonomyEdit<'_>,
    slugify_strategy: SlugifyStrategy,
) -> Result<(), String> {
    match edit {
        PageTaxonomyEdit::SetTerms {
            taxonomy_name,
            terms,
        } => {
            let mut array = Array::new();
            for term in terms {
                array.push(term.as_str());
            }
            table.insert(taxonomy_name, value(array));
        }
        PageTaxonomyEdit::RenameTaxonomy { old_name, new_name } => {
            if old_name != new_name && table.contains_key(new_name) {
                return Err(format!(
                    "Pagina conține deja taxonomia țintă „{new_name}”; redenumirea ar pierde date."
                ));
            }
            if let Some(item) = table.remove(old_name) {
                table.insert(new_name, item);
            }
        }
        PageTaxonomyEdit::RenameTerm {
            taxonomy_name,
            old_slug,
            new_term,
        } => {
            let array = table
                .get_mut(taxonomy_name)
                .and_then(Item::as_array_mut)
                .ok_or_else(|| format!("Taxonomia „{taxonomy_name}” nu este o listă TOML."))?;
            for index in 0..array.len() {
                let replace = array
                    .get(index)
                    .and_then(Value::as_str)
                    .is_some_and(|term| slugify_paths(term, slugify_strategy) == old_slug);
                if replace {
                    array.replace(index, new_term);
                }
            }
            deduplicate_toml_array(array, slugify_strategy);
        }
        PageTaxonomyEdit::RemoveTaxonomy { taxonomy_name } => {
            table.remove(taxonomy_name);
        }
    }
    Ok(())
}

fn rewrite_yaml_frontmatter(
    frontmatter: &str,
    edit: PageTaxonomyEdit<'_>,
    slugify_strategy: SlugifyStrategy,
) -> Result<String, String> {
    let mut root = serde_yaml::from_str::<serde_yaml::Value>(frontmatter)
        .map_err(|error| format!("Frontmatter YAML invalid: {error}"))?;
    let mapping = root
        .as_mapping_mut()
        .ok_or_else(|| "Frontmatter YAML trebuie să fie un obiect.".to_string())?;
    let taxonomies_key = serde_yaml::Value::String("taxonomies".to_string());
    if !mapping.contains_key(&taxonomies_key) {
        mapping.insert(
            taxonomies_key.clone(),
            serde_yaml::Value::Mapping(Default::default()),
        );
    }
    let taxonomies = mapping
        .get_mut(&taxonomies_key)
        .and_then(serde_yaml::Value::as_mapping_mut)
        .ok_or_else(|| "Câmpul taxonomies trebuie să fie un obiect YAML.".to_string())?;
    apply_yaml_page_edit(taxonomies, edit, slugify_strategy)?;
    serde_yaml::to_string(&root)
        .map(|rendered| rendered.trim_start_matches("---\n").to_string())
        .map_err(|error| format!("Frontmatter YAML nu poate fi serializat: {error}"))
}

fn apply_yaml_page_edit(
    table: &mut serde_yaml::Mapping,
    edit: PageTaxonomyEdit<'_>,
    slugify_strategy: SlugifyStrategy,
) -> Result<(), String> {
    let key = |value: &str| serde_yaml::Value::String(value.to_string());
    match edit {
        PageTaxonomyEdit::SetTerms {
            taxonomy_name,
            terms,
        } => {
            table.insert(
                key(taxonomy_name),
                serde_yaml::Value::Sequence(
                    terms
                        .iter()
                        .cloned()
                        .map(serde_yaml::Value::String)
                        .collect(),
                ),
            );
        }
        PageTaxonomyEdit::RenameTaxonomy { old_name, new_name } => {
            if old_name != new_name && table.contains_key(key(new_name)) {
                return Err(format!(
                    "Pagina conține deja taxonomia țintă „{new_name}”; redenumirea ar pierde date."
                ));
            }
            if let Some(value) = table.remove(key(old_name)) {
                table.insert(key(new_name), value);
            }
        }
        PageTaxonomyEdit::RenameTerm {
            taxonomy_name,
            old_slug,
            new_term,
        } => {
            let terms = table
                .get_mut(key(taxonomy_name))
                .and_then(serde_yaml::Value::as_sequence_mut)
                .ok_or_else(|| format!("Taxonomia „{taxonomy_name}” nu este o listă YAML."))?;
            for term in terms.iter_mut() {
                let replace = term
                    .as_str()
                    .is_some_and(|term| slugify_paths(term, slugify_strategy) == old_slug);
                if replace {
                    *term = serde_yaml::Value::String(new_term.to_string());
                }
            }
            deduplicate_yaml_sequence(terms, slugify_strategy);
        }
        PageTaxonomyEdit::RemoveTaxonomy { taxonomy_name } => {
            table.remove(key(taxonomy_name));
        }
    }
    Ok(())
}

fn deduplicate_toml_array(array: &mut Array, strategy: SlugifyStrategy) {
    let mut seen = BTreeSet::new();
    let mut index = 0;
    while index < array.len() {
        let unique = array
            .get(index)
            .and_then(Value::as_str)
            .map(|term| seen.insert(slugify_paths(term, strategy)))
            .unwrap_or(true);
        if unique {
            index += 1;
        } else {
            array.remove(index);
        }
    }
}

fn deduplicate_yaml_sequence(terms: &mut Vec<serde_yaml::Value>, strategy: SlugifyStrategy) {
    let mut seen = BTreeSet::new();
    terms.retain(|term| {
        term.as_str()
            .map(|term| seen.insert(slugify_paths(term, strategy)))
            .unwrap_or(true)
    });
}

fn with_leading_newline(rendered: String) -> String {
    if rendered.starts_with('\n') {
        rendered
    } else {
        format!("\n{}", rendered.trim_end())
    }
}

fn normalize_terms(terms: &[String]) -> Result<Vec<String>, String> {
    let mut normalized = Vec::new();
    let mut seen = BTreeSet::new();
    for term in terms {
        let term = required_term(term, "Termenul")?.to_string();
        let key = term.to_lowercase();
        if seen.insert(key) {
            normalized.push(term);
        }
    }
    Ok(normalized)
}

fn validate_term_assignments(
    entry: &crate::source_graph::taxonomy_catalog::TaxonomyCatalogEntry,
    terms: &[String],
    strategy: SlugifyStrategy,
) -> Result<(), String> {
    let mut seen = HashMap::<String, &str>::new();
    for term in terms {
        let slug = slugify_paths(term, strategy);
        if let Some(previous) = seen.insert(slug.clone(), term) {
            return Err(format!(
                "Termenii „{}” și „{}” produc același slug Zola „{}”.",
                previous, term, slug
            ));
        }
        if let Some(existing) = entry.terms.iter().find(|candidate| {
            candidate.slug == slug
                && candidate.name.as_str() != term.as_str()
                && !candidate.aliases.iter().any(|alias| alias == term)
        }) {
            return Err(format!(
                "Termenul „{}” ar intra în coliziune cu „{}” pe ruta „{}”.",
                term, existing.name, slug
            ));
        }
    }
    Ok(())
}

fn required_term<'a>(term: &'a str, label: &str) -> Result<&'a str, String> {
    let term = term.trim();
    if term.is_empty() {
        Err(format!("{label} este obligatoriu."))
    } else {
        Ok(term)
    }
}

fn required_source<'a>(
    sources: &'a HashMap<String, String>,
    file: &str,
) -> Result<&'a str, String> {
    sources
        .get(file)
        .map(String::as_str)
        .ok_or_else(|| format!("ProjectWorkspace nu urmărește documentul {file}."))
}

fn require_declared<'a>(
    catalog: &'a TaxonomyCatalogSnapshot,
    name: &str,
    language: &str,
) -> Result<&'a crate::source_graph::taxonomy_catalog::TaxonomyCatalogEntry, String> {
    catalog
        .entries
        .iter()
        .find(|entry| entry.declared && entry.name == name && entry.language == language)
        .ok_or_else(|| format!("Taxonomia „{name}” nu este declarată pentru limba {language}."))
}

fn pages_for_language<'a>(
    graph: &'a SourceGraph,
    config: &'a Config,
    language: &'a str,
) -> impl Iterator<Item = &'a SourceGraphPage> + 'a {
    graph
        .pages
        .iter()
        .filter(move |page| page_language(page, config) == language)
}

fn page_language(page: &SourceGraphPage, config: &Config) -> String {
    let filename = page.file.rsplit('/').next().unwrap_or(&page.file);
    let stem = filename.strip_suffix(".md").unwrap_or(filename);
    stem.rsplit_once('.')
        .map(|(_, suffix)| suffix)
        .filter(|suffix| config.languages.contains_key(*suffix))
        .unwrap_or(&config.default_language)
        .to_string()
}

fn plan_id(input: &TaxonomyMutationInput, changes: &[WorkspaceResourceMutation]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(input).expect("TaxonomyMutationInput serialization cannot fail"),
    );
    for change in changes {
        hasher.update(change.relative_path.as_bytes());
        hasher.update([0]);
        hasher.update(change.contents.as_bytes());
        hasher.update([0xff]);
    }
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, fs, path::Path};

    use crate::{
        js::PageJsDraftStore,
        kernel::{
            file_buffer_store::{
                hash_text, FileBufferBaseline, FileBufferEntry, FileBufferStore,
                FileBufferStoreLimits, TextBufferLanguage, TextBufferRole,
            },
            project_session::{
                ProjectRootFingerprint, ProjectSessionScanSummary, ProjectSessionSnapshot,
            },
            project_workspace::WorkspaceHistoryDirection,
        },
        project::{read_project_disk_manifest, AcceptedProjectDiskManifest},
        source_graph::{build_source_graph_from_workspace_projection, build_taxonomy_catalog},
    };

    use super::*;

    #[test]
    fn toml_frontmatter_rewrite_preserves_body_and_unknown_fields() {
        let source = "+++\ntitle = \"Unu\"\nextra.keep = \"da\"\n[taxonomies]\ntags = [\"Rust\"]\n+++\n\nCorp\n";
        let next = rewrite_page_frontmatter(
            source,
            PageTaxonomyEdit::SetTerms {
                taxonomy_name: "authors",
                terms: &["Gabriel".to_string()],
            },
            SlugifyStrategy::On,
        )
        .unwrap();
        assert!(next.contains("extra.keep = \"da\""));
        assert!(next.contains("authors = [\"Gabriel\"]"));
        assert!(next.ends_with("\n\nCorp\n"));
    }

    #[test]
    fn yaml_frontmatter_rewrite_keeps_unknown_fields_and_body() {
        let source =
            "---\ntitle: Unu\nextra:\n  keep: da\ntaxonomies:\n  tags: [Rust]\n---\nCorp\n";
        let next = rewrite_page_frontmatter(
            source,
            PageTaxonomyEdit::RenameTerm {
                taxonomy_name: "tags",
                old_slug: "rust",
                new_term: "Rust lang",
            },
            SlugifyStrategy::On,
        )
        .unwrap();
        assert!(next.contains("keep: da"));
        assert!(next.contains("Rust lang"));
        assert!(next.ends_with("---\nCorp\n"));
    }

    #[test]
    fn config_edit_preserves_unrelated_content() {
        let source = "base_url = \"https://example.com\"\ntitle = \"Site\"\ntaxonomies = [{ name = \"tags\" }]\n";
        let mut document = parse_config(source).unwrap();
        assert!(remove_definition_from_config(&mut document, "tags", "en", "en").unwrap());
        insert_definition_into_config(
            &mut document,
            &TaxonomyDefinitionInput {
                name: "topics".to_string(),
                language: "en".to_string(),
                render: true,
                feed: false,
                paginate_by: Some(10),
                paginate_path: Some("pagina".to_string()),
            },
            "en",
        )
        .unwrap();
        let next = document.to_string();
        assert!(next.contains("title = \"Site\""));
        assert!(next.contains("name = \"topics\""));
        assert!(next.contains("paginate_by = 10"));
    }

    #[test]
    fn config_edit_supports_array_of_tables_representation() {
        let source =
            "base_url = \"https://example.com\"\n\n[[taxonomies]]\nname = \"tags\"\nrender = true\n";
        let mut document = parse_config(source).unwrap();
        assert!(remove_definition_from_config(&mut document, "tags", "en", "en").unwrap());
        insert_definition_into_config(
            &mut document,
            &TaxonomyDefinitionInput {
                name: "topics".to_string(),
                language: "en".to_string(),
                render: true,
                feed: false,
                paginate_by: None,
                paginate_path: None,
            },
            "en",
        )
        .unwrap();
        let next = document.to_string();
        assert!(next.contains("[[taxonomies]]"));
        assert!(next.contains("name = \"topics\""));
        Config::parse(&next).unwrap();
    }

    #[test]
    fn multi_file_taxonomy_rename_is_one_undoable_workspace_entry() {
        let root = test_root("atomic-undo");
        let config = "base_url = \"https://example.test\"\ntaxonomies = [{ name = \"tags\" }]\n";
        let page = "+++\ntitle = \"Unu\"\n[taxonomies]\ntags = [\"Rust\"]\n+++\n\nCorp\n";
        let mut workspace = test_workspace(
            &root,
            HashMap::from([
                ("zola.toml".to_string(), config.to_string()),
                ("content/unu.md".to_string(), page.to_string()),
            ]),
        );
        let projection = workspace.capture_projection_snapshot().unwrap();
        let graph = build_source_graph_from_workspace_projection(&root, &projection).unwrap();
        let catalog = build_taxonomy_catalog(&graph, "zola.toml", config);
        let planned = plan_taxonomy_mutation(
            &graph,
            &catalog,
            &projection.source_texts,
            &TaxonomyMutationInput {
                operation: TaxonomyMutationOperation::UpsertDefinition {
                    original_name: Some("tags".to_string()),
                    original_language: Some("en".to_string()),
                    definition: TaxonomyDefinitionInput {
                        name: "topics".to_string(),
                        language: "en".to_string(),
                        render: true,
                        feed: false,
                        paginate_by: None,
                        paginate_path: None,
                    },
                },
            },
        )
        .unwrap();

        let (plan, receipt) = stage_taxonomy_mutation(&mut workspace, planned, 2).unwrap();
        assert_eq!(plan.touched_files.len(), 2);
        assert_eq!(receipt.history.undo_count, 1);
        assert!(workspace
            .documents
            .text_for("zola.toml")
            .unwrap()
            .contains("name = \"topics\""));
        assert!(workspace
            .documents
            .text_for("content/unu.md")
            .unwrap()
            .contains("topics = [\"Rust\"]"));

        let undo = workspace.undo(&current_identity(&workspace), 3).unwrap();
        assert!(matches!(undo.direction, WorkspaceHistoryDirection::Undo));
        assert_eq!(
            workspace.documents.text_for("zola.toml").as_deref(),
            Some(config)
        );
        assert_eq!(
            workspace.documents.text_for("content/unu.md").as_deref(),
            Some(page)
        );

        let redo = workspace.redo(&current_identity(&workspace), 4).unwrap();
        assert!(matches!(redo.direction, WorkspaceHistoryDirection::Redo));
        assert!(workspace
            .documents
            .text_for("zola.toml")
            .unwrap()
            .contains("name = \"topics\""));
        assert!(workspace
            .documents
            .text_for("content/unu.md")
            .unwrap()
            .contains("topics = [\"Rust\"]"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mutation_rejects_route_collisions_and_language_moves_with_usage() {
        let root = test_root("semantic-guards");
        let config = r#"base_url = "https://example.test"
taxonomies = [{ name = "tags" }]

[languages.fr]
generate_feeds = false
"#;
        let page = "+++\ntitle = \"Unu\"\n[taxonomies]\ntags = [\"Foo Bar\"]\n+++\n";
        let workspace = test_workspace(
            &root,
            HashMap::from([
                ("zola.toml".to_string(), config.to_string()),
                ("content/unu.md".to_string(), page.to_string()),
            ]),
        );
        let projection = workspace.capture_projection_snapshot().unwrap();
        let graph = build_source_graph_from_workspace_projection(&root, &projection).unwrap();
        let catalog = build_taxonomy_catalog(&graph, "zola.toml", config);

        let definition_collision = plan_taxonomy_mutation(
            &graph,
            &catalog,
            &projection.source_texts,
            &TaxonomyMutationInput {
                operation: TaxonomyMutationOperation::UpsertDefinition {
                    original_name: None,
                    original_language: None,
                    definition: TaxonomyDefinitionInput {
                        name: "Tags".to_string(),
                        language: "en".to_string(),
                        render: true,
                        feed: false,
                        paginate_by: None,
                        paginate_path: None,
                    },
                },
            },
        )
        .unwrap_err();
        assert!(definition_collision.contains("aceeași rută Zola"));

        let language_move = plan_taxonomy_mutation(
            &graph,
            &catalog,
            &projection.source_texts,
            &TaxonomyMutationInput {
                operation: TaxonomyMutationOperation::UpsertDefinition {
                    original_name: Some("tags".to_string()),
                    original_language: Some("en".to_string()),
                    definition: TaxonomyDefinitionInput {
                        name: "tags".to_string(),
                        language: "fr".to_string(),
                        render: true,
                        feed: false,
                        paginate_by: None,
                        paginate_path: None,
                    },
                },
            },
        )
        .unwrap_err();
        assert!(language_move.contains("nu poate fi mutată"));

        let term_collision = plan_taxonomy_mutation(
            &graph,
            &catalog,
            &projection.source_texts,
            &TaxonomyMutationInput {
                operation: TaxonomyMutationOperation::SetPageTerms {
                    page_file: "content/unu.md".to_string(),
                    taxonomy_name: "tags".to_string(),
                    terms: vec!["foo-bar".to_string()],
                },
            },
        )
        .unwrap_err();
        assert!(term_collision.contains("coliziune"));
        fs::remove_dir_all(root).unwrap();
    }

    fn current_identity(workspace: &ProjectWorkspace) -> ProjectWorkspaceIdentity {
        ProjectWorkspaceIdentity {
            expected_project_root: workspace.session.project_root.clone(),
            expected_session_id: workspace.runtime_session_id(),
            expected_revision: workspace.revision,
        }
    }

    fn test_root(label: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "pana-taxonomy-mutation-{label}-{}-{}",
            std::process::id(),
            crate::kernel::observability::now_ms()
        ));
        fs::create_dir_all(root.join("content")).unwrap();
        root
    }

    fn test_workspace(root: &Path, sources: HashMap<String, String>) -> ProjectWorkspace {
        for (relative_path, source) in &sources {
            let absolute = root.join(relative_path);
            if let Some(parent) = absolute.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(absolute, source).unwrap();
        }
        let canonical = root.canonicalize().unwrap().to_string_lossy().to_string();
        let session = ProjectSessionSnapshot {
            schema_version: 1,
            id: "taxonomy-mutation-test".to_string(),
            project_root: canonical.clone(),
            zola_root: canonical.clone(),
            session_dir: root.join("session").to_string_lossy().to_string(),
            manifest_path: root.join("session.json").to_string_lossy().to_string(),
            opened_at_ms: 1,
            last_seen_at_ms: 1,
            root_fingerprint: ProjectRootFingerprint {
                canonical_path: canonical.clone(),
                modified_ms: 1,
                size: 0,
                readonly: false,
                unix_device: None,
                unix_inode: None,
            },
            scan_summary: ProjectSessionScanSummary {
                active_theme: None,
                file_count: sources.len(),
                directory_count: 1,
            },
        };
        let mut documents = FileBufferStore::for_project_session(
            &session,
            1,
            FileBufferStoreLimits {
                max_files: 32,
                max_file_bytes: 1024 * 1024,
                max_total_bytes: 4 * 1024 * 1024,
            },
        );
        let mut sorted_sources = sources.into_iter().collect::<Vec<_>>();
        sorted_sources.sort_by(|left, right| left.0.cmp(&right.0));
        for (relative_path, source) in sorted_sources {
            let (language, role) = if relative_path.starts_with("content/") {
                (TextBufferLanguage::Markdown, TextBufferRole::Page)
            } else {
                (TextBufferLanguage::Toml, TextBufferRole::Config)
            };
            documents.insert_loaded_file(FileBufferEntry {
                relative_path: relative_path.clone(),
                absolute_path: root.join(&relative_path).to_string_lossy().to_string(),
                language,
                role,
                baseline: FileBufferBaseline {
                    hash: hash_text(&source),
                    modified_ms: 1,
                    size: source.len() as u64,
                    readonly: false,
                },
                baseline_text: source,
                draft: None,
                revision: 1,
            });
        }
        let accepted = AcceptedProjectDiskManifest::new(
            session.runtime_instance_id(),
            session.project_root.clone(),
            read_project_disk_manifest(root).unwrap(),
        )
        .unwrap();
        ProjectWorkspace::new(
            session.clone(),
            accepted,
            documents,
            PageJsDraftStore::new(&session),
        )
        .unwrap()
    }
}
