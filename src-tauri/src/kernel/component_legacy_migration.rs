use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Deserialize;

use crate::kernel::{
    component_mutation::validate_component_workspace_candidate,
    project_path::normalize_project_relative_path,
    project_workspace::{
        ProjectWorkspace, ProjectWorkspaceIdentity, ProjectWorkspaceMutationReceipt,
        WorkspaceMutationMetadata, WorkspaceResourceDelete, WorkspaceResourceMutation,
    },
};

const LEGACY_LOOP_CATALOG_PATH: &str = "data/pana-studio/loops.json";

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyLoopDefinition {
    #[serde(default)]
    id: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    source_kind: String,
    #[serde(default)]
    alias: String,
    #[serde(default)]
    layout: String,
    section_path: Option<String>,
    extra_key: Option<String>,
    data_path: Option<String>,
    collection_key: Option<String>,
    custom_collection: Option<String>,
    #[serde(default)]
    title_expression: String,
    description_expression: Option<String>,
    url_expression: Option<String>,
}

pub(crate) fn migrate_legacy_component_catalog(
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    now_ms: u128,
) -> Result<Option<ProjectWorkspaceMutationReceipt>, String> {
    let Some(source) = workspace.documents.text_for(LEGACY_LOOP_CATALOG_PATH) else {
        return Ok(None);
    };
    let definitions = parse_legacy_loop_catalog(&source)?;
    let mut writes = BTreeMap::<String, WorkspaceResourceMutation>::new();
    let mut deletes = BTreeSet::<String>::from([LEGACY_LOOP_CATALOG_PATH.to_string()]);

    for (index, definition) in definitions.iter().enumerate() {
        migrate_referenced_data_file(workspace, definition, &mut writes, &mut deletes)?;
        let base_name = legacy_partial_name(definition, index);
        if let Some((destination, contents)) =
            available_legacy_partial_destination(workspace, &writes, definition, &base_name)?
        {
            writes.insert(
                destination.clone(),
                WorkspaceResourceMutation {
                    relative_path: destination,
                    contents,
                    create_only: true,
                },
            );
        }
    }

    let writes = writes.into_values().collect::<Vec<_>>();
    let deletes = deletes
        .into_iter()
        .map(|relative_path| WorkspaceResourceDelete { relative_path })
        .collect::<Vec<_>>();
    let metadata = WorkspaceMutationMetadata {
        label: "Migrare catalog legacy de liste Tera".to_string(),
        source: "components.legacy_loop_migration".to_string(),
        coalesce_key: None,
        transaction_id: None,
    };

    let mut candidate = workspace.clone();
    let candidate_identity = current_identity(&candidate);
    candidate.stage_composite_changes(
        &candidate_identity,
        metadata.clone(),
        writes.clone(),
        deletes.clone(),
        None,
        now_ms,
    )?;
    validate_component_workspace_candidate(project_root, &candidate)?;

    let identity = current_identity(workspace);
    workspace
        .stage_composite_changes(&identity, metadata, writes, deletes, None, now_ms)
        .map(Some)
}

fn parse_legacy_loop_catalog(source: &str) -> Result<Vec<LegacyLoopDefinition>, String> {
    let value = serde_json::from_str::<serde_json::Value>(source).map_err(|error| {
        format!(
            "Catalogul legacy {LEGACY_LOOP_CATALOG_PATH} este JSON invalid și nu poate fi migrat sigur: {error}"
        )
    })?;
    let definitions_value = match value {
        serde_json::Value::Array(definitions) => serde_json::Value::Array(definitions),
        serde_json::Value::Object(mut object) => object
            .remove("definitions")
            .unwrap_or(serde_json::Value::Array(Vec::new())),
        _ => {
            return Err(format!(
                "Catalogul legacy {LEGACY_LOOP_CATALOG_PATH} nu are forma obiect/array cunoscută."
            ));
        }
    };
    serde_json::from_value::<Vec<LegacyLoopDefinition>>(definitions_value).map_err(|error| {
        format!(
            "Definițiile din {LEGACY_LOOP_CATALOG_PATH} nu respectă schema legacy cunoscută: {error}"
        )
    })
}

fn available_legacy_partial_destination(
    workspace: &ProjectWorkspace,
    writes: &BTreeMap<String, WorkspaceResourceMutation>,
    definition: &LegacyLoopDefinition,
    base_name: &str,
) -> Result<Option<(String, String)>, String> {
    for suffix in 1..=10_000usize {
        let logical_name = if suffix == 1 {
            base_name.to_string()
        } else {
            format!("{base_name}-{suffix}")
        };
        let destination = normalize_project_relative_path(&format!(
            "templates/partials/migrat/{logical_name}.html"
        ))?;
        let contents = legacy_loop_partial(definition, &logical_name)?;
        if let Some(existing) = workspace.documents.text_for(&destination) {
            if existing == contents {
                return Ok(None);
            }
            continue;
        }
        if !writes.contains_key(&destination) {
            return Ok(Some((destination, contents)));
        }
    }
    Err(format!(
        "Migrarea listei legacy {} nu a găsit un nume de parțială liber.",
        definition.label
    ))
}

fn migrate_referenced_data_file(
    workspace: &ProjectWorkspace,
    definition: &LegacyLoopDefinition,
    writes: &mut BTreeMap<String, WorkspaceResourceMutation>,
    deletes: &mut BTreeSet<String>,
) -> Result<(), String> {
    if definition.source_kind != "dataFile" {
        return Ok(());
    }
    let source = normalized_legacy_data_path(definition.data_path.as_deref().unwrap_or(""))?;
    let Some(old_path) = source.old_path else {
        return Ok(());
    };
    let Some(old_contents) = workspace.documents.text_for(&old_path) else {
        return Ok(());
    };
    if let Some(existing) = workspace.documents.text_for(&source.canonical_path) {
        if existing != old_contents {
            return Err(format!(
                "Migrarea a găsit conținut diferit în {} și {}; nu suprascrie aproximativ niciunul.",
                old_path, source.canonical_path
            ));
        }
    } else if let Some(existing) = writes.get(&source.canonical_path) {
        if existing.contents != old_contents {
            return Err(format!(
                "Două fișiere legacy ar produce conținut diferit în {}.",
                source.canonical_path
            ));
        }
    } else {
        writes.insert(
            source.canonical_path.clone(),
            WorkspaceResourceMutation {
                relative_path: source.canonical_path,
                contents: old_contents,
                create_only: true,
            },
        );
    }
    deletes.insert(old_path);
    Ok(())
}

struct NormalizedLegacyDataPath {
    canonical_path: String,
    old_path: Option<String>,
}

fn normalized_legacy_data_path(path: &str) -> Result<NormalizedLegacyDataPath, String> {
    let value = path.trim().replace('\\', "/");
    let value = if value.is_empty() {
        "data/items.toml".to_string()
    } else {
        value
    };
    let normalized = normalize_project_relative_path(&value)?;
    if !normalized.ends_with(".toml") {
        return Err(format!(
            "Migrarea listelor acceptă numai surse TOML, nu {normalized}."
        ));
    }
    if let Some(rest) = normalized.strip_prefix("data/") {
        Ok(NormalizedLegacyDataPath {
            canonical_path: normalize_project_relative_path(&format!("date/{rest}"))?,
            old_path: Some(normalized),
        })
    } else if normalized.starts_with("date/") {
        Ok(NormalizedLegacyDataPath {
            canonical_path: normalized,
            old_path: None,
        })
    } else {
        Err(format!(
            "Sursa legacy {normalized} nu aparține rădăcinilor TOML data/ sau date/."
        ))
    }
}

fn legacy_loop_partial(
    definition: &LegacyLoopDefinition,
    logical_name: &str,
) -> Result<String, String> {
    let alias = safe_identifier(&definition.alias, "item");
    let source_kind = definition.source_kind.as_str();
    let section_path = escape_tera_string(
        definition
            .section_path
            .as_deref()
            .unwrap_or("_index.md")
            .trim(),
    );
    let extra_key = safe_identifier(definition.extra_key.as_deref().unwrap_or("items"), "items");
    let collection_key = safe_identifier(
        definition.collection_key.as_deref().unwrap_or("items"),
        "items",
    );
    let mut setup = Vec::new();
    let collection = match source_kind {
        "sectionPages" => {
            setup.push(format!(
                "{{% set loop_section = get_section(path=\"{section_path}\") %}}"
            ));
            "loop_section.pages".to_string()
        }
        "sectionExtra" => {
            setup.push(format!(
                "{{% set loop_section = get_section(path=\"{section_path}\") %}}"
            ));
            format!("loop_section.extra.{extra_key} | default(value=[])")
        }
        "configExtra" => format!("config.extra.{extra_key} | default(value=[])"),
        "dataFile" => {
            let data_path = normalized_legacy_data_path(
                definition.data_path.as_deref().unwrap_or("data/items.toml"),
            )?
            .canonical_path;
            setup.push(format!(
                "{{% set loop_data = load_data(path=\"{}\") %}}",
                escape_tera_string(&data_path)
            ));
            format!("loop_data.{collection_key} | default(value=[])")
        }
        "custom" | "" => clean_expression(
            definition.custom_collection.as_deref().unwrap_or("items"),
            "items",
        ),
        other => {
            return Err(format!(
                "Lista legacy {} folosește sourceKind necunoscut: {other}.",
                definition.label
            ));
        }
    };
    let title = output_expression(&definition.title_expression, &definition.label);
    let description = output_expression(
        definition.description_expression.as_deref().unwrap_or(""),
        "",
    );
    let url = output_expression(definition.url_expression.as_deref().unwrap_or(""), "");
    let body = match definition.layout.as_str() {
        "linkList" => format!(
            "{{% set loop_item_title = {title} %}}\n{{% set loop_item_description = {description} %}}\n{{% set loop_item_url = {url} %}}\n<a class=\"pana-loop-link\" href=\"{{% if loop_item_url %}}{{{{ loop_item_url | safe }}}}{{% else %}}#{{% endif %}}\">{{{{ loop_item_title }}}}</a>"
        ),
        "plainList" => format!(
            "{{% set loop_item_title = {title} %}}\n{{% set loop_item_description = {description} %}}\n{{% set loop_item_url = {url} %}}\n<p class=\"pana-loop-item\">{{{{ loop_item_title }}}}</p>"
        ),
        "cardGrid" | "" => format!(
            "{{% set loop_item_title = {title} %}}\n{{% set loop_item_description = {description} %}}\n{{% set loop_item_url = {url} %}}\n<article class=\"pana-loop-card\">\n  <h3>{{% if loop_item_url %}}<a href=\"{{{{ loop_item_url | safe }}}}\">{{{{ loop_item_title }}}}</a>{{% else %}}{{{{ loop_item_title }}}}{{% endif %}}</h3>\n  {{% if loop_item_description %}}<p>{{{{ loop_item_description }}}}</p>{{% endif %}}\n</article>"
        ),
        other => {
            return Err(format!(
                "Lista legacy {} folosește layout necunoscut: {other}.",
                definition.label
            ));
        }
    };
    let setup = if setup.is_empty() {
        String::new()
    } else {
        format!("{}\n", setup.join("\n"))
    };
    let indented_body = body
        .lines()
        .map(|line| format!("      {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(format!(
        "{{# Migrat lossless din {LEGACY_LOOP_CATALOG_PATH}; definiție: {} #}}\n{setup}<section class=\"pana-loop pana-loop-{logical_name}\">\n  <div class=\"pana-loop-{logical_name}__items\">\n    {{% for {alias} in {collection} %}}\n{indented_body}\n    {{% endfor %}}\n  </div>\n</section>\n",
        escape_tera_string(&definition.id)
    ))
}

fn legacy_partial_name(definition: &LegacyLoopDefinition, index: usize) -> String {
    let source = if definition.id.trim().is_empty() {
        definition.label.as_str()
    } else {
        definition.id.as_str()
    };
    let slug = source
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        format!("lista-{}", index + 1)
    } else {
        slug
    }
}

fn safe_identifier(value: &str, fallback: &str) -> String {
    let mut identifier = value
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    while identifier
        .chars()
        .next()
        .is_some_and(|character| !character.is_ascii_alphabetic() && character != '_')
    {
        identifier.remove(0);
    }
    if identifier.is_empty() {
        fallback.to_string()
    } else {
        identifier
    }
}

fn clean_expression(value: &str, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}

fn output_expression(expression: &str, fallback: &str) -> String {
    let expression = expression.trim();
    if expression.is_empty() {
        format!("\"{}\"", escape_tera_string(fallback))
    } else {
        format!(
            "{} | default(value=\"{}\")",
            expression,
            escape_tera_string(fallback)
        )
    }
}

fn escape_tera_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn current_identity(workspace: &ProjectWorkspace) -> ProjectWorkspaceIdentity {
    ProjectWorkspaceIdentity {
        expected_project_root: workspace.session.project_root.clone(),
        expected_session_id: workspace.runtime_session_id(),
        expected_revision: workspace.revision,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_both_legacy_envelope_and_array() {
        let envelope = r#"{"schemaVersion":1,"definitions":[{"id":"loop-servicii","label":"Servicii","sourceKind":"custom","alias":"item","layout":"plainList","customCollection":"items","titleExpression":"item.title"}]}"#;
        let array = r#"[{"id":"loop-servicii","label":"Servicii","sourceKind":"custom","alias":"item","layout":"plainList","customCollection":"items","titleExpression":"item.title"}]"#;
        assert_eq!(parse_legacy_loop_catalog(envelope).unwrap().len(), 1);
        assert_eq!(parse_legacy_loop_catalog(array).unwrap().len(), 1);
    }

    #[test]
    fn rewrites_legacy_data_root_and_generates_real_tera() {
        let definition = LegacyLoopDefinition {
            id: "loop-servicii".to_string(),
            label: "Servicii".to_string(),
            source_kind: "dataFile".to_string(),
            alias: "service".to_string(),
            layout: "cardGrid".to_string(),
            section_path: None,
            extra_key: None,
            data_path: Some("data/services.toml".to_string()),
            collection_key: Some("services".to_string()),
            custom_collection: None,
            title_expression: "service.title".to_string(),
            description_expression: Some("service.description".to_string()),
            url_expression: Some("service.url".to_string()),
        };
        let source = legacy_loop_partial(&definition, "servicii").unwrap();
        assert!(source.contains("load_data(path=\"date/services.toml\")"));
        assert!(source.contains("{% for service in loop_data.services"));
        assert!(!source.contains("data/services.toml"));
        assert!(
            crate::source_graph::tera_cst::parse_tera_cst(&source, "migrat.html").is_valid_tera()
        );
    }
}
