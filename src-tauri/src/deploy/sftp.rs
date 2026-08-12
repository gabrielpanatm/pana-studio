use std::{
    collections::{BTreeMap, VecDeque},
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    time::Duration,
};

use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};
use ssh2::{FileType, HashType, Session, Sftp};

use super::{
    artifact::{DeployArtifactFile, DeployArtifactManifest},
    credentials::StoredDeployCredential,
    engine::DeployProgressReporter,
    model::{
        validate_remote_prefix, DeployActionKind, DeployCleanupPolicy, DeployCommandError,
        DeployDeleteOrigin, DeployErrorCode, DeployPlan, DeployProgressPhase, DeployReceipt,
        DeployReceiptStatus, DeployTarget, DeployTargetProvider, SftpTargetConfig,
        DEPLOY_RECEIPT_SCHEMA_VERSION,
    },
    remote_manifest::{
        prepare_sync_plan, PreparedSync, RemoteInventoryFile, MAX_REMOTE_INVENTORY_FILES,
        MAX_REMOTE_MANIFEST_BYTES, REMOTE_MANIFEST_FILE_NAME,
    },
    retry::retry_idempotent,
};

const SFTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const SFTP_IO_TIMEOUT: Duration = Duration::from_secs(120);

pub(crate) fn plan_sftp_deploy(
    target: &DeployTarget,
    settings_revision: u64,
    artifact: &DeployArtifactManifest,
    credential: &StoredDeployCredential,
) -> Result<DeployPlan, DeployCommandError> {
    let runtime = SftpRuntimeConfig::from_target(target, credential).map_err(|message| {
        DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
    })?;
    let transport = Libssh2SftpTransport::connect(&runtime)
        .map_err(|message| DeployCommandError::new(DeployErrorCode::ConnectionFailed, message))?;
    plan_sftp_with_transport(&transport, &runtime, target, settings_revision, artifact)
        .map(|prepared| prepared.plan)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_sftp_deploy(
    operation_id: &str,
    target: &DeployTarget,
    settings_revision: u64,
    expected_plan_token: &str,
    artifact: DeployArtifactManifest,
    credential: StoredDeployCredential,
    is_cancelled: &dyn Fn() -> bool,
    progress: &DeployProgressReporter<'_>,
) -> Result<DeployReceipt, DeployCommandError> {
    let runtime = SftpRuntimeConfig::from_target(target, &credential).map_err(|message| {
        DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
    })?;
    let transport = Libssh2SftpTransport::connect(&runtime)
        .map_err(|message| DeployCommandError::new(DeployErrorCode::ConnectionFailed, message))?;
    execute_sftp_with_transport(
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

pub(crate) fn test_sftp_connection(
    target: &DeployTarget,
    credential: &StoredDeployCredential,
) -> Result<(), DeployCommandError> {
    let runtime = SftpRuntimeConfig::from_target(target, credential).map_err(|message| {
        DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
    })?;
    let transport = Libssh2SftpTransport::connect(&runtime)
        .map_err(|message| DeployCommandError::new(DeployErrorCode::ConnectionFailed, message))?;
    transport
        .validate_remote_root(&runtime.remote_root)
        .map_err(|message| DeployCommandError::new(DeployErrorCode::ConnectionFailed, message))
}

#[derive(Clone)]
enum SftpAuthentication {
    Password {
        username: String,
        password: String,
    },
    PrivateKey {
        username: String,
        private_key_pem: String,
        passphrase: Option<String>,
    },
}

#[derive(Clone)]
struct SftpRuntimeConfig {
    host: String,
    port: u16,
    remote_root: PathBuf,
    expected_host_key_sha256: String,
    authentication: SftpAuthentication,
}

impl SftpRuntimeConfig {
    fn from_target(
        target: &DeployTarget,
        credential: &StoredDeployCredential,
    ) -> Result<Self, String> {
        target.validate()?;
        let DeployTargetProvider::Sftp(SftpTargetConfig {
            host,
            port,
            remote_root,
            expected_host_key_sha256,
        }) = &target.provider
        else {
            return Err("Ținta nu este configurată pentru SFTP.".to_string());
        };
        let authentication = match credential {
            StoredDeployCredential::SftpPassword { username, password } => {
                SftpAuthentication::Password {
                    username: username.clone(),
                    password: password.clone(),
                }
            }
            StoredDeployCredential::SftpPrivateKey {
                username,
                private_key_pem,
                passphrase,
            } => SftpAuthentication::PrivateKey {
                username: username.clone(),
                private_key_pem: private_key_pem.clone(),
                passphrase: passphrase.clone(),
            },
            _ => {
                return Err("Credentialele țintei SFTP au un tip incompatibil.".to_string());
            }
        };
        Ok(Self {
            host: host.clone(),
            port: *port,
            remote_root: PathBuf::from(remote_root),
            expected_host_key_sha256: normalize_fingerprint(expected_host_key_sha256),
            authentication,
        })
    }

    fn remote_path(&self, relative_path: &str) -> PathBuf {
        self.remote_root.join(relative_path)
    }
}

trait SftpTransport {
    fn download_optional(&self, path: &Path) -> Result<Option<Vec<u8>>, String>;
    fn list_files(&self, root: &Path) -> Result<Vec<RemoteInventoryFile>, String>;
    fn upload(&self, path: &Path, bytes: Vec<u8>) -> Result<(), String>;
    fn delete(&self, path: &Path) -> Result<(), String>;
}

struct Libssh2SftpTransport {
    _session: Session,
    sftp: Sftp,
}

impl Libssh2SftpTransport {
    fn connect(runtime: &SftpRuntimeConfig) -> Result<Self, String> {
        let addresses = (runtime.host.as_str(), runtime.port)
            .to_socket_addrs()
            .map_err(|_| "Host-ul SFTP nu poate fi rezolvat.".to_string())?;
        let mut connected = None;
        for address in addresses {
            if let Ok(stream) = TcpStream::connect_timeout(&address, SFTP_CONNECT_TIMEOUT) {
                connected = Some(stream);
                break;
            }
        }
        let stream = connected.ok_or_else(|| "Conexiunea TCP SFTP a eșuat.".to_string())?;
        stream
            .set_read_timeout(Some(SFTP_IO_TIMEOUT))
            .map_err(|_| "Timeout-ul de citire SFTP nu poate fi configurat.".to_string())?;
        stream
            .set_write_timeout(Some(SFTP_IO_TIMEOUT))
            .map_err(|_| "Timeout-ul de scriere SFTP nu poate fi configurat.".to_string())?;

        let mut session = Session::new()
            .map_err(|_| "Sesiunea SSH pentru SFTP nu poate fi inițializată.".to_string())?;
        session.set_timeout(SFTP_IO_TIMEOUT.as_millis() as u32);
        session.set_tcp_stream(stream);
        session
            .handshake()
            .map_err(|_| "Handshake-ul SSH pentru SFTP a eșuat.".to_string())?;

        let host_key_hash = session
            .host_key_hash(HashType::Sha256)
            .ok_or_else(|| "Serverul SFTP nu a furnizat un host key SHA-256.".to_string())?;
        let actual_fingerprint = STANDARD_NO_PAD.encode(host_key_hash);
        if actual_fingerprint != runtime.expected_host_key_sha256 {
            return Err(format!(
                "Host key-ul SFTP nu corespunde fingerprint-ului configurat. Fingerprint observat: SHA256:{actual_fingerprint}."
            ));
        }

        match &runtime.authentication {
            SftpAuthentication::Password { username, password } => session
                .userauth_password(username, password)
                .map_err(|_| "Autentificarea SFTP cu parolă a eșuat.".to_string())?,
            SftpAuthentication::PrivateKey {
                username,
                private_key_pem,
                passphrase,
            } => session
                .userauth_pubkey_memory(username, None, private_key_pem, passphrase.as_deref())
                .map_err(|_| "Autentificarea SFTP cu cheia privată a eșuat.".to_string())?,
        }
        if !session.authenticated() {
            return Err("Autentificarea SFTP nu a fost confirmată.".to_string());
        }
        let sftp = session
            .sftp()
            .map_err(|_| "Sub-sistemul SFTP nu poate fi deschis.".to_string())?;
        Ok(Self {
            _session: session,
            sftp,
        })
    }

    fn validate_remote_root(&self, remote_root: &Path) -> Result<(), String> {
        let metadata = self
            .sftp
            .lstat(remote_root)
            .map_err(|_| "Root-ul SFTP configurat nu există sau nu poate fi citit.".to_string())?;
        if metadata.file_type() != FileType::Directory {
            return Err("Root-ul SFTP configurat nu este un director real.".to_string());
        }
        Ok(())
    }

    fn ensure_safe_parent_directories(&self, path: &Path) -> Result<(), String> {
        let parent = path
            .parent()
            .ok_or_else(|| "Path-ul SFTP nu are director părinte.".to_string())?;
        let mut current = PathBuf::from("/");
        for component in parent.components().skip(1) {
            current.push(component.as_os_str());
            match self.sftp.lstat(&current) {
                Ok(metadata) if metadata.file_type() == FileType::Directory => {}
                Ok(_) => {
                    return Err(format!(
                        "Path-ul SFTP '{}' traversează un nod care nu este director.",
                        current.display()
                    ));
                }
                Err(error) if is_sftp_not_found(&error) => {
                    self.sftp.mkdir(&current, 0o755).map_err(|_| {
                        format!("Directorul SFTP '{}' nu poate fi creat.", current.display())
                    })?;
                    let metadata = self.sftp.lstat(&current).map_err(|_| {
                        format!(
                            "Directorul SFTP '{}' nu poate fi verificat după creare.",
                            current.display()
                        )
                    })?;
                    if metadata.file_type() != FileType::Directory {
                        return Err(format!(
                            "Path-ul SFTP '{}' nu este un director real.",
                            current.display()
                        ));
                    }
                }
                Err(_) => {
                    return Err(format!(
                        "Directorul SFTP '{}' nu poate fi verificat.",
                        current.display()
                    ));
                }
            }
        }
        Ok(())
    }

    fn require_regular_or_missing(&self, path: &Path) -> Result<(), String> {
        match self.sftp.lstat(path) {
            Ok(metadata) if metadata.file_type() == FileType::RegularFile => Ok(()),
            Ok(_) => Err(format!(
                "Path-ul SFTP '{}' nu este un fișier regulat; operația a fost blocată.",
                path.display()
            )),
            Err(error) if is_sftp_not_found(&error) => Ok(()),
            Err(_) => Err(format!(
                "Path-ul SFTP '{}' nu poate fi verificat.",
                path.display()
            )),
        }
    }

    fn validate_safe_parent_directories(&self, path: &Path) -> Result<(), String> {
        let parent = path
            .parent()
            .ok_or_else(|| "Path-ul SFTP nu are director părinte.".to_string())?;
        let mut current = PathBuf::from("/");
        for component in parent.components().skip(1) {
            current.push(component.as_os_str());
            let metadata = self.sftp.lstat(&current).map_err(|_| {
                format!(
                    "Directorul SFTP '{}' nu poate fi verificat.",
                    current.display()
                )
            })?;
            if metadata.file_type() != FileType::Directory {
                return Err(format!(
                    "Path-ul SFTP '{}' traversează un nod care nu este director.",
                    current.display()
                ));
            }
        }
        Ok(())
    }
}

impl SftpTransport for Libssh2SftpTransport {
    fn download_optional(&self, path: &Path) -> Result<Option<Vec<u8>>, String> {
        match self.sftp.lstat(path) {
            Ok(metadata) if metadata.file_type() != FileType::RegularFile => {
                return Err("Manifestul SFTP remote nu este un fișier regulat.".to_string());
            }
            Ok(metadata)
                if metadata
                    .size
                    .is_some_and(|size| size > MAX_REMOTE_MANIFEST_BYTES as u64) =>
            {
                return Err("Manifestul SFTP remote depășește limita sigură.".to_string());
            }
            Ok(_) => {}
            Err(error) if is_sftp_not_found(&error) => return Ok(None),
            Err(_) => return Err("Manifestul SFTP remote nu poate fi verificat.".to_string()),
        }
        let file = self
            .sftp
            .open(path)
            .map_err(|_| "Manifestul SFTP remote nu poate fi deschis.".to_string())?;
        let mut bytes = Vec::new();
        file.take(MAX_REMOTE_MANIFEST_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Manifestul SFTP remote nu poate fi citit.".to_string())?;
        if bytes.len() > MAX_REMOTE_MANIFEST_BYTES {
            return Err("Manifestul SFTP remote depășește limita sigură.".to_string());
        }
        Ok(Some(bytes))
    }

    fn list_files(&self, root: &Path) -> Result<Vec<RemoteInventoryFile>, String> {
        self.validate_remote_root(root)?;
        let mut directories = VecDeque::from([root.to_path_buf()]);
        let mut inventory = Vec::new();
        let mut observed_entries = 0usize;
        while let Some(directory) = directories.pop_front() {
            let entries = self.sftp.readdir(&directory).map_err(|_| {
                format!(
                    "Directorul SFTP '{}' nu poate fi inventariat.",
                    directory.display()
                )
            })?;
            for (reported_path, metadata) in entries {
                observed_entries += 1;
                if observed_entries > MAX_REMOTE_INVENTORY_FILES {
                    return Err(format!(
                        "Inventarul SFTP depășește limita sigură de {MAX_REMOTE_INVENTORY_FILES} intrări."
                    ));
                }
                let name = reported_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| {
                        "Inventarul SFTP conține un nume care nu este UTF-8.".to_string()
                    })?;
                if matches!(name, "." | "..") {
                    continue;
                }
                if name.is_empty()
                    || name.contains('/')
                    || name.contains('\\')
                    || name.bytes().any(|byte| byte.is_ascii_control())
                {
                    return Err("Inventarul SFTP conține un nume de obiect nesigur.".to_string());
                }
                let path = directory.join(name);
                match metadata.file_type() {
                    FileType::Directory => directories.push_back(path),
                    FileType::RegularFile => {
                        let relative_path = path
                            .strip_prefix(root)
                            .ok()
                            .and_then(|path| path.to_str())
                            .map(|path| path.replace('\\', "/"))
                            .ok_or_else(|| {
                                "Inventarul SFTP a ieșit din root-ul configurat.".to_string()
                            })?;
                        validate_remote_prefix(&relative_path)?;
                        let size_bytes = metadata.size.ok_or_else(|| {
                            "Inventarul SFTP conține un fișier fără dimensiune.".to_string()
                        })?;
                        inventory.push(RemoteInventoryFile {
                            path: relative_path,
                            size_bytes,
                        });
                    }
                    _ => {
                        return Err(format!(
                            "Inventarul SFTP conține nodul nesigur '{}'.",
                            path.display()
                        ));
                    }
                }
            }
        }
        Ok(inventory)
    }

    fn upload(&self, path: &Path, bytes: Vec<u8>) -> Result<(), String> {
        self.ensure_safe_parent_directories(path)?;
        self.require_regular_or_missing(path)?;
        let mut file = self
            .sftp
            .create(path)
            .map_err(|_| format!("Fișierul SFTP '{}' nu poate fi creat.", path.display()))?;
        file.write_all(&bytes)
            .map_err(|_| format!("Fișierul SFTP '{}' nu poate fi scris.", path.display()))?;
        file.flush()
            .map_err(|_| format!("Fișierul SFTP '{}' nu poate fi finalizat.", path.display()))?;
        Ok(())
    }

    fn delete(&self, path: &Path) -> Result<(), String> {
        self.validate_safe_parent_directories(path)?;
        match self.sftp.unlink(path) {
            Ok(()) => Ok(()),
            Err(error) if is_sftp_not_found(&error) => Ok(()),
            Err(_) => Err(format!(
                "Fișierul SFTP '{}' nu poate fi șters.",
                path.display()
            )),
        }
    }
}

fn is_sftp_not_found(error: &ssh2::Error) -> bool {
    matches!(error.message(), "no such file" | "no such path")
}

fn normalize_fingerprint(fingerprint: &str) -> String {
    fingerprint
        .strip_prefix("SHA256:")
        .unwrap_or(fingerprint)
        .trim_end_matches('=')
        .to_string()
}

fn plan_sftp_with_transport<T: SftpTransport>(
    transport: &T,
    runtime: &SftpRuntimeConfig,
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
fn execute_sftp_with_transport<T: SftpTransport>(
    transport: &T,
    runtime: &SftpRuntimeConfig,
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
        plan_sftp_with_transport(transport, runtime, target, settings_revision, &artifact)?;
    if prepared.plan.plan_token != expected_plan_token {
        return Err(DeployCommandError::new(
            DeployErrorCode::InvalidConfiguration,
            "Planul deploy nu mai corespunde artifactului, configurației sau manifestului SFTP remote. Recalculează planul.",
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
            return Err(cancelled_sftp_error(receipt));
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
                "Planul SFTP referă un fișier care nu există în artifactul capturat.",
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
            return Err(cancelled_sftp_error(receipt));
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
        return Err(cancelled_sftp_error(receipt));
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

fn cancelled_sftp_error(mut receipt: DeployReceipt) -> DeployCommandError {
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
        "Deploy-ul SFTP a fost anulat; consultă receipt-ul pentru starea remote.",
    )
    .with_receipt(receipt)
}

#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        path::PathBuf,
    };

    use super::*;
    use crate::deploy::artifact::DeployArtifactFile;
    use sha2::{Digest, Sha256};

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum Mutation {
        Upload(PathBuf),
        Delete(PathBuf),
    }

    #[derive(Default)]
    struct FakeSftpTransport {
        files: RefCell<BTreeMap<PathBuf, Vec<u8>>>,
        mutations: RefCell<Vec<Mutation>>,
        fail_mutation_at: Cell<Option<usize>>,
        transient_failures: Cell<usize>,
    }

    impl FakeSftpTransport {
        fn maybe_fail(&self) -> Result<(), String> {
            if self.transient_failures.get() > 0 {
                self.transient_failures
                    .set(self.transient_failures.get() - 1);
                return Err("SFTP transient test failure".to_string());
            }
            if self.fail_mutation_at.get() == Some(self.mutations.borrow().len()) {
                return Err("SFTP test failure".to_string());
            }
            Ok(())
        }
    }

    impl SftpTransport for FakeSftpTransport {
        fn download_optional(&self, path: &Path) -> Result<Option<Vec<u8>>, String> {
            Ok(self.files.borrow().get(path).cloned())
        }

        fn list_files(&self, root: &Path) -> Result<Vec<RemoteInventoryFile>, String> {
            self.files
                .borrow()
                .iter()
                .filter(|(path, _)| path.starts_with(root))
                .map(|(path, bytes)| {
                    let relative_path = path
                        .strip_prefix(root)
                        .ok()
                        .and_then(|path| path.to_str())
                        .map(|path| path.trim_start_matches('/').replace('\\', "/"))
                        .ok_or_else(|| "Inventarul SFTP fake a ieșit din root.".to_string())?;
                    Ok(RemoteInventoryFile {
                        path: relative_path,
                        size_bytes: bytes.len() as u64,
                    })
                })
                .collect()
        }

        fn upload(&self, path: &Path, bytes: Vec<u8>) -> Result<(), String> {
            self.maybe_fail()?;
            self.mutations
                .borrow_mut()
                .push(Mutation::Upload(path.to_path_buf()));
            self.files.borrow_mut().insert(path.to_path_buf(), bytes);
            Ok(())
        }

        fn delete(&self, path: &Path) -> Result<(), String> {
            self.maybe_fail()?;
            self.mutations
                .borrow_mut()
                .push(Mutation::Delete(path.to_path_buf()));
            self.files.borrow_mut().remove(path);
            Ok(())
        }
    }

    fn target() -> DeployTarget {
        DeployTarget {
            id: "sftp-production".to_string(),
            name: "SFTP production".to_string(),
            credential_ref: "sftp-secret".to_string(),
            cleanup_policy: DeployCleanupPolicy::ManagedOnly,
            provider: DeployTargetProvider::Sftp(SftpTargetConfig {
                host: "example.com".to_string(),
                port: 22,
                remote_root: "/var/www/site".to_string(),
                expected_host_key_sha256: format!("SHA256:{}", STANDARD_NO_PAD.encode([42u8; 32])),
            }),
        }
    }

    fn credential() -> StoredDeployCredential {
        StoredDeployCredential::SftpPassword {
            username: "deploy".to_string(),
            password: "secret".to_string(),
        }
    }

    fn runtime(target: &DeployTarget) -> SftpRuntimeConfig {
        SftpRuntimeConfig::from_target(target, &credential()).unwrap()
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
    fn sync_publishes_manifest_last() {
        let transport = FakeSftpTransport::default();
        let target = target();
        let runtime = runtime(&target);
        let artifact = artifact(&[("index.html", b"home")], "artifact:1");
        let plan = plan_sftp_with_transport(&transport, &runtime, &target, 1, &artifact).unwrap();
        let sink = |_| {};
        let receipt = execute_sftp_with_transport(
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
    fn retries_a_transient_idempotent_sftp_upload() {
        let transport = FakeSftpTransport::default();
        transport.transient_failures.set(1);
        let target = target();
        let runtime = runtime(&target);
        let artifact = artifact(&[("index.html", b"home")], "artifact:retry");
        let plan = plan_sftp_with_transport(&transport, &runtime, &target, 1, &artifact).unwrap();
        let sink = |_| {};
        let receipt = execute_sftp_with_transport(
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
    fn cancellation_after_sftp_upload_returns_partial_without_manifest_publish() {
        let transport = FakeSftpTransport::default();
        let target = target();
        let runtime = runtime(&target);
        let artifact = artifact(&[("index.html", b"home")], "artifact:cancel");
        let plan = plan_sftp_with_transport(&transport, &runtime, &target, 1, &artifact).unwrap();
        let checks = Cell::new(0);
        let sink = |_| {};
        let error = execute_sftp_with_transport(
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
        let transport = FakeSftpTransport::default();
        let target = target();
        let runtime = runtime(&target);
        let first = artifact(&[("keep.txt", b"same"), ("old.txt", b"old")], "artifact:1");
        let first_plan =
            plan_sftp_with_transport(&transport, &runtime, &target, 1, &first).unwrap();
        let sink = |_| {};
        execute_sftp_with_transport(
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
        let next_plan = plan_sftp_with_transport(&transport, &runtime, &target, 1, &next).unwrap();
        execute_sftp_with_transport(
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
    fn mirror_deletes_unmanaged_sftp_file_and_reports_its_origin() {
        let transport = FakeSftpTransport::default();
        let mut target = target();
        target.cleanup_policy = DeployCleanupPolicy::MirrorDestination;
        let runtime = runtime(&target);
        transport
            .files
            .borrow_mut()
            .insert(runtime.remote_path("foreign.txt"), b"foreign".to_vec());
        let artifact = artifact(&[("index.html", b"home")], "artifact:mirror");
        let plan = plan_sftp_with_transport(&transport, &runtime, &target, 1, &artifact).unwrap();
        assert_eq!(plan.plan.unmanaged_delete_files, 1);
        let sink = |_| {};
        let receipt = execute_sftp_with_transport(
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
    fn failure_after_upload_reports_partial_without_publishing_manifest() {
        let transport = FakeSftpTransport::default();
        transport.fail_mutation_at.set(Some(1));
        let target = target();
        let runtime = runtime(&target);
        let artifact = artifact(&[("a.txt", b"a"), ("b.txt", b"b")], "artifact:1");
        let plan = plan_sftp_with_transport(&transport, &runtime, &target, 1, &artifact).unwrap();
        let sink = |_| {};
        let error = execute_sftp_with_transport(
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
    fn invalid_manifest_and_stale_token_make_no_mutations() {
        let transport = FakeSftpTransport::default();
        let target = target();
        let runtime = runtime(&target);
        let artifact = artifact(&[("index.html", b"home")], "artifact:1");
        transport.files.borrow_mut().insert(
            runtime.remote_path(REMOTE_MANIFEST_FILE_NAME),
            b"invalid".to_vec(),
        );
        assert!(plan_sftp_with_transport(&transport, &runtime, &target, 1, &artifact).is_err());
        assert!(transport.mutations.borrow().is_empty());
        transport.files.borrow_mut().clear();

        let sink = |_| {};
        let error = execute_sftp_with_transport(
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
    fn fingerprint_is_stored_in_openssh_canonical_form() {
        let target = target();
        let runtime = runtime(&target);
        assert_eq!(
            runtime.expected_host_key_sha256,
            STANDARD_NO_PAD.encode([42u8; 32])
        );
    }
}
