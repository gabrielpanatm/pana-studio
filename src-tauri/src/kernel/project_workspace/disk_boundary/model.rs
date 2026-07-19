use serde::Serialize;

use crate::kernel::{
    file_buffer_store::{FileBufferBaseline, FileBufferSaveStamp},
    write_authority::WriteReceipt,
};

pub const SAVE_ENGINE_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveTextFileStatus {
    Created,
    Saved,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveTextFileResult {
    pub schema_version: u32,
    pub relative_path: String,
    pub status: SaveTextFileStatus,
    pub baseline_before: Option<FileBufferBaseline>,
    pub disk_before: Option<FileBufferBaseline>,
    pub baseline_after: Option<FileBufferBaseline>,
    pub file_buffer_before: Option<FileBufferSaveStamp>,
    pub file_buffer_after: FileBufferSaveStamp,
    pub retained_newer_draft: bool,
    pub bytes_written: u64,
    pub receipt: WriteReceipt,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveTextFileResult {
    pub schema_version: u32,
    pub relative_path: String,
    pub baseline_before: FileBufferBaseline,
    pub disk_before: FileBufferBaseline,
    pub expected_hash: String,
    pub bytes_written: u64,
    pub receipt: WriteReceipt,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveConflictDiagnostic {
    pub relative_path: String,
    pub reason: SaveConflictReason,
    pub expected: Option<FileBufferBaseline>,
    pub actual: Option<FileBufferBaseline>,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveConflictReason {
    MissingTrackedBaseline,
    MissingDiskFile,
    DiskChanged,
    ReadonlyTarget,
}

impl SaveConflictDiagnostic {
    pub fn missing_tracked_baseline(
        relative_path: impl Into<String>,
        actual: Option<FileBufferBaseline>,
    ) -> Self {
        let relative_path = relative_path.into();
        Self {
            relative_path: relative_path.clone(),
            reason: SaveConflictReason::MissingTrackedBaseline,
            expected: None,
            actual,
            message: format!(
                "Save blocat pentru {relative_path}: fișier existent fără baseline în FileBufferStore."
            ),
        }
    }

    pub fn missing_disk_file(
        relative_path: impl Into<String>,
        expected: FileBufferBaseline,
    ) -> Self {
        let relative_path = relative_path.into();
        Self {
            relative_path: relative_path.clone(),
            reason: SaveConflictReason::MissingDiskFile,
            expected: Some(expected),
            actual: None,
            message: format!(
                "Save blocat pentru {relative_path}: fișierul de pe disk lipsește față de baseline."
            ),
        }
    }

    pub fn disk_changed(
        relative_path: impl Into<String>,
        expected: FileBufferBaseline,
        actual: FileBufferBaseline,
    ) -> Self {
        let relative_path = relative_path.into();
        Self {
            relative_path: relative_path.clone(),
            reason: SaveConflictReason::DiskChanged,
            expected: Some(expected.clone()),
            actual: Some(actual.clone()),
            message: format!(
                "Save blocat pentru {relative_path}: disk-ul s-a schimbat de la baseline-ul sesiunii (baseline hash {}, disk hash {}).",
                expected.hash, actual.hash
            ),
        }
    }

    pub fn readonly_target(relative_path: impl Into<String>, actual: FileBufferBaseline) -> Self {
        let relative_path = relative_path.into();
        Self {
            relative_path: relative_path.clone(),
            reason: SaveConflictReason::ReadonlyTarget,
            expected: None,
            actual: Some(actual),
            message: format!(
                "Save blocat pentru {relative_path}: fișierul de pe disk este readonly."
            ),
        }
    }

    pub fn to_error(&self) -> String {
        self.message.clone()
    }
}
