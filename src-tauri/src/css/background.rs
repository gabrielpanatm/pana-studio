use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::rules::CssProperty;

pub const CSS_BACKGROUND_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CssBackground {
    pub schema_version: u32,
    pub color: Option<String>,
    pub layers: Vec<CssBackgroundLayer>,
    pub shorthand: Option<String>,
    pub opaque_properties: BTreeMap<String, String>,
    pub structurally_editable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CssBackgroundLayer {
    pub id: String,
    pub kind: CssBackgroundLayerKind,
    pub source: String,
    pub position: String,
    pub size: String,
    pub repeat: String,
    pub attachment: String,
    pub origin: String,
    pub clip: String,
    pub blend_mode: String,
    pub gradient: Option<CssGradient>,
    pub structurally_editable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CssBackgroundLayerKind {
    Image,
    Gradient,
    Opaque,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CssGradient {
    pub kind: CssGradientKind,
    pub repeating: bool,
    pub prelude: String,
    pub items: Vec<CssGradientItem>,
    pub raw: String,
    pub structurally_editable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CssGradientKind {
    Linear,
    Radial,
    Conic,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CssGradientItem {
    Stop {
        id: String,
        color: String,
        positions: Vec<String>,
        raw: String,
    },
    Hint {
        id: String,
        position: String,
        raw: String,
    },
    Opaque {
        id: String,
        raw: String,
    },
}

impl CssBackground {
    pub fn from_rules(rules: &[CssProperty]) -> Self {
        let declarations = rules
            .iter()
            .fold(BTreeMap::new(), |mut values, declaration| {
                values.insert(
                    declaration.property.trim().to_ascii_lowercase(),
                    declaration.value.trim().to_string(),
                );
                values
            });
        Self::from_declarations(&declarations)
    }

    pub fn from_declarations(declarations: &BTreeMap<String, String>) -> Self {
        let color = meaningful_value(declarations.get("background-color"));
        let shorthand = meaningful_value(declarations.get("background"));
        let image_value = declarations
            .get("background-image")
            .map(String::as_str)
            .unwrap_or("")
            .trim();
        let sources = if image_value.is_empty() || image_value.eq_ignore_ascii_case("none") {
            Vec::new()
        } else {
            split_top_level_commas(image_value).unwrap_or_else(|| vec![image_value.to_string()])
        };

        let property_lists = [
            ("position", "background-position", "0% 0%"),
            ("size", "background-size", "auto"),
            ("repeat", "background-repeat", "repeat"),
            ("attachment", "background-attachment", "scroll"),
            ("origin", "background-origin", "padding-box"),
            ("clip", "background-clip", "border-box"),
            ("blend_mode", "background-blend-mode", "normal"),
        ]
        .into_iter()
        .map(|(field, property, default)| {
            let raw = declarations.get(property).map(String::as_str).unwrap_or("");
            let parsed = if raw.trim().is_empty() {
                Some(Vec::new())
            } else if is_opaque_list_expression(raw) {
                None
            } else {
                split_top_level_commas(raw)
            };
            (field, property, default, parsed)
        })
        .collect::<Vec<_>>();

        let opaque_properties = property_lists
            .iter()
            .filter_map(|(_, property, _, list)| {
                list.is_none().then(|| {
                    (
                        (*property).to_string(),
                        declarations.get(*property).cloned().unwrap_or_default(),
                    )
                })
            })
            .collect::<BTreeMap<_, _>>();
        let lists_editable = opaque_properties.is_empty();
        let mut layers = Vec::with_capacity(sources.len());
        for (index, source) in sources.into_iter().enumerate() {
            let gradient = parse_gradient(&source);
            let kind = if gradient.is_some() {
                CssBackgroundLayerKind::Gradient
            } else if function_name(&source).is_some_and(|name| {
                matches!(
                    name.as_str(),
                    "url" | "image" | "image-set" | "cross-fade" | "element"
                )
            }) {
                CssBackgroundLayerKind::Image
            } else {
                CssBackgroundLayerKind::Opaque
            };
            let layer_editable = match kind {
                CssBackgroundLayerKind::Gradient => gradient
                    .as_ref()
                    .is_some_and(|value| value.structurally_editable),
                CssBackgroundLayerKind::Image => true,
                CssBackgroundLayerKind::Opaque => false,
            } && lists_editable;

            let value = |field: &str| {
                property_lists
                    .iter()
                    .find(|(candidate, _, _, _)| *candidate == field)
                    .map(|(_, _, default, list)| repeated_list_value(list.as_ref(), index, default))
                    .unwrap_or_default()
            };
            layers.push(CssBackgroundLayer {
                id: format!("background-layer-{index}"),
                kind,
                source,
                position: value("position"),
                size: value("size"),
                repeat: value("repeat"),
                attachment: value("attachment"),
                origin: value("origin"),
                clip: value("clip"),
                blend_mode: value("blend_mode"),
                gradient,
                structurally_editable: layer_editable,
            });
        }

        let structurally_editable = shorthand.is_none()
            && lists_editable
            && layers.iter().all(|layer| layer.structurally_editable);
        Self {
            schema_version: CSS_BACKGROUND_SCHEMA_VERSION,
            color,
            layers,
            shorthand,
            opaque_properties,
            structurally_editable,
        }
    }

    pub fn to_longhands(&self) -> BTreeMap<String, String> {
        let mut values = BTreeMap::new();
        values.insert(
            "background-color".to_string(),
            self.color.clone().unwrap_or_default(),
        );
        if self.layers.is_empty() {
            values.insert("background-image".to_string(), "none".to_string());
            for property in [
                "background-position",
                "background-size",
                "background-repeat",
                "background-attachment",
                "background-origin",
                "background-clip",
                "background-blend-mode",
            ] {
                values.insert(property.to_string(), String::new());
            }
            for (property, value) in &self.opaque_properties {
                values.insert(property.clone(), value.clone());
            }
            return values;
        }

        let joined = |read: fn(&CssBackgroundLayer) -> &str| {
            self.layers.iter().map(read).collect::<Vec<_>>().join(", ")
        };
        values.insert(
            "background-image".to_string(),
            joined(|layer| &layer.source),
        );
        values.insert(
            "background-position".to_string(),
            joined(|layer| &layer.position),
        );
        values.insert("background-size".to_string(), joined(|layer| &layer.size));
        values.insert(
            "background-repeat".to_string(),
            joined(|layer| &layer.repeat),
        );
        values.insert(
            "background-attachment".to_string(),
            joined(|layer| &layer.attachment),
        );
        values.insert(
            "background-origin".to_string(),
            joined(|layer| &layer.origin),
        );
        values.insert("background-clip".to_string(), joined(|layer| &layer.clip));
        values.insert(
            "background-blend-mode".to_string(),
            joined(|layer| &layer.blend_mode),
        );
        for (property, value) in &self.opaque_properties {
            values.insert(property.clone(), value.clone());
        }
        values
    }
}

fn meaningful_value(value: Option<&String>) -> Option<String> {
    value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn is_opaque_list_expression(value: &str) -> bool {
    let value = value.trim();
    value.starts_with('$') || value.starts_with("var(") || value.starts_with("#{")
}

fn repeated_list_value(values: Option<&Vec<String>>, index: usize, default: &str) -> String {
    let Some(values) = values else {
        return default.to_string();
    };
    if values.is_empty() {
        return default.to_string();
    }
    values[index % values.len()].clone()
}

pub fn split_top_level_commas(value: &str) -> Option<Vec<String>> {
    split_top_level(value, SplitMode::Comma)
}

fn split_top_level_whitespace(value: &str) -> Option<Vec<String>> {
    split_top_level(value, SplitMode::Whitespace)
}

#[derive(Clone, Copy)]
enum SplitMode {
    Comma,
    Whitespace,
}

fn split_top_level(value: &str, mode: SplitMode) -> Option<Vec<String>> {
    let mut result = Vec::new();
    let mut start = 0usize;
    let mut quote = None;
    let mut escaped = false;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut interpolation_depth = 0usize;
    let mut line_comment = false;
    let mut block_comment = false;
    let mut chars = value.char_indices().peekable();

    while let Some((index, ch)) = chars.next() {
        if line_comment {
            if ch == '\n' {
                line_comment = false;
            }
            continue;
        }
        if block_comment {
            if ch == '*' && chars.peek().is_some_and(|(_, next)| *next == '/') {
                chars.next();
                block_comment = false;
            }
            continue;
        }
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }
        if ch == '/' && chars.peek().is_some_and(|(_, next)| *next == '/') {
            chars.next();
            line_comment = true;
            continue;
        }
        if ch == '/' && chars.peek().is_some_and(|(_, next)| *next == '*') {
            chars.next();
            block_comment = true;
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            continue;
        }
        if ch == '#' && chars.peek().is_some_and(|(_, next)| *next == '{') {
            chars.next();
            interpolation_depth += 1;
            continue;
        }
        match ch {
            '(' => paren_depth += 1,
            ')' => {
                if paren_depth == 0 {
                    return None;
                }
                paren_depth -= 1;
            }
            '[' => bracket_depth += 1,
            ']' => {
                if bracket_depth == 0 {
                    return None;
                }
                bracket_depth -= 1;
            }
            '}' if interpolation_depth > 0 => interpolation_depth -= 1,
            ',' if matches!(mode, SplitMode::Comma)
                && paren_depth == 0
                && bracket_depth == 0
                && interpolation_depth == 0 =>
            {
                let part = value[start..index].trim();
                if part.is_empty() {
                    return None;
                }
                result.push(part.to_string());
                start = index + ch.len_utf8();
            }
            whitespace
                if matches!(mode, SplitMode::Whitespace)
                    && whitespace.is_whitespace()
                    && paren_depth == 0
                    && bracket_depth == 0
                    && interpolation_depth == 0 =>
            {
                let part = value[start..index].trim();
                if !part.is_empty() {
                    result.push(part.to_string());
                }
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    if quote.is_some()
        || block_comment
        || paren_depth != 0
        || bracket_depth != 0
        || interpolation_depth != 0
    {
        return None;
    }
    let tail = value[start..].trim();
    if !tail.is_empty() {
        result.push(tail.to_string());
    }
    (!result.is_empty()).then_some(result)
}

fn function_name(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let open = trimmed.find('(')?;
    if !trimmed.ends_with(')') {
        return None;
    }
    let name = trimmed[..open].trim();
    if name.is_empty()
        || !name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return None;
    }
    Some(name.to_ascii_lowercase())
}

pub fn parse_gradient(value: &str) -> Option<CssGradient> {
    let name = function_name(value)?;
    let (kind, repeating) = match name.as_str() {
        "linear-gradient" => (CssGradientKind::Linear, false),
        "repeating-linear-gradient" => (CssGradientKind::Linear, true),
        "radial-gradient" => (CssGradientKind::Radial, false),
        "repeating-radial-gradient" => (CssGradientKind::Radial, true),
        "conic-gradient" => (CssGradientKind::Conic, false),
        "repeating-conic-gradient" => (CssGradientKind::Conic, true),
        _ => return None,
    };
    let open = value.find('(')?;
    let body = &value[open + 1..value.len() - 1];
    let parts = split_top_level_commas(body)?;
    let has_prelude = parts
        .first()
        .is_some_and(|part| is_gradient_prelude(kind, part));
    let prelude = has_prelude
        .then(|| parts[0].trim().to_string())
        .unwrap_or_default();
    let item_parts = &parts[usize::from(has_prelude)..];
    let items = item_parts
        .iter()
        .enumerate()
        .map(|(index, part)| parse_gradient_item(part, index))
        .collect::<Vec<_>>();
    let stop_count = items
        .iter()
        .filter(|item| matches!(item, CssGradientItem::Stop { .. }))
        .count();
    let structurally_editable = stop_count >= 2
        && !items
            .iter()
            .any(|item| matches!(item, CssGradientItem::Opaque { .. }));
    Some(CssGradient {
        kind,
        repeating,
        prelude,
        items,
        raw: value.trim().to_string(),
        structurally_editable,
    })
}

fn is_gradient_prelude(kind: CssGradientKind, value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    match kind {
        CssGradientKind::Linear => {
            normalized.starts_with("to ") || is_angle_token(normalized.as_str())
        }
        CssGradientKind::Radial => {
            normalized.contains(" at ")
                || normalized.starts_with("at ")
                || [
                    "circle",
                    "ellipse",
                    "closest-side",
                    "closest-corner",
                    "farthest-side",
                    "farthest-corner",
                ]
                .iter()
                .any(|prefix| normalized.starts_with(prefix))
        }
        CssGradientKind::Conic => normalized.starts_with("from ") || normalized.starts_with("at "),
    }
}

fn parse_gradient_item(value: &str, index: usize) -> CssGradientItem {
    let raw = value.trim().to_string();
    let Some(tokens) = split_top_level_whitespace(value) else {
        return CssGradientItem::Opaque {
            id: format!("gradient-opaque-{index}"),
            raw,
        };
    };
    if tokens.len() == 1 && is_position_token(&tokens[0]) && !dynamic_position(&tokens[0]) {
        return CssGradientItem::Hint {
            id: format!("gradient-hint-{index}"),
            position: tokens[0].clone(),
            raw,
        };
    }

    let mut position_start = tokens.len();
    while position_start > 0
        && tokens.len() - position_start < 2
        && is_position_token(&tokens[position_start - 1])
    {
        position_start -= 1;
    }
    if position_start == 0 {
        return CssGradientItem::Opaque {
            id: format!("gradient-opaque-{index}"),
            raw,
        };
    }
    let color = tokens[..position_start].join(" ");
    if color.is_empty() {
        return CssGradientItem::Opaque {
            id: format!("gradient-opaque-{index}"),
            raw,
        };
    }
    CssGradientItem::Stop {
        id: format!("gradient-stop-{index}"),
        color,
        positions: tokens[position_start..].to_vec(),
        raw,
    }
}

fn is_angle_token(value: &str) -> bool {
    ["deg", "grad", "rad", "turn"]
        .iter()
        .any(|unit| numeric_unit(value, unit))
        || dynamic_position(value)
}

fn is_position_token(value: &str) -> bool {
    [
        "%", "px", "em", "rem", "ch", "ex", "vw", "vh", "vmin", "vmax", "cm", "mm", "q", "in",
        "pc", "pt", "deg", "grad", "rad", "turn",
    ]
    .iter()
    .any(|unit| numeric_unit(value, unit))
        || value == "0"
        || dynamic_position(value)
}

fn numeric_unit(value: &str, unit: &str) -> bool {
    let Some(number) = value.strip_suffix(unit) else {
        return false;
    };
    !number.is_empty() && number.parse::<f64>().is_ok()
}

fn dynamic_position(value: &str) -> bool {
    value.starts_with('$')
        || ["var(", "calc(", "min(", "max(", "clamp(", "env("]
            .iter()
            .any(|prefix| value.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn property(property: &str, value: &str) -> CssProperty {
        CssProperty {
            property: property.to_string(),
            value: value.to_string(),
        }
    }

    #[test]
    fn top_level_lists_preserve_functions_urls_strings_and_scss_interpolation() {
        assert_eq!(
            split_top_level_commas(
                r#"linear-gradient(rgb(1, 2, 3), var(--x)), url("data:image/svg+xml,a,b"), image-set(url(#{asset($name)}) 1x, url('b,c') 2x)"#,
            ),
            Some(vec![
                "linear-gradient(rgb(1, 2, 3), var(--x))".to_string(),
                r#"url("data:image/svg+xml,a,b")"#.to_string(),
                "image-set(url(#{asset($name)}) 1x, url('b,c') 2x)".to_string(),
            ]),
        );
        assert_eq!(
            split_top_level_commas("url('/a.png') /* keep, comma */, linear-gradient(red, blue)"),
            Some(vec![
                "url('/a.png') /* keep, comma */".to_string(),
                "linear-gradient(red, blue)".to_string(),
            ])
        );
    }

    #[test]
    fn background_lists_follow_css_repetition_and_round_trip_longhands() {
        let model = CssBackground::from_rules(&[
            property(
                "background-image",
                "linear-gradient(45deg, #fff 0%, #000 100%), url('/grain.png'), radial-gradient(circle at center, red 0%, blue 100%)",
            ),
            property("background-position", "center, top left"),
            property("background-size", "cover"),
            property("background-repeat", "no-repeat"),
            property("background-color", "$fundal"),
        ]);
        assert_eq!(model.layers.len(), 3);
        assert_eq!(model.layers[2].position, "center");
        assert_eq!(model.layers[1].size, "cover");
        assert_eq!(model.color.as_deref(), Some("$fundal"));

        let serialized = model.to_longhands();
        let reparsed = CssBackground::from_declarations(&serialized);
        assert_eq!(reparsed.layers.len(), 3);
        assert_eq!(reparsed.layers[2].source, model.layers[2].source);
        assert_eq!(reparsed.layers[2].position, "center");
    }

    #[test]
    fn gradients_preserve_repeating_types_hints_units_and_dynamic_colors() {
        let gradient = parse_gradient(
            "repeating-linear-gradient(to right, $start 0 12px, 18px, color-mix(in oklab, red, blue) 24px 36px)",
        )
        .expect("gradient");
        assert_eq!(gradient.kind, CssGradientKind::Linear);
        assert!(gradient.repeating);
        assert_eq!(gradient.prelude, "to right");
        assert!(gradient.structurally_editable);
        assert!(matches!(gradient.items[1], CssGradientItem::Hint { .. }));
        assert!(matches!(
            &gradient.items[2],
            CssGradientItem::Stop { positions, .. } if positions == &["24px", "36px"]
        ));
    }

    #[test]
    fn shorthand_and_unknown_layers_are_kept_opaque() {
        let shorthand = CssBackground::from_rules(&[property(
            "background",
            "center / cover no-repeat url('/hero.jpg') #111",
        )]);
        assert!(!shorthand.structurally_editable);
        assert_eq!(
            shorthand.shorthand.as_deref(),
            Some("center / cover no-repeat url('/hero.jpg') #111")
        );

        let unknown = CssBackground::from_rules(&[property("background-image", "$fundal-dinamic")]);
        assert_eq!(unknown.layers[0].kind, CssBackgroundLayerKind::Opaque);
        assert!(!unknown.structurally_editable);
        assert_eq!(unknown.layers[0].source, "$fundal-dinamic");

        let dynamic_lists = CssBackground::from_rules(&[
            property("background-image", "url('/a.png'), url('/b.png')"),
            property("background-position", "$pozitii-fundal"),
        ]);
        assert_eq!(
            dynamic_lists
                .opaque_properties
                .get("background-position")
                .map(String::as_str),
            Some("$pozitii-fundal")
        );
        assert_eq!(
            dynamic_lists
                .to_longhands()
                .get("background-position")
                .map(String::as_str),
            Some("$pozitii-fundal")
        );
    }
}
