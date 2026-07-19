use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use super::model::ProjectRootFingerprint;

pub fn fingerprint_project_root(root: &Path) -> Result<ProjectRootFingerprint, String> {
    let canonical = root
        .canonicalize()
        .map_err(|error| format!("Nu am putut calcula identitatea proiectului: {}", error))?;
    let metadata = fs::metadata(&canonical).map_err(|error| {
        format!(
            "Nu am putut citi metadata pentru proiectul deschis {}: {}",
            canonical.display(),
            error
        )
    })?;
    if !metadata.is_dir() {
        return Err(format!(
            "ProjectSession cere un folder, dar path-ul este fișier: {}",
            canonical.display()
        ));
    }

    Ok(ProjectRootFingerprint {
        canonical_path: canonical.to_string_lossy().to_string(),
        modified_ms: modified_ms(&metadata),
        size: metadata.len(),
        readonly: metadata.permissions().readonly(),
        unix_device: unix_device(&metadata),
        unix_inode: unix_inode(&metadata),
    })
}

fn modified_ms(metadata: &fs::Metadata) -> u128 {
    metadata
        .modified()
        .unwrap_or(SystemTime::UNIX_EPOCH)
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(unix)]
fn unix_device(metadata: &fs::Metadata) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    Some(metadata.dev().to_string())
}

#[cfg(not(unix))]
fn unix_device(_metadata: &fs::Metadata) -> Option<String> {
    None
}

#[cfg(unix)]
fn unix_inode(metadata: &fs::Metadata) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    Some(metadata.ino().to_string())
}

#[cfg(not(unix))]
fn unix_inode(_metadata: &fs::Metadata) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::fingerprint_project_root;

    #[test]
    fn fingerprint_project_root_records_canonical_directory_identity() {
        let root = unique_test_dir("project-session-fingerprint");
        fs::create_dir_all(&root).unwrap();

        let fingerprint = fingerprint_project_root(&root).unwrap();

        assert_eq!(
            fingerprint.canonical_path,
            root.canonicalize().unwrap().to_string_lossy().to_string()
        );
        assert!(fingerprint.modified_ms > 0);
        assert!(!fingerprint.readonly);
        #[cfg(unix)]
        {
            assert!(fingerprint.unix_device.is_some());
            assert!(fingerprint.unix_inode.is_some());
        }

        fs::remove_dir_all(root).unwrap();
    }

    fn unique_test_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pana-studio-{label}-{nanos}"))
    }
}
