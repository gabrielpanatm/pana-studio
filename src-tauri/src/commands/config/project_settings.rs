use serde::{Deserialize, Serialize};

use super::model::{ProjectSettingsSnapshot, PROJECT_SETTINGS_SCHEMA_VERSION};

pub const PROJECT_SETTINGS_PATH: &str = ".panastudio/settings.toml";

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProjectAssetsSettingsDocument {
    #[serde(default)]
    cachebust: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProjectSettingsDocument {
    schema_version: u32,
    #[serde(default)]
    assets: ProjectAssetsSettingsDocument,
}

impl Default for ProjectSettingsDocument {
    fn default() -> Self {
        Self {
            schema_version: PROJECT_SETTINGS_SCHEMA_VERSION,
            assets: ProjectAssetsSettingsDocument::default(),
        }
    }
}

pub fn parse_project_settings_source(
    source: Option<&str>,
    workspace_revision: u64,
) -> Result<ProjectSettingsSnapshot, String> {
    let document = match source {
        Some(source) if !source.trim().is_empty() => toml_edit::de::from_str::<
            ProjectSettingsDocument,
        >(source)
        .map_err(|error| format!("Configurația {PROJECT_SETTINGS_PATH} este invalidă: {error}"))?,
        _ => ProjectSettingsDocument::default(),
    };
    if document.schema_version != PROJECT_SETTINGS_SCHEMA_VERSION {
        return Err(format!(
            "Schema {} este {}, așteptat {}.",
            PROJECT_SETTINGS_PATH, document.schema_version, PROJECT_SETTINGS_SCHEMA_VERSION
        ));
    }
    Ok(ProjectSettingsSnapshot {
        schema_version: document.schema_version,
        workspace_revision,
        cachebust_assets: document.assets.cachebust,
    })
}

pub fn serialize_project_settings(cachebust_assets: bool) -> Result<String, String> {
    let document = ProjectSettingsDocument {
        schema_version: PROJECT_SETTINGS_SCHEMA_VERSION,
        assets: ProjectAssetsSettingsDocument {
            cachebust: cachebust_assets,
        },
    };
    let mut source = toml_edit::ser::to_string_pretty(&document)
        .map_err(|error| format!("Configurația Pană nu poate fi serializată: {error}"))?;
    if !source.ends_with('\n') {
        source.push('\n');
    }
    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_document_has_versioned_defaults() {
        let settings = parse_project_settings_source(None, 17).unwrap();
        assert_eq!(settings.schema_version, PROJECT_SETTINGS_SCHEMA_VERSION);
        assert_eq!(settings.workspace_revision, 17);
        assert!(!settings.cachebust_assets);
    }

    #[test]
    fn roundtrip_uses_typed_nested_toml_without_runtime_revision() {
        let source = serialize_project_settings(true).unwrap();
        assert!(source.contains("schema_version = 1"));
        assert!(source.contains("[assets]"));
        assert!(source.contains("cachebust = true"));
        assert!(!source.contains("workspace_revision"));
        let settings = parse_project_settings_source(Some(&source), 29).unwrap();
        assert_eq!(settings.workspace_revision, 29);
        assert!(settings.cachebust_assets);
    }

    #[test]
    fn rejects_unknown_or_future_contracts() {
        assert!(parse_project_settings_source(Some("schema_version = 2\n"), 0).is_err());
        assert!(
            parse_project_settings_source(Some("schema_version = 1\nunknown = true\n"), 0).is_err()
        );
    }
}
