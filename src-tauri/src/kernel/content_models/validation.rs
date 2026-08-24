use std::collections::{BTreeMap, BTreeSet, HashMap};

use sha2::{Digest, Sha256};

use crate::{
    kernel::content_schema::{ContentFieldDefinition, ContentFieldKind, ContentModelDefinition},
    source_graph::SourceGraph,
};

use super::{
    catalog::{AssignmentContract, ContentModelAssignment, ContentModelCatalog},
    CONTENT_MODEL_DIRECTORY, CONTENT_MODEL_SCHEMA_VERSION,
};

pub(super) fn validate_identifier(value: &str, label: &str) -> Result<(), String> {
    let valid = !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        });
    if valid {
        Ok(())
    } else {
        Err(format!(
            "{label} „{value}” acceptă doar litere ASCII, cifre, _ și -."
        ))
    }
}

pub fn validate_model(model: &mut ContentModelDefinition) -> Result<(), String> {
    if model.schema_version != CONTENT_MODEL_SCHEMA_VERSION {
        return Err(format!(
            "Modelul {} folosește schema incompatibilă {}.",
            model.id, model.schema_version
        ));
    }
    validate_identifier(model.id.trim(), "ID-ul modelului")?;
    if model.label.trim().is_empty() {
        return Err("Eticheta modelului este obligatorie.".to_string());
    }
    let mut ids = BTreeSet::new();
    validate_fields(&model.id, &mut model.fields, &mut ids, "")
}

fn validate_fields(
    model_id: &str,
    fields: &mut [ContentFieldDefinition],
    ids: &mut BTreeSet<String>,
    parent_path: &str,
) -> Result<(), String> {
    let mut keys = BTreeSet::new();
    for field in fields {
        validate_identifier(field.key.trim(), "Cheia câmpului")?;
        let path = if parent_path.is_empty() {
            field.key.clone()
        } else {
            format!("{parent_path}.{}", field.key)
        };
        if field.id.trim().is_empty() {
            field.id = stable_field_id(model_id, &path);
        }
        validate_identifier(field.id.trim(), "ID-ul câmpului")?;
        if field.label.trim().is_empty() {
            return Err(format!("Câmpul {} nu are etichetă.", field.key));
        }
        if !ids.insert(field.id.clone()) {
            return Err(format!("ID-ul de câmp „{}” este duplicat.", field.id));
        }
        if !keys.insert(field.key.clone()) {
            return Err(format!("Cheia de câmp „{}” este duplicată.", field.key));
        }
        if field.kind == ContentFieldKind::Select && field.choices.is_empty() {
            return Err(format!(
                "Câmpul select „{}” cere cel puțin o opțiune.",
                field.key
            ));
        }
        if field
            .minimum
            .is_some_and(|minimum| field.maximum.is_some_and(|maximum| minimum > maximum))
        {
            return Err(format!(
                "Limita minimă depășește limita maximă pentru {}.",
                field.key
            ));
        }
        let mut choices = BTreeSet::new();
        if field.kind == ContentFieldKind::Select
            && field.choices.iter().any(|choice| {
                choice.value.trim().is_empty() || !choices.insert(choice.value.clone())
            })
        {
            return Err(format!(
                "Opțiunile câmpului {} trebuie să aibă valori unice și nenule.",
                field.key
            ));
        }
        if !matches!(
            field.kind,
            ContentFieldKind::Group | ContentFieldKind::Repeater
        ) && !field.fields.is_empty()
        {
            return Err(format!(
                "Doar câmpurile group/repeater pot conține subcâmpuri: {}.",
                field.key
            ));
        }
        validate_fields(model_id, &mut field.fields, ids, &path)?;
        if let Some(default_value) = field.default_value.as_ref() {
            validate_value_at_path(field, default_value, &path).map_err(|error| {
                format!(
                    "Valoarea implicită a câmpului {} este invalidă: {error}",
                    field.key
                )
            })?;
        }
    }
    Ok(())
}

pub(super) fn stable_field_id(model_id: &str, key: &str) -> String {
    let digest = Sha256::digest(format!("{model_id}\0{key}").as_bytes());
    let suffix = digest[..6]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("field_{suffix}")
}

pub(super) fn model_path(id: &str) -> String {
    format!("{CONTENT_MODEL_DIRECTORY}/{id}.toml")
}

pub(super) fn require_model<'a>(
    catalog: &'a ContentModelCatalog,
    id: &str,
) -> Result<&'a ContentModelDefinition, String> {
    catalog
        .models
        .iter()
        .find(|model| model.id == id)
        .ok_or_else(|| format!("Modelul „{id}” nu există."))
}

pub(super) fn find_field<'a>(
    fields: &'a [ContentFieldDefinition],
    field_id: &str,
) -> Option<&'a ContentFieldDefinition> {
    for field in fields {
        if field.id == field_id {
            return Some(field);
        }
        if let Some(found) = find_field(&field.fields, field_id) {
            return Some(found);
        }
    }
    None
}

pub(super) fn find_field_mut<'a>(
    fields: &'a mut [ContentFieldDefinition],
    field_id: &str,
) -> Option<&'a mut ContentFieldDefinition> {
    for field in fields {
        if field.id == field_id {
            return Some(field);
        }
        if let Some(found) = find_field_mut(&mut field.fields, field_id) {
            return Some(found);
        }
    }
    None
}

pub(super) fn field_container_mut<'a>(
    fields: &'a mut Vec<ContentFieldDefinition>,
    parent_field_id: Option<&str>,
) -> Result<&'a mut Vec<ContentFieldDefinition>, String> {
    let Some(parent_field_id) = parent_field_id else {
        return Ok(fields);
    };
    let parent = find_field_mut(fields, parent_field_id)
        .ok_or_else(|| format!("Containerul „{parent_field_id}” nu mai există."))?;
    if !matches!(
        parent.kind,
        ContentFieldKind::Group | ContentFieldKind::Repeater
    ) {
        return Err(format!(
            "Câmpul „{}” nu poate conține subcâmpuri.",
            parent.label
        ));
    }
    Ok(&mut parent.fields)
}

pub(super) fn field_path_by_id(
    fields: &[ContentFieldDefinition],
    field_id: &str,
) -> Option<Vec<String>> {
    fn visit(fields: &[ContentFieldDefinition], field_id: &str, path: &mut Vec<String>) -> bool {
        for field in fields {
            path.push(field.key.clone());
            if field.id == field_id || visit(&field.fields, field_id, path) {
                return true;
            }
            path.pop();
        }
        false
    }

    let mut path = Vec::new();
    visit(fields, field_id, &mut path).then_some(path)
}

pub(super) fn field_parent_id_by_id(
    fields: &[ContentFieldDefinition],
    field_id: &str,
) -> Option<String> {
    for field in fields {
        if field.fields.iter().any(|child| child.id == field_id) {
            return Some(field.id.clone());
        }
        if let Some(parent) = field_parent_id_by_id(&field.fields, field_id) {
            return Some(parent);
        }
    }
    None
}

pub(super) fn field_subtree_ids(field: &ContentFieldDefinition) -> BTreeSet<String> {
    let mut ids = BTreeSet::from([field.id.clone()]);
    for child in &field.fields {
        ids.extend(field_subtree_ids(child));
    }
    ids
}

pub(super) fn require_section(graph: &SourceGraph, section_path: &str) -> Result<(), String> {
    if graph.pages.iter().any(|page| {
        page.file == section_path
            && matches!(
                page.page_kind,
                crate::source_graph::model::SourcePageKind::Section
                    | crate::source_graph::model::SourcePageKind::Home
            )
    }) {
        Ok(())
    } else {
        Err(format!("SourceGraph nu conține secțiunea {section_path}."))
    }
}

pub(super) fn serialize_model(model: &ContentModelDefinition) -> Result<String, String> {
    toml_edit::ser::to_string_pretty(model)
        .map_err(|error| format!("Modelul nu poate fi serializat: {error}"))
}

pub(super) fn serialize_assignments(
    assignments: &[ContentModelAssignment],
) -> Result<String, String> {
    toml_edit::ser::to_string_pretty(&AssignmentContract {
        schema_version: CONTENT_MODEL_SCHEMA_VERSION,
        assignments: assignments.to_vec(),
    })
    .map_err(|error| format!("Atribuirile nu pot fi serializate: {error}"))
}

pub(super) fn field_keys(fields: &[ContentFieldDefinition]) -> BTreeSet<String> {
    fields.iter().map(|field| field.key.clone()).collect()
}

pub(super) fn validate_page_values(
    model: &ContentModelDefinition,
    values: &BTreeMap<String, serde_json::Value>,
) -> Result<(), String> {
    let fields = model
        .fields
        .iter()
        .map(|field| (field.key.as_str(), field))
        .collect::<HashMap<_, _>>();
    for key in values.keys() {
        if !fields.contains_key(key.as_str()) {
            return Err(format!(
                "Valoarea „{key}” nu aparține modelului {}.",
                model.id
            ));
        }
    }
    for field in fields.values() {
        let value = values.get(&field.key).or(field.default_value.as_ref());
        if field.required && value.is_none_or(serde_json::Value::is_null) {
            return Err(format!(
                "Câmpul obligatoriu „{}” nu are valoare.",
                field.label
            ));
        }
        if let Some(value) = value.filter(|value| !value.is_null()) {
            validate_value_at_path(field, value, &field.key)?;
        }
    }
    Ok(())
}

pub(super) fn validate_value_at_path(
    field: &ContentFieldDefinition,
    value: &serde_json::Value,
    path: &str,
) -> Result<(), String> {
    let valid = match field.kind {
        ContentFieldKind::Text
        | ContentFieldKind::Textarea
        | ContentFieldKind::Markdown
        | ContentFieldKind::Date
        | ContentFieldKind::Url
        | ContentFieldKind::Color
        | ContentFieldKind::Image
        | ContentFieldKind::Select => value.is_string(),
        ContentFieldKind::Number => value.is_number(),
        ContentFieldKind::Boolean => value.is_boolean(),
        ContentFieldKind::Group => value.is_object(),
        ContentFieldKind::Repeater => value.is_array(),
    };
    if !valid {
        return Err(format!(
            "Valoarea pentru „{}” nu corespunde tipului {:?}.",
            path, field.kind
        ));
    }
    if field.kind == ContentFieldKind::Select {
        let selected = value.as_str().unwrap_or_default();
        if !field.choices.iter().any(|choice| choice.value == selected) {
            return Err(format!(
                "Valoarea „{selected}” nu este o opțiune permisă pentru {}.",
                field.label
            ));
        }
    }
    if let Some(text) = value.as_str() {
        if let Some(pattern) = field
            .pattern
            .as_deref()
            .filter(|pattern| !pattern.is_empty())
        {
            let expression = regex::Regex::new(pattern).map_err(|error| {
                format!(
                    "Pattern-ul câmpului „{}” este invalid: {error}",
                    field.label
                )
            })?;
            if !expression.is_match(text) {
                return Err(format!(
                    "Valoarea pentru „{path}” nu respectă pattern-ul câmpului."
                ));
            }
        }
        match field.kind {
            ContentFieldKind::Date
                if !regex::Regex::new(r"^\d{4}-\d{2}-\d{2}$")
                    .expect("static date regex")
                    .is_match(text) =>
            {
                return Err(format!("Valoarea pentru „{path}” nu este o dată ISO."));
            }
            ContentFieldKind::Url
                if !text.starts_with('/')
                    && !text.starts_with('#')
                    && url::Url::parse(text).is_err() =>
            {
                return Err(format!(
                    "Valoarea pentru „{path}” nu este un URL absolut sau o cale de site."
                ));
            }
            ContentFieldKind::Color
                if !regex::Regex::new(
                    r"^(#[0-9A-Fa-f]{3,4}|#[0-9A-Fa-f]{6}|#[0-9A-Fa-f]{8}|[A-Za-z][A-Za-z0-9-]*)$",
                )
                .expect("static color regex")
                .is_match(text) =>
            {
                return Err(format!(
                    "Valoarea pentru „{path}” nu este o culoare CSS simplă."
                ));
            }
            _ => {}
        }
    }
    if let Some(number) = value.as_f64() {
        if field.minimum.is_some_and(|minimum| number < minimum)
            || field.maximum.is_some_and(|maximum| number > maximum)
        {
            return Err(format!(
                "Valoarea numerică pentru „{}” este în afara limitelor.",
                field.label
            ));
        }
    }
    match field.kind {
        ContentFieldKind::Group => {
            validate_nested_object(
                &field.fields,
                value.as_object().expect("type checked"),
                path,
            )?;
        }
        ContentFieldKind::Repeater => {
            for (index, item) in value.as_array().expect("type checked").iter().enumerate() {
                let object = item.as_object().ok_or_else(|| {
                    format!("Elementul {index} din „{path}” trebuie să fie un obiect.")
                })?;
                validate_nested_object(&field.fields, object, &format!("{path}[{index}]"))?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_nested_object(
    fields: &[ContentFieldDefinition],
    object: &serde_json::Map<String, serde_json::Value>,
    parent_path: &str,
) -> Result<(), String> {
    let definitions = fields
        .iter()
        .map(|field| (field.key.as_str(), field))
        .collect::<HashMap<_, _>>();
    for key in object.keys() {
        if !definitions.contains_key(key.as_str()) {
            return Err(format!(
                "Valoarea „{parent_path}.{key}” nu aparține contractului."
            ));
        }
    }
    for field in fields {
        let path = format!("{parent_path}.{}", field.key);
        let value = object.get(&field.key).or(field.default_value.as_ref());
        if field.required && value.is_none_or(serde_json::Value::is_null) {
            return Err(format!("Câmpul obligatoriu „{path}” nu are valoare."));
        }
        if let Some(value) = value.filter(|value| !value.is_null()) {
            validate_value_at_path(field, value, &path)?;
        }
    }
    Ok(())
}

pub(super) fn collect_missing_required_fields(
    fields: &[ContentFieldDefinition],
    values: &BTreeMap<String, serde_json::Value>,
    parent_path: &str,
    missing: &mut Vec<String>,
) {
    for field in fields {
        let path = if parent_path.is_empty() {
            field.key.clone()
        } else {
            format!("{parent_path}.{}", field.key)
        };
        let value = values.get(&field.key).or(field.default_value.as_ref());
        if field.required && value.is_none_or(serde_json::Value::is_null) {
            missing.push(field.id.clone());
        }
        let Some(value) = value else {
            continue;
        };
        match field.kind {
            ContentFieldKind::Group => {
                if let Some(object) = value.as_object() {
                    let nested = object
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect::<BTreeMap<_, _>>();
                    collect_missing_required_fields(&field.fields, &nested, &path, missing);
                }
            }
            ContentFieldKind::Repeater => {
                for item in value.as_array().into_iter().flatten() {
                    if let Some(object) = item.as_object() {
                        let nested = object
                            .iter()
                            .map(|(key, value)| (key.clone(), value.clone()))
                            .collect::<BTreeMap<_, _>>();
                        collect_missing_required_fields(&field.fields, &nested, &path, missing);
                    }
                }
            }
            _ => {}
        }
    }
}
