#[cfg(test)]
use std::fs;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    kernel::content_models::{
        ContentFieldDefinition, ContentFieldKind, ContentModelCatalog, ContentModelDefinition,
        CustomFieldTemplateUsage,
    },
    source_graph::model::{
        SourceDataPathSegment, SourceDataValueKind, SourceGraph, SourceRange,
        SourceStructuredDocumentKind,
    },
};

pub const DYNAMIC_WIDGET_SCHEMA_VERSION: u32 = 2;
const LEGACY_DYNAMIC_WIDGET_SCHEMA_VERSION: u32 = 1;
const START_MARKER_PREFIX: &str = "{# pana:widget ";
const END_MARKER_PREFIX: &str = "{# /pana:widget ";
const MARKER_SUFFIX: &str = "#}";

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicWidgetGraph {
    pub schema_version: u32,
    pub definitions: Vec<DynamicWidgetProviderDefinition>,
    pub value_catalog: Vec<DynamicValueDefinition>,
    pub source_instances: Vec<DynamicWidgetSourceInstance>,
    pub diagnostics: Vec<DynamicWidgetDiagnostic>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DynamicWidgetProviderKind {
    DynamicField,
    Listing,
}

impl DynamicWidgetProviderKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::DynamicField => "dynamic-field",
            Self::Listing => "listing",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "dynamic-field" => Some(Self::DynamicField),
            "listing" => Some(Self::Listing),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicWidgetProviderDefinition {
    pub id: String,
    pub schema_version: u32,
    pub kind: DynamicWidgetProviderKind,
    pub label: String,
    pub description: String,
    pub capabilities: DynamicWidgetCapabilities,
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicWidgetCapabilities {
    pub can_insert: bool,
    pub can_edit_properties: bool,
    pub can_duplicate: bool,
    pub can_delete: bool,
    pub renders_multiple_instances: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DynamicWidgetResolutionStatus {
    Resolved,
    UnknownProvider,
    InvalidContract,
    Incompatible,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicWidgetSourceInstance {
    pub id: String,
    pub instance_id: String,
    pub provider_id: String,
    pub provider_kind: Option<DynamicWidgetProviderKind>,
    pub file: String,
    pub range: SourceRange,
    pub start_marker_range: SourceRange,
    pub end_marker_range: SourceRange,
    pub source_node_ids: Vec<String>,
    pub root_source_node_ids: Vec<String>,
    pub status: DynamicWidgetResolutionStatus,
    pub properties: Option<DynamicWidgetProperties>,
    pub canonical_binding_path: Option<String>,
    pub canonical_binding_expression: Option<String>,
    pub source_revision: String,
    pub diagnostics: Vec<DynamicWidgetDiagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderedDynamicWidgetInstance {
    pub id: String,
    pub source_instance_id: String,
    pub instance_id: String,
    pub provider_id: String,
    pub render_instance_id: String,
    pub route: String,
    pub source_node_id: Option<String>,
    pub parent_instance_id: Option<String>,
    pub binding_key: Option<String>,
    pub binding_path: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", content = "properties", rename_all = "camelCase")]
pub enum DynamicWidgetProperties {
    DynamicField(DynamicFieldWidgetProperties),
    Listing(ListingWidgetProperties),
}

impl DynamicWidgetProperties {
    pub fn provider_kind(&self) -> DynamicWidgetProviderKind {
        match self {
            Self::DynamicField(_) => DynamicWidgetProviderKind::DynamicField,
            Self::Listing(_) => DynamicWidgetProviderKind::Listing,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DynamicFieldScope {
    Page,
    CollectionItem,
    Section,
    Site,
    RepeaterItem,
    TaxonomyTerm,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DynamicFieldPresentation {
    Auto,
    Text,
    Heading,
    Paragraph,
    Badge,
    Date,
    Number,
    Currency,
    Percent,
    Image,
    Link,
    Button,
    TrustedContent,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DynamicFieldEmptyBehavior {
    Fallback,
    RenderEmpty,
    Hide,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DynamicValueType {
    Text,
    RichHtml,
    Date,
    Number,
    Boolean,
    Url,
    Image,
    ListObject,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum DynamicValueSource {
    Builtin { field: String },
    CustomField { model_id: String, field_id: String },
    ConfigExtra { path: Vec<String> },
    SectionExtra { path: Vec<String> },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicValueBinding {
    pub context: DynamicFieldScope,
    pub source: DynamicValueSource,
    pub value_type: DynamicValueType,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicValueFormat {
    #[serde(default)]
    pub date_format: String,
    pub decimals: Option<u8>,
    #[serde(default)]
    pub currency: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicFieldWidgetProperties {
    pub binding: DynamicValueBinding,
    pub presentation: DynamicFieldPresentation,
    pub tag: String,
    #[serde(default)]
    pub format: DynamicValueFormat,
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub suffix: String,
    #[serde(default)]
    pub fallback: String,
    #[serde(default)]
    pub label: String,
    pub empty_behavior: DynamicFieldEmptyBehavior,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicValueDefinition {
    pub id: String,
    pub group: String,
    pub label: String,
    pub description: String,
    pub contexts: Vec<DynamicFieldScope>,
    pub value_type: DynamicValueType,
    pub source: DynamicValueSource,
    pub model_id: Option<String>,
    pub compatible_presentations: Vec<DynamicFieldPresentation>,
    pub default_presentation: DynamicFieldPresentation,
    pub default_tag: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyDynamicFieldWidgetProperties {
    model_id: String,
    field_id: String,
    scope: DynamicFieldScope,
    repeater_item_path: Option<String>,
    presentation: LegacyDynamicFieldPresentation,
    tag: String,
    #[serde(default)]
    prefix: String,
    #[serde(default)]
    suffix: String,
    #[serde(default)]
    fallback: String,
    #[serde(default)]
    label: String,
    empty_behavior: DynamicFieldEmptyBehavior,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
enum LegacyDynamicFieldPresentation {
    Text,
    Image,
    Link,
    Button,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", content = "properties", rename_all = "camelCase")]
enum LegacyDynamicWidgetProperties {
    DynamicField(LegacyDynamicFieldWidgetProperties),
    Listing(ListingWidgetProperties),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ListingSortBy {
    Date,
    Updated,
    Title,
    Weight,
    Slug,
    None,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ListingSortOrder {
    Asc,
    Desc,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListingWidgetProperties {
    pub section_path: String,
    pub listing_item_id: String,
    pub listing_item_template: String,
    #[serde(default)]
    pub include_subsections: bool,
    pub sort_by: ListingSortBy,
    pub sort_order: ListingSortOrder,
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: usize,
    #[serde(default)]
    pub empty_text: String,
    #[serde(default = "default_listing_tag")]
    pub tag: String,
    #[serde(default)]
    pub class_name: String,
}

fn default_listing_tag() -> String {
    "div".to_string()
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicWidgetDiagnostic {
    pub code: String,
    pub message: String,
    pub file: Option<String>,
    pub instance_id: Option<String>,
}

struct ParsedWidgetBoundary {
    instance_id: String,
    provider_id: String,
    schema_version: u32,
    properties: Option<DynamicWidgetProperties>,
    start: usize,
    start_end: usize,
    end_start: usize,
    end: usize,
    diagnostics: Vec<DynamicWidgetDiagnostic>,
}

pub fn dynamic_widget_registry() -> Vec<DynamicWidgetProviderDefinition> {
    vec![
        DynamicWidgetProviderDefinition {
            id: DynamicWidgetProviderKind::DynamicField.id().to_string(),
            schema_version: DYNAMIC_WIDGET_SCHEMA_VERSION,
            kind: DynamicWidgetProviderKind::DynamicField,
            label: "Câmp dinamic".to_string(),
            description: "Afișează un câmp tipizat din pagina sau elementul curent.".to_string(),
            capabilities: DynamicWidgetCapabilities {
                can_insert: true,
                can_edit_properties: true,
                can_duplicate: true,
                can_delete: true,
                renders_multiple_instances: false,
            },
        },
        DynamicWidgetProviderDefinition {
            id: DynamicWidgetProviderKind::Listing.id().to_string(),
            schema_version: DYNAMIC_WIDGET_SCHEMA_VERSION,
            kind: DynamicWidgetProviderKind::Listing,
            label: "Listing".to_string(),
            description: "Randă articolele unei secțiuni printr-un Listing Item reutilizabil."
                .to_string(),
            capabilities: DynamicWidgetCapabilities {
                can_insert: true,
                can_edit_properties: true,
                can_duplicate: true,
                can_delete: true,
                renders_multiple_instances: true,
            },
        },
    ]
}

pub fn dynamic_value_catalog(source_graph: &SourceGraph) -> Vec<DynamicValueDefinition> {
    let mut definitions = Vec::new();
    let page_contexts = vec![DynamicFieldScope::Page, DynamicFieldScope::CollectionItem];
    for (field, label, description, value_type) in [
        (
            "title",
            "Titlu",
            "Titlul documentului Zola.",
            DynamicValueType::Text,
        ),
        (
            "description",
            "Descriere",
            "Descrierea documentului Zola.",
            DynamicValueType::Text,
        ),
        (
            "date",
            "Data publicării",
            "Data publicării documentului.",
            DynamicValueType::Date,
        ),
        (
            "updated",
            "Data actualizării",
            "Data ultimei actualizări.",
            DynamicValueType::Date,
        ),
        (
            "slug",
            "Slug",
            "Segmentul URL al documentului.",
            DynamicValueType::Text,
        ),
        (
            "path",
            "Cale",
            "Calea internă Zola.",
            DynamicValueType::Text,
        ),
        (
            "permalink",
            "URL permanent",
            "URL-ul absolut generat de Zola.",
            DynamicValueType::Url,
        ),
        (
            "summary",
            "Rezumat",
            "Rezumatul HTML generat de Zola.",
            DynamicValueType::RichHtml,
        ),
        (
            "content",
            "Conținut",
            "Conținutul HTML generat de Zola.",
            DynamicValueType::RichHtml,
        ),
        (
            "lang",
            "Limbă",
            "Codul limbii documentului.",
            DynamicValueType::Text,
        ),
        (
            "weight",
            "Ordine",
            "Greutatea de sortare Zola.",
            DynamicValueType::Number,
        ),
        (
            "word_count",
            "Număr de cuvinte",
            "Numărul de cuvinte calculat de Zola.",
            DynamicValueType::Number,
        ),
        (
            "reading_time",
            "Timp de citire",
            "Timpul de citire calculat de Zola.",
            DynamicValueType::Number,
        ),
    ] {
        definitions.push(value_definition(
            format!("content.{field}"),
            "Document Zola",
            label,
            description,
            page_contexts.clone(),
            value_type,
            DynamicValueSource::Builtin {
                field: field.into(),
            },
            None,
        ));
    }
    for (field, label, description, value_type) in [
        (
            "title",
            "Titlu secțiune",
            "Titlul secțiunii Zola.",
            DynamicValueType::Text,
        ),
        (
            "description",
            "Descriere secțiune",
            "Descrierea secțiunii Zola.",
            DynamicValueType::Text,
        ),
        (
            "path",
            "Cale secțiune",
            "Calea internă a secțiunii.",
            DynamicValueType::Text,
        ),
        (
            "permalink",
            "URL secțiune",
            "URL-ul absolut al secțiunii.",
            DynamicValueType::Url,
        ),
        (
            "lang",
            "Limbă secțiune",
            "Codul limbii secțiunii.",
            DynamicValueType::Text,
        ),
    ] {
        definitions.push(value_definition(
            format!("section.{field}"),
            "Secțiune Zola",
            label,
            description,
            vec![DynamicFieldScope::Section],
            value_type,
            DynamicValueSource::Builtin {
                field: field.into(),
            },
            None,
        ));
    }
    for (field, label, description, value_type) in [
        (
            "title",
            "Numele site-ului",
            "config.title din configurația Zola.",
            DynamicValueType::Text,
        ),
        (
            "description",
            "Descrierea site-ului",
            "config.description din configurația Zola.",
            DynamicValueType::Text,
        ),
        (
            "base_url",
            "URL-ul site-ului",
            "config.base_url din configurația Zola.",
            DynamicValueType::Url,
        ),
        (
            "default_language",
            "Limba implicită",
            "config.default_language din configurația Zola.",
            DynamicValueType::Text,
        ),
    ] {
        definitions.push(value_definition(
            format!("site.{field}"),
            "Site",
            label,
            description,
            vec![DynamicFieldScope::Site],
            value_type,
            DynamicValueSource::Builtin {
                field: field.into(),
            },
            None,
        ));
    }

    for model in &source_graph.content_models.models {
        collect_model_value_definitions(model, &model.fields, false, &mut definitions);
    }
    collect_config_extra_definitions(source_graph, &mut definitions);
    collect_section_extra_definitions(source_graph, &mut definitions);
    definitions.sort_by(|left, right| {
        left.group
            .cmp(&right.group)
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| left.id.cmp(&right.id))
    });
    definitions.dedup_by(|left, right| left.id == right.id);
    definitions
}

// Declarative dynamic values mirror the complete immutable definition schema.
#[allow(clippy::too_many_arguments)]
fn value_definition(
    id: String,
    group: &str,
    label: &str,
    description: &str,
    contexts: Vec<DynamicFieldScope>,
    value_type: DynamicValueType,
    source: DynamicValueSource,
    model_id: Option<String>,
) -> DynamicValueDefinition {
    let compatible_presentations = compatible_presentations(value_type);
    let default_presentation = default_presentation(value_type);
    let default_tag = default_tag(default_presentation).to_string();
    DynamicValueDefinition {
        id,
        group: group.to_string(),
        label: label.to_string(),
        description: description.to_string(),
        contexts,
        value_type,
        source,
        model_id,
        compatible_presentations,
        default_presentation,
        default_tag,
    }
}

fn collect_model_value_definitions(
    model: &ContentModelDefinition,
    fields: &[ContentFieldDefinition],
    inside_repeater: bool,
    output: &mut Vec<DynamicValueDefinition>,
) {
    for field in fields {
        let value_type = content_field_value_type(field.kind);
        let contexts = if inside_repeater {
            vec![DynamicFieldScope::RepeaterItem]
        } else {
            vec![DynamicFieldScope::Page, DynamicFieldScope::CollectionItem]
        };
        output.push(value_definition(
            format!("custom.{}.{}", model.id, field.id),
            &format!("Câmpuri · {}", model.label),
            &field.label,
            if field.help.trim().is_empty() {
                "Câmp definit în modelul de conținut al proiectului."
            } else {
                &field.help
            },
            contexts,
            value_type,
            DynamicValueSource::CustomField {
                model_id: model.id.clone(),
                field_id: field.id.clone(),
            },
            Some(model.id.clone()),
        ));
        collect_model_value_definitions(
            model,
            &field.fields,
            inside_repeater || field.kind == ContentFieldKind::Repeater,
            output,
        );
    }
}

fn collect_config_extra_definitions(
    source_graph: &SourceGraph,
    output: &mut Vec<DynamicValueDefinition>,
) {
    for document in source_graph.structured_documents.iter().filter(|document| {
        document.kind == SourceStructuredDocumentKind::ZolaConfig && document.parse_error.is_none()
    }) {
        for node in &document.nodes {
            let Some(path) = data_key_path(&node.path) else {
                continue;
            };
            let Some(extra_path) = path.strip_prefix(&["extra".to_string()]) else {
                continue;
            };
            if extra_path.is_empty() || node.value_kind.is_none() {
                continue;
            }
            let value_type = source_data_value_type(node.value_kind.as_ref());
            let path_label = extra_path.join(" › ");
            output.push(value_definition(
                format!("site.extra.{}", stable_path_id(extra_path)),
                "Site · extra",
                &path_label,
                "Valoare din config.extra.",
                vec![DynamicFieldScope::Site],
                value_type,
                DynamicValueSource::ConfigExtra {
                    path: extra_path.to_vec(),
                },
                None,
            ));
        }
    }
}

fn collect_section_extra_definitions(
    source_graph: &SourceGraph,
    output: &mut Vec<DynamicValueDefinition>,
) {
    for page in source_graph.pages.iter().filter(|page| {
        matches!(
            page.page_kind,
            crate::source_graph::model::SourcePageKind::Section
        )
    }) {
        for node in &page.frontmatter_nodes {
            let Some(path) = data_key_path(&node.path) else {
                continue;
            };
            let Some(extra_path) = path.strip_prefix(&["extra".to_string()]) else {
                continue;
            };
            if extra_path.is_empty() || node.value_kind.is_none() {
                continue;
            }
            let value_type = source_data_value_type(node.value_kind.as_ref());
            let path_label = extra_path.join(" › ");
            output.push(value_definition(
                format!("section.extra.{}", stable_path_id(extra_path)),
                "Secțiune · extra",
                &path_label,
                "Valoare extra din frontmatter-ul secțiunii.",
                vec![DynamicFieldScope::Section],
                value_type,
                DynamicValueSource::SectionExtra {
                    path: extra_path.to_vec(),
                },
                None,
            ));
        }
    }
}

fn data_key_path(path: &[SourceDataPathSegment]) -> Option<Vec<String>> {
    path.iter()
        .map(|segment| match segment {
            SourceDataPathSegment::Key(key) => Some(key.clone()),
            SourceDataPathSegment::Index(_) => None,
        })
        .collect()
}

fn stable_path_id(path: &[String]) -> String {
    path.iter()
        .map(|segment| {
            segment
                .as_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn source_data_value_type(kind: Option<&SourceDataValueKind>) -> DynamicValueType {
    match kind {
        Some(SourceDataValueKind::Integer | SourceDataValueKind::Float) => DynamicValueType::Number,
        Some(SourceDataValueKind::Boolean) => DynamicValueType::Boolean,
        Some(SourceDataValueKind::Datetime) => DynamicValueType::Date,
        Some(
            SourceDataValueKind::Array
            | SourceDataValueKind::InlineTable
            | SourceDataValueKind::Table
            | SourceDataValueKind::ArrayOfTables,
        ) => DynamicValueType::ListObject,
        _ => DynamicValueType::Text,
    }
}

fn content_field_value_type(kind: ContentFieldKind) -> DynamicValueType {
    match kind {
        ContentFieldKind::Number => DynamicValueType::Number,
        ContentFieldKind::Boolean => DynamicValueType::Boolean,
        ContentFieldKind::Date => DynamicValueType::Date,
        ContentFieldKind::Url => DynamicValueType::Url,
        ContentFieldKind::Image => DynamicValueType::Image,
        ContentFieldKind::Group | ContentFieldKind::Repeater => DynamicValueType::ListObject,
        _ => DynamicValueType::Text,
    }
}

fn compatible_presentations(value_type: DynamicValueType) -> Vec<DynamicFieldPresentation> {
    use DynamicFieldPresentation as Presentation;
    match value_type {
        DynamicValueType::Text => vec![
            Presentation::Auto,
            Presentation::Text,
            Presentation::Heading,
            Presentation::Paragraph,
            Presentation::Badge,
        ],
        DynamicValueType::RichHtml => vec![Presentation::Auto, Presentation::TrustedContent],
        DynamicValueType::Date => vec![Presentation::Auto, Presentation::Date, Presentation::Text],
        DynamicValueType::Number => vec![
            Presentation::Auto,
            Presentation::Number,
            Presentation::Currency,
            Presentation::Percent,
            Presentation::Text,
        ],
        DynamicValueType::Boolean => {
            vec![Presentation::Auto, Presentation::Text, Presentation::Badge]
        }
        DynamicValueType::Url => vec![
            Presentation::Auto,
            Presentation::Link,
            Presentation::Button,
            Presentation::Text,
        ],
        DynamicValueType::Image => vec![Presentation::Auto, Presentation::Image],
        DynamicValueType::ListObject => Vec::new(),
    }
}

fn default_presentation(value_type: DynamicValueType) -> DynamicFieldPresentation {
    match value_type {
        DynamicValueType::RichHtml => DynamicFieldPresentation::TrustedContent,
        DynamicValueType::Date => DynamicFieldPresentation::Date,
        DynamicValueType::Number => DynamicFieldPresentation::Number,
        DynamicValueType::Url => DynamicFieldPresentation::Link,
        DynamicValueType::Image => DynamicFieldPresentation::Image,
        _ => DynamicFieldPresentation::Text,
    }
}

fn default_tag(presentation: DynamicFieldPresentation) -> &'static str {
    match presentation {
        DynamicFieldPresentation::Heading => "h2",
        DynamicFieldPresentation::Paragraph | DynamicFieldPresentation::TrustedContent => "div",
        DynamicFieldPresentation::Image => "img",
        DynamicFieldPresentation::Link | DynamicFieldPresentation::Button => "a",
        _ => "span",
    }
}

pub(crate) fn build_dynamic_widget_graph_from_workspace_projection(
    _project_root: &Path,
    projected_sources: &HashMap<String, String>,
    deleted_sources: &HashSet<String>,
    source_graph: &SourceGraph,
) -> DynamicWidgetGraph {
    let definitions = dynamic_widget_registry();
    let value_catalog = dynamic_value_catalog(source_graph);
    let mut source_instances = Vec::new();
    let mut diagnostics = Vec::new();
    let mut seen_ids = HashSet::new();

    for template in &source_graph.templates {
        if deleted_sources.contains(&template.file) {
            continue;
        }
        let source = projected_sources.get(&template.file).cloned();
        let Some(source) = source else {
            continue;
        };
        for mut boundary in parse_widget_boundaries(&source, &template.file) {
            let mut instance_diagnostics = boundary.diagnostics;
            let provider_kind = DynamicWidgetProviderKind::parse(&boundary.provider_id);
            let mut status = if provider_kind.is_some() {
                DynamicWidgetResolutionStatus::Resolved
            } else {
                instance_diagnostics.push(widget_diagnostic(
                    "dynamic_widget_unknown_provider",
                    format!(
                        "Providerul dinamic {} nu este înregistrat.",
                        boundary.provider_id
                    ),
                    &template.file,
                    &boundary.instance_id,
                ));
                DynamicWidgetResolutionStatus::UnknownProvider
            };
            if !matches!(
                boundary.schema_version,
                LEGACY_DYNAMIC_WIDGET_SCHEMA_VERSION | DYNAMIC_WIDGET_SCHEMA_VERSION
            ) {
                instance_diagnostics.push(widget_diagnostic(
                    "dynamic_widget_schema_mismatch",
                    format!(
                        "Instanța folosește schema {}, iar editorul acceptă schema {}.",
                        boundary.schema_version, DYNAMIC_WIDGET_SCHEMA_VERSION
                    ),
                    &template.file,
                    &boundary.instance_id,
                ));
                status = DynamicWidgetResolutionStatus::InvalidContract;
            }
            if !seen_ids.insert(boundary.instance_id.clone()) {
                instance_diagnostics.push(widget_diagnostic(
                    "dynamic_widget_duplicate_instance",
                    "InstanceId-ul apare de mai multe ori în proiect.".to_string(),
                    &template.file,
                    &boundary.instance_id,
                ));
                status = DynamicWidgetResolutionStatus::InvalidContract;
            }
            if boundary
                .properties
                .as_ref()
                .is_some_and(|properties| Some(properties.provider_kind()) != provider_kind)
            {
                instance_diagnostics.push(widget_diagnostic(
                    "dynamic_widget_provider_properties_mismatch",
                    "Tipul proprietăților nu corespunde providerului din marker.".to_string(),
                    &template.file,
                    &boundary.instance_id,
                ));
                status = DynamicWidgetResolutionStatus::InvalidContract;
            }

            if boundary.schema_version == LEGACY_DYNAMIC_WIDGET_SCHEMA_VERSION {
                if let Some(DynamicWidgetProperties::DynamicField(properties)) =
                    boundary.properties.as_mut()
                {
                    if let Ok(resolved) = resolve_dynamic_value(
                        &source_graph.content_models,
                        &value_catalog,
                        properties,
                        true,
                    ) {
                        properties.binding.value_type = resolved.value_type;
                    }
                }
            }

            let (canonical_binding_path, canonical_binding_expression) =
                match boundary.properties.as_ref() {
                    Some(DynamicWidgetProperties::DynamicField(properties)) => {
                        match resolve_dynamic_value(
                            &source_graph.content_models,
                            &value_catalog,
                            properties,
                            false,
                        )
                        .and_then(|value| {
                            validate_dynamic_widget_source_context(
                                &template.file,
                                properties,
                                source_graph,
                            )?;
                            Ok(value)
                        }) {
                            Ok(value) => (Some(value.canonical_path), Some(value.expression)),
                            Err(message) => {
                                instance_diagnostics.push(widget_diagnostic(
                                    "dynamic_widget_binding_incompatible",
                                    message,
                                    &template.file,
                                    &boundary.instance_id,
                                ));
                                status = DynamicWidgetResolutionStatus::Incompatible;
                                (None, None)
                            }
                        }
                    }
                    Some(DynamicWidgetProperties::Listing(properties)) => {
                        if let Err(message) = validate_listing_properties(properties, source_graph)
                        {
                            instance_diagnostics.push(widget_diagnostic(
                                "dynamic_widget_listing_incompatible",
                                message,
                                &template.file,
                                &boundary.instance_id,
                            ));
                            status = DynamicWidgetResolutionStatus::Incompatible;
                        }
                        (None, None)
                    }
                    None => {
                        status = DynamicWidgetResolutionStatus::InvalidContract;
                        (None, None)
                    }
                };

            let (source_node_ids, root_source_node_ids) = source_nodes_in_boundary(
                source_graph,
                &template.file,
                boundary.start_end,
                boundary.end_start,
            );
            let range = source_range(&source, boundary.start, boundary.end);
            let source_revision = source_revision(&source[boundary.start..boundary.end]);
            diagnostics.extend(instance_diagnostics.clone());
            source_instances.push(DynamicWidgetSourceInstance {
                id: format!("dynamic_widget_source:{}", boundary.instance_id),
                instance_id: boundary.instance_id,
                provider_id: boundary.provider_id,
                provider_kind,
                file: template.file.clone(),
                range,
                start_marker_range: source_range(&source, boundary.start, boundary.start_end),
                end_marker_range: source_range(&source, boundary.end_start, boundary.end),
                source_node_ids,
                root_source_node_ids,
                status,
                properties: boundary.properties,
                canonical_binding_path,
                canonical_binding_expression,
                source_revision,
                diagnostics: instance_diagnostics,
            });
        }
    }
    source_instances.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then_with(|| left.range.start.cmp(&right.range.start))
    });
    DynamicWidgetGraph {
        schema_version: DYNAMIC_WIDGET_SCHEMA_VERSION,
        definitions,
        value_catalog,
        source_instances,
        diagnostics,
    }
}

pub fn validate_dynamic_widget_source_context(
    template_file: &str,
    properties: &DynamicFieldWidgetProperties,
    source_graph: &SourceGraph,
) -> Result<(), String> {
    let normalized = template_file.replace('\\', "/");
    let listing_item = source_graph.listing_items.items.iter().find(|item| {
        item.file.replace('\\', "/") == normalized
            || format!("templates/{}", item.template_name.replace('\\', "/")) == normalized
    });
    let Some(listing_item) = listing_item else {
        let template_name = normalized.trim_start_matches("templates/");
        let consumers = source_graph
            .pages
            .iter()
            .filter(|page| page.resolved_template.as_deref() == Some(template_name))
            .collect::<Vec<_>>();
        if !consumers.is_empty() {
            let has_section = consumers.iter().any(|page| {
                matches!(
                    page.page_kind,
                    crate::source_graph::model::SourcePageKind::Section
                )
            });
            let has_page = consumers.iter().any(|page| {
                !matches!(
                    page.page_kind,
                    crate::source_graph::model::SourcePageKind::Section
                )
            });
            if properties.binding.context == DynamicFieldScope::Section && !has_section {
                return Err(
                    "Contextul section cere un template consumat de o secțiune Zola.".to_string(),
                );
            }
            if properties.binding.context == DynamicFieldScope::Page && !has_page {
                return Err(
                    "Contextul page nu este disponibil într-un template exclusiv de secțiune."
                        .to_string(),
                );
            }
        }
        return Ok(());
    };
    if properties.binding.context != DynamicFieldScope::CollectionItem {
        return Err(format!(
            "Listing Item-ul {} fixează contextul collectionItem.",
            listing_item.label
        ));
    }
    if let DynamicValueSource::CustomField { model_id, .. } = &properties.binding.source {
        let expected = listing_item.model_id.as_deref().ok_or_else(|| {
            format!(
                "Listing Item-ul {} nu are încă un model de conținut rezolvat.",
                listing_item.label
            )
        })?;
        if model_id != expected {
            return Err(format!(
                "Listing Item-ul {} cere modelul {}, nu {}.",
                listing_item.label, expected, model_id
            ));
        }
    }
    Ok(())
}

/// Projects typed Dynamic Field dependencies without requiring the rendered
/// widget graph. ContentModelCatalog uses this during both full and local
/// template invalidation so detach/rename planning cannot miss a widget.
pub fn project_dynamic_field_usages(
    source: &str,
    template_file: &str,
    catalog: &ContentModelCatalog,
) -> Vec<CustomFieldTemplateUsage> {
    let mut usages = parse_widget_boundaries(source, template_file)
        .into_iter()
        .filter_map(|boundary| {
            let DynamicWidgetProperties::DynamicField(properties) = boundary.properties? else {
                return None;
            };
            let DynamicValueSource::CustomField { model_id, field_id } = &properties.binding.source
            else {
                return None;
            };
            // page.extra.* is already discovered by the canonical expression
            // scanner. Item scopes need their typed marker because the local
            // variable name is contextual and deliberately not guessed.
            if properties.binding.context == DynamicFieldScope::Page {
                return None;
            }
            let path = resolve_dynamic_value(
                catalog,
                &[],
                &properties,
                boundary.schema_version == LEGACY_DYNAMIC_WIDGET_SCHEMA_VERSION,
            )
            .ok()?
            .canonical_path;
            Some(CustomFieldTemplateUsage {
                model_id: model_id.clone(),
                field_id: field_id.clone(),
                field_key: path,
                template_file: template_file.to_string(),
                expression: source[boundary.start..boundary.start_end].to_string(),
                offset: boundary.start,
            })
        })
        .collect::<Vec<_>>();
    usages.sort_by_key(|usage| usage.offset);
    usages
}

pub fn render_dynamic_widget(
    instance_id: &str,
    properties: &DynamicWidgetProperties,
    source_graph: &SourceGraph,
) -> Result<String, String> {
    validate_instance_id(instance_id)?;
    let provider = properties.provider_kind();
    let body = match properties {
        DynamicWidgetProperties::DynamicField(properties) => {
            render_dynamic_field(instance_id, properties, source_graph)?
        }
        DynamicWidgetProperties::Listing(properties) => {
            validate_listing_properties(properties, source_graph)?;
            render_listing(instance_id, properties)?
        }
    };
    let encoded = encode_properties(properties)?;
    Ok(format!(
        "{{# pana:widget schema={} provider={} instance={} props={} #}}\n{}\n{{# /pana:widget instance={} #}}",
        DYNAMIC_WIDGET_SCHEMA_VERSION,
        provider.id(),
        instance_id,
        encoded,
        body,
        instance_id
    ))
}

pub fn replace_dynamic_widget_source(
    source: &str,
    instance: &DynamicWidgetSourceInstance,
    properties: &DynamicWidgetProperties,
    source_graph: &SourceGraph,
) -> Result<String, String> {
    if instance.range.end > source.len() || instance.range.start > instance.range.end {
        return Err("Range-ul instanței dinamice nu mai aparține sursei curente.".to_string());
    }
    let live_revision = source_revision(&source[instance.range.start..instance.range.end]);
    if live_revision != instance.source_revision {
        return Err(
            "Instanța dinamică este stale și trebuie reproiectată înainte de rescriere."
                .to_string(),
        );
    }
    let replacement = render_dynamic_widget(&instance.instance_id, properties, source_graph)?;
    Ok(format!(
        "{}{}{}",
        &source[..instance.range.start],
        replacement,
        &source[instance.range.end..]
    ))
}

pub fn generate_dynamic_widget_instance_id(
    provider: DynamicWidgetProviderKind,
    seed: &str,
    existing: impl IntoIterator<Item = impl AsRef<str>>,
) -> String {
    let existing = existing
        .into_iter()
        .map(|value| value.as_ref().to_string())
        .collect::<HashSet<_>>();
    for attempt in 0..128u32 {
        let mut hasher = Sha256::new();
        hasher.update(b"pana-dynamic-widget");
        hasher.update([0]);
        hasher.update(provider.id().as_bytes());
        hasher.update([0]);
        hasher.update(seed.as_bytes());
        hasher.update([0]);
        hasher.update(attempt.to_le_bytes());
        let digest = hasher.finalize();
        let token = digest[..8]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let candidate = format!("{}-{token}", provider.id());
        if !existing.contains(&candidate) {
            return candidate;
        }
    }
    unreachable!("finite existing identities cannot exhaust 128 SHA-256 candidates")
}

fn parse_widget_boundaries(source: &str, file: &str) -> Vec<ParsedWidgetBoundary> {
    let mut output = Vec::new();
    let mut cursor = 0usize;
    while let Some(relative_start) = source[cursor..].find(START_MARKER_PREFIX) {
        let start = cursor + relative_start;
        let Some(relative_start_end) = source[start..].find(MARKER_SUFFIX) else {
            break;
        };
        let start_end = start + relative_start_end + MARKER_SUFFIX.len();
        let attributes_source =
            &source[start + START_MARKER_PREFIX.len()..start + relative_start_end];
        let attributes = parse_marker_attributes(attributes_source);
        let instance_id = attributes.get("instance").cloned().unwrap_or_default();
        let provider_id = attributes.get("provider").cloned().unwrap_or_default();
        let schema_version = attributes
            .get("schema")
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or_default();
        let mut boundary_diagnostics = Vec::new();
        if validate_instance_id(&instance_id).is_err() {
            boundary_diagnostics.push(widget_diagnostic(
                "dynamic_widget_invalid_instance_id",
                "Markerul nu conține un instanceId valid.".to_string(),
                file,
                &instance_id,
            ));
        }
        let end_needle = format!("{END_MARKER_PREFIX}instance={instance_id} {MARKER_SUFFIX}");
        let Some(relative_end_start) = source[start_end..].find(&end_needle) else {
            boundary_diagnostics.push(widget_diagnostic(
                "dynamic_widget_unclosed_boundary",
                "Instanța dinamică nu are marker final corespunzător.".to_string(),
                file,
                &instance_id,
            ));
            output.push(ParsedWidgetBoundary {
                instance_id,
                provider_id,
                schema_version,
                properties: attributes
                    .get("props")
                    .and_then(|value| decode_properties(value, schema_version).ok()),
                start,
                start_end,
                end_start: start_end,
                end: start_end,
                diagnostics: boundary_diagnostics,
            });
            cursor = start_end;
            continue;
        };
        let end_start = start_end + relative_end_start;
        let end = end_start + end_needle.len();
        let properties = match attributes.get("props") {
            Some(value) => match decode_properties(value, schema_version) {
                Ok(properties) => Some(properties),
                Err(message) => {
                    boundary_diagnostics.push(widget_diagnostic(
                        "dynamic_widget_invalid_properties",
                        message,
                        file,
                        &instance_id,
                    ));
                    None
                }
            },
            None => {
                boundary_diagnostics.push(widget_diagnostic(
                    "dynamic_widget_missing_properties",
                    "Markerul dinamic nu conține proprietăți tipizate.".to_string(),
                    file,
                    &instance_id,
                ));
                None
            }
        };
        output.push(ParsedWidgetBoundary {
            instance_id,
            provider_id,
            schema_version,
            properties,
            start,
            start_end,
            end_start,
            end,
            diagnostics: boundary_diagnostics,
        });
        cursor = end;
    }
    output
}

fn parse_marker_attributes(source: &str) -> BTreeMap<String, String> {
    source
        .split_whitespace()
        .filter_map(|token| token.split_once('='))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

fn encode_properties(properties: &DynamicWidgetProperties) -> Result<String, String> {
    let json = serde_json::to_vec(properties)
        .map_err(|error| format!("Proprietățile dinamice nu au putut fi serializate: {error}"))?;
    Ok(json
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>())
}

fn decode_properties(
    encoded: &str,
    schema_version: u32,
) -> Result<DynamicWidgetProperties, String> {
    if encoded.is_empty() || !encoded.len().is_multiple_of(2) || encoded.len() > 64 * 1024 {
        return Err("Payloadul proprietăților dinamice este invalid.".to_string());
    }
    let mut bytes = Vec::with_capacity(encoded.len() / 2);
    for pair in encoded.as_bytes().chunks_exact(2) {
        let pair = std::str::from_utf8(pair)
            .map_err(|_| "Payloadul proprietăților nu este UTF-8 hex.".to_string())?;
        bytes.push(
            u8::from_str_radix(pair, 16)
                .map_err(|_| "Payloadul proprietăților nu este hex valid.".to_string())?,
        );
    }
    if schema_version == LEGACY_DYNAMIC_WIDGET_SCHEMA_VERSION {
        let legacy =
            serde_json::from_slice::<LegacyDynamicWidgetProperties>(&bytes).map_err(|error| {
                format!("Proprietățile dinamice legacy nu respectă schema: {error}")
            })?;
        return Ok(migrate_legacy_properties(legacy));
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("Proprietățile dinamice nu respectă schema: {error}"))
}

fn migrate_legacy_properties(properties: LegacyDynamicWidgetProperties) -> DynamicWidgetProperties {
    match properties {
        LegacyDynamicWidgetProperties::Listing(properties) => {
            DynamicWidgetProperties::Listing(properties)
        }
        LegacyDynamicWidgetProperties::DynamicField(properties) => {
            let presentation = match properties.presentation {
                LegacyDynamicFieldPresentation::Text => DynamicFieldPresentation::Text,
                LegacyDynamicFieldPresentation::Image => DynamicFieldPresentation::Image,
                LegacyDynamicFieldPresentation::Link => DynamicFieldPresentation::Link,
                LegacyDynamicFieldPresentation::Button => DynamicFieldPresentation::Button,
            };
            let _legacy_repeater_path = properties.repeater_item_path;
            DynamicWidgetProperties::DynamicField(DynamicFieldWidgetProperties {
                binding: DynamicValueBinding {
                    context: properties.scope,
                    source: DynamicValueSource::CustomField {
                        model_id: properties.model_id,
                        field_id: properties.field_id,
                    },
                    value_type: DynamicValueType::Text,
                },
                presentation,
                tag: properties.tag,
                format: DynamicValueFormat::default(),
                prefix: properties.prefix,
                suffix: properties.suffix,
                fallback: properties.fallback,
                label: properties.label,
                empty_behavior: properties.empty_behavior,
            })
        }
    }
}

struct ResolvedDynamicValue {
    canonical_path: String,
    expression: String,
    value_type: DynamicValueType,
    trusted_html: bool,
}

fn resolve_dynamic_value(
    catalog: &ContentModelCatalog,
    value_catalog: &[DynamicValueDefinition],
    properties: &DynamicFieldWidgetProperties,
    allow_legacy_type_mismatch: bool,
) -> Result<ResolvedDynamicValue, String> {
    let context = properties.binding.context;
    let resolved = match &properties.binding.source {
        DynamicValueSource::Builtin { field } => {
            let (value_type, trusted_html) = builtin_value_type(context, field)?;
            let root = context_root(context)?;
            ResolvedDynamicValue {
                canonical_path: format!("{root}.{field}"),
                expression: tera_access(root, std::slice::from_ref(field)),
                value_type,
                trusted_html,
            }
        }
        DynamicValueSource::CustomField { model_id, field_id } => {
            let model = catalog
                .models
                .iter()
                .find(|model| model.id == *model_id)
                .ok_or_else(|| format!("Modelul {model_id} nu există."))?;
            let (field, canonical_path, repeater_path) = find_field(model, field_id)
                .ok_or_else(|| format!("Câmpul {field_id} nu există în modelul {model_id}."))?;
            let segments = match context {
                DynamicFieldScope::Page | DynamicFieldScope::CollectionItem => {
                    if repeater_path.is_some() {
                        return Err("Un subcâmp de repetor cere contextul repeaterItem.".to_string());
                    }
                    canonical_path.clone()
                }
                DynamicFieldScope::RepeaterItem => repeater_path.ok_or_else(|| {
                    "Contextul repeaterItem cere un subcâmp aflat într-un repetor.".to_string()
                })?,
                _ => {
                    return Err(
                        "Câmpurile modelului pot folosi numai context page, collectionItem sau repeaterItem."
                            .to_string(),
                    )
                }
            };
            let root = context_root(context)?;
            let expression = if context == DynamicFieldScope::RepeaterItem {
                tera_access(root, &segments)
            } else {
                let mut extra_segments = vec!["extra".to_string()];
                extra_segments.extend(segments);
                tera_access(root, &extra_segments)
            };
            ResolvedDynamicValue {
                canonical_path: canonical_path.join("."),
                expression,
                value_type: content_field_value_type(field.kind),
                trusted_html: false,
            }
        }
        DynamicValueSource::ConfigExtra { path } => {
            if context != DynamicFieldScope::Site {
                return Err("config.extra cere contextul site.".to_string());
            }
            validate_binding_path(path)?;
            let value_type =
                catalog_extra_value_type(value_catalog, context, &properties.binding.source)?;
            let mut segments = vec!["extra".to_string()];
            segments.extend(path.iter().cloned());
            ResolvedDynamicValue {
                canonical_path: format!("config.extra.{}", path.join(".")),
                expression: tera_access("config", &segments),
                value_type,
                trusted_html: false,
            }
        }
        DynamicValueSource::SectionExtra { path } => {
            if context != DynamicFieldScope::Section {
                return Err("section.extra cere contextul section.".to_string());
            }
            validate_binding_path(path)?;
            let value_type =
                catalog_extra_value_type(value_catalog, context, &properties.binding.source)?;
            let mut segments = vec!["extra".to_string()];
            segments.extend(path.iter().cloned());
            ResolvedDynamicValue {
                canonical_path: format!("section.extra.{}", path.join(".")),
                expression: tera_access("section", &segments),
                value_type,
                trusted_html: false,
            }
        }
    };
    if properties.binding.value_type != resolved.value_type && !allow_legacy_type_mismatch {
        return Err(format!(
            "Tipul declarat {:?} nu corespunde sursei {:?}.",
            properties.binding.value_type, resolved.value_type
        ));
    }
    if resolved.value_type == DynamicValueType::ListObject {
        return Err(
            "Valorile listă/obiect cer un Listing sau un context repetor; nu sunt convertite implicit în text."
                .to_string(),
        );
    }
    let presentation = resolve_presentation(properties.presentation, resolved.value_type);
    if presentation == DynamicFieldPresentation::TrustedContent && !resolved.trusted_html {
        return Err(
            "Randarea HTML sigură este permisă numai pentru content/summary generate de Zola."
                .to_string(),
        );
    }
    if !compatible_presentations(resolved.value_type).contains(&presentation)
        && properties.presentation != DynamicFieldPresentation::Auto
    {
        return Err(format!(
            "Prezentarea {:?} nu este compatibilă cu tipul {:?}.",
            properties.presentation, resolved.value_type
        ));
    }
    validate_html_tag(&properties.tag)?;
    validate_presentation_tag(presentation, &properties.tag)?;
    if properties.format.decimals.is_some_and(|value| value > 12) {
        return Err("Precizia numerică nu poate depăși 12 zecimale.".to_string());
    }
    for (label, value) in [
        ("prefix", properties.prefix.as_str()),
        ("suffix", properties.suffix.as_str()),
        ("fallback", properties.fallback.as_str()),
        ("label", properties.label.as_str()),
        ("format dată", properties.format.date_format.as_str()),
        ("monedă", properties.format.currency.as_str()),
    ] {
        if value.len() > 2000 || value.contains('\0') {
            return Err(format!("Valoarea {label} este prea lungă sau invalidă."));
        }
    }
    Ok(resolved)
}

fn catalog_extra_value_type(
    value_catalog: &[DynamicValueDefinition],
    context: DynamicFieldScope,
    source: &DynamicValueSource,
) -> Result<DynamicValueType, String> {
    value_catalog
        .iter()
        .find(|definition| definition.contexts.contains(&context) && definition.source == *source)
        .map(|definition| definition.value_type)
        .ok_or_else(|| {
            "Valoarea extra nu mai există în catalogul autoritativ al proiectului.".to_string()
        })
}

fn builtin_value_type(
    context: DynamicFieldScope,
    field: &str,
) -> Result<(DynamicValueType, bool), String> {
    let result = match context {
        DynamicFieldScope::Page | DynamicFieldScope::CollectionItem => match field {
            "title" | "description" | "slug" | "path" | "lang" => {
                Some((DynamicValueType::Text, false))
            }
            "date" | "updated" => Some((DynamicValueType::Date, false)),
            "permalink" => Some((DynamicValueType::Url, false)),
            "summary" | "content" => Some((DynamicValueType::RichHtml, true)),
            "weight" | "word_count" | "reading_time" => Some((DynamicValueType::Number, false)),
            _ => None,
        },
        DynamicFieldScope::Section => match field {
            "title" | "description" | "path" | "lang" => Some((DynamicValueType::Text, false)),
            "permalink" => Some((DynamicValueType::Url, false)),
            _ => None,
        },
        DynamicFieldScope::Site => match field {
            "title" | "description" | "default_language" => Some((DynamicValueType::Text, false)),
            "base_url" => Some((DynamicValueType::Url, false)),
            _ => None,
        },
        DynamicFieldScope::TaxonomyTerm => match field {
            "name" | "slug" | "path" => Some((DynamicValueType::Text, false)),
            "permalink" => Some((DynamicValueType::Url, false)),
            _ => None,
        },
        DynamicFieldScope::RepeaterItem => None,
    };
    result.ok_or_else(|| format!("Valoarea standard {field} nu există în contextul {context:?}."))
}

fn context_root(context: DynamicFieldScope) -> Result<&'static str, String> {
    match context {
        DynamicFieldScope::Page => Ok("page"),
        DynamicFieldScope::CollectionItem | DynamicFieldScope::RepeaterItem => Ok("item"),
        DynamicFieldScope::Section => Ok("section"),
        DynamicFieldScope::Site => Ok("config"),
        DynamicFieldScope::TaxonomyTerm => Ok("term"),
    }
}

fn validate_binding_path(path: &[String]) -> Result<(), String> {
    if path.is_empty() || path.len() > 32 {
        return Err("Calea valorii dinamice este goală sau prea adâncă.".to_string());
    }
    if path.iter().any(|segment| {
        segment.is_empty() || segment.len() > 256 || segment.contains(['\0', '\n', '\r'])
    }) {
        return Err("Calea valorii dinamice conține segmente invalide.".to_string());
    }
    Ok(())
}

fn tera_access(root: &str, path: &[String]) -> String {
    let mut expression = root.to_string();
    for segment in path {
        if is_tera_identifier(segment) {
            expression.push('.');
            expression.push_str(segment);
        } else {
            expression.push_str("[\"");
            expression.push_str(&escape_tera_string(segment));
            expression.push_str("\"]");
        }
    }
    expression
}

fn is_tera_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn resolve_presentation(
    presentation: DynamicFieldPresentation,
    value_type: DynamicValueType,
) -> DynamicFieldPresentation {
    if presentation == DynamicFieldPresentation::Auto {
        default_presentation(value_type)
    } else {
        presentation
    }
}

fn validate_presentation_tag(
    presentation: DynamicFieldPresentation,
    tag: &str,
) -> Result<(), String> {
    let tag = tag.trim();
    let valid = match presentation {
        DynamicFieldPresentation::Heading => matches!(tag, "h1" | "h2" | "h3" | "h4" | "h5" | "h6"),
        DynamicFieldPresentation::Paragraph => matches!(tag, "p" | "div"),
        DynamicFieldPresentation::Image => tag == "img",
        DynamicFieldPresentation::Link | DynamicFieldPresentation::Button => tag == "a",
        DynamicFieldPresentation::TrustedContent => matches!(tag, "div" | "section" | "article"),
        _ => true,
    };
    if valid {
        Ok(())
    } else {
        Err(format!(
            "Eticheta {tag} nu este compatibilă cu prezentarea {presentation:?}."
        ))
    }
}

fn validate_listing_properties(
    properties: &ListingWidgetProperties,
    source_graph: &SourceGraph,
) -> Result<(), String> {
    let section_path = normalize_content_path(&properties.section_path)?;
    if !source_graph.pages.iter().any(|page| {
        matches!(
            page.page_kind,
            crate::source_graph::model::SourcePageKind::Section
        ) && normalize_content_path(&page.file).ok().as_deref() == Some(section_path.as_str())
    }) {
        return Err(format!(
            "Secțiunea {section_path} nu există în Source Graph."
        ));
    }
    let template = normalize_template_reference(&properties.listing_item_template)?;
    if !template.starts_with("listing-items/") || !template.ends_with(".html") {
        return Err(
            "Listing Item trebuie să fie un partial templates/listing-items/*.html.".to_string(),
        );
    }
    if !source_graph.templates.iter().any(|candidate| {
        normalize_template_reference(&candidate.name)
            .ok()
            .as_deref()
            == Some(template.as_str())
    }) {
        return Err(format!("Listing Item-ul {template} nu există."));
    }
    let item = source_graph
        .listing_items
        .items
        .iter()
        .find(|item| item.id == properties.listing_item_id)
        .ok_or_else(|| {
            format!(
                "Contractul Listing Item {} nu există.",
                properties.listing_item_id
            )
        })?;
    if item.status != crate::kernel::listing_items::ListingItemStatus::Resolved {
        return Err(format!(
            "Listing Item-ul {} nu este rezolvat ({:?}).",
            item.id, item.status
        ));
    }
    let item_template = normalize_template_reference(&item.template_name)?;
    if item_template != template {
        return Err(format!(
            "Listing Item-ul {} aparține template-ului {}, nu {}.",
            item.id, item_template, template
        ));
    }
    if !item.compatible_section_paths.iter().any(|candidate| {
        normalize_content_path(candidate).ok().as_deref() == Some(section_path.as_str())
    }) {
        return Err(format!(
            "Listing Item-ul {} nu este compatibil cu secțiunea {}.",
            item.id, section_path
        ));
    }
    validate_html_tag(&properties.tag)?;
    if properties.offset > 100_000 || properties.limit.is_some_and(|limit| limit > 100_000) {
        return Err("Limita sau offsetul Listing-ului depășește 100000.".to_string());
    }
    Ok(())
}

fn render_dynamic_field(
    instance_id: &str,
    properties: &DynamicFieldWidgetProperties,
    source_graph: &SourceGraph,
) -> Result<String, String> {
    let value_catalog = dynamic_value_catalog(source_graph);
    let resolved = resolve_dynamic_value(
        &source_graph.content_models,
        &value_catalog,
        properties,
        false,
    )?;
    let expression = resolved.expression;
    let presentation = resolve_presentation(properties.presentation, resolved.value_type);
    let mut value = expression.clone();
    match presentation {
        DynamicFieldPresentation::Date => {
            let format = if properties.format.date_format.trim().is_empty() {
                "%d.%m.%Y"
            } else {
                properties.format.date_format.trim()
            };
            value.push_str(&format!(
                " | date(format=\"{}\")",
                escape_tera_string(format)
            ));
        }
        DynamicFieldPresentation::Number
        | DynamicFieldPresentation::Currency
        | DynamicFieldPresentation::Percent => {
            if let Some(decimals) = properties.format.decimals {
                value.push_str(&format!(" | round(precision={decimals})"));
            }
        }
        _ => {}
    }
    let tag = properties.tag.trim();
    let prefix = escape_html_text(&properties.prefix);
    let mut suffix = escape_html_text(&properties.suffix);
    if presentation == DynamicFieldPresentation::Percent && suffix.is_empty() {
        suffix.push('%');
    }
    let currency = escape_html_text(&properties.format.currency);
    let prefix = if presentation == DynamicFieldPresentation::Currency && !currency.is_empty() {
        format!("{prefix}{currency} ")
    } else {
        prefix
    };
    let label = escape_html_text(if properties.label.trim().is_empty() {
        "Deschide"
    } else {
        properties.label.trim()
    });
    let instance = escape_html_attribute(instance_id);
    let present_body = match presentation {
        DynamicFieldPresentation::Auto => unreachable!("auto is resolved before rendering"),
        DynamicFieldPresentation::Text
        | DynamicFieldPresentation::Heading
        | DynamicFieldPresentation::Paragraph
        | DynamicFieldPresentation::Badge
        | DynamicFieldPresentation::Date
        | DynamicFieldPresentation::Number
        | DynamicFieldPresentation::Currency
        | DynamicFieldPresentation::Percent => format!(
            "<{tag} data-pana-widget-instance=\"{instance}\">{prefix}{{{{ {value} }}}}{suffix}</{tag}>"
        ),
        DynamicFieldPresentation::Image => format!(
            "<img data-pana-widget-instance=\"{instance}\" src=\"{{{{ {value} }}}}\" alt=\"{label}\">"
        ),
        DynamicFieldPresentation::Link => format!(
            "<{tag} data-pana-widget-instance=\"{instance}\" href=\"{{{{ {value} }}}}\">{prefix}{label}{suffix}</{tag}>"
        ),
        DynamicFieldPresentation::Button => format!(
            "<{tag} class=\"button\" data-pana-widget-instance=\"{instance}\" href=\"{{{{ {value} }}}}\">{prefix}{label}{suffix}</{tag}>"
        ),
        DynamicFieldPresentation::TrustedContent => format!(
            "<{tag} data-pana-widget-instance=\"{instance}\">{{{{ {expression} | safe }}}}</{tag}>"
        ),
    };
    if properties.empty_behavior == DynamicFieldEmptyBehavior::Hide {
        return Ok(format!(
            "{{% if {expression} is defined and {expression} %}}\n{present_body}\n{{% endif %}}"
        ));
    }

    let fallback = if properties.empty_behavior == DynamicFieldEmptyBehavior::Fallback {
        properties.fallback.as_str()
    } else {
        ""
    };
    let fallback_text = escape_html_text(fallback);
    let fallback_attribute = escape_html_attribute(fallback);
    let missing_body = match presentation {
        DynamicFieldPresentation::Auto => unreachable!("auto is resolved before rendering"),
        DynamicFieldPresentation::Text
        | DynamicFieldPresentation::Heading
        | DynamicFieldPresentation::Paragraph
        | DynamicFieldPresentation::Badge
        | DynamicFieldPresentation::Date
        | DynamicFieldPresentation::Number
        | DynamicFieldPresentation::Currency
        | DynamicFieldPresentation::Percent
            if !fallback.is_empty() =>
        {
            format!(
                "<{tag} data-pana-widget-instance=\"{instance}\">{prefix}{fallback_text}{suffix}</{tag}>"
            )
        }
        DynamicFieldPresentation::Text
        | DynamicFieldPresentation::Heading
        | DynamicFieldPresentation::Paragraph
        | DynamicFieldPresentation::Badge
        | DynamicFieldPresentation::Date
        | DynamicFieldPresentation::Number
        | DynamicFieldPresentation::Currency
        | DynamicFieldPresentation::Percent
        => {
            format!("<{tag} data-pana-widget-instance=\"{instance}\"></{tag}>")
        }
        DynamicFieldPresentation::Image if !fallback.is_empty() => format!(
            "<img data-pana-widget-instance=\"{instance}\" src=\"{fallback_attribute}\" alt=\"{label}\">"
        ),
        DynamicFieldPresentation::Image => {
            format!("<img data-pana-widget-instance=\"{instance}\" alt=\"{label}\">")
        }
        DynamicFieldPresentation::TrustedContent if !fallback.is_empty() => format!(
            "<{tag} data-pana-widget-instance=\"{instance}\">{fallback_text}</{tag}>"
        ),
        DynamicFieldPresentation::TrustedContent => {
            format!("<{tag} data-pana-widget-instance=\"{instance}\"></{tag}>")
        }
        DynamicFieldPresentation::Link if !fallback.is_empty() => format!(
            "<{tag} data-pana-widget-instance=\"{instance}\" href=\"{fallback_attribute}\">{prefix}{label}{suffix}</{tag}>"
        ),
        DynamicFieldPresentation::Button if !fallback.is_empty() => format!(
            "<{tag} class=\"button\" data-pana-widget-instance=\"{instance}\" href=\"{fallback_attribute}\">{prefix}{label}{suffix}</{tag}>"
        ),
        DynamicFieldPresentation::Link => {
            format!("<{tag} data-pana-widget-instance=\"{instance}\"></{tag}>")
        }
        DynamicFieldPresentation::Button => format!(
            "<{tag} class=\"button\" data-pana-widget-instance=\"{instance}\"></{tag}>"
        ),
    };
    Ok(format!(
        "{{% if {expression} is defined %}}\n{present_body}\n{{% else %}}\n{missing_body}\n{{% endif %}}"
    ))
}

fn render_listing(
    instance_id: &str,
    properties: &ListingWidgetProperties,
) -> Result<String, String> {
    let section = normalize_content_path(&properties.section_path)?;
    let template = normalize_template_reference(&properties.listing_item_template)?;
    if !template.starts_with("listing-items/") || !template.ends_with(".html") {
        return Err("Listing-ul cere un partial listing-items/*.html.".to_string());
    }
    validate_html_tag(&properties.tag)?;
    let variable = format!(
        "pana_listing_{}",
        instance_id
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .take(16)
            .collect::<String>()
    );
    let pages_variable = format!("{variable}_pages");
    let collection = listing_collection_expression(&pages_variable, properties);
    let tag = properties.tag.trim();
    let class = escape_html_attribute(&properties.class_name);
    let class_attribute = if class.is_empty() {
        String::new()
    } else {
        format!(" class=\"{class}\"")
    };
    let empty = escape_html_text(if properties.empty_text.trim().is_empty() {
        "Nu există articole."
    } else {
        properties.empty_text.trim()
    });
    let instance = escape_html_attribute(instance_id);
    let escaped_section = escape_tera_string(&section);
    let mut body = format!(
        "{{% set {variable} = get_section(path=\"{escaped_section}\") %}}\n{{% set {pages_variable} = {variable}.pages %}}\n{{% if section is defined %}}\n  {{% if paginator is defined %}}\n    {{% if section.relative_path == \"{escaped_section}\" %}}\n      {{% set_global {pages_variable} = paginator.pages %}}\n    {{% endif %}}\n  {{% endif %}}\n{{% endif %}}\n<{tag}{class_attribute} data-pana-widget-instance=\"{instance}\">\n{{% for item in {collection} %}}\n  {{% include \"{template}\" %}}\n{{% else %}}\n  <p data-pana-listing-empty>{empty}</p>\n{{% endfor %}}"
    );
    if properties.include_subsections {
        body.push_str(&format!(
            "\n{{% for pana_subsection_path in {variable}.subsections %}}\n  {{% set pana_subsection = get_section(path=pana_subsection_path) %}}\n  {{% for item in pana_subsection.pages %}}\n    {{% include \"{template}\" %}}\n  {{% endfor %}}\n{{% endfor %}}"
        ));
    }
    body.push_str(&format!("\n</{tag}>"));
    body.push_str(&format!(
        "\n{{% if section is defined %}}\n  {{% if paginator is defined %}}\n    {{% if section.relative_path == \"{escaped_section}\" and paginator.number_pagers > 1 %}}\n      <nav class=\"paginare\" data-pana-listing-pagination aria-label=\"Paginare\">\n        {{% if paginator.previous %}}<a href=\"{{{{ paginator.previous }}}}\">Pagina anterioară</a>{{% endif %}}\n        <span>Pagina {{{{ paginator.current_index }}}} din {{{{ paginator.number_pagers }}}}</span>\n        {{% if paginator.next %}}<a href=\"{{{{ paginator.next }}}}\">Pagina următoare</a>{{% endif %}}\n      </nav>\n    {{% endif %}}\n  {{% endif %}}\n{{% endif %}}"
    ));
    Ok(body)
}

fn listing_collection_expression(variable: &str, properties: &ListingWidgetProperties) -> String {
    let mut expression = variable.to_string();
    if properties.sort_by != ListingSortBy::None {
        let attribute = match properties.sort_by {
            ListingSortBy::Date => "date",
            ListingSortBy::Updated => "updated",
            ListingSortBy::Title => "title",
            ListingSortBy::Weight => "weight",
            ListingSortBy::Slug => "slug",
            ListingSortBy::None => unreachable!(),
        };
        expression.push_str(&format!(
            " | sort(attribute=\"{attribute}\"){}",
            if properties.sort_order == ListingSortOrder::Desc {
                " | reverse"
            } else {
                ""
            }
        ));
    } else if properties.sort_order == ListingSortOrder::Desc {
        expression.push_str(" | reverse");
    }
    if properties.offset > 0 || properties.limit.is_some() {
        expression.push_str(&format!(" | slice(start={}", properties.offset));
        if let Some(limit) = properties.limit {
            expression.push_str(&format!(
                ", end={}",
                properties.offset.saturating_add(limit)
            ));
        }
        expression.push(')');
    }
    expression
}

type ContentFieldMatch<'a> = (&'a ContentFieldDefinition, Vec<String>, Option<Vec<String>>);

fn find_field<'a>(
    model: &'a ContentModelDefinition,
    field_id: &str,
) -> Option<ContentFieldMatch<'a>> {
    fn visit<'a>(
        fields: &'a [ContentFieldDefinition],
        field_id: &str,
        parent: &[String],
        repeater_parent: Option<&[String]>,
    ) -> Option<ContentFieldMatch<'a>> {
        for field in fields {
            let mut path = parent.to_vec();
            path.push(field.key.clone());
            let item_path = repeater_parent.map(|parent| {
                let mut path = parent.to_vec();
                path.push(field.key.clone());
                path
            });
            if field.id == field_id {
                return Some((field, path, item_path));
            }
            let next_repeater = if field.kind == ContentFieldKind::Repeater {
                Some(Vec::new())
            } else {
                item_path
            };
            if let Some(found) = visit(&field.fields, field_id, &path, next_repeater.as_deref()) {
                return Some(found);
            }
        }
        None
    }
    visit(&model.fields, field_id, &[], None)
}

fn source_nodes_in_boundary(
    graph: &SourceGraph,
    file: &str,
    start: usize,
    end: usize,
) -> (Vec<String>, Vec<String>) {
    let nodes = graph
        .nodes
        .iter()
        .filter(|node| {
            node.file == file
                && node
                    .range
                    .as_ref()
                    .is_some_and(|range| range.start >= start && range.end <= end)
        })
        .collect::<Vec<_>>();
    let ids = nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<HashSet<_>>();
    let mut source_node_ids = nodes.iter().map(|node| node.id.clone()).collect::<Vec<_>>();
    let mut root_source_node_ids = nodes
        .iter()
        .filter(|node| {
            node.parent
                .as_ref()
                .is_none_or(|parent| !ids.contains(parent))
        })
        .map(|node| node.id.clone())
        .collect::<Vec<_>>();
    source_node_ids.sort();
    root_source_node_ids.sort();
    (source_node_ids, root_source_node_ids)
}

fn validate_instance_id(value: &str) -> Result<(), String> {
    if value.len() < 8
        || value.len() > 96
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err("InstanceId-ul dinamic este invalid.".to_string());
    }
    Ok(())
}

fn validate_html_tag(value: &str) -> Result<(), String> {
    const ALLOWED: &[&str] = &[
        "a", "article", "div", "figure", "h1", "h2", "h3", "h4", "h5", "h6", "img", "li", "p",
        "section", "small", "span", "strong",
    ];
    if ALLOWED.contains(&value.trim()) {
        Ok(())
    } else {
        Err(format!("Eticheta HTML {} nu este permisă.", value.trim()))
    }
}

fn normalize_content_path(value: &str) -> Result<String, String> {
    let value = value.trim().replace('\\', "/");
    let value = value.strip_prefix("content/").unwrap_or(&value);
    if value.is_empty()
        || value.starts_with('/')
        || !value.ends_with(".md")
        || value.split('/').any(|part| part.is_empty() || part == "..")
    {
        return Err(format!("Cale de secțiune invalidă: {value}."));
    }
    Ok(value.to_string())
}

fn normalize_template_reference(value: &str) -> Result<String, String> {
    let normalized = value.trim().replace('\\', "/");
    let normalized = normalized.strip_prefix("templates/").unwrap_or(&normalized);
    if normalized.is_empty()
        || normalized.starts_with('/')
        || normalized
            .split('/')
            .any(|part| part.is_empty() || part == "..")
    {
        return Err(format!("Referință Tera invalidă: {value}."));
    }
    Ok(normalized.to_string())
}

fn escape_tera_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn escape_html_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn escape_html_attribute(value: &str) -> String {
    escape_html_text(value)
}

fn source_revision(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn source_range(source: &str, start: usize, end: usize) -> SourceRange {
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
    let safe = offset.min(source.len());
    let prefix = &source[..safe];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit_once('\n')
        .map(|(_, suffix)| suffix.chars().count() + 1)
        .unwrap_or_else(|| prefix.chars().count() + 1);
    (line, column)
}

fn widget_diagnostic(
    code: &str,
    message: String,
    file: &str,
    instance_id: &str,
) -> DynamicWidgetDiagnostic {
    DynamicWidgetDiagnostic {
        code: code.to_string(),
        message,
        file: Some(file.to_string()),
        instance_id: (!instance_id.is_empty()).then(|| instance_id.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

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
            project_workspace::{
                ProjectWorkspace, ProjectWorkspaceIdentity, WorkspaceHistoryDirection,
                WorkspaceMutationMetadata, WorkspaceResourceMutation,
            },
        },
        project::{AcceptedProjectDiskManifest, ProjectDiskManifest, ProjectDiskManifestEntry},
    };

    use super::*;

    fn empty_source_graph() -> SourceGraph {
        SourceGraph {
            node_index: Default::default(),
            project_root: String::new(),
            zola_root: String::new(),
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
        }
    }

    fn field_properties() -> DynamicWidgetProperties {
        DynamicWidgetProperties::DynamicField(DynamicFieldWidgetProperties {
            binding: DynamicValueBinding {
                context: DynamicFieldScope::CollectionItem,
                source: DynamicValueSource::CustomField {
                    model_id: "service".to_string(),
                    field_id: "field-title".to_string(),
                },
                value_type: DynamicValueType::Text,
            },
            presentation: DynamicFieldPresentation::Text,
            tag: "h2".to_string(),
            format: DynamicValueFormat::default(),
            prefix: String::new(),
            suffix: String::new(),
            fallback: "Fără titlu".to_string(),
            label: String::new(),
            empty_behavior: DynamicFieldEmptyBehavior::Fallback,
        })
    }

    fn field_catalog() -> ContentModelCatalog {
        ContentModelCatalog {
            models: vec![ContentModelDefinition {
                schema_version: 1,
                id: "service".to_string(),
                label: "Serviciu".to_string(),
                description: String::new(),
                fields: vec![ContentFieldDefinition {
                    id: "field-title".to_string(),
                    key: "title".to_string(),
                    label: "Titlu".to_string(),
                    kind: ContentFieldKind::Text,
                    required: false,
                    help: String::new(),
                    default_value: None,
                    choices: Vec::new(),
                    minimum: None,
                    maximum: None,
                    pattern: None,
                    fields: Vec::new(),
                }],
                file: ".panastudio/content-models/service.toml".to_string(),
            }],
            ..Default::default()
        }
    }

    fn test_workspace(root: &Path, relative_path: &str, source: &str) -> ProjectWorkspace {
        let session = ProjectSessionSnapshot {
            schema_version: 1,
            id: "dynamic-widget-test".to_string(),
            project_root: root.to_string_lossy().to_string(),
            zola_root: root.to_string_lossy().to_string(),
            session_dir: root.join("session").to_string_lossy().to_string(),
            manifest_path: root.join("session.json").to_string_lossy().to_string(),
            opened_at_ms: 1,
            last_seen_at_ms: 1,
            root_fingerprint: ProjectRootFingerprint {
                canonical_path: root.to_string_lossy().to_string(),
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
            relative_path: relative_path.to_string(),
            absolute_path: root.join(relative_path).to_string_lossy().to_string(),
            language: TextBufferLanguage::Html,
            role: TextBufferRole::Template,
            baseline: FileBufferBaseline {
                hash: hash_text(source),
                modified_ms: 1,
                size: source.len() as u64,
                readonly: false,
            },
            baseline_text: source.to_string(),
            draft: None,
            revision: 1,
        });
        let accepted = AcceptedProjectDiskManifest::new(
            session.runtime_instance_id(),
            session.project_root.clone(),
            ProjectDiskManifest {
                root: session.project_root.clone(),
                files: vec![ProjectDiskManifestEntry {
                    relative_path: relative_path.to_string(),
                    modified_ms: 1,
                    size: source.len() as u64,
                    version_token: String::new(),
                }],
                truncated: false,
                max_files: 8,
            },
        )
        .unwrap();
        let page_js = PageJsDraftStore::new(&session);
        ProjectWorkspace::new(session, accepted, documents, page_js).unwrap()
    }

    fn workspace_identity(workspace: &ProjectWorkspace) -> ProjectWorkspaceIdentity {
        ProjectWorkspaceIdentity {
            expected_project_root: workspace.session.project_root.clone(),
            expected_session_id: workspace.runtime_session_id(),
            expected_revision: workspace.revision,
        }
    }

    #[test]
    fn marker_round_trip_preserves_typed_properties() {
        let properties = field_properties();
        let encoded = encode_properties(&properties).unwrap();
        let decoded = decode_properties(&encoded, DYNAMIC_WIDGET_SCHEMA_VERSION).unwrap();
        assert!(matches!(
            decoded,
            DynamicWidgetProperties::DynamicField(DynamicFieldWidgetProperties {
                binding: DynamicValueBinding {
                    context: DynamicFieldScope::CollectionItem,
                    ..
                },
                presentation: DynamicFieldPresentation::Text,
                ..
            })
        ));
    }

    #[test]
    fn legacy_dynamic_field_marker_is_migrated_to_dynamic_value_binding() {
        let legacy = serde_json::json!({
            "kind": "dynamicField",
            "properties": {
                "modelId": "service",
                "fieldId": "field-title",
                "scope": "collectionItem",
                "repeaterItemPath": null,
                "presentation": "text",
                "tag": "h2",
                "prefix": "",
                "suffix": "",
                "fallback": "Fără titlu",
                "label": "",
                "emptyBehavior": "fallback"
            }
        });
        let encoded = serde_json::to_vec(&legacy)
            .unwrap()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let migrated = decode_properties(&encoded, LEGACY_DYNAMIC_WIDGET_SCHEMA_VERSION).unwrap();
        let DynamicWidgetProperties::DynamicField(properties) = migrated else {
            panic!("markerul legacy trebuia migrat la DynamicField");
        };
        assert_eq!(
            properties.binding.context,
            DynamicFieldScope::CollectionItem
        );
        assert_eq!(properties.binding.value_type, DynamicValueType::Text);
        assert!(matches!(
            properties.binding.source,
            DynamicValueSource::CustomField { ref model_id, ref field_id }
                if model_id == "service" && field_id == "field-title"
        ));
    }

    #[test]
    fn builtin_title_and_trusted_content_render_from_zola_context() {
        let graph = empty_source_graph();
        let title = DynamicWidgetProperties::DynamicField(DynamicFieldWidgetProperties {
            binding: DynamicValueBinding {
                context: DynamicFieldScope::Page,
                source: DynamicValueSource::Builtin {
                    field: "title".into(),
                },
                value_type: DynamicValueType::Text,
            },
            presentation: DynamicFieldPresentation::Heading,
            tag: "h1".into(),
            format: DynamicValueFormat::default(),
            prefix: String::new(),
            suffix: String::new(),
            fallback: String::new(),
            label: String::new(),
            empty_behavior: DynamicFieldEmptyBehavior::RenderEmpty,
        });
        let rendered = render_dynamic_widget("dynamic-field-title1", &title, &graph).unwrap();
        assert!(rendered.contains("{{ page.title }}"));
        assert!(rendered.contains("<h1 data-pana-widget-instance="));

        let mut content = title;
        let DynamicWidgetProperties::DynamicField(ref mut properties) = content else {
            unreachable!()
        };
        properties.binding.source = DynamicValueSource::Builtin {
            field: "content".into(),
        };
        properties.binding.value_type = DynamicValueType::RichHtml;
        properties.presentation = DynamicFieldPresentation::TrustedContent;
        properties.tag = "div".into();
        let rendered = render_dynamic_widget("dynamic-field-content1", &content, &graph).unwrap();
        assert!(rendered.contains("{{ page.content | safe }}"));
    }

    #[test]
    fn optional_dynamic_values_follow_empty_fallback_and_hide_contracts() {
        let mut graph = empty_source_graph();
        graph.content_models = field_catalog();
        let mut context = tera::Context::new();
        context.insert("item", &serde_json::json!({ "extra": {} }));

        let fallback =
            render_dynamic_widget("dynamic-field-fallback01", &field_properties(), &graph).unwrap();
        let fallback_output = tera::Tera::one_off(&fallback, &context, false).unwrap();
        assert!(fallback_output.contains("Fără titlu"));

        let mut render_empty = field_properties();
        let DynamicWidgetProperties::DynamicField(ref mut field) = render_empty else {
            unreachable!()
        };
        field.empty_behavior = DynamicFieldEmptyBehavior::RenderEmpty;
        field.fallback.clear();
        field.presentation = DynamicFieldPresentation::Date;
        field.binding.value_type = DynamicValueType::Date;
        field.tag = "span".into();
        graph.content_models.models[0].fields[0].kind = ContentFieldKind::Date;
        let render_empty =
            render_dynamic_widget("dynamic-field-empty001", &render_empty, &graph).unwrap();
        assert!(render_empty.contains("| date("));
        let render_empty_output = tera::Tera::one_off(&render_empty, &context, false).unwrap();
        assert!(render_empty_output
            .contains("<span data-pana-widget-instance=\"dynamic-field-empty001\"></span>"));

        let mut hidden = field_properties();
        let DynamicWidgetProperties::DynamicField(ref mut field) = hidden else {
            unreachable!()
        };
        field.empty_behavior = DynamicFieldEmptyBehavior::Hide;
        field.fallback.clear();
        graph.content_models.models[0].fields[0].kind = ContentFieldKind::Text;
        let hidden = render_dynamic_widget("dynamic-field-hidden01", &hidden, &graph).unwrap();
        let hidden_output = tera::Tera::one_off(&hidden, &context, false).unwrap();
        assert!(!hidden_output.contains("data-pana-widget-instance"));

        let mut image = field_properties();
        let DynamicWidgetProperties::DynamicField(ref mut field) = image else {
            unreachable!()
        };
        field.empty_behavior = DynamicFieldEmptyBehavior::RenderEmpty;
        field.fallback.clear();
        field.presentation = DynamicFieldPresentation::Image;
        field.binding.value_type = DynamicValueType::Image;
        field.tag = "img".into();
        graph.content_models.models[0].fields[0].kind = ContentFieldKind::Image;
        let image = render_dynamic_widget("dynamic-field-image001", &image, &graph).unwrap();
        let image_output = tera::Tera::one_off(&image, &context, false).unwrap();
        assert!(image_output.contains(
            "<img data-pana-widget-instance=\"dynamic-field-image001\" alt=\"Deschide\">"
        ));
        assert!(!image_output.contains("src="));
    }

    #[test]
    fn schema_two_rejects_a_declared_type_that_disagrees_with_the_catalog() {
        let graph = empty_source_graph();
        let mut properties = field_properties();
        let DynamicWidgetProperties::DynamicField(ref mut field) = properties else {
            unreachable!()
        };
        field.binding.source = DynamicValueSource::Builtin {
            field: "title".into(),
        };
        field.binding.value_type = DynamicValueType::Image;
        field.presentation = DynamicFieldPresentation::Image;
        field.tag = "img".into();
        assert!(
            render_dynamic_widget("dynamic-field-type01", &properties, &graph)
                .unwrap_err()
                .contains("Tipul declarat")
        );
    }

    #[test]
    fn custom_keys_use_safe_tera_bracket_access_and_parse() {
        let mut graph = empty_source_graph();
        let mut catalog = field_catalog();
        catalog.models[0].fields[0].key = "hero-title".into();
        graph.content_models = catalog;
        let mut properties = field_properties();
        let DynamicWidgetProperties::DynamicField(ref mut field) = properties else {
            unreachable!()
        };
        field.binding.context = DynamicFieldScope::Page;
        let rendered = render_dynamic_widget("dynamic-field-safe01", &properties, &graph).unwrap();
        assert!(rendered.contains("page.extra[\"hero-title\"]"));
        let mut tera = tera::Tera::default();
        tera.add_raw_template("safe.html", &rendered).unwrap();
    }

    #[test]
    fn list_object_is_not_stringified_and_untrusted_custom_html_cannot_be_safe() {
        let mut graph = empty_source_graph();
        let mut catalog = field_catalog();
        catalog.models[0].fields[0].kind = ContentFieldKind::Repeater;
        graph.content_models = catalog;
        let mut properties = field_properties();
        if let DynamicWidgetProperties::DynamicField(field) = &mut properties {
            field.binding.context = DynamicFieldScope::Page;
            field.binding.value_type = DynamicValueType::ListObject;
        }
        assert!(
            render_dynamic_widget("dynamic-field-list001", &properties, &graph)
                .unwrap_err()
                .contains("listă/obiect")
        );

        graph.content_models.models[0].fields[0].kind = ContentFieldKind::Markdown;
        if let DynamicWidgetProperties::DynamicField(field) = &mut properties {
            field.binding.value_type = DynamicValueType::Text;
            field.presentation = DynamicFieldPresentation::TrustedContent;
            field.tag = "div".into();
        }
        assert!(
            render_dynamic_widget("dynamic-field-html001", &properties, &graph)
                .unwrap_err()
                .contains("HTML sigură")
        );
    }

    #[test]
    fn listing_item_locks_collection_context_and_custom_model() {
        use crate::kernel::listing_items::{ListingItemDefinition, ListingItemStatus};

        let mut graph = empty_source_graph();
        graph.content_models = field_catalog();
        graph.listing_items.items.push(ListingItemDefinition {
            id: "service-card".into(),
            label: "Card serviciu".into(),
            template_name: "listing-items/service-card.html".into(),
            file: "templates/listing-items/service-card.html".into(),
            model_id: Some("service".into()),
            preview_page_file: None,
            preview_url: None,
            compatible_section_paths: Vec::new(),
            usage_count: 0,
            status: ListingItemStatus::Resolved,
            diagnostics: Vec::new(),
        });
        let mut properties = field_properties();
        let DynamicWidgetProperties::DynamicField(ref mut field) = properties else {
            unreachable!()
        };
        validate_dynamic_widget_source_context(
            "templates/listing-items/service-card.html",
            field,
            &graph,
        )
        .unwrap();

        field.binding.context = DynamicFieldScope::Page;
        assert!(validate_dynamic_widget_source_context(
            "templates/listing-items/service-card.html",
            field,
            &graph,
        )
        .unwrap_err()
        .contains("collectionItem"));

        field.binding.context = DynamicFieldScope::CollectionItem;
        field.binding.source = DynamicValueSource::CustomField {
            model_id: "other".into(),
            field_id: "field-title".into(),
        };
        assert!(validate_dynamic_widget_source_context(
            "templates/listing-items/service-card.html",
            field,
            &graph,
        )
        .unwrap_err()
        .contains("service"));
    }

    #[test]
    fn collection_item_marker_projects_a_typed_content_model_usage() {
        let mut graph = empty_source_graph();
        graph.content_models = field_catalog();
        let source =
            render_dynamic_widget("dynamic-field-a1b2c3d4", &field_properties(), &graph).unwrap();
        let usages = project_dynamic_field_usages(
            &source,
            "templates/listing-items/service.html",
            &graph.content_models,
        );
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].model_id, "service");
        assert_eq!(usages[0].field_id, "field-title");
        assert_eq!(usages[0].field_key, "title");
    }

    #[test]
    fn parser_requires_matching_instance_end_marker() {
        let encoded = encode_properties(&field_properties()).unwrap();
        let source = format!(
            "{{# pana:widget schema={DYNAMIC_WIDGET_SCHEMA_VERSION} provider=dynamic-field instance=dynamic-field-a1b2c3d4 props={encoded} #}}\n<h2>test</h2>\n{{# /pana:widget instance=dynamic-field-a1b2c3d4 #}}"
        );
        let parsed = parse_widget_boundaries(&source, "templates/index.html");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].instance_id, "dynamic-field-a1b2c3d4");
        assert!(parsed[0].diagnostics.is_empty());
        assert!(parsed[0].end > parsed[0].end_start);
    }

    #[test]
    fn listing_emits_get_section_for_and_include() {
        let properties = ListingWidgetProperties {
            section_path: "servicii/_index.md".to_string(),
            listing_item_id: "service-card".to_string(),
            listing_item_template: "listing-items/service-card.html".to_string(),
            include_subsections: false,
            sort_by: ListingSortBy::Date,
            sort_order: ListingSortOrder::Desc,
            limit: Some(6),
            offset: 0,
            empty_text: "Niciun serviciu".to_string(),
            tag: "section".to_string(),
            class_name: "services".to_string(),
        };
        let source = render_listing("listing-a1b2c3d4", &properties).unwrap();
        assert!(source.contains("get_section(path=\"servicii/_index.md\")"));
        assert!(source.contains("section.relative_path == \"servicii/_index.md\""));
        assert!(source.contains("paginator.pages"));
        assert!(source.contains("data-pana-listing-pagination"));
        assert!(source.contains("paginator.number_pagers > 1"));
        assert!(source.contains("{% for item in"));
        assert!(source.contains("{% include \"listing-items/service-card.html\" %}"));
        assert!(source.contains("slice(start=0, end=6)"));
    }

    #[test]
    fn source_rewrite_rejects_stale_boundary() {
        let properties = field_properties();
        let source = "abcdefgh";
        let instance = DynamicWidgetSourceInstance {
            id: "source".to_string(),
            instance_id: "dynamic-field-a1b2c3d4".to_string(),
            provider_id: "dynamic-field".to_string(),
            provider_kind: Some(DynamicWidgetProviderKind::DynamicField),
            file: "templates/index.html".to_string(),
            range: source_range(source, 0, source.len()),
            start_marker_range: source_range(source, 0, 1),
            end_marker_range: source_range(source, 7, 8),
            source_node_ids: Vec::new(),
            root_source_node_ids: Vec::new(),
            status: DynamicWidgetResolutionStatus::Resolved,
            properties: Some(properties.clone()),
            canonical_binding_path: None,
            canonical_binding_expression: None,
            source_revision: "stale".to_string(),
            diagnostics: Vec::new(),
        };
        assert!(replace_dynamic_widget_source(
            source,
            &instance,
            &properties,
            &empty_source_graph()
        )
        .unwrap_err()
        .contains("stale"));
    }

    #[test]
    fn listing_contract_renders_and_rewrites_one_stable_source_instance() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "pana-dynamic-listing-{}-{nonce}",
            std::process::id()
        ));
        for directory in [
            "content/services",
            "templates/listing-items",
            ".panastudio/content-models",
        ] {
            fs::create_dir_all(root.join(directory)).unwrap();
        }
        fs::write(
            root.join("zola.toml"),
            "base_url = \"https://example.test\"\ntitle = \"Studio\"\n[extra]\nbrand_name = \"Pană\"\n",
        )
        .unwrap();
        fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = \"Acasă\"\ntemplate = \"index.html\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("content/services/_index.md"),
            "+++\ntitle = \"Servicii\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("content/services/audit.md"),
            "+++\ntitle = \"Audit\"\ndate = 2026-08-03\n[extra]\nsubtitle = \"Audit tehnic\"\nprice = 80\nwebsite = \"https://example.test/audit\"\ncover = \"/images/audit.webp\"\n+++\n",
        )
        .unwrap();
        fs::write(root.join("templates/index.html"), "<main></main>").unwrap();
        fs::write(
            root.join("templates/listing-items/service-card.html"),
            "<article>{{ item.title }}</article>",
        )
        .unwrap();
        fs::write(
            root.join(".panastudio/project.toml"),
            "schema_version = 1\n",
        )
        .unwrap();
        fs::write(
            root.join(".panastudio/assignments.toml"),
            "schema_version = 1\n\n[[assignments]]\nsectionPath = \"content/services/_index.md\"\nmodelId = \"service\"\n",
        )
        .unwrap();
        fs::write(
            root.join(".panastudio/content-models/service.toml"),
            "schemaVersion = 1\nid = \"service\"\nlabel = \"Serviciu\"\n\n[[fields]]\nid = \"field_subtitle\"\nkey = \"subtitle\"\nlabel = \"Subtitlu\"\nkind = \"text\"\n\n[[fields]]\nid = \"field_price\"\nkey = \"price\"\nlabel = \"Preț\"\nkind = \"number\"\n\n[[fields]]\nid = \"field_website\"\nkey = \"website\"\nlabel = \"Website\"\nkind = \"url\"\n\n[[fields]]\nid = \"field_cover\"\nkey = \"cover\"\nlabel = \"Copertă\"\nkind = \"image\"\n",
        )
        .unwrap();
        fs::write(
            root.join(".panastudio/listing-items.toml"),
            "schema_version = 1\n\n[[items]]\nid = \"service-card\"\nlabel = \"Card serviciu\"\ntemplateName = \"listing-items/service-card.html\"\nmodelId = \"service\"\npreviewPageFile = \"content/services/audit.md\"\n",
        )
        .unwrap();

        let graph =
            crate::source_graph::build_source_graph_from_integration_disk_boundary(&root).unwrap();
        let values = dynamic_value_catalog(&graph);
        assert!(values.iter().any(|value| value.id == "site.title"));
        assert!(values.iter().any(|value| {
            value.label == "brand_name"
                && matches!(value.source, DynamicValueSource::ConfigExtra { .. })
        }));
        assert!(values.iter().any(|value| {
            value.model_id.as_deref() == Some("service")
                && matches!(
                    value.source,
                    DynamicValueSource::CustomField { ref field_id, .. }
                        if field_id == "field_price"
                )
        }));
        let dynamic_field =
            |context, source, value_type, presentation, tag: &str, label: &str, format| {
                DynamicWidgetProperties::DynamicField(DynamicFieldWidgetProperties {
                    binding: DynamicValueBinding {
                        context,
                        source,
                        value_type,
                    },
                    presentation,
                    tag: tag.to_string(),
                    format,
                    prefix: String::new(),
                    suffix: String::new(),
                    fallback: String::new(),
                    label: label.to_string(),
                    empty_behavior: DynamicFieldEmptyBehavior::Hide,
                })
            };
        let title = render_dynamic_widget(
            "dynamic-field-titlefixture",
            &dynamic_field(
                DynamicFieldScope::CollectionItem,
                DynamicValueSource::Builtin {
                    field: "title".into(),
                },
                DynamicValueType::Text,
                DynamicFieldPresentation::Heading,
                "h2",
                "",
                DynamicValueFormat::default(),
            ),
            &graph,
        )
        .unwrap();
        let custom_text = render_dynamic_widget(
            "dynamic-field-textfixture",
            &dynamic_field(
                DynamicFieldScope::CollectionItem,
                DynamicValueSource::CustomField {
                    model_id: "service".into(),
                    field_id: "field_subtitle".into(),
                },
                DynamicValueType::Text,
                DynamicFieldPresentation::Paragraph,
                "p",
                "",
                DynamicValueFormat::default(),
            ),
            &graph,
        )
        .unwrap();
        let date = render_dynamic_widget(
            "dynamic-field-datefixture",
            &dynamic_field(
                DynamicFieldScope::CollectionItem,
                DynamicValueSource::Builtin {
                    field: "date".into(),
                },
                DynamicValueType::Date,
                DynamicFieldPresentation::Date,
                "span",
                "",
                DynamicValueFormat {
                    date_format: "%Y-%m-%d".into(),
                    ..Default::default()
                },
            ),
            &graph,
        )
        .unwrap();
        let price = render_dynamic_widget(
            "dynamic-field-pricefixture",
            &dynamic_field(
                DynamicFieldScope::CollectionItem,
                DynamicValueSource::CustomField {
                    model_id: "service".into(),
                    field_id: "field_price".into(),
                },
                DynamicValueType::Number,
                DynamicFieldPresentation::Currency,
                "span",
                "",
                DynamicValueFormat {
                    decimals: Some(2),
                    currency: "RON".into(),
                    ..Default::default()
                },
            ),
            &graph,
        )
        .unwrap();
        let website = render_dynamic_widget(
            "dynamic-field-webfixture",
            &dynamic_field(
                DynamicFieldScope::CollectionItem,
                DynamicValueSource::CustomField {
                    model_id: "service".into(),
                    field_id: "field_website".into(),
                },
                DynamicValueType::Url,
                DynamicFieldPresentation::Link,
                "a",
                "Deschide serviciul",
                DynamicValueFormat::default(),
            ),
            &graph,
        )
        .unwrap();
        let cover = render_dynamic_widget(
            "dynamic-field-coverfixture",
            &dynamic_field(
                DynamicFieldScope::CollectionItem,
                DynamicValueSource::CustomField {
                    model_id: "service".into(),
                    field_id: "field_cover".into(),
                },
                DynamicValueType::Image,
                DynamicFieldPresentation::Image,
                "img",
                "Copertă serviciu",
                DynamicValueFormat::default(),
            ),
            &graph,
        )
        .unwrap();
        let site_title = render_dynamic_widget(
            "dynamic-field-sitefixture",
            &dynamic_field(
                DynamicFieldScope::Site,
                DynamicValueSource::Builtin {
                    field: "title".into(),
                },
                DynamicValueType::Text,
                DynamicFieldPresentation::Text,
                "span",
                "",
                DynamicValueFormat::default(),
            ),
            &graph,
        )
        .unwrap();
        assert!(title.contains("item.title"));
        assert!(custom_text.contains("item.extra.subtitle"));
        assert!(date.contains("item.date | date(format=\"%Y-%m-%d\")"));
        assert!(price.contains("RON {{ item.extra.price | round(precision=2) }}"));
        assert!(website.contains("href=\"{{ item.extra.website }}\""));
        assert!(cover.contains("src=\"{{ item.extra.cover }}\""));
        assert!(site_title.contains("config.title"));
        let listing_item_source = format!(
            "<article>{title}{custom_text}{date}{price}{website}{cover}{site_title}</article>"
        );
        let mut tera = tera::Tera::default();
        tera.add_raw_template("listing-items/service-card.html", &listing_item_source)
            .unwrap();
        let mut context = tera::Context::new();
        context.insert(
            "item",
            &serde_json::json!({
                "title": "Audit",
                "date": "2026-08-03T00:00:00Z",
                "extra": {
                    "subtitle": "Audit tehnic",
                    "price": 80,
                    "website": "https://example.test/audit",
                    "cover": "/images/audit.webp"
                }
            }),
        );
        context.insert("config", &serde_json::json!({ "title": "Studio" }));
        let preview = tera
            .render("listing-items/service-card.html", &context)
            .unwrap();
        assert!(preview.contains("Audit tehnic"));
        assert!(preview.contains("03.08.2026") || preview.contains("2026-08-03"));
        assert!(preview.contains("RON 80.0") || preview.contains("RON 80"));
        assert!(preview.contains("href=") && preview.contains("example.test"));
        assert!(preview.contains("src=") && preview.contains("audit.webp"));
        assert!(preview.contains("Studio"));
        fs::write(
            root.join("templates/listing-items/service-card.html"),
            &listing_item_source,
        )
        .unwrap();
        let properties = DynamicWidgetProperties::Listing(ListingWidgetProperties {
            section_path: "content/services/_index.md".to_string(),
            listing_item_id: "service-card".to_string(),
            listing_item_template: "listing-items/service-card.html".to_string(),
            include_subsections: false,
            sort_by: ListingSortBy::Title,
            sort_order: ListingSortOrder::Asc,
            limit: Some(4),
            offset: 0,
            empty_text: "Nicio intrare".to_string(),
            tag: "section".to_string(),
            class_name: "services".to_string(),
        });
        let rendered = render_dynamic_widget("listing-a1b2c3d4", &properties, &graph).unwrap();
        assert!(rendered.contains("get_section(path=\"services/_index.md\")"));
        assert!(rendered.contains("{% for item in"));
        assert!(rendered.contains("{% include \"listing-items/service-card.html\" %}"));

        let fixture =
            crate::project_model::test_support::ProjectModelTestFixture::from_integration_disk_boundary(
                &root,
            )
            .unwrap();
        let mut projected = fixture.projection().source_texts;
        projected.insert(
            "templates/index.html".to_string(),
            format!("<main>\n{rendered}\n</main>"),
        );
        let dynamic = build_dynamic_widget_graph_from_workspace_projection(
            &root,
            &projected,
            &HashSet::new(),
            &graph,
        );
        assert_eq!(dynamic.source_instances.len(), 8);
        let instance = dynamic
            .source_instances
            .iter()
            .find(|instance| instance.instance_id == "listing-a1b2c3d4")
            .unwrap();
        assert_eq!(instance.status, DynamicWidgetResolutionStatus::Resolved);
        let mut changed = properties.clone();
        let DynamicWidgetProperties::Listing(ref mut listing) = changed else {
            unreachable!()
        };
        listing.limit = Some(2);
        let next = replace_dynamic_widget_source(
            projected.get("templates/index.html").unwrap(),
            instance,
            &changed,
            &graph,
        )
        .unwrap();
        assert!(next.starts_with("<main>\n"));
        assert!(next.ends_with("\n</main>"));
        assert_eq!(next.matches("instance=listing-a1b2c3d4").count(), 2);
        assert!(next.contains("slice(start=0, end=2)"));

        let initial = projected.get("templates/index.html").unwrap();
        fs::write(root.join("templates/index.html"), initial).unwrap();
        let used_graph =
            crate::source_graph::build_source_graph_from_integration_disk_boundary(&root).unwrap();
        let used_item = used_graph
            .listing_items
            .items
            .iter()
            .find(|item| item.id == "service-card")
            .unwrap();
        assert_eq!(used_item.usage_count, 1);
        let mut workspace = test_workspace(&root, "templates/index.html", initial);
        let receipt = workspace
            .stage_resource_texts(
                &workspace_identity(&workspace),
                WorkspaceMutationMetadata {
                    label: "Actualizare Listing".to_string(),
                    source: "dynamic-widgets.update".to_string(),
                    coalesce_key: None,
                    transaction_id: None,
                },
                vec![WorkspaceResourceMutation {
                    relative_path: "templates/index.html".to_string(),
                    contents: next.clone(),
                    create_only: false,
                }],
                2,
            )
            .unwrap();
        assert_eq!(receipt.history.undo_count, 1);
        assert_eq!(
            workspace
                .documents
                .text_for("templates/index.html")
                .as_deref(),
            Some(next.as_str())
        );

        let undo = workspace.undo(&workspace_identity(&workspace), 3).unwrap();
        assert!(matches!(undo.direction, WorkspaceHistoryDirection::Undo));
        assert_eq!(
            workspace
                .documents
                .text_for("templates/index.html")
                .as_deref(),
            Some(initial.as_str())
        );

        let redo = workspace.redo(&workspace_identity(&workspace), 4).unwrap();
        assert!(matches!(redo.direction, WorkspaceHistoryDirection::Redo));
        assert_eq!(
            workspace
                .documents
                .text_for("templates/index.html")
                .as_deref(),
            Some(next.as_str())
        );

        fs::write(root.join("templates/index.html"), &next).unwrap();
        let reopened =
            crate::source_graph::build_source_graph_from_integration_disk_boundary(&root).unwrap();
        assert_eq!(reopened.dynamic_widget_graph.source_instances.len(), 8);
        assert!(reopened
            .dynamic_widget_graph
            .source_instances
            .iter()
            .any(|instance| instance.instance_id == "listing-a1b2c3d4"));
        assert_eq!(
            reopened
                .dynamic_widget_graph
                .source_instances
                .iter()
                .map(|instance| instance.instance_id.as_str())
                .collect::<HashSet<_>>()
                .len(),
            8
        );

        fs::remove_dir_all(root).unwrap();
    }
}
