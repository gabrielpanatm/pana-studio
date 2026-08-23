import { invoke } from "@tauri-apps/api/core";
import { t } from "$lib/i18n/runtime.svelte";
import {
  AUDIT_RULESET_VERSION,
  AUDIT_RUN_SCHEMA_VERSION,
} from "$lib/audit/contracts";
import {
  DEPLOY_CONFIGURATION_SCHEMA_VERSION,
  DEPLOY_CREDENTIAL_STATUS_SCHEMA_VERSION,
  DEPLOY_SETTINGS_SCHEMA_VERSION,
} from "$lib/deploy/contracts";
import {
  PUBLISH_BUILD_RECEIPT_SCHEMA_VERSION,
  PUBLISH_PREFLIGHT_SCHEMA_VERSION,
} from "$lib/deploy/contracts";
import type {
  DeployConfigurationSnapshot,
  DeployConnectionTestReceipt,
  DeployCredentialStatus,
  DeployCredentialWriteInput,
  DeployExecutionInput,
  DeployPlan,
  DeployPlanInput,
  DeployReceipt,
  DeploySettings,
} from "$lib/deploy/contracts";
import type {
  PublishBuildReceipt,
  PublishOperationCancelReceipt,
  PublishPreflightReceipt,
  PublishPreflightRequest,
} from "$lib/deploy/contracts";
import type { FileBufferRequestIdentity } from "$lib/project/workspace-contract";

function schemaMismatch(resource: string, actual: number, expected: number) {
  return new Error(t("io-schema-mismatch", { resource, actual, expected }));
}

export function validateDeploySettings(settings: DeploySettings) {
  if (settings.schemaVersion !== DEPLOY_SETTINGS_SCHEMA_VERSION) {
    throw schemaMismatch("DeploySettings", settings.schemaVersion, DEPLOY_SETTINGS_SCHEMA_VERSION);
  }
}

function validateDeployCredentialStatus(status: DeployCredentialStatus) {
  if (status.schemaVersion !== DEPLOY_CREDENTIAL_STATUS_SCHEMA_VERSION) {
    throw schemaMismatch(
      "DeployCredentialStatus",
      status.schemaVersion,
      DEPLOY_CREDENTIAL_STATUS_SCHEMA_VERSION,
    );
  }
}

function validateDeployConfigurationSnapshot(snapshot: DeployConfigurationSnapshot) {
  if (snapshot.schemaVersion !== DEPLOY_CONFIGURATION_SCHEMA_VERSION) {
    throw schemaMismatch(
      "DeployConfiguration",
      snapshot.schemaVersion,
      DEPLOY_CONFIGURATION_SCHEMA_VERSION,
    );
  }
  validateDeploySettings(snapshot.settings);
  snapshot.credentialStatuses.forEach(validateDeployCredentialStatus);
}

export async function readDeployConfiguration(): Promise<DeployConfigurationSnapshot> {
  const snapshot = await invoke<DeployConfigurationSnapshot>("read_deploy_configuration");
  validateDeployConfigurationSnapshot(snapshot);
  return snapshot;
}

export async function saveDeploySettings(
  settings: DeploySettings,
): Promise<DeployConfigurationSnapshot> {
  const snapshot = await invoke<DeployConfigurationSnapshot>("save_deploy_settings", { settings });
  validateDeployConfigurationSnapshot(snapshot);
  return snapshot;
}

export async function saveDeployCredential(
  targetId: string,
  credential: DeployCredentialWriteInput,
): Promise<DeployCredentialStatus> {
  const status = await invoke<DeployCredentialStatus>("save_deploy_credential", { targetId, credential });
  validateDeployCredentialStatus(status);
  return status;
}

export function deleteDeployCredential(credentialEnvPrefix: string): Promise<boolean> {
  return invoke<boolean>("delete_deploy_credential", { credentialEnvPrefix });
}

export function testDeployConnection(targetId: string): Promise<DeployConnectionTestReceipt> {
  return invoke<DeployConnectionTestReceipt>("test_deploy_connection", { targetId });
}

export async function runPublishPreflight(
  request: PublishPreflightRequest = { policyOverrides: [], suppressions: [] },
): Promise<PublishPreflightReceipt> {
  const receipt = await invoke<PublishPreflightReceipt>("run_publish_preflight", { request });
  validatePublishPreflightReceipt(receipt);
  return receipt;
}

export async function currentPublishPreflightReceipt(): Promise<PublishPreflightReceipt | null> {
  const receipt = await invoke<PublishPreflightReceipt | null>("current_publish_preflight_receipt");
  if (receipt) validatePublishPreflightReceipt(receipt);
  return receipt;
}

export async function buildForPublish(expectedPreflightToken: string): Promise<PublishBuildReceipt> {
  const receipt = await invoke<PublishBuildReceipt>("build_for_publish", {
    expectedPreflightToken,
  });
  validatePublishBuildReceipt(receipt);
  return receipt;
}

export async function currentPublishBuildReceipt(): Promise<PublishBuildReceipt | null> {
  const receipt = await invoke<PublishBuildReceipt | null>("current_publish_build_receipt");
  if (receipt) validatePublishBuildReceipt(receipt);
  return receipt;
}

export function planDeploy(input: DeployPlanInput): Promise<DeployPlan> {
  return invoke<DeployPlan>("plan_deploy", { input });
}

export function executeDeploy(input: DeployExecutionInput): Promise<DeployReceipt> {
  return invoke<DeployReceipt>("execute_deploy", { input });
}

export function cancelPublishOperation(
  identity: FileBufferRequestIdentity,
): Promise<PublishOperationCancelReceipt> {
  return invoke<PublishOperationCancelReceipt>("cancel_publish_operation", { identity });
}

function validatePublishPreflightReceipt(receipt: PublishPreflightReceipt) {
  if (
    receipt.schemaVersion !== PUBLISH_PREFLIGHT_SCHEMA_VERSION
    || receipt.auditReceipt.schemaVersion !== AUDIT_RUN_SCHEMA_VERSION
    || receipt.auditReceipt.rulesetVersion !== AUDIT_RULESET_VERSION
    || receipt.auditIdentity.schemaVersion !== AUDIT_RUN_SCHEMA_VERSION
    || receipt.auditIdentity.rulesetVersion !== AUDIT_RULESET_VERSION
    || !receipt.preflightToken.startsWith("sha256:")
    || !receipt.observedDiskFingerprint.startsWith("sha256:")
    || typeof receipt.workspaceDirty !== "boolean"
    || typeof receipt.diskCoherent !== "boolean"
    || !receipt.auditIdentity.receiptId.startsWith("sha256:")
    || !Array.isArray(receipt.gates)
    || !receipt.gates.every((gate) => (
      /^[a-z0-9_]+$/.test(gate.id)
      && Array.isArray(gate.evidence)
      && Array.isArray(gate.auditFingerprints)
      && Array.isArray(gate.remediations)
    ))
  ) {
    throw new Error(t("io-publish-preflight-receipt-invalid"));
  }
}

function validatePublishBuildReceipt(receipt: PublishBuildReceipt) {
  if (
    receipt.schemaVersion !== PUBLISH_BUILD_RECEIPT_SCHEMA_VERSION
    || !receipt.preflightToken.startsWith("sha256:")
    || !receipt.buildToken.startsWith("sha256:")
    || !receipt.artifactId.startsWith("sha256:")
  ) {
    throw new Error(t("io-publish-build-receipt-invalid"));
  }
}
