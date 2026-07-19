use std::path::Path;

use crate::kernel::file_buffer_store::hash_bytes;

use super::{
    disk::inspect_generated_asset_path,
    model::{
        GeneratedAssetAction, GeneratedAssetDiskState, GeneratedAssetId, GeneratedAssetPlan,
        GeneratedAssetPlanStatus, GENERATED_ASSET_SCHEMA_VERSION,
    },
    registry::generated_asset_definition,
};

/// Read-only planner used by ProjectWorkspace Save while materializing its
/// complete transaction. Generated assets have no independent write executor.
pub fn plan_generated_asset_intent(
    zola_root: &Path,
    asset_id: GeneratedAssetId,
    action: GeneratedAssetAction,
) -> GeneratedAssetPlan {
    let definition = generated_asset_definition(asset_id);
    let absolute_path = zola_root.join(definition.zola_relative_path);
    let inspection = inspect_generated_asset_path(&absolute_path, definition.bytes);
    let expected_hash = hash_bytes(definition.bytes);
    let mut diagnostics = Vec::new();
    if let Some(diagnostic) = inspection.diagnostic.clone() {
        diagnostics.push(diagnostic);
    }

    let status = match (action, inspection.state) {
        (GeneratedAssetAction::EnsurePresent, GeneratedAssetDiskState::Missing) => {
            GeneratedAssetPlanStatus::Ready
        }
        (GeneratedAssetAction::EnsurePresent, GeneratedAssetDiskState::Matching) => {
            GeneratedAssetPlanStatus::Noop
        }
        (GeneratedAssetAction::RemoveIfMatching, GeneratedAssetDiskState::Matching) => {
            GeneratedAssetPlanStatus::Ready
        }
        (GeneratedAssetAction::RemoveIfMatching, GeneratedAssetDiskState::Missing) => {
            GeneratedAssetPlanStatus::Noop
        }
        (_, GeneratedAssetDiskState::Different) => {
            diagnostics.push(format!(
                "{} există la {}, dar hash-ul diferă de registry. Nucleul nu suprascrie și nu șterge fișiere posibil editate de utilizator.",
                definition.label, definition.project_relative_path
            ));
            GeneratedAssetPlanStatus::Blocked
        }
        (
            _,
            GeneratedAssetDiskState::Directory
            | GeneratedAssetDiskState::Symlink
            | GeneratedAssetDiskState::Unreadable,
        ) => {
            diagnostics.push(format!(
                "{} nu poate fi materializat în siguranță la {} din starea {:?}.",
                definition.label, definition.project_relative_path, inspection.state
            ));
            GeneratedAssetPlanStatus::Blocked
        }
    };

    GeneratedAssetPlan {
        schema_version: GENERATED_ASSET_SCHEMA_VERSION,
        asset_id,
        asset_label: definition.label.to_string(),
        action,
        zola_relative_path: definition.zola_relative_path.to_string(),
        project_relative_path: definition.project_relative_path.to_string(),
        absolute_path: absolute_path.to_string_lossy().to_string(),
        expected_hash,
        expected_bytes: definition.bytes.len() as u64,
        disk_state: inspection.state,
        disk_hash: inspection.hash,
        status,
        diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::plan_generated_asset_intent;
    use crate::kernel::generated_assets::{
        GeneratedAssetAction, GeneratedAssetId, GeneratedAssetPlanStatus,
    };

    #[test]
    fn planner_ensures_missing_asset() {
        let root = unique_test_dir("ensure");
        fs::create_dir_all(&root).unwrap();

        let plan = plan_generated_asset_intent(
            &root,
            GeneratedAssetId::AnimeJsRuntime,
            GeneratedAssetAction::EnsurePresent,
        );

        assert_eq!(plan.status, GeneratedAssetPlanStatus::Ready);
        assert!(plan.diagnostics.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn planner_blocks_user_divergent_asset() {
        let root = unique_test_dir("different");
        let static_dir = root.join("static/js");
        fs::create_dir_all(&static_dir).unwrap();
        fs::write(static_dir.join("anime.min.js"), b"user-custom").unwrap();

        let plan = plan_generated_asset_intent(
            &root,
            GeneratedAssetId::AnimeJsRuntime,
            GeneratedAssetAction::EnsurePresent,
        );

        assert_eq!(plan.status, GeneratedAssetPlanStatus::Blocked);
        assert!(plan
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("hash-ul diferă")));
        fs::remove_dir_all(root).unwrap();
    }

    fn unique_test_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("panastudio-generated-asset-plan-{name}-{unique}"))
    }
}
