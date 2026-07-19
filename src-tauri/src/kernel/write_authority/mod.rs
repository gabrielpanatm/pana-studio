use std::{fmt, path::Path};

use serde::Serialize;

mod authority;
mod boundary;
mod capability;
#[cfg(test)]
mod compliance;
mod model;
mod operation;
mod recovery;
mod registry;
mod root_authority;
#[cfg(test)]
pub(crate) mod test_support;
mod tree_fingerprint;

pub use authority::WriteAuthority;
pub use model::{
    ConflictPolicy, ExpectedLeaf, ExpectedLeafVersion, RecoveryPolicy, WriteAtomicity,
    WriteAuthorityError, WriteCategory, WriteIntent, WriteOperationKind, WriteOwner, WritePolicy,
    WriteReceipt, WriteRecoveryReceipt, WriteRejection, WriteTarget,
};
pub use recovery::{
    WalPhase, WriteAuthorityRecoveryClassification, WriteAuthorityRecoveryItem,
    WriteAuthorityRecoveryResolutionAction, WriteAuthorityRecoveryResolutionInput,
    WriteAuthorityRecoveryResolutionReceipt, WriteAuthorityRecoveryScan,
};
pub use registry::{known_write_declarations, WriteDeclaration};
pub(crate) use root_authority::{ActiveProjectReadLease, CodexConfigLease, ProjectBootstrapLease};
pub use root_authority::{ApplicationAuthorityPaths, WriteAuthorityRuntime};
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub(crate) enum CapabilityMaintenanceError {
    Rejected(WriteRejection),
    RecoveryRequired(CapabilityMaintenanceRecovery),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CapabilityMaintenanceRecovery {
    pub changed: bool,
    pub bytes_written: u64,
    pub diagnostic: String,
    retry_forbidden: bool,
}

#[cfg(test)]
impl CapabilityMaintenanceRecovery {
    pub const fn retry_forbidden(&self) -> bool {
        self.retry_forbidden
    }
}

impl CapabilityMaintenanceError {
    pub fn into_terminal_diagnostic(self) -> String {
        self.to_string()
    }
}

impl From<String> for CapabilityMaintenanceError {
    fn from(diagnostic: String) -> Self {
        Self::Rejected(WriteRejection::new(diagnostic))
    }
}

impl From<&str> for CapabilityMaintenanceError {
    fn from(diagnostic: &str) -> Self {
        Self::Rejected(WriteRejection::new(diagnostic))
    }
}

impl fmt::Display for CapabilityMaintenanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(rejection) => formatter.write_str(&rejection.diagnostic),
            Self::RecoveryRequired(recovery) => write!(
                formatter,
                "CAPABILITY_MAINTENANCE_RECOVERY_REQUIRED: {} Nu repeta operația automat.",
                recovery.diagnostic
            ),
        }
    }
}

impl std::error::Error for CapabilityMaintenanceError {}

fn require_durable_maintenance_effect(
    effect: capability::CapabilityEffect,
) -> Result<capability::CapabilityEffect, CapabilityMaintenanceError> {
    if effect.recovery_required {
        return Err(CapabilityMaintenanceError::RecoveryRequired(
            CapabilityMaintenanceRecovery {
                changed: effect.changed,
                bytes_written: effect.bytes_written,
                diagnostic: effect.diagnostic.unwrap_or_else(|| {
                    "Efectul maintenance este vizibil, dar durabilitatea lui este incertă."
                        .to_string()
                }),
                retry_forbidden: true,
            },
        ));
    }
    Ok(effect)
}

pub(crate) fn capability_append_observability_file(
    runtime: Option<&WriteAuthorityRuntime>,
    path: &Path,
    boundary_root: &Path,
    public_label: &str,
    bytes: &[u8],
) -> Result<u64, CapabilityMaintenanceError> {
    let target = observability_target(runtime, path, boundary_root, public_label)?;
    let effect = capability::append(&target, bytes)?;
    require_durable_maintenance_effect(effect).map(|effect| effect.bytes_written)
}

pub(crate) enum CapabilityMaintenanceLockMode {
    Shared,
    Exclusive,
}

pub(crate) struct CapabilityMaintenanceLock {
    _inner: capability::CapabilityFileLock,
}

/// Held directory capability used only to hand a stable working directory to
/// a trusted subprocess. The child receives `.` after `current_dir` resolves
/// this descriptor-backed path, never the original mutable pathname.
pub(crate) struct CapabilitySubprocessDirectory {
    inner: capability::CapabilityDirectoryLease,
}

impl CapabilitySubprocessDirectory {
    pub fn current_dir_path(&self) -> std::path::PathBuf {
        self.inner.current_dir_path()
    }

    pub fn require_empty(&self) -> Result<(), CapabilityMaintenanceError> {
        self.inner.require_empty().map_err(Into::into)
    }
}

pub(crate) fn capability_capture_subprocess_directory(
    lease: &ProjectBootstrapLease,
    path: &Path,
    public_label: &str,
) -> Result<CapabilitySubprocessDirectory, CapabilityMaintenanceError> {
    capability::capture_directory_lease_from_authority(lease.authority(), path, public_label)
        .map(|inner| CapabilitySubprocessDirectory { inner })
        .map_err(Into::into)
}

pub(crate) fn capability_open_regular_file_readonly_no_follow(
    path: &Path,
    public_label: &str,
) -> Result<std::fs::File, String> {
    capability::open_regular_file_readonly_no_follow(path, public_label)
}

pub(crate) fn capability_open_optional_regular_file_readonly_no_follow(
    path: &Path,
    public_label: &str,
) -> Result<Option<std::fs::File>, String> {
    capability::open_optional_regular_file_readonly_no_follow(path, public_label)
}

pub(crate) fn capability_lock_observability_file(
    runtime: Option<&WriteAuthorityRuntime>,
    path: &Path,
    boundary_root: &Path,
    public_label: &str,
    mode: CapabilityMaintenanceLockMode,
) -> Result<CapabilityMaintenanceLock, CapabilityMaintenanceError> {
    let target = observability_target(runtime, path, boundary_root, public_label)?;
    let mode = match mode {
        CapabilityMaintenanceLockMode::Shared => capability::CapabilityLockMode::Shared,
        CapabilityMaintenanceLockMode::Exclusive => capability::CapabilityLockMode::Exclusive,
    };
    capability::lock_file(&target, mode)
        .map(|inner| CapabilityMaintenanceLock { _inner: inner })
        .map_err(Into::into)
}

pub(crate) fn capability_remove_observability_file(
    runtime: Option<&WriteAuthorityRuntime>,
    path: &Path,
    boundary_root: &Path,
    public_label: &str,
) -> Result<bool, CapabilityMaintenanceError> {
    let target = observability_target(runtime, path, boundary_root, public_label)?;
    let effect = capability::remove_file_if_exists_maintenance(&target)?;
    require_durable_maintenance_effect(effect).map(|effect| effect.changed)
}

pub(crate) fn capability_rename_observability_file(
    runtime: Option<&WriteAuthorityRuntime>,
    source: &Path,
    destination: &Path,
    boundary_root: &Path,
    source_label: &str,
    destination_label: &str,
) -> Result<(), CapabilityMaintenanceError> {
    let source = observability_target(runtime, source, boundary_root, source_label)?;
    let destination = observability_target(runtime, destination, boundary_root, destination_label)?;
    let effect = capability::rename_noreplace(&source, &destination)?;
    require_durable_maintenance_effect(effect).map(|_| ())
}

fn observability_target(
    runtime: Option<&WriteAuthorityRuntime>,
    path: &Path,
    boundary_root: &Path,
    public_label: &str,
) -> Result<WriteTarget, CapabilityMaintenanceError> {
    if let Some(runtime) = runtime {
        let authority = runtime.observability_authority(path)?;
        return WriteTarget::new(path, boundary_root, public_label)
            .bind_authority(authority)
            .map_err(Into::into);
    }
    #[cfg(test)]
    {
        return Ok(WriteTarget::new(path, boundary_root, public_label));
    }
    #[cfg(not(test))]
    Err("Observability capability cere WriteAuthorityRuntime instalat.".into())
}

#[cfg(test)]
mod maintenance_tests {
    use super::{
        require_durable_maintenance_effect, CapabilityMaintenanceError,
        CapabilityMaintenanceRecovery,
    };
    use crate::kernel::write_authority::capability::CapabilityEffect;

    #[test]
    fn maintenance_adapter_never_flattens_recovery_required_effect() {
        let effect = CapabilityEffect {
            changed: true,
            bytes_written: 12,
            recovery_required: true,
            diagnostic: Some("directory fsync failed".to_string()),
        };

        let error = require_durable_maintenance_effect(effect).unwrap_err();
        let CapabilityMaintenanceError::RecoveryRequired(recovery) = error else {
            panic!("maintenance recovery must not become a zero-effect rejection");
        };
        assert_eq!(
            recovery,
            CapabilityMaintenanceRecovery {
                changed: true,
                bytes_written: 12,
                diagnostic: "directory fsync failed".to_string(),
                retry_forbidden: true,
            }
        );
        assert!(recovery.retry_forbidden());
    }

    #[test]
    fn maintenance_adapter_returns_only_durable_effects_as_ok() {
        let effect = CapabilityEffect {
            changed: false,
            bytes_written: 0,
            recovery_required: false,
            diagnostic: None,
        };

        assert_eq!(
            require_durable_maintenance_effect(effect).unwrap().changed,
            false
        );
    }
}
