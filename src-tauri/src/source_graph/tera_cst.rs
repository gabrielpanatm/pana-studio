use tera::Template;

use crate::source_graph::tera_semantics::TeraSemanticDocument;

#[derive(Clone, Debug)]
pub(crate) struct TeraCstDocument {
    source: String,
    pub(crate) nodes: Vec<TeraCstNode>,
    pub(crate) parsed_template: Option<Template>,
    pub(crate) semantics: Option<TeraSemanticDocument>,
    pub(crate) validation_error: Option<String>,
}

impl TeraCstDocument {
    pub(crate) fn source(&self) -> &str {
        &self.source
    }

    pub(crate) fn is_valid_tera(&self) -> bool {
        self.parsed_template.is_some() && self.validation_error.is_none()
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
    Import,
    Block,
    EndBlock,
    Macro,
    EndMacro,
    For,
    EndFor,
    If,
    Elif,
    Else,
    EndIf,
    Set,
    SetGlobal,
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
            Self::Block | Self::Macro | Self::For | Self::If | Self::Filter => {
                TeraScopeAction::Open
            }
            Self::Elif | Self::Else => TeraScopeAction::Branch,
            Self::EndBlock | Self::EndMacro | Self::EndFor | Self::EndIf | Self::EndFilter => {
                TeraScopeAction::Close
            }
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

    let parsed = Template::new(template_name, None, source);
    let (parsed_template, semantics, validation_error) = match parsed {
        Ok(template) => {
            let semantics = TeraSemanticDocument::from_template(&template);
            (Some(template), Some(semantics), None)
        }
        Err(error) => (None, None, Some(error.to_string())),
    };

    TeraCstDocument {
        source: source.to_string(),
        nodes,
        parsed_template,
        semantics,
        validation_error,
    }
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
    let keyword = content.split_whitespace().next().unwrap_or("");
    match keyword {
        "extends" => TeraTagKind::Extends,
        "include" => TeraTagKind::Include,
        "import" => TeraTagKind::Import,
        "block" => TeraTagKind::Block,
        "endblock" => TeraTagKind::EndBlock,
        "macro" => TeraTagKind::Macro,
        "endmacro" => TeraTagKind::EndMacro,
        "for" => TeraTagKind::For,
        "endfor" => TeraTagKind::EndFor,
        "if" => TeraTagKind::If,
        "elif" => TeraTagKind::Elif,
        "else" => TeraTagKind::Else,
        "endif" => TeraTagKind::EndIf,
        "set" => TeraTagKind::Set,
        "set_global" => TeraTagKind::SetGlobal,
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
        assert!(document.parsed_template.is_some());
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
        assert!(document.parsed_template.is_none());
        assert!(document.validation_error.is_some());
    }
}
