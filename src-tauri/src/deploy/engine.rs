use super::{
    artifact::DeployArtifactManifest,
    bunny::{execute_bunny_deploy, plan_bunny_deploy, test_bunny_connection},
    cloudflare_pages::{
        execute_cloudflare_pages_deploy, plan_cloudflare_pages_deploy,
        test_cloudflare_pages_connection,
    },
    credentials::StoredDeployCredential,
    ftp::{execute_ftp_deploy, plan_ftp_deploy, test_ftp_connection},
    model::{
        DeployCommandError, DeployConnectionTestReceipt, DeployPlan, DeployProgressEvent,
        DeployProgressPhase, DeployReceipt, DeployTarget, DeployTargetProvider,
        DEPLOY_CONNECTION_TEST_SCHEMA_VERSION, DEPLOY_PROGRESS_SCHEMA_VERSION,
    },
    s3::{execute_s3_deploy, plan_s3_deploy, test_s3_connection},
    sftp::{execute_sftp_deploy, plan_sftp_deploy, test_sftp_connection},
};

pub(crate) struct DeployProgressReporter<'a> {
    operation_id: &'a str,
    target: &'a DeployTarget,
    sink: &'a dyn Fn(DeployProgressEvent),
}

impl<'a> DeployProgressReporter<'a> {
    pub(crate) fn new(
        operation_id: &'a str,
        target: &'a DeployTarget,
        sink: &'a dyn Fn(DeployProgressEvent),
    ) -> Self {
        Self {
            operation_id,
            target,
            sink,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit(
        &self,
        phase: DeployProgressPhase,
        current_path: Option<String>,
        completed_files: u64,
        total_files: u64,
        completed_bytes: u64,
        total_bytes: u64,
    ) {
        (self.sink)(DeployProgressEvent {
            schema_version: DEPLOY_PROGRESS_SCHEMA_VERSION,
            operation_id: self.operation_id.to_string(),
            target_id: self.target.id.clone(),
            provider: self.target.provider_kind(),
            phase,
            current_path,
            completed_files,
            total_files,
            completed_bytes,
            total_bytes,
            timestamp_ms: crate::kernel::observability::now_ms(),
        });
    }
}

pub(crate) fn plan_deploy_with_artifact(
    target: &DeployTarget,
    settings_revision: u64,
    artifact: &DeployArtifactManifest,
    credential: &StoredDeployCredential,
) -> Result<DeployPlan, DeployCommandError> {
    match &target.provider {
        DeployTargetProvider::Bunny(_) => {
            plan_bunny_deploy(target, settings_revision, artifact, credential)
        }
        DeployTargetProvider::S3(_) => {
            plan_s3_deploy(target, settings_revision, artifact, credential)
        }
        DeployTargetProvider::Sftp(_) => {
            plan_sftp_deploy(target, settings_revision, artifact, credential)
        }
        DeployTargetProvider::Ftp(_) => {
            plan_ftp_deploy(target, settings_revision, artifact, credential)
        }
        DeployTargetProvider::CloudflarePages(_) => {
            plan_cloudflare_pages_deploy(target, settings_revision, artifact, credential)
        }
    }
}

pub(crate) fn test_deploy_connection_with_credential(
    target: &DeployTarget,
    credential: &StoredDeployCredential,
) -> Result<DeployConnectionTestReceipt, DeployCommandError> {
    let observed_remote_objects = match &target.provider {
        DeployTargetProvider::Bunny(_) => {
            test_bunny_connection(target, credential)?;
            None
        }
        DeployTargetProvider::S3(_) => Some(test_s3_connection(target, credential)?),
        DeployTargetProvider::Sftp(_) => {
            test_sftp_connection(target, credential)?;
            None
        }
        DeployTargetProvider::Ftp(_) => {
            test_ftp_connection(target, credential)?;
            None
        }
        DeployTargetProvider::CloudflarePages(_) => {
            test_cloudflare_pages_connection(target, credential)?;
            None
        }
    };
    Ok(DeployConnectionTestReceipt {
        schema_version: DEPLOY_CONNECTION_TEST_SCHEMA_VERSION,
        target_id: target.id.clone(),
        provider: target.provider_kind(),
        checked_at_ms: crate::kernel::observability::now_ms(),
        observed_remote_objects,
        warnings: target.security_warnings(),
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_deploy_with_artifact(
    operation_id: &str,
    target: &DeployTarget,
    settings_revision: u64,
    expected_plan_token: &str,
    artifact: DeployArtifactManifest,
    credential: StoredDeployCredential,
    is_cancelled: &dyn Fn() -> bool,
    progress: &DeployProgressReporter<'_>,
) -> Result<DeployReceipt, DeployCommandError> {
    match &target.provider {
        DeployTargetProvider::Bunny(_) => execute_bunny_deploy(
            operation_id,
            target,
            settings_revision,
            expected_plan_token,
            artifact,
            credential,
            is_cancelled,
            progress,
        ),
        DeployTargetProvider::S3(_) => execute_s3_deploy(
            operation_id,
            target,
            settings_revision,
            expected_plan_token,
            artifact,
            credential,
            is_cancelled,
            progress,
        ),
        DeployTargetProvider::Sftp(_) => execute_sftp_deploy(
            operation_id,
            target,
            settings_revision,
            expected_plan_token,
            artifact,
            credential,
            is_cancelled,
            progress,
        ),
        DeployTargetProvider::Ftp(_) => execute_ftp_deploy(
            operation_id,
            target,
            settings_revision,
            expected_plan_token,
            artifact,
            credential,
            is_cancelled,
            progress,
        ),
        DeployTargetProvider::CloudflarePages(_) => execute_cloudflare_pages_deploy(
            operation_id,
            target,
            settings_revision,
            expected_plan_token,
            artifact,
            credential,
            is_cancelled,
            progress,
        ),
    }
}
