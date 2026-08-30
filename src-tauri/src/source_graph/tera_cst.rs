use crate::source_graph::tera_semantics::TeraSemanticDocument;

#[derive(Clone, Debug)]
pub(crate) struct TeraCstDocument {
    source: String,
    pub(crate) nodes: Vec<TeraCstNode>,
    pub(crate) structurally_valid: bool,
    pub(crate) semantics: Option<TeraSemanticDocument>,
    pub(crate) validation_error: Option<String>,
}

impl TeraCstDocument {
    pub(crate) fn source(&self) -> &str {
        &self.source
    }

    pub(crate) fn is_valid_tera(&self) -> bool {
        self.structurally_valid && self.validation_error.is_none()
    }

    pub(crate) fn validation_error(&self) -> Option<&str> {
        self.validation_error.as_deref()
    }

    pub(crate) fn semantics(&self) -> Option<&TeraSemanticDocument> {
        self.semantics.as_ref()
    }

    pub(crate) fn reconstruct(&self) -> String {
        let mut reconstructed = String::with_capacity(self.source.len());
        for node in &self.nodes {
            reconstructed.push_str(node.full_text(&self.source));
        }
        reconstructed
    }

    pub(crate) fn is_lossless(&self) -> bool {
        let mut cursor = 0usize;
        for node in &self.nodes {
            if node.start != cursor
                || node.end < node.start
                || node.end > self.source.len()
                || !self.source.is_char_boundary(node.start)
                || !self.source.is_char_boundary(node.end)
            {
                return false;
            }
            cursor = node.end;
        }
        cursor == self.source.len() && self.reconstruct() == self.source
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TeraCstNode {
    pub(crate) kind: TeraCstKind,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) content_start: usize,
    pub(crate) content_end: usize,
}

impl TeraCstNode {
    pub(crate) fn full_text<'a>(&self, source: &'a str) -> &'a str {
        source.get(self.start..self.end).unwrap_or("")
    }

    pub(crate) fn content<'a>(&self, source: &'a str) -> &'a str {
        source
            .get(self.content_start..self.content_end)
            .unwrap_or("")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TeraCstKind {
    Text,
    Variable,
    Comment,
    Raw,
    Tag(TeraTagKind),
    Opaque,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TeraTagKind {
    Extends,
    Include,
    Block,
    EndBlock,
    ComponentDefinition,
    EndComponentDefinition,
    ComponentCall,
    EndComponentCall,
    LegacyImport,
    LegacyDefinition,
    EndLegacyDefinition,
    For,
    EndFor,
    If,
    Elif,
    Else,
    EndIf,
    Set,
    SetGlobal,
    SetBlock,
    EndSetBlock,
    Filter,
    EndFilter,
    Break,
    Continue,
    Raw,
    EndRaw,
    Unknown(String),
}

impl TeraTagKind {
    pub(crate) fn scope_action(&self) -> TeraScopeAction {
        match self {
            Self::Block
            | Self::ComponentDefinition
            | Self::ComponentCall
            | Self::LegacyDefinition
            | Self::For
            | Self::If
            | Self::SetBlock
            | Self::Filter => TeraScopeAction::Open,
            Self::Elif | Self::Else => TeraScopeAction::Branch,
            Self::EndBlock
            | Self::EndComponentDefinition
            | Self::EndComponentCall
            | Self::EndLegacyDefinition
            | Self::EndFor
            | Self::EndIf
            | Self::EndSetBlock
            | Self::EndFilter => TeraScopeAction::Close,
            _ => TeraScopeAction::None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TeraScopeAction {
    None,
    Open,
    Branch,
    Close,
}

pub(crate) fn parse_tera_cst(source: &str, template_name: &str) -> TeraCstDocument {
    let bytes = source.as_bytes();
    let mut nodes = Vec::new();
    let mut cursor = 0usize;

    while cursor < bytes.len() {
        let Some(start) = find_next_delimiter(bytes, cursor) else {
            push_text(&mut nodes, cursor, bytes.len());
            break;
        };
        push_text(&mut nodes, cursor, start);

        let delimiter = bytes[start + 1];
        let scanned = match delimiter {
            b'{' => scan_token_end(bytes, start + 2, b'}', b'}', true),
            b'%' => scan_token_end(bytes, start + 2, b'%', b'}', true),
            b'#' => scan_token_end(bytes, start + 2, b'#', b'}', false),
            _ => None,
        };
        let Some(end) = scanned else {
            nodes.push(TeraCstNode {
                kind: TeraCstKind::Opaque,
                start,
                end: bytes.len(),
                content_start: start,
                content_end: bytes.len(),
            });
            cursor = bytes.len();
            continue;
        };
        let (content_start, content_end) = content_bounds(bytes, start, end);

        match delimiter {
            b'{' => nodes.push(TeraCstNode {
                kind: TeraCstKind::Variable,
                start,
                end,
                content_start,
                content_end,
            }),
            b'#' => nodes.push(TeraCstNode {
                kind: TeraCstKind::Comment,
                start,
                end,
                content_start,
                content_end,
            }),
            b'%' => {
                let tag = classify_tag(source.get(content_start..content_end).unwrap_or("").trim());
                if tag == TeraTagKind::Raw {
                    if let Some((raw_close_start, raw_end)) = find_raw_close(source, end) {
                        nodes.push(TeraCstNode {
                            kind: TeraCstKind::Raw,
                            start,
                            end: raw_end,
                            content_start: end,
                            content_end: raw_close_start,
                        });
                        cursor = raw_end;
                        continue;
                    }
                }
                nodes.push(TeraCstNode {
                    kind: TeraCstKind::Tag(tag),
                    start,
                    end,
                    content_start,
                    content_end,
                });
            }
            _ => unreachable!("find_next_delimiter accepts only Tera delimiters"),
        }
        cursor = end;
    }

    let _ = template_name;
    let validation_error = structural_validation_error(source, &nodes);
    let structurally_valid = validation_error.is_none();
    let semantics = Some(TeraSemanticDocument::from_cst(source, &nodes));

    TeraCstDocument {
        source: source.to_string(),
        nodes,
        structurally_valid,
        semantics,
        validation_error,
    }
}

fn structural_validation_error(source: &str, nodes: &[TeraCstNode]) -> Option<String> {
    let mut scopes = Vec::new();
    let mut component_calls = Vec::new();
    for node in nodes {
        match &node.kind {
            TeraCstKind::Opaque => return Some("Expresie Tera neînchisă.".to_string()),
            TeraCstKind::Tag(TeraTagKind::Unknown(keyword)) => {
                return Some(format!("Tag Tera necunoscut: {keyword}."));
            }
            TeraCstKind::Tag(tag) => match tag.scope_action() {
                TeraScopeAction::Open => scopes.push(tag),
                TeraScopeAction::Branch => match tag {
                    TeraTagKind::Elif if !matches!(scopes.last(), Some(TeraTagKind::If)) => {
                        return Some("Ramură Tera elif în afara unui scope if.".to_string());
                    }
                    TeraTagKind::Else
                        if !matches!(scopes.last(), Some(TeraTagKind::If | TeraTagKind::For)) =>
                    {
                        return Some("Ramură Tera else în afara unui scope if/for.".to_string());
                    }
                    _ => {}
                },
                TeraScopeAction::Close => {
                    let Some(open) = scopes.pop() else {
                        return Some("Închidere Tera fără scope părinte.".to_string());
                    };
                    if !scope_close_matches(open, tag) {
                        return Some(format!(
                            "Închiderea Tera {tag:?} nu corespunde scope-ului {open:?}."
                        ));
                    }
                }
                _ => {}
            },
            TeraCstKind::Variable => {
                let content = node.content(source).trim();
                if content.starts_with("</") {
                    if component_calls.pop().is_none() {
                        return Some("Închidere de componentă fără apel părinte.".to_string());
                    }
                } else if content.starts_with('<') && !content.ends_with("/>") {
                    component_calls.push(content);
                }
            }
            _ => {}
        }
    }
    if scopes.is_empty() && component_calls.is_empty() {
        None
    } else {
        Some("Scope Tera neînchis.".to_string())
    }
}

fn scope_close_matches(open: &TeraTagKind, close: &TeraTagKind) -> bool {
    matches!(
        (open, close),
        (TeraTagKind::Block, TeraTagKind::EndBlock)
            | (
                TeraTagKind::ComponentDefinition,
                TeraTagKind::EndComponentDefinition
            )
            | (TeraTagKind::ComponentCall, TeraTagKind::EndComponentCall)
            | (
                TeraTagKind::LegacyDefinition,
                TeraTagKind::EndLegacyDefinition
            )
            | (TeraTagKind::For, TeraTagKind::EndFor)
            | (TeraTagKind::If, TeraTagKind::EndIf)
            | (TeraTagKind::SetBlock, TeraTagKind::EndSetBlock)
            | (TeraTagKind::Filter, TeraTagKind::EndFilter)
    )
}

fn push_text(nodes: &mut Vec<TeraCstNode>, start: usize, end: usize) {
    if start >= end {
        return;
    }
    nodes.push(TeraCstNode {
        kind: TeraCstKind::Text,
        start,
        end,
        content_start: start,
        content_end: end,
    });
}

fn find_next_delimiter(bytes: &[u8], mut cursor: usize) -> Option<usize> {
    while cursor + 1 < bytes.len() {
        if bytes[cursor] == b'{' && matches!(bytes[cursor + 1], b'{' | b'%' | b'#') {
            return Some(cursor);
        }
        cursor += 1;
    }
    None
}

fn scan_token_end(
    bytes: &[u8],
    mut cursor: usize,
    close_a: u8,
    close_b: u8,
    respect_strings: bool,
) -> Option<usize> {
    let mut quote = None;
    while cursor + 1 < bytes.len() {
        let byte = bytes[cursor];
        if respect_strings && matches!(byte, b'\'' | b'"' | b'`') {
            match quote {
                Some(active) if active == byte => quote = None,
                None => quote = Some(byte),
                _ => {}
            }
            cursor += 1;
            continue;
        }
        if quote.is_none() && byte == close_a && bytes[cursor + 1] == close_b {
            return Some(cursor + 2);
        }
        cursor += 1;
    }
    None
}

fn content_bounds(bytes: &[u8], start: usize, end: usize) -> (usize, usize) {
    let mut content_start = (start + 2).min(end);
    let mut content_end = end.saturating_sub(2).max(content_start);
    if bytes.get(content_start) == Some(&b'-') {
        content_start += 1;
    }
    if content_end > content_start && bytes.get(content_end - 1) == Some(&b'-') {
        content_end -= 1;
    }
    (content_start, content_end)
}

fn find_raw_close(source: &str, mut cursor: usize) -> Option<(usize, usize)> {
    let bytes = source.as_bytes();
    while cursor + 1 < bytes.len() {
        let relative = find_next_delimiter(bytes, cursor)?;
        cursor = relative;
        if bytes[cursor + 1] != b'%' {
            cursor += 2;
            continue;
        }
        let end = scan_token_end(bytes, cursor + 2, b'%', b'}', true)?;
        let (content_start, content_end) = content_bounds(bytes, cursor, end);
        let content = source.get(content_start..content_end)?.trim();
        if classify_tag(content) == TeraTagKind::EndRaw {
            return Some((cursor, end));
        }
        cursor = end;
    }
    None
}

fn classify_tag(content: &str) -> TeraTagKind {
    if content.trim_start().starts_with("</") {
        return TeraTagKind::EndComponentCall;
    }
    if content.trim_start().starts_with('<') {
        return TeraTagKind::ComponentCall;
    }
    let keyword = content.split_whitespace().next().unwrap_or("");
    match keyword {
        "extends" => TeraTagKind::Extends,
        "include" => TeraTagKind::Include,
        "import" => TeraTagKind::LegacyImport,
        "block" => TeraTagKind::Block,
        "endblock" => TeraTagKind::EndBlock,
        "component" => TeraTagKind::ComponentDefinition,
        "endcomponent" => TeraTagKind::EndComponentDefinition,
        "macro" => TeraTagKind::LegacyDefinition,
        "endmacro" => TeraTagKind::EndLegacyDefinition,
        "for" => TeraTagKind::For,
        "endfor" => TeraTagKind::EndFor,
        "if" => TeraTagKind::If,
        "elif" => TeraTagKind::Elif,
        "else" => TeraTagKind::Else,
        "endif" => TeraTagKind::EndIf,
        "set" if content.contains('=') => TeraTagKind::Set,
        "set" => TeraTagKind::SetBlock,
        "set_global" => TeraTagKind::SetGlobal,
        "endset" => TeraTagKind::EndSetBlock,
        "filter" => TeraTagKind::Filter,
        "endfilter" => TeraTagKind::EndFilter,
        "break" => TeraTagKind::Break,
        "continue" => TeraTagKind::Continue,
        "raw" => TeraTagKind::Raw,
        "endraw" => TeraTagKind::EndRaw,
        other => TeraTagKind::Unknown(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cst_round_trips_mixed_html_and_all_tera_delimiters() {
        let source = r#"<section data-title="{{ page.title }}">
  {# păstrează comentariul #}
  {% if page.extra.visible -%}
    {{- page.title | upper -}}
  {%- endif %}
</section>
"#;
        let document = parse_tera_cst(source, "roundtrip.html");

        assert!(document.is_lossless());
        assert_eq!(document.reconstruct(), source);
        assert!(document.validation_error.is_none());
        assert!(document.structurally_valid);
    }

    #[test]
    fn token_end_inside_a_string_does_not_close_the_tag() {
        let source = r#"{{ load_data(literal="value }} still string", format="json") }}"#;
        let document = parse_tera_cst(source, "quoted-close.html");

        assert!(document.is_lossless());
        assert_eq!(document.nodes.len(), 1);
        assert_eq!(document.nodes[0].end, source.len());
    }

    #[test]
    fn raw_body_is_one_lossless_node_and_is_not_reparsed() {
        let source = "{% raw %}<p>{{ not_a_variable }}</p>{% endraw %}";
        let document = parse_tera_cst(source, "raw.html");

        assert!(document.is_lossless());
        assert_eq!(document.nodes.len(), 1);
        assert_eq!(document.nodes[0].kind, TeraCstKind::Raw);
        assert_eq!(
            document.nodes[0].content(document.source()),
            "<p>{{ not_a_variable }}</p>"
        );
    }

    #[test]
    fn scope_actions_distinguish_branches_from_nested_scopes() {
        assert_eq!(TeraTagKind::If.scope_action(), TeraScopeAction::Open);
        assert_eq!(TeraTagKind::Elif.scope_action(), TeraScopeAction::Branch);
        assert_eq!(TeraTagKind::Else.scope_action(), TeraScopeAction::Branch);
        assert_eq!(TeraTagKind::EndIf.scope_action(), TeraScopeAction::Close);
    }

    #[test]
    fn embedded_tera_rejects_non_tera_with_syntax() {
        let document = parse_tera_cst("{% with value = 1 %}{% endwith %}", "with.html");

        assert!(document.is_lossless());
        assert!(!document.structurally_valid);
        assert!(document.validation_error.is_some());
    }
}
