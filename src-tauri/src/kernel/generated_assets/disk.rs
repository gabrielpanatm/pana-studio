use std::{fs, io::ErrorKind, path::Path};

use crate::{kernel::file_buffer_store::hash_bytes, project::project_disk_metadata_version_token};

use super::model::GeneratedAssetDiskState;

#[derive(Clone, Debug)]
pub(crate) struct GeneratedAssetDiskInspection {
    pub state: GeneratedAssetDiskState,
    pub hash: Option<String>,
    pub diagnostic: Option<String>,
}

pub(crate) fn inspect_generated_asset_path(
    path: &Path,
    expected_bytes: &[u8],
) -> GeneratedAssetDiskInspection {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => GeneratedAssetDiskInspection {
            state: GeneratedAssetDiskState::Symlink,
            hash: None,
            diagnostic: Some(format!(
                "Asset-ul generat {} este symlink; nucleul nu îl urmărește și nu îl suprascrie.",
                path.display()
            )),
        },
        Ok(metadata) if metadata.is_dir() => GeneratedAssetDiskInspection {
            state: GeneratedAssetDiskState::Directory,
            hash: None,
            diagnostic: Some(format!(
                "Asset-ul generat {} este director; operația este blocată.",
                path.display()
            )),
        },
        Ok(metadata_before) => match fs::read(path) {
            Ok(bytes) => {
                let hash = hash_bytes(&bytes);
                let expected_hash = hash_bytes(expected_bytes);
                let version_before = project_disk_metadata_version_token(&metadata_before);
                let metadata_after = match fs::metadata(path) {
                    Ok(metadata) => metadata,
                    Err(error) => {
                        return GeneratedAssetDiskInspection {
                            state: GeneratedAssetDiskState::Unreadable,
                            hash: None,
                            diagnostic: Some(format!(
                                "Asset-ul generat {} nu mai poate fi verificat după hash preflight: {}",
                                path.display(), error
                            )),
                        };
                    }
                };
                let version_after = project_disk_metadata_version_token(&metadata_after);
                if version_before != version_after {
                    return GeneratedAssetDiskInspection {
                        state: GeneratedAssetDiskState::Unreadable,
                        hash: None,
                        diagnostic: Some(format!(
                            "Asset-ul generat {} s-a modificat în timpul hash preflight.",
                            path.display()
                        )),
                    };
                }
                GeneratedAssetDiskInspection {
                    state: if hash == expected_hash {
                        GeneratedAssetDiskState::Matching
                    } else {
                        GeneratedAssetDiskState::Different
                    },
                    hash: Some(hash),
                    diagnostic: None,
                }
            }
            Err(error) => GeneratedAssetDiskInspection {
                state: GeneratedAssetDiskState::Unreadable,
                hash: None,
                diagnostic: Some(format!(
                    "Asset-ul generat {} nu poate fi citit pentru hash preflight: {}",
                    path.display(),
                    error
                )),
            },
        },
        Err(error) if error.kind() == ErrorKind::NotFound => GeneratedAssetDiskInspection {
            state: GeneratedAssetDiskState::Missing,
            hash: None,
            diagnostic: None,
        },
        Err(error) => GeneratedAssetDiskInspection {
            state: GeneratedAssetDiskState::Unreadable,
            hash: None,
            diagnostic: Some(format!(
                "Asset-ul generat {} nu poate fi inspectat înainte de mutație: {}",
                path.display(),
                error
            )),
        },
    }
}
