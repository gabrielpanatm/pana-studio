use std::collections::HashMap;

const MAX_SELECTOR_BYTES: usize = 4 * 1024;
const MAX_PROPERTY_BYTES: usize = 256;
const MAX_VALUE_BYTES: usize = 64 * 1024;

pub fn validate_panel_rule_input(
    selector: &str,
    properties: &HashMap<String, String>,
    viewport: &str,
) -> Result<(), String> {
    validate_panel_selector(selector)?;
    if !matches!(viewport, "desktop" | "tablet" | "mobile") {
        return Err(format!(
            "[css_panel_invalid_viewport] Viewport CSS necunoscut: {viewport}."
        ));
    }
    for (property, value) in properties {
        validate_panel_property(property)?;
        validate_panel_value(value, true)?;
    }
    Ok(())
}

pub fn validate_panel_variable_value(value: &str) -> Result<(), String> {
    validate_panel_value(value, false)
}

fn validate_panel_selector(selector: &str) -> Result<(), String> {
    let selector = selector.trim();
    if selector.is_empty() || selector.len() > MAX_SELECTOR_BYTES {
        return Err(
            "[css_panel_invalid_selector] Selectorul CSS este gol sau prea lung.".to_string(),
        );
    }
    if selector
        .chars()
        .any(|character| character.is_control() || matches!(character, '{' | '}' | ';'))
    {
        return Err(
            "[css_panel_invalid_selector] Selectorul conține separatori care nu aparțin unui selector."
                .to_string(),
        );
    }
    validate_balanced_text(selector, "selector")
}

fn validate_panel_property(property: &str) -> Result<(), String> {
    let property = property.trim();
    let valid = !property.is_empty()
        && property.len() <= MAX_PROPERTY_BYTES
        && property
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-');
    if !valid
        || property
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_digit())
    {
        return Err(format!(
            "[css_panel_invalid_property] Numele proprietății CSS nu este valid: {property}."
        ));
    }
    Ok(())
}

fn validate_panel_value(value: &str, allow_empty: bool) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() {
        return if allow_empty {
            Ok(())
        } else {
            Err(
                "[css_panel_empty_variable] O variabilă SCSS nu poate avea valoare goală."
                    .to_string(),
            )
        };
    }
    if value.len() > MAX_VALUE_BYTES {
        return Err("[css_panel_value_too_long] Valoarea CSS/SCSS este prea lungă.".to_string());
    }
    if value
        .chars()
        .any(|character| character == '\0' || character == '\r' || character == '\n')
    {
        return Err(
            "[css_panel_invalid_value] Panoul CSS acceptă o singură expresie, fără linii noi."
                .to_string(),
        );
    }
    validate_balanced_text(value, "valoare")?;

    let mut quote = None;
    let mut escaped = false;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    for character in value.chars() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(character, '\'' | '"') {
            quote = Some(character);
            continue;
        }
        match character {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            ';' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                return Err(
                    "[css_panel_invalid_value] Valoarea conține un separator de declarații la nivel superior."
                        .to_string(),
                );
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_balanced_text(value: &str, label: &str) -> Result<(), String> {
    let mut stack = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    for character in value.chars() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(character, '\'' | '"') {
            quote = Some(character);
            continue;
        }
        match character {
            '(' | '[' | '{' => stack.push(character),
            ')' | ']' | '}' => {
                let expected = match character {
                    ')' => '(',
                    ']' => '[',
                    '}' => '{',
                    _ => unreachable!(),
                };
                if stack.pop() != Some(expected) {
                    return Err(format!(
                        "[css_panel_unbalanced_expression] {label} CSS/SCSS are delimitatori dezechilibrați."
                    ));
                }
            }
            _ => {}
        }
    }
    if quote.is_some() || !stack.is_empty() {
        return Err(format!(
            "[css_panel_unbalanced_expression] {label} CSS/SCSS este incompletă."
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_property_value_is_a_valid_delete_intent() {
        let mut properties = HashMap::new();
        properties.insert("text-align".to_string(), String::new());
        assert!(validate_panel_rule_input(".hero:hover", &properties, "desktop").is_ok());
    }

    #[test]
    fn rejects_incomplete_or_injected_values_before_workspace_mutation() {
        for value in [
            "linear-gradient(",
            "url(\"image.png\"",
            "red; display: none",
        ] {
            assert!(validate_panel_value(value, true).is_err(), "{value}");
        }
    }

    #[test]
    fn accepts_scss_expressions_used_by_the_panel() {
        for value in [
            "$space-m",
            "clamp(1rem, 2vw + 1rem, 3rem)",
            "rgba(0, 0, 0, 0.25)",
            "calc(100% - #{$space-m})",
            "url(\"/imagini/a;b.png\")",
        ] {
            assert!(validate_panel_value(value, false).is_ok(), "{value}");
        }
    }

    #[test]
    fn variable_values_cannot_be_committed_empty() {
        assert!(validate_panel_variable_value("  ").is_err());
    }

    #[test]
    fn rejects_unknown_viewports_and_selector_blocks() {
        let properties = HashMap::from([("color".to_string(), "red".to_string())]);
        assert!(validate_panel_rule_input(".hero", &properties, "watch").is_err());
        assert!(validate_panel_rule_input(".hero { body", &properties, "desktop").is_err());
    }
}
