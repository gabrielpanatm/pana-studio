use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    artifact::DeployArtifactManifest,
    model::{
        validate_remote_prefix, DeployAction, DeployActionKind, DeployCleanupPolicy,
        DeployDeleteOrigin, DeployPlan, DeployProviderKind, DeployTarget,
        DEPLOY_PLAN_SCHEMA_VERSION,
    },
};

pub(crate) const REMOTE_MANIFEST_FILE_NAME: &str = ".pana-deploy-manifest.json";
pub(crate) const REMOTE_MANIFEST_SCHEMA_VERSION: u32 = 1;
const REMOTE_MANIFEST_OWNER: &str = "pana-studio";
pub(crate) const MAX_REMOTE_MANIFEST_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_REMOTE_INVENTORY_FILES: usize = 250_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RemoteInventoryFile {
    pub path: String,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteDeployManifest {
    schema_version: u32,
    owner: String,
    target_id: String,
    provider: DeployProviderKind,
    artifact_id: String,
    files: Vec<RemoteDeployFile>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteDeployFile {
    path: String,
    size_bytes: u64,
    sha256: String,
}

#[derive(Debug)]
pub(crate) struct PreparedSync {
    pub plan: DeployPlan,
    pub next_manifest_bytes: Vec<u8>,
}

impl RemoteDeployManifest {
    fn from_artifact(target: &DeployTarget, artifact: &DeployArtifactManifest) -> Self {
        Self {
            schema_version: REMOTE_MANIFEST_SCHEMA_VERSION,
            owner: REMOTE_MANIFEST_OWNER.to_string(),
            target_id: target.id.clone(),
            provider: target.provider_kind(),
            artifact_id: artifact.artifact_id.clone(),
            files: artifact
                .files
                .iter()
                .map(|file| RemoteDeployFile {
                    path: file.relative_path.clone(),
                    size_bytes: file.bytes.len() as u64,
                    sha256: file.sha256_uppercase.clone(),
                })
                .collect(),
        }
    }

    fn parse(bytes: &[u8], target: &DeployTarget) -> Result<Self, String> {
        if bytes.len() > MAX_REMOTE_MANIFEST_BYTES {
            return Err(format!(
                "Manifestul remote Pana depășește limita de {MAX_REMOTE_MANIFEST_BYTES} bytes."
            ));
        }
        let manifest: Self = serde_json::from_slice(bytes)
            .map_err(|_| "Manifestul remote Pana nu este JSON valid.".to_string())?;
        manifest.validate(target)?;
        Ok(manifest)
    }

    fn validate(&self, target: &DeployTarget) -> Result<(), String> {
        if self.schema_version != REMOTE_MANIFEST_SCHEMA_VERSION
            || self.owner != REMOTE_MANIFEST_OWNER
        {
            return Err("Manifestul remote nu este un manifest Pana suportat.".to_string());
        }
        if self.target_id != target.id || self.provider != target.provider_kind() {
            return Err(
                "Manifestul remote aparține altei ținte sau altui provider; ștergerea a fost blocată."
                    .to_string(),
            );
        }
        let mut paths = BTreeSet::new();
        for file in &self.files {
            validate_remote_prefix(&file.path)?;
            if file.path == REMOTE_MANIFEST_FILE_NAME {
                return Err(
                    "Manifestul remote nu se poate declara pe sine ca artifact.".to_string()
                );
            }
            if !paths.insert(file.path.as_str()) {
                return Err(format!(
                    "Manifestul remote conține path-ul duplicat '{}'.",
                    file.path
                ));
            }
            if file.sha256.len() != 64
                || !file
                    .sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_lowercase())
            {
                return Err(format!(
                    "Manifestul remote conține un checksum invalid pentru '{}'.",
                    file.path
                ));
            }
        }
        Ok(())
    }

    fn to_json_bytes(&self) -> Result<Vec<u8>, String> {
        let mut bytes = serde_json::to_vec_pretty(self)
            .map_err(|error| format!("Manifestul remote nu poate fi serializat: {error}."))?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

pub(crate) fn prepare_sync_plan(
    target: &DeployTarget,
    settings_revision: u64,
    artifact: &DeployArtifactManifest,
    remote_manifest_bytes: Option<&[u8]>,
    remote_inventory: Option<&[RemoteInventoryFile]>,
) -> Result<PreparedSync, String> {
    target.validate()?;
    if artifact
        .files
        .iter()
        .any(|file| file.relative_path == REMOTE_MANIFEST_FILE_NAME)
    {
        return Err(format!(
            "Artifactul conține path-ul rezervat {REMOTE_MANIFEST_FILE_NAME}."
        ));
    }
    let previous = remote_manifest_bytes
        .map(|bytes| RemoteDeployManifest::parse(bytes, target))
        .transpose()?;
    let next = RemoteDeployManifest::from_artifact(target, artifact);

    let previous_files: BTreeMap<&str, &RemoteDeployFile> = previous
        .as_ref()
        .map(|manifest| {
            manifest
                .files
                .iter()
                .map(|file| (file.path.as_str(), file))
                .collect()
        })
        .unwrap_or_default();
    let inventory_files = validated_inventory(remote_inventory)?;
    if target.cleanup_policy == DeployCleanupPolicy::MirrorDestination && remote_inventory.is_none()
    {
        return Err(
            "Oglindirea completă necesită inventarierea integrală a destinației remote."
                .to_string(),
        );
    }
    let next_paths: BTreeSet<&str> = next.files.iter().map(|file| file.path.as_str()).collect();

    let mut actions = Vec::with_capacity(
        artifact.files.len() + previous.as_ref().map_or(0, |manifest| manifest.files.len()),
    );
    let mut upload_files = 0u64;
    let mut upload_bytes = 0u64;
    let mut skipped_files = 0u64;
    for file in &next.files {
        let unchanged = previous_files
            .get(file.path.as_str())
            .is_some_and(|previous| {
                previous.size_bytes == file.size_bytes && previous.sha256 == file.sha256
            })
            && (target.cleanup_policy == DeployCleanupPolicy::ManagedOnly
                || inventory_files
                    .get(file.path.as_str())
                    .is_some_and(|remote| remote.size_bytes == file.size_bytes));
        let kind = if unchanged {
            skipped_files += 1;
            DeployActionKind::Skip
        } else {
            upload_files += 1;
            upload_bytes = upload_bytes.saturating_add(file.size_bytes);
            DeployActionKind::Upload
        };
        actions.push(DeployAction {
            kind,
            path: file.path.clone(),
            size_bytes: file.size_bytes,
            sha256: Some(file.sha256.clone()),
            delete_origin: None,
        });
    }

    let mut delete_files = 0u64;
    let mut managed_delete_files = 0u64;
    let mut unmanaged_delete_files = 0u64;
    if target.cleanup_policy == DeployCleanupPolicy::MirrorDestination {
        for (path, remote) in &inventory_files {
            if *path == REMOTE_MANIFEST_FILE_NAME || next_paths.contains(path) {
                continue;
            }
            let previous_file = previous_files.get(path);
            let delete_origin = if previous_file.is_some() {
                managed_delete_files += 1;
                DeployDeleteOrigin::Managed
            } else {
                unmanaged_delete_files += 1;
                DeployDeleteOrigin::Unmanaged
            };
            delete_files += 1;
            actions.push(DeployAction {
                kind: DeployActionKind::Delete,
                path: (*path).to_string(),
                size_bytes: remote.size_bytes,
                sha256: previous_file.map(|file| file.sha256.clone()),
                delete_origin: Some(delete_origin),
            });
        }
    } else if let Some(previous) = previous.as_ref() {
        for file in &previous.files {
            if !next_paths.contains(file.path.as_str()) {
                delete_files += 1;
                managed_delete_files += 1;
                actions.push(DeployAction {
                    kind: DeployActionKind::Delete,
                    path: file.path.clone(),
                    size_bytes: file.size_bytes,
                    sha256: Some(file.sha256.clone()),
                    delete_origin: Some(DeployDeleteOrigin::Managed),
                });
            }
        }
    }

    let plan_token = plan_token(
        target,
        settings_revision,
        artifact,
        previous.as_ref(),
        &actions,
    );
    Ok(PreparedSync {
        plan: DeployPlan {
            schema_version: DEPLOY_PLAN_SCHEMA_VERSION,
            plan_token,
            settings_revision,
            target_id: target.id.clone(),
            provider: target.provider_kind(),
            artifact_id: artifact.artifact_id.clone(),
            preflight_token: String::new(),
            build_token: String::new(),
            upload_files,
            upload_bytes,
            skipped_files,
            delete_files,
            managed_delete_files,
            unmanaged_delete_files,
            actions,
            warnings: target.security_warnings(),
        },
        next_manifest_bytes: next.to_json_bytes()?,
    })
}

fn validated_inventory(
    remote_inventory: Option<&[RemoteInventoryFile]>,
) -> Result<BTreeMap<&str, &RemoteInventoryFile>, String> {
    let Some(remote_inventory) = remote_inventory else {
        return Ok(BTreeMap::new());
    };
    if remote_inventory.len() > MAX_REMOTE_INVENTORY_FILES {
        return Err(format!(
            "Inventarul remote depășește limita sigură de {MAX_REMOTE_INVENTORY_FILES} fișiere."
        ));
    }
    let mut files = BTreeMap::new();
    for file in remote_inventory {
        validate_remote_prefix(&file.path)?;
        if file.path.is_empty() {
            return Err("Inventarul remote conține un path gol.".to_string());
        }
        if files.insert(file.path.as_str(), file).is_some() {
            return Err(format!(
                "Inventarul remote conține path-ul duplicat '{}'.",
                file.path
            ));
        }
    }
    Ok(files)
}

fn plan_token(
    target: &DeployTarget,
    settings_revision: u64,
    artifact: &DeployArtifactManifest,
    previous: Option<&RemoteDeployManifest>,
    actions: &[DeployAction],
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"pana-deploy-plan-v1\0");
    hash_field(&mut digest, target.id.as_bytes());
    digest.update(settings_revision.to_be_bytes());
    hash_field(&mut digest, target.provider_kind().as_str().as_bytes());
    hash_field(&mut digest, artifact.artifact_id.as_bytes());
    hash_field(
        &mut digest,
        previous
            .map(|manifest| manifest.artifact_id.as_bytes())
            .unwrap_or_default(),
    );
    for action in actions {
        digest.update([match action.kind {
            DeployActionKind::Upload => 1,
            DeployActionKind::Skip => 2,
            DeployActionKind::Delete => 3,
        }]);
        hash_field(&mut digest, action.path.as_bytes());
        digest.update(action.size_bytes.to_be_bytes());
        hash_field(
            &mut digest,
            action.sha256.as_deref().unwrap_or_default().as_bytes(),
        );
        digest.update([match action.delete_origin {
            Some(DeployDeleteOrigin::Managed) => 1,
            Some(DeployDeleteOrigin::Unmanaged) => 2,
            None => 0,
        }]);
    }
    format!("plan:{:x}", digest.finalize())
}

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_be_bytes());
    digest.update(bytes);
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::deploy::{artifact::DeployArtifactFile, BunnyTargetConfig, DeployTargetProvider};

    fn target() -> DeployTarget {
        DeployTarget {
            id: "production".to_string(),
            name: "Production".to_string(),
            credential_env_prefix: "PANA_DEPLOY_PRODUCTION".to_string(),
            cleanup_policy: DeployCleanupPolicy::ManagedOnly,
            provider: DeployTargetProvider::Bunny(BunnyTargetConfig {
                storage_zone: "site".to_string(),
                storage_region: "de".to_string(),
                pull_zone_id: "42".to_string(),
                remote_prefix: String::new(),
            }),
        }
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

    #[test]
    fn first_sync_uploads_every_file_and_publishes_owned_manifest() {
        let artifact = artifact(
            &[("index.html", b"hello"), ("app.css", b"css")],
            "sha256:new",
        );
        let prepared = prepare_sync_plan(&target(), 0, &artifact, None, None).unwrap();
        assert_eq!(prepared.plan.upload_files, 2);
        assert_eq!(prepared.plan.skipped_files, 0);
        assert_eq!(prepared.plan.delete_files, 0);
        let stored: RemoteDeployManifest =
            serde_json::from_slice(&prepared.next_manifest_bytes).unwrap();
        assert_eq!(stored.owner, REMOTE_MANIFEST_OWNER);
        assert_eq!(stored.artifact_id, "sha256:new");
    }

    #[test]
    fn mirror_requires_complete_inventory_and_deletes_unmanaged_files() {
        let artifact = artifact(&[("index.html", b"new")], "sha256:new");
        let mut mirror_target = target();
        mirror_target.cleanup_policy = DeployCleanupPolicy::MirrorDestination;

        let missing_inventory =
            prepare_sync_plan(&mirror_target, 0, &artifact, None, None).unwrap_err();
        assert!(missing_inventory.contains("inventarierea integrală"));

        let inventory = vec![
            RemoteInventoryFile {
                path: "index.html".to_string(),
                size_bytes: 3,
            },
            RemoteInventoryFile {
                path: "legacy/old.css".to_string(),
                size_bytes: 42,
            },
            RemoteInventoryFile {
                path: REMOTE_MANIFEST_FILE_NAME.to_string(),
                size_bytes: 100,
            },
        ];
        let prepared =
            prepare_sync_plan(&mirror_target, 0, &artifact, None, Some(&inventory)).unwrap();
        assert_eq!(prepared.plan.upload_files, 1);
        assert_eq!(prepared.plan.delete_files, 1);
        assert_eq!(prepared.plan.managed_delete_files, 0);
        assert_eq!(prepared.plan.unmanaged_delete_files, 1);
        assert!(prepared.plan.actions.iter().any(|action| {
            action.kind == DeployActionKind::Delete
                && action.path == "legacy/old.css"
                && action.delete_origin == Some(DeployDeleteOrigin::Unmanaged)
        }));
        assert!(!prepared
            .plan
            .actions
            .iter()
            .any(|action| action.path == REMOTE_MANIFEST_FILE_NAME));
    }

    #[test]
    fn mirror_distinguishes_managed_and_unmanaged_deletions() {
        let first = artifact(
            &[("index.html", b"old"), ("managed-old.css", b"old")],
            "sha256:first",
        );
        let first_plan = prepare_sync_plan(&target(), 0, &first, None, None).unwrap();
        let second = artifact(&[("index.html", b"new")], "sha256:second");
        let inventory = vec![
            RemoteInventoryFile {
                path: "index.html".to_string(),
                size_bytes: 3,
            },
            RemoteInventoryFile {
                path: "managed-old.css".to_string(),
                size_bytes: 3,
            },
            RemoteInventoryFile {
                path: "manual.txt".to_string(),
                size_bytes: 9,
            },
        ];
        let mut mirror_target = target();
        mirror_target.cleanup_policy = DeployCleanupPolicy::MirrorDestination;
        let prepared = prepare_sync_plan(
            &mirror_target,
            0,
            &second,
            Some(&first_plan.next_manifest_bytes),
            Some(&inventory),
        )
        .unwrap();
        assert_eq!(prepared.plan.delete_files, 2);
        assert_eq!(prepared.plan.managed_delete_files, 1);
        assert_eq!(prepared.plan.unmanaged_delete_files, 1);
    }

    #[test]
    fn next_sync_skips_equal_files_and_deletes_only_manifest_owned_stale_paths() {
        let first = artifact(
            &[("index.html", b"old"), ("owned-stale.txt", b"stale")],
            "sha256:first",
        );
        let first_plan = prepare_sync_plan(&target(), 0, &first, None, None).unwrap();
        let second = artifact(
            &[("index.html", b"old"), ("new.txt", b"new")],
            "sha256:second",
        );
        let second_plan = prepare_sync_plan(
            &target(),
            0,
            &second,
            Some(&first_plan.next_manifest_bytes),
            None,
        )
        .unwrap();
        assert_eq!(second_plan.plan.upload_files, 1);
        assert_eq!(second_plan.plan.skipped_files, 1);
        assert_eq!(second_plan.plan.delete_files, 1);
        assert!(second_plan.plan.actions.iter().any(|action| {
            action.kind == DeployActionKind::Delete && action.path == "owned-stale.txt"
        }));
        assert!(!second_plan
            .plan
            .actions
            .iter()
            .any(|action| action.path == "unknown-remote.txt"));
    }

    #[test]
    fn invalid_or_foreign_manifest_blocks_deletes_instead_of_becoming_empty_inventory() {
        let artifact = artifact(&[("index.html", b"new")], "sha256:new");
        assert!(prepare_sync_plan(&target(), 0, &artifact, Some(b"not-json"), None).is_err());

        let other_target = DeployTarget {
            id: "other".to_string(),
            ..target()
        };
        let manifest = prepare_sync_plan(&other_target, 0, &artifact, None, None)
            .unwrap()
            .next_manifest_bytes;
        let error = prepare_sync_plan(&target(), 0, &artifact, Some(&manifest), None).unwrap_err();
        assert!(error.contains("altei ținte"));
    }

    #[test]
    fn plan_token_changes_with_remote_ownership_state() {
        let artifact = artifact(&[("index.html", b"same")], "sha256:same");
        let first = prepare_sync_plan(&target(), 0, &artifact, None, None).unwrap();
        let second = prepare_sync_plan(
            &target(),
            0,
            &artifact,
            Some(&first.next_manifest_bytes),
            None,
        )
        .unwrap();
        assert_ne!(first.plan.plan_token, second.plan.plan_token);
    }

    #[test]
    fn plan_token_is_bound_to_settings_revision() {
        let artifact = artifact(&[("index.html", b"same")], "sha256:same");
        let first = prepare_sync_plan(&target(), 3, &artifact, None, None).unwrap();
        let second = prepare_sync_plan(&target(), 4, &artifact, None, None).unwrap();
        assert_ne!(first.plan.plan_token, second.plan.plan_token);
    }

    #[test]
    fn artifact_cannot_overwrite_the_owned_remote_manifest() {
        let artifact = artifact(
            &[(REMOTE_MANIFEST_FILE_NAME, b"user-controlled")],
            "sha256:reserved",
        );
        let error = prepare_sync_plan(&target(), 0, &artifact, None, None).unwrap_err();
        assert!(error.contains("rezervat"));
    }
}
