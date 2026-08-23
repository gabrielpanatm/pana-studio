import type {
  AUDIT_RULESET_VERSION,
  AUDIT_RUN_SCHEMA_VERSION,
  AuditLocation,
  AuditPolicyOverride,
  AuditRunReceipt,
  AuditSuppression,
} from "$lib/audit/contracts";
import type { LocalizedDiagnostic } from "$lib/contracts/localized-diagnostic";

export type DeployProviderKind = "bunny" | "ftp" | "sftp" | "s3" | "cloudflare_pages";

type DeployCleanupPolicy = "managed_only" | "mirror_destination";

type BunnyTargetConfig = {
  storageZone: string;
  storageRegion: string;
  pullZoneId: string;
  remotePrefix: string;
};

type FtpSecurityMode = "ftps_explicit" | "plain";

type FtpTargetConfig = {
  host: string;
  port: number;
  remoteRoot: string;
  security: FtpSecurityMode;
  allowInsecureFtp: boolean;
};

type SftpTargetConfig = {
  host: string;
  port: number;
  remoteRoot: string;
  expectedHostKeySha256: string;
};

type S3TargetConfig = {
  bucket: string;
  prefix: string;
  region: string;
  endpoint: string | null;
  forcePathStyle: boolean;
  allowInsecureEndpoint: boolean;
  cacheControl: string | null;
};

type CloudflarePagesTargetConfig = {
  accountId: string;
  projectName: string;
  branch: string | null;
};

type DeployTargetProvider =
  | { provider: "bunny"; config: BunnyTargetConfig }
  | { provider: "ftp"; config: FtpTargetConfig }
  | { provider: "sftp"; config: SftpTargetConfig }
  | { provider: "s3"; config: S3TargetConfig }
  | { provider: "cloudflare_pages"; config: CloudflarePagesTargetConfig };

export type DeployTarget = {
  id: string;
  name: string;
  credentialEnvPrefix: string;
  cleanupPolicy: DeployCleanupPolicy;
} & DeployTargetProvider;

export type DeploySettings = {
  schemaVersion: typeof DEPLOY_SETTINGS_SCHEMA_VERSION;
  revision: number;
  activeTargetId: string | null;
  targets: DeployTarget[];
};

export const DEPLOY_SETTINGS_SCHEMA_VERSION = 1 as const;

export const DEPLOY_CREDENTIAL_STATUS_SCHEMA_VERSION = 2 as const;

export const DEPLOY_CONFIGURATION_SCHEMA_VERSION = 2 as const;

type ProviderCapabilities = {
  remoteInventory: boolean;
  deleteStale: boolean;
  atomicActivation: boolean;
  cacheInvalidation: boolean;
  metadataHeaders: boolean;
  connectionTest: boolean;
};

export type DeployCredentialKind =
  | "bunny"
  | "ftp"
  | "sftp_password"
  | "sftp_private_key"
  | "s3"
  | "cloudflare_pages";

export type DeployCredentialStatus = {
  schemaVersion: typeof DEPLOY_CREDENTIAL_STATUS_SCHEMA_VERSION;
  credentialEnvPrefix: string;
  kind: DeployCredentialKind;
  configured: boolean;
  missingFields: string[];
};

type DeployTargetCapabilitySnapshot = {
  targetId: string;
  provider: DeployProviderKind;
  capabilities: ProviderCapabilities;
};

export type DeployConfigurationSnapshot = {
  schemaVersion: typeof DEPLOY_CONFIGURATION_SCHEMA_VERSION;
  settings: DeploySettings;
  credentialStatuses: DeployCredentialStatus[];
  targetCapabilities: DeployTargetCapabilitySnapshot[];
};

export type DeployCredentialWriteInput =
  | { credentialEnvPrefix: string; kind: "bunny"; storageKey: string; cdnApiKey: string }
  | { credentialEnvPrefix: string; kind: "ftp"; username: string; password: string }
  | { credentialEnvPrefix: string; kind: "sftp_password"; username: string; password: string }
  | {
      credentialEnvPrefix: string;
      kind: "sftp_private_key";
      username: string;
      privateKeyPem: string;
      passphrase: string | null;
    }
  | {
      credentialEnvPrefix: string;
      kind: "s3";
      accessKeyId: string;
      secretAccessKey: string;
      sessionToken: string | null;
    }
  | { credentialEnvPrefix: string; kind: "cloudflare_pages"; apiToken: string };

type DeployActionKind = "upload" | "skip" | "delete";

type DeployDeleteOrigin = "managed" | "unmanaged";

type DeployAction = {
  kind: DeployActionKind;
  path: string;
  sizeBytes: number;
  sha256?: string;
  deleteOrigin?: DeployDeleteOrigin;
};

export type DeployPlan = {
  schemaVersion: number;
  planToken: string;
  settingsRevision: number;
  targetId: string;
  provider: DeployProviderKind;
  artifactId: string;
  preflightToken: string;
  buildToken: string;
  uploadFiles: number;
  uploadBytes: number;
  skippedFiles: number;
  deleteFiles: number;
  managedDeleteFiles: number;
  unmanagedDeleteFiles: number;
  actions: DeployAction[];
  warnings: string[];
};

export type DeployPlanInput = {
  targetId: string;
  expectedBuildToken: string;
  expectedArtifactId: string;
};

export type DeployExecutionInput = {
  targetId: string;
  expectedSettingsRevision: number;
  expectedPlanToken: string;
  expectedPreflightToken: string;
  expectedBuildToken: string;
  expectedArtifactId: string;
};

type DeployProgressPhase =
  | "preparing"
  | "inventory"
  | "uploading"
  | "deleting"
  | "activating"
  | "invalidating_cache"
  | "completed"
  | "failed"
  | "cancelled";

export type DeployProgressEvent = {
  schemaVersion: number;
  operationId: string;
  targetId: string;
  provider: DeployProviderKind;
  phase: DeployProgressPhase;
  currentPath?: string;
  completedFiles: number;
  totalFiles: number;
  completedBytes: number;
  totalBytes: number;
  timestampMs: number;
};

export type DeployConnectionTestReceipt = {
  schemaVersion: number;
  targetId: string;
  provider: DeployProviderKind;
  checkedAtMs: number;
  observedRemoteObjects?: number;
  warnings: string[];
};

type DeployReceiptStatus = "completed" | "failed" | "cancelled" | "partial";

export type DeployReceipt = {
  schemaVersion: number;
  operationId: string;
  targetId: string;
  provider: DeployProviderKind;
  artifactId: string;
  planToken: string;
  settingsRevision: number;
  status: DeployReceiptStatus;
  startedAtMs: number;
  completedAtMs: number;
  uploadedFiles: number;
  uploadedBytes: number;
  skippedFiles: number;
  deletedFiles: number;
  deletedManagedFiles: number;
  deletedUnmanagedFiles: number;
  remoteManifestPublished: boolean;
  cacheInvalidated: boolean;
  deploymentId?: string;
  deploymentUrl?: string;
  warnings: string[];
};

type DeployErrorCode =
  | "invalid_configuration"
  | "missing_credentials"
  | "artifact_unavailable"
  | "connection_failed"
  | "remote_inventory_failed"
  | "upload_failed"
  | "delete_failed"
  | "activation_failed"
  | "cache_invalidation_failed"
  | "cancelled"
  | "internal";

export type DeployCommandError = {
  schemaVersion: number;
  code: DeployErrorCode;
  message: string;
  receipt?: DeployReceipt;
};

export const PUBLISH_PREFLIGHT_SCHEMA_VERSION = 1 as const;

export const PUBLISH_BUILD_RECEIPT_SCHEMA_VERSION = 1 as const;

type PublishPreflightStatus = "ready" | "blocked" | "failed";

type PublishPreflightGateOutcome =
  | "passed"
  | "blocked"
  | "advisory"
  | "skipped"
  | "engine_error";

type PublishPreflightEvidenceKind =
  | "workspace"
  | "disk"
  | "audit"
  | "build"
  | "deploy_configuration"
  | "credentials"
  | "runtime";

type PublishPreflightRemediationKind =
  | "save_workspace"
  | "reconcile_disk"
  | "open_audit"
  | "open_source"
  | "configure_deploy"
  | "configure_credentials"
  | "retry";

type PublishPreflightEvidence = {
  kind: PublishPreflightEvidenceKind;
  diagnostic: LocalizedDiagnostic;
  value?: string;
};

export type PublishPreflightRemediation = {
  kind: PublishPreflightRemediationKind;
  diagnostic: LocalizedDiagnostic;
  location?: AuditLocation;
};

export type PublishPreflightGate = {
  id: string;
  outcome: PublishPreflightGateOutcome;
  diagnostic: LocalizedDiagnostic;
  evidence: PublishPreflightEvidence[];
  auditFingerprints: string[];
  remediations: PublishPreflightRemediation[];
};

type PublishPreflightTargetIdentity = {
  targetId: string;
  provider: string;
  credentialEnvPrefix: string;
  credentialKind: string;
  credentialConfigured: boolean;
};

export type PublishPreflightReceipt = {
  schemaVersion: typeof PUBLISH_PREFLIGHT_SCHEMA_VERSION;
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number;
  diskGeneration: number;
  workspaceDirty: boolean;
  diskCoherent: boolean;
  observedDiskFingerprint: string;
  projectModelRevision: string;
  deploySettingsRevision: number;
  deploySettingsFingerprint: string;
  activeTarget?: PublishPreflightTargetIdentity;
  auditIdentity: {
    schemaVersion: typeof AUDIT_RUN_SCHEMA_VERSION;
    rulesetVersion: typeof AUDIT_RULESET_VERSION;
    receiptId: string;
    projectModelRevision: string;
  };
  auditReceipt: AuditRunReceipt;
  status: PublishPreflightStatus;
  gates: PublishPreflightGate[];
  preflightToken: string;
};

export type PublishBuildReceipt = {
  schemaVersion: typeof PUBLISH_BUILD_RECEIPT_SCHEMA_VERSION;
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number;
  diskGeneration: number;
  projectModelRevision: string;
  deploySettingsRevision: number;
  deploySettingsFingerprint: string;
  targetId: string;
  preflightToken: string;
  artifactId: string;
  artifactFiles: number;
  artifactBytes: number;
  completedAtMs: number;
  buildToken: string;
  log: string;
};

export type PublishPreflightRequest = {
  policyOverrides: AuditPolicyOverride[];
  suppressions: AuditSuppression[];
};

export type AuditRefreshResult =
  | { ok: true; receipt: AuditRunReceipt }
  | { ok: false; error: string; stale: boolean };

type PublishOperationKind = "build" | "deploy";

export type PublishOperationCancelReceipt = {
  schemaVersion: 1;
  operationId: string;
  kind: PublishOperationKind;
  cancellationRequested: boolean;
};
