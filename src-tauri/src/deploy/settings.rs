use serde::{Deserialize, Serialize};

use crate::kernel::file_buffer_store::FileBufferStore;

use super::model::{DeploySettings, DeployTarget, DEPLOY_SETTINGS_SCHEMA_VERSION};

pub const DEPLOY_SETTINGS_PATH: &str = ".panastudio/deploy.toml";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DeploySettingsDocument {
    schema_version: u32,
    #[serde(default)]
    active_target_id: Option<String>,
    #[serde(default)]
    targets: Vec<DeployTarget>,
}

impl Default for DeploySettingsDocument {
    fn default() -> Self {
        Self {
            schema_version: DEPLOY_SETTINGS_SCHEMA_VERSION,
            active_target_id: None,
            targets: Vec::new(),
        }
    }
}

pub fn read_deploy_settings_from_store(
    store: &FileBufferStore,
    workspace_revision: u64,
) -> Result<DeploySettings, String> {
    parse_deploy_settings_source(
        store.text_for(DEPLOY_SETTINGS_PATH).as_deref(),
        workspace_revision,
    )
}

pub fn parse_deploy_settings_source(
    source: Option<&str>,
    workspace_revision: u64,
) -> Result<DeploySettings, String> {
    let document = match source {
        Some(source) if !source.trim().is_empty() => toml_edit::de::from_str::<
            DeploySettingsDocument,
        >(source)
        .map_err(|error| format!("Configurația {DEPLOY_SETTINGS_PATH} este invalidă: {error}"))?,
        _ => DeploySettingsDocument::default(),
    };
    let settings = DeploySettings {
        schema_version: document.schema_version,
        revision: workspace_revision,
        active_target_id: document.active_target_id,
        targets: document.targets,
    };
    settings.validate()?;
    Ok(settings)
}

pub fn serialize_deploy_settings(settings: &DeploySettings) -> Result<String, String> {
    settings.validate()?;
    let document = DeploySettingsDocument {
        schema_version: settings.schema_version,
        active_target_id: settings.active_target_id.clone(),
        targets: settings.targets.clone(),
    };
    let mut source = toml_edit::ser::to_string_pretty(&document)
        .map_err(|error| format!("Configurația deploy nu poate fi serializată: {error}"))?;
    if !source.ends_with('\n') {
        source.push('\n');
    }
    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deploy::model::{BunnyTargetConfig, DeployCleanupPolicy, DeployTargetProvider};

    fn settings(revision: u64) -> DeploySettings {
        DeploySettings {
            schema_version: DEPLOY_SETTINGS_SCHEMA_VERSION,
            revision,
            active_target_id: Some("production".to_string()),
            targets: vec![DeployTarget {
                id: "production".to_string(),
                name: "Production".to_string(),
                credential_env_prefix: "PANA_DEPLOY_PRODUCTION".to_string(),
                cleanup_policy: DeployCleanupPolicy::ManagedOnly,
                provider: DeployTargetProvider::Bunny(BunnyTargetConfig {
                    storage_zone: "site".to_string(),
                    storage_region: "de".to_string(),
                    pull_zone_id: "42".to_string(),
                    remote_prefix: String::new(),
                }),
            }],
        }
    }

    #[test]
    fn roundtrip_does_not_persist_runtime_revision_or_secrets() {
        let source = serialize_deploy_settings(&settings(91)).unwrap();
        assert!(source.contains("schema_version = 1"));
        assert!(source.contains("credentialEnvPrefix = \"PANA_DEPLOY_PRODUCTION\""));
        assert!(!source.contains("revision"));
        assert!(!source.to_ascii_lowercase().contains("secret"));

        let parsed = parse_deploy_settings_source(Some(&source), 12).unwrap();
        assert_eq!(parsed.revision, 12);
        assert_eq!(parsed.targets, settings(91).targets);
    }

    #[test]
    fn missing_document_is_empty_and_versioned() {
        assert_eq!(
            parse_deploy_settings_source(None, 7).unwrap(),
            DeploySettings {
                revision: 7,
                ..DeploySettings::default()
            }
        );
    }
}
