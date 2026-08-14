use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    io::{Cursor, Read},
    net::ToSocketAddrs,
    time::Duration,
};

use suppaftp::{
    list::File as FtpListFile, native_tls::TlsConnector, FtpError, NativeTlsConnector,
    NativeTlsFtpStream, Status,
};

use super::{
    artifact::{DeployArtifactFile, DeployArtifactManifest},
    credentials::StoredDeployCredential,
    engine::DeployProgressReporter,
    model::{
        validate_remote_prefix, DeployActionKind, DeployCleanupPolicy, DeployCommandError,
        DeployDeleteOrigin, DeployErrorCode, DeployPlan, DeployProgressPhase, DeployReceipt,
        DeployReceiptStatus, DeployTarget, DeployTargetProvider, FtpSecurityMode, FtpTargetConfig,
        DEPLOY_RECEIPT_SCHEMA_VERSION,
    },
    remote_manifest::{
        prepare_sync_plan, PreparedSync, RemoteInventoryFile, MAX_REMOTE_INVENTORY_FILES,
        MAX_REMOTE_MANIFEST_BYTES, REMOTE_MANIFEST_FILE_NAME,
    },
    retry::retry_idempotent,
};

const FTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const FTP_IO_TIMEOUT: Duration = Duration::from_secs(120);

pub(crate) fn plan_ftp_deploy(
    target: &DeployTarget,
    settings_revision: u64,
    artifact: &DeployArtifactManifest,
    credential: &StoredDeployCredential,
) -> Result<DeployPlan, DeployCommandError> {
    let runtime = FtpRuntimeConfig::from_target(target, credential).map_err(|message| {
        DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
    })?;
    let transport = SuppaFtpTransport::connect(&runtime)
        .map_err(|message| DeployCommandError::new(DeployErrorCode::ConnectionFailed, message))?;
    plan_ftp_with_transport(&transport, &runtime, target, settings_revision, artifact)
        .map(|prepared| prepared.plan)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_ftp_deploy(
    operation_id: &str,
    target: &DeployTarget,
    settings_revision: u64,
    expected_plan_token: &str,
    artifact: DeployArtifactManifest,
    credential: StoredDeployCredential,
    is_cancelled: &dyn Fn() -> bool,
    progress: &DeployProgressReporter<'_>,
) -> Result<DeployReceipt, DeployCommandError> {
    let runtime = FtpRuntimeConfig::from_target(target, &credential).map_err(|message| {
        DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
    })?;
    let transport = SuppaFtpTransport::connect(&runtime)
        .map_err(|message| DeployCommandError::new(DeployErrorCode::ConnectionFailed, message))?;
    execute_ftp_with_transport(
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

pub(crate) fn test_ftp_connection(
    target: &DeployTarget,
    credential: &StoredDeployCredential,
) -> Result<(), DeployCommandError> {
    let runtime = FtpRuntimeConfig::from_target(target, credential).map_err(|message| {
        DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
    })?;
    let transport = SuppaFtpTransport::connect(&runtime)
        .map_err(|message| DeployCommandError::new(DeployErrorCode::ConnectionFailed, message))?;
    transport
        .validate_remote_root(&runtime.remote_root)
        .map_err(|message| DeployCommandError::new(DeployErrorCode::ConnectionFailed, message))
}

#[derive(Clone)]
struct FtpRuntimeConfig {
    host: String,
    port: u16,
    remote_root: String,
    security: FtpSecurityMode,
    username: String,
    password: String,
}

impl FtpRuntimeConfig {
    fn from_target(
        target: &DeployTarget,
        credential: &StoredDeployCredential,
    ) -> Result<Self, String> {
        target.validate()?;
        let DeployTargetProvider::Ftp(FtpTargetConfig {
            host,
            port,
            remote_root,
            security,
            ..
        }) = &target.provider
        else {
            return Err("Ținta nu este configurată pentru FTP/FTPS.".to_string());
        };
        let StoredDeployCredential::Ftp { username, password } = credential else {
            return Err("Credentialele țintei FTP/FTPS au un tip incompatibil.".to_string());
        };
        Ok(Self {
            host: host.clone(),
            port: *port,
            remote_root: remote_root.clone(),
            security: *security,
            username: username.clone(),
            password: password.clone(),
        })
    }

    fn remote_path(&self, relative_path: &str) -> String {
        if self.remote_root == "/" {
            format!("/{relative_path}")
        } else {
            format!("{}/{relative_path}", self.remote_root)
        }
    }
}

trait FtpTransport {
    fn download_optional(&self, path: &str) -> Result<Option<Vec<u8>>, String>;
    fn list_files(&self, root: &str) -> Result<Vec<RemoteInventoryFile>, String>;
    fn upload(&self, path: &str, bytes: Vec<u8>) -> Result<(), String>;
    fn delete(&self, path: &str) -> Result<(), String>;
}

struct SuppaFtpTransport {
    stream: RefCell<NativeTlsFtpStream>,
}

impl SuppaFtpTransport {
    fn connect(runtime: &FtpRuntimeConfig) -> Result<Self, String> {
        let addresses = (runtime.host.as_str(), runtime.port)
            .to_socket_addrs()
            .map_err(|_| "Host-ul FTP/FTPS nu poate fi rezolvat.".to_string())?;
        let mut connected = None;
        for address in addresses {
            if let Ok(stream) = NativeTlsFtpStream::connect_timeout(address, FTP_CONNECT_TIMEOUT) {
                connected = Some(stream);
                break;
            }
        }
        let mut stream = connected.ok_or_else(|| "Conexiunea TCP FTP/FTPS a eșuat.".to_string())?;
        stream
            .get_ref()
            .set_read_timeout(Some(FTP_IO_TIMEOUT))
            .map_err(|_| "Timeout-ul de citire FTP/FTPS nu poate fi configurat.".to_string())?;
        stream
            .get_ref()
            .set_write_timeout(Some(FTP_IO_TIMEOUT))
            .map_err(|_| "Timeout-ul de scriere FTP/FTPS nu poate fi configurat.".to_string())?;

        if runtime.security == FtpSecurityMode::FtpsExplicit {
            let connector = TlsConnector::builder()
                .build()
                .map_err(|_| "Clientul TLS pentru FTPS nu poate fi inițializat.".to_string())?;
            stream = stream
                .into_secure(NativeTlsConnector::from(connector), &runtime.host)
                .map_err(|_| {
                    "Negocierea explicit FTPS cu verificarea certificatului a eșuat.".to_string()
                })?;
        }
        stream
            .login(&runtime.username, &runtime.password)
            .map_err(|_| "Autentificarea FTP/FTPS a eșuat.".to_string())?;
        Ok(Self {
            stream: RefCell::new(stream),
        })
    }

    fn validate_remote_root(&self, remote_root: &str) -> Result<(), String> {
        let mut stream = self.stream.borrow_mut();
        stream
            .cwd(remote_root)
            .map_err(|_| "Root-ul FTP/FTPS configurat nu este accesibil.".to_string())?;
        stream
            .noop()
            .map_err(|_| "Serverul FTP/FTPS nu a confirmat conexiunea.".to_string())
    }

    fn ensure_parent_directories(&self, path: &str) -> Result<(), String> {
        let parent = path
            .rsplit_once('/')
            .map(|(parent, _)| parent)
            .ok_or_else(|| "Path-ul FTP/FTPS nu are director părinte.".to_string())?;
        let mut current = String::new();
        let mut stream = self.stream.borrow_mut();
        for segment in parent.split('/').filter(|segment| !segment.is_empty()) {
            current.push('/');
            current.push_str(segment);
            if stream.cwd(&current).is_ok() {
                continue;
            }
            stream.mkdir(&current).map_err(|_| {
                format!(
                    "Directorul FTP/FTPS '{}' nu poate fi creat sau verificat.",
                    current
                )
            })?;
            stream.cwd(&current).map_err(|_| {
                format!(
                    "Directorul FTP/FTPS '{}' nu poate fi verificat după creare.",
                    current
                )
            })?;
        }
        Ok(())
    }
}

impl Drop for SuppaFtpTransport {
    fn drop(&mut self) {
        let _ = self.stream.get_mut().quit();
    }
}

impl FtpTransport for SuppaFtpTransport {
    fn download_optional(&self, path: &str) -> Result<Option<Vec<u8>>, String> {
        let mut stream = self.stream.borrow_mut();
        let bytes = match stream.retr(path, |reader| {
            let mut bytes = Vec::new();
            reader
                .take(MAX_REMOTE_MANIFEST_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(FtpError::ConnectionError)?;
            Ok(bytes)
        }) {
            Ok(bytes) => bytes,
            Err(error) if ftp_file_unavailable(&error) => return Ok(None),
            Err(_) => return Err("Manifestul FTP/FTPS remote nu poate fi citit.".to_string()),
        };
        if bytes.len() > MAX_REMOTE_MANIFEST_BYTES {
            return Err("Manifestul FTP/FTPS remote depășește limita sigură.".to_string());
        }
        Ok(Some(bytes))
    }

    fn list_files(&self, root: &str) -> Result<Vec<RemoteInventoryFile>, String> {
        let mut directories = VecDeque::from([root.to_string()]);
        let mut inventory = Vec::new();
        let mut observed_entries = 0usize;
        let mut stream = self.stream.borrow_mut();
        while let Some(directory) = directories.pop_front() {
            let lines = stream.mlsd(Some(&directory)).map_err(|_| {
                format!("Directorul FTP/FTPS '{directory}' nu poate fi inventariat prin MLSD.")
            })?;
            for line in lines {
                observed_entries += 1;
                if observed_entries > MAX_REMOTE_INVENTORY_FILES {
                    return Err(format!(
                        "Inventarul FTP/FTPS depășește limita sigură de {MAX_REMOTE_INVENTORY_FILES} intrări."
                    ));
                }
                let entry = FtpListFile::try_from(line.as_str()).map_err(|_| {
                    format!("Inventarul FTP/FTPS din '{directory}' conține o intrare invalidă.")
                })?;
                let name = entry.name();
                if matches!(name, "." | "..") {
                    continue;
                }
                if name.is_empty()
                    || name.contains('/')
                    || name.contains('\\')
                    || name.bytes().any(|byte| byte.is_ascii_control())
                {
                    return Err(
                        "Inventarul FTP/FTPS conține un nume de obiect nesigur.".to_string()
                    );
                }
                let full_path = join_ftp_path(&directory, name);
                if entry.is_directory() {
                    directories.push_back(full_path);
                } else if entry.is_file() {
                    let relative_path = ftp_relative_path(root, &full_path)?;
                    validate_remote_prefix(&relative_path)?;
                    inventory.push(RemoteInventoryFile {
                        path: relative_path,
                        size_bytes: entry.size() as u64,
                    });
                } else {
                    return Err(format!(
                        "Inventarul FTP/FTPS conține linkul simbolic nesigur '{full_path}'."
                    ));
                }
            }
        }
        Ok(inventory)
    }

    fn upload(&self, path: &str, bytes: Vec<u8>) -> Result<(), String> {
        self.ensure_parent_directories(path)?;
        let expected_bytes = bytes.len() as u64;
        let written = self
            .stream
            .borrow_mut()
            .put_file(path, &mut Cursor::new(bytes))
            .map_err(|_| format!("Fișierul FTP/FTPS '{path}' nu poate fi încărcat."))?;
        if written != expected_bytes {
            return Err(format!(
                "Fișierul FTP/FTPS '{path}' a fost încărcat incomplet."
            ));
        }
        Ok(())
    }

    fn delete(&self, path: &str) -> Result<(), String> {
        match self.stream.borrow_mut().rm(path) {
            Ok(()) => Ok(()),
            Err(error) if ftp_file_unavailable(&error) => Ok(()),
            Err(_) => Err(format!("Fișierul FTP/FTPS '{path}' nu poate fi șters.")),
        }
    }
}

fn join_ftp_path(directory: &str, name: &str) -> String {
    if directory == "/" {
        format!("/{name}")
    } else {
        format!("{}/{name}", directory.trim_end_matches('/'))
    }
}

fn ftp_relative_path(root: &str, full_path: &str) -> Result<String, String> {
    let relative = if root == "/" {
        full_path.strip_prefix('/')
    } else {
        full_path
            .strip_prefix(root)
            .and_then(|path| path.strip_prefix('/'))
    }
    .ok_or_else(|| "Inventarul FTP/FTPS a ieșit din root-ul configurat.".to_string())?;
    if relative.is_empty() {
        return Err("Inventarul FTP/FTPS conține un path gol.".to_string());
    }
    Ok(relative.to_string())
}

fn ftp_file_unavailable(error: &FtpError) -> bool {
    matches!(
        error,
        FtpError::UnexpectedResponse(response) if response.status == Status::FileUnavailable
    )
}

fn plan_ftp_with_transport<T: FtpTransport>(
    transport: &T,
    runtime: &FtpRuntimeConfig,
    target: &DeployTarget,
    settings_revision: u64,
    artifact: &DeployArtifactManifest,
) -> Result<PreparedSync, DeployCommandError> {
    let manifest_path = runtime.remote_path(REMOTE_MANIFEST_FILE_NAME);
    let remote_manifest = retry_idempotent(|| transport.download_optional(&manifest_path))
        .map_err(|message| {
            DeployCommandError::new(DeployErrorCode::RemoteInventoryFailed, message)
        })?;
    let remote_inventory = if target.cleanup_policy == DeployCleanupPolicy::MirrorDestination {
        Some(
            retry_idempotent(|| transport.list_files(&runtime.remote_root)).map_err(|message| {
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

#[allow(clippy::too_many_arguments)]
fn execute_ftp_with_transport<T: FtpTransport>(
    transport: &T,
    runtime: &FtpRuntimeConfig,
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
        plan_ftp_with_transport(transport, runtime, target, settings_revision, &artifact)?;
    if prepared.plan.plan_token != expected_plan_token {
        return Err(DeployCommandError::new(
            DeployErrorCode::InvalidConfiguration,
            "Planul deploy nu mai corespunde artifactului, configurației sau manifestului FTP/FTPS remote. Recalculează planul.",
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
            return Err(cancelled_ftp_error(receipt));
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
                "Planul FTP/FTPS referă un fișier care nu există în artifactul capturat.",
            )
        })?;
        if let Err(message) = retry_idempotent(|| {
            transport.upload(
                &runtime.remote_path(&file.relative_path),
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
            return Err(cancelled_ftp_error(receipt));
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
            retry_idempotent(|| transport.delete(&runtime.remote_path(&action.path)))
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
        return Err(cancelled_ftp_error(receipt));
    }
    progress.emit(
        DeployProgressPhase::Activating,
        Some(REMOTE_MANIFEST_FILE_NAME.to_string()),
        completed_mutations,
        total_mutations,
        receipt.uploaded_bytes,
        prepared.plan.upload_bytes,
    );
    if let Err(message) = retry_idempotent(|| {
        transport.upload(
            &runtime.remote_path(REMOTE_MANIFEST_FILE_NAME),
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

fn cancelled_ftp_error(mut receipt: DeployReceipt) -> DeployCommandError {
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
        "Deploy-ul FTP/FTPS a fost anulat; consultă receipt-ul pentru starea remote.",
    )
    .with_receipt(receipt)
}

#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        path::PathBuf,
    };

    use sha2::{Digest, Sha256};

    use super::*;
    use crate::deploy::artifact::DeployArtifactFile;

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum Mutation {
        Upload(String),
        Delete(String),
    }

    #[derive(Default)]
    struct FakeFtpTransport {
        files: RefCell<BTreeMap<String, Vec<u8>>>,
        mutations: RefCell<Vec<Mutation>>,
        fail_mutation_at: Cell<Option<usize>>,
        transient_failures: Cell<usize>,
    }

    impl FakeFtpTransport {
        fn maybe_fail(&self) -> Result<(), String> {
            if self.transient_failures.get() > 0 {
                self.transient_failures
                    .set(self.transient_failures.get() - 1);
                return Err("FTP transient test failure".to_string());
            }
            if self.fail_mutation_at.get() == Some(self.mutations.borrow().len()) {
                return Err("FTP test failure".to_string());
            }
            Ok(())
        }
    }

    impl FtpTransport for FakeFtpTransport {
        fn download_optional(&self, path: &str) -> Result<Option<Vec<u8>>, String> {
            Ok(self.files.borrow().get(path).cloned())
        }

        fn list_files(&self, root: &str) -> Result<Vec<RemoteInventoryFile>, String> {
            self.files
                .borrow()
                .iter()
                .filter(|(path, _)| {
                    if root == "/" {
                        path.starts_with('/')
                    } else {
                        path.starts_with(&format!("{root}/"))
                    }
                })
                .map(|(path, bytes)| {
                    Ok(RemoteInventoryFile {
                        path: ftp_relative_path(root, path)?,
                        size_bytes: bytes.len() as u64,
                    })
                })
                .collect()
        }

        fn upload(&self, path: &str, bytes: Vec<u8>) -> Result<(), String> {
            self.maybe_fail()?;
            self.mutations
                .borrow_mut()
                .push(Mutation::Upload(path.to_string()));
            self.files.borrow_mut().insert(path.to_string(), bytes);
            Ok(())
        }

        fn delete(&self, path: &str) -> Result<(), String> {
            self.maybe_fail()?;
            self.mutations
                .borrow_mut()
                .push(Mutation::Delete(path.to_string()));
            self.files.borrow_mut().remove(path);
            Ok(())
        }
    }

    fn target(security: FtpSecurityMode, allow_insecure_ftp: bool) -> DeployTarget {
        DeployTarget {
            id: "ftp-production".to_string(),
            name: "FTP production".to_string(),
            credential_env_prefix: "PANA_DEPLOY_FTP".to_string(),
            cleanup_policy: DeployCleanupPolicy::ManagedOnly,
            provider: DeployTargetProvider::Ftp(FtpTargetConfig {
                host: "example.com".to_string(),
                port: 21,
                remote_root: "/public_html".to_string(),
                security,
                allow_insecure_ftp,
            }),
        }
    }

    fn credential() -> StoredDeployCredential {
        StoredDeployCredential::Ftp {
            username: "deploy".to_string(),
            password: "secret".to_string(),
        }
    }

    fn runtime(target: &DeployTarget) -> FtpRuntimeConfig {
        FtpRuntimeConfig::from_target(target, &credential()).unwrap()
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
    fn ftps_sync_publishes_manifest_last() {
        let transport = FakeFtpTransport::default();
        let target = target(FtpSecurityMode::FtpsExplicit, false);
        let runtime = runtime(&target);
        let artifact = artifact(&[("index.html", b"home")], "artifact:1");
        let plan = plan_ftp_with_transport(&transport, &runtime, &target, 1, &artifact).unwrap();
        let sink = |_| {};
        let receipt = execute_ftp_with_transport(
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
        assert!(matches!(
            transport.mutations.borrow().last(),
            Some(Mutation::Upload(path)) if path.ends_with(REMOTE_MANIFEST_FILE_NAME)
        ));
    }

    #[test]
    fn retries_a_transient_idempotent_ftp_upload() {
        let transport = FakeFtpTransport::default();
        transport.transient_failures.set(1);
        let target = target(FtpSecurityMode::FtpsExplicit, false);
        let runtime = runtime(&target);
        let artifact = artifact(&[("index.html", b"home")], "artifact:retry");
        let plan = plan_ftp_with_transport(&transport, &runtime, &target, 1, &artifact).unwrap();
        let sink = |_| {};
        let receipt = execute_ftp_with_transport(
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
    fn cancellation_after_ftp_upload_returns_partial_without_manifest_publish() {
        let transport = FakeFtpTransport::default();
        let target = target(FtpSecurityMode::FtpsExplicit, false);
        let runtime = runtime(&target);
        let artifact = artifact(&[("index.html", b"home")], "artifact:cancel");
        let plan = plan_ftp_with_transport(&transport, &runtime, &target, 1, &artifact).unwrap();
        let checks = Cell::new(0);
        let sink = |_| {};
        let error = execute_ftp_with_transport(
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
    fn stale_owned_file_is_deleted_but_foreign_file_is_preserved() {
        let transport = FakeFtpTransport::default();
        let target = target(FtpSecurityMode::FtpsExplicit, false);
        let runtime = runtime(&target);
        let first = artifact(&[("keep.txt", b"same"), ("old.txt", b"old")], "artifact:1");
        let first_plan = plan_ftp_with_transport(&transport, &runtime, &target, 1, &first).unwrap();
        let sink = |_| {};
        execute_ftp_with_transport(
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
            .files
            .borrow_mut()
            .insert(runtime.remote_path("foreign.txt"), b"foreign".to_vec());
        transport.mutations.borrow_mut().clear();

        let next = artifact(&[("keep.txt", b"same")], "artifact:2");
        let next_plan = plan_ftp_with_transport(&transport, &runtime, &target, 1, &next).unwrap();
        execute_ftp_with_transport(
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
            .files
            .borrow()
            .contains_key(&runtime.remote_path("foreign.txt")));
        assert!(!transport
            .files
            .borrow()
            .contains_key(&runtime.remote_path("old.txt")));
    }

    #[test]
    fn mirror_deletes_unmanaged_ftp_file_and_reports_its_origin() {
        let transport = FakeFtpTransport::default();
        let mut target = target(FtpSecurityMode::FtpsExplicit, false);
        target.cleanup_policy = DeployCleanupPolicy::MirrorDestination;
        let runtime = runtime(&target);
        transport
            .files
            .borrow_mut()
            .insert(runtime.remote_path("foreign.txt"), b"foreign".to_vec());
        let artifact = artifact(&[("index.html", b"home")], "artifact:mirror");
        let plan = plan_ftp_with_transport(&transport, &runtime, &target, 1, &artifact).unwrap();
        assert_eq!(plan.plan.unmanaged_delete_files, 1);
        let sink = |_| {};
        let receipt = execute_ftp_with_transport(
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
            .files
            .borrow()
            .contains_key(&runtime.remote_path("foreign.txt")));
    }

    #[test]
    fn failure_after_upload_reports_partial_without_manifest_publish() {
        let transport = FakeFtpTransport::default();
        transport.fail_mutation_at.set(Some(1));
        let target = target(FtpSecurityMode::FtpsExplicit, false);
        let runtime = runtime(&target);
        let artifact = artifact(&[("a.txt", b"a"), ("b.txt", b"b")], "artifact:1");
        let plan = plan_ftp_with_transport(&transport, &runtime, &target, 1, &artifact).unwrap();
        let sink = |_| {};
        let error = execute_ftp_with_transport(
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

        let receipt = error.receipt.unwrap();
        assert_eq!(receipt.status, DeployReceiptStatus::Partial);
        assert_eq!(receipt.uploaded_files, 1);
        assert!(!receipt.remote_manifest_published);
    }

    #[test]
    fn plain_ftp_requires_explicit_opt_in_and_surfaces_warning() {
        let blocked = target(FtpSecurityMode::Plain, false);
        assert!(blocked.validate().is_err());

        let allowed = target(FtpSecurityMode::Plain, true);
        allowed.validate().unwrap();
        assert!(allowed
            .security_warnings()
            .iter()
            .any(|warning| warning.contains("necriptat")));
    }

    #[test]
    fn invalid_manifest_and_stale_token_make_no_mutations() {
        let transport = FakeFtpTransport::default();
        let target = target(FtpSecurityMode::FtpsExplicit, false);
        let runtime = runtime(&target);
        let artifact = artifact(&[("index.html", b"home")], "artifact:1");
        transport.files.borrow_mut().insert(
            runtime.remote_path(REMOTE_MANIFEST_FILE_NAME),
            b"invalid".to_vec(),
        );
        assert!(plan_ftp_with_transport(&transport, &runtime, &target, 1, &artifact).is_err());
        assert!(transport.mutations.borrow().is_empty());
        transport.files.borrow_mut().clear();

        let sink = |_| {};
        let error = execute_ftp_with_transport(
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
}
