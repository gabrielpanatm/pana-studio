use std::{
    collections::{BTreeSet, HashMap, HashSet},
    path::Path,
};

use serde::{Deserialize, Serialize};

use crate::source_graph::model::{SourceGraph, SourcePageKind};

pub const LISTING_ITEM_SCHEMA_VERSION: u32 = 1;
pub const LISTING_ITEM_METADATA_PATH: &str = ".panastudio/listing-items.toml";
pub const LISTING_ITEM_TEMPLATE_DIRECTORY: &str = "templates/listing-items";

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListingItemCatalog {
    pub schema_version: u32,
    pub metadata_present: bool,
    pub items: Vec<ListingItemDefinition>,
    pub diagnostics: Vec<ListingItemDiagnostic>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListingItemContractEntry {
    pub id: String,
    pub label: String,
    pub template_name: String,
    pub model_id: String,
    pub preview_page_file: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ListingItemContract {
    schema_version: u32,
    #[serde(default)]
    items: Vec<ListingItemContractEntry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ListingItemStatus {
    Resolved,
    MissingMetadata,
    MissingTemplate,
    MissingModel,
    MissingPreviewPage,
    IncompatiblePreviewPage,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListingItemDefinition {
    pub id: String,
    pub label: String,
    pub template_name: String,
    pub file: String,
    pub model_id: Option<String>,
    pub preview_page_file: Option<String>,
    pub preview_url: Option<String>,
    pub compatible_section_paths: Vec<String>,
    pub usage_count: usize,
    pub status: ListingItemStatus,
    pub diagnostics: Vec<ListingItemDiagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListingItemDiagnostic {
    pub code: String,
    pub message: String,
    pub file: Option<String>,
    pub item_id: Option<String>,
}

pub(crate) fn build_listing_item_catalog_from_workspace_projection(
    _project_root: &Path,
    projected_sources: &HashMap<String, String>,
    deleted_sources: &HashSet<String>,
    graph: &SourceGraph,
) -> ListingItemCatalog {
    let source = if deleted_sources.contains(LISTING_ITEM_METADATA_PATH) {
        None
    } else {
        projected_sources.get(LISTING_ITEM_METADATA_PATH).cloned()
    };
    let metadata_present = source.is_some();
    let mut diagnostics = Vec::new();
    let mut entries = Vec::new();
    if let Some(source) = source {
        match toml_edit::de::from_str::<ListingItemContract>(&source) {
            Ok(contract) if contract.schema_version == LISTING_ITEM_SCHEMA_VERSION => {
                entries = contract.items;
            }
            Ok(contract) => diagnostics.push(diagnostic(
                "listing_items_schema_mismatch",
                format!(
                    "Catalogul Listing Item folosește schema {}, iar editorul cere schema {}.",
                    contract.schema_version, LISTING_ITEM_SCHEMA_VERSION
                ),
                Some(LISTING_ITEM_METADATA_PATH),
                None,
            )),
            Err(error) => diagnostics.push(diagnostic(
                "listing_items_metadata_invalid",
                format!("Metadatele Listing Item nu sunt TOML valid: {error}"),
                Some(LISTING_ITEM_METADATA_PATH),
                None,
            )),
        }
    }

    let mut seen_ids = BTreeSet::new();
    let mut seen_templates = BTreeSet::new();
    let mut items = Vec::new();
    for entry in entries {
        let normalized_template = match normalize_listing_item_template(&entry.template_name) {
            Ok(template) => template,
            Err(message) => {
                diagnostics.push(diagnostic(
                    "listing_item_template_invalid",
                    message,
                    Some(LISTING_ITEM_METADATA_PATH),
                    Some(&entry.id),
                ));
                continue;
            }
        };
        if !valid_id(&entry.id) || !seen_ids.insert(entry.id.clone()) {
            diagnostics.push(diagnostic(
                "listing_item_identity_invalid",
                format!("ID-ul Listing Item {} este invalid sau duplicat.", entry.id),
                Some(LISTING_ITEM_METADATA_PATH),
                Some(&entry.id),
            ));
            continue;
        }
        if !seen_templates.insert(normalized_template.clone()) {
            diagnostics.push(diagnostic(
                "listing_item_template_duplicate",
                format!("Template-ul {normalized_template} este declarat de mai multe ori."),
                Some(LISTING_ITEM_METADATA_PATH),
                Some(&entry.id),
            ));
            continue;
        }
        items.push(project_entry(graph, entry, normalized_template));
    }

    for template in graph
        .templates
        .iter()
        .filter(|template| template.name.starts_with("listing-items/"))
        .filter(|template| !seen_templates.contains(&template.name))
    {
        let id = template
            .name
            .trim_start_matches("listing-items/")
            .trim_end_matches(".html")
            .replace('/', "-");
        let item_diagnostic = diagnostic(
            "listing_item_metadata_missing",
            "Partialul există, dar nu are contract editorial în .panastudio/listing-items.toml."
                .to_string(),
            Some(&template.file),
            Some(&id),
        );
        diagnostics.push(item_diagnostic.clone());
        items.push(ListingItemDefinition {
            id,
            label: template
                .name
                .trim_start_matches("listing-items/")
                .trim_end_matches(".html")
                .replace(['-', '_'], " "),
            template_name: template.name.clone(),
            file: template.file.clone(),
            model_id: None,
            preview_page_file: None,
            preview_url: None,
            compatible_section_paths: Vec::new(),
            usage_count: listing_item_usage_count(graph, &template.name),
            status: ListingItemStatus::MissingMetadata,
            diagnostics: vec![item_diagnostic],
        });
    }
    items.sort_by(|left, right| left.label.cmp(&right.label).then(left.id.cmp(&right.id)));
    ListingItemCatalog {
        schema_version: LISTING_ITEM_SCHEMA_VERSION,
        metadata_present,
        items,
        diagnostics,
    }
}

pub fn serialize_listing_item_contract(
    entries: &[ListingItemContractEntry],
) -> Result<String, String> {
    let mut entries = entries.to_vec();
    entries.sort_by(|left, right| left.id.cmp(&right.id));
    toml_edit::ser::to_string_pretty(&ListingItemContract {
        schema_version: LISTING_ITEM_SCHEMA_VERSION,
        items: entries,
    })
    .map_err(|error| format!("Contractul Listing Item nu a putut fi serializat: {error}"))
}

pub fn listing_item_contract_entries(
    source: Option<&str>,
) -> Result<Vec<ListingItemContractEntry>, String> {
    let Some(source) = source else {
        return Ok(Vec::new());
    };
    let contract = toml_edit::de::from_str::<ListingItemContract>(source)
        .map_err(|error| format!("Contractul Listing Item nu este TOML valid: {error}"))?;
    if contract.schema_version != LISTING_ITEM_SCHEMA_VERSION {
        return Err(format!(
            "Contractul Listing Item folosește schema {}, nu {}.",
            contract.schema_version, LISTING_ITEM_SCHEMA_VERSION
        ));
    }
    Ok(contract.items)
}

pub fn normalize_listing_item_template(value: &str) -> Result<String, String> {
    let normalized = value.trim().replace('\\', "/");
    let normalized = normalized.strip_prefix("templates/").unwrap_or(&normalized);
    if !normalized.starts_with("listing-items/")
        || !normalized.ends_with(".html")
        || normalized
            .split('/')
            .any(|part| part.is_empty() || part == "..")
    {
        return Err(format!(
            "Listing Item-ul trebuie păstrat în templates/listing-items/*.html: {value}."
        ));
    }
    Ok(normalized.to_string())
}

fn project_entry(
    graph: &SourceGraph,
    entry: ListingItemContractEntry,
    template_name: String,
) -> ListingItemDefinition {
    let mut item_diagnostics = Vec::new();
    let template = graph
        .templates
        .iter()
        .find(|template| template.name == template_name);
    let model = graph
        .content_models
        .models
        .iter()
        .find(|model| model.id == entry.model_id);
    let preview = graph
        .pages
        .iter()
        .find(|page| page.file == entry.preview_page_file);
    let compatible_section_paths = graph
        .content_models
        .assignments
        .iter()
        .filter(|assignment| assignment.model_id == entry.model_id)
        .map(|assignment| assignment.section_path.clone())
        .collect::<Vec<_>>();
    let status = if template.is_none() {
        item_diagnostics.push(diagnostic(
            "listing_item_template_missing",
            format!("Template-ul {template_name} lipsește."),
            Some(&format!("templates/{template_name}")),
            Some(&entry.id),
        ));
        ListingItemStatus::MissingTemplate
    } else if model.is_none() {
        item_diagnostics.push(diagnostic(
            "listing_item_model_missing",
            format!("Modelul {} lipsește.", entry.model_id),
            Some(LISTING_ITEM_METADATA_PATH),
            Some(&entry.id),
        ));
        ListingItemStatus::MissingModel
    } else if preview.is_none() {
        item_diagnostics.push(diagnostic(
            "listing_item_preview_missing",
            format!("Articolul de preview {} lipsește.", entry.preview_page_file),
            Some(LISTING_ITEM_METADATA_PATH),
            Some(&entry.id),
        ));
        ListingItemStatus::MissingPreviewPage
    } else if !preview.is_some_and(|page| {
        page_model_id(graph, &page.file).as_deref() == Some(entry.model_id.as_str())
    }) {
        item_diagnostics.push(diagnostic(
            "listing_item_preview_incompatible",
            format!(
                "Articolul de preview {} nu aparține modelului {}.",
                entry.preview_page_file, entry.model_id
            ),
            Some(&entry.preview_page_file),
            Some(&entry.id),
        ));
        ListingItemStatus::IncompatiblePreviewPage
    } else {
        ListingItemStatus::Resolved
    };
    ListingItemDefinition {
        id: entry.id,
        label: entry.label,
        file: format!("templates/{template_name}"),
        template_name: template_name.clone(),
        model_id: Some(entry.model_id),
        preview_page_file: Some(entry.preview_page_file),
        preview_url: preview.map(|page| page.url.clone()),
        compatible_section_paths,
        usage_count: listing_item_usage_count(graph, &template_name),
        status,
        diagnostics: item_diagnostics,
    }
}

fn page_model_id(graph: &SourceGraph, page_file: &str) -> Option<String> {
    graph
        .content_models
        .page_bindings
        .iter()
        .find(|binding| binding.page_file == page_file)
        .map(|binding| binding.model_id.clone())
        .or_else(|| {
            let page = graph.pages.iter().find(|page| page.file == page_file)?;
            if page.page_kind != SourcePageKind::Page {
                return None;
            }
            graph
                .content_models
                .assignments
                .iter()
                .filter(|assignment| {
                    let section_directory = assignment
                        .section_path
                        .trim_start_matches("content/")
                        .trim_end_matches("_index.md")
                        .trim_end_matches('/');
                    page.file
                        .trim_start_matches("content/")
                        .starts_with(&format!("{section_directory}/"))
                })
                .max_by_key(|assignment| assignment.section_path.len())
                .map(|assignment| assignment.model_id.clone())
        })
}

fn listing_item_usage_count(graph: &SourceGraph, template_name: &str) -> usize {
    graph
        .templates
        .iter()
        .filter(|template| {
            template
                .includes
                .iter()
                .any(|include| include == template_name)
        })
        .count()
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
}

fn diagnostic(
    code: &str,
    message: String,
    file: Option<&str>,
    item_id: Option<&str>,
) -> ListingItemDiagnostic {
    ListingItemDiagnostic {
        code: code.to_string(),
        message,
        file: file.map(str::to_string),
        item_id: item_id.map(str::to_string),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_round_trip_is_stable_and_sorted() {
        let source = serialize_listing_item_contract(&[
            ListingItemContractEntry {
                id: "zeta".to_string(),
                label: "Zeta".to_string(),
                template_name: "listing-items/zeta.html".to_string(),
                model_id: "service".to_string(),
                preview_page_file: "content/services/zeta.md".to_string(),
            },
            ListingItemContractEntry {
                id: "alpha".to_string(),
                label: "Alpha".to_string(),
                template_name: "listing-items/alpha.html".to_string(),
                model_id: "service".to_string(),
                preview_page_file: "content/services/alpha.md".to_string(),
            },
        ])
        .unwrap();
        let entries = listing_item_contract_entries(Some(&source)).unwrap();
        assert_eq!(entries[0].id, "alpha");
        assert_eq!(entries[1].id, "zeta");
    }

    #[test]
    fn template_path_is_restricted_to_listing_items_directory() {
        assert!(normalize_listing_item_template("listing-items/card.html").is_ok());
        assert!(normalize_listing_item_template("partials/card.html").is_err());
        assert!(normalize_listing_item_template("listing-items/../card.html").is_err());
    }
}
