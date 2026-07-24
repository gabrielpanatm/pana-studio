use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    blocks::{
        native::{NativeBlockOptionDefault, NativeBlockOptionDefinition},
        native_block_by_id,
    },
    project_model::attribute_engine::raw_tag_attributes,
    source_graph::model::BlockOptionValue,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeBlockOptionIntent {
    pub provider_id: String,
    pub option_id: String,
    pub value: BlockOptionValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NativeBlockMarkerKind {
    Canonical,
    Legacy,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeBlockOptionState {
    pub id: String,
    pub value: BlockOptionValue,
    pub is_default: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeBlockSourceInspection {
    pub provider_id: String,
    pub marker_kind: NativeBlockMarkerKind,
    pub editable: bool,
    pub diagnostic: Option<String>,
    pub definition: Option<crate::source_graph::model::BlockDefinition>,
    pub options: Vec<NativeBlockOptionState>,
}

pub(crate) fn inspect_native_block_source(
    opening_tag: &str,
) -> Result<NativeBlockSourceInspection, String> {
    let attributes = tag_attribute_map(opening_tag);
    let canonical = attribute_value(&attributes, "data-pana-block");
    let legacy = attribute_value(&attributes, "data-pana-component");
    let (provider_id, marker_kind) = match (canonical, legacy) {
        (Some(provider), _) if !provider.trim().is_empty() => (
            provider.trim().to_string(),
            NativeBlockMarkerKind::Canonical,
        ),
        (None, Some(provider)) if !provider.trim().is_empty() => {
            (provider.trim().to_string(), NativeBlockMarkerKind::Legacy)
        }
        _ => {
            return Err(
                "Elementul sursă nu are un marcaj data-pana-block sau data-pana-component valid."
                    .to_string(),
            );
        }
    };

    let Some(block) = native_block_by_id(&provider_id) else {
        return Ok(NativeBlockSourceInspection {
            provider_id: provider_id.clone(),
            marker_kind,
            editable: false,
            diagnostic: Some(format!(
                "Providerul `{provider_id}` nu există în NativeBlockRegistry. Instanța rămâne read-only."
            )),
            definition: None,
            options: Vec::new(),
        });
    };
    let mut diagnostics = Vec::new();
    let options = block
        .options
        .iter()
        .map(|option| option_state_from_attributes(option, &attributes, &mut diagnostics))
        .collect();
    let editable = marker_kind == NativeBlockMarkerKind::Canonical;
    if !editable {
        diagnostics.push(
            "Marcajul data-pana-component este compatibil la citire, dar proprietățile sale sunt read-only."
                .to_string(),
        );
    }

    Ok(NativeBlockSourceInspection {
        provider_id,
        marker_kind,
        editable,
        diagnostic: (!diagnostics.is_empty()).then(|| diagnostics.join(" ")),
        definition: Some(crate::blocks::native_block_contract_definition(block)),
        options,
    })
}

pub(crate) fn plan_native_block_option_attribute(
    opening_tag: &str,
    intent: &NativeBlockOptionIntent,
) -> Result<(String, Option<String>), String> {
    let inspection = inspect_native_block_source(opening_tag)?;
    if inspection.provider_id != intent.provider_id.trim() {
        return Err(format!(
            "Instanța sursă aparține providerului `{}`, nu providerului `{}`.",
            inspection.provider_id,
            intent.provider_id.trim()
        ));
    }
    if !inspection.editable {
        return Err(inspection
            .diagnostic
            .unwrap_or_else(|| "Instanța legacy sau necunoscută este read-only.".to_string()));
    }
    let block = native_block_by_id(&inspection.provider_id).ok_or_else(|| {
        format!(
            "Providerul `{}` nu există în NativeBlockRegistry.",
            inspection.provider_id
        )
    })?;
    let option = block
        .options
        .iter()
        .find(|option| option.id == intent.option_id.trim())
        .ok_or_else(|| {
            format!(
                "Opțiunea `{}` nu aparține providerului `{}`.",
                intent.option_id.trim(),
                inspection.provider_id
            )
        })?;
    let value = validate_option_value(option, &intent.value)?;
    let serialized = if option.omit_when_default && value == default_value(option) {
        None
    } else {
        Some(serialize_option_value(&value))
    };
    Ok((option.attribute.to_string(), serialized))
}

fn option_state_from_attributes(
    option: &NativeBlockOptionDefinition,
    attributes: &BTreeMap<String, Option<String>>,
    diagnostics: &mut Vec<String>,
) -> NativeBlockOptionState {
    let fallback = default_value(option);
    let value = attribute_value(attributes, option.attribute)
        .map(|raw| parse_source_option_value(option, raw))
        .transpose()
        .unwrap_or_else(|error| {
            diagnostics.push(error);
            None
        })
        .unwrap_or_else(|| fallback.clone());
    NativeBlockOptionState {
        id: option.id.to_string(),
        is_default: value == fallback,
        value,
    }
}

fn parse_source_option_value(
    option: &NativeBlockOptionDefinition,
    raw: &str,
) -> Result<BlockOptionValue, String> {
    let value = match option.default_value {
        NativeBlockOptionDefault::Boolean(_) => {
            let parsed = match raw.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" => true,
                "false" | "0" | "no" => false,
                _ => {
                    return Err(format!(
                        "Valoarea `{raw}` pentru `{}` nu este booleană; este afișat default-ul.",
                        option.id
                    ))
                }
            };
            BlockOptionValue::Boolean(parsed)
        }
        NativeBlockOptionDefault::Integer(_) => {
            let parsed = raw.trim().parse::<i64>().map_err(|_| {
                format!(
                    "Valoarea `{raw}` pentru `{}` nu este un număr întreg; este afișat default-ul.",
                    option.id
                )
            })?;
            BlockOptionValue::Integer(parsed)
        }
        NativeBlockOptionDefault::Text(_) => BlockOptionValue::Text(decode_attribute_value(raw)),
    };
    validate_option_value(option, &value)
        .map_err(|error| format!("{error} Este afișat default-ul."))
}

fn validate_option_value(
    option: &NativeBlockOptionDefinition,
    value: &BlockOptionValue,
) -> Result<BlockOptionValue, String> {
    match (option.default_value, value) {
        (NativeBlockOptionDefault::Boolean(_), BlockOptionValue::Boolean(value)) => {
            Ok(BlockOptionValue::Boolean(*value))
        }
        (NativeBlockOptionDefault::Integer(_), BlockOptionValue::Integer(value)) => {
            if option.minimum.is_some_and(|minimum| *value < minimum)
                || option.maximum.is_some_and(|maximum| *value > maximum)
            {
                return Err(format!(
                    "Valoarea pentru `{}` trebuie să fie între {} și {}.",
                    option.id,
                    option.minimum.unwrap_or(i64::MIN),
                    option.maximum.unwrap_or(i64::MAX)
                ));
            }
            if let Some(step) = option.step.filter(|step| *step > 0) {
                let origin = option.minimum.unwrap_or(0);
                if (*value - origin).rem_euclid(step) != 0 {
                    return Err(format!(
                        "Valoarea pentru `{}` trebuie să respecte pasul {step}.",
                        option.id
                    ));
                }
            }
            Ok(BlockOptionValue::Integer(*value))
        }
        (NativeBlockOptionDefault::Text(_), BlockOptionValue::Text(value)) => {
            if value.contains(['\n', '\r', '\0']) {
                return Err(format!(
                    "Valoarea pentru `{}` nu poate conține linii noi sau caractere nule.",
                    option.id
                ));
            }
            if option
                .maximum_length
                .is_some_and(|maximum| value.chars().count() > maximum)
            {
                return Err(format!(
                    "Valoarea pentru `{}` poate avea cel mult {} caractere.",
                    option.id,
                    option.maximum_length.unwrap_or_default()
                ));
            }
            if !option.choices.is_empty()
                && !option
                    .choices
                    .iter()
                    .any(|(candidate, _)| *candidate == value)
            {
                return Err(format!(
                    "Valoarea `{value}` nu este permisă pentru `{}`.",
                    option.id
                ));
            }
            Ok(BlockOptionValue::Text(value.clone()))
        }
        _ => Err(format!(
            "Tipul valorii nu corespunde opțiunii `{}` din NativeBlockRegistry.",
            option.id
        )),
    }
}

fn default_value(option: &NativeBlockOptionDefinition) -> BlockOptionValue {
    option.default_value.into()
}

fn serialize_option_value(value: &BlockOptionValue) -> String {
    match value {
        BlockOptionValue::Boolean(value) => value.to_string(),
        BlockOptionValue::Integer(value) => value.to_string(),
        BlockOptionValue::Text(value) => value.clone(),
    }
}

fn tag_attribute_map(opening_tag: &str) -> BTreeMap<String, Option<String>> {
    raw_tag_attributes(opening_tag)
        .into_iter()
        .map(|attribute| (attribute.name, attribute.value))
        .collect()
}

fn attribute_value<'a>(
    attributes: &'a BTreeMap<String, Option<String>>,
    name: &str,
) -> Option<&'a str> {
    attributes.get(name).and_then(|value| value.as_deref())
}

fn decode_attribute_value(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_defaults_from_canonical_source_markup() {
        let inspection = inspect_native_block_source(
            r#"<span data-pana-block="counter" data-tinta="200" data-sufix=" ani">"#,
        )
        .expect("inspection");

        assert!(inspection.editable);
        assert_eq!(inspection.options.len(), 3);
        assert_eq!(inspection.options[0].value, BlockOptionValue::Integer(200));
        assert_eq!(inspection.options[1].value, BlockOptionValue::Integer(1800));
    }

    #[test]
    fn legacy_markup_is_read_only() {
        let inspection = inspect_native_block_source(r#"<div data-pana-component="accordion">"#)
            .expect("inspection");

        assert!(!inspection.editable);
        assert_eq!(inspection.marker_kind, NativeBlockMarkerKind::Legacy);
    }

    #[test]
    fn canonical_write_omits_default_when_contract_requires_it() {
        let mutation = plan_native_block_option_attribute(
            r#"<div data-pana-block="accordion" data-multiple="true">"#,
            &NativeBlockOptionIntent {
                provider_id: "accordion".to_string(),
                option_id: "allowMultiple".to_string(),
                value: BlockOptionValue::Boolean(false),
            },
        )
        .expect("mutation");

        assert_eq!(mutation, ("data-multiple".to_string(), None));
    }

    #[test]
    fn invalid_value_is_rejected_by_rust_schema() {
        let error = plan_native_block_option_attribute(
            r#"<div data-pana-block="offcanvas">"#,
            &NativeBlockOptionIntent {
                provider_id: "offcanvas".to_string(),
                option_id: "side".to_string(),
                value: BlockOptionValue::Text("middle".to_string()),
            },
        )
        .expect_err("invalid side");

        assert!(error.contains("nu este permisă"));
    }
}
