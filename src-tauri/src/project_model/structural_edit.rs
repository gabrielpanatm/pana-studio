use std::collections::{BTreeMap, HashSet};

use serde::Serialize;

use crate::{
    project_model::model::ProjectModel,
    source_graph::{
        mixed_cst::{parse_mixed_cst, HtmlElementCst, MixedCstDocument},
        model::{SourceNode, SourceNodeKind},
        tera_cst::TeraCstKind,
    },
};

use super::move_engine::same_model_path;

const DEFAULT_SPACE_INDENT_WIDTH: usize = 2;
const PROTECTED_HTML_TAGS: &[&str] = &["pre", "textarea", "script", "style"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StructuralIndentationStyle {
    unit: String,
    line_ending: String,
}

impl StructuralIndentationStyle {
    pub(crate) fn detect(source: &str) -> Self {
        let line_ending = detect_line_ending(source).to_string();
        let mut tab_lines = 0usize;
        let mut space_widths = Vec::new();

        for line in source.split('\n') {
            let line = line.strip_suffix('\r').unwrap_or(line);
            let indent = leading_indent(line);
            let content = &line[indent.len()..];
            if content.is_empty() || !is_structural_line(content) {
                continue;
            }
            if indent.starts_with('\t') && indent.chars().all(|character| character == '\t') {
                tab_lines += 1;
            } else if !indent.is_empty() && indent.bytes().all(|byte| byte == b' ') {
                space_widths.push(indent.len());
            }
        }

        let unit = if tab_lines > space_widths.len() {
            "\t".to_string()
        } else {
            infer_space_unit(&space_widths)
                .map(|width| " ".repeat(width))
                .unwrap_or_else(|| " ".repeat(DEFAULT_SPACE_INDENT_WIDTH))
        };
        Self { unit, line_ending }
    }

    pub(crate) fn unit(&self) -> &str {
        &self.unit
    }

    pub(crate) fn line_ending(&self) -> &str {
        &self.line_ending
    }

    pub(crate) fn child_indent(&self, parent: &str) -> String {
        format!("{parent}{}", self.unit)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StructuralPlacement {
    pub(crate) indent: String,
    pub(crate) style: StructuralIndentationStyle,
}

impl StructuralPlacement {
    pub(crate) fn for_html_target(model: &ProjectModel, source: &str, target: &SourceNode) -> Self {
        let style = StructuralIndentationStyle::detect(source);
        let indent = semantic_html_indent(model, source, target, &style);
        Self { indent, style }
    }

    pub(crate) fn for_direct_target(source: &str, offset: usize) -> Self {
        Self {
            indent: indent_at(source, offset),
            style: StructuralIndentationStyle::detect(source),
        }
    }

    pub(crate) fn child_indent(&self) -> String {
        self.style.child_indent(&self.indent)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StructuralIndentationIssue {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) line: usize,
    pub(crate) actual: String,
    pub(crate) expected: String,
    pub(crate) tag: String,
    pub(crate) closing: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StructuralIndentationAudit {
    pub(crate) issues: Vec<StructuralIndentationIssue>,
    pub(crate) repaired_contents: Option<String>,
}

#[derive(Clone, Debug)]
struct PrefixEdit {
    start: usize,
    end: usize,
    replacement: String,
    tag: String,
    closing: bool,
}

pub(crate) fn indent_at(source: &str, offset: usize) -> String {
    let prefix = source.get(..offset.min(source.len())).unwrap_or("");
    let line_start = prefix.rfind('\n').map(|index| index + 1).unwrap_or(0);
    source
        .get(line_start..offset.min(source.len()))
        .filter(|candidate| candidate.bytes().all(|byte| matches!(byte, b' ' | b'\t')))
        .unwrap_or("")
        .to_string()
}

pub(crate) fn semantic_html_indent(
    model: &ProjectModel,
    source: &str,
    node: &SourceNode,
    style: &StructuralIndentationStyle,
) -> String {
    let mut html_chain = vec![node];
    let mut current = node;
    let mut visited = HashSet::new();
    visited.insert(node.id.as_str());

    while let Some(parent_id) = current.parent.as_deref() {
        let Some(parent) = model
            .source_graph
            .node_by_id(parent_id)
            .filter(|candidate| {
                candidate.kind == SourceNodeKind::Html
                    && same_model_path(&candidate.file, &node.file)
            })
        else {
            break;
        };
        if !visited.insert(parent.id.as_str()) {
            break;
        }
        html_chain.push(parent);
        current = parent;
    }

    let root_indent = html_chain
        .last()
        .and_then(|root| root.range.as_ref())
        .map(|range| indent_at(source, range.start))
        .unwrap_or_else(|| {
            node.range
                .as_ref()
                .map(|range| indent_at(source, range.start))
                .unwrap_or_default()
        });
    format!(
        "{root_indent}{}",
        style.unit.repeat(html_chain.len().saturating_sub(1))
    )
}

/// Formats only whitespace prefixes owned by standalone HTML tags. Text and
/// attribute bytes are never trimmed, and opaque whitespace inside
/// pre/textarea/script/style is left untouched.
pub(crate) fn format_html_fragment(
    fragment: &str,
    base_indent: &str,
    style: &StructuralIndentationStyle,
) -> Result<String, String> {
    let trimmed = trim_boundary_line_endings(fragment);
    let document = parse_mixed_cst(trimmed, "pana-structural-fragment.html");
    if !document.is_lossless() {
        return Err("Mixed CST nu a putut reconstrui lossless fragmentul HTML.".to_string());
    }
    let edits = html_prefix_edits(&document, trimmed, |elements, index| {
        let depth = html_depth(elements, index);
        format!("{base_indent}{}", style.unit.repeat(depth))
    });
    apply_prefix_edits(trimmed, edits)
}

/// Relocates a heterogeneous/Tera contract without trimming line content.
/// Relative indentation is converted to the destination unit where it is
/// unambiguous; raw Tera payload stays byte-identical.
pub(crate) fn relocate_lossless_fragment(
    fragment: &str,
    source_indent: &str,
    target_indent: &str,
    style: &StructuralIndentationStyle,
) -> Result<String, String> {
    let trimmed = trim_boundary_line_endings(fragment);
    let source_style = StructuralIndentationStyle::detect(trimmed);
    let tera =
        crate::source_graph::tera_cst::parse_tera_cst(trimmed, "pana-structural-fragment.html");
    if !tera.is_lossless() {
        return Err("Tera CST nu a putut reconstrui lossless fragmentul mutat.".to_string());
    }
    let raw_ranges = tera
        .nodes
        .iter()
        .filter(|node| node.kind == TeraCstKind::Raw)
        .map(|node| (node.content_start, node.content_end))
        .collect::<Vec<_>>();
    let mut result = String::with_capacity(trimmed.len() + target_indent.len());
    let mut cursor = 0usize;

    for line in split_lines_with_offsets(trimmed) {
        let content_offset = line.start + leading_indent(line.text).len();
        if raw_ranges
            .iter()
            .any(|(start, end)| *start <= content_offset && content_offset < *end)
        {
            result.push_str(&trimmed[line.start..line.end]);
            cursor = line.end;
            continue;
        } else if line.text.trim().is_empty() {
            result.push_str(line.text);
        } else {
            let actual = leading_indent(line.text);
            let relative = actual.strip_prefix(source_indent).unwrap_or(actual);
            let relative = convert_indent(relative, source_style.unit(), style.unit());
            result.push_str(target_indent);
            result.push_str(&relative);
            result.push_str(&line.text[actual.len()..]);
        }
        cursor = line.end;
        if line.has_ending {
            result.push_str(style.line_ending());
        }
    }
    debug_assert_eq!(cursor, trimmed.len());
    Ok(result)
}

pub(crate) fn format_tera_fragment(
    fragment: &str,
    target_indent: &str,
    style: &StructuralIndentationStyle,
) -> Result<String, String> {
    let trimmed = trim_boundary_line_endings(fragment);
    let source_indent = trimmed
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .find(|line| !line.trim().is_empty())
        .map(leading_indent)
        .unwrap_or("");
    relocate_lossless_fragment(trimmed, source_indent, target_indent, style)
}

pub(crate) fn normalize_html_subtree(
    source: &str,
    opening_start: usize,
    base_indent: &str,
    style: &StructuralIndentationStyle,
) -> Result<String, String> {
    let document = parse_mixed_cst(source, "pana-structural-document.html");
    if !document.is_lossless() {
        return Err("Mixed CST nu a putut reconstrui lossless documentul HTML.".to_string());
    }
    let element_index = document
        .elements
        .iter()
        .position(|element| {
            document
                .nodes
                .get(element.opening_node)
                .is_some_and(|node| node.start == opening_start)
        })
        .ok_or_else(|| "Ținta HTML nu mai există în Mixed CST după mutație.".to_string())?;
    let element = &document.elements[element_index];
    let opening = &document.nodes[element.opening_node];
    let closing_end = element
        .closing_node
        .and_then(|index| document.nodes.get(index))
        .map(|node| node.end)
        .unwrap_or(opening.end);
    let line_start = line_start_at(source, opening_start);
    let root_prefix = source.get(line_start..opening_start).unwrap_or("");
    if !root_prefix.bytes().all(|byte| matches!(byte, b' ' | b'\t')) {
        return Ok(source.to_string());
    }
    let fragment = source
        .get(opening_start..closing_end)
        .ok_or_else(|| "Subarborele HTML are un range invalid.".to_string())?;
    let formatted = format_html_fragment(fragment, base_indent, style)?;
    Ok(format!(
        "{}{}{}",
        &source[..line_start],
        formatted,
        &source[closing_end..]
    ))
}

pub(crate) fn audit_html_indentation(
    source: &str,
    template_name: &str,
) -> Result<StructuralIndentationAudit, String> {
    let style = StructuralIndentationStyle::detect(source);
    let document = parse_mixed_cst(source, template_name);
    if !document.is_lossless() {
        return Err("Auditul structural cere un Mixed CST lossless.".to_string());
    }
    let root_indents = document
        .elements
        .iter()
        .enumerate()
        .filter(|(_, element)| element.parent.is_none())
        .map(|(index, element)| {
            let start = document.nodes[element.opening_node].start;
            (index, indent_at(source, start))
        })
        .collect::<BTreeMap<_, _>>();
    let edits = html_prefix_edits(&document, source, |elements, index| {
        let root = html_root(elements, index);
        let root_indent = root_indents.get(&root).cloned().unwrap_or_default();
        format!(
            "{root_indent}{}",
            style.unit.repeat(html_depth(elements, index))
        )
    });
    let mut issues = Vec::new();
    for edit in &edits {
        let actual = source.get(edit.start..edit.end).unwrap_or("");
        if actual != edit.replacement {
            issues.push(StructuralIndentationIssue {
                start: edit.start,
                end: edit.end,
                line: line_number(source, edit.end),
                actual: actual.to_string(),
                expected: edit.replacement.clone(),
                tag: edit.tag.clone(),
                closing: edit.closing,
            });
        }
    }
    let repaired_contents = if issues.is_empty() {
        None
    } else {
        Some(apply_prefix_edits(source, edits)?)
    };
    Ok(StructuralIndentationAudit {
        issues,
        repaired_contents,
    })
}

fn html_prefix_edits(
    document: &MixedCstDocument,
    source: &str,
    expected: impl Fn(&[HtmlElementCst], usize) -> String,
) -> Vec<PrefixEdit> {
    let mut edits = Vec::new();
    for (index, element) in document.elements.iter().enumerate() {
        if has_protected_ancestor(&document.elements, index) {
            continue;
        }
        let replacement = expected(&document.elements, index);
        if let Some(node) = document.nodes.get(element.opening_node) {
            push_prefix_edit(
                source,
                node.start,
                &replacement,
                &element.tag,
                false,
                &mut edits,
            );
        }
        if let Some(node) = element
            .closing_node
            .and_then(|closing| document.nodes.get(closing))
        {
            push_prefix_edit(
                source,
                node.start,
                &replacement,
                &element.tag,
                true,
                &mut edits,
            );
        }
    }
    edits
}

fn push_prefix_edit(
    source: &str,
    token_start: usize,
    replacement: &str,
    tag: &str,
    closing: bool,
    edits: &mut Vec<PrefixEdit>,
) {
    let start = line_start_at(source, token_start);
    let Some(prefix) = source.get(start..token_start) else {
        return;
    };
    if prefix.bytes().all(|byte| matches!(byte, b' ' | b'\t')) {
        edits.push(PrefixEdit {
            start,
            end: token_start,
            replacement: replacement.to_string(),
            tag: tag.to_string(),
            closing,
        });
    }
}

fn apply_prefix_edits(source: &str, mut edits: Vec<PrefixEdit>) -> Result<String, String> {
    edits.sort_by_key(|edit| (edit.start, edit.end));
    edits.dedup_by(|left, right| left.start == right.start && left.end == right.end);
    if edits.windows(2).any(|pair| pair[0].end > pair[1].start) {
        return Err("Patch-ul de indentare conține prefixe suprapuse.".to_string());
    }
    let mut result = source.to_string();
    for edit in edits.into_iter().rev() {
        if edit.end > result.len()
            || !result.is_char_boundary(edit.start)
            || !result.is_char_boundary(edit.end)
        {
            return Err("Patch-ul de indentare are un range invalid.".to_string());
        }
        result.replace_range(edit.start..edit.end, &edit.replacement);
    }
    Ok(result)
}

fn html_depth(elements: &[HtmlElementCst], mut index: usize) -> usize {
    let mut depth = 0usize;
    let mut visited = HashSet::new();
    while let Some(parent) = elements.get(index).and_then(|element| element.parent) {
        if !visited.insert(parent) {
            break;
        }
        depth += 1;
        index = parent;
    }
    depth
}

fn html_root(elements: &[HtmlElementCst], mut index: usize) -> usize {
    let mut visited = HashSet::new();
    while let Some(parent) = elements.get(index).and_then(|element| element.parent) {
        if !visited.insert(parent) {
            break;
        }
        index = parent;
    }
    index
}

fn has_protected_ancestor(elements: &[HtmlElementCst], mut index: usize) -> bool {
    while let Some(parent) = elements.get(index).and_then(|element| element.parent) {
        if elements
            .get(parent)
            .is_some_and(|element| PROTECTED_HTML_TAGS.contains(&element.tag.as_str()))
        {
            return true;
        }
        index = parent;
    }
    false
}

fn convert_indent(indent: &str, source_unit: &str, target_unit: &str) -> String {
    if indent.is_empty() || source_unit.is_empty() || source_unit == target_unit {
        return indent.to_string();
    }
    let mut rest = indent;
    let mut count = 0usize;
    while let Some(next) = rest.strip_prefix(source_unit) {
        count += 1;
        rest = next;
    }
    if rest.is_empty() {
        target_unit.repeat(count)
    } else {
        indent.to_string()
    }
}

fn infer_space_unit(widths: &[usize]) -> Option<usize> {
    let widths = widths
        .iter()
        .copied()
        .filter(|width| *width > 0)
        .collect::<Vec<_>>();
    if widths.is_empty() {
        return None;
    }
    if widths.iter().all(|width| width % 4 == 0) {
        return Some(4);
    }
    if widths.iter().all(|width| width % 2 == 0) {
        return Some(2);
    }
    let gcd = widths.into_iter().reduce(greatest_common_divisor)?;
    Some(gcd.clamp(1, 8))
}

fn greatest_common_divisor(mut left: usize, mut right: usize) -> usize {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn is_structural_line(content: &str) -> bool {
    content.starts_with('<')
        || content.starts_with("{%")
        || content.starts_with("{{")
        || content.starts_with("{#")
}

fn detect_line_ending(source: &str) -> &'static str {
    let crlf = source.match_indices("\r\n").count();
    let lf = source.bytes().filter(|byte| *byte == b'\n').count();
    if crlf > lf.saturating_sub(crlf) {
        "\r\n"
    } else {
        "\n"
    }
}

fn leading_indent(line: &str) -> &str {
    let length = line
        .bytes()
        .take_while(|byte| matches!(byte, b' ' | b'\t'))
        .count();
    &line[..length]
}

fn trim_boundary_line_endings(source: &str) -> &str {
    source.trim_matches(|character| matches!(character, '\n' | '\r'))
}

fn line_start_at(source: &str, offset: usize) -> usize {
    source
        .get(..offset.min(source.len()))
        .and_then(|prefix| prefix.rfind('\n').map(|index| index + 1))
        .unwrap_or(0)
}

fn line_number(source: &str, offset: usize) -> usize {
    source
        .get(..offset.min(source.len()))
        .unwrap_or("")
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

struct SourceLine<'a> {
    start: usize,
    end: usize,
    text: &'a str,
    has_ending: bool,
}

fn split_lines_with_offsets(source: &str) -> Vec<SourceLine<'_>> {
    let mut lines = Vec::new();
    let mut start = 0usize;
    while start < source.len() {
        let Some(relative_end) = source[start..].find('\n') else {
            lines.push(SourceLine {
                start,
                end: source.len(),
                text: source[start..]
                    .strip_suffix('\r')
                    .unwrap_or(&source[start..]),
                has_ending: false,
            });
            return lines;
        };
        let raw_end = start + relative_end;
        let text = source[start..raw_end]
            .strip_suffix('\r')
            .unwrap_or(&source[start..raw_end]);
        lines.push(SourceLine {
            start,
            end: raw_end + 1,
            text,
            has_ending: true,
        });
        start = raw_end + 1;
    }
    if source.is_empty() {
        lines.push(SourceLine {
            start: 0,
            end: 0,
            text: "",
            has_ending: false,
        });
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_tabs_two_spaces_four_spaces_and_crlf() {
        assert_eq!(
            StructuralIndentationStyle::detect("<main>\n  <p></p>\n</main>\n").unit(),
            "  "
        );
        assert_eq!(
            StructuralIndentationStyle::detect("<main>\n    <p></p>\n</main>\n").unit(),
            "    "
        );
        let tabs = StructuralIndentationStyle::detect("<main>\r\n\t<p></p>\r\n</main>\r\n");
        assert_eq!(tabs.unit(), "\t");
        assert_eq!(tabs.line_ending(), "\r\n");
    }

    #[test]
    fn html_formatter_repairs_only_structural_prefixes() {
        let style = StructuralIndentationStyle::detect("<main>\n  <div></div>\n</main>\n");
        let formatted = format_html_fragment(
            "<div>\n<img>\n<pre>\n    păstrează textul\n<span>literal</span>\n</pre>\n</div>",
            "  ",
            &style,
        )
        .unwrap();
        assert_eq!(
            formatted,
            concat!(
                "  <div>\n",
                "    <img>\n",
                "    <pre>\n",
                "    păstrează textul\n",
                "<span>literal</span>\n",
                "    </pre>\n",
                "  </div>",
            )
        );
    }

    #[test]
    fn tera_relocation_preserves_raw_payload_and_relative_structure() {
        let source = concat!(
            "  {% if show %}\n",
            "    <p>Text</p>\n",
            "    {% raw %}\n",
            " keep {{ exact }}\n",
            "    {% endraw %}\n",
            "  {% endif %}",
        );
        let style = StructuralIndentationStyle::detect("<main>\n    <p></p>\n</main>\n");
        let moved = relocate_lossless_fragment(source, "  ", "    ", &style).unwrap();
        assert_eq!(
            moved,
            concat!(
                "    {% if show %}\n",
                "        <p>Text</p>\n",
                "        {% raw %}\n",
                " keep {{ exact }}\n",
                "        {% endraw %}\n",
                "    {% endif %}",
            )
        );
    }

    #[test]
    fn formatter_preserves_crlf_utf8_multiline_attributes_and_protected_payloads() {
        let style =
            StructuralIndentationStyle::detect("<main>\r\n\t<article></article>\r\n</main>\r\n");
        let fragment = concat!(
            "    <article\r\n",
            " data-titlu=\"Știre\"\r\n",
            "      aria-label=\"Păstrează\">\r\n",
            "<textarea>\r\n",
            "  text  exact\r\n",
            "</textarea>\r\n",
            "<!--  comentariu exact  -->\r\n",
            "</article>\r\n",
        );
        let formatted = format_html_fragment(fragment, "\t", &style).unwrap();
        assert_eq!(
            formatted,
            concat!(
                "\t<article\r\n",
                " data-titlu=\"Știre\"\r\n",
                "      aria-label=\"Păstrează\">\r\n",
                "\t\t<textarea>\r\n",
                "  text  exact\r\n",
                "\t\t</textarea>\r\n",
                "<!--  comentariu exact  -->\r\n",
                "\t</article>",
            )
        );
    }

    #[test]
    fn raw_payload_keeps_original_line_endings_during_relocation() {
        let fragment = concat!(
            "  {% raw %}\r\n",
            " raw {{ exact }}  \r\n",
            "  {% endraw %}",
        );
        let style = StructuralIndentationStyle::detect("<main>\n  <p></p>\n</main>\n");
        let moved = relocate_lossless_fragment(fragment, "  ", "    ", &style).unwrap();
        assert_eq!(
            moved,
            concat!(
                "    {% raw %}\n",
                " raw {{ exact }}  \r\n",
                "    {% endraw %}",
            )
        );
    }

    #[test]
    fn auditor_repairs_nested_opening_and_closing_prefixes() {
        let source = concat!(
            "  <section>\n",
            "    <div>\n",
            "            <article>\n",
            "        <p>Text</p>\n",
            "            </article>\n",
            "    </div>\n",
            "  </section>\n",
        );
        let audit = audit_html_indentation(source, "index.html").unwrap();
        assert_eq!(audit.issues.len(), 2);
        let repaired = audit.repaired_contents.unwrap();
        assert_eq!(
            repaired,
            concat!(
                "  <section>\n",
                "    <div>\n",
                "      <article>\n",
                "        <p>Text</p>\n",
                "      </article>\n",
                "    </div>\n",
                "  </section>\n",
            )
        );
        let second = audit_html_indentation(&repaired, "index.html").unwrap();
        assert!(second.issues.is_empty());
        assert_eq!(second.repaired_contents, None);
    }

    #[test]
    fn auditor_repairs_nineteen_prefix_drifts_and_second_run_is_clean() {
        let mut source = String::from("<main>\n");
        for index in 0..19 {
            source.push_str(&format!("          <span>{index}</span>\n"));
        }
        source.push_str("</main>\n");

        let audit = audit_html_indentation(&source, "templates/base.html").unwrap();
        assert_eq!(audit.issues.len(), 19);
        let repaired = audit.repaired_contents.expect("repair");
        assert!(repaired
            .lines()
            .skip(1)
            .take(19)
            .all(|line| line.starts_with("  <span>")));
        let second = audit_html_indentation(&repaired, "templates/base.html").unwrap();
        assert!(second.issues.is_empty());
        assert_eq!(second.repaired_contents, None);
    }
}
