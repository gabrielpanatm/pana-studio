#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssDeclaration {
    pub property: String,
    pub value: String,
}

pub fn parse_declarations(content: &str) -> Vec<CssDeclaration> {
    declaration_fragments(content)
        .into_iter()
        .filter_map(|fragment| parse_declaration_fragment(fragment))
        .collect()
}

fn declaration_fragments(content: &str) -> Vec<&str> {
    let mut fragments = Vec::new();
    let mut start = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut line_comment = false;
    let mut block_comment = false;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut chars = content.char_indices().peekable();

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
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == active_quote {
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

        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            continue;
        }

        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' if paren_depth == 0 && bracket_depth == 0 => {
                brace_depth += 1;
            }
            '}' if paren_depth == 0 && bracket_depth == 0 => {
                brace_depth = brace_depth.saturating_sub(1);
                if brace_depth == 0 {
                    start = index + ch.len_utf8();
                }
            }
            ';' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                if start < index {
                    fragments.push(&content[start..index]);
                }
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }

    if start < content.len() {
        fragments.push(&content[start..]);
    }

    fragments
}

fn parse_declaration_fragment(fragment: &str) -> Option<CssDeclaration> {
    let cleaned = strip_comments(fragment);
    let value = cleaned.trim();
    if value.is_empty() || value.contains('{') || value.contains('}') {
        return None;
    }

    let colon = top_level_colon(value)?;
    let property = value[..colon].trim();
    let declaration_value = value[colon + 1..].trim().trim_end_matches(';').trim();

    if property.is_empty() || declaration_value.is_empty() || property.contains('{') {
        return None;
    }

    Some(CssDeclaration {
        property: property.to_string(),
        value: declaration_value.to_string(),
    })
}

fn top_level_colon(value: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;

    for (index, ch) in value.char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == active_quote {
                quote = None;
            }
            continue;
        }

        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            continue;
        }

        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            ':' if paren_depth == 0 && bracket_depth == 0 => return Some(index),
            _ => {}
        }
    }

    None
}

fn strip_comments(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.char_indices().peekable();
    let mut line_comment = false;
    let mut block_comment = false;

    while let Some((_, ch)) = chars.next() {
        if line_comment {
            if ch == '\n' {
                line_comment = false;
                result.push('\n');
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

        result.push(ch);
    }

    result
}
