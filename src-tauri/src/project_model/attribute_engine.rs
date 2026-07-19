use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::{
    project_model::model::{ProjectModel, ProjectModelFile, ProjectModelFileKind},
    source_graph::model::SourceNode,
};

use super::html_editor_schema::validate_visual_attribute_mutation;
use super::move_engine::{
    content_revision, direct_location_without_source_id, html_tag_at, offset_for_source_location,
    parse_html_tag_at, resolve_html_node_for_anchor, same_model_path, source_location_at_offset,
    source_missing_message, ProjectSourceEditLocation,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlAttributeIntent {
    pub target_source_id: Option<String>,
    pub target_location: Option<ProjectSourceEditLocation>,
    pub target_tag: Option<String>,
    pub target_selector: Option<String>,
    pub attributes: Vec<ProjectHtmlAttributeMutation>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProjectHtmlAttributeMutation {
    SetAttribute { name: String, value: String },
    RemoveAttribute { name: String },
}

impl ProjectHtmlAttributeMutation {
    #[cfg(test)]
    pub(crate) fn set(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self::SetAttribute {
            name: name.into(),
            value: value.into(),
        }
    }

    #[cfg(test)]
    pub(crate) fn remove(name: impl Into<String>) -> Self {
        Self::RemoveAttribute { name: name.into() }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlAttributePlan {
    pub allowed: bool,
    pub diagnostic: Option<String>,
    pub model_revision: String,
    pub patch: Option<ProjectHtmlAttributePatch>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlAttributePatch {
    pub file: String,
    pub resolved_target_id: String,
    pub before_revision: String,
    pub after_revision: String,
    pub contents: String,
    pub target_location: ProjectSourceEditLocation,
    pub source_start_line: usize,
    pub tag: String,
    pub attributes: BTreeMap<String, Option<String>>,
}

struct AttributeApplication {
    contents: String,
    target_location: ProjectSourceEditLocation,
    source_start_line: usize,
}

pub fn plan_html_attributes(
    model: &ProjectModel,
    intent: &ProjectHtmlAttributeIntent,
    aliases: &HashMap<String, String>,
) -> ProjectHtmlAttributePlan {
    match plan_html_attributes_inner(model, intent, aliases) {
        Ok(patch) => ProjectHtmlAttributePlan {
            allowed: true,
            diagnostic: None,
            model_revision: model.revision.clone(),
            patch: Some(patch),
        },
        Err(message) => ProjectHtmlAttributePlan {
            allowed: false,
            diagnostic: Some(message),
            model_revision: model.revision.clone(),
            patch: None,
        },
    }
}

fn plan_html_attributes_inner(
    model: &ProjectModel,
    intent: &ProjectHtmlAttributeIntent,
    aliases: &HashMap<String, String>,
) -> Result<ProjectHtmlAttributePatch, String> {
    let attributes = normalize_attribute_mutations(&intent.attributes)?;

    if let Some(target_node) = resolve_html_node_for_anchor(
        model,
        intent.target_source_id.as_deref(),
        intent.target_location.as_ref(),
        intent.target_tag.as_deref(),
        aliases,
    ) {
        return plan_html_attributes_from_source_node(model, intent, target_node, attributes);
    }

    if let Some(location) = direct_location_without_source_id(
        intent.target_source_id.as_deref(),
        intent.target_location.as_ref(),
    ) {
        return plan_html_attributes_from_direct_location(model, intent, location, attributes);
    }

    Err(source_missing_message(
        "țintă de atribute",
        intent.target_source_id.as_deref(),
        intent.target_location.as_ref(),
        intent.target_selector.as_deref(),
    ))
}

fn plan_html_attributes_from_source_node(
    model: &ProjectModel,
    intent: &ProjectHtmlAttributeIntent,
    target_node: &SourceNode,
    attributes: BTreeMap<String, Option<String>>,
) -> Result<ProjectHtmlAttributePatch, String> {
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
        return Err(
            "HTML Attribute Engine este activ doar pentru template-uri Zola/Tera.".to_string(),
        );
    }

    let target_range = target_node
        .range
        .as_ref()
        .ok_or_else(|| "Ținta nu are range stabil în Source Graph.".to_string())?;
    let target_tag = html_tag_at(&file.contents, target_range.start)?;
    validate_target_tag(intent, &target_tag)?;
    validate_schema_attributes(&target_tag, &attributes)?;
    let applied = apply_html_attributes(
        &file.contents,
        &target_node.file,
        target_range.start,
        &attributes,
    )?;

    Ok(ProjectHtmlAttributePatch {
        file: target_node.file.clone(),
        resolved_target_id: target_node.id.clone(),
        before_revision: file.revision.clone(),
        after_revision: content_revision(&applied.contents),
        contents: applied.contents,
        target_location: applied.target_location,
        source_start_line: applied.source_start_line,
        tag: target_tag,
        attributes,
    })
}

fn plan_html_attributes_from_direct_location(
    model: &ProjectModel,
    intent: &ProjectHtmlAttributeIntent,
    location: &ProjectSourceEditLocation,
    attributes: BTreeMap<String, Option<String>>,
) -> Result<ProjectHtmlAttributePatch, String> {
    let file = model
        .files
        .iter()
        .find(|file| same_model_path(&file.relative_path, &location.file))
        .ok_or_else(|| format!("Nu am găsit fișierul {} în Project Model.", location.file))?;

    if !is_direct_html_attribute_file(file) {
        return Err(
            "Atributele prin locație directă sunt active doar pentru fișiere HTML 1:1 din proiect, nu pentru template-uri Tera.".to_string(),
        );
    }

    let offset = offset_for_source_location(&file.contents, location)?;
    let tag = parse_html_tag_at(&file.contents, offset)
        .ok_or_else(|| "Locația nu indică începutul unui tag HTML pentru atribute.".to_string())?;
    if tag.is_closing {
        return Err("Locația indică un tag de închidere, nu un element mutabil.".to_string());
    }
    validate_target_tag(intent, &tag.tag)?;
    validate_schema_attributes(&tag.tag, &attributes)?;
    let applied =
        apply_html_attributes(&file.contents, &file.relative_path, tag.start, &attributes)?;
    let resolved_target_id = intent.target_source_id.clone().unwrap_or_else(|| {
        format!(
            "location:{}:{}:{}",
            location.file, location.line, location.column
        )
    });

    Ok(ProjectHtmlAttributePatch {
        file: file.relative_path.clone(),
        resolved_target_id,
        before_revision: file.revision.clone(),
        after_revision: content_revision(&applied.contents),
        contents: applied.contents,
        target_location: applied.target_location,
        source_start_line: applied.source_start_line,
        tag: tag.tag,
        attributes,
    })
}

fn validate_target_tag(
    intent: &ProjectHtmlAttributeIntent,
    actual_tag: &str,
) -> Result<(), String> {
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
        return Err("Elementul <html> nu este editabil vizual pentru atribute.".to_string());
    }
    Ok(())
}

fn apply_html_attributes(
    source: &str,
    file: &str,
    opening_start: usize,
    attributes: &BTreeMap<String, Option<String>>,
) -> Result<AttributeApplication, String> {
    let opening = parse_html_tag_at(source, opening_start)
        .ok_or_else(|| "Range-ul nu mai indică un tag HTML stabil.".to_string())?;
    if opening.is_closing {
        return Err("Range-ul indică un tag HTML de închidere, nu un element mutabil.".to_string());
    }

    let opening_source = source
        .get(opening.start..opening.end)
        .ok_or_else(|| "Nu am putut citi tag-ul HTML de deschidere.".to_string())?;
    let mut updated_opening = opening_source.to_string();
    for (name, value) in attributes {
        updated_opening = match value {
            Some(value) => set_tag_attribute_value(&updated_opening, name, value),
            None => remove_tag_attribute(&updated_opening, name),
        };
    }

    let contents = replace_range(source, opening.start, opening.end, &updated_opening);
    let target_location = source_location_at_offset(source, file, opening.start);
    Ok(AttributeApplication {
        contents,
        source_start_line: target_location.line,
        target_location,
    })
}

fn normalize_attribute_mutations(
    attributes: &[ProjectHtmlAttributeMutation],
) -> Result<BTreeMap<String, Option<String>>, String> {
    if attributes.is_empty() {
        return Err("Nu există atribute de aplicat.".to_string());
    }

    let mut normalized = BTreeMap::new();
    for attribute in attributes {
        let (raw_name, raw_value) = match attribute {
            ProjectHtmlAttributeMutation::SetAttribute { name, value } => {
                (name.as_str(), Some(value.as_str()))
            }
            ProjectHtmlAttributeMutation::RemoveAttribute { name } => (name.as_str(), None),
        };
        let name = raw_name.trim().to_ascii_lowercase();
        if name.is_empty() {
            return Err("Atributul fără nume nu poate fi aplicat.".to_string());
        }
        if !is_valid_attribute_name(&name) {
            return Err(format!("Atributul {name} are nume invalid."));
        }
        if is_protected_attribute(&name) {
            return Err(format!(
                "Atributul intern {name} nu poate fi modificat direct."
            ));
        }

        let value = raw_value.map(validate_attribute_value).transpose()?;
        normalized.insert(name, value);
    }

    if normalized.is_empty() {
        return Err("Nu există atribute valide de aplicat.".to_string());
    }
    Ok(normalized)
}

fn validate_attribute_value(value: &str) -> Result<String, String> {
    if value
        .chars()
        .any(|character| matches!(character, '\n' | '\r' | '\0'))
    {
        return Err("Valorile de atribut nu pot conține linii noi sau caractere nule.".to_string());
    }
    Ok(value.to_string())
}

fn validate_schema_attributes(
    tag: &str,
    attributes: &BTreeMap<String, Option<String>>,
) -> Result<(), String> {
    for (name, value) in attributes {
        validate_visual_attribute_mutation(tag, name, value.as_deref())?;
    }
    Ok(())
}

fn is_valid_attribute_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_' || first == ':') {
        return false;
    }
    chars.all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | ':' | '.')
    })
}

fn is_protected_attribute(name: &str) -> bool {
    // The entire namespace is owned by the editor runtime. A prefix invariant
    // remains safe when new Canvas/Workbench identities are introduced.
    name.starts_with("data-pana-")
}

fn is_direct_html_attribute_file(file: &ProjectModelFile) -> bool {
    matches!(
        file.kind,
        ProjectModelFileKind::StaticText | ProjectModelFileKind::OtherText
    ) && is_html_path(&file.relative_path)
}

fn is_html_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".html") || lower.ends_with(".htm")
}

fn remove_tag_attribute(tag: &str, attr: &str) -> String {
    let mut next = tag.to_string();
    for attribute in find_tag_attributes(&next, attr).into_iter().rev() {
        let remove_start = previous_whitespace_start(&next, attribute.attr_start);
        next = replace_range(&next, remove_start, attribute.attr_end, "");
    }
    next
}

fn set_tag_attribute_value(tag: &str, attr: &str, value: &str) -> String {
    let mut next = tag.to_string();
    let matches = find_tag_attributes(&next, attr);
    for duplicate in matches.iter().skip(1).rev() {
        let remove_start = previous_whitespace_start(&next, duplicate.attr_start);
        next = replace_range(&next, remove_start, duplicate.attr_end, "");
    }

    if let Some(attribute) = find_tag_attributes(&next, attr).into_iter().next() {
        return match attribute.value_style {
            TagAttributeValueStyle::DoubleQuoted => replace_range(
                &next,
                attribute.value_start,
                attribute.value_end,
                &escape_quoted_attr_value(value, '"'),
            ),
            TagAttributeValueStyle::SingleQuoted => replace_range(
                &next,
                attribute.value_start,
                attribute.value_end,
                &escape_quoted_attr_value(value, '\''),
            ),
            TagAttributeValueStyle::Unquoted if is_safe_unquoted_attr_value(value) => {
                replace_range(
                    &next,
                    attribute.value_start,
                    attribute.value_end,
                    &escape_unquoted_attr_value(value),
                )
            }
            TagAttributeValueStyle::Minimized if value.is_empty() => next,
            TagAttributeValueStyle::Unquoted | TagAttributeValueStyle::Minimized => replace_range(
                &next,
                attribute.attr_start,
                attribute.attr_end,
                &format!(r#"{}="{}""#, attr, escape_attr_value(value)),
            ),
        };
    }
    insert_tag_attribute(&next, attr, value)
}

#[derive(Clone, Copy)]
struct TagAttribute {
    attr_start: usize,
    value_start: usize,
    value_end: usize,
    attr_end: usize,
    value_style: TagAttributeValueStyle,
}

#[derive(Clone, Copy)]
enum TagAttributeValueStyle {
    Minimized,
    DoubleQuoted,
    SingleQuoted,
    Unquoted,
}

fn find_tag_attributes(tag: &str, attr: &str) -> Vec<TagAttribute> {
    let attr_lower = attr.to_ascii_lowercase();
    parse_tag_attributes(tag)
        .into_iter()
        .filter(|candidate| {
            tag.get(candidate.name_start..candidate.name_end)
                .is_some_and(|name| name.to_ascii_lowercase() == attr_lower)
        })
        .map(|candidate| TagAttribute {
            attr_start: candidate.attr_start,
            value_start: candidate.value_start,
            value_end: candidate.value_end,
            attr_end: candidate.attr_end,
            value_style: candidate.value_style,
        })
        .collect()
}

#[derive(Clone, Copy)]
struct ParsedTagAttribute {
    attr_start: usize,
    name_start: usize,
    name_end: usize,
    value_start: usize,
    value_end: usize,
    attr_end: usize,
    value_style: TagAttributeValueStyle,
}

fn parse_tag_attributes(tag: &str) -> Vec<ParsedTagAttribute> {
    let mut attributes = Vec::new();
    let mut cursor = tag.find('<').map(|index| index + 1).unwrap_or(0);
    cursor = skip_ascii_whitespace(tag, cursor);
    if char_at(tag, cursor) == Some('/') {
        cursor += 1;
    }
    while let Some(character) = char_at(tag, cursor) {
        if character.is_ascii_whitespace() || character == '>' || character == '/' {
            break;
        }
        cursor += character.len_utf8();
    }

    loop {
        cursor = skip_ascii_whitespace(tag, cursor);
        let Some(character) = char_at(tag, cursor) else {
            break;
        };
        if character == '>' || (character == '/' && char_at(tag, cursor + 1) == Some('>')) {
            break;
        }

        let attr_start = cursor;
        let name_start = cursor;
        while let Some(character) = char_at(tag, cursor) {
            if character.is_ascii_whitespace()
                || matches!(character, '=' | '>' | '/' | '"' | '\'' | '<')
            {
                break;
            }
            cursor += character.len_utf8();
        }
        let name_end = cursor;
        if name_start == name_end {
            cursor += character.len_utf8();
            continue;
        }

        cursor = skip_ascii_whitespace(tag, cursor);
        if char_at(tag, cursor) != Some('=') {
            attributes.push(ParsedTagAttribute {
                attr_start,
                name_start,
                name_end,
                value_start: name_end,
                value_end: name_end,
                attr_end: name_end,
                value_style: TagAttributeValueStyle::Minimized,
            });
            continue;
        }

        cursor += 1;
        cursor = skip_ascii_whitespace(tag, cursor);
        let Some(value_lead) = char_at(tag, cursor) else {
            attributes.push(ParsedTagAttribute {
                attr_start,
                name_start,
                name_end,
                value_start: cursor,
                value_end: cursor,
                attr_end: cursor,
                value_style: TagAttributeValueStyle::Unquoted,
            });
            break;
        };

        if value_lead == '"' || value_lead == '\'' {
            let quote = value_lead;
            cursor += quote.len_utf8();
            let value_start = cursor;
            while let Some(character) = char_at(tag, cursor) {
                if character == quote {
                    break;
                }
                cursor += character.len_utf8();
            }
            let value_end = cursor;
            if char_at(tag, cursor) == Some(quote) {
                cursor += quote.len_utf8();
            }
            attributes.push(ParsedTagAttribute {
                attr_start,
                name_start,
                name_end,
                value_start,
                value_end,
                attr_end: cursor,
                value_style: if quote == '"' {
                    TagAttributeValueStyle::DoubleQuoted
                } else {
                    TagAttributeValueStyle::SingleQuoted
                },
            });
            continue;
        }

        let value_start = cursor;
        while let Some(character) = char_at(tag, cursor) {
            if character.is_ascii_whitespace()
                || character == '>'
                || (character == '/' && char_at(tag, cursor + 1) == Some('>'))
            {
                break;
            }
            cursor += character.len_utf8();
        }
        attributes.push(ParsedTagAttribute {
            attr_start,
            name_start,
            name_end,
            value_start,
            value_end: cursor,
            attr_end: cursor,
            value_style: TagAttributeValueStyle::Unquoted,
        });
    }

    attributes
}

fn char_at(source: &str, cursor: usize) -> Option<char> {
    source.get(cursor..)?.chars().next()
}

fn previous_whitespace_start(source: &str, index: usize) -> usize {
    let mut cursor = index;
    while cursor > 0 {
        let Some((previous_index, character)) = source[..cursor].char_indices().next_back() else {
            break;
        };
        if !character.is_ascii_whitespace() || character == '\n' || character == '\r' {
            break;
        }
        cursor = previous_index;
    }
    cursor
}

fn insert_tag_attribute(tag: &str, attr: &str, value: &str) -> String {
    let insert_at = tag
        .rfind("/>")
        .or_else(|| tag.rfind('>'))
        .unwrap_or(tag.len());
    format!(
        "{} {}=\"{}\"{}",
        &tag[..insert_at],
        attr,
        escape_attr_value(value),
        &tag[insert_at..]
    )
}

fn replace_range(source: &str, start: usize, end: usize, replacement: &str) -> String {
    let mut next = String::with_capacity(source.len() - (end - start) + replacement.len());
    next.push_str(&source[..start]);
    next.push_str(replacement);
    next.push_str(&source[end..]);
    next
}

fn escape_attr_value(value: &str) -> String {
    escape_quoted_attr_value(value, '"')
}

fn escape_quoted_attr_value(value: &str, quote: char) -> String {
    let escaped = value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    if quote == '\'' {
        escaped.replace('\'', "&#39;")
    } else {
        escaped.replace('"', "&quot;")
    }
}

fn is_safe_unquoted_attr_value(value: &str) -> bool {
    !value.is_empty()
        && !value.chars().any(|character| {
            character.is_ascii_whitespace()
                || matches!(character, '"' | '\'' | '`' | '=' | '<' | '>')
        })
}

fn escape_unquoted_attr_value(value: &str) -> String {
    value.replace('&', "&amp;")
}

fn skip_ascii_whitespace(source: &str, mut cursor: usize) -> usize {
    while let Some(character) = source[cursor..].chars().next() {
        if !character.is_ascii_whitespace() {
            break;
        }
        cursor += character.len_utf8();
    }
    cursor
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
    fn plan_html_attributes_updates_template_anchor() {
        let root = unique_test_dir();
        write_project(
            &root,
            concat!(
                "{% block content %}\n",
                "<section class=\"hero\" title=\"Old\">\n",
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

        let plan = plan_html_attributes(
            &model,
            &ProjectHtmlAttributeIntent {
                target_source_id: Some(section.id.clone()),
                target_location: None,
                target_tag: Some("section".to_string()),
                target_selector: Some(".hero".to_string()),
                attributes: vec![
                    ProjectHtmlAttributeMutation::set("class", "hero hero--mare"),
                    ProjectHtmlAttributeMutation::remove("title"),
                    ProjectHtmlAttributeMutation::set("data-anim", "ps-hero-abc123"),
                ],
            },
            &HashMap::new(),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.unwrap();
        assert!(patch
            .contents
            .contains(r#"<section class="hero hero--mare" data-anim="ps-hero-abc123">"#));
        assert!(!patch.contents.contains("title="));
        assert_eq!(patch.tag, "section");
        assert_eq!(patch.source_start_line, 2);
    }

    #[test]
    fn plan_html_attributes_resolves_active_html_by_direct_location() {
        let root = unique_test_dir();
        write_project(&root, "<main></main>\n");
        fs::create_dir_all(root.join("sursa/static")).unwrap();
        fs::write(
            root.join("sursa/static/plain.html"),
            concat!(
                "<!DOCTYPE html>\n",
                "<html>\n",
                "<body>\n",
                "  <img class=\"photo\" src=\"old.jpg\" alt=\"Old\">\n",
                "</body>\n",
                "</html>\n",
            ),
        )
        .unwrap();
        let model = build_project_model(&root, &HashMap::new()).unwrap();

        let plan = plan_html_attributes(
            &model,
            &ProjectHtmlAttributeIntent {
                target_source_id: None,
                target_location: Some(ProjectSourceEditLocation {
                    file: "sursa/static/plain.html".to_string(),
                    line: 4,
                    column: 3,
                }),
                target_tag: Some("img".to_string()),
                target_selector: Some("body:nth-of-type(1) > img:nth-of-type(1)".to_string()),
                attributes: vec![
                    ProjectHtmlAttributeMutation::set("src", "nou.jpg"),
                    ProjectHtmlAttributeMutation::set("alt", "Imagine nouă"),
                ],
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
        assert!(patch.contents.contains(r#"src="nou.jpg""#));
        assert!(patch.contents.contains(r#"alt="Imagine nouă""#));
        assert_eq!(patch.source_start_line, 4);
    }

    #[test]
    fn visual_attribute_schema_blocks_active_and_semantically_invalid_values() {
        let active = BTreeMap::from([("onclick".to_string(), Some("alert(1)".to_string()))]);
        assert!(validate_schema_attributes("button", &active).is_err());

        let aria = BTreeMap::from([("aria-hidden".to_string(), Some("yes".to_string()))]);
        assert!(validate_schema_attributes("button", &aria).is_err());

        let direction = BTreeMap::from([("dir".to_string(), Some("sideways".to_string()))]);
        assert!(validate_schema_attributes("div", &direction).is_err());
    }

    #[test]
    fn visual_attribute_schema_distinguishes_source_only_and_meaningful_empty_values() {
        let source_only = BTreeMap::from([
            ("target".to_string(), Some("_blank".to_string())),
            ("download".to_string(), Some(String::new())),
        ]);
        assert!(validate_schema_attributes("a", &source_only).is_ok());

        let meaningful_empty = BTreeMap::from([
            ("href".to_string(), Some(String::new())),
            ("aria-label".to_string(), Some(String::new())),
            ("data-state".to_string(), Some(String::new())),
        ]);
        assert!(validate_schema_attributes("a", &meaningful_empty).is_ok());

        let empty_enumerated = BTreeMap::from([("dir".to_string(), Some(String::new()))]);
        assert!(validate_schema_attributes("div", &empty_enumerated).is_err());
    }

    #[test]
    fn every_pana_runtime_attribute_is_protected() {
        assert!(is_protected_attribute("data-pana-source-id"));
        assert!(is_protected_attribute("data-pana-render-instance-id"));
        assert!(is_protected_attribute(
            "data-pana-workbench-active-template"
        ));
        assert!(!is_protected_attribute("data-anim"));
        assert!(!is_protected_attribute("data-component"));
    }

    #[test]
    fn empty_attribute_values_remain_explicit_set_operations() {
        let normalized =
            normalize_attribute_mutations(&[ProjectHtmlAttributeMutation::set("alt", "")]).unwrap();

        assert_eq!(normalized.get("alt"), Some(&Some(String::new())));
        assert_eq!(
            set_tag_attribute_value(r#"<img alt="decorativ">"#, "alt", ""),
            r#"<img alt="">"#
        );
        assert_eq!(
            set_tag_attribute_value("<input disabled>", "disabled", ""),
            "<input disabled>"
        );
    }

    #[test]
    fn attribute_rewriter_handles_minimized_and_unquoted_attributes_without_duplicates() {
        assert_eq!(
            remove_tag_attribute("<input disabled>", "disabled"),
            "<input>"
        );
        assert_eq!(
            set_tag_attribute_value("<div id=hero class=card>", "id", "principal"),
            "<div id=principal class=card>",
        );
        assert_eq!(
            set_tag_attribute_value(
                r#"<input disabled disabled="disabled">"#,
                "disabled",
                "disabled"
            ),
            r#"<input disabled="disabled">"#,
        );
    }

    #[test]
    fn attribute_rewriter_preserves_single_and_double_quoted_syntax() {
        assert_eq!(
            set_tag_attribute_value("<div title='vechi'>", "title", "nou 'sigur'"),
            "<div title='nou &#39;sigur&#39;'>",
        );
        assert_eq!(
            set_tag_attribute_value(r#"<div title="vechi">"#, "title", "nou \"sigur\""),
            r#"<div title="nou &quot;sigur&quot;">"#,
        );
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
            "pana-studio-attribute-engine-{}-{stamp}",
            std::process::id()
        ))
    }
}
