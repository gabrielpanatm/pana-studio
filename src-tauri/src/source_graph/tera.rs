use crate::source_graph::{
    model::SourceNodeKind,
    tera_cst::{parse_tera_cst, TeraCstKind, TeraCstNode, TeraScopeAction, TeraTagKind},
};

#[derive(Clone, Debug)]
pub struct TeraItem {
    pub kind: TeraItemKind,
    pub node_kind: Option<SourceNodeKind>,
    pub label: String,
    pub target: Option<String>,
    pub targets: Vec<String>,
    pub ignore_missing: bool,
    pub scope_action: TeraScopeAction,
    pub start: usize,
    pub end: usize,
}

impl TeraItem {
    pub fn opens_scope(&self) -> bool {
        self.scope_action == TeraScopeAction::Open
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TeraItemKind {
    Node,
    EndScope,
}

pub fn parse_tera_items(source: &str) -> Vec<TeraItem> {
    let document = parse_tera_cst(source, "pana-source-graph.html");
    tera_items_from_document(&document)
}

pub(crate) fn tera_items_from_document(
    document: &crate::source_graph::tera_cst::TeraCstDocument,
) -> Vec<TeraItem> {
    debug_assert!(document.is_lossless());
    document
        .nodes
        .iter()
        .filter_map(|node| item_from_cst(document.source(), node))
        .collect()
}

fn item_from_cst(source: &str, node: &TeraCstNode) -> Option<TeraItem> {
    let content = node.content(source).trim();
    let base = |kind, node_kind, label: String, scope_action| TeraItem {
        kind,
        node_kind,
        label,
        target: None,
        targets: Vec::new(),
        ignore_missing: false,
        scope_action,
        start: node.start,
        end: node.end,
    };

    match &node.kind {
        TeraCstKind::Text => None,
        TeraCstKind::Variable => Some(base(
            TeraItemKind::Node,
            Some(if content.trim() == "super()" {
                SourceNodeKind::Super
            } else {
                SourceNodeKind::TeraVariable
            }),
            shorten(content, "variable"),
            TeraScopeAction::None,
        )),
        TeraCstKind::Comment => Some(base(
            TeraItemKind::Node,
            Some(SourceNodeKind::TeraComment),
            shorten(content, "comment"),
            TeraScopeAction::None,
        )),
        TeraCstKind::Raw => Some(base(
            TeraItemKind::Node,
            Some(SourceNodeKind::Raw),
            "raw".to_string(),
            TeraScopeAction::None,
        )),
        TeraCstKind::Opaque => Some(base(
            TeraItemKind::Node,
            Some(SourceNodeKind::Tera),
            shorten(content, "opaque"),
            TeraScopeAction::None,
        )),
        TeraCstKind::Tag(tag) => Some(item_from_tag(content, tag, node)),
    }
}

fn item_from_tag(content: &str, tag: &TeraTagKind, node: &TeraCstNode) -> TeraItem {
    let mut item = match tag {
        TeraTagKind::Extends => TeraItem {
            kind: TeraItemKind::Node,
            node_kind: Some(SourceNodeKind::Extends),
            label: target_label("extends", extract_first_string(content).as_deref()),
            target: extract_first_string(content),
            targets: extract_strings(content),
            ignore_missing: false,
            scope_action: tag.scope_action(),
            start: node.start,
            end: node.end,
        },
        TeraTagKind::Include => TeraItem {
            kind: TeraItemKind::Node,
            node_kind: Some(SourceNodeKind::Include),
            label: target_label("include", extract_first_string(content).as_deref()),
            target: extract_first_string(content),
            targets: extract_strings(content),
            ignore_missing: has_ignore_missing(content),
            scope_action: tag.scope_action(),
            start: node.start,
            end: node.end,
        },
        TeraTagKind::Import => TeraItem {
            kind: TeraItemKind::Node,
            node_kind: Some(SourceNodeKind::Import),
            label: target_label("import", extract_first_string(content).as_deref()),
            target: extract_first_string(content),
            targets: extract_strings(content),
            ignore_missing: false,
            scope_action: tag.scope_action(),
            start: node.start,
            end: node.end,
        },
        TeraTagKind::Block => {
            let name = word_after_keyword(content).unwrap_or_else(|| "block".to_string());
            TeraItem {
                kind: TeraItemKind::Node,
                node_kind: Some(SourceNodeKind::Block),
                label: name,
                target: None,
                targets: Vec::new(),
                ignore_missing: false,
                scope_action: tag.scope_action(),
                start: node.start,
                end: node.end,
            }
        }
        TeraTagKind::Macro => {
            let name = macro_name(content).unwrap_or_else(|| "macro".to_string());
            TeraItem {
                kind: TeraItemKind::Node,
                node_kind: Some(SourceNodeKind::Macro),
                label: name,
                target: None,
                targets: Vec::new(),
                ignore_missing: false,
                scope_action: tag.scope_action(),
                start: node.start,
                end: node.end,
            }
        }
        TeraTagKind::For => TeraItem {
            kind: TeraItemKind::Node,
            node_kind: Some(SourceNodeKind::For),
            label: shorten(content, "for"),
            target: None,
            targets: Vec::new(),
            ignore_missing: false,
            scope_action: tag.scope_action(),
            start: node.start,
            end: node.end,
        },
        TeraTagKind::If | TeraTagKind::Elif | TeraTagKind::Else => TeraItem {
            kind: TeraItemKind::Node,
            node_kind: Some(match tag {
                TeraTagKind::If => SourceNodeKind::If,
                TeraTagKind::Elif => SourceNodeKind::Elif,
                TeraTagKind::Else => SourceNodeKind::Else,
                _ => unreachable!(),
            }),
            label: shorten(content, "condition"),
            target: None,
            targets: Vec::new(),
            ignore_missing: false,
            scope_action: tag.scope_action(),
            start: node.start,
            end: node.end,
        },
        TeraTagKind::Set | TeraTagKind::SetGlobal => TeraItem {
            kind: TeraItemKind::Node,
            node_kind: Some(if *tag == TeraTagKind::SetGlobal {
                SourceNodeKind::SetGlobal
            } else {
                SourceNodeKind::Set
            }),
            label: shorten(content, "set"),
            target: None,
            targets: Vec::new(),
            ignore_missing: false,
            scope_action: tag.scope_action(),
            start: node.start,
            end: node.end,
        },
        TeraTagKind::EndBlock
        | TeraTagKind::EndMacro
        | TeraTagKind::EndFor
        | TeraTagKind::EndIf
        | TeraTagKind::EndFilter => TeraItem {
            kind: TeraItemKind::EndScope,
            node_kind: None,
            label: shorten(content, "end"),
            target: None,
            targets: Vec::new(),
            ignore_missing: false,
            scope_action: TeraScopeAction::Close,
            start: node.start,
            end: node.end,
        },
        TeraTagKind::Filter | TeraTagKind::Break | TeraTagKind::Continue => TeraItem {
            kind: TeraItemKind::Node,
            node_kind: Some(match tag {
                TeraTagKind::Filter => SourceNodeKind::Filter,
                TeraTagKind::Break => SourceNodeKind::Break,
                TeraTagKind::Continue => SourceNodeKind::Continue,
                _ => unreachable!(),
            }),
            label: shorten(content, "tera"),
            target: None,
            targets: Vec::new(),
            ignore_missing: false,
            scope_action: tag.scope_action(),
            start: node.start,
            end: node.end,
        },
        TeraTagKind::Raw | TeraTagKind::EndRaw | TeraTagKind::Unknown(_) => TeraItem {
            kind: TeraItemKind::Node,
            node_kind: Some(SourceNodeKind::Tera),
            label: shorten(content, "tera"),
            target: None,
            targets: Vec::new(),
            ignore_missing: false,
            scope_action: tag.scope_action(),
            start: node.start,
            end: node.end,
        },
    };
    if item.target.is_none() {
        item.target = item.targets.first().cloned();
    }
    item
}

pub fn set_assignment_name(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let rest = trimmed
        .strip_prefix("set_global")
        .or_else(|| trimmed.strip_prefix("set"))?
        .trim();
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
    extract_strings(value).into_iter().next()
}

fn extract_strings(value: &str) -> Vec<String> {
    let bytes = value.as_bytes();
    let mut index = 0;
    let mut strings = Vec::new();

    while index < bytes.len() {
        let quote = bytes[index];
        if !matches!(quote, b'"' | b'\'' | b'`') {
            index += 1;
            continue;
        }

        let start = index + 1;
        index = start;
        while index < bytes.len() {
            if bytes[index] == quote {
                strings.push(value[start..index].to_string());
                index += 1;
                break;
            }
            index += 1;
        }
    }

    strings
}

fn has_ignore_missing(value: &str) -> bool {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .contains("ignore missing")
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

    #[test]
    fn parses_the_structural_contract_supported_by_embedded_tera() {
        let source = r#"{% include ["missing.html", "fallback.html"] ignore missing %}
{% set_global total = 0 %}
{% block content %}
{{ super() }}
{% filter upper %}
{% if page.extra.first %}
first
{% elif page.extra.second %}
second
{% else %}
fallback
{% endif %}
{% endfilter %}
{% for item in page.extra.items %}
{% if item.hidden %}{% continue %}{% endif %}
{% if item.stop %}{% break %}{% endif %}
{% endfor %}
{% endblock %}
"#;
        let items = parse_tera_items(source);

        let include = items
            .iter()
            .find(|item| item.node_kind == Some(SourceNodeKind::Include))
            .expect("include");
        assert_eq!(include.targets, vec!["missing.html", "fallback.html"]);
        assert!(include.ignore_missing);
        assert!(items
            .iter()
            .any(|item| item.node_kind == Some(SourceNodeKind::SetGlobal)));
        assert!(items
            .iter()
            .any(|item| { item.node_kind == Some(SourceNodeKind::Filter) && item.opens_scope() }));
        assert!(items
            .iter()
            .any(|item| { item.node_kind == Some(SourceNodeKind::Elif) && !item.opens_scope() }));
        assert!(items
            .iter()
            .any(|item| { item.node_kind == Some(SourceNodeKind::Else) && !item.opens_scope() }));
        assert!(items
            .iter()
            .any(|item| item.node_kind == Some(SourceNodeKind::Break)));
        assert!(items
            .iter()
            .any(|item| item.node_kind == Some(SourceNodeKind::Continue)));
        assert!(items
            .iter()
            .any(|item| item.node_kind == Some(SourceNodeKind::Super)));
    }
}
