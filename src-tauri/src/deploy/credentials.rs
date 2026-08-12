use std::{collections::BTreeMap, fs, path::Path};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};

use crate::{
    app_home::{project_deploy_secrets_path, projects_config_dir},
    kernel::write_authority::{
        WriteAuthority, WriteCategory, WriteIntent, WriteOperationKind, WriteOwner, WritePolicy,
        WriteTarget,
    },
};

use super::{
    env::{env_require, read_env_from_root},
    model::{
        BunnyTargetConfig, DeployCleanupPolicy, DeployProviderKind, DeploySettings, DeployTarget,
        DeployTargetProvider,
    },
};

pub const DEPLOY_CREDENTIAL_STATUS_SCHEMA_VERSION: u32 = 1;
pub const DEPLOY_CONFIGURATION_SCHEMA_VERSION: u32 = 1;
const DEPLOY_SECRET_STORE_SCHEMA_VERSION: u32 = 1;
const MAX_SECRET_BYTES: usize = 1024 * 1024;
pub(crate) const LEGACY_BUNNY_CREDENTIAL_REF: &str = "legacy-bunny-env";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeployCredentialKind {
    Bunny,
    Ftp,
    SftpPassword,
    SftpPrivateKey,
    S3,
    CloudflarePages,
}

impl DeployCredentialKind {
    fn supports_provider(self, provider: DeployProviderKind) -> bool {
        matches!(
            (self, provider),
            (Self::Bunny, DeployProviderKind::Bunny)
                | (Self::Ftp, DeployProviderKind::Ftp)
                | (
                    Self::SftpPassword | Self::SftpPrivateKey,
                    DeployProviderKind::Sftp
                )
                | (Self::S3, DeployProviderKind::S3)
                | (Self::CloudflarePages, DeployProviderKind::CloudflarePages)
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployCredentialStatus {
    pub schema_version: u32,
    pub credential_ref: String,
    pub kind: DeployCredentialKind,
    pub configured: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployTargetCapabilitySnapshot {
    pub target_id: String,
    pub provider: DeployProviderKind,
    pub capabilities: super::model::ProviderCapabilities,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployConfigurationSnapshot {
    pub schema_version: u32,
    pub settings: DeploySettings,
    pub credential_statuses: Vec<DeployCredentialStatus>,
    pub target_capabilities: Vec<DeployTargetCapabilitySnapshot>,
    pub legacy_bunny_fallback: bool,
}

pub fn configuration_snapshot<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &Path,
    settings: DeploySettings,
) -> Result<DeployConfigurationSnapshot, String> {
    settings.validate()?;
    let credential_statuses = credential_statuses(app, project_root, &settings.targets)?;
    let target_capabilities = settings
        .targets
        .iter()
        .map(|target| DeployTargetCapabilitySnapshot {
            target_id: target.id.clone(),
            provider: target.provider_kind(),
            capabilities: target.capabilities(),
        })
        .collect();
    let legacy_bunny_fallback = settings
        .targets
        .iter()
        .any(|target| target.credential_ref == LEGACY_BUNNY_CREDENTIAL_REF);
    Ok(DeployConfigurationSnapshot {
        schema_version: DEPLOY_CONFIGURATION_SCHEMA_VERSION,
        settings,
        credential_statuses,
        target_capabilities,
        legacy_bunny_fallback,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployCredentialWriteInput {
    pub credential_ref: String,
    #[serde(flatten)]
    pub secret: DeployCredentialSecretInput,
}

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum DeployCredentialSecretInput {
    Bunny {
        storage_key: String,
        cdn_api_key: String,
    },
    Ftp {
        username: String,
        password: String,
    },
    SftpPassword {
        username: String,
        password: String,
    },
    SftpPrivateKey {
        username: String,
        private_key_pem: String,
        #[serde(default)]
        passphrase: Option<String>,
    },
    S3 {
        access_key_id: String,
        secret_access_key: String,
        #[serde(default)]
        session_token: Option<String>,
    },
    CloudflarePages {
        api_token: String,
    },
}

impl DeployCredentialSecretInput {
    fn validate(&self) -> Result<(), String> {
        match self {
            Self::Bunny {
                storage_key,
                cdn_api_key,
            } => {
                validate_secret("Bunny Storage key", storage_key, 16 * 1024)?;
                validate_secret("Bunny CDN API key", cdn_api_key, 16 * 1024)
            }
            Self::Ftp { username, password } | Self::SftpPassword { username, password } => {
                validate_username(username)?;
                validate_secret("Parola", password, 64 * 1024)
            }
            Self::SftpPrivateKey {
                username,
                private_key_pem,
                passphrase,
            } => {
                validate_username(username)?;
                validate_secret("Cheia privată SFTP", private_key_pem, MAX_SECRET_BYTES)?;
                if !private_key_pem.contains("-----BEGIN") {
                    return Err(
                        "Cheia privată SFTP nu are un header PEM/OpenSSH valid.".to_string()
                    );
                }
                if let Some(passphrase) = passphrase {
                    validate_optional_secret("Passphrase-ul SFTP", passphrase, 64 * 1024)?;
                }
                Ok(())
            }
            Self::S3 {
                access_key_id,
                secret_access_key,
                session_token,
            } => {
                validate_secret("Access key ID S3", access_key_id, 16 * 1024)?;
                validate_secret("Secret access key S3", secret_access_key, 64 * 1024)?;
                if let Some(session_token) = session_token {
                    validate_optional_secret("Session token S3", session_token, 256 * 1024)?;
                }
                Ok(())
            }
            Self::CloudflarePages { api_token } => {
                validate_secret("Token-ul Cloudflare API", api_token, 64 * 1024)
            }
        }
    }

    fn into_stored(self) -> StoredDeployCredential {
        match self {
            Self::Bunny {
                storage_key,
                cdn_api_key,
            } => StoredDeployCredential::Bunny {
                storage_key,
                cdn_api_key,
            },
            Self::Ftp { username, password } => StoredDeployCredential::Ftp { username, password },
            Self::SftpPassword { username, password } => {
                StoredDeployCredential::SftpPassword { username, password }
            }
            Self::SftpPrivateKey {
                username,
                private_key_pem,
                passphrase,
            } => StoredDeployCredential::SftpPrivateKey {
                username,
                private_key_pem,
                passphrase,
            },
            Self::S3 {
                access_key_id,
                secret_access_key,
                session_token,
            } => StoredDeployCredential::S3 {
                access_key_id,
                secret_access_key,
                session_token,
            },
            Self::CloudflarePages { api_token } => {
                StoredDeployCredential::CloudflarePages { api_token }
            }
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum StoredDeployCredential {
    Bunny {
        storage_key: String,
        cdn_api_key: String,
    },
    Ftp {
        username: String,
        password: String,
    },
    SftpPassword {
        username: String,
        password: String,
    },
    SftpPrivateKey {
        username: String,
        private_key_pem: String,
        #[serde(default)]
        passphrase: Option<String>,
    },
    S3 {
        access_key_id: String,
        secret_access_key: String,
        #[serde(default)]
        session_token: Option<String>,
    },
    CloudflarePages {
        api_token: String,
    },
}

impl StoredDeployCredential {
    pub(crate) fn kind(&self) -> DeployCredentialKind {
        match self {
            Self::Bunny { .. } => DeployCredentialKind::Bunny,
            Self::Ftp { .. } => DeployCredentialKind::Ftp,
            Self::SftpPassword { .. } => DeployCredentialKind::SftpPassword,
            Self::SftpPrivateKey { .. } => DeployCredentialKind::SftpPrivateKey,
            Self::S3 { .. } => DeployCredentialKind::S3,
            Self::CloudflarePages { .. } => DeployCredentialKind::CloudflarePages,
        }
    }
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeploySecretStore {
    #[serde(default = "secret_store_schema_version")]
    schema_version: u32,
    #[serde(default)]
    credentials: BTreeMap<String, StoredDeployCredential>,
}

pub(crate) fn resolve_credential<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &Path,
    target: &DeployTarget,
) -> Result<StoredDeployCredential, String> {
    target.validate()?;
    let project_path = canonical_project_path(project_root)?;
    let store = read_secret_store(app, &project_path)?;
    let credential = match store.credentials.get(&target.credential_ref).cloned() {
        Some(credential) => credential,
        None if target.credential_ref == LEGACY_BUNNY_CREDENTIAL_REF
            && target.provider_kind() == DeployProviderKind::Bunny =>
        {
            legacy_bunny_credential(project_root)?
        }
        None => {
            return Err(format!(
                "Credentialele '{}' nu sunt configurate pentru ținta '{}'.",
                target.credential_ref, target.name
            ))
        }
    };
    if !credential.kind().supports_provider(target.provider_kind()) {
        return Err(format!(
            "Credentialele '{}' nu corespund providerului {}.",
            target.credential_ref,
            target.provider_kind().as_str()
        ));
    }
    Ok(credential)
}

pub fn settings_with_legacy_bunny_fallback(
    project_root: &Path,
    settings: DeploySettings,
) -> DeploySettings {
    if !settings.targets.is_empty() {
        return settings;
    }
    let Ok(env) = read_env_from_root(project_root) else {
        return settings;
    };
    let Some(storage_zone) = env
        .get("BUNNY_STORAGE_ZONE")
        .filter(|value| !value.is_empty())
        .cloned()
    else {
        return settings;
    };
    let Some(pull_zone_id) = env
        .get("BUNNY_PULL_ZONE_ID")
        .filter(|value| !value.is_empty())
        .cloned()
    else {
        return settings;
    };
    if env.get("BUNNY_STORAGE_KEY").is_none_or(String::is_empty)
        || env.get("BUNNY_CDN_API_KEY").is_none_or(String::is_empty)
    {
        return settings;
    }
    let target = DeployTarget {
        id: "legacy-bunny".to_string(),
        name: "Bunny (.env legacy)".to_string(),
        credential_ref: LEGACY_BUNNY_CREDENTIAL_REF.to_string(),
        cleanup_policy: DeployCleanupPolicy::ManagedOnly,
        provider: DeployTargetProvider::Bunny(BunnyTargetConfig {
            storage_zone,
            storage_region: env
                .get("BUNNY_STORAGE_REGION")
                .filter(|value| !value.is_empty())
                .cloned()
                .unwrap_or_else(|| "de".to_string()),
            pull_zone_id,
            remote_prefix: String::new(),
        }),
    };
    if target.validate().is_err() {
        return settings;
    }
    DeploySettings {
        active_target_id: Some(target.id.clone()),
        targets: vec![target],
        ..DeploySettings::default()
    }
}

pub fn credential_statuses<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &Path,
    targets: &[DeployTarget],
) -> Result<Vec<DeployCredentialStatus>, String> {
    let project_path = canonical_project_path(project_root)?;
    let store = read_secret_store(app, &project_path)?;
    Ok(targets
        .iter()
        .map(|target| {
            let stored = store.credentials.get(&target.credential_ref);
            let legacy = stored.is_none()
                && target.credential_ref == LEGACY_BUNNY_CREDENTIAL_REF
                && target.provider_kind() == DeployProviderKind::Bunny
                && legacy_bunny_credential(project_root).is_ok();
            DeployCredentialStatus {
                schema_version: DEPLOY_CREDENTIAL_STATUS_SCHEMA_VERSION,
                credential_ref: target.credential_ref.clone(),
                kind: stored.map_or_else(
                    || default_credential_kind(target.provider_kind()),
                    StoredDeployCredential::kind,
                ),
                configured: stored.is_some_and(|credential| {
                    credential.kind().supports_provider(target.provider_kind())
                }) || legacy,
            }
        })
        .collect())
}

pub fn save_credential<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &Path,
    target: &DeployTarget,
    input: DeployCredentialWriteInput,
) -> Result<DeployCredentialStatus, String> {
    target.validate()?;
    validate_credential_ref(&input.credential_ref)?;
    if input.credential_ref != target.credential_ref {
        return Err("Referința credentialelor nu corespunde țintei deploy.".to_string());
    }
    input.secret.validate()?;
    let credential = input.secret.into_stored();
    if !credential.kind().supports_provider(target.provider_kind()) {
        return Err("Tipul credentialelor nu corespunde providerului țintei.".to_string());
    }
    let project_path = canonical_project_path(project_root)?;
    let mut store = read_secret_store(app, &project_path)?;
    let kind = credential.kind();
    store
        .credentials
        .insert(input.credential_ref.clone(), credential);
    write_secret_store(app, &project_path, &store)?;
    Ok(DeployCredentialStatus {
        schema_version: DEPLOY_CREDENTIAL_STATUS_SCHEMA_VERSION,
        credential_ref: input.credential_ref,
        kind,
        configured: true,
    })
}

pub fn delete_credential<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &Path,
    credential_ref: &str,
) -> Result<bool, String> {
    validate_credential_ref(credential_ref)?;
    let project_path = canonical_project_path(project_root)?;
    let mut store = read_secret_store(app, &project_path)?;
    let removed = store.credentials.remove(credential_ref).is_some();
    if removed {
        write_secret_store(app, &project_path, &store)?;
    }
    Ok(removed)
}

fn legacy_bunny_credential(project_root: &Path) -> Result<StoredDeployCredential, String> {
    let env = read_env_from_root(project_root)?;
    Ok(StoredDeployCredential::Bunny {
        storage_key: env_require(&env, "BUNNY_STORAGE_KEY")?,
        cdn_api_key: env_require(&env, "BUNNY_CDN_API_KEY")?,
    })
}

fn read_secret_store<R: Runtime>(
    app: &AppHandle<R>,
    project_path: &str,
) -> Result<DeploySecretStore, String> {
    let path = project_deploy_secrets_path(app, project_path)?;
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(DeploySecretStore {
                schema_version: DEPLOY_SECRET_STORE_SCHEMA_VERSION,
                ..DeploySecretStore::default()
            })
        }
        Err(error) => {
            return Err(format!(
                "Magazia internă de credentiale deploy nu poate fi citită: {error}."
            ))
        }
    };
    let store: DeploySecretStore = serde_json::from_str(&source)
        .map_err(|_| "Magazia internă de credentiale deploy este invalidă.".to_string())?;
    if store.schema_version != DEPLOY_SECRET_STORE_SCHEMA_VERSION {
        return Err(format!(
            "Schema magaziei de credentiale este {}, așteptat {}.",
            store.schema_version, DEPLOY_SECRET_STORE_SCHEMA_VERSION
        ));
    }
    Ok(store)
}

fn write_secret_store<R: Runtime>(
    app: &AppHandle<R>,
    project_path: &str,
    store: &DeploySecretStore,
) -> Result<(), String> {
    let path = project_deploy_secrets_path(app, project_path)?;
    let boundary = projects_config_dir(app)?;
    let public_label = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| format!("config/projects/{name}"))
        .unwrap_or_else(|| "config/projects/deploy-secrets.json".to_string());
    let mut contents = serde_json::to_string_pretty(store)
        .map_err(|_| "Magazia de credentiale deploy nu poate fi serializată.".to_string())?;
    contents.push('\n');
    let intent = WriteIntent::new(
        WriteCategory::InternalAppWrite,
        WriteOwner::AppConfig,
        WriteOperationKind::WriteText,
        WriteTarget::new(path, boundary, public_label),
        WritePolicy::internal_atomic(),
        "Scriere credentiale deploy separate de configurația publică",
    );
    WriteAuthority::new(app)
        .write_text(intent, &contents)
        .map_err(|error| error.into_terminal_diagnostic())?;
    Ok(())
}

fn canonical_project_path(project_root: &Path) -> Result<String, String> {
    fs::canonicalize(project_root)
        .map(|path| path.to_string_lossy().to_string())
        .map_err(|error| format!("ProjectRoot nu poate fi capturat pentru credentiale: {error}."))
}

fn validate_credential_ref(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err("Referința credentialelor deploy este invalidă.".to_string());
    }
    Ok(())
}

fn validate_username(value: &str) -> Result<(), String> {
    validate_secret("Utilizatorul remote", value, 1024)?;
    if value.bytes().any(|byte| byte.is_ascii_control()) {
        return Err("Utilizatorul remote conține caractere de control.".to_string());
    }
    Ok(())
}

fn validate_secret(label: &str, value: &str, max_bytes: usize) -> Result<(), String> {
    if value.is_empty() || value.len() > max_bytes || value.contains('\0') {
        return Err(format!("{label} este gol sau depășește limita sigură."));
    }
    Ok(())
}

fn validate_optional_secret(label: &str, value: &str, max_bytes: usize) -> Result<(), String> {
    if value.len() > max_bytes || value.contains('\0') {
        return Err(format!("{label} depășește limita sigură."));
    }
    Ok(())
}

fn secret_store_schema_version() -> u32 {
    DEPLOY_SECRET_STORE_SCHEMA_VERSION
}

fn default_credential_kind(provider: DeployProviderKind) -> DeployCredentialKind {
    match provider {
        DeployProviderKind::Bunny => DeployCredentialKind::Bunny,
        DeployProviderKind::Ftp => DeployCredentialKind::Ftp,
        DeployProviderKind::Sftp => DeployCredentialKind::SftpPassword,
        DeployProviderKind::S3 => DeployCredentialKind::S3,
        DeployProviderKind::CloudflarePages => DeployCredentialKind::CloudflarePages,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env,
        time::{SystemTime, UNIX_EPOCH},
    };

    use tauri::Manager;

    use crate::{
        app_home::{ensure_app_home, project_deploy_secrets_path, TEST_APP_ENV_LOCK},
        kernel::write_authority::WriteAuthorityRuntime,
    };

    #[test]
    fn credential_store_roundtrip_uses_the_declared_write_authority_path() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = temp_dir("store-roundtrip");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();

        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock app");
        ensure_app_home(app.handle()).expect("app home");
        app.state::<WriteAuthorityRuntime>()
            .boot_recovery()
            .expect("write recovery bootstrap");

        let target = DeployTarget {
            id: "production".to_string(),
            name: "Production".to_string(),
            credential_ref: "production-credentials".to_string(),
            cleanup_policy: DeployCleanupPolicy::ManagedOnly,
            provider: DeployTargetProvider::Bunny(BunnyTargetConfig {
                storage_zone: "site".to_string(),
                storage_region: "de".to_string(),
                pull_zone_id: "42".to_string(),
                remote_prefix: "pana-tests/manual".to_string(),
            }),
        };
        let status = save_credential(
            app.handle(),
            &project,
            &target,
            DeployCredentialWriteInput {
                credential_ref: target.credential_ref.clone(),
                secret: DeployCredentialSecretInput::Bunny {
                    storage_key: "storage-secret".to_string(),
                    cdn_api_key: "cdn-secret".to_string(),
                },
            },
        )
        .expect("credential save through WriteAuthority");
        assert!(status.configured);

        let canonical_project = fs::canonicalize(&project).unwrap();
        let store_path =
            project_deploy_secrets_path(app.handle(), &canonical_project.to_string_lossy())
                .unwrap();
        assert!(store_path.exists());
        assert!(store_path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".deploy-secrets.json")));

        match resolve_credential(app.handle(), &project, &target).unwrap() {
            StoredDeployCredential::Bunny {
                storage_key,
                cdn_api_key,
            } => {
                assert_eq!(storage_key, "storage-secret");
                assert_eq!(cdn_api_key, "cdn-secret");
            }
            _ => panic!("credential kind changed during roundtrip"),
        }

        assert!(delete_credential(app.handle(), &project, &target.credential_ref).unwrap());
        assert!(
            read_secret_store(app.handle(), &canonical_project.to_string_lossy())
                .unwrap()
                .credentials
                .is_empty()
        );

        drop(app);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn credential_input_accepts_secret_material_but_status_never_serializes_it() {
        let source = r#"{
          "credentialRef":"production",
          "kind":"s3",
          "accessKeyId":"access-value",
          "secretAccessKey":"secret-value",
          "sessionToken":"session-value"
        }"#;
        let input: DeployCredentialWriteInput = serde_json::from_str(source).unwrap();
        input.secret.validate().unwrap();
        let status = DeployCredentialStatus {
            schema_version: DEPLOY_CREDENTIAL_STATUS_SCHEMA_VERSION,
            credential_ref: input.credential_ref,
            kind: DeployCredentialKind::S3,
            configured: true,
        };
        let public = serde_json::to_string(&status).unwrap();
        for forbidden in ["access-value", "secret-value", "session-value"] {
            assert!(!public.contains(forbidden));
        }
    }

    #[test]
    fn credential_kind_must_match_provider() {
        assert!(DeployCredentialKind::S3.supports_provider(DeployProviderKind::S3));
        assert!(!DeployCredentialKind::S3.supports_provider(DeployProviderKind::Ftp));
        assert!(DeployCredentialKind::SftpPrivateKey.supports_provider(DeployProviderKind::Sftp));
    }

    #[test]
    fn private_key_validation_rejects_arbitrary_text_and_nul_secrets() {
        let key = DeployCredentialSecretInput::SftpPrivateKey {
            username: "deploy".to_string(),
            private_key_pem: "not-a-key".to_string(),
            passphrase: None,
        };
        assert!(key.validate().is_err());
        assert!(validate_secret("secret", "bad\0value", 100).is_err());
    }

    #[test]
    fn stored_credentials_debug_is_intentionally_unavailable_and_public_type_is_redacted() {
        fn assert_serialize<T: Serialize>() {}
        assert_serialize::<DeployCredentialStatus>();
        let fields = serde_json::to_value(DeployCredentialStatus {
            schema_version: 1,
            credential_ref: "ref".to_string(),
            kind: DeployCredentialKind::Bunny,
            configured: true,
        })
        .unwrap();
        assert_eq!(fields.as_object().unwrap().len(), 4);
    }

    #[test]
    fn complete_legacy_bunny_env_becomes_a_typed_target_without_exposing_secrets() {
        let root = temp_dir("legacy-bunny");
        fs::write(
            root.join(".env"),
            "BUNNY_STORAGE_ZONE=site\nBUNNY_STORAGE_KEY=storage-secret\nBUNNY_STORAGE_REGION=ny\nBUNNY_PULL_ZONE_ID=42\nBUNNY_CDN_API_KEY=cdn-secret\n",
        )
        .unwrap();
        let settings = settings_with_legacy_bunny_fallback(&root, DeploySettings::default());
        settings.validate().unwrap();
        assert_eq!(settings.active_target_id.as_deref(), Some("legacy-bunny"));
        let public = serde_json::to_string(&settings).unwrap();
        assert!(!public.contains("storage-secret"));
        assert!(!public.contains("cdn-secret"));
        assert!(public.contains(LEGACY_BUNNY_CREDENTIAL_REF));
        fs::remove_dir_all(root).unwrap();
    }

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "pana-deploy-credentials-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    struct TestEnvGuard {
        previous_values: Vec<(&'static str, Option<String>)>,
    }

    impl TestEnvGuard {
        fn from_root(root: &Path) -> Self {
            let bindings = [
                ("XDG_CONFIG_HOME", root.join("config")),
                ("XDG_DATA_HOME", root.join("data")),
                ("XDG_CACHE_HOME", root.join("cache")),
                ("XDG_STATE_HOME", root.join("state")),
            ];
            let previous_values = bindings
                .iter()
                .map(|(key, _)| (*key, env::var(key).ok()))
                .collect::<Vec<_>>();
            for (key, path) in bindings {
                env::set_var(key, path);
            }
            Self { previous_values }
        }
    }

    impl Drop for TestEnvGuard {
        fn drop(&mut self) {
            for (key, value) in self.previous_values.drain(..) {
                match value {
                    Some(previous) => env::set_var(key, previous),
                    None => env::remove_var(key),
                }
            }
        }
    }
}
