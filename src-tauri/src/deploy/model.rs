use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use url::Url;

pub const DEPLOY_SETTINGS_SCHEMA_VERSION: u32 = 1;
pub const DEPLOY_PLAN_SCHEMA_VERSION: u32 = 1;
pub const DEPLOY_PROGRESS_SCHEMA_VERSION: u32 = 1;
pub const DEPLOY_RECEIPT_SCHEMA_VERSION: u32 = 1;
pub const DEPLOY_ERROR_SCHEMA_VERSION: u32 = 1;
pub const DEPLOY_CONNECTION_TEST_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeployProviderKind {
    Bunny,
    Ftp,
    Sftp,
    S3,
    CloudflarePages,
}

impl DeployProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bunny => "bunny",
            Self::Ftp => "ftp",
            Self::Sftp => "sftp",
            Self::S3 => "s3",
            Self::CloudflarePages => "cloudflare_pages",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploySettings {
    pub schema_version: u32,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub active_target_id: Option<String>,
    #[serde(default)]
    pub targets: Vec<DeployTarget>,
}

impl Default for DeploySettings {
    fn default() -> Self {
        Self {
            schema_version: DEPLOY_SETTINGS_SCHEMA_VERSION,
            revision: 0,
            active_target_id: None,
            targets: Vec::new(),
        }
    }
}

impl DeploySettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != DEPLOY_SETTINGS_SCHEMA_VERSION {
            return Err(format!(
                "Schema deploy nesuportată: {}, așteptat {}.",
                self.schema_version, DEPLOY_SETTINGS_SCHEMA_VERSION
            ));
        }
        let mut target_ids = std::collections::HashSet::new();
        for target in &self.targets {
            target.validate()?;
            if !target_ids.insert(target.id.as_str()) {
                return Err(format!(
                    "Ținta deploy '{}' este definită de mai multe ori.",
                    target.id
                ));
            }
        }
        if let Some(active_target_id) = self.active_target_id.as_deref() {
            if !target_ids.contains(active_target_id) {
                return Err(format!(
                    "Ținta deploy activă '{active_target_id}' nu există în configurație."
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployTarget {
    pub id: String,
    pub name: String,
    pub credential_ref: String,
    #[serde(default)]
    pub cleanup_policy: DeployCleanupPolicy,
    #[serde(flatten)]
    pub provider: DeployTargetProvider,
}

impl DeployTarget {
    pub fn provider_kind(&self) -> DeployProviderKind {
        self.provider.kind()
    }

    pub fn capabilities(&self) -> ProviderCapabilities {
        self.provider.capabilities()
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_identifier("ID-ul țintei", &self.id)?;
        validate_display_name(&self.name)?;
        validate_identifier("Referința credentialelor", &self.credential_ref)?;
        if self.cleanup_policy == DeployCleanupPolicy::MirrorDestination
            && matches!(self.provider, DeployTargetProvider::CloudflarePages(_))
        {
            return Err(
                "Cloudflare Pages publică versiuni atomice și nu acceptă oglindirea unei destinații remote."
                    .to_string(),
            );
        }
        self.provider.validate()
    }

    pub fn security_warnings(&self) -> Vec<String> {
        let mut warnings = match &self.provider {
            DeployTargetProvider::Ftp(config) if config.security == FtpSecurityMode::Plain => {
                vec![
                    "Ținta folosește FTP necriptat; credentialele și conținutul circulă fără TLS."
                        .to_string(),
                ]
            }
            DeployTargetProvider::S3(config)
                if config
                    .endpoint
                    .as_deref()
                    .is_some_and(|endpoint| endpoint.starts_with("http://")) =>
            {
                vec!["Ținta S3 folosește un endpoint HTTP necriptat.".to_string()]
            }
            _ => Vec::new(),
        };
        if self.cleanup_policy == DeployCleanupPolicy::MirrorDestination {
            warnings.push(
                "Oglindirea completă șterge inclusiv fișierele remote care nu au fost publicate de Pană Studio."
                    .to_string(),
            );
            if self.remote_scope_is_root() {
                warnings.push(
                    "Oglindirea este activă pe rădăcina destinației; toate fișierele absente din build intră în planul de ștergere."
                        .to_string(),
                );
            }
        }
        warnings
    }

    fn remote_scope_is_root(&self) -> bool {
        match &self.provider {
            DeployTargetProvider::Bunny(config) => config.remote_prefix.is_empty(),
            DeployTargetProvider::Ftp(config) => config.remote_root == "/",
            DeployTargetProvider::Sftp(config) => config.remote_root == "/",
            DeployTargetProvider::S3(config) => config.prefix.is_empty(),
            DeployTargetProvider::CloudflarePages(_) => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeployCleanupPolicy {
    #[default]
    ManagedOnly,
    MirrorDestination,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "provider", content = "config", rename_all = "snake_case")]
pub enum DeployTargetProvider {
    Bunny(BunnyTargetConfig),
    Ftp(FtpTargetConfig),
    Sftp(SftpTargetConfig),
    S3(S3TargetConfig),
    CloudflarePages(CloudflarePagesTargetConfig),
}

impl DeployTargetProvider {
    pub fn kind(&self) -> DeployProviderKind {
        match self {
            Self::Bunny(_) => DeployProviderKind::Bunny,
            Self::Ftp(_) => DeployProviderKind::Ftp,
            Self::Sftp(_) => DeployProviderKind::Sftp,
            Self::S3(_) => DeployProviderKind::S3,
            Self::CloudflarePages(_) => DeployProviderKind::CloudflarePages,
        }
    }

    pub fn capabilities(&self) -> ProviderCapabilities {
        match self {
            Self::Bunny(_) => ProviderCapabilities::sync(true, false),
            Self::Ftp(_) | Self::Sftp(_) => ProviderCapabilities::sync(false, false),
            Self::S3(_) => ProviderCapabilities::sync(false, true),
            Self::CloudflarePages(_) => ProviderCapabilities {
                remote_inventory: false,
                delete_stale: false,
                atomic_activation: true,
                cache_invalidation: false,
                metadata_headers: false,
                connection_test: true,
            },
        }
    }

    fn validate(&self) -> Result<(), String> {
        match self {
            Self::Bunny(config) => config.validate(),
            Self::Ftp(config) => config.validate(),
            Self::Sftp(config) => config.validate(),
            Self::S3(config) => config.validate(),
            Self::CloudflarePages(config) => config.validate(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    pub remote_inventory: bool,
    pub delete_stale: bool,
    pub atomic_activation: bool,
    pub cache_invalidation: bool,
    pub metadata_headers: bool,
    pub connection_test: bool,
}

impl ProviderCapabilities {
    fn sync(cache_invalidation: bool, metadata_headers: bool) -> Self {
        Self {
            remote_inventory: true,
            delete_stale: true,
            atomic_activation: false,
            cache_invalidation,
            metadata_headers,
            connection_test: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BunnyTargetConfig {
    pub storage_zone: String,
    #[serde(default = "default_bunny_region")]
    pub storage_region: String,
    pub pull_zone_id: String,
    #[serde(default)]
    pub remote_prefix: String,
}

impl BunnyTargetConfig {
    fn validate(&self) -> Result<(), String> {
        validate_nonempty("Zona Bunny Storage", &self.storage_zone, 128)?;
        validate_host_label("Regiunea Bunny Storage", &self.storage_region)?;
        validate_nonempty("ID-ul Bunny Pull Zone", &self.pull_zone_id, 128)?;
        validate_remote_prefix(&self.remote_prefix)
    }
}

fn default_bunny_region() -> String {
    "de".to_string()
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FtpSecurityMode {
    FtpsExplicit,
    Plain,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FtpTargetConfig {
    pub host: String,
    #[serde(default = "default_ftp_port")]
    pub port: u16,
    pub remote_root: String,
    pub security: FtpSecurityMode,
    #[serde(default)]
    pub allow_insecure_ftp: bool,
}

impl FtpTargetConfig {
    fn validate(&self) -> Result<(), String> {
        validate_hostname(&self.host)?;
        validate_port(self.port)?;
        validate_remote_root(&self.remote_root)?;
        if self.security == FtpSecurityMode::Plain && !self.allow_insecure_ftp {
            return Err(
                "FTP necriptat necesită confirmarea explicită allowInsecureFtp=true.".to_string(),
            );
        }
        Ok(())
    }
}

fn default_ftp_port() -> u16 {
    21
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpTargetConfig {
    pub host: String,
    #[serde(default = "default_sftp_port")]
    pub port: u16,
    pub remote_root: String,
    pub expected_host_key_sha256: String,
}

impl SftpTargetConfig {
    fn validate(&self) -> Result<(), String> {
        validate_hostname(&self.host)?;
        validate_port(self.port)?;
        validate_remote_root(&self.remote_root)?;
        let fingerprint = self
            .expected_host_key_sha256
            .strip_prefix("SHA256:")
            .unwrap_or(&self.expected_host_key_sha256)
            .trim_end_matches('=');
        if STANDARD_NO_PAD
            .decode(fingerprint)
            .map_or(true, |digest| digest.len() != 32)
        {
            return Err("Fingerprint-ul SFTP SHA-256 este invalid.".to_string());
        }
        Ok(())
    }
}

fn default_sftp_port() -> u16 {
    22
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct S3TargetConfig {
    pub bucket: String,
    #[serde(default)]
    pub prefix: String,
    pub region: String,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub force_path_style: bool,
    #[serde(default)]
    pub allow_insecure_endpoint: bool,
    #[serde(default)]
    pub cache_control: Option<String>,
}

impl S3TargetConfig {
    fn validate(&self) -> Result<(), String> {
        validate_nonempty("Bucket-ul S3", &self.bucket, 255)?;
        validate_remote_prefix(&self.prefix)?;
        validate_nonempty("Regiunea S3", &self.region, 128)?;
        if let Some(endpoint) = self.endpoint.as_deref() {
            let endpoint = Url::parse(endpoint)
                .map_err(|_| "Endpoint-ul S3 configurat este invalid.".to_string())?;
            if !matches!(endpoint.scheme(), "https" | "http") || endpoint.host_str().is_none() {
                return Err("Endpoint-ul S3 trebuie să fie un URL HTTP(S) absolut.".to_string());
            }
            if endpoint.scheme() == "http" && !self.allow_insecure_endpoint {
                return Err("Endpoint-ul S3 HTTP necesită allowInsecureEndpoint=true.".to_string());
            }
            if endpoint.query().is_some() || endpoint.fragment().is_some() {
                return Err("Endpoint-ul S3 nu poate conține query sau fragment.".to_string());
            }
        }
        if let Some(cache_control) = self.cache_control.as_deref() {
            validate_nonempty("Cache-Control S3", cache_control, 1024)?;
            if cache_control.bytes().any(|byte| byte.is_ascii_control()) {
                return Err("Cache-Control S3 conține caractere de control.".to_string());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudflarePagesTargetConfig {
    pub account_id: String,
    pub project_name: String,
    #[serde(default)]
    pub branch: Option<String>,
}

impl CloudflarePagesTargetConfig {
    fn validate(&self) -> Result<(), String> {
        validate_identifier("Cloudflare account ID", &self.account_id)?;
        validate_host_label("Numele proiectului Cloudflare Pages", &self.project_name)?;
        if let Some(branch) = self.branch.as_deref() {
            validate_nonempty("Branch-ul Cloudflare Pages", branch, 255)?;
            if branch.bytes().any(|byte| byte.is_ascii_control()) {
                return Err("Branch-ul Cloudflare Pages conține caractere de control.".to_string());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeployActionKind {
    Upload,
    Skip,
    Delete,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeployDeleteOrigin {
    Managed,
    Unmanaged,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployAction {
    pub kind: DeployActionKind,
    pub path: String,
    pub size_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delete_origin: Option<DeployDeleteOrigin>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployPlan {
    pub schema_version: u32,
    pub plan_token: String,
    pub settings_revision: u64,
    pub target_id: String,
    pub provider: DeployProviderKind,
    pub artifact_id: String,
    pub preflight_token: String,
    pub build_token: String,
    pub upload_files: u64,
    pub upload_bytes: u64,
    pub skipped_files: u64,
    pub delete_files: u64,
    #[serde(default)]
    pub managed_delete_files: u64,
    #[serde(default)]
    pub unmanaged_delete_files: u64,
    pub actions: Vec<DeployAction>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployPlanInput {
    pub target_id: String,
    pub expected_build_token: String,
    pub expected_artifact_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployExecutionInput {
    pub target_id: String,
    pub expected_settings_revision: u64,
    pub expected_plan_token: String,
    pub expected_preflight_token: String,
    pub expected_build_token: String,
    pub expected_artifact_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeployProgressPhase {
    Preparing,
    Inventory,
    Uploading,
    Deleting,
    Activating,
    InvalidatingCache,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployProgressEvent {
    pub schema_version: u32,
    pub operation_id: String,
    pub target_id: String,
    pub provider: DeployProviderKind,
    pub phase: DeployProgressPhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_path: Option<String>,
    pub completed_files: u64,
    pub total_files: u64,
    pub completed_bytes: u64,
    pub total_bytes: u64,
    pub timestamp_ms: u128,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployConnectionTestReceipt {
    pub schema_version: u32,
    pub target_id: String,
    pub provider: DeployProviderKind,
    pub checked_at_ms: u128,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_remote_objects: Option<u64>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeployReceiptStatus {
    Completed,
    Failed,
    Cancelled,
    Partial,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployReceipt {
    pub schema_version: u32,
    pub operation_id: String,
    pub target_id: String,
    pub provider: DeployProviderKind,
    pub artifact_id: String,
    pub plan_token: String,
    pub settings_revision: u64,
    pub status: DeployReceiptStatus,
    pub started_at_ms: u128,
    pub completed_at_ms: u128,
    pub uploaded_files: u64,
    pub uploaded_bytes: u64,
    pub skipped_files: u64,
    pub deleted_files: u64,
    #[serde(default)]
    pub deleted_managed_files: u64,
    #[serde(default)]
    pub deleted_unmanaged_files: u64,
    pub remote_manifest_published: bool,
    pub cache_invalidated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment_url: Option<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

impl std::fmt::Display for DeployReceipt {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Deploy {} {:?}: {} uploaduri, {} omise, {} șterse ({} administrate, {} neadministrate), {} bytes.",
            self.target_id,
            self.status,
            self.uploaded_files,
            self.skipped_files,
            self.deleted_files,
            self.deleted_managed_files,
            self.deleted_unmanaged_files,
            self.uploaded_bytes
        )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeployErrorCode {
    InvalidConfiguration,
    MissingCredentials,
    ArtifactUnavailable,
    ConnectionFailed,
    RemoteInventoryFailed,
    UploadFailed,
    DeleteFailed,
    ActivationFailed,
    CacheInvalidationFailed,
    Cancelled,
    Internal,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployCommandError {
    pub schema_version: u32,
    pub code: DeployErrorCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<Box<DeployReceipt>>,
}

impl DeployCommandError {
    pub fn new(code: DeployErrorCode, message: impl Into<String>) -> Self {
        Self {
            schema_version: DEPLOY_ERROR_SCHEMA_VERSION,
            code,
            message: message.into(),
            receipt: None,
        }
    }

    pub fn with_receipt(mut self, receipt: DeployReceipt) -> Self {
        self.receipt = Some(Box::new(receipt));
        self
    }
}

impl std::fmt::Display for DeployCommandError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

fn validate_identifier(label: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(format!(
            "{label} trebuie să conțină 1-128 caractere ASCII alfanumerice, '-', '_' sau '.'."
        ));
    }
    Ok(())
}

fn validate_display_name(value: &str) -> Result<(), String> {
    validate_nonempty("Numele țintei", value.trim(), 128)?;
    if value.bytes().any(|byte| byte.is_ascii_control()) {
        return Err("Numele țintei conține caractere de control.".to_string());
    }
    Ok(())
}

fn validate_nonempty(label: &str, value: &str, max_bytes: usize) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > max_bytes {
        return Err(format!("{label} trebuie să conțină 1-{max_bytes} bytes."));
    }
    Ok(())
}

fn validate_hostname(value: &str) -> Result<(), String> {
    validate_nonempty("Host-ul", value, 253)?;
    if value.bytes().any(|byte| {
        !(byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b':' | b'[' | b']'))
    }) {
        return Err("Host-ul conține caractere invalide.".to_string());
    }
    Ok(())
}

fn validate_host_label(label: &str, value: &str) -> Result<(), String> {
    validate_nonempty(label, value, 128)?;
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(format!("{label} conține caractere invalide."));
    }
    Ok(())
}

fn validate_port(port: u16) -> Result<(), String> {
    if port == 0 {
        return Err("Portul providerului nu poate fi 0.".to_string());
    }
    Ok(())
}

pub fn validate_remote_prefix(prefix: &str) -> Result<(), String> {
    if prefix.is_empty() {
        return Ok(());
    }
    if prefix.len() > 1024
        || prefix.starts_with('/')
        || prefix.ends_with('/')
        || prefix.contains('\\')
        || prefix.bytes().any(|byte| byte.is_ascii_control())
        || prefix
            .split('/')
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
    {
        return Err(
            "Prefixul remote trebuie să fie relativ, normalizat și fără '.', '..' sau backslash."
                .to_string(),
        );
    }
    Ok(())
}

fn validate_remote_root(root: &str) -> Result<(), String> {
    if root.is_empty()
        || root.len() > 2048
        || !root.starts_with('/')
        || (root.len() > 1 && root.ends_with('/'))
        || root.contains("//")
        || root.contains('\\')
        || root.bytes().any(|byte| byte.is_ascii_control())
        || root
            .split('/')
            .skip(1)
            .any(|segment| matches!(segment, "." | ".."))
    {
        return Err(
            "Root-ul remote trebuie să fie absolut și normalizat, fără '.' sau '..'.".to_string(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(provider: DeployTargetProvider) -> DeployTarget {
        DeployTarget {
            id: "production".to_string(),
            name: "Production".to_string(),
            credential_ref: "production-credentials".to_string(),
            cleanup_policy: DeployCleanupPolicy::ManagedOnly,
            provider,
        }
    }

    #[test]
    fn settings_require_unique_targets_and_valid_active_target() {
        let bunny = target(DeployTargetProvider::Bunny(BunnyTargetConfig {
            storage_zone: "site".to_string(),
            storage_region: "de".to_string(),
            pull_zone_id: "42".to_string(),
            remote_prefix: String::new(),
        }));
        let settings = DeploySettings {
            active_target_id: Some(bunny.id.clone()),
            targets: vec![bunny],
            ..DeploySettings::default()
        };
        assert!(settings.validate().is_ok());

        let duplicate = DeploySettings {
            targets: vec![settings.targets[0].clone(), settings.targets[0].clone()],
            ..settings.clone()
        };
        assert!(duplicate.validate().unwrap_err().contains("mai multe ori"));

        let stale = DeploySettings {
            active_target_id: Some("missing".to_string()),
            ..settings
        };
        assert!(stale.validate().unwrap_err().contains("nu există"));
    }

    #[test]
    fn legacy_target_without_cleanup_policy_defaults_to_managed_only() {
        let target: DeployTarget = serde_json::from_value(serde_json::json!({
            "id": "production",
            "name": "Production",
            "credentialRef": "production-credentials",
            "provider": "bunny",
            "config": {
                "storageZone": "site",
                "storageRegion": "de",
                "pullZoneId": "42",
                "remotePrefix": ""
            }
        }))
        .unwrap();
        assert_eq!(target.cleanup_policy, DeployCleanupPolicy::ManagedOnly);
    }

    #[test]
    fn cloudflare_pages_rejects_mirror_destination() {
        let mut pages = target(DeployTargetProvider::CloudflarePages(
            CloudflarePagesTargetConfig {
                account_id: "0123456789abcdef".to_string(),
                project_name: "pana-site".to_string(),
                branch: None,
            },
        ));
        pages.cleanup_policy = DeployCleanupPolicy::MirrorDestination;
        assert!(pages.validate().unwrap_err().contains("versiuni atomice"));
    }

    #[test]
    fn insecure_ftp_and_http_s3_require_explicit_opt_in() {
        let ftp = target(DeployTargetProvider::Ftp(FtpTargetConfig {
            host: "ftp.example.test".to_string(),
            port: 21,
            remote_root: "/public_html".to_string(),
            security: FtpSecurityMode::Plain,
            allow_insecure_ftp: false,
        }));
        assert!(ftp.validate().unwrap_err().contains("allowInsecureFtp"));

        let s3 = target(DeployTargetProvider::S3(S3TargetConfig {
            bucket: "site".to_string(),
            prefix: String::new(),
            region: "us-east-1".to_string(),
            endpoint: Some("http://127.0.0.1:9000".to_string()),
            force_path_style: true,
            allow_insecure_endpoint: false,
            cache_control: None,
        }));
        assert!(s3.validate().unwrap_err().contains("allowInsecureEndpoint"));
    }

    #[test]
    fn provider_capabilities_do_not_pretend_all_targets_are_filesystems() {
        let pages = target(DeployTargetProvider::CloudflarePages(
            CloudflarePagesTargetConfig {
                account_id: "0123456789abcdef".to_string(),
                project_name: "pana-site".to_string(),
                branch: None,
            },
        ));
        let capabilities = pages.capabilities();
        assert!(capabilities.atomic_activation);
        assert!(!capabilities.remote_inventory);
        assert!(!capabilities.delete_stale);
    }

    #[test]
    fn capability_dispatch_matches_each_provider_semantics() {
        let bunny = DeployTargetProvider::Bunny(BunnyTargetConfig {
            storage_zone: "site".to_string(),
            storage_region: "de".to_string(),
            pull_zone_id: "42".to_string(),
            remote_prefix: String::new(),
        })
        .capabilities();
        let ftp = DeployTargetProvider::Ftp(FtpTargetConfig {
            host: "ftp.example.test".to_string(),
            port: 21,
            remote_root: "/public_html".to_string(),
            security: FtpSecurityMode::FtpsExplicit,
            allow_insecure_ftp: false,
        })
        .capabilities();
        let sftp = DeployTargetProvider::Sftp(SftpTargetConfig {
            host: "sftp.example.test".to_string(),
            port: 22,
            remote_root: "/srv/site".to_string(),
            expected_host_key_sha256: format!("SHA256:{}", STANDARD_NO_PAD.encode([7u8; 32])),
        })
        .capabilities();
        let s3 = DeployTargetProvider::S3(S3TargetConfig {
            bucket: "site".to_string(),
            prefix: String::new(),
            region: "us-east-1".to_string(),
            endpoint: None,
            force_path_style: false,
            allow_insecure_endpoint: false,
            cache_control: None,
        })
        .capabilities();

        assert!(bunny.remote_inventory && bunny.delete_stale && bunny.cache_invalidation);
        assert!(ftp.remote_inventory && ftp.delete_stale && !ftp.cache_invalidation);
        assert!(sftp.remote_inventory && sftp.delete_stale && !sftp.atomic_activation);
        assert!(s3.remote_inventory && s3.delete_stale && s3.metadata_headers);
        assert!([bunny, ftp, sftp, s3]
            .iter()
            .all(|capabilities| capabilities.connection_test));
    }

    #[test]
    fn remote_paths_reject_traversal_and_backslashes() {
        for invalid in ["/absolute", "a/../b", "a//b", "a\\b", "a/./b"] {
            assert!(
                validate_remote_prefix(invalid).is_err(),
                "accepted {invalid}"
            );
        }
        assert!(validate_remote_prefix("assets/site").is_ok());
    }

    #[test]
    fn serialized_target_contains_reference_but_no_credential_material() {
        let target = target(DeployTargetProvider::S3(S3TargetConfig {
            bucket: "site".to_string(),
            prefix: "production".to_string(),
            region: "auto".to_string(),
            endpoint: Some("https://account.r2.cloudflarestorage.com".to_string()),
            force_path_style: false,
            allow_insecure_endpoint: false,
            cache_control: None,
        }));
        let json = serde_json::to_string(&target).unwrap();
        assert!(json.contains("credentialRef"));
        for forbidden in ["secretAccessKey", "password", "privateKey", "apiToken"] {
            assert!(!json.contains(forbidden));
        }
    }
}
