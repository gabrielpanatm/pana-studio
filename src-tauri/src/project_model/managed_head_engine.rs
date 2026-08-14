use crate::source_graph::mixed_cst::{parse_mixed_cst, MixedCstDocument, MixedCstKind};

use super::structural_edit::{indent_at, StructuralIndentationStyle};

#[derive(Clone, Debug)]
struct HeadRange {
    opening_start: usize,
    opening_end: usize,
    closing_start: usize,
}

#[derive(Clone, Debug)]
struct MarkerRange {
    start: usize,
    end: usize,
}

#[derive(Clone, Debug)]
struct OwnedLine {
    start: usize,
    end: usize,
    indent: String,
}

#[derive(Clone, Debug)]
struct ManagedBlock {
    start_marker: MarkerRange,
    end_marker: MarkerRange,
    owned_start: usize,
    owned_end: usize,
    indent: String,
}

/// Reports whether a template has one real, paired HTML `<head>` element.
/// Text that merely contains `</head>` (including comments) is not accepted.
pub(crate) fn has_unique_html_head(source: &str, template_name: &str) -> bool {
    let document = parse_mixed_cst(source, template_name);
    document.is_lossless() && unique_head(&document).is_ok()
}

/// Returns the bytes owned by a managed block, excluding its marker comments.
/// Marker discovery is CST-based so marker-like text cannot become edit authority.
pub(crate) fn managed_head_block_body<'a>(
    source: &'a str,
    template_name: &str,
    start_marker: &str,
    end_marker: &str,
) -> Result<Option<&'a str>, String> {
    let document = parse_document(source, template_name)?;
    let head = unique_head(&document)?;
    let Some(block) = managed_block(&document, source, &head, start_marker, end_marker)? else {
        return Ok(None);
    };
    Ok(source.get(block.start_marker.end..block.end_marker.start))
}

/// Reconciles one comment-delimited block inside `<head>` with a single
/// whole-line edit. Existing indentation is preserved exactly; newly created
/// blocks inherit the document's structural indentation and line ending.
pub(crate) fn reconcile_managed_head_block(
    source: &str,
    template_name: &str,
    start_marker: &str,
    end_marker: &str,
    body_lines: &[String],
) -> Result<String, String> {
    validate_contract(start_marker, end_marker, body_lines)?;
    let document = parse_document(source, template_name)?;
    let head = unique_head(&document)?;
    let existing = managed_block(&document, source, &head, start_marker, end_marker)?;

    if body_lines.is_empty() {
        let Some(block) = existing else {
            return Ok(source.to_string());
        };
        let candidate = replace_range(source, block.owned_start, block.owned_end, "")?;
        validate_candidate(&candidate, template_name, start_marker, end_marker, false)?;
        return Ok(candidate);
    }

    let style = StructuralIndentationStyle::detect(source);
    let (edit_start, edit_end, replacement) = if let Some(block) = existing {
        let rendered = render_block(
            start_marker,
            end_marker,
            body_lines,
            &block.indent,
            style.line_ending(),
        );
        (
            block.owned_start,
            block.owned_end,
            format!("{rendered}{}", style.line_ending()),
        )
    } else {
        insertion_edit(source, &head, start_marker, end_marker, body_lines, &style)?
    };

    let candidate = replace_range(source, edit_start, edit_end, &replacement)?;
    validate_candidate(&candidate, template_name, start_marker, end_marker, true)?;
    Ok(candidate)
}

fn parse_document(source: &str, template_name: &str) -> Result<MixedCstDocument, String> {
    let document = parse_mixed_cst(source, template_name);
    if !document.is_lossless() {
        return Err(format!(
            "Template-ul {template_name} nu poate fi reconstruit lossless; blocul gestionat nu a fost modificat."
        ));
    }
    Ok(document)
}

fn unique_head(document: &MixedCstDocument) -> Result<HeadRange, String> {
    let heads = document
        .elements
        .iter()
        .filter(|element| element.tag == "head")
        .collect::<Vec<_>>();
    if heads.len() != 1 {
        return Err(format!(
            "Template-ul trebuie să conțină exact un element HTML <head>; au fost găsite {}.",
            heads.len()
        ));
    }
    let head = heads[0];
    let opening = document.nodes.get(head.opening_node).ok_or_else(|| {
        "CST-ul HTML nu mai conține nodul de deschidere pentru <head>.".to_string()
    })?;
    let closing = head
        .closing_node
        .and_then(|index| document.nodes.get(index))
        .ok_or_else(|| "Elementul HTML <head> nu are un </head> asociat.".to_string())?;
    Ok(HeadRange {
        opening_start: opening.start,
        opening_end: opening.end,
        closing_start: closing.start,
    })
}

fn managed_block(
    document: &MixedCstDocument,
    source: &str,
    head: &HeadRange,
    start_marker: &str,
    end_marker: &str,
) -> Result<Option<ManagedBlock>, String> {
    let starts = marker_ranges(document, source, start_marker);
    let ends = marker_ranges(document, source, end_marker);
    match (starts.len(), ends.len()) {
        (0, 0) => return Ok(None),
        (1, 1) => {}
        (start_count, end_count) => {
            return Err(format!(
                "Blocul gestionat este ambiguu: {start_count} markere de început și {end_count} markere de sfârșit."
            ));
        }
    }

    let start_marker = starts.into_iter().next().expect("count checked");
    let end_marker = ends.into_iter().next().expect("count checked");
    if start_marker.start >= end_marker.start {
        return Err("Markerele blocului gestionat sunt în ordine invalidă.".to_string());
    }
    if start_marker.start < head.opening_end || end_marker.end > head.closing_start {
        return Err("Blocul gestionat trebuie să fie complet în interiorul <head>.".to_string());
    }

    let start_line = owned_marker_line(source, &start_marker)?;
    let end_line = owned_marker_line(source, &end_marker)?;
    if start_line.end > end_line.start {
        return Err("Markerele blocului gestionat trebuie să ocupe linii distincte.".to_string());
    }
    Ok(Some(ManagedBlock {
        start_marker,
        end_marker,
        owned_start: start_line.start,
        owned_end: end_line.end,
        indent: start_line.indent,
    }))
}

fn marker_ranges(document: &MixedCstDocument, source: &str, marker: &str) -> Vec<MarkerRange> {
    document
        .nodes
        .iter()
        .filter(|node| matches!(node.kind, MixedCstKind::Comment { .. }))
        .filter(|node| node.full_text(source) == marker)
        .map(|node| MarkerRange {
            start: node.start,
            end: node.end,
        })
        .collect()
}

fn owned_marker_line(source: &str, marker: &MarkerRange) -> Result<OwnedLine, String> {
    let line_start = source
        .get(..marker.start)
        .and_then(|prefix| prefix.rfind('\n'))
        .map(|index| index + 1)
        .unwrap_or(0);
    let raw_line_end = source
        .get(marker.end..)
        .and_then(|suffix| suffix.find('\n'))
        .map(|relative| marker.end + relative)
        .unwrap_or(source.len());
    let content_end = raw_line_end
        .checked_sub(1)
        .filter(|index| source.as_bytes().get(*index) == Some(&b'\r'))
        .unwrap_or(raw_line_end);
    let before = source.get(line_start..marker.start).unwrap_or("");
    let after = source.get(marker.end..content_end).unwrap_or("");
    if !is_horizontal_whitespace(before) || !is_horizontal_whitespace(after) {
        return Err(
            "Fiecare marker al blocului gestionat trebuie să fie singur pe linia sa.".to_string(),
        );
    }
    Ok(OwnedLine {
        start: line_start,
        end: if raw_line_end < source.len() {
            raw_line_end + 1
        } else {
            raw_line_end
        },
        indent: before.to_string(),
    })
}

fn insertion_edit(
    source: &str,
    head: &HeadRange,
    start_marker: &str,
    end_marker: &str,
    body_lines: &[String],
    style: &StructuralIndentationStyle,
) -> Result<(usize, usize, String), String> {
    let head_indent = indent_at(source, head.opening_start);
    let block_indent = style.child_indent(&head_indent);
    let block = render_block(
        start_marker,
        end_marker,
        body_lines,
        &block_indent,
        style.line_ending(),
    );
    let closing_line_start = source
        .get(..head.closing_start)
        .and_then(|prefix| prefix.rfind('\n'))
        .map(|index| index + 1)
        .unwrap_or(0);
    let closing_prefix = source
        .get(closing_line_start..head.closing_start)
        .ok_or_else(|| "Poziția </head> nu este o limită UTF-8 validă.".to_string())?;

    if is_horizontal_whitespace(closing_prefix) {
        Ok((
            closing_line_start,
            closing_line_start,
            format!("{block}{}", style.line_ending()),
        ))
    } else {
        Ok((
            head.closing_start,
            head.closing_start,
            format!(
                "{}{block}{}{head_indent}",
                style.line_ending(),
                style.line_ending()
            ),
        ))
    }
}

fn render_block(
    start_marker: &str,
    end_marker: &str,
    body_lines: &[String],
    indent: &str,
    line_ending: &str,
) -> String {
    std::iter::once(start_marker)
        .chain(body_lines.iter().map(String::as_str))
        .chain(std::iter::once(end_marker))
        .map(|line| format!("{indent}{line}"))
        .collect::<Vec<_>>()
        .join(line_ending)
}

fn validate_contract(
    start_marker: &str,
    end_marker: &str,
    body_lines: &[String],
) -> Result<(), String> {
    if start_marker == end_marker
        || !start_marker.starts_with("<!--")
        || !start_marker.ends_with("-->")
        || !end_marker.starts_with("<!--")
        || !end_marker.ends_with("-->")
    {
        return Err(
            "Contractul blocului gestionat cere două comentarii-marker distincte.".to_string(),
        );
    }
    if body_lines.iter().any(|line| line.contains(['\r', '\n'])) {
        return Err("O linie din blocul gestionat conține un separator de linie.".to_string());
    }
    Ok(())
}

fn validate_candidate(
    candidate: &str,
    template_name: &str,
    start_marker: &str,
    end_marker: &str,
    expected: bool,
) -> Result<(), String> {
    let document = parse_document(candidate, template_name)?;
    let head = unique_head(&document)?;
    let found = managed_block(&document, candidate, &head, start_marker, end_marker)?.is_some();
    if found != expected {
        return Err("Validarea blocului gestionat după editare a eșuat.".to_string());
    }
    Ok(())
}

fn replace_range(
    source: &str,
    start: usize,
    end: usize,
    replacement: &str,
) -> Result<String, String> {
    if start > end
        || end > source.len()
        || !source.is_char_boundary(start)
        || !source.is_char_boundary(end)
    {
        return Err("Intervalul blocului gestionat nu este o limită UTF-8 validă.".to_string());
    }
    let mut candidate = String::with_capacity(source.len() - (end - start) + replacement.len());
    candidate.push_str(&source[..start]);
    candidate.push_str(replacement);
    candidate.push_str(&source[end..]);
    Ok(candidate)
}

fn is_horizontal_whitespace(value: &str) -> bool {
    value.bytes().all(|byte| matches!(byte, b' ' | b'\t'))
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: &str = "<!-- managed:start -->";
    const END: &str = "<!-- managed:end -->";

    fn lines(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn inserts_before_real_head_end_with_detected_indentation() {
        let source = "<html>\n  <head>\n    <title>Exemplu</title>\n  </head>\n</html>\n";
        let updated = reconcile_managed_head_block(
            source,
            "templates/base.html",
            START,
            END,
            &lines(&["<link href=\"/font.woff2\">"]),
        )
        .expect("insert block");
        assert_eq!(
            updated,
            "<html>\n  <head>\n    <title>Exemplu</title>\n    <!-- managed:start -->\n    <link href=\"/font.woff2\">\n    <!-- managed:end -->\n  </head>\n</html>\n"
        );
    }

    #[test]
    fn repeated_reconciliation_does_not_accumulate_indentation() {
        let source = "<html>\n  <head>\n        <!-- managed:start -->\n  <link href=\"/old.woff2\">\n    <!-- managed:end -->\n  </head>\n</html>\n";
        let first = reconcile_managed_head_block(
            source,
            "templates/base.html",
            START,
            END,
            &lines(&["<link href=\"/new.woff2\">"]),
        )
        .expect("first reconcile");
        let second = reconcile_managed_head_block(
            &first,
            "templates/base.html",
            START,
            END,
            &lines(&["<link href=\"/new.woff2\">"]),
        )
        .expect("second reconcile");
        assert_eq!(first, second);
        assert!(first.contains("\n        <!-- managed:start -->\n"));
        assert!(!first.contains("\n                <!-- managed:start -->\n"));
    }

    #[test]
    fn removes_the_whole_owned_lines_without_orphan_indentation() {
        let source = "<html>\n  <head>\n    <title>x</title>\n    <!-- managed:start -->\n    <link href=\"/font.woff2\">\n    <!-- managed:end -->\n  </head>\n</html>\n";
        let updated = reconcile_managed_head_block(source, "templates/base.html", START, END, &[])
            .expect("remove block");
        assert_eq!(
            updated,
            "<html>\n  <head>\n    <title>x</title>\n  </head>\n</html>\n"
        );
    }

    #[test]
    fn preserves_crlf_and_tab_indentation_for_new_blocks() {
        let source = "<html>\r\n\t<head>\r\n\t</head>\r\n</html>\r\n";
        let updated = reconcile_managed_head_block(
            source,
            "templates/base.html",
            START,
            END,
            &lines(&["<link href=\"/font.woff2\">"]),
        )
        .expect("insert block");
        assert!(updated.contains(
            "\r\n\t\t<!-- managed:start -->\r\n\t\t<link href=\"/font.woff2\">\r\n\t\t<!-- managed:end -->\r\n\t</head>"
        ));
        assert!(!updated.replace("\r\n", "").contains('\n'));
    }

    #[test]
    fn ignores_head_end_text_inside_comments() {
        let source = "<!-- </head> -->\n<div>fără head real</div>\n";
        assert!(!has_unique_html_head(source, "templates/index.html"));
    }

    #[test]
    fn rejects_partial_or_inline_marker_contracts() {
        let partial = "<html><head>\n  <!-- managed:start -->\n</head></html>";
        assert!(reconcile_managed_head_block(
            partial,
            "templates/base.html",
            START,
            END,
            &lines(&["<link>"])
        )
        .is_err());

        let inline =
            "<html><head>prefix <!-- managed:start -->\n<!-- managed:end -->\n</head></html>";
        assert!(reconcile_managed_head_block(
            inline,
            "templates/base.html",
            START,
            END,
            &lines(&["<link>"])
        )
        .is_err());
    }
}
