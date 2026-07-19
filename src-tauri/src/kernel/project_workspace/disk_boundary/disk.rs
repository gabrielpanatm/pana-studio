use std::{
    fs::{self, File},
    io::Read,
    path::Path,
    time::UNIX_EPOCH,
};

use crate::{
    kernel::file_buffer_store::{hash_text, FileBufferBaseline, FileBufferStoreLimits},
    project::project_disk_metadata_version_token,
};

#[derive(Clone, Debug)]
pub struct DiskTextBaseline {
    pub baseline: FileBufferBaseline,
    pub version_token: String,
}

pub fn read_disk_text_baseline(
    path: &Path,
    limits: &FileBufferStoreLimits,
) -> Result<Option<DiskTextBaseline>, String> {
    if !path.exists() {
        return Ok(None);
    }

    let mut file = File::open(path).map_err(|error| {
        format!(
            "Nu am putut deschide fișierul pentru verificare Save {}: {}",
            path.display(),
            error
        )
    })?;
    let metadata_before = file.metadata().map_err(|error| {
        format!(
            "Nu am putut citi metadata pentru verificare Save {}: {}",
            path.display(),
            error
        )
    })?;

    if !metadata_before.is_file() {
        return Err(format!(
            "Save blocat: target-ul {} nu este fișier text.",
            path.display()
        ));
    }

    if metadata_before.len() > limits.max_file_bytes {
        return Err(format!(
            "Save blocat: {} are {} bytes, peste limita FileBufferStore de {} bytes.",
            path.display(),
            metadata_before.len(),
            limits.max_file_bytes
        ));
    }

    let mut text = String::new();
    file.read_to_string(&mut text).map_err(|error| {
        format!(
            "Save blocat: {} nu poate fi citit ca UTF-8 pentru verificare conflict: {}",
            path.display(),
            error
        )
    })?;

    let metadata_after = file.metadata().map_err(|error| {
        format!(
            "Save blocat: metadata pentru {} nu mai poate fi citită după read: {}",
            path.display(),
            error
        )
    })?;
    let version_before = project_disk_metadata_version_token(&metadata_before);
    let version_after = project_disk_metadata_version_token(&metadata_after);
    if version_before != version_after {
        return Err(format!(
            "Save blocat: {} s-a modificat în timpul citirii baseline-ului disk.",
            path.display()
        ));
    }
    Ok(Some(DiskTextBaseline {
        baseline: baseline_from_metadata(&metadata_after, &text),
        version_token: version_after,
    }))
}

fn baseline_from_metadata(metadata: &fs::Metadata, text: &str) -> FileBufferBaseline {
    FileBufferBaseline {
        hash: hash_text(text),
        modified_ms: metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis())
            .unwrap_or(0),
        size: metadata.len(),
        readonly: metadata.permissions().readonly(),
    }
}
