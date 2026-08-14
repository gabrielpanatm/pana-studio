use std::{collections::BTreeMap, path::Path};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};

use crate::kernel::project_env_store::{validate_env_prefix, ProjectEnvStore};

use super::model::{DeployProviderKind, DeploySettings, DeployTarget};

pub const DEPLOY_CREDENTIAL_STATUS_SCHEMA_VERSION: u32 = 2;
pub const DEPLOY_CONFIGURATION_SCHEMA_VERSION: u32 = 2;
const MAX_SECRET_BYTES: usize = 1024 * 1024;

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
    pub credential_env_prefix: String,
    pub kind: DeployCredentialKind,
    pub configured: bool,
    pub missing_fields: Vec<String>,
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
}

pub fn configuration_snapshot(
    project_root: &Path,
    settings: DeploySettings,
) -> Result<DeployConfigurationSnapshot, String> {
    settings.validate()?;
    let credential_statuses = credential_statuses(project_root, &settings.targets)?;
    let target_capabilities = settings
        .targets
        .iter()
        .map(|target| DeployTargetCapabilitySnapshot {
            target_id: target.id.clone(),
            provider: target.provider_kind(),
            capabilities: target.capabilities(),
        })
        .collect();
    Ok(DeployConfigurationSnapshot {
        schema_version: DEPLOY_CONFIGURATION_SCHEMA_VERSION,
        settings,
        credential_statuses,
        target_capabilities,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployCredentialWriteInput {
    pub credential_env_prefix: String,
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

pub(crate) struct PreparedCredentialWrite {
    pub(crate) credential_env_prefix: String,
    pub(crate) kind: DeployCredentialKind,
    pub(crate) values: BTreeMap<String, String>,
}

pub(crate) fn prepare_credential_write(
    target: &DeployTarget,
    input: DeployCredentialWriteInput,
) -> Result<PreparedCredentialWrite, String> {
    target.validate()?;
    validate_env_prefix(&input.credential_env_prefix)?;
    if input.credential_env_prefix != target.credential_env_prefix {
        return Err("Prefixul ENV nu corespunde țintei deploy.".to_string());
    }
    let (kind, values) = input.secret.into_env_values()?;
    if !kind.supports_provider(target.provider_kind()) {
        return Err("Tipul credentialelor nu corespunde providerului țintei.".to_string());
    }
    Ok(PreparedCredentialWrite {
        credential_env_prefix: input.credential_env_prefix,
        kind,
        values,
    })
}

impl DeployCredentialSecretInput {
    fn into_env_values(self) -> Result<(DeployCredentialKind, BTreeMap<String, String>), String> {
        let mut values = BTreeMap::new();
        let kind = match self {
            Self::Bunny {
                storage_key,
                cdn_api_key,
            } => {
                validate_secret("Bunny Storage key", &storage_key, 16 * 1024)?;
                validate_secret("Bunny CDN API key", &cdn_api_key, 16 * 1024)?;
                values.insert("CDN_API_KEY".to_string(), cdn_api_key);
                values.insert("STORAGE_KEY".to_string(), storage_key);
                DeployCredentialKind::Bunny
            }
            Self::Ftp { username, password } => {
                validate_username(&username)?;
                validate_secret("Parola FTP", &password, 64 * 1024)?;
                values.insert("PASSWORD".to_string(), password);
                values.insert("USERNAME".to_string(), username);
                DeployCredentialKind::Ftp
            }
            Self::SftpPassword { username, password } => {
                validate_username(&username)?;
                validate_secret("Parola SFTP", &password, 64 * 1024)?;
                values.insert("AUTH_MODE".to_string(), "password".to_string());
                values.insert("PASSWORD".to_string(), password);
                values.insert("USERNAME".to_string(), username);
                DeployCredentialKind::SftpPassword
            }
            Self::SftpPrivateKey {
                username,
                private_key_pem,
                passphrase,
            } => {
                validate_username(&username)?;
                validate_secret("Cheia privată SFTP", &private_key_pem, MAX_SECRET_BYTES)?;
                if !private_key_pem.contains("-----BEGIN") {
                    return Err(
                        "Cheia privată SFTP nu are un header PEM/OpenSSH valid.".to_string()
                    );
                }
                if let Some(passphrase) = passphrase.as_deref() {
                    validate_optional_secret("Passphrase-ul SFTP", passphrase, 64 * 1024)?;
                }
                values.insert("AUTH_MODE".to_string(), "private_key".to_string());
                values.insert(
                    "PRIVATE_KEY_BASE64".to_string(),
                    STANDARD.encode(private_key_pem.as_bytes()),
                );
                if let Some(passphrase) = passphrase.filter(|value| !value.is_empty()) {
                    values.insert("PASSPHRASE".to_string(), passphrase);
                }
                values.insert("USERNAME".to_string(), username);
                DeployCredentialKind::SftpPrivateKey
            }
            Self::S3 {
                access_key_id,
                secret_access_key,
                session_token,
            } => {
                validate_secret("Access key ID S3", &access_key_id, 16 * 1024)?;
                validate_secret("Secret access key S3", &secret_access_key, 64 * 1024)?;
                if let Some(session_token) = session_token.as_deref() {
                    validate_optional_secret("Session token S3", session_token, 256 * 1024)?;
                }
                values.insert("ACCESS_KEY_ID".to_string(), access_key_id);
                values.insert("SECRET_ACCESS_KEY".to_string(), secret_access_key);
                if let Some(session_token) = session_token.filter(|value| !value.is_empty()) {
                    values.insert("SESSION_TOKEN".to_string(), session_token);
                }
                DeployCredentialKind::S3
            }
            Self::CloudflarePages { api_token } => {
                validate_secret("Token-ul Cloudflare API", &api_token, 64 * 1024)?;
                values.insert("API_TOKEN".to_string(), api_token);
                DeployCredentialKind::CloudflarePages
            }
        };
        Ok((kind, values))
    }
}

#[derive(Clone)]
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
        passphrase: Option<String>,
    },
    S3 {
        access_key_id: String,
        secret_access_key: String,
        session_token: Option<String>,
    },
    CloudflarePages {
        api_token: String,
    },
}

pub(crate) fn resolve_credential(
    project_root: &Path,
    target: &DeployTarget,
) -> Result<StoredDeployCredential, String> {
    target.validate()?;
    let env = ProjectEnvStore::read_namespace(project_root, &target.credential_env_prefix)?;
    let credential = match target.provider_kind() {
        DeployProviderKind::Bunny => StoredDeployCredential::Bunny {
            storage_key: require(&env, "STORAGE_KEY", target)?,
            cdn_api_key: require(&env, "CDN_API_KEY", target)?,
        },
        DeployProviderKind::Ftp => StoredDeployCredential::Ftp {
            username: require(&env, "USERNAME", target)?,
            password: require(&env, "PASSWORD", target)?,
        },
        DeployProviderKind::Sftp => match env.get("AUTH_MODE").map(String::as_str) {
            Some("private_key") => {
                let encoded = require(&env, "PRIVATE_KEY_BASE64", target)?;
                let bytes = STANDARD.decode(encoded).map_err(|_| {
                    format!(
                        "Cheia privată SFTP pentru '{}' nu este Base64 valid.",
                        target.name
                    )
                })?;
                let private_key_pem = String::from_utf8(bytes).map_err(|_| {
                    format!("Cheia privată SFTP pentru '{}' nu este UTF-8.", target.name)
                })?;
                StoredDeployCredential::SftpPrivateKey {
                    username: require(&env, "USERNAME", target)?,
                    private_key_pem,
                    passphrase: env.get("PASSPHRASE").cloned(),
                }
            }
            Some("password") => StoredDeployCredential::SftpPassword {
                username: require(&env, "USERNAME", target)?,
                password: require(&env, "PASSWORD", target)?,
            },
            _ => {
                return Err(format!(
                    "Credentialele SFTP pentru '{}' cer AUTH_MODE=password sau private_key.",
                    target.name
                ))
            }
        },
        DeployProviderKind::S3 => StoredDeployCredential::S3 {
            access_key_id: require(&env, "ACCESS_KEY_ID", target)?,
            secret_access_key: require(&env, "SECRET_ACCESS_KEY", target)?,
            session_token: env.get("SESSION_TOKEN").cloned(),
        },
        DeployProviderKind::CloudflarePages => StoredDeployCredential::CloudflarePages {
            api_token: require(&env, "API_TOKEN", target)?,
        },
    };
    Ok(credential)
}

pub fn credential_statuses(
    project_root: &Path,
    targets: &[DeployTarget],
) -> Result<Vec<DeployCredentialStatus>, String> {
    targets
        .iter()
        .map(|target| credential_status(project_root, target))
        .collect()
}

pub(crate) fn credential_status(
    project_root: &Path,
    target: &DeployTarget,
) -> Result<DeployCredentialStatus, String> {
    target.validate()?;
    let env = ProjectEnvStore::read_namespace(project_root, &target.credential_env_prefix)?;
    let (kind, required): (DeployCredentialKind, &[&str]) = match target.provider_kind() {
        DeployProviderKind::Bunny => (DeployCredentialKind::Bunny, &["STORAGE_KEY", "CDN_API_KEY"]),
        DeployProviderKind::Ftp => (DeployCredentialKind::Ftp, &["USERNAME", "PASSWORD"]),
        DeployProviderKind::Sftp
            if env.get("AUTH_MODE").map(String::as_str) == Some("private_key") =>
        {
            (
                DeployCredentialKind::SftpPrivateKey,
                &["AUTH_MODE", "USERNAME", "PRIVATE_KEY_BASE64"],
            )
        }
        DeployProviderKind::Sftp => (
            DeployCredentialKind::SftpPassword,
            &["AUTH_MODE", "USERNAME", "PASSWORD"],
        ),
        DeployProviderKind::S3 => (
            DeployCredentialKind::S3,
            &["ACCESS_KEY_ID", "SECRET_ACCESS_KEY"],
        ),
        DeployProviderKind::CloudflarePages => {
            (DeployCredentialKind::CloudflarePages, &["API_TOKEN"])
        }
    };
    let missing_fields = required
        .iter()
        .filter(|suffix| env.get(**suffix).is_none_or(String::is_empty))
        .map(|suffix| (*suffix).to_string())
        .collect::<Vec<_>>();
    let configured = missing_fields.is_empty()
        && (target.provider_kind() != DeployProviderKind::Sftp
            || matches!(
                env.get("AUTH_MODE").map(String::as_str),
                Some("password" | "private_key")
            ));
    Ok(DeployCredentialStatus {
        schema_version: DEPLOY_CREDENTIAL_STATUS_SCHEMA_VERSION,
        credential_env_prefix: target.credential_env_prefix.clone(),
        kind,
        configured,
        missing_fields,
    })
}

fn require(
    env: &BTreeMap<String, String>,
    suffix: &str,
    target: &DeployTarget,
) -> Result<String, String> {
    env.get(suffix)
        .filter(|value| !value.is_empty())
        .cloned()
        .ok_or_else(|| {
            format!(
                "Lipsește {}__{} pentru ținta '{}'.",
                target.credential_env_prefix, suffix, target.name
            )
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deploy::model::{
        BunnyTargetConfig, CloudflarePagesTargetConfig, DeployCleanupPolicy, DeploySettings,
        DeployTargetProvider, FtpSecurityMode, FtpTargetConfig, S3TargetConfig, SftpTargetConfig,
    };
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

    fn target(id: &str, prefix: &str, provider: DeployTargetProvider) -> DeployTarget {
        DeployTarget {
            id: id.to_string(),
            name: id.to_string(),
            credential_env_prefix: prefix.to_string(),
            cleanup_policy: DeployCleanupPolicy::ManagedOnly,
            provider,
        }
    }

    fn bunny_target(prefix: &str) -> DeployTarget {
        target(
            "production",
            prefix,
            DeployTargetProvider::Bunny(BunnyTargetConfig {
                storage_zone: "site".to_string(),
                storage_region: "de".to_string(),
                pull_zone_id: "42".to_string(),
                remote_prefix: String::new(),
            }),
        )
    }

    fn ftp_target(prefix: &str) -> DeployTarget {
        target(
            "ftp",
            prefix,
            DeployTargetProvider::Ftp(FtpTargetConfig {
                host: "ftp.example.com".to_string(),
                port: 21,
                remote_root: "/public_html".to_string(),
                security: FtpSecurityMode::FtpsExplicit,
                allow_insecure_ftp: false,
            }),
        )
    }

    fn sftp_target(id: &str, prefix: &str) -> DeployTarget {
        target(
            id,
            prefix,
            DeployTargetProvider::Sftp(SftpTargetConfig {
                host: "sftp.example.com".to_string(),
                port: 22,
                remote_root: "/site".to_string(),
                expected_host_key_sha256: "SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
                    .to_string(),
            }),
        )
    }

    fn s3_target(prefix: &str) -> DeployTarget {
        target(
            "s3",
            prefix,
            DeployTargetProvider::S3(S3TargetConfig {
                bucket: "site".to_string(),
                prefix: "production".to_string(),
                region: "eu-central-1".to_string(),
                endpoint: None,
                force_path_style: false,
                allow_insecure_endpoint: false,
                cache_control: Some("public, max-age=3600".to_string()),
            }),
        )
    }

    fn cloudflare_target(prefix: &str) -> DeployTarget {
        target(
            "pages",
            prefix,
            DeployTargetProvider::CloudflarePages(CloudflarePagesTargetConfig {
                account_id: "0123456789abcdef".to_string(),
                project_name: "pana-site".to_string(),
                branch: Some("main".to_string()),
            }),
        )
    }

    fn unique_test_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "panastudio-deploy-credentials-{label}-{}-{stamp}",
            TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn write_input_becomes_only_namespaced_env_fields() {
        let prepared = prepare_credential_write(
            &bunny_target("PANA_DEPLOY_PRODUCTION"),
            DeployCredentialWriteInput {
                credential_env_prefix: "PANA_DEPLOY_PRODUCTION".to_string(),
                secret: DeployCredentialSecretInput::Bunny {
                    storage_key: "storage-secret".to_string(),
                    cdn_api_key: "cdn-secret".to_string(),
                },
            },
        )
        .unwrap();
        assert_eq!(prepared.kind, DeployCredentialKind::Bunny);
        assert_eq!(prepared.values.len(), 2);
        assert_eq!(prepared.values["STORAGE_KEY"], "storage-secret");
    }

    #[test]
    fn private_key_is_encoded_for_single_line_env_storage() {
        let target = sftp_target("sftp", "PANA_DEPLOY_SFTP");
        let prepared = prepare_credential_write(
            &target,
            DeployCredentialWriteInput {
                credential_env_prefix: target.credential_env_prefix.clone(),
                secret: DeployCredentialSecretInput::SftpPrivateKey {
                    username: "deploy".to_string(),
                    private_key_pem: "-----BEGIN KEY-----\nabc\n-----END KEY-----".to_string(),
                    passphrase: None,
                },
            },
        )
        .unwrap();
        assert!(!prepared.values["PRIVATE_KEY_BASE64"].contains('\n'));
        assert_eq!(prepared.values["AUTH_MODE"], "private_key");
    }

    #[test]
    fn every_provider_credential_has_an_exact_namespaced_contract() {
        let cases = [
            (
                bunny_target("PANA_DEPLOY_BUNNY"),
                DeployCredentialWriteInput {
                    credential_env_prefix: "PANA_DEPLOY_BUNNY".to_string(),
                    secret: DeployCredentialSecretInput::Bunny {
                        storage_key: "bunny-storage".to_string(),
                        cdn_api_key: "bunny-cdn".to_string(),
                    },
                },
                DeployCredentialKind::Bunny,
                vec!["CDN_API_KEY", "STORAGE_KEY"],
            ),
            (
                ftp_target("PANA_DEPLOY_FTP"),
                DeployCredentialWriteInput {
                    credential_env_prefix: "PANA_DEPLOY_FTP".to_string(),
                    secret: DeployCredentialSecretInput::Ftp {
                        username: "ftp-user".to_string(),
                        password: "ftp-password".to_string(),
                    },
                },
                DeployCredentialKind::Ftp,
                vec!["PASSWORD", "USERNAME"],
            ),
            (
                sftp_target("sftp-password", "PANA_DEPLOY_SFTP_PASSWORD"),
                DeployCredentialWriteInput {
                    credential_env_prefix: "PANA_DEPLOY_SFTP_PASSWORD".to_string(),
                    secret: DeployCredentialSecretInput::SftpPassword {
                        username: "sftp-user".to_string(),
                        password: "sftp-password".to_string(),
                    },
                },
                DeployCredentialKind::SftpPassword,
                vec!["AUTH_MODE", "PASSWORD", "USERNAME"],
            ),
            (
                sftp_target("sftp-key", "PANA_DEPLOY_SFTP_KEY"),
                DeployCredentialWriteInput {
                    credential_env_prefix: "PANA_DEPLOY_SFTP_KEY".to_string(),
                    secret: DeployCredentialSecretInput::SftpPrivateKey {
                        username: "key-user".to_string(),
                        private_key_pem: "-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n-----END OPENSSH PRIVATE KEY-----".to_string(),
                        passphrase: Some("key-passphrase".to_string()),
                    },
                },
                DeployCredentialKind::SftpPrivateKey,
                vec!["AUTH_MODE", "PASSPHRASE", "PRIVATE_KEY_BASE64", "USERNAME"],
            ),
            (
                s3_target("PANA_DEPLOY_S3"),
                DeployCredentialWriteInput {
                    credential_env_prefix: "PANA_DEPLOY_S3".to_string(),
                    secret: DeployCredentialSecretInput::S3 {
                        access_key_id: "s3-access".to_string(),
                        secret_access_key: "s3-secret".to_string(),
                        session_token: Some("s3-session".to_string()),
                    },
                },
                DeployCredentialKind::S3,
                vec!["ACCESS_KEY_ID", "SECRET_ACCESS_KEY", "SESSION_TOKEN"],
            ),
            (
                cloudflare_target("PANA_DEPLOY_CLOUDFLARE"),
                DeployCredentialWriteInput {
                    credential_env_prefix: "PANA_DEPLOY_CLOUDFLARE".to_string(),
                    secret: DeployCredentialSecretInput::CloudflarePages {
                        api_token: "cloudflare-token".to_string(),
                    },
                },
                DeployCredentialKind::CloudflarePages,
                vec!["API_TOKEN"],
            ),
        ];

        for (target, input, expected_kind, expected_keys) in cases {
            let prepared = prepare_credential_write(&target, input).unwrap();
            assert_eq!(prepared.kind, expected_kind);
            assert_eq!(
                prepared
                    .values
                    .keys()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                expected_keys
            );
        }
    }

    #[test]
    fn multi_target_status_and_configuration_never_expose_secret_values() {
        let root = unique_test_dir("status");
        fs::create_dir_all(&root).unwrap();
        let private_key =
            "-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n-----END OPENSSH PRIVATE KEY-----";
        let encoded_private_key = STANDARD.encode(private_key.as_bytes());
        let source = format!(
            concat!(
                "PANA_DEPLOY_BUNNY__STORAGE_KEY=\"bunny-storage-secret\"\n",
                "PANA_DEPLOY_BUNNY__CDN_API_KEY=\"bunny-cdn-secret\"\n",
                "PANA_DEPLOY_FTP__USERNAME=\"ftp-user\"\n",
                "PANA_DEPLOY_FTP__PASSWORD=\"ftp-password-secret\"\n",
                "PANA_DEPLOY_SFTP_PASSWORD__AUTH_MODE=\"password\"\n",
                "PANA_DEPLOY_SFTP_PASSWORD__USERNAME=\"sftp-user\"\n",
                "PANA_DEPLOY_SFTP_PASSWORD__PASSWORD=\"sftp-password-secret\"\n",
                "PANA_DEPLOY_SFTP_KEY__AUTH_MODE=\"private_key\"\n",
                "PANA_DEPLOY_SFTP_KEY__USERNAME=\"key-user\"\n",
                "PANA_DEPLOY_SFTP_KEY__PRIVATE_KEY_BASE64=\"{}\"\n",
                "PANA_DEPLOY_SFTP_KEY__PASSPHRASE=\"key-passphrase-secret\"\n",
                "PANA_DEPLOY_S3__ACCESS_KEY_ID=\"s3-access\"\n",
                "PANA_DEPLOY_S3__SECRET_ACCESS_KEY=\"s3-secret-value\"\n",
                "PANA_DEPLOY_S3__SESSION_TOKEN=\"s3-session-secret\"\n",
                "PANA_DEPLOY_CLOUDFLARE__API_TOKEN=\"cloudflare-secret-token\"\n"
            ),
            encoded_private_key
        );
        fs::write(root.join(".env"), source).unwrap();

        let targets = vec![
            bunny_target("PANA_DEPLOY_BUNNY"),
            ftp_target("PANA_DEPLOY_FTP"),
            sftp_target("sftp-password", "PANA_DEPLOY_SFTP_PASSWORD"),
            sftp_target("sftp-key", "PANA_DEPLOY_SFTP_KEY"),
            s3_target("PANA_DEPLOY_S3"),
            cloudflare_target("PANA_DEPLOY_CLOUDFLARE"),
        ];
        let settings = DeploySettings {
            schema_version: super::super::model::DEPLOY_SETTINGS_SCHEMA_VERSION,
            revision: 41,
            active_target_id: Some("production".to_string()),
            targets: targets.clone(),
        };
        let snapshot = configuration_snapshot(&root, settings).unwrap();
        assert_eq!(snapshot.credential_statuses.len(), targets.len());
        assert!(snapshot
            .credential_statuses
            .iter()
            .all(|status| status.configured && status.missing_fields.is_empty()));

        let json = serde_json::to_string(&snapshot).unwrap();
        for secret in [
            "bunny-storage-secret",
            "bunny-cdn-secret",
            "ftp-password-secret",
            "sftp-password-secret",
            "key-passphrase-secret",
            "s3-secret-value",
            "s3-session-secret",
            "cloudflare-secret-token",
            encoded_private_key.as_str(),
        ] {
            assert!(!json.contains(secret), "status leaked {secret}");
        }

        assert!(matches!(
            resolve_credential(&root, &targets[0]).unwrap(),
            StoredDeployCredential::Bunny { storage_key, cdn_api_key }
                if storage_key == "bunny-storage-secret" && cdn_api_key == "bunny-cdn-secret"
        ));
        assert!(matches!(
            resolve_credential(&root, &targets[1]).unwrap(),
            StoredDeployCredential::Ftp { username, password }
                if username == "ftp-user" && password == "ftp-password-secret"
        ));
        assert!(matches!(
            resolve_credential(&root, &targets[2]).unwrap(),
            StoredDeployCredential::SftpPassword { username, password }
                if username == "sftp-user" && password == "sftp-password-secret"
        ));
        assert!(matches!(
            resolve_credential(&root, &targets[3]).unwrap(),
            StoredDeployCredential::SftpPrivateKey { username, private_key_pem, passphrase }
                if username == "key-user"
                    && private_key_pem == private_key
                    && passphrase.as_deref() == Some("key-passphrase-secret")
        ));
        assert!(matches!(
            resolve_credential(&root, &targets[4]).unwrap(),
            StoredDeployCredential::S3 { access_key_id, secret_access_key, session_token }
                if access_key_id == "s3-access"
                    && secret_access_key == "s3-secret-value"
                    && session_token.as_deref() == Some("s3-session-secret")
        ));
        assert!(matches!(
            resolve_credential(&root, &targets[5]).unwrap(),
            StoredDeployCredential::CloudflarePages { api_token }
                if api_token == "cloudflare-secret-token"
        ));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn credential_kind_and_prefix_must_match_the_target() {
        let target = bunny_target("PANA_DEPLOY_PRODUCTION");
        let wrong_prefix = prepare_credential_write(
            &target,
            DeployCredentialWriteInput {
                credential_env_prefix: "PANA_DEPLOY_OTHER".to_string(),
                secret: DeployCredentialSecretInput::Bunny {
                    storage_key: "storage".to_string(),
                    cdn_api_key: "cdn".to_string(),
                },
            },
        )
        .err()
        .unwrap();
        assert!(wrong_prefix.contains("nu corespunde"));

        let wrong_kind = prepare_credential_write(
            &target,
            DeployCredentialWriteInput {
                credential_env_prefix: target.credential_env_prefix.clone(),
                secret: DeployCredentialSecretInput::CloudflarePages {
                    api_token: "token".to_string(),
                },
            },
        )
        .err()
        .unwrap();
        assert!(wrong_kind.contains("providerului"));
    }
}
