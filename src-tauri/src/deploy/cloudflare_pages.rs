use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::Path,
    thread,
    time::Duration,
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use reqwest::blocking::{multipart, Client, RequestBuilder, Response};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    artifact::{DeployArtifactFile, DeployArtifactManifest},
    credentials::StoredDeployCredential,
    engine::DeployProgressReporter,
    model::{
        CloudflarePagesTargetConfig, DeployAction, DeployActionKind, DeployCommandError,
        DeployErrorCode, DeployPlan, DeployProgressPhase, DeployReceipt, DeployReceiptStatus,
        DeployTarget, DeployTargetProvider, DEPLOY_PLAN_SCHEMA_VERSION,
        DEPLOY_RECEIPT_SCHEMA_VERSION,
    },
    retry::retry_idempotent,
};

const API_BASE: &str = "https://api.cloudflare.com/client/v4";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const MAX_ASSET_FILES: usize = 20_000;
const MAX_ASSET_BYTES: usize = 25 * 1024 * 1024;
const UPLOAD_BUCKET_BYTES: usize = 40 * 1024 * 1024;
const UPLOAD_BUCKET_FILES: usize = 2_000;
const DEPLOYMENT_POLL_ATTEMPTS: usize = 5;

pub(crate) fn plan_cloudflare_pages_deploy(
    target: &DeployTarget,
    settings_revision: u64,
    artifact: &DeployArtifactManifest,
    credential: &StoredDeployCredential,
) -> Result<DeployPlan, DeployCommandError> {
    let runtime = RuntimeConfig::from_target(target, credential).map_err(|message| {
        DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
    })?;
    let transport = HttpTransport::new(&runtime).map_err(|message| {
        DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
    })?;
    prepare_plan(&transport, target, settings_revision, artifact).map(|prepared| prepared.plan)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_cloudflare_pages_deploy(
    operation_id: &str,
    target: &DeployTarget,
    settings_revision: u64,
    expected_plan_token: &str,
    artifact: DeployArtifactManifest,
    credential: StoredDeployCredential,
    is_cancelled: &dyn Fn() -> bool,
    progress: &DeployProgressReporter<'_>,
) -> Result<DeployReceipt, DeployCommandError> {
    let runtime = RuntimeConfig::from_target(target, &credential).map_err(|message| {
        DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
    })?;
    let transport = HttpTransport::new(&runtime).map_err(|message| {
        DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
    })?;
    execute_with_transport(
        &transport,
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

pub(crate) fn test_cloudflare_pages_connection(
    target: &DeployTarget,
    credential: &StoredDeployCredential,
) -> Result<(), DeployCommandError> {
    let runtime = RuntimeConfig::from_target(target, credential).map_err(|message| {
        DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
    })?;
    HttpTransport::new(&runtime)
        .and_then(|transport| transport.test_project())
        .map_err(|message| DeployCommandError::new(DeployErrorCode::ConnectionFailed, message))
}

#[derive(Clone)]
struct RuntimeConfig {
    account_id: String,
    project_name: String,
    branch: Option<String>,
    api_token: String,
}

impl RuntimeConfig {
    fn from_target(
        target: &DeployTarget,
        credential: &StoredDeployCredential,
    ) -> Result<Self, String> {
        target.validate()?;
        let DeployTargetProvider::CloudflarePages(CloudflarePagesTargetConfig {
            account_id,
            project_name,
            branch,
        }) = &target.provider
        else {
            return Err("Ținta nu este configurată pentru Cloudflare Pages.".to_string());
        };
        let StoredDeployCredential::CloudflarePages { api_token } = credential else {
            return Err(
                "Credentialele țintei Cloudflare Pages au un tip incompatibil.".to_string(),
            );
        };
        Ok(Self {
            account_id: account_id.clone(),
            project_name: project_name.clone(),
            branch: branch.clone(),
            api_token: api_token.clone(),
        })
    }
}

#[derive(Clone)]
struct Asset {
    path: String,
    hash: String,
    content_type: String,
    bytes: Vec<u8>,
}

#[derive(Clone)]
struct SpecialFile {
    field_name: &'static str,
    content_type: &'static str,
    bytes: Vec<u8>,
}

struct PreparedPlan {
    plan: DeployPlan,
    assets: Vec<Asset>,
    specials: Vec<SpecialFile>,
    missing_hashes: BTreeSet<String>,
    upload_token: String,
}

#[derive(Clone)]
struct Deployment {
    id: String,
    url: Option<String>,
    stage_status: Option<String>,
}

trait PagesTransport {
    fn upload_token(&self) -> Result<String, String>;
    fn check_missing(&self, upload_token: &str, hashes: &[String]) -> Result<Vec<String>, String>;
    fn upload_assets(&self, upload_token: &str, assets: &[Asset]) -> Result<(), String>;
    fn upsert_hashes(&self, upload_token: &str, hashes: &[String]) -> Result<(), String>;
    fn create_deployment(
        &self,
        manifest: &BTreeMap<String, String>,
        branch: Option<&str>,
        special_files: &[SpecialFile],
    ) -> Result<Deployment, String>;
    fn deployment(&self, deployment_id: &str) -> Result<Deployment, String>;
}

struct HttpTransport {
    client: Client,
    account_id: String,
    project_name: String,
    api_token: String,
}

impl HttpTransport {
    fn new(runtime: &RuntimeConfig) -> Result<Self, String> {
        Ok(Self {
            client: Client::builder()
                .connect_timeout(CONNECT_TIMEOUT)
                .timeout(REQUEST_TIMEOUT)
                .build()
                .map_err(|_| "Clientul HTTP Cloudflare nu poate fi inițializat.".to_string())?,
            account_id: runtime.account_id.clone(),
            project_name: runtime.project_name.clone(),
            api_token: runtime.api_token.clone(),
        })
    }

    fn project_url(&self) -> String {
        format!(
            "{API_BASE}/accounts/{}/pages/projects/{}",
            self.account_id, self.project_name
        )
    }

    fn authenticated(&self, request: RequestBuilder) -> RequestBuilder {
        request.bearer_auth(&self.api_token)
    }

    fn test_project(&self) -> Result<(), String> {
        let _: serde_json::Value = api_result(
            self.authenticated(self.client.get(self.project_url())),
            "verificarea proiectului Pages",
        )?;
        Ok(())
    }
}

#[derive(Deserialize)]
struct UploadTokenResult {
    jwt: String,
}

#[derive(Serialize)]
struct HashesRequest<'a> {
    hashes: &'a [String],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AssetPayload<'a> {
    key: &'a str,
    value: String,
    metadata: AssetMetadata<'a>,
    base64: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AssetMetadata<'a> {
    content_type: &'a str,
}

#[derive(Deserialize)]
struct DeploymentResult {
    id: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    latest_stage: Option<DeploymentStage>,
}

#[derive(Deserialize)]
struct DeploymentStage {
    status: String,
}

impl From<DeploymentResult> for Deployment {
    fn from(value: DeploymentResult) -> Self {
        Self {
            id: value.id,
            url: value.url,
            stage_status: value.latest_stage.map(|stage| stage.status),
        }
    }
}

impl PagesTransport for HttpTransport {
    fn upload_token(&self) -> Result<String, String> {
        let result: UploadTokenResult = api_result(
            self.authenticated(
                self.client
                    .get(format!("{}/upload-token", self.project_url())),
            ),
            "obținerea tokenului de upload Pages",
        )?;
        if result.jwt.is_empty() || result.jwt.len() > 16 * 1024 {
            return Err("Cloudflare a furnizat un token de upload invalid.".to_string());
        }
        Ok(result.jwt)
    }

    fn check_missing(&self, upload_token: &str, hashes: &[String]) -> Result<Vec<String>, String> {
        api_result(
            self.client
                .post(format!("{API_BASE}/pages/assets/check-missing"))
                .bearer_auth(upload_token)
                .json(&HashesRequest { hashes }),
            "verificarea asseturilor Pages lipsă",
        )
    }

    fn upload_assets(&self, upload_token: &str, assets: &[Asset]) -> Result<(), String> {
        let payload: Vec<_> = assets
            .iter()
            .map(|asset| AssetPayload {
                key: &asset.hash,
                value: BASE64_STANDARD.encode(&asset.bytes),
                metadata: AssetMetadata {
                    content_type: &asset.content_type,
                },
                base64: true,
            })
            .collect();
        api_success(
            self.client
                .post(format!("{API_BASE}/pages/assets/upload"))
                .bearer_auth(upload_token)
                .json(&payload),
            "upload-ul asseturilor Pages",
        )
    }

    fn upsert_hashes(&self, upload_token: &str, hashes: &[String]) -> Result<(), String> {
        api_success(
            self.client
                .post(format!("{API_BASE}/pages/assets/upsert-hashes"))
                .bearer_auth(upload_token)
                .json(&HashesRequest { hashes }),
            "actualizarea cache-ului de hash-uri Pages",
        )
    }

    fn create_deployment(
        &self,
        manifest: &BTreeMap<String, String>,
        branch: Option<&str>,
        special_files: &[SpecialFile],
    ) -> Result<Deployment, String> {
        let mut form = multipart::Form::new().text(
            "manifest",
            serde_json::to_string(manifest)
                .map_err(|_| "Manifestul Pages nu poate fi serializat.".to_string())?,
        );
        if let Some(branch) = branch {
            form = form.text("branch", branch.to_string());
        }
        for special in special_files {
            let part = multipart::Part::bytes(special.bytes.clone())
                .file_name(special.field_name.to_string())
                .mime_str(special.content_type)
                .map_err(|_| "Tipul unui fișier special Pages este invalid.".to_string())?;
            form = form.part(special.field_name, part);
        }
        let result: DeploymentResult = api_result(
            self.authenticated(
                self.client
                    .post(format!("{}/deployments", self.project_url()))
                    .multipart(form),
            ),
            "crearea deployment-ului Pages",
        )?;
        Ok(result.into())
    }

    fn deployment(&self, deployment_id: &str) -> Result<Deployment, String> {
        let result: DeploymentResult = api_result(
            self.authenticated(self.client.get(format!(
                "{}/deployments/{deployment_id}",
                self.project_url()
            ))),
            "citirea stării deployment-ului Pages",
        )?;
        Ok(result.into())
    }
}

#[derive(Deserialize)]
struct ApiEnvelope<T> {
    success: bool,
    result: Option<T>,
    #[serde(default)]
    errors: Vec<ApiError>,
}

#[derive(Deserialize)]
struct ApiError {
    code: u64,
}

fn api_result<T: DeserializeOwned>(request: RequestBuilder, operation: &str) -> Result<T, String> {
    let envelope: ApiEnvelope<T> = api_envelope(request, operation)?;
    if !envelope.success {
        return Err(api_failure(operation, &envelope.errors));
    }
    envelope
        .result
        .ok_or_else(|| format!("Cloudflare nu a returnat rezultatul pentru {operation}."))
}

fn api_success(request: RequestBuilder, operation: &str) -> Result<(), String> {
    let envelope: ApiEnvelope<serde_json::Value> = api_envelope(request, operation)?;
    if envelope.success {
        Ok(())
    } else {
        Err(api_failure(operation, &envelope.errors))
    }
}

fn api_envelope<T: DeserializeOwned>(
    request: RequestBuilder,
    operation: &str,
) -> Result<ApiEnvelope<T>, String> {
    let response = request
        .send()
        .map_err(|_| format!("Conexiunea Cloudflare a eșuat la {operation}."))?;
    read_response(response, operation)
}

fn read_response<T: DeserializeOwned>(
    response: Response,
    operation: &str,
) -> Result<ApiEnvelope<T>, String> {
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(format!(
            "Răspunsul Cloudflare este prea mare la {operation}."
        ));
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_RESPONSE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| format!("Răspunsul Cloudflare nu poate fi citit la {operation}."))?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(format!(
            "Răspunsul Cloudflare este prea mare la {operation}."
        ));
    }
    let envelope: ApiEnvelope<T> = serde_json::from_slice(&bytes).map_err(|_| {
        format!(
            "Cloudflare a returnat un răspuns invalid la {operation} (HTTP {}).",
            status.as_u16()
        )
    })?;
    if !status.is_success() && envelope.success {
        return Err(format!(
            "Cloudflare a respins {operation} (HTTP {}).",
            status.as_u16()
        ));
    }
    Ok(envelope)
}

fn api_failure(operation: &str, errors: &[ApiError]) -> String {
    errors.first().map_or_else(
        || format!("Cloudflare a respins {operation}."),
        |error| format!("Cloudflare a respins {operation} (cod API {}).", error.code),
    )
}

fn prepare_plan<T: PagesTransport>(
    transport: &T,
    target: &DeployTarget,
    settings_revision: u64,
    artifact: &DeployArtifactManifest,
) -> Result<PreparedPlan, DeployCommandError> {
    let (assets, specials) = provider_files(artifact).map_err(|message| {
        DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
    })?;
    let upload_token = retry_idempotent(|| transport.upload_token()).map_err(|message| {
        DeployCommandError::new(DeployErrorCode::RemoteInventoryFailed, message)
    })?;
    let unique_hashes: Vec<String> = assets
        .iter()
        .map(|asset| asset.hash.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let missing_hashes: BTreeSet<String> =
        retry_idempotent(|| transport.check_missing(&upload_token, &unique_hashes))
            .map_err(|message| {
                DeployCommandError::new(DeployErrorCode::RemoteInventoryFailed, message)
            })?
            .into_iter()
            .filter(|hash| unique_hashes.binary_search(hash).is_ok())
            .collect();

    let mut actions = Vec::with_capacity(artifact.files.len());
    let mut upload_files = 0u64;
    let mut upload_bytes = 0u64;
    let mut skipped_files = 0u64;
    for asset in &assets {
        let kind = if missing_hashes.contains(&asset.hash) {
            upload_files += 1;
            upload_bytes = upload_bytes.saturating_add(asset.bytes.len() as u64);
            DeployActionKind::Upload
        } else {
            skipped_files += 1;
            DeployActionKind::Skip
        };
        actions.push(DeployAction {
            kind,
            path: asset.path.clone(),
            size_bytes: asset.bytes.len() as u64,
            sha256: None,
            delete_origin: None,
        });
    }
    for special in &specials {
        upload_files += 1;
        upload_bytes = upload_bytes.saturating_add(special.bytes.len() as u64);
        actions.push(DeployAction {
            kind: DeployActionKind::Upload,
            path: special.field_name.to_string(),
            size_bytes: special.bytes.len() as u64,
            sha256: None,
            delete_origin: None,
        });
    }
    actions.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(PreparedPlan {
        plan: DeployPlan {
            schema_version: DEPLOY_PLAN_SCHEMA_VERSION,
            plan_token: plan_token(target, settings_revision, artifact),
            settings_revision,
            target_id: target.id.clone(),
            provider: target.provider_kind(),
            artifact_id: artifact.artifact_id.clone(),
            preflight_token: String::new(),
            build_token: String::new(),
            upload_files,
            upload_bytes,
            skipped_files,
            delete_files: 0,
            managed_delete_files: 0,
            unmanaged_delete_files: 0,
            actions,
            warnings: target.security_warnings(),
        },
        assets,
        specials,
        missing_hashes,
        upload_token,
    })
}

fn provider_files(
    artifact: &DeployArtifactManifest,
) -> Result<(Vec<Asset>, Vec<SpecialFile>), String> {
    if artifact.files.len() > MAX_ASSET_FILES {
        return Err(format!(
            "Cloudflare Pages acceptă cel mult {MAX_ASSET_FILES} fișiere per deployment."
        ));
    }
    let mut assets = Vec::new();
    let mut specials = Vec::new();
    for file in &artifact.files {
        if file.bytes.len() > MAX_ASSET_BYTES {
            return Err(format!(
                "Fișierul '{}' depășește limita Cloudflare Pages de {MAX_ASSET_BYTES} bytes.",
                file.relative_path
            ));
        }
        if let Some((field_name, content_type)) = special_file(&file.relative_path) {
            specials.push(SpecialFile {
                field_name,
                content_type,
                bytes: file.bytes.clone(),
            });
        } else {
            assets.push(Asset {
                path: file.relative_path.clone(),
                hash: asset_hash(file),
                content_type: mime_guess::from_path(&file.relative_path)
                    .first_raw()
                    .unwrap_or("application/octet-stream")
                    .to_string(),
                bytes: file.bytes.clone(),
            });
        }
    }
    Ok((assets, specials))
}

fn special_file(path: &str) -> Option<(&'static str, &'static str)> {
    match path {
        "_headers" => Some(("_headers", "text/plain")),
        "_redirects" => Some(("_redirects", "text/plain")),
        "_routes.json" => Some(("_routes.json", "application/json")),
        "_worker.js" => Some(("_worker.js", "application/javascript")),
        _ => None,
    }
}

fn asset_hash(file: &DeployArtifactFile) -> String {
    let base64 = BASE64_STANDARD.encode(&file.bytes);
    let extension = Path::new(&file.relative_path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    let mut input = String::with_capacity(base64.len() + extension.len());
    input.push_str(&base64);
    input.push_str(extension);
    blake3::hash(input.as_bytes()).to_hex()[..32].to_string()
}

fn plan_token(target: &DeployTarget, revision: u64, artifact: &DeployArtifactManifest) -> String {
    let mut digest = Sha256::new();
    digest.update(b"pana-cloudflare-pages-plan-v1\0");
    digest.update((target.id.len() as u64).to_be_bytes());
    digest.update(target.id.as_bytes());
    digest.update(revision.to_be_bytes());
    digest.update((artifact.artifact_id.len() as u64).to_be_bytes());
    digest.update(artifact.artifact_id.as_bytes());
    format!("plan:{:x}", digest.finalize())
}

#[allow(clippy::too_many_arguments)]
fn execute_with_transport<T: PagesTransport>(
    transport: &T,
    runtime: &RuntimeConfig,
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
    let prepared = prepare_plan(transport, target, settings_revision, &artifact)?;
    if prepared.plan.plan_token != expected_plan_token {
        return Err(DeployCommandError::new(
            DeployErrorCode::InvalidConfiguration,
            "Planul deploy nu mai corespunde artifactului sau configurației Cloudflare Pages. Recalculează planul.",
        ));
    }
    let mut receipt = DeployReceipt {
        schema_version: DEPLOY_RECEIPT_SCHEMA_VERSION,
        operation_id: operation_id.to_string(),
        target_id: target.id.clone(),
        provider: target.provider_kind(),
        artifact_id: artifact.artifact_id,
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

    let buckets = upload_buckets(unique_missing_assets(&prepared));
    for bucket in buckets {
        if is_cancelled() {
            return Err(cancelled_error(receipt));
        }
        let bucket_hashes: BTreeSet<&str> =
            bucket.iter().map(|asset| asset.hash.as_str()).collect();
        let logical: Vec<&Asset> = prepared
            .assets
            .iter()
            .filter(|asset| bucket_hashes.contains(asset.hash.as_str()))
            .collect();
        progress.emit(
            DeployProgressPhase::Uploading,
            bucket.first().map(|asset| asset.path.clone()),
            receipt.uploaded_files,
            prepared.plan.upload_files,
            receipt.uploaded_bytes,
            prepared.plan.upload_bytes,
        );
        retry_idempotent(|| transport.upload_assets(&prepared.upload_token, &bucket)).map_err(
            |message| mutation_error(DeployErrorCode::UploadFailed, message, receipt.clone()),
        )?;
        receipt.uploaded_files = receipt.uploaded_files.saturating_add(logical.len() as u64);
        receipt.uploaded_bytes = receipt
            .uploaded_bytes
            .saturating_add(logical.iter().map(|asset| asset.bytes.len() as u64).sum());
    }

    let all_hashes: Vec<String> = prepared
        .assets
        .iter()
        .map(|asset| asset.hash.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if retry_idempotent(|| transport.upsert_hashes(&prepared.upload_token, &all_hashes)).is_err() {
        receipt.warnings.push(
            "Cloudflare nu a confirmat cache-ul de hash-uri; deployment-ul rămâne valid, dar următorul upload poate retrimite asseturi."
                .to_string(),
        );
    }
    if is_cancelled() {
        return Err(cancelled_error(receipt));
    }
    progress.emit(
        DeployProgressPhase::Activating,
        None,
        receipt.uploaded_files,
        prepared.plan.upload_files,
        receipt.uploaded_bytes,
        prepared.plan.upload_bytes,
    );
    let manifest = prepared
        .assets
        .iter()
        .map(|asset| (format!("/{}", asset.path), asset.hash.clone()))
        .collect();
    let deployment = transport
        .create_deployment(&manifest, runtime.branch.as_deref(), &prepared.specials)
        .map_err(|message| {
            mutation_error(DeployErrorCode::ActivationFailed, message, receipt.clone())
        })?;
    receipt.deployment_id = Some(deployment.id.clone());
    receipt.deployment_url = deployment.url.clone();
    receipt.uploaded_files = receipt
        .uploaded_files
        .saturating_add(prepared.specials.len() as u64);
    receipt.uploaded_bytes = receipt.uploaded_bytes.saturating_add(
        prepared
            .specials
            .iter()
            .map(|file| file.bytes.len() as u64)
            .sum(),
    );
    let final_deployment = wait_for_deployment(transport, deployment, is_cancelled, &mut receipt)?;
    receipt.deployment_url = final_deployment.url.or(receipt.deployment_url);
    receipt.status = DeployReceiptStatus::Completed;
    receipt.completed_at_ms = crate::kernel::observability::now_ms();
    progress.emit(
        DeployProgressPhase::Completed,
        None,
        prepared.plan.upload_files,
        prepared.plan.upload_files,
        receipt.uploaded_bytes,
        prepared.plan.upload_bytes,
    );
    Ok(receipt)
}

fn unique_missing_assets(prepared: &PreparedPlan) -> Vec<Asset> {
    let mut seen = BTreeSet::new();
    prepared
        .assets
        .iter()
        .filter(|asset| {
            prepared.missing_hashes.contains(&asset.hash) && seen.insert(asset.hash.clone())
        })
        .cloned()
        .collect()
}

fn upload_buckets(assets: Vec<Asset>) -> Vec<Vec<Asset>> {
    let mut buckets = Vec::new();
    let mut current = Vec::new();
    let mut bytes = 0usize;
    for asset in assets {
        if !current.is_empty()
            && (current.len() >= UPLOAD_BUCKET_FILES
                || bytes.saturating_add(asset.bytes.len()) > UPLOAD_BUCKET_BYTES)
        {
            buckets.push(std::mem::take(&mut current));
            bytes = 0;
        }
        bytes = bytes.saturating_add(asset.bytes.len());
        current.push(asset);
    }
    if !current.is_empty() {
        buckets.push(current);
    }
    buckets
}

fn wait_for_deployment<T: PagesTransport>(
    transport: &T,
    mut deployment: Deployment,
    is_cancelled: &dyn Fn() -> bool,
    receipt: &mut DeployReceipt,
) -> Result<Deployment, DeployCommandError> {
    for attempt in 0..DEPLOYMENT_POLL_ATTEMPTS {
        match deployment.stage_status.as_deref() {
            Some("success") => return Ok(deployment),
            Some("failure" | "canceled") => {
                receipt.status = DeployReceiptStatus::Partial;
                receipt.completed_at_ms = crate::kernel::observability::now_ms();
                return Err(DeployCommandError::new(
                    DeployErrorCode::ActivationFailed,
                    "Deployment-ul Cloudflare Pages a fost creat, dar activarea a eșuat.",
                )
                .with_receipt(receipt.clone()));
            }
            _ => {}
        }
        if attempt + 1 == DEPLOYMENT_POLL_ATTEMPTS {
            break;
        }
        cancellable_wait(Duration::from_secs(1u64 << attempt), is_cancelled)
            .map_err(|_| cancelled_error(receipt.clone()))?;
        deployment = transport.deployment(&deployment.id).map_err(|message| {
            mutation_error(DeployErrorCode::ActivationFailed, message, receipt.clone())
        })?;
    }
    receipt.status = DeployReceiptStatus::Partial;
    receipt.completed_at_ms = crate::kernel::observability::now_ms();
    Err(DeployCommandError::new(
        DeployErrorCode::ActivationFailed,
        "Deployment-ul Cloudflare Pages a fost creat, dar starea finală nu a putut fi confirmată.",
    )
    .with_receipt(receipt.clone()))
}

fn cancellable_wait(duration: Duration, is_cancelled: &dyn Fn() -> bool) -> Result<(), ()> {
    let mut remaining = duration;
    let slice = Duration::from_millis(100);
    while !remaining.is_zero() {
        if is_cancelled() {
            return Err(());
        }
        let wait = remaining.min(slice);
        thread::sleep(wait);
        remaining = remaining.saturating_sub(wait);
    }
    Ok(())
}

fn mutation_error(
    code: DeployErrorCode,
    message: String,
    mut receipt: DeployReceipt,
) -> DeployCommandError {
    receipt.completed_at_ms = crate::kernel::observability::now_ms();
    receipt.status = if receipt.uploaded_files > 0 || receipt.deployment_id.is_some() {
        DeployReceiptStatus::Partial
    } else {
        DeployReceiptStatus::Failed
    };
    DeployCommandError::new(code, message).with_receipt(receipt)
}

fn cancelled_error(mut receipt: DeployReceipt) -> DeployCommandError {
    receipt.completed_at_ms = crate::kernel::observability::now_ms();
    receipt.status = if receipt.uploaded_files > 0 || receipt.deployment_id.is_some() {
        DeployReceiptStatus::Partial
    } else {
        DeployReceiptStatus::Cancelled
    };
    DeployCommandError::new(
        DeployErrorCode::Cancelled,
        "Deploy-ul Cloudflare Pages a fost anulat; consultă receipt-ul pentru starea deployment-ului.",
    )
    .with_receipt(receipt)
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, path::PathBuf};

    use sha2::{Digest, Sha256};

    use super::*;

    #[derive(Default)]
    struct FakeTransport {
        missing: RefCell<Vec<String>>,
        uploaded_batches: RefCell<Vec<Vec<String>>>,
        deployments: RefCell<Vec<BTreeMap<String, String>>>,
        fail_upload: RefCell<bool>,
        transient_upload_failures: RefCell<usize>,
    }

    impl PagesTransport for FakeTransport {
        fn upload_token(&self) -> Result<String, String> {
            Ok("upload-jwt".to_string())
        }

        fn check_missing(&self, _: &str, _: &[String]) -> Result<Vec<String>, String> {
            Ok(self.missing.borrow().clone())
        }

        fn upload_assets(&self, _: &str, assets: &[Asset]) -> Result<(), String> {
            if *self.transient_upload_failures.borrow() > 0 {
                *self.transient_upload_failures.borrow_mut() -= 1;
                return Err("Cloudflare transient test failure".to_string());
            }
            if *self.fail_upload.borrow() {
                return Err("Cloudflare test failure".to_string());
            }
            self.uploaded_batches
                .borrow_mut()
                .push(assets.iter().map(|asset| asset.hash.clone()).collect());
            Ok(())
        }

        fn upsert_hashes(&self, _: &str, _: &[String]) -> Result<(), String> {
            Ok(())
        }

        fn create_deployment(
            &self,
            manifest: &BTreeMap<String, String>,
            _: Option<&str>,
            _: &[SpecialFile],
        ) -> Result<Deployment, String> {
            self.deployments.borrow_mut().push(manifest.clone());
            Ok(Deployment {
                id: "deployment-42".to_string(),
                url: Some("https://deployment.pages.dev".to_string()),
                stage_status: Some("success".to_string()),
            })
        }

        fn deployment(&self, _: &str) -> Result<Deployment, String> {
            unreachable!("deployment-ul fake este deja final")
        }
    }

    fn target() -> DeployTarget {
        DeployTarget {
            id: "pages-production".to_string(),
            name: "Cloudflare Pages".to_string(),
            credential_ref: "pages-secret".to_string(),
            cleanup_policy: crate::deploy::model::DeployCleanupPolicy::ManagedOnly,
            provider: DeployTargetProvider::CloudflarePages(CloudflarePagesTargetConfig {
                account_id: "0123456789abcdef0123456789abcdef".to_string(),
                project_name: "pana-site".to_string(),
                branch: Some("main".to_string()),
            }),
        }
    }

    fn runtime() -> RuntimeConfig {
        RuntimeConfig::from_target(
            &target(),
            &StoredDeployCredential::CloudflarePages {
                api_token: "secret-token".to_string(),
            },
        )
        .unwrap()
    }

    fn artifact(files: &[(&str, &[u8])]) -> DeployArtifactManifest {
        DeployArtifactManifest {
            root: PathBuf::from("/artifact"),
            files: files
                .iter()
                .map(|(path, bytes)| DeployArtifactFile {
                    relative_path: (*path).to_string(),
                    bytes: bytes.to_vec(),
                    sha256_uppercase: format!("{:X}", Sha256::digest(bytes)),
                })
                .collect(),
            total_bytes: files.iter().map(|(_, bytes)| bytes.len() as u64).sum(),
            artifact_id: "artifact:1".to_string(),
        }
    }

    fn reporter<'a>(
        operation_id: &'a str,
        target: &'a DeployTarget,
        sink: &'a dyn Fn(super::super::model::DeployProgressEvent),
    ) -> DeployProgressReporter<'a> {
        DeployProgressReporter::new(operation_id, target, sink)
    }

    #[test]
    fn plan_uses_cached_assets_as_skips() {
        let transport = FakeTransport::default();
        let target = target();
        let artifact = artifact(&[("index.html", b"home"), ("style.css", b"css")]);
        let (assets, _) = provider_files(&artifact).unwrap();
        transport.missing.borrow_mut().push(assets[1].hash.clone());

        let prepared = prepare_plan(&transport, &target, 1, &artifact).unwrap();
        assert_eq!(prepared.plan.upload_files, 1);
        assert_eq!(prepared.plan.skipped_files, 1);
        assert_eq!(assets[0].hash.len(), 32);
    }

    #[test]
    fn cloudflare_api_diagnostic_uses_only_numeric_code_not_remote_message() {
        let envelope: ApiEnvelope<serde_json::Value> = serde_json::from_str(
            r#"{"success":false,"result":null,"errors":[{"code":10000,"message":"leaked-secret-token"}]}"#,
        )
        .unwrap();
        let diagnostic = api_failure("test", &envelope.errors);

        assert!(diagnostic.contains("10000"));
        assert!(!diagnostic.contains("leaked-secret-token"));
    }

    #[test]
    fn deployment_returns_version_id_url_and_excludes_specials_from_manifest() {
        let transport = FakeTransport::default();
        let target = target();
        let runtime = runtime();
        let artifact = artifact(&[("index.html", b"home"), ("_headers", b"/*\n  X-Test: yes")]);
        let (assets, _) = provider_files(&artifact).unwrap();
        transport
            .missing
            .borrow_mut()
            .extend(assets.iter().map(|asset| asset.hash.clone()));
        let plan = prepare_plan(&transport, &target, 1, &artifact).unwrap();
        let sink = |_| {};
        let receipt = execute_with_transport(
            &transport,
            &runtime,
            "operation",
            &target,
            1,
            &plan.plan.plan_token,
            artifact,
            &|| false,
            &reporter("operation", &target, &sink),
        )
        .unwrap();

        assert_eq!(receipt.status, DeployReceiptStatus::Completed);
        assert_eq!(receipt.deployment_id.as_deref(), Some("deployment-42"));
        assert_eq!(
            receipt.deployment_url.as_deref(),
            Some("https://deployment.pages.dev")
        );
        assert!(transport.deployments.borrow()[0].contains_key("/index.html"));
        assert!(!transport.deployments.borrow()[0].contains_key("/_headers"));
    }

    #[test]
    fn retries_a_transient_content_addressed_asset_upload() {
        let transport = FakeTransport::default();
        *transport.transient_upload_failures.borrow_mut() = 1;
        let target = target();
        let runtime = runtime();
        let artifact = artifact(&[("index.html", b"home")]);
        let (assets, _) = provider_files(&artifact).unwrap();
        transport.missing.borrow_mut().push(assets[0].hash.clone());
        let plan = prepare_plan(&transport, &target, 1, &artifact).unwrap();
        let sink = |_| {};
        let receipt = execute_with_transport(
            &transport,
            &runtime,
            "retry",
            &target,
            1,
            &plan.plan.plan_token,
            artifact,
            &|| false,
            &reporter("retry", &target, &sink),
        )
        .unwrap();

        assert_eq!(receipt.status, DeployReceiptStatus::Completed);
        assert_eq!(*transport.transient_upload_failures.borrow(), 0);
        assert_eq!(transport.uploaded_batches.borrow().len(), 1);
    }

    #[test]
    fn cancellation_after_pages_asset_upload_returns_partial_without_deployment() {
        let transport = FakeTransport::default();
        let target = target();
        let runtime = runtime();
        let artifact = artifact(&[("index.html", b"home")]);
        let (assets, _) = provider_files(&artifact).unwrap();
        transport.missing.borrow_mut().push(assets[0].hash.clone());
        let plan = prepare_plan(&transport, &target, 1, &artifact).unwrap();
        let checks = std::cell::Cell::new(0);
        let sink = |_| {};
        let error = execute_with_transport(
            &transport,
            &runtime,
            "cancel",
            &target,
            1,
            &plan.plan.plan_token,
            artifact,
            &|| {
                checks.set(checks.get() + 1);
                checks.get() > 1
            },
            &reporter("cancel", &target, &sink),
        )
        .unwrap_err();

        assert_eq!(error.code, DeployErrorCode::Cancelled);
        let receipt = error.receipt.unwrap();
        assert_eq!(receipt.status, DeployReceiptStatus::Partial);
        assert!(receipt.deployment_id.is_none());
        assert!(transport.deployments.borrow().is_empty());
    }

    #[test]
    fn cached_asset_is_not_uploaded_but_is_deployed() {
        let transport = FakeTransport::default();
        let target = target();
        let runtime = runtime();
        let artifact = artifact(&[("index.html", b"cached")]);
        let plan = prepare_plan(&transport, &target, 1, &artifact).unwrap();
        let sink = |_| {};
        let receipt = execute_with_transport(
            &transport,
            &runtime,
            "cached",
            &target,
            1,
            &plan.plan.plan_token,
            artifact,
            &|| false,
            &reporter("cached", &target, &sink),
        )
        .unwrap();

        assert_eq!(receipt.skipped_files, 1);
        assert!(transport.uploaded_batches.borrow().is_empty());
        assert_eq!(transport.deployments.borrow()[0].len(), 1);
    }

    #[test]
    fn upload_failure_has_failed_receipt_and_does_not_create_deployment() {
        let transport = FakeTransport::default();
        *transport.fail_upload.borrow_mut() = true;
        let target = target();
        let runtime = runtime();
        let artifact = artifact(&[("index.html", b"home")]);
        let (assets, _) = provider_files(&artifact).unwrap();
        transport.missing.borrow_mut().push(assets[0].hash.clone());
        let plan = prepare_plan(&transport, &target, 1, &artifact).unwrap();
        let sink = |_| {};
        let error = execute_with_transport(
            &transport,
            &runtime,
            "failed",
            &target,
            1,
            &plan.plan.plan_token,
            artifact,
            &|| false,
            &reporter("failed", &target, &sink),
        )
        .unwrap_err();

        assert_eq!(error.receipt.unwrap().status, DeployReceiptStatus::Failed);
        assert!(transport.deployments.borrow().is_empty());
    }

    #[test]
    fn stale_token_has_zero_remote_mutations() {
        let transport = FakeTransport::default();
        let target = target();
        let runtime = runtime();
        let artifact = artifact(&[("index.html", b"home")]);
        let sink = |_| {};
        let error = execute_with_transport(
            &transport,
            &runtime,
            "stale",
            &target,
            1,
            "plan:stale",
            artifact,
            &|| false,
            &reporter("stale", &target, &sink),
        )
        .unwrap_err();

        assert_eq!(error.code, DeployErrorCode::InvalidConfiguration);
        assert!(transport.uploaded_batches.borrow().is_empty());
        assert!(transport.deployments.borrow().is_empty());
    }
}
