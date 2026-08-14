use std::{collections::BTreeMap, path::Path};

use aws_credential_types::Credentials;
use aws_sdk_s3::{
    config::{Region, RequestChecksumCalculation},
    error::ProvideErrorMetadata,
    primitives::ByteStream,
    Client,
};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use sha2::{Digest, Sha256};

use super::{
    artifact::{DeployArtifactFile, DeployArtifactManifest},
    credentials::StoredDeployCredential,
    engine::DeployProgressReporter,
    model::{
        validate_remote_prefix, DeployActionKind, DeployCleanupPolicy, DeployCommandError,
        DeployDeleteOrigin, DeployErrorCode, DeployPlan, DeployProgressPhase, DeployReceipt,
        DeployReceiptStatus, DeployTarget, DeployTargetProvider, S3TargetConfig,
        DEPLOY_RECEIPT_SCHEMA_VERSION,
    },
    remote_manifest::{
        prepare_sync_plan, PreparedSync, RemoteInventoryFile, MAX_REMOTE_INVENTORY_FILES,
        MAX_REMOTE_MANIFEST_BYTES, REMOTE_MANIFEST_FILE_NAME,
    },
    retry::retry_idempotent,
};

const S3_MANIFEST_CACHE_CONTROL: &str = "no-cache, no-store, must-revalidate";
const S3_CHECKSUM_METADATA_KEY: &str = "pana-sha256";

pub(crate) fn plan_s3_deploy(
    target: &DeployTarget,
    settings_revision: u64,
    artifact: &DeployArtifactManifest,
    credential: &StoredDeployCredential,
) -> Result<DeployPlan, DeployCommandError> {
    let runtime = S3RuntimeConfig::from_target(target, credential).map_err(|message| {
        DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
    })?;
    let transport = AwsS3Transport::new(&runtime).map_err(|message| {
        DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
    })?;
    plan_s3_with_transport(&transport, &runtime, target, settings_revision, artifact)
        .map(|prepared| prepared.plan)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_s3_deploy(
    operation_id: &str,
    target: &DeployTarget,
    settings_revision: u64,
    expected_plan_token: &str,
    artifact: DeployArtifactManifest,
    credential: StoredDeployCredential,
    is_cancelled: &dyn Fn() -> bool,
    progress: &DeployProgressReporter<'_>,
) -> Result<DeployReceipt, DeployCommandError> {
    let runtime = S3RuntimeConfig::from_target(target, &credential).map_err(|message| {
        DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
    })?;
    let transport = AwsS3Transport::new(&runtime).map_err(|message| {
        DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
    })?;
    execute_s3_with_transport(
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

pub(crate) fn test_s3_connection(
    target: &DeployTarget,
    credential: &StoredDeployCredential,
) -> Result<u64, DeployCommandError> {
    let runtime = S3RuntimeConfig::from_target(target, credential).map_err(|message| {
        DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
    })?;
    let transport = AwsS3Transport::new(&runtime).map_err(|message| {
        DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
    })?;
    transport
        .list_objects(&runtime.list_prefix())
        .map(|objects| objects.len() as u64)
        .map_err(|message| DeployCommandError::new(DeployErrorCode::ConnectionFailed, message))
}

#[derive(Clone)]
struct S3RuntimeConfig {
    bucket: String,
    prefix: String,
    region: String,
    endpoint: Option<String>,
    force_path_style: bool,
    cache_control: Option<String>,
    access_key_id: String,
    secret_access_key: String,
    session_token: Option<String>,
}

impl S3RuntimeConfig {
    fn from_target(
        target: &DeployTarget,
        credential: &StoredDeployCredential,
    ) -> Result<Self, String> {
        target.validate()?;
        let DeployTargetProvider::S3(S3TargetConfig {
            bucket,
            prefix,
            region,
            endpoint,
            force_path_style,
            cache_control,
            ..
        }) = &target.provider
        else {
            return Err("Ținta nu este configurată pentru S3/R2.".to_string());
        };
        let StoredDeployCredential::S3 {
            access_key_id,
            secret_access_key,
            session_token,
        } = credential
        else {
            return Err("Credentialele țintei S3/R2 au un tip incompatibil.".to_string());
        };
        Ok(Self {
            bucket: bucket.clone(),
            prefix: prefix.clone(),
            region: region.clone(),
            endpoint: endpoint.clone(),
            force_path_style: *force_path_style,
            cache_control: cache_control.clone(),
            access_key_id: access_key_id.clone(),
            secret_access_key: secret_access_key.clone(),
            session_token: session_token.clone(),
        })
    }

    fn object_key(&self, relative_path: &str) -> String {
        if self.prefix.is_empty() {
            relative_path.to_string()
        } else {
            format!("{}/{relative_path}", self.prefix)
        }
    }

    fn list_prefix(&self) -> String {
        if self.prefix.is_empty() {
            String::new()
        } else {
            format!("{}/", self.prefix)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct S3RemoteObject {
    key: String,
    size_bytes: u64,
}

trait S3Transport {
    fn download_optional(&self, key: &str) -> Result<Option<Vec<u8>>, String>;

    fn list_objects(&self, prefix: &str) -> Result<Vec<S3RemoteObject>, String>;

    fn upload(
        &self,
        key: &str,
        content_type: &str,
        cache_control: Option<&str>,
        checksum_uppercase: &str,
        bytes: Vec<u8>,
    ) -> Result<(), String>;

    fn delete(&self, key: &str) -> Result<(), String>;
}

struct AwsS3Transport {
    client: Client,
    bucket: String,
}

impl AwsS3Transport {
    fn new(runtime: &S3RuntimeConfig) -> Result<Self, String> {
        let credentials = Credentials::new(
            runtime.access_key_id.clone(),
            runtime.secret_access_key.clone(),
            runtime.session_token.clone(),
            None,
            "pana-studio-deploy",
        );
        let mut builder = aws_sdk_s3::Config::builder()
            .behavior_version_latest()
            .region(Region::new(runtime.region.clone()))
            .credentials_provider(credentials)
            .force_path_style(runtime.force_path_style)
            .request_checksum_calculation(RequestChecksumCalculation::WhenSupported);
        if let Some(endpoint) = runtime.endpoint.as_deref() {
            builder = builder.endpoint_url(endpoint);
        }
        Ok(Self {
            client: Client::from_conf(builder.build()),
            bucket: runtime.bucket.clone(),
        })
    }

    fn list_objects_paginated(&self, prefix: &str) -> Result<Vec<S3RemoteObject>, String> {
        let mut continuation_token = None;
        let mut objects = Vec::new();
        loop {
            let mut request = self
                .client
                .list_objects_v2()
                .bucket(&self.bucket)
                .prefix(prefix)
                .max_keys(1_000);
            if let Some(token) = continuation_token.take() {
                request = request.continuation_token(token);
            }
            let output = tauri::async_runtime::block_on(request.send())
                .map_err(|error| safe_s3_sdk_error("listarea bucket-ului", &error))?;
            for object in output.contents() {
                let key = object
                    .key()
                    .ok_or_else(|| "Inventarul S3 conține un obiect fără key.".to_string())?;
                let size = object.size().ok_or_else(|| {
                    "Inventarul S3 conține un obiect fără dimensiune.".to_string()
                })?;
                let size_bytes = u64::try_from(size)
                    .map_err(|_| "Inventarul S3 conține o dimensiune negativă.".to_string())?;
                objects.push(S3RemoteObject {
                    key: key.to_string(),
                    size_bytes,
                });
                if objects.len() > MAX_REMOTE_INVENTORY_FILES {
                    return Err(format!(
                        "Inventarul S3 depășește limita sigură de {MAX_REMOTE_INVENTORY_FILES} fișiere."
                    ));
                }
            }
            if !output.is_truncated().unwrap_or(false) {
                break;
            }
            continuation_token = Some(
                output
                    .next_continuation_token()
                    .ok_or_else(|| {
                        "S3 a indicat o pagină următoare fără continuation token.".to_string()
                    })?
                    .to_string(),
            );
        }
        Ok(objects)
    }
}

impl S3Transport for AwsS3Transport {
    fn download_optional(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        let output = match tauri::async_runtime::block_on(
            self.client
                .get_object()
                .bucket(&self.bucket)
                .key(key)
                .send(),
        ) {
            Ok(output) => output,
            Err(error)
                if error
                    .as_service_error()
                    .is_some_and(|service| service.is_no_such_key()) =>
            {
                return Ok(None);
            }
            Err(error) => return Err(safe_s3_sdk_error("citirea manifestului", &error)),
        };
        if output
            .content_length()
            .is_some_and(|length| length < 0 || length as usize > MAX_REMOTE_MANIFEST_BYTES)
        {
            return Err("Manifestul S3 remote depășește limita sigură.".to_string());
        }
        let bytes = tauri::async_runtime::block_on(output.body.collect())
            .map_err(|_| "Body-ul manifestului S3 remote nu poate fi citit.".to_string())?
            .to_vec();
        if bytes.len() > MAX_REMOTE_MANIFEST_BYTES {
            return Err("Manifestul S3 remote depășește limita sigură.".to_string());
        }
        Ok(Some(bytes))
    }

    fn list_objects(&self, prefix: &str) -> Result<Vec<S3RemoteObject>, String> {
        self.list_objects_paginated(prefix)
    }

    fn upload(
        &self,
        key: &str,
        content_type: &str,
        cache_control: Option<&str>,
        checksum_uppercase: &str,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        let checksum_base64 = BASE64_STANDARD.encode(Sha256::digest(&bytes));
        let mut request = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(ByteStream::from(bytes))
            .content_type(content_type)
            .checksum_sha256(checksum_base64)
            .metadata(S3_CHECKSUM_METADATA_KEY, checksum_uppercase);
        if let Some(cache_control) = cache_control {
            request = request.cache_control(cache_control);
        }
        tauri::async_runtime::block_on(request.send())
            .map_err(|error| safe_s3_sdk_error("upload-ul obiectului", &error))?;
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<(), String> {
        tauri::async_runtime::block_on(
            self.client
                .delete_object()
                .bucket(&self.bucket)
                .key(key)
                .send(),
        )
        .map_err(|error| safe_s3_sdk_error("ștergerea obiectului", &error))?;
        Ok(())
    }
}

fn safe_s3_sdk_error<E, R>(operation: &str, error: &aws_sdk_s3::error::SdkError<E, R>) -> String
where
    E: ProvideErrorMetadata,
{
    let code = error
        .as_service_error()
        .and_then(ProvideErrorMetadata::code)
        .filter(|code| {
            !code.is_empty()
                && code.len() <= 128
                && code.bytes().all(|byte| byte.is_ascii_alphanumeric())
        });
    match code {
        Some(code) => format!("S3 a respins {operation} ({code})."),
        None => format!("S3 nu a putut finaliza {operation}."),
    }
}

fn plan_s3_with_transport<T: S3Transport>(
    transport: &T,
    runtime: &S3RuntimeConfig,
    target: &DeployTarget,
    settings_revision: u64,
    artifact: &DeployArtifactManifest,
) -> Result<PreparedSync, DeployCommandError> {
    let manifest_key = runtime.object_key(REMOTE_MANIFEST_FILE_NAME);
    let remote_manifest =
        retry_idempotent(|| transport.download_optional(&manifest_key)).map_err(|message| {
            DeployCommandError::new(DeployErrorCode::RemoteInventoryFailed, message)
        })?;
    let remote_inventory = if target.cleanup_policy == DeployCleanupPolicy::MirrorDestination {
        let list_prefix = runtime.list_prefix();
        let objects =
            retry_idempotent(|| transport.list_objects(&list_prefix)).map_err(|message| {
                DeployCommandError::new(DeployErrorCode::RemoteInventoryFailed, message)
            })?;
        Some(
            s3_relative_inventory(&objects, &list_prefix).map_err(|message| {
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

fn s3_relative_inventory(
    objects: &[S3RemoteObject],
    list_prefix: &str,
) -> Result<Vec<RemoteInventoryFile>, String> {
    let mut inventory = Vec::with_capacity(objects.len());
    for object in objects {
        let relative_path = object.key.strip_prefix(list_prefix).ok_or_else(|| {
            "Inventarul S3 a returnat un obiect din afara prefixului configurat.".to_string()
        })?;
        if relative_path.is_empty() || relative_path.ends_with('/') {
            continue;
        }
        validate_remote_prefix(relative_path)?;
        inventory.push(RemoteInventoryFile {
            path: relative_path.to_string(),
            size_bytes: object.size_bytes,
        });
    }
    Ok(inventory)
}

#[allow(clippy::too_many_arguments)]
fn execute_s3_with_transport<T: S3Transport>(
    transport: &T,
    runtime: &S3RuntimeConfig,
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
    let prepared =
        plan_s3_with_transport(transport, runtime, target, settings_revision, &artifact)?;
    if prepared.plan.plan_token != expected_plan_token {
        return Err(DeployCommandError::new(
            DeployErrorCode::InvalidConfiguration,
            "Planul deploy nu mai corespunde artifactului, configurației sau manifestului S3 remote. Recalculează planul.",
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
            return Err(cancelled_s3_error(receipt));
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
                "Planul S3 referă un fișier care nu există în artifactul capturat.",
            )
        })?;
        if let Err(message) = retry_idempotent(|| {
            transport.upload(
                &runtime.object_key(&file.relative_path),
                mime_for_path(Path::new(&file.relative_path)),
                runtime.cache_control.as_deref(),
                &file.sha256_uppercase,
                file.bytes.clone(),
            )
        }) {
            return Err(mutation_error(
                DeployErrorCode::UploadFailed,
                message,
                receipt,
            ));
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
            return Err(cancelled_s3_error(receipt));
        }
        progress.emit(
            DeployProgressPhase::Deleting,
            Some(action.path.clone()),
            completed_mutations,
            total_mutations,
            receipt.uploaded_bytes,
            prepared.plan.upload_bytes,
        );
        if let Err(message) =
            retry_idempotent(|| transport.delete(&runtime.object_key(&action.path)))
        {
            return Err(mutation_error(
                DeployErrorCode::DeleteFailed,
                message,
                receipt,
            ));
        }
        receipt.deleted_files += 1;
        match action.delete_origin {
            Some(DeployDeleteOrigin::Unmanaged) => receipt.deleted_unmanaged_files += 1,
            _ => receipt.deleted_managed_files += 1,
        }
        completed_mutations += 1;
    }

    if is_cancelled() {
        return Err(cancelled_s3_error(receipt));
    }
    progress.emit(
        DeployProgressPhase::Activating,
        Some(REMOTE_MANIFEST_FILE_NAME.to_string()),
        completed_mutations,
        total_mutations,
        receipt.uploaded_bytes,
        prepared.plan.upload_bytes,
    );
    let manifest_checksum = format!("{:X}", Sha256::digest(&prepared.next_manifest_bytes));
    if let Err(message) = retry_idempotent(|| {
        transport.upload(
            &runtime.object_key(REMOTE_MANIFEST_FILE_NAME),
            "application/json",
            Some(S3_MANIFEST_CACHE_CONTROL),
            &manifest_checksum,
            prepared.next_manifest_bytes.clone(),
        )
    }) {
        return Err(mutation_error(
            DeployErrorCode::ActivationFailed,
            message,
            receipt,
        ));
    }
    receipt.remote_manifest_published = true;
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

fn mutation_error(
    code: DeployErrorCode,
    message: String,
    mut receipt: DeployReceipt,
) -> DeployCommandError {
    receipt.completed_at_ms = crate::kernel::observability::now_ms();
    receipt.status = if receipt.uploaded_files > 0
        || receipt.deleted_files > 0
        || receipt.remote_manifest_published
    {
        DeployReceiptStatus::Partial
    } else {
        DeployReceiptStatus::Failed
    };
    DeployCommandError::new(code, message).with_receipt(receipt)
}

fn cancelled_s3_error(mut receipt: DeployReceipt) -> DeployCommandError {
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
        "Deploy-ul S3/R2 a fost anulat; consultă receipt-ul pentru starea remote.",
    )
    .with_receipt(receipt)
}

fn mime_for_path(path: &Path) -> &'static str {
    mime_guess::from_path(path)
        .first_raw()
        .unwrap_or("application/octet-stream")
}

#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        path::PathBuf,
    };

    use super::*;
    use crate::deploy::artifact::DeployArtifactFile;

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum Mutation {
        Upload {
            key: String,
            checksum: String,
            cache_control: Option<String>,
        },
        Delete(String),
    }

    #[derive(Default)]
    struct FakeS3Transport {
        objects: RefCell<BTreeMap<String, Vec<u8>>>,
        mutations: RefCell<Vec<Mutation>>,
        fail_mutation_at: Cell<Option<usize>>,
        transient_failures: Cell<usize>,
    }

    impl FakeS3Transport {
        fn maybe_fail(&self) -> Result<(), String> {
            if self.transient_failures.get() > 0 {
                self.transient_failures
                    .set(self.transient_failures.get() - 1);
                return Err("S3 transient test failure".to_string());
            }
            let next = self.mutations.borrow().len();
            if self.fail_mutation_at.get() == Some(next) {
                return Err("S3 test failure".to_string());
            }
            Ok(())
        }
    }

    impl S3Transport for FakeS3Transport {
        fn download_optional(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
            Ok(self.objects.borrow().get(key).cloned())
        }

        fn list_objects(&self, prefix: &str) -> Result<Vec<S3RemoteObject>, String> {
            Ok(self
                .objects
                .borrow()
                .iter()
                .filter(|(key, _)| key.starts_with(prefix))
                .map(|(key, bytes)| S3RemoteObject {
                    key: key.clone(),
                    size_bytes: bytes.len() as u64,
                })
                .collect())
        }

        fn upload(
            &self,
            key: &str,
            _content_type: &str,
            cache_control: Option<&str>,
            checksum_uppercase: &str,
            bytes: Vec<u8>,
        ) -> Result<(), String> {
            self.maybe_fail()?;
            assert_eq!(checksum_uppercase, format!("{:X}", Sha256::digest(&bytes)));
            self.mutations.borrow_mut().push(Mutation::Upload {
                key: key.to_string(),
                checksum: checksum_uppercase.to_string(),
                cache_control: cache_control.map(str::to_string),
            });
            self.objects.borrow_mut().insert(key.to_string(), bytes);
            Ok(())
        }

        fn delete(&self, key: &str) -> Result<(), String> {
            self.maybe_fail()?;
            self.mutations
                .borrow_mut()
                .push(Mutation::Delete(key.to_string()));
            self.objects.borrow_mut().remove(key);
            Ok(())
        }
    }

    fn target() -> DeployTarget {
        DeployTarget {
            id: "r2-production".to_string(),
            name: "R2 production".to_string(),
            credential_env_prefix: "PANA_DEPLOY_R2".to_string(),
            cleanup_policy: DeployCleanupPolicy::ManagedOnly,
            provider: DeployTargetProvider::S3(S3TargetConfig {
                bucket: "site".to_string(),
                prefix: "public".to_string(),
                region: "auto".to_string(),
                endpoint: Some("https://account.r2.cloudflarestorage.com".to_string()),
                force_path_style: true,
                allow_insecure_endpoint: false,
                cache_control: Some("public, max-age=60".to_string()),
            }),
        }
    }

    fn credential() -> StoredDeployCredential {
        StoredDeployCredential::S3 {
            access_key_id: "access".to_string(),
            secret_access_key: "secret".to_string(),
            session_token: None,
        }
    }

    fn runtime(target: &DeployTarget) -> S3RuntimeConfig {
        S3RuntimeConfig::from_target(target, &credential()).unwrap()
    }

    fn artifact(files: &[(&str, &[u8])], artifact_id: &str) -> DeployArtifactManifest {
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
            artifact_id: artifact_id.to_string(),
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
    fn first_sync_uploads_files_then_manifest_with_checksum_metadata() {
        let transport = FakeS3Transport::default();
        let target = target();
        let runtime = runtime(&target);
        let artifact = artifact(&[("index.html", b"home"), ("app.js", b"js")], "artifact:1");
        let prepared = plan_s3_with_transport(&transport, &runtime, &target, 2, &artifact).unwrap();
        let sink = |_| {};
        let receipt = execute_s3_with_transport(
            &transport,
            &runtime,
            "operation",
            &target,
            2,
            &prepared.plan.plan_token,
            artifact,
            &|| false,
            &reporter("operation", &target, &sink),
        )
        .unwrap();

        assert_eq!(receipt.status, DeployReceiptStatus::Completed);
        assert_eq!(receipt.uploaded_files, 2);
        assert!(receipt.remote_manifest_published);
        let mutations = transport.mutations.borrow();
        assert_eq!(mutations.len(), 3);
        assert!(matches!(
            mutations.last(),
            Some(Mutation::Upload { key, cache_control, .. })
                if key == "public/.pana-deploy-manifest.json"
                    && cache_control.as_deref() == Some(S3_MANIFEST_CACHE_CONTROL)
        ));
    }

    #[test]
    fn retries_a_transient_idempotent_s3_upload() {
        let transport = FakeS3Transport::default();
        transport.transient_failures.set(1);
        let target = target();
        let runtime = runtime(&target);
        let artifact = artifact(&[("index.html", b"home")], "artifact:retry");
        let plan = plan_s3_with_transport(&transport, &runtime, &target, 1, &artifact).unwrap();
        let sink = |_| {};
        let receipt = execute_s3_with_transport(
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
        assert_eq!(transport.transient_failures.get(), 0);
    }

    #[test]
    fn cancellation_after_s3_upload_returns_partial_without_manifest_publish() {
        let transport = FakeS3Transport::default();
        let target = target();
        let runtime = runtime(&target);
        let artifact = artifact(&[("index.html", b"home")], "artifact:cancel");
        let plan = plan_s3_with_transport(&transport, &runtime, &target, 1, &artifact).unwrap();
        let checks = Cell::new(0);
        let sink = |_| {};
        let error = execute_s3_with_transport(
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
        assert!(!receipt.remote_manifest_published);
    }

    #[test]
    fn next_sync_skips_unchanged_and_deletes_only_manifest_owned_stale_key() {
        let transport = FakeS3Transport::default();
        let target = target();
        let runtime = runtime(&target);
        let first = artifact(
            &[("index.html", b"same"), ("old.txt", b"old")],
            "artifact:1",
        );
        let first_plan = plan_s3_with_transport(&transport, &runtime, &target, 1, &first).unwrap();
        let sink = |_| {};
        execute_s3_with_transport(
            &transport,
            &runtime,
            "first",
            &target,
            1,
            &first_plan.plan.plan_token,
            first,
            &|| false,
            &reporter("first", &target, &sink),
        )
        .unwrap();
        transport
            .objects
            .borrow_mut()
            .insert("public/foreign.txt".to_string(), b"foreign".to_vec());
        transport.mutations.borrow_mut().clear();

        let next = artifact(
            &[("index.html", b"same"), ("new.txt", b"new")],
            "artifact:2",
        );
        let next_plan = plan_s3_with_transport(&transport, &runtime, &target, 1, &next).unwrap();
        assert_eq!(next_plan.plan.skipped_files, 1);
        assert_eq!(next_plan.plan.upload_files, 1);
        assert_eq!(next_plan.plan.delete_files, 1);
        execute_s3_with_transport(
            &transport,
            &runtime,
            "next",
            &target,
            1,
            &next_plan.plan.plan_token,
            next,
            &|| false,
            &reporter("next", &target, &sink),
        )
        .unwrap();

        assert!(transport
            .objects
            .borrow()
            .contains_key("public/foreign.txt"));
        assert!(!transport.objects.borrow().contains_key("public/old.txt"));
        assert!(transport
            .mutations
            .borrow()
            .contains(&Mutation::Delete("public/old.txt".to_string())));
    }

    #[test]
    fn mirror_deletes_unmanaged_s3_object_and_respects_prefix_boundary() {
        let transport = FakeS3Transport::default();
        transport
            .objects
            .borrow_mut()
            .insert("public/foreign.txt".to_string(), b"foreign".to_vec());
        transport
            .objects
            .borrow_mut()
            .insert("publication/keep.txt".to_string(), b"keep".to_vec());
        let mut target = target();
        target.cleanup_policy = DeployCleanupPolicy::MirrorDestination;
        let runtime = runtime(&target);
        let artifact = artifact(&[("index.html", b"home")], "artifact:mirror");
        let plan = plan_s3_with_transport(&transport, &runtime, &target, 1, &artifact).unwrap();
        assert_eq!(plan.plan.unmanaged_delete_files, 1);
        let sink = |_| {};
        let receipt = execute_s3_with_transport(
            &transport,
            &runtime,
            "mirror",
            &target,
            1,
            &plan.plan.plan_token,
            artifact,
            &|| false,
            &reporter("mirror", &target, &sink),
        )
        .unwrap();
        assert_eq!(receipt.deleted_unmanaged_files, 1);
        assert!(!transport
            .objects
            .borrow()
            .contains_key("public/foreign.txt"));
        assert!(transport
            .objects
            .borrow()
            .contains_key("publication/keep.txt"));
    }

    #[test]
    fn upload_failure_after_a_mutation_returns_partial_receipt_without_manifest_publish() {
        let transport = FakeS3Transport::default();
        transport.fail_mutation_at.set(Some(1));
        let target = target();
        let runtime = runtime(&target);
        let artifact = artifact(&[("a.txt", b"a"), ("b.txt", b"b")], "artifact:1");
        let plan = plan_s3_with_transport(&transport, &runtime, &target, 1, &artifact).unwrap();
        let sink = |_| {};
        let error = execute_s3_with_transport(
            &transport,
            &runtime,
            "partial",
            &target,
            1,
            &plan.plan.plan_token,
            artifact,
            &|| false,
            &reporter("partial", &target, &sink),
        )
        .unwrap_err();

        assert_eq!(error.code, DeployErrorCode::UploadFailed);
        let receipt = error.receipt.unwrap();
        assert_eq!(receipt.status, DeployReceiptStatus::Partial);
        assert_eq!(receipt.uploaded_files, 1);
        assert!(!receipt.remote_manifest_published);
    }

    #[test]
    fn stale_plan_token_causes_zero_mutations() {
        let transport = FakeS3Transport::default();
        let target = target();
        let runtime = runtime(&target);
        let artifact = artifact(&[("index.html", b"home")], "artifact:1");
        let sink = |_| {};
        let error = execute_s3_with_transport(
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
        assert!(transport.mutations.borrow().is_empty());
    }

    #[test]
    fn invalid_remote_manifest_blocks_every_mutation() {
        let transport = FakeS3Transport::default();
        transport.objects.borrow_mut().insert(
            "public/.pana-deploy-manifest.json".to_string(),
            b"not json".to_vec(),
        );
        let target = target();
        let runtime = runtime(&target);
        let artifact = artifact(&[("index.html", b"home")], "artifact:1");
        let error =
            plan_s3_with_transport(&transport, &runtime, &target, 1, &artifact).unwrap_err();

        assert_eq!(error.code, DeployErrorCode::RemoteInventoryFailed);
        assert!(transport.mutations.borrow().is_empty());
    }
}
