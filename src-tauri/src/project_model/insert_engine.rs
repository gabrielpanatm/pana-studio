use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    page_components::{
        component_root_class_name, page_component_by_id, render_page_component_html,
        unique_page_component_identity,
    },
    project_model::model::{ProjectModel, ProjectModelFile, ProjectModelFileKind},
    source_graph::model::SourceNode,
};

use super::move_engine::{
    can_receive_children, content_revision, direct_location_without_source_id, html_tag_at,
    inside_prefix_for_insert, line_indent_at_offset, line_number_at_offset,
    offset_for_source_location, parse_html_tag_at, reindent_html_fragment,
    resolve_html_element_span, resolve_html_node_for_anchor, same_model_path,
    source_location_at_offset, source_missing_message, ProjectMovePosition,
    ProjectSourceEditLocation, Span,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlInsertIntent {
    pub target_source_id: Option<String>,
    pub target_location: Option<ProjectSourceEditLocation>,
    pub target_tag: Option<String>,
    pub target_selector: Option<String>,
    pub target_kind: Option<String>,
    pub position: ProjectMovePosition,
    pub element: ProjectHtmlInsertElement,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlInsertElement {
    pub kind: Option<String>,
    pub component_id: Option<String>,
    pub tag: String,
    pub class_name: Option<String>,
    pub text: Option<String>,
    pub label: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlInsertPlan {
    pub allowed: bool,
    pub diagnostic: Option<String>,
    pub model_revision: String,
    pub patch: Option<ProjectHtmlInsertPatch>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlInsertPatch {
    pub file: String,
    pub resolved_target_id: String,
    pub inserted_label: String,
    pub before_revision: String,
    pub after_revision: String,
    pub contents: String,
    pub target_location: ProjectSourceEditLocation,
    pub inserted_location: ProjectSourceEditLocation,
    pub inserted_start_line: usize,
    pub line_shift_start: usize,
    pub line_shift: isize,
    pub tag: String,
    pub class_name: String,
    pub text: String,
    pub html: String,
    pub component_id: Option<String>,
    pub data_anim: Option<String>,
    pub component_instance_id: Option<String>,
}

struct InsertApplication {
    contents: String,
    inserted_location: ProjectSourceEditLocation,
    inserted_start_line: usize,
    line_shift_start: usize,
    line_shift: isize,
}

pub fn plan_html_insert(
    model: &ProjectModel,
    intent: &ProjectHtmlInsertIntent,
    aliases: &HashMap<String, String>,
) -> ProjectHtmlInsertPlan {
    match plan_html_insert_inner(model, intent, aliases) {
        Ok(patch) => ProjectHtmlInsertPlan {
            allowed: true,
            diagnostic: None,
            model_revision: model.revision.clone(),
            patch: Some(patch),
        },
        Err(message) => ProjectHtmlInsertPlan {
            allowed: false,
            diagnostic: Some(message),
            model_revision: model.revision.clone(),
            patch: None,
        },
    }
}

fn plan_html_insert_inner(
    model: &ProjectModel,
    intent: &ProjectHtmlInsertIntent,
    aliases: &HashMap<String, String>,
) -> Result<ProjectHtmlInsertPatch, String> {
    if intent
        .target_kind
        .as_deref()
        .is_some_and(|kind| kind.trim() == "empty-tera-slot")
    {
        return Err(
            "Inserarea în slot Tera cere planner Tera dedicat; HTML Insert Engine refuză acest caz."
                .to_string(),
        );
    }

    let snippet = build_insert_snippet(model, &intent.element)?;
    if let Some(target_node) = resolve_html_node_for_anchor(
        model,
        intent.target_source_id.as_deref(),
        intent.target_location.as_ref(),
        intent.target_tag.as_deref(),
        aliases,
    ) {
        return plan_html_insert_from_source_node(intent, &snippet, target_node, model);
    }

    if let Some(location) = direct_location_without_source_id(
        intent.target_source_id.as_deref(),
        intent.target_location.as_ref(),
    ) {
        return plan_html_insert_from_direct_location(model, intent, &snippet, location);
    }

    Err(source_missing_message(
        "destinație",
        intent.target_source_id.as_deref(),
        intent.target_location.as_ref(),
        intent.target_selector.as_deref(),
    ))
}

fn plan_html_insert_from_source_node(
    intent: &ProjectHtmlInsertIntent,
    snippet: &InsertSnippet,
    target_node: &SourceNode,
    model: &ProjectModel,
) -> Result<ProjectHtmlInsertPatch, String> {
    if !target_node.capabilities.can_edit_visual {
        return Err(target_node
            .capabilities
            .reason
            .clone()
            .unwrap_or_else(|| "Destinația nu este editabilă vizual.".to_string()));
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
        return Err(
            "HTML Insert Engine este activ doar pentru template-uri Zola/Tera.".to_string(),
        );
    }

    let target_range = target_node
        .range
        .as_ref()
        .ok_or_else(|| "Destinația nu are range stabil în Source Graph.".to_string())?;
    let target_span = resolve_html_element_span(&file.contents, target_range.start)?;
    let target_tag = html_tag_at(&file.contents, target_range.start)?;
    validate_insert_target(&target_tag, intent.position)?;

    let target_location =
        source_location_at_offset(&file.contents, &target_node.file, target_span.start);
    let applied = apply_html_insert(
        &file.contents,
        &target_node.file,
        target_span,
        &target_tag,
        intent.position,
        &snippet.html,
    )?;

    Ok(ProjectHtmlInsertPatch {
        file: target_node.file.clone(),
        resolved_target_id: target_node.id.clone(),
        inserted_label: format!("<{}>", snippet.tag),
        before_revision: file.revision.clone(),
        after_revision: content_revision(&applied.contents),
        contents: applied.contents,
        target_location,
        inserted_location: applied.inserted_location,
        inserted_start_line: applied.inserted_start_line,
        line_shift_start: applied.line_shift_start,
        line_shift: applied.line_shift,
        tag: snippet.tag.clone(),
        class_name: snippet.class_name.clone(),
        text: snippet.text.clone(),
        html: snippet.html.clone(),
        component_id: snippet.component_id.clone(),
        data_anim: snippet.data_anim.clone(),
        component_instance_id: snippet.component_instance_id.clone(),
    })
}

fn plan_html_insert_from_direct_location(
    model: &ProjectModel,
    intent: &ProjectHtmlInsertIntent,
    snippet: &InsertSnippet,
    location: &ProjectSourceEditLocation,
) -> Result<ProjectHtmlInsertPatch, String> {
    let file = model
        .files
        .iter()
        .find(|file| same_model_path(&file.relative_path, &location.file))
        .ok_or_else(|| format!("Nu am găsit fișierul {} în Project Model.", location.file))?;

    if !is_direct_html_insert_file(file) {
        return Err(
            "Inserarea prin locație directă este activă doar pentru fișiere HTML 1:1 din proiect, nu pentru template-uri Tera.".to_string(),
        );
    }

    let offset = offset_for_source_location(&file.contents, location)?;
    let tag = parse_html_tag_at(&file.contents, offset)
        .ok_or_else(|| "Locația nu indică începutul unui tag HTML pentru inserare.".to_string())?;
    if tag.is_closing {
        return Err("Locația indică un tag de închidere, nu o destinație de inserare.".to_string());
    }
    if let Some(expected_tag) = intent.target_tag.as_deref() {
        let expected_tag = expected_tag.trim().to_ascii_lowercase();
        if !expected_tag.is_empty() && expected_tag != tag.tag {
            return Err(format!(
                "Locația indică <{}>, dar intenția preview a cerut <{}>.",
                tag.tag, expected_tag
            ));
        }
    }
    validate_insert_target(&tag.tag, intent.position)?;

    let target_span = resolve_html_element_span(&file.contents, tag.start)?;
    let applied = apply_html_insert(
        &file.contents,
        &file.relative_path,
        target_span,
        &tag.tag,
        intent.position,
        &snippet.html,
    )?;
    let resolved_target_id = intent.target_source_id.clone().unwrap_or_else(|| {
        format!(
            "location:{}:{}:{}",
            location.file, location.line, location.column
        )
    });

    Ok(ProjectHtmlInsertPatch {
        file: file.relative_path.clone(),
        resolved_target_id,
        inserted_label: format!("<{}>", snippet.tag),
        before_revision: file.revision.clone(),
        after_revision: content_revision(&applied.contents),
        contents: applied.contents,
        target_location: source_location_at_offset(
            &file.contents,
            &file.relative_path,
            target_span.start,
        ),
        inserted_location: applied.inserted_location,
        inserted_start_line: applied.inserted_start_line,
        line_shift_start: applied.line_shift_start,
        line_shift: applied.line_shift,
        tag: snippet.tag.clone(),
        class_name: snippet.class_name.clone(),
        text: snippet.text.clone(),
        html: snippet.html.clone(),
        component_id: snippet.component_id.clone(),
        data_anim: snippet.data_anim.clone(),
        component_instance_id: snippet.component_instance_id.clone(),
    })
}

fn validate_insert_target(tag: &str, position: ProjectMovePosition) -> Result<(), String> {
    if tag.eq_ignore_ascii_case("html") {
        return Err("Elementul <html> nu este o destinație de inserare vizuală.".to_string());
    }
    if tag.eq_ignore_ascii_case("body") && position != ProjectMovePosition::Inside {
        return Err("Elementul <body> poate primi inserări doar în interior.".to_string());
    }
    if position == ProjectMovePosition::Inside
        && !tag.eq_ignore_ascii_case("body")
        && !can_receive_children(tag)
    {
        return Err(format!("<{tag}> nu este container pentru copii."));
    }
    Ok(())
}

fn is_direct_html_insert_file(file: &ProjectModelFile) -> bool {
    matches!(
        file.kind,
        ProjectModelFileKind::StaticText | ProjectModelFileKind::OtherText
    ) && is_html_path(&file.relative_path)
}

fn is_html_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".html") || lower.ends_with(".htm")
}

struct InsertSnippet {
    tag: String,
    class_name: String,
    text: String,
    html: String,
    component_id: Option<String>,
    data_anim: Option<String>,
    component_instance_id: Option<String>,
}

fn build_insert_snippet(
    model: &ProjectModel,
    element: &ProjectHtmlInsertElement,
) -> Result<InsertSnippet, String> {
    if element
        .kind
        .as_deref()
        .is_some_and(|kind| kind.trim() == "component")
        || element
            .component_id
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
    {
        return build_component_insert_snippet(model, element);
    }

    let tag = normalize_tag(&element.tag)?;
    let class_name = normalize_class_name(element.class_name.as_deref().unwrap_or(""));
    let text = normalize_text(element.text.as_deref().unwrap_or(""));
    Ok(InsertSnippet {
        html: build_html_snippet(&tag, &class_name, &text),
        tag,
        class_name,
        text,
        component_id: None,
        data_anim: None,
        component_instance_id: None,
    })
}

fn build_component_insert_snippet(
    model: &ProjectModel,
    element: &ProjectHtmlInsertElement,
) -> Result<InsertSnippet, String> {
    let component_id = element
        .component_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Inserarea componentei nu a primit componentId.".to_string())?;
    let component = page_component_by_id(component_id).ok_or_else(|| {
        format!("Componenta {component_id} nu există în Page Component Registry Rust.")
    })?;
    let provided_tag = element.tag.trim().to_ascii_lowercase();
    if !provided_tag.is_empty() && provided_tag != component.tag {
        return Err(format!(
            "Componenta {} cere tag <{}>, dar UI-ul a cerut <{}>.",
            component.id, component.tag, provided_tag
        ));
    }

    let seed = format!(
        "{}:{}:{}:{}",
        model.revision,
        component.id,
        element.label.as_deref().unwrap_or(component.label),
        component.tag
    );
    let identity = unique_page_component_identity(component.id, &seed, |candidate| {
        model
            .files
            .iter()
            .any(|file| file.contents.contains(candidate))
    });
    let html = render_page_component_html(component, &identity);

    Ok(InsertSnippet {
        tag: component.tag.to_string(),
        class_name: component_root_class_name(component, &identity),
        text: component.text.to_string(),
        html,
        component_id: Some(component.id.to_string()),
        data_anim: Some(identity.data_anim),
        component_instance_id: Some(identity.instance_id),
    })
}

fn apply_html_insert(
    source: &str,
    file: &str,
    target_span: Span,
    target_tag: &str,
    position: ProjectMovePosition,
    snippet: &str,
) -> Result<InsertApplication, String> {
    let target_indent = line_indent_at_offset(source, target_span.start);
    match position {
        ProjectMovePosition::Before => {
            let inserted = reindent_html_fragment(snippet, &target_indent);
            let inserted_start_line = line_number_at_offset(source, target_span.start);
            Ok(InsertApplication {
                contents: format!(
                    "{}{}\n{}",
                    &source[..target_span.start],
                    inserted,
                    &source[target_span.start..]
                ),
                inserted_location: ProjectSourceEditLocation {
                    file: file.to_string(),
                    line: inserted_start_line,
                    column: target_indent.chars().count() + 1,
                },
                inserted_start_line,
                line_shift_start: inserted_start_line,
                line_shift: snippet_line_count(&inserted) as isize,
            })
        }
        ProjectMovePosition::After => {
            let inserted = reindent_html_fragment(snippet, &target_indent);
            let inserted_start_line = line_number_at_offset(source, target_span.end) + 1;
            Ok(InsertApplication {
                contents: format!(
                    "{}\n{}{}",
                    &source[..target_span.end],
                    inserted,
                    &source[target_span.end..]
                ),
                inserted_location: ProjectSourceEditLocation {
                    file: file.to_string(),
                    line: inserted_start_line,
                    column: target_indent.chars().count() + 1,
                },
                inserted_start_line,
                line_shift_start: inserted_start_line,
                line_shift: snippet_line_count(&inserted) as isize,
            })
        }
        ProjectMovePosition::Inside => {
            let target_source = source
                .get(target_span.start..target_span.end)
                .ok_or_else(|| "Range destinație invalid pentru inserare.".to_string())?;
            let close_tag = format!("</{target_tag}>");
            let close_offset = target_source
                .to_ascii_lowercase()
                .rfind(&close_tag.to_ascii_lowercase())
                .ok_or_else(|| format!("Nu am găsit {close_tag} pentru inserare."))?;
            let opening = parse_html_tag_at(source, target_span.start).ok_or_else(|| {
                "Nu am putut reciti tag-ul destinație pentru inserare.".to_string()
            })?;
            let child_indent = format!("{target_indent}  ");
            let inserted = reindent_html_fragment(snippet, &child_indent);
            let insert_at = target_span.start + close_offset;
            let before_insert = inside_prefix_for_insert(source, opening.end, insert_at);
            let inserted_start_line =
                line_number_at_offset(&before_insert, before_insert.len()) + 1;
            Ok(InsertApplication {
                contents: format!(
                    "{}\n{}\n{}{}",
                    before_insert,
                    inserted,
                    target_indent,
                    &source[insert_at..]
                ),
                inserted_location: ProjectSourceEditLocation {
                    file: file.to_string(),
                    line: inserted_start_line,
                    column: child_indent.chars().count() + 1,
                },
                inserted_start_line,
                line_shift_start: inserted_start_line,
                line_shift: snippet_line_count(&inserted) as isize + 1,
            })
        }
    }
}

fn normalize_tag(value: &str) -> Result<String, String> {
    let tag = value.trim().to_ascii_lowercase();
    let mut chars = tag.chars();
    let Some(first) = chars.next() else {
        return Err("HTML Insert Engine a primit tag gol.".to_string());
    };
    if !first.is_ascii_lowercase() {
        return Err(format!("HTML Insert Engine a primit tag invalid: {value}."));
    }
    if !chars.all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
    }) {
        return Err(format!("HTML Insert Engine a primit tag invalid: {value}."));
    }
    Ok(tag)
}

fn normalize_class_name(value: &str) -> String {
    value
        .split_whitespace()
        .map(str::trim)
        .filter(|token| !token.is_empty() && !token.contains('\0'))
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_text(value: &str) -> String {
    value.trim().chars().take(4000).collect()
}

fn snippet_line_count(value: &str) -> usize {
    value.split('\n').count()
}

fn build_html_snippet(tag: &str, class_name: &str, text: &str) -> String {
    let attrs = root_attrs(class_name);
    if tag == "a" {
        return format!("<a{attrs} href=\"#\">{}</a>", text_or(text, "Link nou"));
    }
    if tag == "button" {
        return format!(
            "<button{attrs} type=\"button\">{}</button>",
            text_or(text, "Buton nou")
        );
    }
    if tag == "img" {
        return format!(
            "<img{attrs} src=\"\" alt=\"{}\">",
            escape_attr(text_or_raw(text, "Imagine"))
        );
    }
    if tag == "input" {
        return format!(
            "<input{attrs} type=\"text\" placeholder=\"{}\">",
            escape_attr(text_or_raw(text, "Text"))
        );
    }
    if tag == "source" {
        return format!("<source{attrs} src=\"\" type=\"\">");
    }
    if tag == "video" {
        return format!("<video{attrs} controls></video>");
    }
    if tag == "audio" {
        return format!("<audio{attrs} controls></audio>");
    }
    if tag == "iframe" {
        return format!(
            "<iframe{attrs} src=\"\" title=\"{}\"></iframe>",
            escape_attr(text_or_raw(text, "Iframe"))
        );
    }
    if tag == "picture" {
        return format!(
            "<picture{attrs}><img src=\"\" alt=\"{}\"></picture>",
            escape_attr(text_or_raw(text, "Imagine"))
        );
    }
    if tag == "ul" {
        return format!(
            "<ul{attrs}><li>{}</li></ul>",
            text_or(text, "Element listă")
        );
    }
    if tag == "ol" {
        return format!(
            "<ol{attrs}><li>{}</li></ol>",
            text_or(text, "Element listă")
        );
    }
    if tag == "dl" {
        return format!(
            "<dl{attrs}><dt>{}</dt><dd>Descriere</dd></dl>",
            text_or(text, "Termen")
        );
    }
    if tag == "form" {
        return format!(
            "<form{attrs}><button type=\"submit\">{}</button></form>",
            text_or(text, "Trimite")
        );
    }
    if tag == "textarea" {
        return format!(
            "<textarea{attrs} placeholder=\"{}\"></textarea>",
            escape_attr(text_or_raw(text, "Text"))
        );
    }
    if tag == "select" {
        return format!(
            "<select{attrs}><option>{}</option></select>",
            text_or(text, "Opțiune")
        );
    }
    if tag == "fieldset" {
        return format!(
            "<fieldset{attrs}><legend>{}</legend></fieldset>",
            text_or(text, "Legendă")
        );
    }
    if tag == "table" {
        return format!(
            "<table{attrs}><tbody><tr><td>{}</td></tr></tbody></table>",
            text_or(text, "Celulă")
        );
    }
    if tag == "thead" {
        return format!(
            "<thead{attrs}><tr><th>{}</th></tr></thead>",
            text_or(text, "Titlu")
        );
    }
    if tag == "tbody" {
        return format!(
            "<tbody{attrs}><tr><td>{}</td></tr></tbody>",
            text_or(text, "Celulă")
        );
    }
    if tag == "tfoot" {
        return format!(
            "<tfoot{attrs}><tr><td>{}</td></tr></tfoot>",
            text_or(text, "Total")
        );
    }
    if tag == "tr" {
        return format!("<tr{attrs}><td>{}</td></tr>", text_or(text, "Celulă"));
    }
    if tag == "th" {
        return format!("<th{attrs}>{}</th>", text_or(text, "Titlu"));
    }
    if tag == "td" {
        return format!("<td{attrs}>{}</td>", text_or(text, "Celulă"));
    }
    if tag == "caption" {
        return format!(
            "<caption{attrs}>{}</caption>",
            text_or(text, "Descriere tabel")
        );
    }
    if is_void_snippet_tag(tag) {
        return format!("<{tag}{attrs}>");
    }
    format!(
        "<{tag}{attrs}>{}</{tag}>",
        if text.trim().is_empty() {
            String::new()
        } else {
            escape_text(text)
        }
    )
}

fn root_attrs(class_name: &str) -> String {
    if class_name.trim().is_empty() {
        String::new()
    } else {
        format!(" class=\"{}\"", escape_attr(class_name))
    }
}

fn text_or(value: &str, fallback: &str) -> String {
    escape_text(text_or_raw(value, fallback))
}

fn text_or_raw<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback
    } else {
        trimmed
    }
}

fn escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_attr(value: &str) -> String {
    escape_text(value).replace('"', "&quot;")
}

fn is_void_snippet_tag(tag: &str) -> bool {
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
    use std::{
        collections::HashMap,
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::project_model::build_project_model;

    use super::*;

    #[test]
    fn plan_html_insert_inserts_child_with_project_model_anchor() {
        let root = unique_test_dir();
        write_project(
            &root,
            concat!(
                "{% block content %}\n",
                "<section class=\"hero\">\n",
                "  <h1>Titlu</h1>\n",
                "</section>\n",
                "{% endblock %}\n",
            ),
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let section = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section .hero>")
            .unwrap();

        let plan = plan_html_insert(
            &model,
            &ProjectHtmlInsertIntent {
                target_source_id: Some(section.id.clone()),
                target_location: None,
                target_tag: Some("section".to_string()),
                target_selector: Some(".hero".to_string()),
                target_kind: Some("html".to_string()),
                position: ProjectMovePosition::Inside,
                element: ProjectHtmlInsertElement {
                    kind: Some("html".to_string()),
                    component_id: None,
                    tag: "p".to_string(),
                    class_name: Some("lede".to_string()),
                    text: Some("Salut".to_string()),
                    label: Some("Paragraph".to_string()),
                },
            },
            &HashMap::new(),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.unwrap();
        assert!(patch.contents.contains("  <p class=\"lede\">Salut</p>"));
        assert_eq!(patch.inserted_location.line, 4);
        assert_eq!(patch.tag, "p");
    }

    #[test]
    fn plan_html_insert_renders_registered_component_from_rust_registry() {
        let root = unique_test_dir();
        write_project(&root, "<section></section>\n");
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let section = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section>")
            .unwrap();

        let plan = plan_html_insert(
            &model,
            &ProjectHtmlInsertIntent {
                target_source_id: Some(section.id.clone()),
                target_location: None,
                target_tag: Some("section".to_string()),
                target_selector: Some("section".to_string()),
                target_kind: Some("html".to_string()),
                position: ProjectMovePosition::Inside,
                element: ProjectHtmlInsertElement {
                    kind: Some("component".to_string()),
                    component_id: Some("counter".to_string()),
                    tag: "span".to_string(),
                    class_name: None,
                    text: None,
                    label: Some("Counter".to_string()),
                },
            },
            &HashMap::new(),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.unwrap();
        assert_eq!(patch.component_id.as_deref(), Some("counter"));
        assert!(patch.html.contains(r#"data-pana-component="counter""#));
        assert!(patch.html.contains("ps-counter-"));
        assert!(!patch.html.contains("__PANA_"));
        assert!(patch
            .contents
            .contains(r#"data-pana-instance="counter-counter-"#));
    }

    #[test]
    fn plan_html_insert_resolves_active_html_by_direct_location() {
        let root = unique_test_dir();
        write_project(&root, "<main></main>\n");
        fs::create_dir_all(root.join("sursa/static")).unwrap();
        fs::write(
            root.join("sursa/static/plain.html"),
            concat!(
                "<!DOCTYPE html>\n",
                "<html>\n",
                "<body>\n",
                "  <section id=\"hero\">\n",
                "  </section>\n",
                "</body>\n",
                "</html>\n",
            ),
        )
        .unwrap();
        let model = build_project_model(&root, &HashMap::new()).unwrap();

        let plan = plan_html_insert(
            &model,
            &ProjectHtmlInsertIntent {
                target_source_id: None,
                target_location: Some(ProjectSourceEditLocation {
                    file: "sursa/static/plain.html".to_string(),
                    line: 4,
                    column: 3,
                }),
                target_tag: Some("section".to_string()),
                target_selector: Some("body:nth-of-type(1) > section:nth-of-type(1)".to_string()),
                target_kind: Some("html".to_string()),
                position: ProjectMovePosition::Inside,
                element: ProjectHtmlInsertElement {
                    kind: Some("html".to_string()),
                    component_id: None,
                    tag: "p".to_string(),
                    class_name: Some("lede".to_string()),
                    text: Some("Salut".to_string()),
                    label: Some("Paragraph".to_string()),
                },
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
        assert!(patch.contents.contains("    <p class=\"lede\">Salut</p>"));
        assert_eq!(patch.inserted_location.line, 5);
        assert_eq!(patch.tag, "p");
    }

    #[test]
    fn plan_html_insert_blocks_unknown_component_id() {
        let root = unique_test_dir();
        write_project(&root, "<section></section>\n");
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let section = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section>")
            .unwrap();

        let plan = plan_html_insert(
            &model,
            &ProjectHtmlInsertIntent {
                target_source_id: Some(section.id.clone()),
                target_location: None,
                target_tag: Some("section".to_string()),
                target_selector: Some("section".to_string()),
                target_kind: Some("html".to_string()),
                position: ProjectMovePosition::Inside,
                element: ProjectHtmlInsertElement {
                    kind: Some("component".to_string()),
                    component_id: Some("hero-card".to_string()),
                    tag: "section".to_string(),
                    class_name: None,
                    text: None,
                    label: Some("Hero Card".to_string()),
                },
            },
            &HashMap::new(),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan
            .diagnostic
            .unwrap()
            .contains("Page Component Registry Rust"));
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
            "pana-studio-insert-engine-{}-{stamp}",
            std::process::id()
        ))
    }
}
