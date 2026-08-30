use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    blocks::{
        inspect_native_icon_source, plan_native_block_option_attribute, NativeBlockOptionIntent,
        NativeIconMutationIntent, NativeIconState,
    },
    project_model::model::{ProjectModel, ProjectModelFile, ProjectModelFileKind},
    source_graph::model::SourceNode,
};

use super::html_editor_schema::validate_visual_attribute_mutation;
use super::move_engine::{
    content_revision, html_tag_at, parse_html_tag_at, resolve_html_node_for_anchor,
    same_model_path, source_location_at_offset, source_missing_message, ProjectSourceEditLocation,
};
use super::zola_image_engine::{
    apply_zola_image_contract, inspect_zola_image_at, ProjectZolaImageIntent, ZolaImagePresentation,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlAttributeIntent {
    pub target_source_id: Option<String>,
    pub target_tag: Option<String>,
    #[serde(default)]
    pub attributes: Vec<ProjectHtmlAttributeMutation>,
    #[serde(default)]
    pub zola_image: Option<ProjectZolaImageIntent>,
    #[serde(default)]
    pub native_block_option: Option<NativeBlockOptionIntent>,
    #[serde(default)]
    pub native_icon: Option<NativeIconMutationIntent>,
    #[serde(default)]
    pub generated_identity: Option<ProjectGeneratedIdentityIntent>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectGeneratedIdentityKind {
    Class,
    DataAnim,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectGeneratedIdentityIntent {
    pub kind: ProjectGeneratedIdentityKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectGeneratedIdentityProjection {
    pub kind: ProjectGeneratedIdentityKind,
    pub value: String,
    pub classes: Vec<String>,
    pub data_anim: Option<String>,
    pub already_present: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProjectHtmlAttributeMutation {
    SetAttribute { name: String, value: String },
    RemoveAttribute { name: String },
}

impl ProjectHtmlAttributeMutation {
    #[cfg(test)]
    pub(crate) fn set(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self::SetAttribute {
            name: name.into(),
            value: value.into(),
        }
    }

    #[cfg(test)]
    pub(crate) fn remove(name: impl Into<String>) -> Self {
        Self::RemoveAttribute { name: name.into() }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlAttributePlan {
    pub allowed: bool,
    pub diagnostic: Option<String>,
    pub model_revision: String,
    pub patch: Option<ProjectHtmlAttributePatch>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlAttributePatch {
    pub file: String,
    pub resolved_target_id: String,
    pub before_revision: String,
    pub after_revision: String,
    pub contents: String,
    pub target_location: ProjectSourceEditLocation,
    pub source_start_line: usize,
    pub tag: String,
    pub attributes: BTreeMap<String, Option<String>>,
    pub zola_image_contract: bool,
    pub zola_image: Option<ZolaImagePresentation>,
    pub managed_icon: Option<ProjectManagedIconPatch>,
    pub generated_identity: Option<ProjectGeneratedIdentityProjection>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectManagedIconPatch {
    pub state: NativeIconState,
    pub previous_state: NativeIconState,
    pub previous_attributes: BTreeMap<String, Option<String>>,
    pub children_html: String,
    pub previous_children_html: String,
}

struct AttributeApplication {
    contents: String,
    target_location: ProjectSourceEditLocation,
    source_start_line: usize,
}

struct GeneratedIdentityAttributeResolution {
    attributes: BTreeMap<String, Option<String>>,
    projection: Option<ProjectGeneratedIdentityProjection>,
}

pub fn plan_html_attributes(
    model: &ProjectModel,
    intent: &ProjectHtmlAttributeIntent,
) -> ProjectHtmlAttributePlan {
    match plan_html_attributes_inner(model, intent) {
        Ok(patch) => ProjectHtmlAttributePlan {
            allowed: true,
            diagnostic: None,
            model_revision: model.revision.clone(),
            patch: Some(patch),
        },
        Err(message) => ProjectHtmlAttributePlan {
            allowed: false,
            diagnostic: Some(message),
            model_revision: model.revision.clone(),
            patch: None,
        },
    }
}

fn plan_html_attributes_inner(
    model: &ProjectModel,
    intent: &ProjectHtmlAttributeIntent,
) -> Result<ProjectHtmlAttributePatch, String> {
    let specialized_contracts = usize::from(intent.zola_image.is_some())
        + usize::from(intent.native_block_option.is_some())
        + usize::from(intent.native_icon.is_some())
        + usize::from(intent.generated_identity.is_some());
    if specialized_contracts > 1 {
        return Err(
            "O intenție de atribute poate aplica un singur contract specializat.".to_string(),
        );
    }
    if specialized_contracts > 0 && !intent.attributes.is_empty() {
        return Err(
            "Contractele specializate nu pot fi combinate cu mutații HTML generice.".to_string(),
        );
    }
    let attributes = if specialized_contracts > 0 {
        BTreeMap::new()
    } else {
        normalize_attribute_mutations(&intent.attributes)?
    };

    if let Some(target_node) = resolve_html_node_for_anchor(
        model,
        intent.target_source_id.as_deref(),
        intent.target_tag.as_deref(),
    ) {
        return plan_html_attributes_from_source_node(model, intent, target_node, attributes);
    }

    Err(source_missing_message(
        "țintă de atribute",
        intent.target_source_id.as_deref(),
    ))
}

fn plan_html_attributes_from_source_node(
    model: &ProjectModel,
    intent: &ProjectHtmlAttributeIntent,
    target_node: &SourceNode,
    attributes: BTreeMap<String, Option<String>>,
) -> Result<ProjectHtmlAttributePatch, String> {
    if !target_node.capabilities.can_edit_visual {
        return Err(target_node
            .capabilities
            .technical_reason()
            .map(str::to_string)
            .unwrap_or_else(|| "Ținta nu este editabilă vizual.".to_string()));
    }

    let file = model
        .files
        .iter()
        .find(|file| same_model_path(&file.relative_path, &target_node.file))
        .ok_or_else(|| {
            format!(
                "Nu am găsit fișierul {} în Project Model.",
                target_node.file
            )
        })?;
    if file.kind != ProjectModelFileKind::Template {
        return Err(
            "HTML Attribute Engine este activ doar pentru template-uri Zola/Tera.".to_string(),
        );
    }

    let target_range = target_node
        .range
        .as_ref()
        .ok_or_else(|| "Ținta nu are range stabil în Source Graph.".to_string())?;
    let target_tag = html_tag_at(&file.contents, target_range.start)?;
    validate_target_tag(intent, &target_tag)?;
    if let Some(zola_image) = intent.zola_image.as_ref() {
        let applied = apply_zola_image_contract(
            model,
            &target_node.file,
            &file.contents,
            target_range.start,
            zola_image,
        )?;
        return Ok(ProjectHtmlAttributePatch {
            file: target_node.file.clone(),
            resolved_target_id: target_node.id.clone(),
            before_revision: file.revision.clone(),
            after_revision: content_revision(&applied.contents),
            contents: applied.contents,
            target_location: applied.target_location,
            source_start_line: applied.source_start_line,
            tag: target_tag,
            attributes,
            zola_image_contract: true,
            zola_image: applied.presentation,
            managed_icon: None,
            generated_identity: None,
        });
    }
    if let Some(icon_intent) = intent.native_icon.as_ref() {
        return plan_native_icon_from_source_node(
            file,
            target_node,
            target_range.start,
            target_range.end,
            &target_tag,
            icon_intent,
        );
    }
    let GeneratedIdentityAttributeResolution {
        attributes,
        projection: generated_identity,
    } = resolve_generated_identity_attributes(
        model,
        &file.contents,
        target_range.start,
        &target_tag,
        &target_node.id,
        intent,
        attributes,
    )?;
    let attributes = resolve_native_block_option_attributes(
        &file.contents,
        target_range.start,
        intent,
        attributes,
    )?;
    validate_zola_managed_attributes(&file.contents, target_range.start, &attributes)?;
    if intent.native_block_option.is_none() {
        validate_schema_attributes(&target_tag, &attributes)?;
    }
    let applied = apply_html_attributes(
        &file.contents,
        &target_node.file,
        target_range.start,
        &attributes,
    )?;

    Ok(ProjectHtmlAttributePatch {
        file: target_node.file.clone(),
        resolved_target_id: target_node.id.clone(),
        before_revision: file.revision.clone(),
        after_revision: content_revision(&applied.contents),
        contents: applied.contents,
        target_location: applied.target_location,
        source_start_line: applied.source_start_line,
        tag: target_tag,
        attributes,
        zola_image_contract: false,
        zola_image: None,
        managed_icon: None,
        generated_identity,
    })
}

fn plan_native_icon_from_source_node(
    file: &ProjectModelFile,
    target_node: &SourceNode,
    element_start: usize,
    element_end: usize,
    target_tag: &str,
    intent: &NativeIconMutationIntent,
) -> Result<ProjectHtmlAttributePatch, String> {
    if target_tag != "svg" {
        return Err("Providerul Icon cere o rădăcină <svg>.".to_string());
    }
    let opening = parse_html_tag_at(&file.contents, element_start)
        .ok_or_else(|| "Range-ul iconului nu mai indică un <svg> stabil.".to_string())?;
    let opening_source = file
        .contents
        .get(opening.start..opening.end)
        .ok_or_else(|| "Rădăcina iconului nu poate fi citită din sursa canonică.".to_string())?;
    let previous_state = inspect_native_icon_source(opening_source)?
        .ok_or_else(|| "Ținta nu este un block Icon canonic.".to_string())?;
    let planned = crate::blocks::icons::plan_native_icon_mutation(opening_source, intent)?;
    let original_attributes = raw_tag_attributes(opening_source)
        .into_iter()
        .map(|attribute| (attribute.name, attribute.value))
        .collect::<BTreeMap<_, _>>();
    let previous_attributes = planned
        .attributes
        .keys()
        .map(|name| {
            (
                name.clone(),
                original_attributes
                    .get(&name.to_ascii_lowercase())
                    .cloned()
                    .flatten()
                    .map(|value| crate::blocks::icons::decode_icon_attribute_value(&value)),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut updated_opening = opening_source.to_string();
    for (name, value) in &planned.attributes {
        updated_opening = match value {
            Some(value) => set_tag_attribute_value(&updated_opening, name, value),
            None => remove_tag_attribute(&updated_opening, name),
        };
    }
    let element_source = file
        .contents
        .get(element_start..element_end)
        .ok_or_else(|| "Range-ul complet al iconului este invalid.".to_string())?;
    let normalized = element_source.to_ascii_lowercase();
    let closing_relative = normalized
        .rfind("</svg>")
        .ok_or_else(|| "Block-ul Icon nu mai are închiderea </svg> canonică.".to_string())?;
    let children_start = opening.end - element_start;
    if children_start > closing_relative {
        return Err("Interiorul block-ului Icon are un range invalid.".to_string());
    }
    let previous_children_html = element_source[children_start..closing_relative].to_string();
    let expected_previous_children =
        crate::blocks::icons::render_icon_children_by_identity(&previous_state.icon_identity)?;
    if previous_children_html.trim() != expected_previous_children {
        return Err(
            "Geometria block-ului Icon nu mai corespunde registrului Rust și nu poate fi suprascrisă automat."
                .to_string(),
        );
    }
    let closing = &element_source[closing_relative..];
    let replacement = format!("{}{}{}", updated_opening, planned.children_html, closing);
    let contents = replace_range(&file.contents, element_start, element_end, &replacement);
    let target_location =
        source_location_at_offset(&file.contents, &file.relative_path, element_start);
    let source_start_line = target_location.line;
    Ok(ProjectHtmlAttributePatch {
        file: target_node.file.clone(),
        resolved_target_id: target_node.id.clone(),
        before_revision: file.revision.clone(),
        after_revision: content_revision(&contents),
        contents,
        target_location,
        source_start_line,
        tag: target_tag.to_string(),
        attributes: planned.attributes,
        zola_image_contract: false,
        zola_image: None,
        managed_icon: Some(ProjectManagedIconPatch {
            state: planned.state,
            previous_state,
            previous_attributes,
            children_html: planned.children_html,
            previous_children_html,
        }),
        generated_identity: None,
    })
}

fn resolve_generated_identity_attributes(
    model: &ProjectModel,
    source: &str,
    opening_start: usize,
    target_tag: &str,
    target_identity: &str,
    intent: &ProjectHtmlAttributeIntent,
    generic_attributes: BTreeMap<String, Option<String>>,
) -> Result<GeneratedIdentityAttributeResolution, String> {
    let Some(generated_intent) = intent.generated_identity.as_ref() else {
        return Ok(GeneratedIdentityAttributeResolution {
            attributes: generic_attributes,
            projection: None,
        });
    };
    if !generic_attributes.is_empty() {
        return Err(
            "Generarea identității HTML nu poate fi combinată cu atribute generice.".to_string(),
        );
    }

    let opening = parse_html_tag_at(source, opening_start).ok_or_else(|| {
        "Range-ul elementului nu mai indică un tag HTML stabil pentru identitate.".to_string()
    })?;
    if opening.is_closing {
        return Err("Identitatea nu poate fi generată pe un tag de închidere.".to_string());
    }
    let opening_source = source
        .get(opening.start..opening.end)
        .ok_or_else(|| "Tag-ul HTML pentru identitate nu poate fi citit.".to_string())?;
    let class_source = tag_attribute_value(opening_source, "class").unwrap_or_default();
    let mut source_class_tokens = class_source
        .split_ascii_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let current_data_anim = tag_attribute_value(opening_source, "data-anim")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let (value, already_present) = match generated_intent.kind {
        ProjectGeneratedIdentityKind::Class => {
            if let Some(existing) = source_class_tokens
                .iter()
                .find(|class_name| is_generated_pana_identity(class_name))
            {
                (existing.clone(), true)
            } else if let Some(reusable) = current_data_anim
                .as_ref()
                .filter(|value| is_generated_pana_identity(value) && valid_class_token(value))
            {
                (reusable.clone(), false)
            } else {
                (
                    generate_unique_pana_identity(
                        model,
                        target_tag,
                        target_identity,
                        generated_intent.kind,
                    )?,
                    false,
                )
            }
        }
        ProjectGeneratedIdentityKind::DataAnim => {
            if let Some(existing) = current_data_anim.as_ref() {
                (existing.clone(), true)
            } else if let Some(reusable) = source_class_tokens
                .iter()
                .find(|class_name| is_generated_pana_identity(class_name))
            {
                (reusable.clone(), false)
            } else {
                (
                    generate_unique_pana_identity(
                        model,
                        target_tag,
                        target_identity,
                        generated_intent.kind,
                    )?,
                    false,
                )
            }
        }
    };

    let mut attributes = BTreeMap::new();
    let data_anim = match generated_intent.kind {
        ProjectGeneratedIdentityKind::Class => {
            if !source_class_tokens.iter().any(|token| token == &value) {
                source_class_tokens.push(value.clone());
            }
            attributes.insert("class".to_string(), Some(source_class_tokens.join(" ")));
            current_data_anim
        }
        ProjectGeneratedIdentityKind::DataAnim => {
            attributes.insert("data-anim".to_string(), Some(value.clone()));
            Some(value.clone())
        }
    };
    let classes = source_class_tokens
        .iter()
        .filter(|token| valid_class_token(token))
        .cloned()
        .collect();
    Ok(GeneratedIdentityAttributeResolution {
        attributes,
        projection: Some(ProjectGeneratedIdentityProjection {
            kind: generated_intent.kind,
            value,
            classes,
            data_anim,
            already_present,
        }),
    })
}

fn tag_attribute_value(opening_tag: &str, expected_name: &str) -> Option<String> {
    raw_tag_attributes(opening_tag)
        .into_iter()
        .find(|attribute| attribute.name.eq_ignore_ascii_case(expected_name))
        .and_then(|attribute| attribute.value)
}

fn valid_class_token(value: &str) -> bool {
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || matches!(first, '_' | '-'))
        && characters
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
}

fn is_generated_pana_identity(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    if !valid_class_token(&normalized) {
        return false;
    }
    let Some(remainder) = normalized.strip_prefix("ps-") else {
        return false;
    };
    let Some((tag, token)) = remainder.rsplit_once('-') else {
        return false;
    };
    !tag.is_empty()
        && tag
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
        && token.len() >= 6
        && token
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
}

fn generate_unique_pana_identity(
    model: &ProjectModel,
    target_tag: &str,
    target_identity: &str,
    kind: ProjectGeneratedIdentityKind,
) -> Result<String, String> {
    let normalized_tag = target_tag
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    let normalized_tag = if normalized_tag.is_empty() {
        "el"
    } else {
        &normalized_tag
    };
    let kind_label = match kind {
        ProjectGeneratedIdentityKind::Class => "class",
        ProjectGeneratedIdentityKind::DataAnim => "data-anim",
    };

    for attempt in 0_u16..=255 {
        let digest = Sha256::digest(format!(
            "pana-generated-identity-v1\0{}\0{}\0{}\0{}\0{}",
            model.revision, target_identity, normalized_tag, kind_label, attempt
        ));
        let token = digest[..4]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let candidate = format!("ps-{normalized_tag}-{token}");
        if !model
            .files
            .iter()
            .any(|file| file.contents.contains(&candidate))
        {
            return Ok(candidate);
        }
    }

    Err("ProjectModel nu a putut aloca o identitate HTML unică.".to_string())
}

fn resolve_native_block_option_attributes(
    source: &str,
    opening_start: usize,
    intent: &ProjectHtmlAttributeIntent,
    generic_attributes: BTreeMap<String, Option<String>>,
) -> Result<BTreeMap<String, Option<String>>, String> {
    let Some(option_intent) = intent.native_block_option.as_ref() else {
        return Ok(generic_attributes);
    };
    let opening = parse_html_tag_at(source, opening_start)
        .ok_or_else(|| "Range-ul blocului nu mai indică un tag HTML stabil.".to_string())?;
    if opening.is_closing {
        return Err("Range-ul blocului indică un tag de închidere.".to_string());
    }
    let opening_source = source
        .get(opening.start..opening.end)
        .ok_or_else(|| "Nu am putut citi rădăcina blocului din sursa autoritativă.".to_string())?;
    let (attribute, value) = plan_native_block_option_attribute(opening_source, option_intent)?;
    Ok(BTreeMap::from([(attribute, value)]))
}

fn validate_target_tag(
    intent: &ProjectHtmlAttributeIntent,
    actual_tag: &str,
) -> Result<(), String> {
    if let Some(expected_tag) = intent.target_tag.as_deref() {
        let expected_tag = expected_tag.trim().to_ascii_lowercase();
        if !expected_tag.is_empty() && expected_tag != actual_tag {
            return Err(format!(
                "Locația indică <{}>, dar intenția preview a cerut <{}>.",
                actual_tag, expected_tag
            ));
        }
    }
    if actual_tag.eq_ignore_ascii_case("html") {
        return Err("Elementul <html> nu este editabil vizual pentru atribute.".to_string());
    }
    Ok(())
}

fn apply_html_attributes(
    source: &str,
    file: &str,
    opening_start: usize,
    attributes: &BTreeMap<String, Option<String>>,
) -> Result<AttributeApplication, String> {
    let opening = parse_html_tag_at(source, opening_start)
        .ok_or_else(|| "Range-ul nu mai indică un tag HTML stabil.".to_string())?;
    if opening.is_closing {
        return Err("Range-ul indică un tag HTML de închidere, nu un element mutabil.".to_string());
    }

    let opening_source = source
        .get(opening.start..opening.end)
        .ok_or_else(|| "Nu am putut citi tag-ul HTML de deschidere.".to_string())?;
    let mut updated_opening = opening_source.to_string();
    for (name, value) in attributes {
        updated_opening = match value {
            Some(value) => set_tag_attribute_value(&updated_opening, name, value),
            None => remove_tag_attribute(&updated_opening, name),
        };
    }

    let contents = replace_range(source, opening.start, opening.end, &updated_opening);
    let target_location = source_location_at_offset(source, file, opening.start);
    Ok(AttributeApplication {
        contents,
        source_start_line: target_location.line,
        target_location,
    })
}

fn normalize_attribute_mutations(
    attributes: &[ProjectHtmlAttributeMutation],
) -> Result<BTreeMap<String, Option<String>>, String> {
    if attributes.is_empty() {
        return Err("Nu există atribute de aplicat.".to_string());
    }

    let mut normalized = BTreeMap::new();
    for attribute in attributes {
        let (raw_name, raw_value) = match attribute {
            ProjectHtmlAttributeMutation::SetAttribute { name, value } => {
                (name.as_str(), Some(value.as_str()))
            }
            ProjectHtmlAttributeMutation::RemoveAttribute { name } => (name.as_str(), None),
        };
        let name = raw_name.trim().to_ascii_lowercase();
        if name.is_empty() {
            return Err("Atributul fără nume nu poate fi aplicat.".to_string());
        }
        if !is_valid_attribute_name(&name) {
            return Err(format!("Atributul {name} are nume invalid."));
        }
        if is_protected_attribute(&name) {
            return Err(format!(
                "Atributul intern {name} nu poate fi modificat direct."
            ));
        }

        let value = raw_value.map(validate_attribute_value).transpose()?;
        normalized.insert(name, value);
    }

    if normalized.is_empty() {
        return Err("Nu există atribute valide de aplicat.".to_string());
    }
    Ok(normalized)
}

fn validate_attribute_value(value: &str) -> Result<String, String> {
    if value
        .chars()
        .any(|character| matches!(character, '\n' | '\r' | '\0'))
    {
        return Err("Valorile de atribut nu pot conține linii noi sau caractere nule.".to_string());
    }
    Ok(value.to_string())
}

fn validate_schema_attributes(
    tag: &str,
    attributes: &BTreeMap<String, Option<String>>,
) -> Result<(), String> {
    for (name, value) in attributes {
        validate_visual_attribute_mutation(tag, name, value.as_deref())?;
    }
    Ok(())
}

fn validate_zola_managed_attributes(
    source: &str,
    opening_start: usize,
    attributes: &BTreeMap<String, Option<String>>,
) -> Result<(), String> {
    let Some(_) = inspect_zola_image_at(source, opening_start)? else {
        return Ok(());
    };
    if let Some(name) = ["src", "width", "height"]
        .into_iter()
        .find(|name| attributes.contains_key(*name))
    {
        return Err(format!(
            "Atributul {name} este administrat de contractul resize_image; actualizează-l prin controalele Zola."
        ));
    }
    Ok(())
}

fn is_valid_attribute_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_' || first == ':') {
        return false;
    }
    chars.all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | ':' | '.')
    })
}

fn is_protected_attribute(name: &str) -> bool {
    // The entire namespace is owned by the editor runtime. A prefix invariant
    // remains safe when new Canvas/Workbench identities are introduced.
    name.starts_with("data-pana-")
}

pub(crate) fn remove_tag_attribute(tag: &str, attr: &str) -> String {
    let mut next = tag.to_string();
    for attribute in find_tag_attributes(&next, attr).into_iter().rev() {
        let remove_start = previous_whitespace_start(&next, attribute.attr_start);
        next = replace_range(&next, remove_start, attribute.attr_end, "");
    }
    next
}

pub(crate) fn set_tag_attribute_value(tag: &str, attr: &str, value: &str) -> String {
    let mut next = tag.to_string();
    let matches = find_tag_attributes(&next, attr);
    for duplicate in matches.iter().skip(1).rev() {
        let remove_start = previous_whitespace_start(&next, duplicate.attr_start);
        next = replace_range(&next, remove_start, duplicate.attr_end, "");
    }

    if let Some(attribute) = find_tag_attributes(&next, attr).into_iter().next() {
        return match attribute.value_style {
            TagAttributeValueStyle::DoubleQuoted => replace_range(
                &next,
                attribute.value_start,
                attribute.value_end,
                &escape_quoted_attr_value(value, '"'),
            ),
            TagAttributeValueStyle::SingleQuoted => replace_range(
                &next,
                attribute.value_start,
                attribute.value_end,
                &escape_quoted_attr_value(value, '\''),
            ),
            TagAttributeValueStyle::Unquoted if is_safe_unquoted_attr_value(value) => {
                replace_range(
                    &next,
                    attribute.value_start,
                    attribute.value_end,
                    &escape_unquoted_attr_value(value),
                )
            }
            TagAttributeValueStyle::Minimized if value.is_empty() => next,
            TagAttributeValueStyle::Unquoted | TagAttributeValueStyle::Minimized => replace_range(
                &next,
                attribute.attr_start,
                attribute.attr_end,
                &format!(r#"{}="{}""#, attr, escape_attr_value(value)),
            ),
        };
    }
    insert_tag_attribute(&next, attr, value)
}

#[derive(Clone, Copy)]
struct TagAttribute {
    attr_start: usize,
    value_start: usize,
    value_end: usize,
    attr_end: usize,
    value_style: TagAttributeValueStyle,
}

#[derive(Clone, Copy)]
enum TagAttributeValueStyle {
    Minimized,
    DoubleQuoted,
    SingleQuoted,
    Unquoted,
}

fn find_tag_attributes(tag: &str, attr: &str) -> Vec<TagAttribute> {
    let attr_lower = attr.to_ascii_lowercase();
    parse_tag_attributes(tag)
        .into_iter()
        .filter(|candidate| {
            tag.get(candidate.name_start..candidate.name_end)
                .is_some_and(|name| name.to_ascii_lowercase() == attr_lower)
        })
        .map(|candidate| TagAttribute {
            attr_start: candidate.attr_start,
            value_start: candidate.value_start,
            value_end: candidate.value_end,
            attr_end: candidate.attr_end,
            value_style: candidate.value_style,
        })
        .collect()
}

#[derive(Clone, Copy)]
struct ParsedTagAttribute {
    attr_start: usize,
    name_start: usize,
    name_end: usize,
    value_start: usize,
    value_end: usize,
    attr_end: usize,
    value_style: TagAttributeValueStyle,
}

#[derive(Clone, Debug)]
pub(crate) struct RawTagAttribute {
    pub(crate) name: String,
    pub(crate) raw: String,
    pub(crate) value: Option<String>,
}

pub(crate) fn raw_tag_attributes(tag: &str) -> Vec<RawTagAttribute> {
    parse_tag_attributes(tag)
        .into_iter()
        .filter_map(|attribute| {
            let name = tag
                .get(attribute.name_start..attribute.name_end)?
                .to_ascii_lowercase();
            let raw = tag
                .get(attribute.attr_start..attribute.attr_end)?
                .to_string();
            let value =
                (!matches!(attribute.value_style, TagAttributeValueStyle::Minimized)).then(|| {
                    tag.get(attribute.value_start..attribute.value_end)
                        .unwrap_or("")
                        .to_string()
                });
            Some(RawTagAttribute { name, raw, value })
        })
        .collect()
}

pub(crate) fn insert_raw_tag_attribute(
    tag: &str,
    expected_name: &str,
    raw: &str,
) -> Result<String, String> {
    let raw = raw.trim();
    if raw.is_empty() || raw.contains(['\n', '\r', '\0']) {
        return Err("Atributul original al imaginii Zola este invalid.".to_string());
    }
    let probe = format!("<img {raw}>");
    let parsed = raw_tag_attributes(&probe);
    if parsed.len() != 1 || parsed[0].raw != raw || parsed[0].name != expected_name {
        return Err(
            "Atributul original al imaginii Zola nu mai are o reprezentare HTML sigură."
                .to_string(),
        );
    }
    Ok(insert_raw_tag_attribute_unchecked(tag, raw))
}

fn insert_raw_tag_attribute_unchecked(tag: &str, raw: &str) -> String {
    let insert_at = tag
        .rfind("/>")
        .or_else(|| tag.rfind('>'))
        .unwrap_or(tag.len());
    format!("{} {}{}", &tag[..insert_at], raw, &tag[insert_at..])
}

fn parse_tag_attributes(tag: &str) -> Vec<ParsedTagAttribute> {
    let mut attributes = Vec::new();
    let mut cursor = tag.find('<').map(|index| index + 1).unwrap_or(0);
    cursor = skip_ascii_whitespace(tag, cursor);
    if char_at(tag, cursor) == Some('/') {
        cursor += 1;
    }
    while let Some(character) = char_at(tag, cursor) {
        if character.is_ascii_whitespace() || character == '>' || character == '/' {
            break;
        }
        cursor += character.len_utf8();
    }

    loop {
        cursor = skip_ascii_whitespace(tag, cursor);
        let Some(character) = char_at(tag, cursor) else {
            break;
        };
        if character == '>' || (character == '/' && char_at(tag, cursor + 1) == Some('>')) {
            break;
        }

        let attr_start = cursor;
        let name_start = cursor;
        while let Some(character) = char_at(tag, cursor) {
            if character.is_ascii_whitespace()
                || matches!(character, '=' | '>' | '/' | '"' | '\'' | '<')
            {
                break;
            }
            cursor += character.len_utf8();
        }
        let name_end = cursor;
        if name_start == name_end {
            cursor += character.len_utf8();
            continue;
        }

        cursor = skip_ascii_whitespace(tag, cursor);
        if char_at(tag, cursor) != Some('=') {
            attributes.push(ParsedTagAttribute {
                attr_start,
                name_start,
                name_end,
                value_start: name_end,
                value_end: name_end,
                attr_end: name_end,
                value_style: TagAttributeValueStyle::Minimized,
            });
            continue;
        }

        cursor += 1;
        cursor = skip_ascii_whitespace(tag, cursor);
        let Some(value_lead) = char_at(tag, cursor) else {
            attributes.push(ParsedTagAttribute {
                attr_start,
                name_start,
                name_end,
                value_start: cursor,
                value_end: cursor,
                attr_end: cursor,
                value_style: TagAttributeValueStyle::Unquoted,
            });
            break;
        };

        if value_lead == '"' || value_lead == '\'' {
            let quote = value_lead;
            cursor += quote.len_utf8();
            let value_start = cursor;
            while let Some(character) = char_at(tag, cursor) {
                if character == quote {
                    break;
                }
                cursor += character.len_utf8();
            }
            let value_end = cursor;
            if char_at(tag, cursor) == Some(quote) {
                cursor += quote.len_utf8();
            }
            attributes.push(ParsedTagAttribute {
                attr_start,
                name_start,
                name_end,
                value_start,
                value_end,
                attr_end: cursor,
                value_style: if quote == '"' {
                    TagAttributeValueStyle::DoubleQuoted
                } else {
                    TagAttributeValueStyle::SingleQuoted
                },
            });
            continue;
        }

        let value_start = cursor;
        while let Some(character) = char_at(tag, cursor) {
            if character.is_ascii_whitespace()
                || character == '>'
                || (character == '/' && char_at(tag, cursor + 1) == Some('>'))
            {
                break;
            }
            cursor += character.len_utf8();
        }
        attributes.push(ParsedTagAttribute {
            attr_start,
            name_start,
            name_end,
            value_start,
            value_end: cursor,
            attr_end: cursor,
            value_style: TagAttributeValueStyle::Unquoted,
        });
    }

    attributes
}

fn char_at(source: &str, cursor: usize) -> Option<char> {
    source.get(cursor..)?.chars().next()
}

fn previous_whitespace_start(source: &str, index: usize) -> usize {
    let mut cursor = index;
    while cursor > 0 {
        let Some((previous_index, character)) = source[..cursor].char_indices().next_back() else {
            break;
        };
        if !character.is_ascii_whitespace() || character == '\n' || character == '\r' {
            break;
        }
        cursor = previous_index;
    }
    cursor
}

fn insert_tag_attribute(tag: &str, attr: &str, value: &str) -> String {
    let insert_at = tag
        .rfind("/>")
        .or_else(|| tag.rfind('>'))
        .unwrap_or(tag.len());
    format!(
        "{} {}=\"{}\"{}",
        &tag[..insert_at],
        attr,
        escape_attr_value(value),
        &tag[insert_at..]
    )
}

fn replace_range(source: &str, start: usize, end: usize, replacement: &str) -> String {
    let mut next = String::with_capacity(source.len() - (end - start) + replacement.len());
    next.push_str(&source[..start]);
    next.push_str(replacement);
    next.push_str(&source[end..]);
    next
}

fn escape_attr_value(value: &str) -> String {
    escape_quoted_attr_value(value, '"')
}

fn escape_quoted_attr_value(value: &str, quote: char) -> String {
    let escaped = value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    if quote == '\'' {
        escaped.replace('\'', "&#39;")
    } else {
        escaped.replace('"', "&quot;")
    }
}

fn is_safe_unquoted_attr_value(value: &str) -> bool {
    !value.is_empty()
        && !value.chars().any(|character| {
            character.is_ascii_whitespace()
                || matches!(character, '"' | '\'' | '`' | '=' | '<' | '>')
        })
}

fn escape_unquoted_attr_value(value: &str) -> String {
    value.replace('&', "&amp;")
}

fn skip_ascii_whitespace(source: &str, mut cursor: usize) -> usize {
    while let Some(character) = source[cursor..].chars().next() {
        if !character.is_ascii_whitespace() {
            break;
        }
        cursor += character.len_utf8();
    }
    cursor
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::project_model::{
        test_support::ProjectModelTestFixture,
        zola_image_engine::{ProjectZolaImageIntent, ZolaImageFormat, ZolaImageOperation},
    };
    use crate::source_graph::model::{BlockOptionValue, SourceNodeKind};

    use super::*;

    #[test]
    fn plan_html_attributes_updates_template_anchor() {
        let root = unique_test_dir();
        let fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            concat!(
                "{% block content %}\n",
                "<section class=\"hero\" title=\"Old\">\n",
                "  <h1>Titlu</h1>\n",
                "</section>\n",
                "{% endblock %}\n",
            ),
        )
        .unwrap();
        let model = fixture.build_model().unwrap();
        let section = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section .hero>")
            .unwrap();

        let plan = plan_html_attributes(
            &model,
            &ProjectHtmlAttributeIntent {
                target_source_id: Some(section.id.clone()),
                target_tag: Some("section".to_string()),
                attributes: vec![
                    ProjectHtmlAttributeMutation::set("class", "hero hero--mare"),
                    ProjectHtmlAttributeMutation::remove("title"),
                    ProjectHtmlAttributeMutation::set("data-anim", "ps-hero-abc123"),
                ],
                zola_image: None,
                native_block_option: None,
                native_icon: None,
                generated_identity: None,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.unwrap();
        assert!(patch
            .contents
            .contains(r#"<section class="hero hero--mare" data-anim="ps-hero-abc123">"#));
        assert!(!patch.contents.contains("title="));
        assert_eq!(patch.tag, "section");
        assert_eq!(patch.source_start_line, 2);
    }

    #[test]
    fn generated_class_is_allocated_by_project_model_and_preserves_source_classes() {
        let root = unique_test_dir();
        let fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            "<main><div class=\"layout\">Conținut</div></main>\n",
        )
        .unwrap();
        let model = fixture.build_model().unwrap();
        let target = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<div .layout>")
            .expect("div source node");

        let plan = plan_html_attributes(
            &model,
            &ProjectHtmlAttributeIntent {
                target_source_id: Some(target.id.clone()),
                target_tag: Some("div".to_string()),
                attributes: Vec::new(),
                zola_image: None,
                native_block_option: None,
                native_icon: None,
                generated_identity: Some(ProjectGeneratedIdentityIntent {
                    kind: ProjectGeneratedIdentityKind::Class,
                }),
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.expect("generated class patch");
        let projection = patch
            .generated_identity
            .expect("generated identity receipt");
        assert_eq!(projection.kind, ProjectGeneratedIdentityKind::Class);
        assert!(projection.value.starts_with("ps-div-"));
        assert_eq!(projection.value.len(), "ps-div-".len() + 8);
        assert_eq!(
            projection.classes,
            vec!["layout", projection.value.as_str()]
        );
        assert_eq!(projection.data_anim, None);
        assert!(!projection.already_present);
        assert_eq!(
            patch
                .attributes
                .get("class")
                .and_then(|value| value.as_deref()),
            Some(format!("layout {}", projection.value).as_str()),
        );
        assert!(patch
            .contents
            .contains(&format!("class=\"layout {}\"", projection.value)));
    }

    #[test]
    fn generated_data_anim_reuses_the_project_model_generated_class() {
        let root = unique_test_dir();
        let fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            "<main><div class=\"layout ps-div-a1b2c3d4\">Conținut</div></main>\n",
        )
        .unwrap();
        let model = fixture.build_model().unwrap();
        let target = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label.contains("<div"))
            .expect("div source node");

        let plan = plan_html_attributes(
            &model,
            &ProjectHtmlAttributeIntent {
                target_source_id: Some(target.id.clone()),
                target_tag: Some("div".to_string()),
                attributes: Vec::new(),
                zola_image: None,
                native_block_option: None,
                native_icon: None,
                generated_identity: Some(ProjectGeneratedIdentityIntent {
                    kind: ProjectGeneratedIdentityKind::DataAnim,
                }),
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.expect("data-anim patch");
        let projection = patch
            .generated_identity
            .expect("generated identity receipt");
        assert_eq!(projection.kind, ProjectGeneratedIdentityKind::DataAnim);
        assert_eq!(projection.value, "ps-div-a1b2c3d4");
        assert_eq!(projection.data_anim.as_deref(), Some("ps-div-a1b2c3d4"));
        assert_eq!(
            projection.classes,
            vec!["layout".to_string(), "ps-div-a1b2c3d4".to_string()]
        );
        assert!(!projection.already_present);
        assert!(patch.contents.contains("data-anim=\"ps-div-a1b2c3d4\""));
    }

    #[test]
    fn generated_class_reuses_data_anim_and_reports_existing_identity_as_noop() {
        let root = unique_test_dir();
        let fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            "<main><div class=\"layout\" data-anim=\"ps-div-deadbeef\">Conținut</div></main>\n",
        )
        .unwrap();
        let model = fixture.build_model().unwrap();
        let target = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label.contains("<div"))
            .expect("div source node");
        let intent = ProjectHtmlAttributeIntent {
            target_source_id: Some(target.id.clone()),
            target_tag: Some("div".to_string()),
            attributes: Vec::new(),
            zola_image: None,
            native_block_option: None,
            native_icon: None,
            generated_identity: Some(ProjectGeneratedIdentityIntent {
                kind: ProjectGeneratedIdentityKind::Class,
            }),
        };

        let first = plan_html_attributes(&model, &intent);
        assert!(first.allowed, "{:?}", first.diagnostic);
        let first_patch = first.patch.expect("class patch");
        let first_projection = first_patch
            .generated_identity
            .as_ref()
            .expect("generated identity receipt");
        assert_eq!(first_projection.value, "ps-div-deadbeef");
        assert_eq!(
            first_projection.classes,
            vec!["layout".to_string(), "ps-div-deadbeef".to_string()]
        );
        assert!(!first_projection.already_present);

        let mut committed_model = model.clone();
        let committed_file = committed_model
            .files
            .iter_mut()
            .find(|file| file.relative_path == first_patch.file)
            .expect("committed model file");
        committed_file.contents = first_patch.contents;
        committed_file.revision = first_patch.after_revision;
        committed_file.source_hash =
            crate::kernel::file_buffer_store::hash_text(&committed_file.contents);
        let second = plan_html_attributes(&committed_model, &intent);

        fs::remove_dir_all(&root).unwrap();
        assert!(second.allowed, "{:?}", second.diagnostic);
        let second_patch = second.patch.expect("existing class patch");
        let second_projection = second_patch
            .generated_identity
            .expect("existing identity receipt");
        assert_eq!(second_projection.value, "ps-div-deadbeef");
        assert!(second_projection.already_present);
        assert_eq!(second_patch.before_revision, second_patch.after_revision);
    }

    #[test]
    fn plan_html_attributes_accepts_preview_identity_for_html_wrapping_tera_text() {
        let root = unique_test_dir();
        let source = concat!(
            "<section>\n",
            "<h1 id=\"title\">\n",
            "<span>{% if lang == \"en\" %}Build visually.{% else %}Construiește vizual.{% endif %}</span>\n",
            "<span>{% if lang == \"en\" %}Keep control.{% else %}Păstrează controlul.{% endif %}</span>\n",
            "</h1>\n",
            "</section>\n",
        );
        let fixture = ProjectModelTestFixture::standard_zola(root.clone(), source).unwrap();
        let model = fixture.build_model().unwrap();
        let preview_index = crate::preview::preprocess::SourceIdIndex::for_source_graph(
            &model.source_graph,
            [("templates/index.html", source)],
        )
        .unwrap();
        let preview_source_id = preview_index
            .source_id_for("templates/index.html:3:1")
            .expect("preview span source identity")
            .to_string();

        let plan = plan_html_attributes(
            &model,
            &ProjectHtmlAttributeIntent {
                target_source_id: Some(preview_source_id),
                target_tag: Some("span".to_string()),
                attributes: vec![ProjectHtmlAttributeMutation::set(
                    "class",
                    "ps-build-visually-a1b2c3",
                )],
                zola_image: None,
                native_block_option: None,
                native_icon: None,
                generated_identity: None,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        assert!(plan
            .patch
            .expect("attribute patch")
            .contents
            .contains("<span class=\"ps-build-visually-a1b2c3\">{% if lang"));
    }

    #[test]
    fn native_block_option_uses_rust_registry_to_mutate_protected_attribute() {
        let root = unique_test_dir();
        let fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            concat!(
                "{% block content %}\n",
                "<div class=\"offcanvas\" data-pana-block=\"offcanvas\" data-pana-offcanvas-side=\"end\"></div>\n",
                "{% endblock %}\n",
            ),
        )
        .unwrap();
        let model = fixture.build_model().unwrap();
        let block_root = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<div .offcanvas>")
            .expect("block root");

        let plan = plan_html_attributes(
            &model,
            &ProjectHtmlAttributeIntent {
                target_source_id: Some(block_root.id.clone()),
                target_tag: Some("div".to_string()),
                attributes: Vec::new(),
                zola_image: None,
                native_block_option: Some(NativeBlockOptionIntent {
                    provider_id: "offcanvas".to_string(),
                    option_id: "side".to_string(),
                    value: BlockOptionValue::Text("start".to_string()),
                }),
                native_icon: None,
                generated_identity: None,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        assert!(plan
            .patch
            .expect("native option patch")
            .contents
            .contains(r#"data-pana-offcanvas-side="start""#));
    }

    #[test]
    fn plan_html_attributes_rejects_location_without_source_id() {
        let root = unique_test_dir();
        let mut fixture =
            ProjectModelTestFixture::standard_zola(root.clone(), "<main></main>\n").unwrap();
        fixture.source(
            "static/plain.html",
            concat!(
                "<!DOCTYPE html>\n",
                "<html>\n",
                "<body>\n",
                "  <img class=\"photo\" src=\"old.jpg\" alt=\"Old\">\n",
                "</body>\n",
                "</html>\n",
            ),
        );
        let model = fixture.build_model().unwrap();

        let plan = plan_html_attributes(
            &model,
            &ProjectHtmlAttributeIntent {
                target_source_id: None,
                target_tag: Some("img".to_string()),
                attributes: vec![
                    ProjectHtmlAttributeMutation::set("src", "nou.jpg"),
                    ProjectHtmlAttributeMutation::set("alt", "Imagine nouă"),
                ],
                zola_image: None,
                native_block_option: None,
                native_icon: None,
                generated_identity: None,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.patch.is_none());
    }

    #[test]
    fn visual_attribute_schema_blocks_active_and_semantically_invalid_values() {
        let active = BTreeMap::from([("onclick".to_string(), Some("alert(1)".to_string()))]);
        assert!(validate_schema_attributes("button", &active).is_err());

        let aria = BTreeMap::from([("aria-hidden".to_string(), Some("yes".to_string()))]);
        assert!(validate_schema_attributes("button", &aria).is_err());

        let direction = BTreeMap::from([("dir".to_string(), Some("sideways".to_string()))]);
        assert!(validate_schema_attributes("div", &direction).is_err());
    }

    #[test]
    fn visual_attribute_schema_distinguishes_source_only_and_meaningful_empty_values() {
        let source_only = BTreeMap::from([
            ("target".to_string(), Some("_blank".to_string())),
            ("download".to_string(), Some(String::new())),
        ]);
        assert!(validate_schema_attributes("a", &source_only).is_ok());

        let meaningful_empty = BTreeMap::from([
            ("href".to_string(), Some(String::new())),
            ("aria-label".to_string(), Some(String::new())),
            ("data-state".to_string(), Some(String::new())),
        ]);
        assert!(validate_schema_attributes("a", &meaningful_empty).is_ok());

        let empty_enumerated = BTreeMap::from([("dir".to_string(), Some(String::new()))]);
        assert!(validate_schema_attributes("div", &empty_enumerated).is_err());
    }

    #[test]
    fn every_pana_runtime_attribute_is_protected() {
        assert!(is_protected_attribute("data-pana-source-id"));
        assert!(is_protected_attribute("data-pana-render-instance-id"));
        assert!(is_protected_attribute(
            "data-pana-workbench-active-template"
        ));
        assert!(!is_protected_attribute("data-anim"));
        assert!(!is_protected_attribute("data-component"));
    }

    #[test]
    fn empty_attribute_values_remain_explicit_set_operations() {
        let normalized =
            normalize_attribute_mutations(&[ProjectHtmlAttributeMutation::set("alt", "")]).unwrap();

        assert_eq!(normalized.get("alt"), Some(&Some(String::new())));
        assert_eq!(
            set_tag_attribute_value(r#"<img alt="decorativ">"#, "alt", ""),
            r#"<img alt="">"#
        );
        assert_eq!(
            set_tag_attribute_value("<input disabled>", "disabled", ""),
            "<input disabled>"
        );
    }

    #[test]
    fn attribute_rewriter_handles_minimized_and_unquoted_attributes_without_duplicates() {
        assert_eq!(
            remove_tag_attribute("<input disabled>", "disabled"),
            "<input>"
        );
        assert_eq!(
            set_tag_attribute_value("<div id=hero class=card>", "id", "principal"),
            "<div id=principal class=card>",
        );
        assert_eq!(
            set_tag_attribute_value(
                r#"<input disabled disabled="disabled">"#,
                "disabled",
                "disabled"
            ),
            r#"<input disabled="disabled">"#,
        );
    }

    #[test]
    fn attribute_rewriter_preserves_single_and_double_quoted_syntax() {
        assert_eq!(
            set_tag_attribute_value("<div title='vechi'>", "title", "nou 'sigur'"),
            "<div title='nou &#39;sigur&#39;'>",
        );
        assert_eq!(
            set_tag_attribute_value(r#"<div title="vechi">"#, "title", "nou \"sigur\""),
            r#"<div title="nou &quot;sigur&quot;">"#,
        );
    }

    #[test]
    fn managed_zola_image_attributes_cannot_be_overwritten_generically() {
        let root = unique_test_dir();
        let mut fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            "<img src=\"/images/hero.jpg\" alt=\"Inițial\">\n",
        )
        .unwrap();
        fixture.accepted_resource("static/images/hero.jpg", 5);
        let before = fixture.build_model().unwrap();
        let image = before
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<img>")
            .expect("missing img");
        let enable = plan_html_attributes(
            &before,
            &ProjectHtmlAttributeIntent {
                target_source_id: Some(image.id.clone()),
                target_tag: Some("img".to_string()),
                attributes: Vec::new(),
                zola_image: Some(ProjectZolaImageIntent {
                    enabled: true,
                    source_url: Some("/images/hero.jpg".to_string()),
                    source_path: Some("static/images/hero.jpg".to_string()),
                    width: Some(800),
                    height: None,
                    operation: Some(ZolaImageOperation::FitWidth),
                    format: Some(ZolaImageFormat::Webp),
                    quality: Some(82),
                    filter: None,
                }),
                native_block_option: None,
                native_icon: None,
                generated_identity: None,
            },
        );
        assert!(enable.allowed, "{:?}", enable.diagnostic);
        let enabled = enable.patch.unwrap();
        fixture.draft(enabled.file.clone(), enabled.contents.clone());
        let projected = fixture.build_model().unwrap();
        let projected_image = projected
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<img>")
            .expect("missing projected img");

        let overwrite = plan_html_attributes(
            &projected,
            &ProjectHtmlAttributeIntent {
                target_source_id: Some(projected_image.id.clone()),
                target_tag: Some("img".to_string()),
                attributes: vec![ProjectHtmlAttributeMutation::set("src", "/other.jpg")],
                zola_image: None,
                native_block_option: None,
                native_icon: None,
                generated_identity: None,
            },
        );
        assert!(!overwrite.allowed);
        assert!(overwrite
            .diagnostic
            .as_deref()
            .is_some_and(|message| message.contains("administrat")));

        let alt = plan_html_attributes(
            &projected,
            &ProjectHtmlAttributeIntent {
                target_source_id: Some(projected_image.id.clone()),
                target_tag: Some("img".to_string()),
                attributes: vec![ProjectHtmlAttributeMutation::set("alt", "Nou")],
                zola_image: None,
                native_block_option: None,
                native_icon: None,
                generated_identity: None,
            },
        );
        fs::remove_dir_all(root).unwrap();
        assert!(alt.allowed, "{:?}", alt.diagnostic);
        let alt_contents = alt.patch.unwrap().contents;
        assert!(alt_contents.contains("alt=\"Nou\""));
        assert!(alt_contents.contains("pana-studio:zola-image"));
        assert!(alt_contents.contains(".url | safe"));
    }

    #[test]
    fn native_icon_mutation_replaces_geometry_and_preserves_user_root_attributes() {
        let root = unique_test_dir();
        let icon = crate::blocks::icons::render_icon_block_html(
            "home",
            "ps-icon-test custom",
            "ps-icon-test",
            "icon-test",
        )
        .unwrap()
        .replacen(">", " style=\"color: rebeccapurple\">", 1);
        let fixture =
            ProjectModelTestFixture::standard_zola(root.clone(), format!("<main>{icon}</main>\n"))
                .unwrap();
        let model = fixture.build_model().unwrap();
        let marker = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::BlockMarker && node.label == "icon")
            .expect("icon marker");
        let icon_root = model
            .source_graph
            .nodes
            .iter()
            .find(|node| marker.parent.as_deref() == Some(node.id.as_str()))
            .expect("icon root");
        let plan = plan_html_attributes(
            &model,
            &ProjectHtmlAttributeIntent {
                target_source_id: Some(icon_root.id.clone()),
                target_tag: Some("svg".to_string()),
                attributes: Vec::new(),
                zola_image: None,
                native_block_option: None,
                native_icon: Some(NativeIconMutationIntent {
                    icon_identity: "tabler-outline:star".to_string(),
                    size: 32,
                    stroke_width: "1.5".to_string(),
                    decorative: false,
                    accessible_label: Some("Favorite".to_string()),
                }),
                generated_identity: None,
            },
        );
        fs::remove_dir_all(&root).unwrap();

        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.expect("icon patch");
        let managed = patch.managed_icon.expect("managed icon");
        assert_eq!(managed.previous_state.icon_id, "home");
        assert_eq!(managed.state.icon_id, "star");
        assert!(patch
            .contents
            .contains("class=\"icon ps-icon-test custom\""));
        assert!(patch.contents.contains("style=\"color: rebeccapurple\""));
        assert!(patch
            .contents
            .contains("data-pana-icon=\"tabler-outline:star\""));
        assert!(patch.contents.contains("width=\"32\""));
        assert!(patch.contents.contains("role=\"img\""));
        assert!(patch.contents.contains("aria-label=\"Favorite\""));
        assert!(!patch.contents.contains("aria-hidden=\"true\""));
        assert_ne!(managed.children_html, managed.previous_children_html);
    }

    #[test]
    fn native_icon_mutation_rejects_stale_or_missing_source_node_id() {
        let root = unique_test_dir();
        let icon = crate::blocks::icons::render_icon_block_html(
            "home",
            "ps-icon-test",
            "ps-icon-test",
            "icon-test",
        )
        .unwrap();
        let fixture =
            ProjectModelTestFixture::standard_zola(root.clone(), format!("<main>{icon}</main>\n"))
                .unwrap();
        let model = fixture.build_model().unwrap();
        let intent = NativeIconMutationIntent {
            icon_identity: "tabler-outline:star".to_string(),
            size: 24,
            stroke_width: "2".to_string(),
            decorative: true,
            accessible_label: None,
        };

        let stale = plan_html_attributes(
            &model,
            &ProjectHtmlAttributeIntent {
                target_source_id: Some("sg_stale_icon_identity".to_string()),
                target_tag: Some("svg".to_string()),
                attributes: Vec::new(),
                zola_image: None,
                native_block_option: None,
                native_icon: Some(intent.clone()),
                generated_identity: None,
            },
        );
        let location_only = plan_html_attributes(
            &model,
            &ProjectHtmlAttributeIntent {
                target_source_id: None,
                target_tag: Some("svg".to_string()),
                attributes: Vec::new(),
                zola_image: None,
                native_block_option: None,
                native_icon: Some(intent),
                generated_identity: None,
            },
        );
        fs::remove_dir_all(&root).unwrap();

        assert!(!stale.allowed);
        assert!(stale.patch.is_none());
        assert!(!location_only.allowed);
        assert!(location_only.patch.is_none());
    }

    fn unique_test_dir() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "pana-studio-attribute-engine-{}-{stamp}",
            std::process::id()
        ))
    }
}
