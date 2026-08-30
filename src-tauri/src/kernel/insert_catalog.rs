use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    blocks::native_block_registry_snapshot,
    kernel::{
        dynamic_widgets::{
            DynamicFieldEmptyBehavior, DynamicFieldPresentation, DynamicFieldScope,
            DynamicFieldWidgetProperties, DynamicValueBinding, DynamicValueFormat,
            DynamicValueSource, DynamicValueType, DynamicWidgetProperties, ListingSortBy,
            ListingSortOrder, ListingWidgetProperties,
        },
        listing_items::{ListingItemDefinition, ListingItemStatus},
    },
    project_model::model::ProjectModel,
    source_graph::model::{
        ComponentDefinition, ComponentDefinitionKind, ComponentOrigin, SourceGraph, SourceNodeKind,
    },
};

pub const INSERT_CATALOG_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsertCatalogContext {
    pub active_document_path: Option<String>,
    pub active_template_path: Option<String>,
    pub active_page_path: Option<String>,
    pub canvas_preview_revision: Option<String>,
    #[serde(default)]
    pub canvas_available: bool,
    pub target_source_id: Option<String>,
    pub target_tag: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsertCatalogSnapshot {
    pub schema_version: u32,
    pub project_root: String,
    pub runtime_session_id: String,
    pub workspace_revision: u64,
    pub model_revision: String,
    pub context: InsertCatalogContext,
    pub groups: Vec<InsertCatalogGroup>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InsertCatalogCategory {
    Html,
    Block,
    Component,
    Tera,
    DynamicWidget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InsertCatalogOrigin {
    Application,
    Native,
    Project,
    Theme,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsertCatalogGroup {
    pub id: String,
    pub category: InsertCatalogCategory,
    pub label: String,
    pub description: String,
    pub items: Vec<InsertCatalogItem>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsertCatalogItem {
    pub id: String,
    pub category: InsertCatalogCategory,
    pub origin: InsertCatalogOrigin,
    pub label: String,
    pub description: String,
    pub capabilities: InsertCatalogCapabilities,
    pub payload: InsertCatalogPayload,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsertCatalogCapabilities {
    pub can_drag: bool,
    pub allowed_positions: Vec<String>,
    pub reason_code: Option<String>,
    pub reason_arguments: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum InsertCatalogPayload {
    Html {
        tag: String,
        class_name: String,
        text: String,
    },
    Block {
        block_id: String,
        block_kind: String,
        tag: String,
        class_name: String,
        text: String,
    },
    Component {
        component_id: String,
        tera_kind: String,
        family: String,
        target: String,
        name: Option<String>,
        expression: Option<String>,
    },
    Tera {
        tera_kind: String,
        family: String,
        target: Option<String>,
        name: Option<String>,
        expression: Option<String>,
    },
    DynamicWidget {
        provider_id: String,
        properties: DynamicWidgetProperties,
    },
}

pub fn build_insert_catalog(
    model: &ProjectModel,
    project_root: String,
    runtime_session_id: String,
    workspace_revision: u64,
    context: InsertCatalogContext,
) -> InsertCatalogSnapshot {
    let base_capability = document_capability(&context);
    let graph = &model.source_graph;
    let groups = vec![
        html_group(&base_capability),
        block_group(graph, &base_capability),
        component_group(graph, &base_capability),
        tera_group(&base_capability),
        dynamic_widget_group(graph, &context, &base_capability),
    ];
    InsertCatalogSnapshot {
        schema_version: INSERT_CATALOG_SCHEMA_VERSION,
        project_root,
        runtime_session_id,
        workspace_revision,
        model_revision: model.revision.clone(),
        context,
        groups,
    }
}

fn document_capability(context: &InsertCatalogContext) -> InsertCatalogCapabilities {
    let active_document = context
        .active_document_path
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let active_template = context
        .active_template_path
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let has_html_document = active_document.ends_with(".html")
        || active_document.ends_with(".htm")
        || active_template.ends_with(".html")
        || active_template.ends_with(".htm");
    if !context.canvas_available {
        return blocked_capability("insert_catalog_canvas_unavailable");
    }
    if !has_html_document {
        return blocked_capability("insert_catalog_document_not_insertable");
    }
    InsertCatalogCapabilities {
        can_drag: true,
        allowed_positions: vec!["before".into(), "inside".into(), "after".into()],
        reason_code: None,
        reason_arguments: BTreeMap::new(),
    }
}

fn blocked_capability(reason_code: &str) -> InsertCatalogCapabilities {
    InsertCatalogCapabilities {
        can_drag: false,
        allowed_positions: Vec::new(),
        reason_code: Some(reason_code.to_string()),
        reason_arguments: BTreeMap::new(),
    }
}

fn merge_capability(
    base: &InsertCatalogCapabilities,
    allowed: bool,
    reason_code: &str,
) -> InsertCatalogCapabilities {
    if !base.can_drag {
        return base.clone();
    }
    if !allowed {
        return blocked_capability(reason_code);
    }
    base.clone()
}

fn html_group(base: &InsertCatalogCapabilities) -> InsertCatalogGroup {
    const TAGS: &[(&str, &str, &str, &str)] = &[
        ("div", "Container", "Element structural <div>.", ""),
        ("section", "Secțiune", "Secțiune semantică <section>.", ""),
        (
            "article",
            "Articol",
            "Element semantic <article>.",
            "Articol nou",
        ),
        ("main", "Conținut principal", "Element semantic <main>.", ""),
        ("header", "Antet", "Element semantic <header>.", ""),
        ("footer", "Subsol", "Element semantic <footer>.", ""),
        ("nav", "Navigație", "Element semantic <nav>.", ""),
        ("aside", "Conținut lateral", "Element semantic <aside>.", ""),
        (
            "address",
            "Date de contact",
            "Informații de contact <address>.",
            "Date de contact",
        ),
        (
            "hgroup",
            "Grup de titluri",
            "Grup semantic <hgroup>.",
            "Titlu grup",
        ),
        ("figure", "Figură", "Element semantic <figure>.", ""),
        (
            "figcaption",
            "Legendă figură",
            "Legendă <figcaption>.",
            "Descriere",
        ),
        ("search", "Căutare", "Regiune semantică <search>.", ""),
        ("hr", "Separator tematic", "Separator <hr>.", ""),
        ("p", "Paragraf", "Text în <p>.", "Paragraf nou."),
        ("h1", "Titlu H1", "Titlu principal <h1>.", "Titlu principal"),
        ("h2", "Titlu H2", "Titlu de nivel doi <h2>.", "Titlu nou"),
        (
            "h3",
            "Titlu H3",
            "Titlu de nivel trei <h3>.",
            "Subtitlu nou",
        ),
        ("h4", "Titlu H4", "Titlu de nivel patru <h4>.", "Titlu nou"),
        ("h5", "Titlu H5", "Titlu de nivel cinci <h5>.", "Titlu nou"),
        ("h6", "Titlu H6", "Titlu de nivel șase <h6>.", "Titlu nou"),
        ("span", "Text în linie", "Text în <span>.", "Text"),
        ("blockquote", "Citat", "Citat <blockquote>.", "Citat nou."),
        ("q", "Citat în linie", "Citat scurt <q>.", "Citat"),
        ("cite", "Sursă citat", "Referință <cite>.", "Sursă"),
        (
            "pre",
            "Text preformatat",
            "Conținut <pre>.",
            "Text preformatat",
        ),
        ("code", "Cod", "Fragment <code>.", "code"),
        (
            "strong",
            "Text important",
            "Text <strong>.",
            "Text important",
        ),
        ("em", "Text accentuat", "Text <em>.", "Text accentuat"),
        ("small", "Text secundar", "Text <small>.", "Text secundar"),
        (
            "mark",
            "Text evidențiat",
            "Evidențiere <mark>.",
            "Text evidențiat",
        ),
        ("abbr", "Abreviere", "Abreviere <abbr>.", "Abreviere"),
        ("dfn", "Termen definit", "Termen <dfn>.", "Termen"),
        ("time", "Dată sau oră", "Valoare temporală <time>.", "Dată"),
        (
            "data",
            "Valoare de date",
            "Valoare lizibilă <data>.",
            "Valoare",
        ),
        ("sub", "Indice inferior", "Text <sub>.", "Indice"),
        ("sup", "Indice superior", "Text <sup>.", "Indice"),
        ("kbd", "Intrare tastatură", "Intrare <kbd>.", "Ctrl+K"),
        ("samp", "Rezultat program", "Rezultat <samp>.", "Rezultat"),
        ("var", "Variabilă", "Variabilă <var>.", "x"),
        ("b", "Atenție vizuală", "Text <b>.", "Text"),
        ("i", "Voce alternativă", "Text <i>.", "Text"),
        ("u", "Adnotare", "Text <u>.", "Text"),
        ("s", "Text nerelevant", "Text <s>.", "Text"),
        ("bdi", "Text izolat", "Izolare bidi <bdi>.", "Text"),
        ("bdo", "Direcție text", "Suprascriere bidi <bdo>.", "Text"),
        ("br", "Rând nou", "Întrerupere de rând <br>.", ""),
        (
            "wbr",
            "Punct de rupere",
            "Posibilitate de rupere <wbr>.",
            "",
        ),
        (
            "ins",
            "Text inserat",
            "Conținut adăugat <ins>.",
            "Text adăugat",
        ),
        (
            "del",
            "Text șters",
            "Conținut eliminat <del>.",
            "Text eliminat",
        ),
        ("ruby", "Adnotare Ruby", "Adnotare <ruby>.", "Text"),
        ("rt", "Pronunție Ruby", "Pronunție <rt>.", "Pronunție"),
        ("rp", "Fallback Ruby", "Fallback <rp>.", "("),
        ("label", "Etichetă", "Etichetă <label>.", "Etichetă"),
        ("ul", "Listă", "Listă neordonată <ul>.", "Element listă"),
        (
            "ol",
            "Listă numerotată",
            "Listă ordonată <ol>.",
            "Element listă",
        ),
        ("li", "Element listă", "Element <li>.", "Element listă"),
        ("dl", "Listă de definiții", "Listă <dl>.", "Termen"),
        ("dt", "Termen", "Termen <dt>.", "Termen"),
        ("dd", "Definiție", "Definiție <dd>.", "Descriere"),
        (
            "menu",
            "Meniu semantic",
            "Listă de comenzi <menu>.",
            "Element meniu",
        ),
        ("img", "Imagine", "Imagine <img>.", "Imagine"),
        (
            "picture",
            "Imagine adaptivă",
            "Container <picture>.",
            "Imagine",
        ),
        ("video", "Video", "Conținut video <video>.", ""),
        ("audio", "Audio", "Conținut audio <audio>.", ""),
        ("source", "Sursă media", "Sursă <source>.", ""),
        ("track", "Pistă media", "Pistă text <track>.", ""),
        (
            "iframe",
            "Pagină încorporată",
            "Document <iframe>.",
            "Pagină încorporată",
        ),
        (
            "canvas",
            "Canvas",
            "Suprafață grafică <canvas>.",
            "Canvas indisponibil",
        ),
        (
            "object",
            "Obiect extern",
            "Resursă <object>.",
            "Conținut indisponibil",
        ),
        ("embed", "Resursă încorporată", "Resursă <embed>.", ""),
        ("map", "Hartă imagine", "Hartă <map>.", ""),
        ("area", "Zonă hartă", "Zonă <area>.", "Zonă"),
        ("a", "Link", "Legătură <a>.", "Link nou"),
        ("button", "Buton", "Control <button>.", "Buton nou"),
        (
            "details",
            "Detalii",
            "Conținut extensibil <details>.",
            "Detalii",
        ),
        (
            "summary",
            "Rezumat detalii",
            "Etichetă <summary>.",
            "Detalii",
        ),
        (
            "dialog",
            "Dialog",
            "Fereastră nativă <dialog>.",
            "Dialog nou",
        ),
        ("form", "Formular", "Formular <form>.", "Trimite"),
        ("input", "Câmp", "Câmp <input>.", "Text"),
        ("textarea", "Text multilinie", "Câmp <textarea>.", "Text"),
        ("select", "Selecție", "Control <select>.", "Opțiune"),
        ("option", "Opțiune", "Opțiune <option>.", "Opțiune"),
        ("optgroup", "Grup de opțiuni", "Grup <optgroup>.", "Grup"),
        ("datalist", "Sugestii câmp", "Listă <datalist>.", "Opțiune"),
        ("fieldset", "Grup formular", "Grup <fieldset>.", "Legendă"),
        ("legend", "Legendă formular", "Legendă <legend>.", "Legendă"),
        (
            "output",
            "Rezultat formular",
            "Rezultat <output>.",
            "Rezultat",
        ),
        ("progress", "Progres", "Indicator <progress>.", "0%"),
        ("meter", "Indicator scalar", "Valoare <meter>.", "0"),
        ("table", "Tabel", "Tabel <table>.", "Celulă"),
        ("colgroup", "Grup de coloane", "Grup <colgroup>.", ""),
        ("col", "Coloană", "Coloană <col>.", ""),
        ("thead", "Antet tabel", "Antet <thead>.", "Antet"),
        ("tbody", "Corp tabel", "Corp <tbody>.", "Celulă"),
        ("tfoot", "Subsol tabel", "Subsol <tfoot>.", "Total"),
        ("tr", "Rând tabel", "Rând <tr>.", "Celulă"),
        ("th", "Celulă antet", "Celulă <th>.", "Antet"),
        ("td", "Celulă", "Celulă <td>.", "Celulă"),
        (
            "caption",
            "Descriere tabel",
            "Descriere <caption>.",
            "Descriere tabel",
        ),
        (
            "template",
            "Șablon HTML",
            "Fragment inert <template>.",
            "Conținut șablon",
        ),
        (
            "slot",
            "Slot componentă",
            "Punct de inserare <slot>.",
            "Conținut implicit",
        ),
    ];
    let items = TAGS
        .iter()
        .map(|(tag, label, description, text)| InsertCatalogItem {
            id: format!("html:{tag}"),
            category: InsertCatalogCategory::Html,
            origin: InsertCatalogOrigin::Application,
            label: (*label).to_string(),
            description: (*description).to_string(),
            capabilities: base.clone(),
            payload: InsertCatalogPayload::Html {
                tag: (*tag).to_string(),
                class_name: if *tag == "button" { "btn" } else { "" }.to_string(),
                text: (*text).to_string(),
            },
        })
        .collect();
    InsertCatalogGroup {
        id: "html".into(),
        category: InsertCatalogCategory::Html,
        label: "HTML".into(),
        description: "Elemente HTML semantice inserabile vizual.".into(),
        items,
    }
}

fn block_group(graph: &SourceGraph, base: &InsertCatalogCapabilities) -> InsertCatalogGroup {
    let native = native_block_registry_snapshot();
    let native_by_id = native
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let mut definitions = graph.block_graph.definitions.iter().collect::<Vec<_>>();
    definitions.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    let items = definitions
        .into_iter()
        .filter_map(|definition| {
            let block = native_by_id.get(definition.provider_id.as_str())?;
            Some(InsertCatalogItem {
                id: format!("block:{}", definition.id),
                category: InsertCatalogCategory::Block,
                origin: InsertCatalogOrigin::Native,
                label: definition.display_name.clone(),
                description: definition.description.clone(),
                capabilities: merge_capability(
                    base,
                    definition.capabilities.can_insert,
                    "insert_catalog_block_not_insertable",
                ),
                payload: InsertCatalogPayload::Block {
                    block_id: block.id.to_string(),
                    block_kind: block.kind.code().to_string(),
                    tag: block.tag.to_string(),
                    class_name: block.class_name.to_string(),
                    text: block.text.to_string(),
                },
            })
        })
        .collect();
    InsertCatalogGroup {
        id: "blocks".into(),
        category: InsertCatalogCategory::Block,
        label: "Blocuri".into(),
        description: "Blocuri proiectate din BlockGraph și NativeBlockRegistry.".into(),
        items,
    }
}

fn component_group(graph: &SourceGraph, base: &InsertCatalogCapabilities) -> InsertCatalogGroup {
    let mut definitions = graph
        .component_graph
        .definitions
        .iter()
        .filter_map(|definition| component_item(definition, base))
        .collect::<Vec<_>>();
    definitions.sort_by(|left, right| left.label.cmp(&right.label));
    InsertCatalogGroup {
        id: "components".into(),
        category: InsertCatalogCategory::Component,
        label: "Componente".into(),
        description: "Partialuri și componente Tera 2 active, reprezentabile în siguranță.".into(),
        items: definitions,
    }
}

fn component_item(
    definition: &ComponentDefinition,
    base: &InsertCatalogCapabilities,
) -> Option<InsertCatalogItem> {
    let origin = match definition.origin {
        ComponentOrigin::Project => InsertCatalogOrigin::Project,
        ComponentOrigin::Theme => InsertCatalogOrigin::Theme,
    };
    match definition.kind {
        ComponentDefinitionKind::Partial => {
            let target = definition.template_name.clone()?;
            Some(InsertCatalogItem {
                id: format!("component:{}", definition.id),
                category: InsertCatalogCategory::Component,
                origin,
                label: definition.display_name.clone(),
                description: format!("Include Tera validat: {target}"),
                capabilities: merge_capability(
                    base,
                    definition.active && definition.shadowed_by.is_none(),
                    "insert_catalog_component_inactive",
                ),
                payload: InsertCatalogPayload::Component {
                    component_id: definition.id.clone(),
                    tera_kind: "include".into(),
                    family: "composition".into(),
                    target,
                    name: None,
                    expression: None,
                },
            })
        }
        ComponentDefinitionKind::TeraComponent => {
            let name = definition
                .symbol
                .clone()
                .or_else(|| Some(definition.name.clone()))?;
            let safe = definition.active
                && definition.shadowed_by.is_none()
                && definition
                    .parameters
                    .iter()
                    .all(|parameter| !parameter.required);
            Some(InsertCatalogItem {
                id: format!("component:{}", definition.id),
                category: InsertCatalogCategory::Component,
                origin,
                label: definition.display_name.clone(),
                description: "Apel de componentă Tera 2.".to_string(),
                capabilities: merge_capability(
                    base,
                    safe,
                    "insert_catalog_component_requires_arguments",
                ),
                payload: InsertCatalogPayload::Component {
                    component_id: definition.id.clone(),
                    tera_kind: "componentCall".into(),
                    family: "reuse".into(),
                    target: name.clone(),
                    name: Some(name),
                    expression: None,
                },
            })
        }
        _ => None,
    }
}

fn tera_group(base: &InsertCatalogCapabilities) -> InsertCatalogGroup {
    let definitions = [
        (
            "extends:base",
            "extends",
            "composition",
            "Extinde layout",
            "Extinde base.html.",
            Some("base.html"),
            None,
            None,
        ),
        (
            "block:content",
            "block",
            "composition",
            "Block",
            "Definește un block Tera.",
            None,
            Some("content"),
            None,
        ),
        (
            "include:partial",
            "include",
            "composition",
            "Include",
            "Include un partial Tera.",
            Some("partials/cta.html"),
            None,
            None,
        ),
        (
            "for:items",
            "for",
            "logic",
            "Buclă",
            "Iterează o colecție.",
            None,
            None,
            Some("item in items"),
        ),
        (
            "if:condition",
            "if",
            "logic",
            "Condiție",
            "Adaugă o condiție Tera.",
            None,
            None,
            Some("condition"),
        ),
        (
            "set:name",
            "set",
            "data",
            "Variabilă",
            "Definește o variabilă Tera.",
            None,
            None,
            Some("name = value"),
        ),
        (
            "variable:value",
            "teraVariable",
            "data",
            "Valoare",
            "Afișează o expresie Tera.",
            None,
            None,
            Some("value"),
        ),
        (
            "component:definition",
            "componentDefinition",
            "reuse",
            "Componentă Tera",
            "Definește o componentă Tera 2.",
            None,
            Some("component"),
            None,
        ),
        (
            "comment:tera",
            "teraComment",
            "safe",
            "Comentariu Tera",
            "Adaugă un comentariu Tera.",
            None,
            None,
            Some("comment"),
        ),
        (
            "raw:tera",
            "raw",
            "safe",
            "Conținut raw",
            "Adaugă un block raw Tera.",
            None,
            None,
            None,
        ),
    ];
    let items = definitions
        .into_iter()
        .map(
            |(id, tera_kind, family, label, description, target, name, expression)| {
                InsertCatalogItem {
                    id: format!("tera:{id}"),
                    category: InsertCatalogCategory::Tera,
                    origin: InsertCatalogOrigin::Application,
                    label: label.into(),
                    description: description.into(),
                    capabilities: base.clone(),
                    payload: InsertCatalogPayload::Tera {
                        tera_kind: tera_kind.into(),
                        family: family.into(),
                        target: target.map(str::to_string),
                        name: name.map(str::to_string),
                        expression: expression.map(str::to_string),
                    },
                }
            },
        )
        .collect();
    InsertCatalogGroup {
        id: "tera".into(),
        category: InsertCatalogCategory::Tera,
        label: "Tera".into(),
        description: "Structuri Tera standard planificate de Rust.".into(),
        items,
    }
}

fn dynamic_widget_group(
    graph: &SourceGraph,
    context: &InsertCatalogContext,
    base: &InsertCatalogCapabilities,
) -> InsertCatalogGroup {
    let target_inside_loop = target_inside_loop(graph, context.target_source_id.as_deref());
    let mut items = Vec::new();
    if let Some(properties) = default_dynamic_field_properties(graph, context) {
        items.push(InsertCatalogItem {
            id: "dynamic-widget:dynamic-field".into(),
            category: InsertCatalogCategory::DynamicWidget,
            origin: InsertCatalogOrigin::Application,
            label: "Câmp dinamic".into(),
            description: "Afișează un câmp al paginii sau al articolului curent; proprietățile rămân editabile în inspector.".into(),
            capabilities: base.clone(),
            payload: InsertCatalogPayload::DynamicWidget {
                provider_id: "dynamic-field".into(),
                properties,
            },
        });
    }
    if let Some(properties) = default_listing_properties(graph) {
        items.push(InsertCatalogItem {
            id: "dynamic-widget:listing".into(),
            category: InsertCatalogCategory::DynamicWidget,
            origin: InsertCatalogOrigin::Application,
            label: "Listing".into(),
            description: "Randă articolele unei secțiuni printr-un Listing Item reutilizabil."
                .into(),
            capabilities: merge_capability(
                base,
                !target_inside_loop,
                "insert_catalog_listing_nested_loop",
            ),
            payload: InsertCatalogPayload::DynamicWidget {
                provider_id: "listing".into(),
                properties,
            },
        });
    }
    InsertCatalogGroup {
        id: "dynamic-widgets".into(),
        category: InsertCatalogCategory::DynamicWidget,
        label: "Widgeturi dinamice".into(),
        description: "Instanțe Tera tipizate, generate și rescrise de Rust.".into(),
        items,
    }
}

fn default_dynamic_field_properties(
    graph: &SourceGraph,
    context: &InsertCatalogContext,
) -> Option<DynamicWidgetProperties> {
    let listing_item = listing_item_for_context(graph, context);
    let inferred_context = if listing_item.is_some() {
        DynamicFieldScope::CollectionItem
    } else if context
        .active_page_path
        .as_deref()
        .is_some_and(|active_page| {
            let active_page = normalize_path(active_page);
            graph.pages.iter().any(|page| {
                normalize_path(&page.file) == active_page
                    && matches!(
                        page.page_kind,
                        crate::source_graph::model::SourcePageKind::Section
                    )
            })
        })
    {
        DynamicFieldScope::Section
    } else {
        DynamicFieldScope::Page
    };
    Some(DynamicWidgetProperties::DynamicField(
        DynamicFieldWidgetProperties {
            binding: DynamicValueBinding {
                context: inferred_context,
                source: DynamicValueSource::Builtin {
                    field: "title".into(),
                },
                value_type: DynamicValueType::Text,
            },
            presentation: DynamicFieldPresentation::Heading,
            tag: "h2".into(),
            format: DynamicValueFormat::default(),
            prefix: String::new(),
            suffix: String::new(),
            fallback: String::new(),
            label: "Titlu".into(),
            empty_behavior: DynamicFieldEmptyBehavior::RenderEmpty,
        },
    ))
}

fn default_listing_properties(graph: &SourceGraph) -> Option<DynamicWidgetProperties> {
    let (item, section_path) = graph
        .listing_items
        .items
        .iter()
        .filter(|item| item.status == ListingItemStatus::Resolved)
        .find_map(|item| {
            item.compatible_section_paths
                .first()
                .map(|section| (item, section.clone()))
        })?;
    Some(DynamicWidgetProperties::Listing(ListingWidgetProperties {
        section_path,
        listing_item_id: item.id.clone(),
        listing_item_template: item.template_name.clone(),
        include_subsections: false,
        sort_by: ListingSortBy::None,
        sort_order: ListingSortOrder::Asc,
        limit: None,
        offset: 0,
        empty_text: "Nu există articole.".into(),
        tag: "div".into(),
        class_name: String::new(),
    }))
}

fn listing_item_for_context<'a>(
    graph: &'a SourceGraph,
    context: &InsertCatalogContext,
) -> Option<&'a ListingItemDefinition> {
    let active_file = context
        .active_template_path
        .as_deref()
        .or(context.active_document_path.as_deref())
        .map(normalize_path)?;
    graph.listing_items.items.iter().find(|item| {
        normalize_path(&item.file) == active_file
            || normalize_path(&item.template_name) == active_file.trim_start_matches("templates/")
    })
}

fn target_inside_loop(graph: &SourceGraph, target_source_id: Option<&str>) -> bool {
    let Some(target_source_id) = target_source_id else {
        return false;
    };
    let mut current = graph.nodes.iter().find(|node| node.id == target_source_id);
    let mut visited = BTreeSet::new();
    while let Some(node) = current {
        if !visited.insert(node.id.as_str()) {
            break;
        }
        if node.kind == SourceNodeKind::For {
            return true;
        }
        current = node
            .parent
            .as_deref()
            .and_then(|parent| graph.nodes.iter().find(|node| node.id == parent));
    }
    false
}

fn normalize_path(path: &str) -> String {
    path.trim()
        .trim_start_matches("./")
        .trim_start_matches('/')
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draggable() -> InsertCatalogCapabilities {
        InsertCatalogCapabilities {
            can_drag: true,
            allowed_positions: vec!["before".into(), "inside".into(), "after".into()],
            reason_code: None,
            reason_arguments: BTreeMap::new(),
        }
    }

    #[test]
    fn html_catalog_covers_authorable_body_elements() {
        let group = html_group(&draggable());
        assert!(group.items.iter().any(|item| item.id == "html:section"));
        for required in [
            "h6", "hgroup", "address", "search", "hr", "details", "summary", "dialog", "mark",
            "dfn", "time", "b", "i", "u", "s", "bdi", "bdo", "br", "wbr", "ins", "del", "ruby",
            "rt", "rp", "menu", "img", "picture", "video", "audio", "source", "track", "iframe",
            "canvas", "object", "embed", "map", "area", "optgroup", "datalist", "progress",
            "meter", "colgroup", "col", "template", "slot",
        ] {
            assert!(
                group
                    .items
                    .iter()
                    .any(|item| item.id == format!("html:{required}")),
                "elementul HTML util {required} trebuie proiectat de Rust"
            );
        }
        assert_eq!(
            group
                .items
                .iter()
                .map(|item| item.id.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
            group.items.len(),
            "fiecare element HTML trebuie să aibă o singură identitate în catalog"
        );
        for excluded in [
            "html", "head", "body", "base", "link", "meta", "title", "style", "script",
        ] {
            assert!(!group
                .items
                .iter()
                .any(|item| item.id == format!("html:{excluded}")));
        }
    }

    #[test]
    fn document_capability_requires_canvas_and_html_context() {
        let unavailable = document_capability(&InsertCatalogContext {
            active_document_path: Some("templates/index.html".into()),
            canvas_available: false,
            ..Default::default()
        });
        assert!(!unavailable.can_drag);
        assert_eq!(
            unavailable.reason_code.as_deref(),
            Some("insert_catalog_canvas_unavailable")
        );
        let available = document_capability(&InsertCatalogContext {
            active_document_path: Some("templates/index.html".into()),
            canvas_available: true,
            ..Default::default()
        });
        assert!(available.can_drag);
    }

    #[test]
    fn discriminated_payload_serializes_camel_case_contract() {
        let item = InsertCatalogItem {
            id: "block:native:counter".into(),
            category: InsertCatalogCategory::Block,
            origin: InsertCatalogOrigin::Native,
            label: "Contor".into(),
            description: String::new(),
            capabilities: draggable(),
            payload: InsertCatalogPayload::Block {
                block_id: "counter".into(),
                block_kind: "js".into(),
                tag: "span".into(),
                class_name: "counter".into(),
                text: "0".into(),
            },
        };
        let value = serde_json::to_value(item).unwrap();
        assert_eq!(value["category"], "block");
        assert_eq!(value["origin"], "native");
        assert_eq!(value["payload"]["kind"], "block");
        assert_eq!(value["payload"]["blockId"], "counter");
        assert_eq!(value["payload"]["className"], "counter");
    }
}
