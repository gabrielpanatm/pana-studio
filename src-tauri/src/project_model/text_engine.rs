use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    project_model::model::{ProjectModel, ProjectModelFile, ProjectModelFileKind},
    source_graph::model::SourceNode,
};

use super::move_engine::{
    content_revision, direct_location_without_source_id, html_tag_at, offset_for_source_location,
    parse_html_tag_at, resolve_html_element_span, resolve_html_node_for_anchor, same_model_path,
    source_location_at_offset, source_missing_message, HtmlTag, ProjectSourceEditLocation, Span,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlTextIntent {
    pub target_source_id: Option<String>,
    pub target_location: Option<ProjectSourceEditLocation>,
    pub target_tag: Option<String>,
    pub target_selector: Option<String>,
    pub text: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlTextPlan {
    pub allowed: bool,
    pub diagnostic: Option<String>,
    pub model_revision: String,
    pub patch: Option<ProjectHtmlTextPatch>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlTextPatch {
    pub file: String,
    pub resolved_target_id: String,
    pub before_revision: String,
    pub after_revision: String,
    pub contents: String,
    pub target_location: ProjectSourceEditLocation,
    pub source_start_line: usize,
    pub line_shift_start: usize,
    pub line_shift: isize,
    pub tag: String,
    pub text: String,
}

struct TextApplication {
    contents: String,
    target_location: ProjectSourceEditLocation,
    source_start_line: usize,
    line_shift_start: usize,
    line_shift: isize,
}

pub fn plan_html_text(
    model: &ProjectModel,
    intent: &ProjectHtmlTextIntent,
    aliases: &HashMap<String, String>,
) -> ProjectHtmlTextPlan {
    match plan_html_text_inner(model, intent, aliases) {
        Ok(patch) => ProjectHtmlTextPlan {
            allowed: true,
            diagnostic: None,
            model_revision: model.revision.clone(),
            patch: Some(patch),
        },
        Err(message) => ProjectHtmlTextPlan {
            allowed: false,
            diagnostic: Some(message),
            model_revision: model.revision.clone(),
            patch: None,
        },
    }
}

fn plan_html_text_inner(
    model: &ProjectModel,
    intent: &ProjectHtmlTextIntent,
    aliases: &HashMap<String, String>,
) -> Result<ProjectHtmlTextPatch, String> {
    let text = normalize_text_value(&intent.text)?;

    if let Some(target_node) = resolve_html_node_for_anchor(
        model,
        intent.target_source_id.as_deref(),
        intent.target_location.as_ref(),
        intent.target_tag.as_deref(),
        aliases,
    ) {
        return plan_html_text_from_source_node(model, intent, target_node, text);
    }

    if let Some(location) = direct_location_without_source_id(
        intent.target_source_id.as_deref(),
        intent.target_location.as_ref(),
    ) {
        return plan_html_text_from_direct_location(model, intent, location, text);
    }

    Err(source_missing_message(
        "țintă de text",
        intent.target_source_id.as_deref(),
        intent.target_location.as_ref(),
        intent.target_selector.as_deref(),
    ))
}

fn plan_html_text_from_source_node(
    model: &ProjectModel,
    intent: &ProjectHtmlTextIntent,
    target_node: &SourceNode,
    text: String,
) -> Result<ProjectHtmlTextPatch, String> {
    if !target_node.capabilities.can_edit_visual {
        return Err(target_node
            .capabilities
            .reason
            .clone()
            .unwrap_or_else(|| "Ținta nu este editabilă vizual.".to_string()));
    }

    let file = model
        .files
        .iter()
        .find(|file| same_model_path(&file.relative_path, &target_node.file))
        .ok_or_else(|| {
            format!(
                "Nu am găsit fișierul {} în Project Model.",
                target_node.file
            )
        })?;
    if file.kind != ProjectModelFileKind::Template {
        return Err("HTML Text Engine este activ doar pentru template-uri Zola/Tera.".to_string());
    }

    let target_range = target_node
        .range
        .as_ref()
        .ok_or_else(|| "Ținta nu are range stabil în Source Graph.".to_string())?;
    let target_tag = html_tag_at(&file.contents, target_range.start)?;
    validate_target_tag(intent, &target_tag)?;
    let applied = apply_html_text(&file.contents, &target_node.file, target_range.start, &text)?;

    Ok(ProjectHtmlTextPatch {
        file: target_node.file.clone(),
        resolved_target_id: target_node.id.clone(),
        before_revision: file.revision.clone(),
        after_revision: content_revision(&applied.contents),
        contents: applied.contents,
        target_location: applied.target_location,
        source_start_line: applied.source_start_line,
        line_shift_start: applied.line_shift_start,
        line_shift: applied.line_shift,
        tag: target_tag,
        text,
    })
}

fn plan_html_text_from_direct_location(
    model: &ProjectModel,
    intent: &ProjectHtmlTextIntent,
    location: &ProjectSourceEditLocation,
    text: String,
) -> Result<ProjectHtmlTextPatch, String> {
    let file = model
        .files
        .iter()
        .find(|file| same_model_path(&file.relative_path, &location.file))
        .ok_or_else(|| format!("Nu am găsit fișierul {} în Project Model.", location.file))?;

    if !is_direct_html_text_file(file) {
        return Err(
            "Textul prin locație directă este activ doar pentru fișiere HTML 1:1 din proiect, nu pentru template-uri Tera.".to_string(),
        );
    }

    let offset = offset_for_source_location(&file.contents, location)?;
    let tag = parse_html_tag_at(&file.contents, offset)
        .ok_or_else(|| "Locația nu indică începutul unui tag HTML pentru text.".to_string())?;
    if tag.is_closing {
        return Err("Locația indică un tag de închidere, nu un element mutabil.".to_string());
    }
    validate_target_tag(intent, &tag.tag)?;
    let applied = apply_html_text(&file.contents, &file.relative_path, tag.start, &text)?;
    let resolved_target_id = intent.target_source_id.clone().unwrap_or_else(|| {
        format!(
            "location:{}:{}:{}",
            location.file, location.line, location.column
        )
    });

    Ok(ProjectHtmlTextPatch {
        file: file.relative_path.clone(),
        resolved_target_id,
        before_revision: file.revision.clone(),
        after_revision: content_revision(&applied.contents),
        contents: applied.contents,
        target_location: applied.target_location,
        source_start_line: applied.source_start_line,
        line_shift_start: applied.line_shift_start,
        line_shift: applied.line_shift,
        tag: tag.tag,
        text,
    })
}

fn validate_target_tag(intent: &ProjectHtmlTextIntent, actual_tag: &str) -> Result<(), String> {
    if let Some(expected_tag) = intent.target_tag.as_deref() {
        let expected_tag = expected_tag.trim().to_ascii_lowercase();
        if !expected_tag.is_empty() && expected_tag != actual_tag {
            return Err(format!(
                "Locația indică <{}>, dar intenția preview a cerut <{}>.",
                actual_tag, expected_tag
            ));
        }
    }
    if actual_tag.eq_ignore_ascii_case("html") {
        return Err("Elementul <html> nu este editabil vizual pentru text.".to_string());
    }
    if is_raw_or_programmatic_text_tag(actual_tag) {
        return Err(format!(
            "Elementul <{}> cere editor dedicat, nu editare text vizuală.",
            actual_tag
        ));
    }
    Ok(())
}

fn apply_html_text(
    source: &str,
    file: &str,
    opening_start: usize,
    text: &str,
) -> Result<TextApplication, String> {
    let opening = parse_html_tag_at(source, opening_start)
        .ok_or_else(|| "Range-ul nu mai indică un tag HTML stabil.".to_string())?;
    if opening.is_closing {
        return Err("Range-ul indică un tag HTML de închidere, nu un element mutabil.".to_string());
    }
    if opening.is_self_closing || is_void_text_tag(&opening.tag) {
        return Err(format!(
            "Elementul <{}> nu poate primi text pentru că este void sau self-closing.",
            opening.tag
        ));
    }

    let span = resolve_html_element_span(source, opening.start)?;
    let closing_start = matching_closing_tag_start(source, &opening, span)?;
    let content_start = opening.end;
    let content_end = closing_start;
    let current_text = source
        .get(content_start..content_end)
        .ok_or_else(|| "Nu am putut citi textul curent al elementului.".to_string())?;

    if contains_tera_token(current_text) {
        return Err(
            "Textul curent conține token-uri Tera; editarea vizuală simplă este blocată."
                .to_string(),
        );
    }
    if contains_html_child_tag(current_text) {
        return Err(
            "Elementul conține copii HTML; Text Engine editează doar text leaf simplu.".to_string(),
        );
    }

    let escaped_text = escape_text_content(text);
    let contents = replace_range(source, content_start, content_end, &escaped_text);
    let target_location = source_location_at_offset(source, file, opening.start);
    let text_location = source_location_at_offset(source, file, content_start);
    let line_shift =
        line_break_count(&escaped_text) as isize - line_break_count(current_text) as isize;

    Ok(TextApplication {
        contents,
        source_start_line: target_location.line,
        line_shift_start: text_location.line,
        line_shift,
        target_location,
    })
}

fn normalize_text_value(text: &str) -> Result<String, String> {
    if text.chars().any(|character| character == '\0') {
        return Err("Textul HTML nu poate conține caractere nule.".to_string());
    }
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    if contains_tera_token(&normalized) {
        return Err(
            "Textul conține delimitatori Tera; folosește editorul de cod pentru acest caz."
                .to_string(),
        );
    }
    Ok(normalized)
}

fn matching_closing_tag_start(
    source: &str,
    opening: &HtmlTag,
    span: Span,
) -> Result<usize, String> {
    let mut depth = 1usize;
    let mut cursor = opening.end;
    while let Some(tag) = next_html_tag_until(source, cursor, span.end) {
        cursor = tag.end;
        if tag.tag != opening.tag {
            continue;
        }
        if tag.is_closing {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Ok(tag.start);
            }
        } else if !tag.is_self_closing && !is_void_text_tag(&tag.tag) {
            depth += 1;
        }
    }

    Err(format!(
        "Nu am găsit tag-ul de închidere pentru <{}>.",
        opening.tag
    ))
}

fn next_html_tag_until(source: &str, start: usize, end: usize) -> Option<HtmlTag> {
    let bytes = source.as_bytes();
    let mut cursor = start;
    while cursor < end && cursor < bytes.len() {
        if is_tera_start(bytes, cursor) {
            cursor = skip_tera_token(bytes, cursor).unwrap_or(cursor + 2);
            continue;
        }
        if bytes[cursor] != b'<' {
            cursor += 1;
            continue;
        }
        let after_lt = cursor + 1;
        let next = *bytes.get(after_lt)?;
        if next == b'!' || next == b'?' {
            cursor += 1;
            continue;
        }
        if next != b'/' && !next.is_ascii_alphabetic() {
            cursor += 1;
            continue;
        }
        if let Some(tag) = parse_html_tag_at(source, cursor) {
            if tag.end <= end {
                return Some(tag);
            }
            return None;
        }
        cursor += 1;
    }
    None
}

fn contains_html_child_tag(source: &str) -> bool {
    let bytes = source.as_bytes();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        if bytes[cursor] != b'<' {
            cursor += 1;
            continue;
        }
        let Some(next) = bytes.get(cursor + 1).copied() else {
            return false;
        };
        if next == b'!' || next == b'?' {
            return true;
        }
        if (next == b'/' || next.is_ascii_alphabetic())
            && parse_html_tag_at(source, cursor).is_some()
        {
            return true;
        }
        cursor += 1;
    }
    false
}

fn contains_tera_token(source: &str) -> bool {
    let bytes = source.as_bytes();
    let mut cursor = 0usize;
    while cursor + 1 < bytes.len() {
        if is_tera_start(bytes, cursor) {
            return true;
        }
        cursor += 1;
    }
    false
}

fn is_tera_start(bytes: &[u8], index: usize) -> bool {
    index + 1 < bytes.len()
        && bytes[index] == b'{'
        && matches!(bytes[index + 1], b'%' | b'{' | b'#')
}

fn skip_tera_token(bytes: &[u8], index: usize) -> Option<usize> {
    let (close_a, close_b) = match bytes.get(index + 1).copied()? {
        b'%' => (b'%', b'}'),
        b'{' => (b'}', b'}'),
        b'#' => (b'#', b'}'),
        _ => return None,
    };
    let mut cursor = index + 2;
    while cursor + 1 < bytes.len() {
        if bytes[cursor] == close_a && bytes[cursor + 1] == close_b {
            return Some(cursor + 2);
        }
        cursor += 1;
    }
    None
}

fn escape_text_content(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

fn line_break_count(value: &str) -> usize {
    value.bytes().filter(|byte| *byte == b'\n').count()
}

fn is_direct_html_text_file(file: &ProjectModelFile) -> bool {
    matches!(
        file.kind,
        ProjectModelFileKind::StaticText | ProjectModelFileKind::OtherText
    ) && is_html_path(&file.relative_path)
}

fn is_html_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".html") || lower.ends_with(".htm")
}

fn is_void_text_tag(tag: &str) -> bool {
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

fn is_raw_or_programmatic_text_tag(tag: &str) -> bool {
    matches!(tag, "script" | "style" | "template")
}

fn replace_range(source: &str, start: usize, end: usize, replacement: &str) -> String {
    format!("{}{}{}", &source[..start], replacement, &source[end..])
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::project_model::{
        build_project_model,
        move_engine::ProjectSourceEditLocation,
        text_engine::{plan_html_text, ProjectHtmlTextIntent},
    };

    #[test]
    fn plan_html_text_updates_template_anchor() {
        let root = unique_test_dir();
        write_project(&root, "<main>\n  <p class=\"lede\">Vechi</p>\n</main>\n");
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let paragraph = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<p .lede>")
            .unwrap();

        let plan = plan_html_text(
            &model,
            &ProjectHtmlTextIntent {
                target_source_id: Some(paragraph.id.clone()),
                target_location: None,
                target_tag: Some("p".to_string()),
                target_selector: Some(".lede".to_string()),
                text: "Nou & <sigur>".to_string(),
            },
            &HashMap::new(),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.unwrap();
        assert!(patch
            .contents
            .contains("<p class=\"lede\">Nou &amp; &lt;sigur&gt;</p>"));
        assert!(!patch.contents.contains("Vechi"));
        assert_eq!(patch.tag, "p");
        assert_eq!(patch.source_start_line, 2);
        assert_eq!(patch.line_shift, 0);
    }

    #[test]
    fn plan_html_text_resolves_active_html_by_direct_location() {
        let root = unique_test_dir();
        write_project(&root, "<main></main>\n");
        fs::create_dir_all(root.join("sursa/static")).unwrap();
        fs::write(
            root.join("sursa/static/plain.html"),
            concat!(
                "<!DOCTYPE html>\n",
                "<html>\n",
                "<body>\n",
                "  <p id=\"lead\">Vechi</p>\n",
                "</body>\n",
                "</html>\n",
            ),
        )
        .unwrap();
        let model = build_project_model(&root, &HashMap::new()).unwrap();

        let plan = plan_html_text(
            &model,
            &ProjectHtmlTextIntent {
                target_source_id: None,
                target_location: Some(ProjectSourceEditLocation {
                    file: "sursa/static/plain.html".to_string(),
                    line: 4,
                    column: 3,
                }),
                target_tag: Some("p".to_string()),
                target_selector: Some("body:nth-of-type(1) > p:nth-of-type(1)".to_string()),
                text: "Text nou".to_string(),
            },
            &HashMap::new(),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.unwrap();
        assert_eq!(patch.file, "sursa/static/plain.html");
        assert_eq!(
            patch.resolved_target_id,
            "location:sursa/static/plain.html:4:3"
        );
        assert!(patch.contents.contains("<p id=\"lead\">Text nou</p>"));
        assert_eq!(patch.source_start_line, 4);
    }

    #[test]
    fn plan_html_text_blocks_elements_with_html_children() {
        let root = unique_test_dir();
        write_project(
            &root,
            "<main>\n  <p>Salut <strong>tare</strong></p>\n</main>\n",
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let paragraph = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<p>")
            .unwrap();

        let plan = plan_html_text(
            &model,
            &ProjectHtmlTextIntent {
                target_source_id: Some(paragraph.id.clone()),
                target_location: None,
                target_tag: Some("p".to_string()),
                target_selector: Some("p".to_string()),
                text: "Alt text".to_string(),
            },
            &HashMap::new(),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.diagnostic.unwrap_or_default().contains("copii HTML"));
    }

    fn write_project(root: &PathBuf, template: &str) {
        fs::create_dir_all(root.join("sursa/content")).unwrap();
        fs::create_dir_all(root.join("sursa/templates")).unwrap();
        fs::write(
            root.join("sursa/zola.toml"),
            "base_url = \"http://example.test\"\n",
        )
        .unwrap();
        fs::write(
            root.join("sursa/content/_index.md"),
            "+++\ntitle = \"Acasă\"\ntemplate = \"index.html\"\n+++\n",
        )
        .unwrap();
        fs::write(root.join("sursa/templates/index.html"), template).unwrap();
    }

    fn unique_test_dir() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "pana-studio-text-engine-{}-{stamp}",
            std::process::id()
        ))
    }
}
