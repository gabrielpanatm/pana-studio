use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

use serde::{Deserialize, Serialize};

use crate::project_model::attribute_engine::raw_tag_attributes;

pub const ICON_CATALOG_SCHEMA_VERSION: u32 = 1;
pub const ICON_PACK_ID: &str = "tabler-outline";
pub const ICON_PACK_VERSION: &str = "3.41.1";
pub const DEFAULT_ICON_ID: &str = "home";
const DEFAULT_PAGE_LIMIT: usize = 48;
const MAX_PAGE_LIMIT: usize = 96;
const MAX_QUERY_BYTES: usize = 128;
const ICON_REGISTRY_JSON: &str =
    include_str!("../../resources/icon-packs/tabler-outline-3.41.1.json");

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IconRegistryResource {
    schema_version: u32,
    pack_id: String,
    pack_version: String,
    license: String,
    icons: BTreeMap<String, IconResource>,
}

#[derive(Clone, Debug, Deserialize)]
struct IconResource {
    category: String,
    tags: Vec<String>,
    nodes: Vec<(String, BTreeMap<String, String>)>,
}

#[derive(Clone, Debug)]
struct IconRegistry {
    license: String,
    icons: BTreeMap<String, IconResource>,
    categories: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IconCatalogSummary {
    pub schema_version: u32,
    pub pack_id: &'static str,
    pub pack_version: &'static str,
    pub license: String,
    pub total: usize,
    pub categories: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IconCatalogSearchInput {
    #[serde(default)]
    pub query: String,
    pub category: Option<String>,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IconCatalogPage {
    pub schema_version: u32,
    pub pack_id: &'static str,
    pub pack_version: &'static str,
    pub query: String,
    pub category: Option<String>,
    pub offset: usize,
    pub limit: usize,
    pub total: usize,
    pub has_more: bool,
    pub items: Vec<IconCatalogItem>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IconCatalogItem {
    pub id: String,
    pub label: String,
    pub category: String,
    pub tags: Vec<String>,
    pub nodes: Vec<IconCatalogNode>,
}

#[derive(Clone, Debug, Serialize)]
pub struct IconCatalogNode {
    pub tag: String,
    pub attributes: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeIconMutationIntent {
    pub icon_identity: String,
    pub size: u16,
    pub stroke_width: String,
    #[serde(default = "default_decorative")]
    pub decorative: bool,
    pub accessible_label: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeIconState {
    pub icon_identity: String,
    pub pack_id: String,
    pub icon_id: String,
    pub size: u16,
    pub stroke_width: String,
    pub decorative: bool,
    pub accessible_label: Option<String>,
}

pub(crate) struct PlannedNativeIconMutation {
    pub state: NativeIconState,
    pub attributes: BTreeMap<String, Option<String>>,
    pub children_html: String,
}

pub fn read_icon_catalog() -> Result<IconCatalogSummary, String> {
    let registry = registry()?;
    Ok(IconCatalogSummary {
        schema_version: ICON_CATALOG_SCHEMA_VERSION,
        pack_id: ICON_PACK_ID,
        pack_version: ICON_PACK_VERSION,
        license: registry.license.clone(),
        total: registry.icons.len(),
        categories: registry.categories.clone(),
    })
}

pub fn search_icon_catalog(input: IconCatalogSearchInput) -> Result<IconCatalogPage, String> {
    let registry = registry()?;
    let query = normalize_query(&input.query)?;
    let category = normalize_category(input.category.as_deref())?;
    let offset = input.offset.unwrap_or(0);
    let limit = input
        .limit
        .unwrap_or(DEFAULT_PAGE_LIMIT)
        .clamp(1, MAX_PAGE_LIMIT);

    let mut matches = registry
        .icons
        .iter()
        .filter(|(_, icon)| {
            category
                .as_deref()
                .is_none_or(|value| icon.category.eq_ignore_ascii_case(value))
        })
        .filter_map(|(id, icon)| icon_search_rank(id, icon, &query).map(|rank| (rank, id, icon)))
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(right.1)));

    let total = matches.len();
    let items = matches
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|(_, id, icon)| IconCatalogItem {
            id: id.clone(),
            label: icon_label(id),
            category: icon.category.clone(),
            tags: icon.tags.clone(),
            nodes: icon
                .nodes
                .iter()
                .map(|(tag, attributes)| IconCatalogNode {
                    tag: tag.clone(),
                    attributes: attributes.clone(),
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let has_more = offset.saturating_add(items.len()) < total;

    Ok(IconCatalogPage {
        schema_version: ICON_CATALOG_SCHEMA_VERSION,
        pack_id: ICON_PACK_ID,
        pack_version: ICON_PACK_VERSION,
        query,
        category,
        offset,
        limit,
        total,
        has_more,
        items,
    })
}

pub fn icon_exists(icon_id: &str) -> bool {
    normalize_icon_id(icon_id)
        .ok()
        .and_then(|id| registry().ok().and_then(|registry| registry.icons.get(id)))
        .is_some()
}

pub fn normalize_icon_identity(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    let Some((pack_id, icon_id)) = trimmed.split_once(':') else {
        return Err(
            "Identitatea iconului trebuie să aibă forma tabler-outline:<icon_id>.".to_string(),
        );
    };
    if pack_id != ICON_PACK_ID {
        return Err(format!("Pack de iconuri necunoscut: {pack_id}."));
    }
    let icon_id = normalize_icon_id(icon_id)?;
    if !icon_exists(icon_id) {
        return Err(format!("Iconul `{icon_id}` nu există în {ICON_PACK_ID}."));
    }
    Ok(format!("{ICON_PACK_ID}:{icon_id}"))
}

pub fn render_icon_block_html(
    icon_id: &str,
    class_name: &str,
    data_anim: &str,
    instance_id: &str,
) -> Result<String, String> {
    let icon_id = normalize_icon_id(icon_id)?;
    let registry = registry()?;
    let icon = registry
        .icons
        .get(icon_id)
        .ok_or_else(|| format!("Iconul `{icon_id}` nu există în {ICON_PACK_ID}."))?;
    let children = render_icon_children(icon)?;
    Ok(format!(
        "<svg class=\"icon {}\" data-anim=\"{}\" data-pana-block=\"icon\" data-pana-instance=\"{}\" data-pana-icon=\"{}:{}\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\" width=\"24\" height=\"24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" aria-hidden=\"true\" focusable=\"false\">{children}</svg>",
        escape_attribute(class_name),
        escape_attribute(data_anim),
        escape_attribute(instance_id),
        ICON_PACK_ID,
        icon_id,
    ))
}

pub fn render_icon_children_by_identity(identity: &str) -> Result<String, String> {
    let identity = normalize_icon_identity(identity)?;
    let (_, icon_id) = identity.split_once(':').expect("identitate normalizată");
    let icon = registry()?
        .icons
        .get(icon_id)
        .ok_or_else(|| format!("Iconul `{icon_id}` lipsește din registru."))?;
    render_icon_children(icon)
}

pub fn inspect_native_icon_source(opening_tag: &str) -> Result<Option<NativeIconState>, String> {
    let attributes = raw_tag_attributes(opening_tag)
        .into_iter()
        .filter_map(|attribute| {
            attribute
                .value
                .map(|value| (attribute.name, decode_icon_attribute_value(&value)))
        })
        .collect::<BTreeMap<_, _>>();
    if attributes.get("data-pana-block").map(String::as_str) != Some("icon") {
        return Ok(None);
    }
    let icon_identity = attributes
        .get("data-pana-icon")
        .ok_or_else(|| "Block-ul Icon nu are identitatea data-pana-icon.".to_string())?;
    let icon_identity = normalize_icon_identity(icon_identity)?;
    for (name, expected) in [
        ("xmlns", "http://www.w3.org/2000/svg"),
        ("viewbox", "0 0 24 24"),
        ("fill", "none"),
        ("stroke", "currentColor"),
        ("stroke-linecap", "round"),
        ("stroke-linejoin", "round"),
        ("focusable", "false"),
    ] {
        if attributes.get(name).map(String::as_str) != Some(expected) {
            return Err(format!(
                "Block-ul Icon cere atributul canonic {name}=\"{expected}\"."
            ));
        }
    }
    let (pack_id, icon_id) = icon_identity
        .split_once(':')
        .expect("identitate normalizată");
    let pack_id = pack_id.to_string();
    let icon_id = icon_id.to_string();
    let width = parse_size(attributes.get("width").map(String::as_str).unwrap_or("24"))?;
    let height = parse_size(attributes.get("height").map(String::as_str).unwrap_or("24"))?;
    if width != height {
        return Err("Block-ul Icon cere aceeași dimensiune pentru width și height.".to_string());
    }
    let stroke_width = normalize_stroke_width(
        attributes
            .get("stroke-width")
            .map(String::as_str)
            .unwrap_or("2"),
    )?;
    let decorative = attributes
        .get("aria-hidden")
        .is_some_and(|value| value == "true");
    let accessible_label = attributes
        .get("aria-label")
        .map(|value| normalize_accessible_label(value))
        .transpose()?
        .flatten();
    if !decorative {
        if attributes.contains_key("aria-hidden") {
            return Err(
                "Iconul semantic nu poate păstra atributul decorativ aria-hidden.".to_string(),
            );
        }
        if attributes.get("role").map(String::as_str) != Some("img") {
            return Err("Iconul semantic trebuie să declare role=\"img\".".to_string());
        }
        if accessible_label.is_none() {
            return Err("Iconul semantic trebuie să aibă aria-label.".to_string());
        }
    } else if attributes.contains_key("role") || attributes.contains_key("aria-label") {
        return Err("Iconul decorativ nu poate păstra role sau aria-label semantic.".to_string());
    }
    Ok(Some(NativeIconState {
        icon_identity,
        pack_id,
        icon_id,
        size: width,
        stroke_width,
        decorative,
        accessible_label,
    }))
}

pub(crate) fn plan_native_icon_mutation(
    opening_tag: &str,
    intent: &NativeIconMutationIntent,
) -> Result<PlannedNativeIconMutation, String> {
    let _current = inspect_native_icon_source(opening_tag)?
        .ok_or_else(|| "Ținta nu este un block Icon canonic.".to_string())?;
    let icon_identity = normalize_icon_identity(&intent.icon_identity)?;
    let (pack_id, icon_id) = icon_identity
        .split_once(':')
        .expect("identitate normalizată");
    let pack_id = pack_id.to_string();
    let icon_id = icon_id.to_string();
    if !(8..=512).contains(&intent.size) {
        return Err("Dimensiunea iconului trebuie să fie între 8 și 512 px.".to_string());
    }
    let stroke_width = normalize_stroke_width(&intent.stroke_width)?;
    let accessible_label =
        normalize_accessible_label(intent.accessible_label.as_deref().unwrap_or(""))?;
    if !intent.decorative && accessible_label.is_none() {
        return Err("Iconul semantic trebuie să aibă o etichetă accesibilă.".to_string());
    }
    let mut attributes = BTreeMap::from([
        ("data-pana-icon".to_string(), Some(icon_identity.clone())),
        (
            "xmlns".to_string(),
            Some("http://www.w3.org/2000/svg".to_string()),
        ),
        ("viewBox".to_string(), Some("0 0 24 24".to_string())),
        ("width".to_string(), Some(intent.size.to_string())),
        ("height".to_string(), Some(intent.size.to_string())),
        ("fill".to_string(), Some("none".to_string())),
        ("stroke".to_string(), Some("currentColor".to_string())),
        ("stroke-width".to_string(), Some(stroke_width.clone())),
        ("stroke-linecap".to_string(), Some("round".to_string())),
        ("stroke-linejoin".to_string(), Some("round".to_string())),
        ("focusable".to_string(), Some("false".to_string())),
    ]);
    if intent.decorative {
        attributes.insert("aria-hidden".to_string(), Some("true".to_string()));
        attributes.insert("role".to_string(), None);
        attributes.insert("aria-label".to_string(), None);
    } else {
        attributes.insert("aria-hidden".to_string(), None);
        attributes.insert("role".to_string(), Some("img".to_string()));
        attributes.insert("aria-label".to_string(), accessible_label.clone());
    }
    let children_html = render_icon_children_by_identity(&icon_identity)?;
    Ok(PlannedNativeIconMutation {
        state: NativeIconState {
            icon_identity,
            pack_id,
            icon_id,
            size: intent.size,
            stroke_width,
            decorative: intent.decorative,
            accessible_label,
        },
        attributes,
        children_html,
    })
}

fn registry() -> Result<&'static IconRegistry, String> {
    static REGISTRY: OnceLock<Result<IconRegistry, String>> = OnceLock::new();
    REGISTRY
        .get_or_init(load_registry)
        .as_ref()
        .map_err(Clone::clone)
}

fn load_registry() -> Result<IconRegistry, String> {
    let resource: IconRegistryResource = serde_json::from_str(ICON_REGISTRY_JSON)
        .map_err(|error| format!("Registrul Tabler Outline este invalid: {error}"))?;
    if resource.schema_version != ICON_CATALOG_SCHEMA_VERSION
        || resource.pack_id != ICON_PACK_ID
        || resource.pack_version != ICON_PACK_VERSION
        || resource.license != "MIT"
    {
        return Err(
            "Identitatea registrului Tabler Outline nu corespunde contractului compilat."
                .to_string(),
        );
    }
    if resource.icons.len() != 5_039 || !resource.icons.contains_key(DEFAULT_ICON_ID) {
        return Err("Registrul Tabler Outline este incomplet.".to_string());
    }
    let mut categories = BTreeSet::new();
    for (id, icon) in &resource.icons {
        normalize_icon_id(id)?;
        if icon.category.trim().is_empty()
            || icon.category.len() > 80
            || icon.category.chars().any(char::is_control)
            || icon.tags.len() > 64
            || icon.tags.iter().any(|tag| {
                tag.trim().is_empty() || tag.len() > 80 || tag.chars().any(char::is_control)
            })
            || icon.nodes.is_empty()
            || icon.nodes.len() > 32
        {
            return Err(format!("Metadata invalidă pentru iconul `{id}`."));
        }
        categories.insert(icon.category.clone());
        for (tag, attributes) in &icon.nodes {
            validate_node(id, tag, attributes)?;
        }
    }
    Ok(IconRegistry {
        license: resource.license,
        icons: resource.icons,
        categories: categories.into_iter().collect(),
    })
}

fn validate_node(id: &str, tag: &str, attributes: &BTreeMap<String, String>) -> Result<(), String> {
    if tag != "path" || attributes.is_empty() {
        return Err(format!("Nod SVG nepermis în iconul `{id}`."));
    }
    for (name, value) in attributes {
        let allowed = match name.as_str() {
            "d" => {
                !value.is_empty()
                    && value.len() <= 8_192
                    && value.chars().all(is_safe_path_character)
            }
            "fill" => value == "currentColor",
            "stroke" => value == "none",
            "opacity" => value == ".5",
            _ => false,
        };
        if !allowed {
            return Err(format!("Atribut SVG nepermis `{name}` în iconul `{id}`."));
        }
    }
    Ok(())
}

fn is_safe_path_character(character: char) -> bool {
    character.is_ascii_digit()
        || character.is_ascii_whitespace()
        || matches!(
            character,
            'M' | 'm'
                | 'A'
                | 'a'
                | 'C'
                | 'c'
                | 'H'
                | 'h'
                | 'L'
                | 'l'
                | 'Q'
                | 'q'
                | 'S'
                | 's'
                | 'T'
                | 't'
                | 'V'
                | 'v'
                | 'Z'
                | 'z'
                | '+'
                | '-'
                | '.'
                | ','
        )
}

fn normalize_icon_id(value: &str) -> Result<&str, String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 96
        || value.starts_with('-')
        || value.ends_with('-')
        || value
            .bytes()
            .any(|byte| !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'))
        || value.contains("--")
    {
        return Err("ID de icon invalid.".to_string());
    }
    Ok(value)
}

fn default_decorative() -> bool {
    true
}

fn parse_size(value: &str) -> Result<u16, String> {
    value
        .trim()
        .parse::<u16>()
        .ok()
        .filter(|size| (8..=512).contains(size))
        .ok_or_else(|| "Dimensiunea iconului trebuie să fie între 8 și 512 px.".to_string())
}

fn normalize_stroke_width(value: &str) -> Result<String, String> {
    let parsed = value
        .trim()
        .parse::<f32>()
        .map_err(|_| "Grosimea liniei iconului este invalidă.".to_string())?;
    if !parsed.is_finite() || !(0.5..=4.0).contains(&parsed) {
        return Err("Grosimea liniei iconului trebuie să fie între 0.5 și 4.".to_string());
    }
    let normalized = format!("{parsed:.2}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string();
    Ok(normalized)
}

fn normalize_accessible_label(value: &str) -> Result<Option<String>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > 160 || value.chars().any(|character| character.is_control()) {
        return Err(
            "Eticheta accesibilă acceptă maximum 160 de caractere fără controale.".to_string(),
        );
    }
    Ok(Some(value.to_string()))
}

fn normalize_query(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.len() > MAX_QUERY_BYTES {
        return Err(format!(
            "Căutarea iconurilor acceptă cel mult {MAX_QUERY_BYTES} bytes."
        ));
    }
    if value.chars().any(|character| character.is_control()) {
        return Err("Căutarea iconurilor conține caractere de control.".to_string());
    }
    Ok(value.to_ascii_lowercase())
}

fn normalize_category(value: Option<&str>) -> Result<Option<String>, String> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if value.len() > 80 || value.chars().any(|character| character.is_control()) {
        return Err("Categoria iconurilor este invalidă.".to_string());
    }
    let registry = registry()?;
    let category = registry
        .categories
        .iter()
        .find(|candidate| candidate.eq_ignore_ascii_case(value))
        .cloned()
        .ok_or_else(|| format!("Categoria de iconuri `{value}` nu există."))?;
    Ok(Some(category))
}

fn icon_search_rank(id: &str, icon: &IconResource, query: &str) -> Option<u8> {
    if query.is_empty() {
        return Some(3);
    }
    if id == query {
        return Some(0);
    }
    if id.starts_with(query) {
        return Some(1);
    }
    if id.contains(query)
        || icon.category.to_ascii_lowercase().contains(query)
        || icon
            .tags
            .iter()
            .any(|tag| tag.to_ascii_lowercase().contains(query))
    {
        return Some(2);
    }
    None
}

fn icon_label(id: &str) -> String {
    id.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_icon_children(icon: &IconResource) -> Result<String, String> {
    let mut output = String::new();
    for (tag, attributes) in &icon.nodes {
        validate_node("render", tag, attributes)?;
        output.push('<');
        output.push_str(tag);
        for (name, value) in attributes {
            output.push(' ');
            output.push_str(name);
            output.push_str("=\"");
            output.push_str(&escape_attribute(value));
            output.push('"');
        }
        output.push_str("></");
        output.push_str(tag);
        output.push('>');
    }
    Ok(output)
}

fn escape_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub(crate) fn decode_icon_attribute_value(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_registry_is_complete_and_safe() {
        let summary = read_icon_catalog().expect("registry");
        assert_eq!(summary.pack_id, "tabler-outline");
        assert_eq!(summary.pack_version, "3.41.1");
        assert_eq!(summary.total, 5_039);
        assert!(summary
            .categories
            .iter()
            .any(|category| category == "Arrows"));
    }

    #[test]
    fn search_is_bounded_paged_and_deterministic() {
        let input = IconCatalogSearchInput {
            query: "home".to_string(),
            category: None,
            offset: Some(0),
            limit: Some(10_000),
        };
        let first = search_icon_catalog(input.clone()).expect("search");
        let second = search_icon_catalog(input).expect("search");
        assert_eq!(first.limit, MAX_PAGE_LIMIT);
        assert_eq!(first.items[0].id, "home");
        assert_eq!(
            first.items.iter().map(|item| &item.id).collect::<Vec<_>>(),
            second.items.iter().map(|item| &item.id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn renderer_emits_only_managed_svg_paths() {
        let html = render_icon_block_html("home", "ps-icon-test", "ps-icon-test", "icon-test")
            .expect("render");
        assert!(html.starts_with("<svg class=\"icon ps-icon-test\""));
        assert!(html.contains("data-pana-block=\"icon\""));
        assert!(html.contains("data-pana-icon=\"tabler-outline:home\""));
        assert!(html.contains("<path d=\""));
        assert!(!html.contains("<script"));
        assert!(!html.contains("onload="));
        assert!(!html.contains("href="));
    }

    #[test]
    fn malicious_or_unknown_identities_are_rejected() {
        for value in [
            "tabler-outline:<script>",
            "tabler-outline:home--x",
            "foreign:home",
            "home",
        ] {
            assert!(normalize_icon_identity(value).is_err(), "{value}");
        }
    }

    #[test]
    fn icon_mutation_is_typed_and_preserves_only_managed_attributes() {
        let opening = concat!(
            "<svg class=\"custom\" style=\"color:red\" data-pana-block=\"icon\" ",
            "data-pana-icon=\"tabler-outline:home\" width=\"24\" height=\"24\" ",
            "xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\" fill=\"none\" ",
            "stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" ",
            "stroke-linejoin=\"round\" focusable=\"false\" aria-hidden=\"true\">",
        );
        let plan = plan_native_icon_mutation(
            opening,
            &NativeIconMutationIntent {
                icon_identity: "tabler-outline:star".to_string(),
                size: 32,
                stroke_width: "1.5".to_string(),
                decorative: false,
                accessible_label: Some("Favorite".to_string()),
            },
        )
        .expect("plan");
        assert_eq!(plan.state.icon_id, "star");
        assert_eq!(plan.attributes["width"].as_deref(), Some("32"));
        assert_eq!(plan.attributes["role"].as_deref(), Some("img"));
        assert_eq!(plan.attributes["aria-label"].as_deref(), Some("Favorite"));
        assert!(!plan.attributes.contains_key("class"));
        assert!(!plan.attributes.contains_key("style"));
        assert!(plan.children_html.contains("<path"));
    }

    #[test]
    fn icon_accessible_label_is_decoded_before_round_trip() {
        let opening = concat!(
            "<svg data-pana-block=\"icon\" data-pana-icon=\"tabler-outline:home\" ",
            "width=\"24\" height=\"24\" stroke-width=\"2\" role=\"img\" ",
            "xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\" fill=\"none\" ",
            "stroke=\"currentColor\" stroke-linecap=\"round\" stroke-linejoin=\"round\" ",
            "focusable=\"false\" aria-label=\"Favorite &amp; More &quot;quoted&quot;\">",
        );
        let state = inspect_native_icon_source(opening)
            .expect("inspect")
            .expect("icon state");
        assert_eq!(
            state.accessible_label.as_deref(),
            Some("Favorite & More \"quoted\"")
        );
    }
}
