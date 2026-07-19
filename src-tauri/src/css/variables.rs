use serde::Serialize;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScssVariable {
    pub name: String,
    pub value: String,
    pub file: String,
}

pub fn update_variable_in_source(source: &str, name: &str, new_value: &str) -> Option<String> {
    let mut result = String::with_capacity(source.len());
    let mut found = false;

    for line in source.lines() {
        let trimmed = line.trim();
        if !found && trimmed.starts_with('$') {
            if let Some(colon) = trimmed.find(':') {
                let var_name = trimmed[1..colon].trim();
                if var_name == name {
                    let leading = &line[..line.len() - line.trim_start().len()];
                    result.push_str(&format!("{}${}: {};\n", leading, name, new_value));
                    found = true;
                    continue;
                }
            }
        }
        result.push_str(line);
        result.push('\n');
    }

    if found {
        Some(result)
    } else {
        None
    }
}

pub fn variable_value_in_source(source: &str, name: &str) -> Option<String> {
    let mut variables = Vec::new();
    parse_variables_from_source(source, "<current-buffer>", &mut variables);
    variables
        .into_iter()
        .find(|variable| variable.name == name)
        .map(|variable| variable.value)
}

pub fn parse_variables_from_source(
    source: &str,
    relative: &str,
    variables: &mut Vec<ScssVariable>,
) {
    let mut in_block_comment = false;

    for line in source.lines() {
        let line = line.trim();

        // block comment handling
        if in_block_comment {
            if line.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }

        if line.starts_with("/*") {
            if !line.contains("*/") {
                in_block_comment = true;
            }
            continue;
        }

        // skip line comments
        if line.starts_with("//") {
            continue;
        }

        // strip inline comment
        let line = strip_inline_comment(line);
        let line = line.trim();

        // match $name: value;
        if !line.starts_with('$') {
            continue;
        }

        let Some(colon) = line.find(':') else {
            continue;
        };

        let name = line[1..colon].trim();

        if name.is_empty() || !is_valid_variable_name(name) {
            continue;
        }

        let rest = line[colon + 1..].trim();
        let value = rest.trim_end_matches(';').trim();

        if value.is_empty() {
            continue;
        }

        variables.push(ScssVariable {
            name: name.to_string(),
            value: value.to_string(),
            file: relative.to_string(),
        });
    }
}

fn strip_inline_comment(line: &str) -> &str {
    if let Some(pos) = line.find("//") {
        // make sure it's not inside a string
        let before = &line[..pos];
        let quotes = before.chars().filter(|&c| c == '"' || c == '\'').count();
        if quotes % 2 == 0 {
            return &line[..pos];
        }
    }
    line
}

fn is_valid_variable_name(name: &str) -> bool {
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Vec<ScssVariable> {
        let mut vars = Vec::new();
        parse_variables_from_source(source, "test.scss", &mut vars);
        vars
    }

    #[test]
    fn parses_simple_variable() {
        let vars = parse("$color-primary: #3b82f6;");
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].name, "color-primary");
        assert_eq!(vars[0].value, "#3b82f6");
    }

    #[test]
    fn parses_clamp_value() {
        let vars = parse("$text-base: clamp(1rem, 0.93rem + 0.35vw, 1.125rem);");
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].value, "clamp(1rem, 0.93rem + 0.35vw, 1.125rem)");
    }

    #[test]
    fn skips_line_comments() {
        let vars = parse("// $color-primary: #3b82f6;");
        assert_eq!(vars.len(), 0);
    }

    #[test]
    fn skips_block_comments() {
        let vars = parse("/* $color-primary: #3b82f6; */");
        assert_eq!(vars.len(), 0);
    }

    #[test]
    fn parses_multiple_variables() {
        let source = "$space-s: 1rem;\n$space-m: 1.5rem;\n$space-l: 2rem;";
        let vars = parse(source);
        assert_eq!(vars.len(), 3);
        assert_eq!(vars[1].name, "space-m");
    }

    #[test]
    fn finds_variable_value_in_current_source() {
        let source = "$bp-mobil: 768px;\n// $bp-mobil: 320px;\n$color: red;";
        assert_eq!(
            variable_value_in_source(source, "bp-mobil").as_deref(),
            Some("768px")
        );
    }

    #[test]
    fn skips_non_variable_lines() {
        let source = ".btn { color: red; }\n$color: blue;";
        let vars = parse(source);
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].name, "color");
    }
}
