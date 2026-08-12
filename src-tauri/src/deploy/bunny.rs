use reqwest::{blocking::Client, StatusCode, Url};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, VecDeque},
    io::Read,
    path::Path,
    time::Duration,
};

use super::{
    artifact::{DeployArtifactFile, DeployArtifactManifest},
    credentials::StoredDeployCredential,
    engine::DeployProgressReporter,
    model::{
        validate_remote_prefix, BunnyTargetConfig, DeployActionKind, DeployCleanupPolicy,
        DeployCommandError, DeployDeleteOrigin, DeployErrorCode, DeployPlan, DeployProgressPhase,
        DeployReceipt, DeployReceiptStatus, DeployTarget, DeployTargetProvider,
        DEPLOY_RECEIPT_SCHEMA_VERSION,
    },
    remote_manifest::{
        prepare_sync_plan, PreparedSync, RemoteInventoryFile, MAX_REMOTE_INVENTORY_FILES,
        MAX_REMOTE_MANIFEST_BYTES, REMOTE_MANIFEST_FILE_NAME,
    },
    retry::retry_idempotent,
};

#[cfg(test)]
use super::{
    artifact::build_deploy_artifact_manifest,
    env::{env_require, read_env_from_root},
};

const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

pub(crate) fn plan_bunny_deploy(
    target: &DeployTarget,
    settings_revision: u64,
    artifact: &DeployArtifactManifest,
    credential: &StoredDeployCredential,
) -> Result<DeployPlan, DeployCommandError> {
    let runtime = BunnyRuntimeConfig::from_target(target, credential).map_err(|message| {
        DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
    })?;
    let endpoints = BunnyEndpoints::production(&runtime.region).map_err(|message| {
        DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
    })?;
    let transport = ReqwestBunnyTransport {
        client: bunny_client()
            .map_err(|message| DeployCommandError::new(DeployErrorCode::Internal, message))?,
    };
    plan_bunny_with_transport(
        &transport,
        &endpoints,
        &runtime,
        target,
        settings_revision,
        artifact,
    )
    .map(|prepared| prepared.plan)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_bunny_deploy(
    operation_id: &str,
    target: &DeployTarget,
    settings_revision: u64,
    expected_plan_token: &str,
    artifact: DeployArtifactManifest,
    credential: StoredDeployCredential,
    is_cancelled: &dyn Fn() -> bool,
    progress: &DeployProgressReporter<'_>,
) -> Result<DeployReceipt, DeployCommandError> {
    let runtime = BunnyRuntimeConfig::from_target(target, &credential).map_err(|message| {
        DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
    })?;
    let endpoints = BunnyEndpoints::production(&runtime.region).map_err(|message| {
        DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
    })?;
    let transport = ReqwestBunnyTransport {
        client: bunny_client()
            .map_err(|message| DeployCommandError::new(DeployErrorCode::Internal, message))?,
    };
    execute_bunny_with_transport(
        &transport,
        &endpoints,
        &runtime,
        operation_id,
        target,
        settings_revision,
        expected_plan_token,
        artifact,
        is_cancelled,
        progress,
    )
}

pub(crate) fn test_bunny_connection(
    target: &DeployTarget,
    credential: &StoredDeployCredential,
) -> Result<(), DeployCommandError> {
    let runtime = BunnyRuntimeConfig::from_target(target, credential).map_err(|message| {
        DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
    })?;
    let endpoints = BunnyEndpoints::production(&runtime.region).map_err(|message| {
        DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
    })?;
    let transport = ReqwestBunnyTransport {
        client: bunny_client()
            .map_err(|message| DeployCommandError::new(DeployErrorCode::Internal, message))?,
    };
    let manifest_url = storage_object_url(
        &endpoints.storage_base,
        &runtime.zone,
        &runtime.remote_prefix,
        REMOTE_MANIFEST_FILE_NAME,
    )
    .map_err(|message| DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message))?;
    transport
        .download_optional(manifest_url, &runtime.storage_key)
        .map_err(|message| DeployCommandError::new(DeployErrorCode::ConnectionFailed, message))?;
    let mut pull_zone_url = endpoints.api_base;
    pull_zone_url
        .path_segments_mut()
        .map_err(|_| {
            DeployCommandError::new(
                DeployErrorCode::InvalidConfiguration,
                "Endpointul Bunny CDN nu poate primi segmente de path.",
            )
        })?
        .pop_if_empty()
        .push("pullzone")
        .push(&runtime.pull_zone_id);
    let response = transport
        .client
        .get(pull_zone_url)
        .header("AccessKey", &runtime.cdn_key)
        .send()
        .map_err(|error| {
            DeployCommandError::new(
                DeployErrorCode::ConnectionFailed,
                format!("Testul Bunny CDN a eșuat: {error}"),
            )
        })?;
    if !response.status().is_success() {
        return Err(DeployCommandError::new(
            DeployErrorCode::ConnectionFailed,
            format!(
                "Bunny CDN a răspuns HTTP {} la verificarea Pull Zone.",
                response.status()
            ),
        ));
    }
    Ok(())
}

#[derive(Clone)]
struct BunnyRuntimeConfig {
    zone: String,
    storage_key: String,
    region: String,
    pull_zone_id: String,
    cdn_key: String,
    remote_prefix: String,
}

impl BunnyRuntimeConfig {
    fn from_target(
        target: &DeployTarget,
        credential: &StoredDeployCredential,
    ) -> Result<Self, String> {
        let DeployTargetProvider::Bunny(BunnyTargetConfig {
            storage_zone,
            storage_region,
            pull_zone_id,
            remote_prefix,
        }) = &target.provider
        else {
            return Err("Ținta nu este configurată pentru Bunny.".to_string());
        };
        let StoredDeployCredential::Bunny {
            storage_key,
            cdn_api_key,
        } = credential
        else {
            return Err("Credentialele țintei Bunny au un tip incompatibil.".to_string());
        };
        Ok(Self {
            zone: storage_zone.clone(),
            storage_key: storage_key.clone(),
            region: storage_region.clone(),
            pull_zone_id: pull_zone_id.clone(),
            cdn_key: cdn_api_key.clone(),
            remote_prefix: remote_prefix.clone(),
        })
    }
}

#[cfg(test)]
fn deploy_project_with_transport<T, F>(
    project_root: &Path,
    zola_root: &Path,
    env_root: &Path,
    transport: &T,
    endpoints: F,
) -> Result<String, String>
where
    T: BunnyTransport,
    F: FnOnce(&BunnyCredentials) -> Result<BunnyEndpoints, String>,
{
    deploy_project_with_transport_cancellable(
        project_root,
        zola_root,
        env_root,
        transport,
        endpoints,
        || false,
    )
}

#[cfg(test)]
fn deploy_project_with_transport_cancellable<T, F, C>(
    project_root: &Path,
    zola_root: &Path,
    env_root: &Path,
    transport: &T,
    endpoints: F,
    is_cancelled: C,
) -> Result<String, String>
where
    T: BunnyTransport,
    F: FnOnce(&BunnyCredentials) -> Result<BunnyEndpoints, String>,
    C: Fn() -> bool,
{
    // Ordering is a safety contract: the complete bounded/no-follow artifact
    // must exist in memory before the transport can receive an upload call.
    let manifest = build_deploy_artifact_manifest(project_root, zola_root)?;
    let credentials = BunnyCredentials::from_root(env_root)?;
    let endpoints = endpoints(&credentials)?;
    upload_manifest_and_purge(transport, endpoints, credentials, manifest, is_cancelled)
}

#[cfg(test)]
#[derive(Debug)]
struct BunnyCredentials {
    zone: String,
    storage_key: String,
    pull_zone_id: String,
    cdn_key: String,
}

#[cfg(test)]
impl BunnyCredentials {
    fn from_root(root: &Path) -> Result<Self, String> {
        let env = read_env_from_root(root)?;
        Ok(Self {
            zone: env_require(&env, "BUNNY_STORAGE_ZONE")?,
            storage_key: env_require(&env, "BUNNY_STORAGE_KEY")?,
            pull_zone_id: env_require(&env, "BUNNY_PULL_ZONE_ID")?,
            cdn_key: env_require(&env, "BUNNY_CDN_API_KEY")?,
        })
    }
}

#[derive(Clone, Debug)]
struct BunnyEndpoints {
    storage_base: Url,
    api_base: Url,
}

impl BunnyEndpoints {
    fn production(region: &str) -> Result<Self, String> {
        let host = storage_host(region)?;
        Ok(Self {
            storage_base: Url::parse(&format!("https://{host}/"))
                .map_err(|error| format!("Endpointul Bunny Storage este invalid: {error}."))?,
            api_base: Url::parse("https://api.bunny.net/")
                .map_err(|error| format!("Endpointul Bunny CDN este invalid: {error}."))?,
        })
    }
}

fn bunny_client() -> Result<Client, String> {
    Client::builder()
        .connect_timeout(HTTP_CONNECT_TIMEOUT)
        .timeout(HTTP_REQUEST_TIMEOUT)
        .build()
        .map_err(|error| format!("Clientul HTTP Bunny nu poate fi inițializat: {error}."))
}

trait BunnyTransport {
    fn download_optional(&self, url: Url, access_key: &str) -> Result<Option<Vec<u8>>, String>;

    fn list_directory(&self, url: Url, access_key: &str)
        -> Result<Vec<BunnyStorageObject>, String>;

    #[allow(clippy::too_many_arguments)]
    fn upload(
        &self,
        url: Url,
        access_key: &str,
        content_type: &'static str,
        checksum: &str,
        bytes: Vec<u8>,
    ) -> Result<(), String>;

    fn delete(&self, url: Url, access_key: &str) -> Result<(), String>;

    fn purge(&self, url: Url, access_key: &str) -> Result<(), String>;
}

struct ReqwestBunnyTransport {
    client: Client,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
struct BunnyStorageObject {
    object_name: String,
    length: i64,
    is_directory: bool,
}

impl BunnyTransport for ReqwestBunnyTransport {
    fn download_optional(&self, url: Url, access_key: &str) -> Result<Option<Vec<u8>>, String> {
        let response = self
            .client
            .get(url)
            .header("AccessKey", access_key)
            .send()
            .map_err(|error| format!("request-ul HTTP Bunny Storage a eșuat: {error}"))?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(format!(
                "Bunny Storage a răspuns HTTP {} la citirea manifestului",
                response.status()
            ));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_REMOTE_MANIFEST_BYTES as u64)
        {
            return Err("Manifestul Bunny remote depășește limita sigură.".to_string());
        }
        let mut bytes = Vec::new();
        response
            .take(MAX_REMOTE_MANIFEST_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("Manifestul Bunny remote nu poate fi citit: {error}."))?;
        if bytes.len() > MAX_REMOTE_MANIFEST_BYTES {
            return Err("Manifestul Bunny remote depășește limita sigură.".to_string());
        }
        Ok(Some(bytes))
    }

    fn list_directory(
        &self,
        url: Url,
        access_key: &str,
    ) -> Result<Vec<BunnyStorageObject>, String> {
        let response = self
            .client
            .get(url)
            .header("AccessKey", access_key)
            .send()
            .map_err(|error| format!("listarea Bunny Storage a eșuat: {error}"))?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(Vec::new());
        }
        if !response.status().is_success() {
            return Err(format!(
                "Bunny Storage a răspuns HTTP {} la inventarierea destinației",
                response.status()
            ));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_REMOTE_MANIFEST_BYTES as u64)
        {
            return Err("O pagină a inventarului Bunny depășește limita sigură.".to_string());
        }
        let mut bytes = Vec::new();
        response
            .take(MAX_REMOTE_MANIFEST_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Inventarul Bunny remote nu poate fi citit.".to_string())?;
        if bytes.len() > MAX_REMOTE_MANIFEST_BYTES {
            return Err("O pagină a inventarului Bunny depășește limita sigură.".to_string());
        }
        serde_json::from_slice(&bytes)
            .map_err(|_| "Inventarul Bunny remote nu este JSON valid.".to_string())
    }

    fn upload(
        &self,
        url: Url,
        access_key: &str,
        content_type: &'static str,
        checksum: &str,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        let response = self
            .client
            .put(url)
            .header("AccessKey", access_key)
            .header("Content-Type", content_type)
            .header("Checksum", checksum)
            .body(bytes)
            .send()
            .map_err(|error| format!("request-ul HTTP a eșuat: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "Bunny Storage a răspuns HTTP {}",
                response.status()
            ));
        }
        Ok(())
    }

    fn delete(&self, url: Url, access_key: &str) -> Result<(), String> {
        let response = self
            .client
            .delete(url)
            .header("AccessKey", access_key)
            .send()
            .map_err(|error| format!("request-ul HTTP Bunny Storage a eșuat: {error}"))?;
        if response.status() == StatusCode::NOT_FOUND || response.status().is_success() {
            return Ok(());
        }
        Err(format!(
            "Bunny Storage a răspuns HTTP {} la ștergere",
            response.status()
        ))
    }

    fn purge(&self, url: Url, access_key: &str) -> Result<(), String> {
        let response = self
            .client
            .post(url)
            .header("AccessKey", access_key)
            .header("Content-Length", "0")
            .send()
            .map_err(|error| format!("request-ul HTTP a eșuat: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("Bunny CDN a răspuns HTTP {}", response.status()));
        }
        Ok(())
    }
}

fn plan_bunny_with_transport<T: BunnyTransport>(
    transport: &T,
    endpoints: &BunnyEndpoints,
    runtime: &BunnyRuntimeConfig,
    target: &DeployTarget,
    settings_revision: u64,
    artifact: &DeployArtifactManifest,
) -> Result<PreparedSync, DeployCommandError> {
    let manifest_url = storage_object_url(
        &endpoints.storage_base,
        &runtime.zone,
        &runtime.remote_prefix,
        REMOTE_MANIFEST_FILE_NAME,
    )
    .map_err(|message| DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message))?;
    let remote_manifest = retry_idempotent(|| {
        transport.download_optional(manifest_url.clone(), &runtime.storage_key)
    })
    .map_err(|message| DeployCommandError::new(DeployErrorCode::RemoteInventoryFailed, message))?;
    let remote_inventory = if target.cleanup_policy == DeployCleanupPolicy::MirrorDestination {
        Some(
            list_bunny_inventory(transport, endpoints, runtime).map_err(|message| {
                DeployCommandError::new(DeployErrorCode::RemoteInventoryFailed, message)
            })?,
        )
    } else {
        None
    };
    prepare_sync_plan(
        target,
        settings_revision,
        artifact,
        remote_manifest.as_deref(),
        remote_inventory.as_deref(),
    )
    .map_err(|message| DeployCommandError::new(DeployErrorCode::RemoteInventoryFailed, message))
}

fn list_bunny_inventory<T: BunnyTransport>(
    transport: &T,
    endpoints: &BunnyEndpoints,
    runtime: &BunnyRuntimeConfig,
) -> Result<Vec<RemoteInventoryFile>, String> {
    let mut directories = VecDeque::from([String::new()]);
    let mut inventory = Vec::new();
    let mut observed_entries = 0usize;
    while let Some(directory) = directories.pop_front() {
        let url = storage_directory_url(
            &endpoints.storage_base,
            &runtime.zone,
            &runtime.remote_prefix,
            &directory,
        )?;
        let entries =
            retry_idempotent(|| transport.list_directory(url.clone(), &runtime.storage_key))?;
        for entry in entries {
            observed_entries += 1;
            if observed_entries > MAX_REMOTE_INVENTORY_FILES {
                return Err(format!(
                    "Inventarul Bunny depășește limita sigură de {MAX_REMOTE_INVENTORY_FILES} intrări."
                ));
            }
            let name = entry.object_name;
            if name.is_empty()
                || matches!(name.as_str(), "." | "..")
                || name.contains('/')
                || name.contains('\\')
                || name.bytes().any(|byte| byte.is_ascii_control())
            {
                return Err("Inventarul Bunny conține un nume de obiect nesigur.".to_string());
            }
            let path = if directory.is_empty() {
                name
            } else {
                format!("{directory}/{name}")
            };
            validate_remote_prefix(&path)?;
            if entry.is_directory {
                directories.push_back(path);
            } else {
                let size_bytes = u64::try_from(entry.length)
                    .map_err(|_| "Inventarul Bunny conține o dimensiune negativă.".to_string())?;
                inventory.push(RemoteInventoryFile { path, size_bytes });
            }
        }
    }
    Ok(inventory)
}

#[allow(clippy::too_many_arguments)]
fn execute_bunny_with_transport<T: BunnyTransport>(
    transport: &T,
    endpoints: &BunnyEndpoints,
    runtime: &BunnyRuntimeConfig,
    operation_id: &str,
    target: &DeployTarget,
    settings_revision: u64,
    expected_plan_token: &str,
    artifact: DeployArtifactManifest,
    is_cancelled: &dyn Fn() -> bool,
    progress: &DeployProgressReporter<'_>,
) -> Result<DeployReceipt, DeployCommandError> {
    let started_at_ms = crate::kernel::observability::now_ms();
    progress.emit(
        DeployProgressPhase::Inventory,
        None,
        0,
        0,
        0,
        artifact.total_bytes,
    );
    let prepared = plan_bunny_with_transport(
        transport,
        endpoints,
        runtime,
        target,
        settings_revision,
        &artifact,
    )?;
    if prepared.plan.plan_token != expected_plan_token {
        return Err(DeployCommandError::new(
            DeployErrorCode::InvalidConfiguration,
            "Planul deploy nu mai corespunde artifactului, configurației sau manifestului remote. Recalculează planul.",
        ));
    }

    let total_mutations = prepared.plan.upload_files + prepared.plan.delete_files;
    let mut receipt = DeployReceipt {
        schema_version: DEPLOY_RECEIPT_SCHEMA_VERSION,
        operation_id: operation_id.to_string(),
        target_id: target.id.clone(),
        provider: target.provider_kind(),
        artifact_id: artifact.artifact_id.clone(),
        plan_token: prepared.plan.plan_token.clone(),
        settings_revision,
        status: DeployReceiptStatus::Failed,
        started_at_ms,
        completed_at_ms: started_at_ms,
        uploaded_files: 0,
        uploaded_bytes: 0,
        skipped_files: prepared.plan.skipped_files,
        deleted_files: 0,
        deleted_managed_files: 0,
        deleted_unmanaged_files: 0,
        remote_manifest_published: false,
        cache_invalidated: false,
        deployment_id: None,
        deployment_url: None,
        warnings: prepared.plan.warnings.clone(),
    };
    let files: BTreeMap<String, DeployArtifactFile> = artifact
        .files
        .into_iter()
        .map(|file| (file.relative_path.clone(), file))
        .collect();
    let mut completed_mutations = 0u64;

    for action in prepared
        .plan
        .actions
        .iter()
        .filter(|action| action.kind == DeployActionKind::Upload)
    {
        if is_cancelled() {
            return Err(cancelled_bunny_error(receipt));
        }
        progress.emit(
            DeployProgressPhase::Uploading,
            Some(action.path.clone()),
            completed_mutations,
            total_mutations,
            receipt.uploaded_bytes,
            prepared.plan.upload_bytes,
        );
        let file = files.get(&action.path).ok_or_else(|| {
            DeployCommandError::new(
                DeployErrorCode::Internal,
                "Planul Bunny referă un fișier care nu există în artifactul capturat.",
            )
        })?;
        let url = storage_object_url(
            &endpoints.storage_base,
            &runtime.zone,
            &runtime.remote_prefix,
            &file.relative_path,
        )
        .map_err(|message| {
            DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
        })?;
        if let Err(message) = retry_idempotent(|| {
            transport.upload(
                url.clone(),
                &runtime.storage_key,
                mime_for_extension(Path::new(&file.relative_path)),
                &file.sha256_uppercase,
                file.bytes.clone(),
            )
        }) {
            receipt.completed_at_ms = crate::kernel::observability::now_ms();
            receipt.status = mutation_failure_status(&receipt);
            return Err(
                DeployCommandError::new(DeployErrorCode::UploadFailed, message)
                    .with_receipt(receipt),
            );
        }
        receipt.uploaded_files += 1;
        receipt.uploaded_bytes = receipt
            .uploaded_bytes
            .saturating_add(file.bytes.len() as u64);
        completed_mutations += 1;
    }

    for action in prepared
        .plan
        .actions
        .iter()
        .filter(|action| action.kind == DeployActionKind::Delete)
    {
        if is_cancelled() {
            return Err(cancelled_bunny_error(receipt));
        }
        progress.emit(
            DeployProgressPhase::Deleting,
            Some(action.path.clone()),
            completed_mutations,
            total_mutations,
            receipt.uploaded_bytes,
            prepared.plan.upload_bytes,
        );
        let url = storage_object_url(
            &endpoints.storage_base,
            &runtime.zone,
            &runtime.remote_prefix,
            &action.path,
        )
        .map_err(|message| {
            DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
        })?;
        if let Err(message) =
            retry_idempotent(|| transport.delete(url.clone(), &runtime.storage_key))
        {
            receipt.completed_at_ms = crate::kernel::observability::now_ms();
            receipt.status = mutation_failure_status(&receipt);
            return Err(
                DeployCommandError::new(DeployErrorCode::DeleteFailed, message)
                    .with_receipt(receipt),
            );
        }
        receipt.deleted_files += 1;
        match action.delete_origin {
            Some(DeployDeleteOrigin::Unmanaged) => receipt.deleted_unmanaged_files += 1,
            _ => receipt.deleted_managed_files += 1,
        }
        completed_mutations += 1;
    }

    if is_cancelled() {
        return Err(cancelled_bunny_error(receipt));
    }
    progress.emit(
        DeployProgressPhase::Activating,
        Some(REMOTE_MANIFEST_FILE_NAME.to_string()),
        completed_mutations,
        total_mutations,
        receipt.uploaded_bytes,
        prepared.plan.upload_bytes,
    );
    let manifest_url = storage_object_url(
        &endpoints.storage_base,
        &runtime.zone,
        &runtime.remote_prefix,
        REMOTE_MANIFEST_FILE_NAME,
    )
    .map_err(|message| DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message))?;
    let manifest_checksum = uppercase_sha256(&prepared.next_manifest_bytes);
    if let Err(message) = retry_idempotent(|| {
        transport.upload(
            manifest_url.clone(),
            &runtime.storage_key,
            "application/json",
            &manifest_checksum,
            prepared.next_manifest_bytes.clone(),
        )
    }) {
        receipt.completed_at_ms = crate::kernel::observability::now_ms();
        receipt.status = mutation_failure_status(&receipt);
        return Err(
            DeployCommandError::new(DeployErrorCode::UploadFailed, message).with_receipt(receipt),
        );
    }
    receipt.remote_manifest_published = true;

    if is_cancelled() {
        return Err(cancelled_bunny_error(receipt));
    }
    progress.emit(
        DeployProgressPhase::InvalidatingCache,
        None,
        completed_mutations,
        total_mutations,
        receipt.uploaded_bytes,
        prepared.plan.upload_bytes,
    );
    if let Err(message) = retry_idempotent(|| {
        purge_cdn_cache(
            transport,
            &endpoints.api_base,
            &runtime.pull_zone_id,
            &runtime.cdn_key,
        )
    }) {
        receipt.completed_at_ms = crate::kernel::observability::now_ms();
        receipt.status = DeployReceiptStatus::Partial;
        return Err(
            DeployCommandError::new(DeployErrorCode::CacheInvalidationFailed, message)
                .with_receipt(receipt),
        );
    }
    receipt.cache_invalidated = true;
    receipt.status = DeployReceiptStatus::Completed;
    receipt.completed_at_ms = crate::kernel::observability::now_ms();
    progress.emit(
        DeployProgressPhase::Completed,
        None,
        total_mutations,
        total_mutations,
        receipt.uploaded_bytes,
        prepared.plan.upload_bytes,
    );
    Ok(receipt)
}

fn cancelled_bunny_error(mut receipt: DeployReceipt) -> DeployCommandError {
    receipt.completed_at_ms = crate::kernel::observability::now_ms();
    receipt.status = if receipt.uploaded_files > 0
        || receipt.deleted_files > 0
        || receipt.remote_manifest_published
    {
        DeployReceiptStatus::Partial
    } else {
        DeployReceiptStatus::Cancelled
    };
    DeployCommandError::new(
        DeployErrorCode::Cancelled,
        "Deploy-ul Bunny a fost anulat; consultă receipt-ul pentru starea remote.",
    )
    .with_receipt(receipt)
}

fn mutation_failure_status(receipt: &DeployReceipt) -> DeployReceiptStatus {
    if receipt.uploaded_files > 0 || receipt.deleted_files > 0 || receipt.remote_manifest_published
    {
        DeployReceiptStatus::Partial
    } else {
        DeployReceiptStatus::Failed
    }
}

#[cfg(test)]
fn upload_manifest_and_purge<T: BunnyTransport, C: Fn() -> bool>(
    transport: &T,
    endpoints: BunnyEndpoints,
    credentials: BunnyCredentials,
    manifest: DeployArtifactManifest,
    is_cancelled: C,
) -> Result<String, String> {
    let total_files = manifest.files.len();
    let total_bytes = manifest.total_bytes;
    let artifact_root = manifest.root.display().to_string();
    let mut uploaded = 0usize;
    let mut log = String::new();

    for file in manifest.files {
        if is_cancelled() {
            return Err(format!(
                "[publish_cancelled] Deploy Bunny anulat după {uploaded}/{total_files} uploaduri. Cache-ul CDN nu a fost purjat."
            ));
        }
        let remote_path = file.relative_path.clone();
        let url = storage_file_url(&endpoints.storage_base, &credentials.zone, &file)?;
        let content_type = mime_for_extension(Path::new(&file.relative_path));
        transport
            .upload(
                url,
                &credentials.storage_key,
                content_type,
                &file.sha256_uppercase,
                file.bytes,
            )
            .map_err(|error| {
                format!(
                    "Deploy Bunny oprit după {uploaded}/{total_files} uploaduri la {remote_path}: {error}. Cache-ul CDN nu a fost purjat."
                )
            })?;
        uploaded += 1;
        log.push_str(&format!("upload {remote_path}\n"));
    }

    if is_cancelled() {
        return Err(format!(
            "[publish_cancelled] Deploy Bunny anulat după {uploaded}/{total_files} uploaduri. Cache-ul CDN nu a fost purjat."
        ));
    }

    purge_cdn_cache(
        transport,
        &endpoints.api_base,
        &credentials.pull_zone_id,
        &credentials.cdn_key,
    )?;
    log.push_str("CDN cache purged\n");
    Ok(format!(
        "Deploy complet: {uploaded} fișiere / {total_bytes} bytes din {artifact_root}; checksum SHA-256 verificat, purge CDN confirmat.\n\n{log}"
    ))
}

#[cfg(test)]
fn storage_file_url(
    storage_base: &Url,
    zone: &str,
    file: &DeployArtifactFile,
) -> Result<Url, String> {
    storage_object_url(storage_base, zone, "", &file.relative_path)
}

fn storage_object_url(
    storage_base: &Url,
    zone: &str,
    remote_prefix: &str,
    relative_path: &str,
) -> Result<Url, String> {
    let mut url = storage_base.clone();
    let mut segments = url
        .path_segments_mut()
        .map_err(|_| "Endpointul Bunny Storage nu poate primi segmente de path.".to_string())?;
    segments.pop_if_empty().push(zone);
    for segment in remote_prefix
        .split('/')
        .filter(|segment| !segment.is_empty())
    {
        segments.push(segment);
    }
    for segment in relative_path.split('/') {
        segments.push(segment);
    }
    drop(segments);
    Ok(url)
}

fn storage_directory_url(
    storage_base: &Url,
    zone: &str,
    remote_prefix: &str,
    relative_directory: &str,
) -> Result<Url, String> {
    let mut url = storage_base.clone();
    let mut segments = url
        .path_segments_mut()
        .map_err(|_| "Endpointul Bunny Storage nu poate primi segmente de path.".to_string())?;
    segments.pop_if_empty().push(zone);
    for segment in remote_prefix
        .split('/')
        .filter(|segment| !segment.is_empty())
    {
        segments.push(segment);
    }
    for segment in relative_directory
        .split('/')
        .filter(|segment| !segment.is_empty())
    {
        segments.push(segment);
    }
    segments.push("");
    drop(segments);
    Ok(url)
}

fn uppercase_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect()
}

fn purge_cdn_cache<T: BunnyTransport>(
    transport: &T,
    api_base: &Url,
    pull_zone_id: &str,
    cdn_key: &str,
) -> Result<(), String> {
    let mut url = api_base.clone();
    let mut segments = url
        .path_segments_mut()
        .map_err(|_| "Endpointul Bunny CDN nu poate primi segmente de path.".to_string())?;
    segments
        .pop_if_empty()
        .push("pullzone")
        .push(pull_zone_id)
        .push("purgeCache");
    drop(segments);

    transport.purge(url, cdn_key).map_err(|error| {
            format!(
                "Uploadurile au reușit, dar purge-ul CDN a eșuat: {error}. Deploy-ul nu este confirmat complet."
            )
        })?;
    Ok(())
}

fn storage_host(region: &str) -> Result<String, String> {
    let normalized = region.trim().to_ascii_lowercase();
    if !normalized
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err("BUNNY_STORAGE_REGION conține caractere invalide.".to_string());
    }
    Ok(match normalized.as_str() {
        "" | "de" => "storage.bunnycdn.com".to_string(),
        value => format!("{value}.storage.bunnycdn.com"),
    })
}

fn mime_for_extension(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("json") => "application/json",
        Some("xml") => "application/xml",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("txt") => "text/plain; charset=utf-8",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::Digest;
    use std::{
        cell::{Cell, RefCell},
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[derive(Default)]
    struct FakeTransport {
        download: RefCell<Option<Vec<u8>>>,
        directories: RefCell<BTreeMap<String, Vec<BunnyStorageObject>>>,
        uploads: RefCell<Vec<(String, String, Vec<u8>)>>,
        deletes: RefCell<Vec<String>>,
        purge_calls: Cell<usize>,
        fail_download: bool,
        fail_upload: bool,
        fail_upload_at: Option<usize>,
        transient_upload_failures: Cell<usize>,
        fail_delete: bool,
        fail_purge: bool,
    }

    impl BunnyTransport for FakeTransport {
        fn download_optional(
            &self,
            _url: Url,
            _access_key: &str,
        ) -> Result<Option<Vec<u8>>, String> {
            if self.fail_download {
                Err("download injectat eșuat".to_string())
            } else {
                Ok(self.download.borrow().clone())
            }
        }

        fn list_directory(
            &self,
            url: Url,
            _access_key: &str,
        ) -> Result<Vec<BunnyStorageObject>, String> {
            Ok(self
                .directories
                .borrow()
                .get(url.path())
                .cloned()
                .unwrap_or_default())
        }

        fn upload(
            &self,
            url: Url,
            _access_key: &str,
            _content_type: &'static str,
            checksum: &str,
            bytes: Vec<u8>,
        ) -> Result<(), String> {
            if self.transient_upload_failures.get() > 0 {
                self.transient_upload_failures
                    .set(self.transient_upload_failures.get() - 1);
                return Err("upload Bunny temporar eșuat".to_string());
            }
            self.uploads
                .borrow_mut()
                .push((url.to_string(), checksum.to_string(), bytes));
            let upload_index = self.uploads.borrow().len();
            if self.fail_upload
                || self
                    .fail_upload_at
                    .is_some_and(|failure_index| upload_index >= failure_index)
            {
                Err("upload injectat eșuat".to_string())
            } else {
                Ok(())
            }
        }

        fn delete(&self, url: Url, _access_key: &str) -> Result<(), String> {
            self.deletes.borrow_mut().push(url.to_string());
            if self.fail_delete {
                Err("delete injectat eșuat".to_string())
            } else {
                Ok(())
            }
        }

        fn purge(&self, _url: Url, _access_key: &str) -> Result<(), String> {
            self.purge_calls.set(self.purge_calls.get() + 1);
            if self.fail_purge {
                Err("purge injectat eșuat".to_string())
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn checksum_header_value_is_uppercase_sha256() {
        let checksum = sha2::Sha256::digest(b"abc")
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<String>();
        assert_eq!(
            checksum,
            "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"
        );
    }

    #[test]
    fn storage_url_encodes_zone_and_artifact_segments() {
        let file = DeployArtifactFile {
            relative_path: "assets/a b.css".to_string(),
            bytes: Vec::new(),
            sha256_uppercase: String::new(),
        };
        let url = storage_file_url(
            &Url::parse("https://storage.bunnycdn.com/").unwrap(),
            "zone/name",
            &file,
        )
        .unwrap();
        assert_eq!(
            url.as_str(),
            "https://storage.bunnycdn.com/zone%2Fname/assets/a%20b.css"
        );
    }

    #[test]
    fn mirror_inventory_recurses_and_receipt_counts_unmanaged_deletes() {
        let mut target = typed_target();
        target.cleanup_policy = DeployCleanupPolicy::MirrorDestination;
        let credential = typed_credential();
        let runtime = BunnyRuntimeConfig::from_target(&target, &credential).unwrap();
        let artifact = captured_artifact(&[("index.html", b"new")], "sha256:mirror");
        let transport = FakeTransport::default();
        transport.directories.borrow_mut().insert(
            "/site/".to_string(),
            vec![
                BunnyStorageObject {
                    object_name: "legacy.html".to_string(),
                    length: 8,
                    is_directory: false,
                },
                BunnyStorageObject {
                    object_name: "assets".to_string(),
                    length: 0,
                    is_directory: true,
                },
            ],
        );
        transport.directories.borrow_mut().insert(
            "/site/assets/".to_string(),
            vec![BunnyStorageObject {
                object_name: "old.css".to_string(),
                length: 7,
                is_directory: false,
            }],
        );
        let plan = plan_bunny_with_transport(
            &transport,
            &test_endpoints(),
            &runtime,
            &target,
            1,
            &artifact,
        )
        .unwrap();
        assert_eq!(plan.plan.delete_files, 2);
        assert_eq!(plan.plan.unmanaged_delete_files, 2);

        let sink = |_| {};
        let receipt = execute_bunny_with_transport(
            &transport,
            &test_endpoints(),
            &runtime,
            "mirror",
            &target,
            1,
            &plan.plan.plan_token,
            artifact,
            &|| false,
            &DeployProgressReporter::new("mirror", &target, &sink),
        )
        .unwrap();
        assert_eq!(receipt.deleted_files, 2);
        assert_eq!(receipt.deleted_managed_files, 0);
        assert_eq!(receipt.deleted_unmanaged_files, 2);
        assert!(transport
            .deletes
            .borrow()
            .iter()
            .any(|url| url.ends_with("/site/assets/old.css")));
    }

    #[test]
    fn invalid_region_cannot_change_storage_host() {
        assert!(storage_host("de/path").is_err());
        assert_eq!(storage_host("DE").unwrap(), "storage.bunnycdn.com");
        assert_eq!(storage_host("ny").unwrap(), "ny.storage.bunnycdn.com");
    }

    #[cfg(unix)]
    #[test]
    fn artifact_preflight_failure_makes_zero_transport_calls() {
        use std::os::unix::fs::symlink;

        let root = deploy_fixture("zero-request");
        let outside = root.parent().unwrap().join("outside");
        fs::create_dir_all(&outside).unwrap();
        symlink(outside, fixture_output(&root)).unwrap();
        let transport = FakeTransport::default();

        let error =
            deploy_project_with_transport(&root, &root.to_path_buf(), &root, &transport, |_| {
                Ok(test_endpoints())
            })
            .unwrap_err();

        assert!(error.contains("symlink"));
        assert!(transport.uploads.borrow().is_empty());
        assert_eq!(transport.purge_calls.get(), 0);
        cleanup(root);
    }

    #[test]
    fn upload_failure_is_terminal_and_skips_purge() {
        let root = deploy_fixture("upload-failure");
        let output = fixture_output(&root);
        fs::create_dir_all(&output).unwrap();
        fs::write(output.join("index.html"), "payload").unwrap();
        let transport = FakeTransport {
            fail_upload: true,
            ..FakeTransport::default()
        };

        let error =
            deploy_project_with_transport(&root, &root.to_path_buf(), &root, &transport, |_| {
                Ok(test_endpoints())
            })
            .unwrap_err();

        assert!(error.contains("nu a fost purjat"));
        assert_eq!(transport.uploads.borrow().len(), 1);
        assert_eq!(transport.purge_calls.get(), 0);
        cleanup(root);
    }

    #[test]
    fn successful_manifest_sends_uppercase_checksum_then_purges_once() {
        let root = deploy_fixture("checksum-purge");
        let output = fixture_output(&root);
        fs::create_dir_all(&output).unwrap();
        fs::write(output.join("index.html"), "abc").unwrap();
        let transport = FakeTransport::default();

        let result =
            deploy_project_with_transport(&root, &root.to_path_buf(), &root, &transport, |_| {
                Ok(test_endpoints())
            })
            .unwrap();

        let uploads = transport.uploads.borrow();
        assert_eq!(uploads.len(), 1);
        assert_eq!(
            uploads[0].1,
            "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"
        );
        assert_eq!(uploads[0].2, b"abc");
        assert_eq!(transport.purge_calls.get(), 1);
        assert!(result.contains("purge CDN confirmat"));
        drop(uploads);
        cleanup(root);
    }

    #[test]
    fn cancellation_stops_between_uploads_and_skips_purge() {
        let root = deploy_fixture("cancel-between-uploads");
        let output = fixture_output(&root);
        fs::create_dir_all(&output).unwrap();
        fs::write(output.join("a.html"), "a").unwrap();
        fs::write(output.join("b.html"), "b").unwrap();
        let transport = FakeTransport::default();
        let cancellation_checks = Cell::new(0usize);

        let error = deploy_project_with_transport_cancellable(
            &root,
            &root.to_path_buf(),
            &root,
            &transport,
            |_| Ok(test_endpoints()),
            || {
                let next = cancellation_checks.get() + 1;
                cancellation_checks.set(next);
                next >= 2
            },
        )
        .unwrap_err();

        assert!(error.contains("[publish_cancelled]"));
        assert_eq!(transport.uploads.borrow().len(), 1);
        assert_eq!(transport.purge_calls.get(), 0);
        cleanup(root);
    }

    #[test]
    fn purge_failure_is_terminal_after_successful_uploads() {
        let root = deploy_fixture("purge-failure");
        let output = fixture_output(&root);
        fs::create_dir_all(&output).unwrap();
        fs::write(output.join("index.html"), "payload").unwrap();
        let transport = FakeTransport {
            fail_purge: true,
            ..FakeTransport::default()
        };

        let error =
            deploy_project_with_transport(&root, &root.to_path_buf(), &root, &transport, |_| {
                Ok(test_endpoints())
            })
            .unwrap_err();

        assert!(error.contains("nu este confirmat complet"));
        assert_eq!(transport.uploads.borrow().len(), 1);
        assert_eq!(transport.purge_calls.get(), 1);
        cleanup(root);
    }

    #[test]
    fn typed_bunny_execution_uploads_owned_manifest_last_then_purges() {
        let target = typed_target();
        let credential = typed_credential();
        let runtime = BunnyRuntimeConfig::from_target(&target, &credential).unwrap();
        let artifact = captured_artifact(&[("index.html", b"hello")], "sha256:first");
        let transport = FakeTransport::default();
        let plan = plan_bunny_with_transport(
            &transport,
            &test_endpoints(),
            &runtime,
            &target,
            7,
            &artifact,
        )
        .unwrap();
        let events = RefCell::new(Vec::new());
        let sink = |event| events.borrow_mut().push(event);
        let reporter = DeployProgressReporter::new("operation-1", &target, &sink);

        let receipt = execute_bunny_with_transport(
            &transport,
            &test_endpoints(),
            &runtime,
            "operation-1",
            &target,
            7,
            &plan.plan.plan_token,
            artifact,
            &|| false,
            &reporter,
        )
        .unwrap();

        let uploads = transport.uploads.borrow();
        assert_eq!(uploads.len(), 2);
        assert!(uploads[0].0.ends_with("/site/index.html"));
        assert!(uploads[1]
            .0
            .ends_with(&format!("/site/{REMOTE_MANIFEST_FILE_NAME}")));
        assert_eq!(receipt.status, DeployReceiptStatus::Completed);
        assert!(receipt.remote_manifest_published);
        assert!(receipt.cache_invalidated);
        assert_eq!(receipt.settings_revision, 7);
        assert_eq!(transport.purge_calls.get(), 1);
        assert!(events
            .borrow()
            .iter()
            .any(|event| event.phase == DeployProgressPhase::Completed));
    }

    #[test]
    fn typed_bunny_retries_an_idempotent_upload_after_a_transient_failure() {
        let target = typed_target();
        let runtime = BunnyRuntimeConfig::from_target(&target, &typed_credential()).unwrap();
        let artifact = captured_artifact(&[("index.html", b"hello")], "sha256:retry");
        let transport = FakeTransport::default();
        transport.transient_upload_failures.set(1);
        let plan = plan_bunny_with_transport(
            &transport,
            &test_endpoints(),
            &runtime,
            &target,
            1,
            &artifact,
        )
        .unwrap();
        let sink = |_| {};
        let receipt = execute_bunny_with_transport(
            &transport,
            &test_endpoints(),
            &runtime,
            "retry",
            &target,
            1,
            &plan.plan.plan_token,
            artifact,
            &|| false,
            &DeployProgressReporter::new("retry", &target, &sink),
        )
        .unwrap();

        assert_eq!(receipt.status, DeployReceiptStatus::Completed);
        assert_eq!(transport.transient_upload_failures.get(), 0);
        assert_eq!(transport.uploads.borrow().len(), 2);
    }

    #[test]
    fn typed_bunny_cancellation_after_upload_returns_partial_without_manifest_or_purge() {
        let target = typed_target();
        let runtime = BunnyRuntimeConfig::from_target(&target, &typed_credential()).unwrap();
        let artifact = captured_artifact(&[("index.html", b"hello")], "sha256:cancel");
        let transport = FakeTransport::default();
        let plan = plan_bunny_with_transport(
            &transport,
            &test_endpoints(),
            &runtime,
            &target,
            1,
            &artifact,
        )
        .unwrap();
        let cancellation_checks = Cell::new(0);
        let sink = |_| {};
        let error = execute_bunny_with_transport(
            &transport,
            &test_endpoints(),
            &runtime,
            "cancel",
            &target,
            1,
            &plan.plan.plan_token,
            artifact,
            &|| {
                cancellation_checks.set(cancellation_checks.get() + 1);
                cancellation_checks.get() > 1
            },
            &DeployProgressReporter::new("cancel", &target, &sink),
        )
        .unwrap_err();

        assert_eq!(error.code, DeployErrorCode::Cancelled);
        let receipt = error.receipt.unwrap();
        assert_eq!(receipt.status, DeployReceiptStatus::Partial);
        assert_eq!(receipt.uploaded_files, 1);
        assert!(!receipt.remote_manifest_published);
        assert_eq!(transport.purge_calls.get(), 0);
    }

    #[test]
    fn typed_bunny_failure_after_a_remote_mutation_returns_partial_receipt() {
        let target = typed_target();
        let credential = typed_credential();
        let runtime = BunnyRuntimeConfig::from_target(&target, &credential).unwrap();
        let artifact = captured_artifact(&[("a.html", b"a"), ("b.html", b"b")], "sha256:partial");
        let transport = FakeTransport {
            fail_upload_at: Some(2),
            ..FakeTransport::default()
        };
        let plan = plan_bunny_with_transport(
            &transport,
            &test_endpoints(),
            &runtime,
            &target,
            1,
            &artifact,
        )
        .unwrap();
        let sink = |_| {};
        let reporter = DeployProgressReporter::new("operation-partial", &target, &sink);

        let error = execute_bunny_with_transport(
            &transport,
            &test_endpoints(),
            &runtime,
            "operation-partial",
            &target,
            1,
            &plan.plan.plan_token,
            artifact,
            &|| false,
            &reporter,
        )
        .unwrap_err();

        assert_eq!(error.code, DeployErrorCode::UploadFailed);
        let receipt = error.receipt.unwrap();
        assert_eq!(receipt.status, DeployReceiptStatus::Partial);
        assert_eq!(receipt.uploaded_files, 1);
        assert!(!receipt.remote_manifest_published);
        assert_eq!(transport.purge_calls.get(), 0);
        assert!(!error.message.contains("storage-secret"));
        assert!(!error.message.contains("cdn-secret"));
    }

    #[test]
    fn typed_bunny_sync_deletes_only_previous_manifest_paths() {
        let target = typed_target();
        let credential = typed_credential();
        let runtime = BunnyRuntimeConfig::from_target(&target, &credential).unwrap();
        let previous = captured_artifact(
            &[("index.html", b"same"), ("stale.txt", b"old")],
            "sha256:old",
        );
        let transport = FakeTransport::default();
        let previous_plan = plan_bunny_with_transport(
            &transport,
            &test_endpoints(),
            &runtime,
            &target,
            2,
            &previous,
        )
        .unwrap();
        *transport.download.borrow_mut() = Some(previous_plan.next_manifest_bytes);
        let current = captured_artifact(&[("index.html", b"same")], "sha256:new");
        let plan = plan_bunny_with_transport(
            &transport,
            &test_endpoints(),
            &runtime,
            &target,
            2,
            &current,
        )
        .unwrap();
        assert_eq!(plan.plan.skipped_files, 1);
        assert_eq!(plan.plan.delete_files, 1);
        let sink = |_| {};
        let reporter = DeployProgressReporter::new("operation-delete", &target, &sink);

        let receipt = execute_bunny_with_transport(
            &transport,
            &test_endpoints(),
            &runtime,
            "operation-delete",
            &target,
            2,
            &plan.plan.plan_token,
            current,
            &|| false,
            &reporter,
        )
        .unwrap();

        assert_eq!(receipt.deleted_files, 1);
        assert_eq!(receipt.skipped_files, 1);
        let deletes = transport.deletes.borrow();
        assert_eq!(deletes.len(), 1);
        assert!(deletes[0].ends_with("/site/stale.txt"));
    }

    #[test]
    fn typed_bunny_rejects_stale_plan_before_remote_mutation() {
        let target = typed_target();
        let credential = typed_credential();
        let runtime = BunnyRuntimeConfig::from_target(&target, &credential).unwrap();
        let artifact = captured_artifact(&[("index.html", b"hello")], "sha256:first");
        let transport = FakeTransport::default();
        let sink = |_| {};
        let reporter = DeployProgressReporter::new("operation-stale", &target, &sink);

        let error = execute_bunny_with_transport(
            &transport,
            &test_endpoints(),
            &runtime,
            "operation-stale",
            &target,
            1,
            "plan:stale",
            artifact,
            &|| false,
            &reporter,
        )
        .unwrap_err();

        assert_eq!(error.code, DeployErrorCode::InvalidConfiguration);
        assert!(transport.uploads.borrow().is_empty());
        assert!(transport.deletes.borrow().is_empty());
        assert_eq!(transport.purge_calls.get(), 0);
    }

    fn deploy_fixture(label: &str) -> PathBuf {
        let outer = unique_temp_dir(label);
        let root = outer.join("site");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("zola.toml"),
            "base_url = '/'\noutput_dir = '../export'\n",
        )
        .unwrap();
        fs::write(
            root.join(".env"),
            "BUNNY_STORAGE_ZONE=zone\nBUNNY_STORAGE_KEY=storage-key\nBUNNY_PULL_ZONE_ID=42\nBUNNY_CDN_API_KEY=cdn-key\n",
        )
        .unwrap();
        root.canonicalize().unwrap()
    }

    fn fixture_output(root: &Path) -> PathBuf {
        root.parent().unwrap().join("export")
    }

    fn test_endpoints() -> BunnyEndpoints {
        BunnyEndpoints {
            storage_base: Url::parse("https://storage.invalid/").unwrap(),
            api_base: Url::parse("https://api.invalid/").unwrap(),
        }
    }

    fn typed_target() -> DeployTarget {
        DeployTarget {
            id: "production".to_string(),
            name: "Production".to_string(),
            credential_ref: "production-credentials".to_string(),
            cleanup_policy: DeployCleanupPolicy::ManagedOnly,
            provider: DeployTargetProvider::Bunny(BunnyTargetConfig {
                storage_zone: "site".to_string(),
                storage_region: "de".to_string(),
                pull_zone_id: "42".to_string(),
                remote_prefix: String::new(),
            }),
        }
    }

    fn typed_credential() -> StoredDeployCredential {
        StoredDeployCredential::Bunny {
            storage_key: "storage-secret".to_string(),
            cdn_api_key: "cdn-secret".to_string(),
        }
    }

    fn captured_artifact(files: &[(&str, &[u8])], artifact_id: &str) -> DeployArtifactManifest {
        DeployArtifactManifest {
            root: PathBuf::from("/artifact"),
            files: files
                .iter()
                .map(|(path, bytes)| DeployArtifactFile {
                    relative_path: (*path).to_string(),
                    bytes: bytes.to_vec(),
                    sha256_uppercase: uppercase_sha256(bytes),
                })
                .collect(),
            total_bytes: files.iter().map(|(_, bytes)| bytes.len() as u64).sum(),
            artifact_id: artifact_id.to_string(),
        }
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "panastudio-bunny-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn cleanup(path: PathBuf) {
        let target = path.parent().unwrap_or(&path).to_path_buf();
        let _ = fs::remove_dir_all(target);
    }
}
