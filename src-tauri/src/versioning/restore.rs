use std::collections::{BTreeMap, BTreeSet};

use crate::kernel::{
    file_buffer_store::language_for_relative_path,
    project_workspace::{
        ProjectWorkspace, WorkspaceBinaryRestoreChange, WorkspaceResourceDelete,
        WorkspaceResourceMutation,
    },
};

use super::{repository::attributes_define_external_driver, VersionTree};

#[derive(Clone, Debug)]
pub(crate) struct VersionRestoreExpectedFile {
    pub project_relative_path: String,
    pub expected_bytes: Option<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub(crate) struct VersionRestorePlan {
    pub text_changes: Vec<WorkspaceResourceMutation>,
    pub text_deletes: Vec<WorkspaceResourceDelete>,
    pub binary_changes: Vec<WorkspaceBinaryRestoreChange>,
    pub expected_files: Vec<VersionRestoreExpectedFile>,
    pub changed_paths: Vec<String>,
}

impl VersionRestorePlan {
    pub(crate) fn is_empty(&self) -> bool {
        self.changed_paths.is_empty()
    }
}

pub(crate) fn build_version_restore_plan(
    workspace: &ProjectWorkspace,
    current: &VersionTree,
    target: &VersionTree,
) -> Result<VersionRestorePlan, String> {
    reject_external_driver_attributes(target)?;
    let current_files = current
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect::<BTreeMap<_, _>>();
    let target_files = target
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect::<BTreeMap<_, _>>();
    let paths = current_files
        .keys()
        .chain(target_files.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    let mut text_changes = Vec::new();
    let mut text_deletes = Vec::new();
    let mut binary_changes = Vec::new();
    let mut expected_files = Vec::new();
    let mut changed_paths = Vec::new();

    for path in paths {
        let before = current_files.get(path).copied();
        let after = target_files.get(path).copied();
        if before.map(|file| (&file.oid, file.executable))
            == after.map(|file| (&file.oid, file.executable))
        {
            continue;
        }
        if before.is_some_and(|file| file.executable) || after.is_some_and(|file| file.executable) {
            return Err(format!(
                "Restaurarea a fost blocată pentru {path}: schimbările de fișiere executabile nu pot fi reproduse exact de ProjectWorkspace."
            ));
        }
        let project_relative_path = format!("sursa/{path}");
        let workspace_tracks_text = workspace
            .documents
            .files
            .contains_key(&project_relative_path);
        let safe_new_text = before.is_none()
            && language_for_relative_path(&project_relative_path).is_some()
            && after
                .and_then(|file| std::str::from_utf8(&file.bytes).ok())
                .is_some();
        let use_text_boundary = workspace_tracks_text || safe_new_text;

        if use_text_boundary {
            match after {
                Some(file) => {
                    let contents = String::from_utf8(file.bytes.clone()).map_err(|_| {
                        format!(
                            "Restaurarea a fost blocată: {path} este urmărit ca text, dar versiunea țintă nu este UTF-8."
                        )
                    })?;
                    text_changes.push(WorkspaceResourceMutation {
                        relative_path: project_relative_path.clone(),
                        contents,
                        create_only: before.is_none(),
                    });
                }
                None => text_deletes.push(WorkspaceResourceDelete {
                    relative_path: project_relative_path.clone(),
                }),
            }
        } else {
            binary_changes.push(WorkspaceBinaryRestoreChange {
                relative_path: project_relative_path.clone(),
                before: before.map(|file| file.bytes.clone()),
                after: after.map(|file| file.bytes.clone()),
            });
        }
        expected_files.push(VersionRestoreExpectedFile {
            project_relative_path,
            expected_bytes: after.map(|file| file.bytes.clone()),
        });
        changed_paths.push(path.to_string());
    }

    Ok(VersionRestorePlan {
        text_changes,
        text_deletes,
        binary_changes,
        expected_files,
        changed_paths,
    })
}

pub(crate) fn reject_external_driver_attributes(tree: &VersionTree) -> Result<(), String> {
    for file in &tree.files {
        if !file.path.ends_with(".gitattributes") {
            continue;
        }
        let source = std::str::from_utf8(&file.bytes).map_err(|_| {
            format!(
                "Restaurarea a refuzat {}: fișierul de atribute Git nu este UTF-8.",
                file.path
            )
        })?;
        if attributes_define_external_driver(source) {
            return Err(format!(
                "Operația a refuzat {}: versiunea definește atribute Git filter/merge, iar Pană Studio nu execută drivere clean/smudge/merge externe.",
                file.path
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_target_git_attributes_with_filters() {
        let tree = VersionTree {
            commit_oid: "a".repeat(40),
            tree_oid: "b".repeat(40),
            files: vec![super::super::VersionTreeFile {
                path: ".gitattributes".to_string(),
                oid: "c".repeat(40),
                bytes: b"*.png filter=external\n".to_vec(),
                executable: false,
            }],
            total_bytes: 22,
        };
        assert!(reject_external_driver_attributes(&tree).is_err());
    }
}
