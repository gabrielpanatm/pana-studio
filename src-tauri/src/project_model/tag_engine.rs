use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    project_model::model::{ProjectModel, ProjectModelFile, ProjectModelFileKind},
    source_graph::model::SourceNode,
};

use super::html_editor_schema::tag_transition_diagnostic;
use super::move_engine::{
    content_revision, direct_location_without_source_id, html_tag_at, offset_for_source_location,
    parse_html_tag_at, resolve_html_element_span, resolve_html_node_for_anchor, same_model_path,
    source_location_at_offset, source_missing_message, HtmlTag, ProjectSourceEditLocation, Span,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlTagIntent {
    pub target_source_id: Option<String>,
    pub target_location: Option<ProjectSourceEditLocation>,
    pub target_tag: Option<String>,
    pub target_selector: Option<String>,
    pub new_tag: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlTagPlan {
    pub allowed: bool,
    pub diagnostic: Option<String>,
    pub model_revision: String,
    pub patch: Option<ProjectHtmlTagPatch>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlTagPatch {
    pub file: String,
    pub resolved_target_id: String,
    pub before_revision: String,
    pub after_revision: String,
    pub contents: String,
    pub target_location: ProjectSourceEditLocation,
    pub source_start_line: usize,
    pub line_shift_start: usize,
    pub line_shift: isize,
    pub old_tag: String,
    pub new_tag: String,
}

struct TagApplication {
    contents: String,
    target_location: ProjectSourceEditLocation,
    source_start_line: usize,
}

pub fn plan_html_tag(
    model: &ProjectModel,
    intent: &ProjectHtmlTagIntent,
    aliases: &HashMap<String, String>,
) -> ProjectHtmlTagPlan {
    match plan_html_tag_inner(model, intent, aliases) {
        Ok(patch) => ProjectHtmlTagPlan {
            allowed: true,
            diagnostic: None,
            model_revision: model.revision.clone(),
            patch: Some(patch),
        },
        Err(message) => ProjectHtmlTagPlan {
            allowed: false,
            diagnostic: Some(message),
            model_revision: model.revision.clone(),
            patch: None,
        },
    }
}

fn plan_html_tag_inner(
    model: &ProjectModel,
    intent: &ProjectHtmlTagIntent,
    aliases: &HashMap<String, String>,
) -> Result<ProjectHtmlTagPatch, String> {
    let new_tag = normalize_new_tag(&intent.new_tag)?;

    if let Some(target_node) = resolve_html_node_for_anchor(
        model,
        intent.target_source_id.as_deref(),
        intent.target_location.as_ref(),
        intent.target_tag.as_deref(),
        aliases,
    ) {
        return plan_html_tag_from_source_node(model, intent, target_node, new_tag);
    }

    if let Some(location) = direct_location_without_source_id(
        intent.target_source_id.as_deref(),
        intent.target_location.as_ref(),
    ) {
        return plan_html_tag_from_direct_location(model, intent, location, new_tag);
    }

    Err(source_missing_message(
        "țintă de tag",
        intent.target_source_id.as_deref(),
        intent.target_location.as_ref(),
        intent.target_selector.as_deref(),
    ))
}

fn plan_html_tag_from_source_node(
    model: &ProjectModel,
    intent: &ProjectHtmlTagIntent,
    target_node: &SourceNode,
    new_tag: String,
) -> Result<ProjectHtmlTagPatch, String> {
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
        return Err("HTML Tag Engine este activ doar pentru template-uri Zola/Tera.".to_string());
    }

    let target_range = target_node
        .range
        .as_ref()
        .ok_or_else(|| "Ținta nu are range stabil în Source Graph.".to_string())?;
    let old_tag = html_tag_at(&file.contents, target_range.start)?;
    validate_current_tag(intent, &old_tag)?;
    let applied = apply_html_tag(
        &file.contents,
        &target_node.file,
        target_range.start,
        &new_tag,
    )?;

    Ok(ProjectHtmlTagPatch {
        file: target_node.file.clone(),
        resolved_target_id: target_node.id.clone(),
        before_revision: file.revision.clone(),
        after_revision: content_revision(&applied.contents),
        contents: applied.contents,
        target_location: applied.target_location,
        source_start_line: applied.source_start_line,
        line_shift_start: applied.source_start_line,
        line_shift: 0,
        old_tag,
        new_tag,
    })
}

fn plan_html_tag_from_direct_location(
    model: &ProjectModel,
    intent: &ProjectHtmlTagIntent,
    location: &ProjectSourceEditLocation,
    new_tag: String,
) -> Result<ProjectHtmlTagPatch, String> {
    let file = model
        .files
        .iter()
        .find(|file| same_model_path(&file.relative_path, &location.file))
        .ok_or_else(|| format!("Nu am găsit fișierul {} în Project Model.", location.file))?;

    if !is_direct_html_tag_file(file) {
        return Err(
            "Schimbarea tag-ului prin locație directă este activă doar pentru fișiere HTML 1:1 din proiect, nu pentru template-uri Tera.".to_string(),
        );
    }

    let offset = offset_for_source_location(&file.contents, location)?;
    let tag = parse_html_tag_at(&file.contents, offset)
        .ok_or_else(|| "Locația nu indică începutul unui tag HTML pentru schimbare.".to_string())?;
    if tag.is_closing {
        return Err("Locația indică un tag de închidere, nu un element mutabil.".to_string());
    }
    validate_current_tag(intent, &tag.tag)?;
    let applied = apply_html_tag(&file.contents, &file.relative_path, tag.start, &new_tag)?;
    let resolved_target_id = intent.target_source_id.clone().unwrap_or_else(|| {
        format!(
            "location:{}:{}:{}",
            location.file, location.line, location.column
        )
    });

    Ok(ProjectHtmlTagPatch {
        file: file.relative_path.clone(),
        resolved_target_id,
        before_revision: file.revision.clone(),
        after_revision: content_revision(&applied.contents),
        contents: applied.contents,
        target_location: applied.target_location,
        source_start_line: applied.source_start_line,
        line_shift_start: applied.source_start_line,
        line_shift: 0,
        old_tag: tag.tag,
        new_tag,
    })
}

fn validate_current_tag(intent: &ProjectHtmlTagIntent, actual_tag: &str) -> Result<(), String> {
    if let Some(expected_tag) = intent.target_tag.as_deref() {
        let expected_tag = expected_tag.trim().to_ascii_lowercase();
        if !expected_tag.is_empty() && expected_tag != actual_tag {
            return Err(format!(
                "Locația indică <{}>, dar intenția preview a cerut <{}>.",
                actual_tag, expected_tag
            ));
        }
    }
    if is_protected_tag(actual_tag) {
        return Err(format!(
            "Elementul <{}> nu este editabil vizual pentru schimbare de tag.",
            actual_tag
        ));
    }
    Ok(())
}

fn apply_html_tag(
    source: &str,
    file: &str,
    opening_start: usize,
    new_tag: &str,
) -> Result<TagApplication, String> {
    let opening = parse_html_tag_at(source, opening_start)
        .ok_or_else(|| "Range-ul nu mai indică un tag HTML stabil.".to_string())?;
    if opening.is_closing {
        return Err("Range-ul indică un tag HTML de închidere, nu un element mutabil.".to_string());
    }
    if opening.tag == new_tag {
        return Err(format!("Elementul este deja <{}>.", new_tag));
    }
    if opening.is_self_closing || is_void_tag(&opening.tag) {
        return Err(format!(
            "Elementul <{}> este void sau self-closing; conversia de tag este blocată.",
            opening.tag
        ));
    }
    if let Some(diagnostic) = tag_transition_diagnostic(&opening.tag, new_tag) {
        return Err(diagnostic);
    }

    let span = resolve_html_element_span(source, opening.start)?;
    let closing = matching_closing_tag(source, &opening, span)?;
    let updated_opening = replace_tag_name(source, opening.start, opening.end, false, new_tag)?;
    let updated_closing = replace_tag_name(source, closing.start, closing.end, true, new_tag)?;
    let contents = format!(
        "{}{}{}{}{}",
        &source[..opening.start],
        updated_opening,
        &source[opening.end..closing.start],
        updated_closing,
        &source[closing.end..]
    );
    let target_location = source_location_at_offset(source, file, opening.start);

    Ok(TagApplication {
        contents,
        source_start_line: target_location.line,
        target_location,
    })
}

fn normalize_new_tag(tag: &str) -> Result<String, String> {
    let normalized = tag.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err("Tag-ul nou este gol.".to_string());
    }
    if !is_valid_html_tag_name(&normalized) {
        return Err(format!("Tag-ul nou <{}> are nume invalid.", normalized));
    }
    if is_protected_tag(&normalized) {
        return Err(format!(
            "Tag-ul nou <{}> cere editor dedicat sau este protejat.",
            normalized
        ));
    }
    if is_void_tag(&normalized) {
        return Err(format!(
            "Conversia către elementul void <{}> este blocată pentru a nu pierde conținut.",
            normalized
        ));
    }
    Ok(normalized)
}

fn matching_closing_tag(source: &str, opening: &HtmlTag, span: Span) -> Result<HtmlTag, String> {
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
                return Ok(tag);
            }
        } else if !tag.is_self_closing && !is_void_tag(&tag.tag) {
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

fn replace_tag_name(
    source: &str,
    start: usize,
    end: usize,
    closing: bool,
    new_tag: &str,
) -> Result<String, String> {
    let raw = source
        .get(start..end)
        .ok_or_else(|| "Nu am putut citi tag-ul HTML.".to_string())?;
    let bytes = raw.as_bytes();
    let mut cursor = 1usize;
    if closing {
        if bytes.get(cursor).copied() != Some(b'/') {
            return Err("Tag-ul de închidere nu are slash stabil.".to_string());
        }
        cursor += 1;
    }
    while bytes
        .get(cursor)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        cursor += 1;
    }
    let name_start = cursor;
    while bytes
        .get(cursor)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'-' || *byte == b':')
    {
        cursor += 1;
    }
    if cursor == name_start {
        return Err("Nu am putut localiza numele tag-ului.".to_string());
    }
    Ok(format!(
        "{}{}{}",
        &raw[..name_start],
        new_tag,
        &raw[cursor..]
    ))
}

fn is_valid_html_tag_name(tag: &str) -> bool {
    let mut chars = tag.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() {
        return false;
    }
    chars.all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | ':'))
}

fn is_protected_tag(tag: &str) -> bool {
    matches!(
        tag,
        "html" | "body" | "head" | "script" | "style" | "template"
    )
}

fn is_direct_html_tag_file(file: &ProjectModelFile) -> bool {
    matches!(
        file.kind,
        ProjectModelFileKind::StaticText | ProjectModelFileKind::OtherText
    ) && is_html_path(&file.relative_path)
}

fn is_html_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".html") || lower.ends_with(".htm")
}

fn is_void_tag(tag: &str) -> bool {
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
        tag_engine::{plan_html_tag, ProjectHtmlTagIntent},
    };

    #[test]
    fn plan_html_tag_updates_template_anchor() {
        let root = unique_test_dir();
        write_project(
            &root,
            "<main>\n  <section class=\"hero\"><p>Text</p></section>\n</main>\n",
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let section = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section .hero>")
            .unwrap();

        let plan = plan_html_tag(
            &model,
            &ProjectHtmlTagIntent {
                target_source_id: Some(section.id.clone()),
                target_location: None,
                target_tag: Some("section".to_string()),
                target_selector: Some(".hero".to_string()),
                new_tag: "article".to_string(),
            },
            &HashMap::new(),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.unwrap();
        assert!(patch
            .contents
            .contains("<article class=\"hero\"><p>Text</p></article>"));
        assert!(!patch.contents.contains("<section"));
        assert_eq!(patch.old_tag, "section");
        assert_eq!(patch.new_tag, "article");
        assert_eq!(patch.source_start_line, 2);
    }

    #[test]
    fn plan_html_tag_resolves_active_html_by_direct_location() {
        let root = unique_test_dir();
        write_project(&root, "<main></main>\n");
        fs::create_dir_all(root.join("sursa/static")).unwrap();
        fs::write(
            root.join("sursa/static/plain.html"),
            concat!(
                "<!DOCTYPE html>\n",
                "<html>\n",
                "<body>\n",
                "  <div id=\"card\"><span>Text</span></div>\n",
                "</body>\n",
                "</html>\n",
            ),
        )
        .unwrap();
        let model = build_project_model(&root, &HashMap::new()).unwrap();

        let plan = plan_html_tag(
            &model,
            &ProjectHtmlTagIntent {
                target_source_id: None,
                target_location: Some(ProjectSourceEditLocation {
                    file: "sursa/static/plain.html".to_string(),
                    line: 4,
                    column: 3,
                }),
                target_tag: Some("div".to_string()),
                target_selector: Some("body:nth-of-type(1) > div:nth-of-type(1)".to_string()),
                new_tag: "section".to_string(),
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
        assert!(patch
            .contents
            .contains("<section id=\"card\"><span>Text</span></section>"));
    }

    #[test]
    fn plan_html_tag_blocks_void_conversion() {
        let root = unique_test_dir();
        write_project(&root, "<main>\n  <div>Text</div>\n</main>\n");
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let div = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<div>")
            .unwrap();

        let plan = plan_html_tag(
            &model,
            &ProjectHtmlTagIntent {
                target_source_id: Some(div.id.clone()),
                target_location: None,
                target_tag: Some("div".to_string()),
                target_selector: Some("div".to_string()),
                new_tag: "img".to_string(),
            },
            &HashMap::new(),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.diagnostic.unwrap_or_default().contains("void"));
    }

    #[test]
    fn plan_html_tag_blocks_structurally_incompatible_conversion() {
        let root = unique_test_dir();
        write_project(&root, "<main>\n  <ul><li>Text</li></ul>\n</main>\n");
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let list = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<ul>")
            .unwrap();

        let plan = plan_html_tag(
            &model,
            &ProjectHtmlTagIntent {
                target_source_id: Some(list.id.clone()),
                target_location: None,
                target_tag: Some("ul".to_string()),
                target_selector: Some("ul".to_string()),
                new_tag: "section".to_string(),
            },
            &HashMap::new(),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.diagnostic.unwrap_or_default().contains("structural"));
    }

    #[test]
    fn plan_html_tag_blocks_destination_missing_from_design_safe_preview() {
        let root = unique_test_dir();
        write_project(&root, "<main>\n  <div>Text</div>\n</main>\n");
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let div = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<div>")
            .unwrap();

        let plan = plan_html_tag(
            &model,
            &ProjectHtmlTagIntent {
                target_source_id: Some(div.id.clone()),
                target_location: None,
                target_tag: Some("div".to_string()),
                target_selector: Some("div".to_string()),
                new_tag: "iframe".to_string(),
            },
            &HashMap::new(),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.diagnostic.unwrap_or_default().contains("Design Safe"));
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
            "pana-studio-tag-engine-{}-{stamp}",
            std::process::id()
        ))
    }
}
