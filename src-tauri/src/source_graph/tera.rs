use crate::source_graph::model::SourceNodeKind;

#[derive(Clone)]
pub struct TeraItem {
    pub kind: TeraItemKind,
    pub node_kind: Option<SourceNodeKind>,
    pub label: String,
    pub target: Option<String>,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, PartialEq, Eq)]
pub enum TeraItemKind {
    Node,
    EndScope,
}

pub fn parse_tera_items(source: &str) -> Vec<TeraItem> {
    let bytes = source.as_bytes();
    let mut items = Vec::new();
    let mut index = 0;

    while index + 1 < bytes.len() {
        if bytes[index] != b'{' {
            index += 1;
            continue;
        }

        match bytes[index + 1] {
            b'#' => {
                if let Some(end) = find_close(bytes, index + 2, b'#', b'}') {
                    let content = source[index + 2..end - 2].trim();
                    items.push(TeraItem {
                        kind: TeraItemKind::Node,
                        node_kind: Some(SourceNodeKind::TeraComment),
                        label: shorten(content, "comment"),
                        target: None,
                        start: index,
                        end,
                    });
                    index = end;
                } else {
                    break;
                }
            }
            b'{' => {
                if let Some(end) = find_close(bytes, index + 2, b'}', b'}') {
                    let content = trim_tera_markers(&source[index + 2..end - 2]);
                    items.push(TeraItem {
                        kind: TeraItemKind::Node,
                        node_kind: Some(SourceNodeKind::TeraVariable),
                        label: shorten(&content, "variable"),
                        target: None,
                        start: index,
                        end,
                    });
                    index = end;
                } else {
                    break;
                }
            }
            b'%' => {
                if let Some(end) = find_close(bytes, index + 2, b'%', b'}') {
                    let content = trim_tera_markers(&source[index + 2..end - 2]);
                    let keyword = first_word(&content);

                    if keyword == "raw" {
                        if let Some(raw_end) = find_endraw(source, end) {
                            items.push(TeraItem {
                                kind: TeraItemKind::Node,
                                node_kind: Some(SourceNodeKind::Raw),
                                label: "raw".to_string(),
                                target: None,
                                start: index,
                                end: raw_end,
                            });
                            index = raw_end;
                            continue;
                        }
                    }

                    items.push(item_from_tag(&content, keyword, index, end));
                    index = end;
                } else {
                    break;
                }
            }
            _ => {
                index += 1;
            }
        }
    }

    items
}

fn item_from_tag(content: &str, keyword: &str, start: usize, end: usize) -> TeraItem {
    match keyword {
        "extends" => TeraItem {
            kind: TeraItemKind::Node,
            node_kind: Some(SourceNodeKind::Extends),
            label: target_label("extends", extract_first_string(content).as_deref()),
            target: extract_first_string(content),
            start,
            end,
        },
        "include" => TeraItem {
            kind: TeraItemKind::Node,
            node_kind: Some(SourceNodeKind::Include),
            label: target_label("include", extract_first_string(content).as_deref()),
            target: extract_first_string(content),
            start,
            end,
        },
        "import" => TeraItem {
            kind: TeraItemKind::Node,
            node_kind: Some(SourceNodeKind::Import),
            label: target_label("import", extract_first_string(content).as_deref()),
            target: extract_first_string(content),
            start,
            end,
        },
        "block" => {
            let name = word_after_keyword(content).unwrap_or_else(|| "block".to_string());
            TeraItem {
                kind: TeraItemKind::Node,
                node_kind: Some(SourceNodeKind::Block),
                label: name,
                target: None,
                start,
                end,
            }
        }
        "macro" => {
            let name = macro_name(content).unwrap_or_else(|| "macro".to_string());
            TeraItem {
                kind: TeraItemKind::Node,
                node_kind: Some(SourceNodeKind::Macro),
                label: name,
                target: None,
                start,
                end,
            }
        }
        "for" => TeraItem {
            kind: TeraItemKind::Node,
            node_kind: Some(SourceNodeKind::For),
            label: shorten(content, "for"),
            target: None,
            start,
            end,
        },
        "if" | "elif" | "else" => TeraItem {
            kind: TeraItemKind::Node,
            node_kind: Some(SourceNodeKind::If),
            label: shorten(content, keyword),
            target: None,
            start,
            end,
        },
        "set" => TeraItem {
            kind: TeraItemKind::Node,
            node_kind: Some(SourceNodeKind::Set),
            label: shorten(content, "set"),
            target: None,
            start,
            end,
        },
        "with" => TeraItem {
            kind: TeraItemKind::Node,
            node_kind: Some(SourceNodeKind::With),
            label: shorten(content, "with"),
            target: None,
            start,
            end,
        },
        "endblock" | "endfor" | "endif" | "endmacro" | "endset" | "endwith" => TeraItem {
            kind: TeraItemKind::EndScope,
            node_kind: None,
            label: keyword.to_string(),
            target: None,
            start,
            end,
        },
        _ => TeraItem {
            kind: TeraItemKind::Node,
            node_kind: Some(SourceNodeKind::Tera),
            label: shorten(content, keyword),
            target: None,
            start,
            end,
        },
    }
}

fn find_close(bytes: &[u8], mut index: usize, close_a: u8, close_b: u8) -> Option<usize> {
    while index + 1 < bytes.len() {
        if bytes[index] == close_a && bytes[index + 1] == close_b {
            return Some(index + 2);
        }
        index += 1;
    }
    None
}

fn find_endraw(source: &str, start: usize) -> Option<usize> {
    let rest = &source[start..];
    let relative = rest.find("{% endraw")?;
    let token_start = start + relative;
    let bytes = source.as_bytes();
    find_close(bytes, token_start + 2, b'%', b'}')
}

fn trim_tera_markers(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('-')
        .trim_end_matches('-')
        .trim()
        .to_string()
}

fn first_word(value: &str) -> &str {
    value.split_whitespace().next().unwrap_or("")
}

pub fn set_assignment_name(value: &str) -> Option<String> {
    let rest = value.trim().strip_prefix("set")?.trim();
    let (before_equals, _) = rest.split_once('=')?;
    first_identifier(before_equals)
}

pub fn for_collection_root(value: &str) -> Option<String> {
    let rest = value.trim().strip_prefix("for")?.trim();
    let tokens = rest.split_whitespace().collect::<Vec<_>>();
    let in_index = tokens.iter().position(|token| *token == "in")?;
    let collection = tokens.get(in_index + 1..)?.join(" ");
    first_identifier(&collection)
}

fn first_identifier(value: &str) -> Option<String> {
    let trimmed = value
        .trim()
        .trim_start_matches(|character: char| !is_identifier_start(character));
    let mut chars = trimmed.chars();
    let first = chars.next()?;
    if !is_identifier_start(first) {
        return None;
    }
    let mut identifier = String::from(first);
    for character in chars {
        if is_identifier_continue(character) {
            identifier.push(character);
        } else {
            break;
        }
    }
    Some(identifier).filter(|identifier| !identifier.is_empty())
}

fn is_identifier_start(character: char) -> bool {
    character == '_' || character.is_ascii_alphabetic()
}

fn is_identifier_continue(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

fn word_after_keyword(value: &str) -> Option<String> {
    value
        .split_whitespace()
        .nth(1)
        .map(|word| {
            word.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
                .to_string()
        })
        .filter(|word| !word.is_empty())
}

fn macro_name(value: &str) -> Option<String> {
    let rest = value.trim_start_matches("macro").trim();
    let name = rest.split('(').next()?.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn extract_first_string(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        let quote = bytes[index];
        if quote != b'"' && quote != b'\'' {
            index += 1;
            continue;
        }

        let start = index + 1;
        index = start;
        while index < bytes.len() {
            if bytes[index] == quote && bytes[index.saturating_sub(1)] != b'\\' {
                return Some(value[start..index].to_string());
            }
            index += 1;
        }
        break;
    }

    None
}

fn target_label(kind: &str, target: Option<&str>) -> String {
    target
        .map(|target| format!("{kind} {target}"))
        .unwrap_or_else(|| kind.to_string())
}

fn shorten(value: &str, fallback: &str) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        return fallback.to_string();
    }
    if compact.chars().count() <= 80 {
        compact
    } else {
        format!("{}...", compact.chars().take(77).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_core_tera_relations() {
        let source = r#"{% extends "base.html" %}
{% import "macros/cards.html" as cards %}
{% block content %}
{% include "partials/header.html" %}
{{ section.title }}
{% endblock %}
"#;
        let items = parse_tera_items(source);
        assert!(items
            .iter()
            .any(|item| item.node_kind == Some(SourceNodeKind::Extends)
                && item.target.as_deref() == Some("base.html")));
        assert!(items
            .iter()
            .any(|item| item.node_kind == Some(SourceNodeKind::Import)
                && item.target.as_deref() == Some("macros/cards.html")));
        assert!(items
            .iter()
            .any(|item| item.node_kind == Some(SourceNodeKind::Block) && item.label == "content"));
        assert!(items
            .iter()
            .any(|item| item.node_kind == Some(SourceNodeKind::Include)
                && item.target.as_deref() == Some("partials/header.html")));
        assert!(items
            .iter()
            .any(|item| item.node_kind == Some(SourceNodeKind::TeraVariable)));
    }
}
