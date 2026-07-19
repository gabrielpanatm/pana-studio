use std::path::Path;

use crate::kernel::{
    bounded_journal_reader::read_bounded_regular_file_snapshot, file_buffer_store::hash_bytes,
};

use super::model::MAX_PROJECT_TRANSITION_DECISION_RETENTION_HOT_JOURNAL_BYTES;

#[derive(Clone, Debug)]
pub(super) struct RetentionFileBaseline {
    pub version_token: String,
    pub content_hash: String,
    pub text: String,
}

pub(super) fn capture_retention_file_baseline(
    path: &Path,
    label: &str,
) -> Result<RetentionFileBaseline, String> {
    let snapshot = read_bounded_regular_file_snapshot(
        path,
        label,
        MAX_PROJECT_TRANSITION_DECISION_RETENTION_HOT_JOURNAL_BYTES,
    )?
    .ok_or_else(|| format!("{label} {} lipsește.", path.display()))?;
    let text = String::from_utf8(snapshot.bytes)
        .map_err(|error| format!("{label} {} nu este UTF-8 valid: {error}", path.display()))?;
    Ok(RetentionFileBaseline {
        version_token: snapshot.version_token,
        content_hash: hash_bytes(text.as_bytes()),
        text,
    })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        kernel::file_buffer_store::hash_text, project::project_disk_metadata_version_token,
    };

    use super::capture_retention_file_baseline;

    #[test]
    fn retention_file_baseline_binds_exact_text_hash_and_metadata_version() {
        let root = temp_dir("retention-stable-file");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("hot.json");
        let body = "{\"retentionId\":\"retention-1\"}";
        fs::write(&path, body).unwrap();

        let baseline = capture_retention_file_baseline(&path, "Retention test").unwrap();

        assert_eq!(baseline.text, body);
        assert_eq!(baseline.content_hash, hash_text(body));
        assert_eq!(
            baseline.version_token,
            project_disk_metadata_version_token(&fs::symlink_metadata(&path).unwrap())
        );
        fs::remove_dir_all(root).unwrap();
    }

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "pana-studio-{label}-{}-{nanos}",
            std::process::id()
        ))
    }
}
