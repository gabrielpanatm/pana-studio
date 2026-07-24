use std::collections::HashMap;

use crate::source_graph::tera_cst::{parse_tera_cst, TeraCstDocument, TeraCstKind, TeraCstNode};

#[derive(Clone, Debug)]
pub(crate) struct MixedCstDocument {
    source: String,
    pub(crate) tera: TeraCstDocument,
    pub(crate) nodes: Vec<MixedCstNode>,
    pub(crate) elements: Vec<HtmlElementCst>,
}

impl MixedCstDocument {
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
pub(crate) struct MixedCstNode {
    pub(crate) kind: MixedCstKind,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl MixedCstNode {
    pub(crate) fn full_text<'a>(&self, source: &'a str) -> &'a str {
        source.get(self.start..self.end).unwrap_or("")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MixedCstKind {
    Text,
    StartTag(HtmlStartTagCst),
    EndTag(HtmlEndTagCst),
    Comment { embedded_tera: Vec<usize> },
    Doctype,
    Tera { tera_node: usize },
    Opaque,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HtmlStartTagCst {
    pub(crate) name: String,
    pub(crate) self_closing: bool,
    pub(crate) attributes: Vec<HtmlAttributeCst>,
    pub(crate) embedded_tera: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HtmlEndTagCst {
    pub(crate) name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HtmlAttributeCst {
    pub(crate) name: String,
    pub(crate) name_start: usize,
    pub(crate) name_end: usize,
    pub(crate) value_start: Option<usize>,
    pub(crate) value_end: Option<usize>,
    pub(crate) quote: Option<char>,
    pub(crate) embedded_tera: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HtmlElementCst {
    pub(crate) tag: String,
    pub(crate) opening_node: usize,
    pub(crate) closing_node: Option<usize>,
    pub(crate) parent: Option<usize>,
    pub(crate) children: Vec<usize>,
}

pub(crate) fn parse_mixed_cst(source: &str, template_name: &str) -> MixedCstDocument {
    let tera = parse_tera_cst(source, template_name);
    let islands = tera
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| !matches!(node.kind, TeraCstKind::Text))
        .map(|(index, node)| TeraIsland { index, node })
        .collect::<Vec<_>>();
    let island_by_start = islands
        .iter()
        .map(|island| (island.node.start, *island))
        .collect::<HashMap<_, _>>();
    let mut nodes = Vec::new();
    let mut cursor = 0usize;

    while cursor < source.len() {
        if let Some(island) = island_by_start.get(&cursor) {
            nodes.push(MixedCstNode {
                kind: MixedCstKind::Tera {
                    tera_node: island.index,
                },
                start: island.node.start,
                end: island.node.end,
            });
            cursor = island.node.end;
            continue;
        }

        if source.as_bytes().get(cursor) == Some(&b'<') {
            if let Some(node) = scan_html_node(source, cursor, &islands) {
                cursor = node.end;
                nodes.push(node);
                continue;
            }
            let end = next_root_boundary(source, cursor, &islands);
            nodes.push(MixedCstNode {
                kind: MixedCstKind::Opaque,
                start: cursor,
                end,
            });
            cursor = end;
            continue;
        }

        let end = next_root_boundary(source, cursor, &islands);
        nodes.push(MixedCstNode {
            kind: MixedCstKind::Text,
            start: cursor,
            end,
        });
        cursor = end;
    }

    let elements = build_element_tree(&nodes);
    MixedCstDocument {
        source: source.to_string(),
        tera,
        nodes,
        elements,
    }
}

#[derive(Clone, Copy)]
struct TeraIsland<'a> {
    index: usize,
    node: &'a TeraCstNode,
}

fn next_root_boundary(source: &str, cursor: usize, islands: &[TeraIsland<'_>]) -> usize {
    let next_html = source
        .get(cursor + 1..)
        .and_then(|rest| rest.find('<'))
        .map(|relative| cursor + 1 + relative);
    let next_tera = islands
        .iter()
        .find(|island| island.node.start > cursor)
        .map(|island| island.node.start);
    match (next_html, next_tera) {
        (Some(html), Some(tera)) => html.min(tera),
        (Some(html), None) => html,
        (None, Some(tera)) => tera,
        (None, None) => source.len(),
    }
}

fn scan_html_node(source: &str, start: usize, islands: &[TeraIsland<'_>]) -> Option<MixedCstNode> {
    let rest = source.get(start..)?;
    if rest.starts_with("<!--") {
        let end = rest
            .find("-->")
            .map(|relative| start + relative + 3)
            .unwrap_or(source.len());
        return Some(MixedCstNode {
            kind: MixedCstKind::Comment {
                embedded_tera: islands_in_range(islands, start, end),
            },
            start,
            end,
        });
    }
    if rest
        .get(..9)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("<!doctype"))
    {
        let end = scan_angle_end(source, start + 2, islands).unwrap_or(source.len());
        return Some(MixedCstNode {
            kind: MixedCstKind::Doctype,
            start,
            end,
        });
    }
    if rest.starts_with("</") {
        let end = scan_angle_end(source, start + 2, islands)?;
        let name = html_name(source, start + 2, end.saturating_sub(1))?;
        return Some(MixedCstNode {
            kind: MixedCstKind::EndTag(HtmlEndTagCst { name }),
            start,
            end,
        });
    }
    let name = html_name(source, start + 1, source.len())?;
    if name.is_empty() {
        return None;
    }
    let end = scan_angle_end(source, start + 1 + name.len(), islands)?;
    let embedded_tera = islands_in_range(islands, start, end);
    let self_closing = source
        .get(start..end.saturating_sub(1))
        .is_some_and(|value| value.trim_end().ends_with('/'));
    let attributes = parse_attributes(
        source,
        start + 1 + name.len(),
        end.saturating_sub(1),
        islands,
    );
    Some(MixedCstNode {
        kind: MixedCstKind::StartTag(HtmlStartTagCst {
            name,
            self_closing,
            attributes,
            embedded_tera,
        }),
        start,
        end,
    })
}

fn scan_angle_end(source: &str, mut cursor: usize, islands: &[TeraIsland<'_>]) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut quote = None;
    while cursor < bytes.len() {
        if let Some(island) = island_at(islands, cursor) {
            cursor = island.node.end;
            continue;
        }
        let byte = bytes[cursor];
        if matches!(byte, b'\'' | b'"') {
            match quote {
                Some(active) if active == byte => quote = None,
                None => quote = Some(byte),
                _ => {}
            }
            cursor += 1;
            continue;
        }
        if quote.is_none() && byte == b'>' {
            return Some(cursor + 1);
        }
        cursor += 1;
    }
    None
}

fn html_name(source: &str, mut cursor: usize, end: usize) -> Option<String> {
    let bytes = source.as_bytes();
    while cursor < end && bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
        cursor += 1;
    }
    let start = cursor;
    while cursor < end
        && bytes.get(cursor).is_some_and(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.')
        })
    {
        cursor += 1;
    }
    (cursor > start).then(|| source[start..cursor].to_ascii_lowercase())
}

fn parse_attributes(
    source: &str,
    mut cursor: usize,
    mut end: usize,
    islands: &[TeraIsland<'_>],
) -> Vec<HtmlAttributeCst> {
    let bytes = source.as_bytes();
    let mut attributes = Vec::new();
    while end > cursor && bytes.get(end - 1) == Some(&b'/') {
        end -= 1;
    }

    while cursor < end {
        while cursor < end && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= end {
            break;
        }
        if let Some(island) = island_at(islands, cursor) {
            cursor = island.node.end;
            continue;
        }

        let name_start = cursor;
        while cursor < end
            && !bytes[cursor].is_ascii_whitespace()
            && !matches!(bytes[cursor], b'=' | b'>' | b'/')
            && island_at(islands, cursor).is_none()
        {
            cursor += 1;
        }
        if cursor == name_start {
            cursor += 1;
            continue;
        }
        let name_end = cursor;
        while cursor < end && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }

        let mut value_start = None;
        let mut value_end = None;
        let mut quote = None;
        if cursor < end && bytes[cursor] == b'=' {
            cursor += 1;
            while cursor < end && bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            if cursor < end && matches!(bytes[cursor], b'\'' | b'"') {
                let quote_byte = bytes[cursor];
                quote = Some(quote_byte as char);
                cursor += 1;
                let start = cursor;
                while cursor < end {
                    if let Some(island) = island_at(islands, cursor) {
                        cursor = island.node.end;
                        continue;
                    }
                    if bytes[cursor] == quote_byte {
                        break;
                    }
                    cursor += 1;
                }
                value_start = Some(start);
                value_end = Some(cursor);
                if cursor < end {
                    cursor += 1;
                }
            } else {
                let start = cursor;
                while cursor < end && !bytes[cursor].is_ascii_whitespace() {
                    if let Some(island) = island_at(islands, cursor) {
                        cursor = island.node.end;
                    } else {
                        cursor += 1;
                    }
                }
                value_start = Some(start);
                value_end = Some(cursor);
            }
        }
        let attribute_end = value_end.unwrap_or(name_end);
        attributes.push(HtmlAttributeCst {
            name: source[name_start..name_end].to_string(),
            name_start,
            name_end,
            value_start,
            value_end,
            quote,
            embedded_tera: islands_in_range(islands, name_start, attribute_end),
        });
    }
    attributes
}

fn island_at<'a>(islands: &'a [TeraIsland<'a>], cursor: usize) -> Option<TeraIsland<'a>> {
    islands
        .iter()
        .find(|island| island.node.start == cursor)
        .copied()
}

fn islands_in_range(islands: &[TeraIsland<'_>], start: usize, end: usize) -> Vec<usize> {
    islands
        .iter()
        .filter(|island| start <= island.node.start && island.node.end <= end)
        .map(|island| island.index)
        .collect()
}

fn build_element_tree(nodes: &[MixedCstNode]) -> Vec<HtmlElementCst> {
    let mut elements = Vec::<HtmlElementCst>::new();
    let mut stack = Vec::<usize>::new();

    for (node_index, node) in nodes.iter().enumerate() {
        match &node.kind {
            MixedCstKind::StartTag(tag) => {
                let parent = stack.last().copied();
                let element_index = elements.len();
                elements.push(HtmlElementCst {
                    tag: tag.name.clone(),
                    opening_node: node_index,
                    closing_node: None,
                    parent,
                    children: Vec::new(),
                });
                if let Some(parent) = parent {
                    elements[parent].children.push(element_index);
                }
                if !tag.self_closing && !is_void_html_tag(&tag.name) {
                    stack.push(element_index);
                }
            }
            MixedCstKind::EndTag(tag) => {
                if let Some(position) = stack
                    .iter()
                    .rposition(|element| elements[*element].tag == tag.name)
                {
                    let element = stack[position];
                    elements[element].closing_node = Some(node_index);
                    stack.truncate(position);
                }
            }
            _ => {}
        }
    }
    elements
}

fn is_void_html_tag(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_cst_round_trips_multiline_html_with_tera_attributes() {
        let source = r#"<article
  class="card {{ card.class }}"
  data-id="{{ card.id }}"
>
  <h2>{{ card.title }}</h2>
</article>
"#;
        let document = parse_mixed_cst(source, "card.html");

        assert!(document.is_lossless());
        assert_eq!(document.reconstruct(), source);
        let article = document
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                MixedCstKind::StartTag(tag) if tag.name == "article" => Some(tag),
                _ => None,
            })
            .expect("article start tag");
        assert_eq!(article.attributes.len(), 2);
        assert_eq!(article.embedded_tera.len(), 2);
        assert_eq!(document.elements.len(), 2);
        assert_eq!(document.elements[1].parent, Some(0));
    }

    #[test]
    fn tera_blocks_partition_the_root_but_html_inside_real_tags_stays_structural() {
        let source = "{% if visible %}<div><img src=\"{{ image }}\"></div>{% endif %}";
        let document = parse_mixed_cst(source, "conditional.html");

        assert!(document.is_lossless());
        assert_eq!(document.elements.len(), 2);
        assert!(document
            .nodes
            .iter()
            .any(|node| matches!(node.kind, MixedCstKind::Tera { .. })));
    }
}
