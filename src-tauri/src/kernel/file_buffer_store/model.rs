use std::{collections::BTreeMap, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::localization::LocalizedDiagnostic;

pub const FILE_BUFFER_STORE_SCHEMA_VERSION: u32 = 4;

#[derive(Clone, Debug)]
pub struct FileBufferStore {
    pub schema_version: u32,
    pub session_id: String,
    pub runtime_session_id: String,
    pub project_root: String,
    pub loaded_at_ms: u128,
    pub files: Arc<BTreeMap<String, Arc<FileBufferEntry>>>,
    pub diagnostics: Vec<FileBufferDiagnostic>,
    pub limits: FileBufferStoreLimits,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileBufferEntry {
    pub relative_path: String,
    pub absolute_path: String,
    pub language: TextBufferLanguage,
    pub role: TextBufferRole,
    pub baseline: FileBufferBaseline,
    pub baseline_text: Arc<str>,
    pub draft: Option<FileBufferDraft>,
    pub revision: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileBufferStoreSnapshot {
    pub schema_version: u32,
    pub session_id: String,
    pub runtime_session_id: String,
    pub project_root: String,
    pub loaded_at_ms: u128,
    pub file_count: usize,
    pub loaded_file_count: usize,
    pub skipped_file_count: usize,
    pub dirty_file_count: usize,
    pub total_loaded_bytes: u64,
    pub limits: FileBufferStoreLimits,
    pub files: Vec<FileBufferFileSnapshot>,
    pub diagnostics: Vec<FileBufferDiagnostic>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileBufferFileSnapshot {
    pub relative_path: String,
    pub absolute_path: String,
    pub language: TextBufferLanguage,
    pub role: TextBufferRole,
    pub baseline: FileBufferBaseline,
    pub has_draft: bool,
    pub dirty: bool,
    pub current_hash: String,
    pub current_bytes: u64,
    pub revision: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileBufferTextSnapshot {
    pub relative_path: String,
    pub text: String,
    pub dirty: bool,
    pub hash: String,
    pub bytes: u64,
    pub revision: u64,
}

/// Versioned identity of the exact FileBuffer state observed by a Save.
///
/// Revision alone is not sufficient evidence: a caller must also bind the
/// bytes/hash and the derived dirty state so a stale Save All item can be
/// skipped before it creates a transaction or touches disk.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileBufferSaveStamp {
    pub revision: u64,
    pub hash: String,
    pub bytes: u64,
    pub dirty: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileBufferSaveSnapshot {
    pub relative_path: String,
    pub contents: String,
    pub stamp: FileBufferSaveStamp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileBufferSaveProjection {
    pub before: Option<FileBufferSaveStamp>,
    pub after: FileBufferSaveStamp,
    pub retained_newer_draft: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileBufferMutationExpectation {
    pub expected_revision: u64,
    pub expected_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileBufferBaseline {
    pub hash: String,
    pub modified_ms: u128,
    pub size: u64,
    pub readonly: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileBufferDraft {
    pub text: String,
    pub hash: String,
    pub bytes: u64,
    pub updated_at_ms: u128,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileBufferStoreLimits {
    pub max_files: usize,
    pub max_file_bytes: u64,
    pub max_total_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileBufferDiagnostic {
    pub severity: FileBufferDiagnosticSeverity,
    pub code: String,
    pub relative_path: Option<String>,
    pub message_diagnostic: LocalizedDiagnostic,
    #[serde(skip_serializing)]
    pub message: String,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileBufferDiagnosticSeverity {
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextBufferLanguage {
    Html,
    Markdown,
    Css,
    Scss,
    JavaScript,
    Toml,
    Json,
    Yaml,
    Plain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextBufferRole {
    Page,
    Template,
    Style,
    Script,
    Config,
    Data,
    Other,
}

impl FileBufferDiagnostic {
    pub fn warning(
        code: impl Into<String>,
        relative_path: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        let code = code.into();
        Self {
            severity: FileBufferDiagnosticSeverity::Warning,
            message_diagnostic: file_buffer_message_diagnostic(&code, relative_path.as_deref()),
            code,
            relative_path,
            message: message.into(),
        }
    }

    pub fn error(
        code: impl Into<String>,
        relative_path: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        let code = code.into();
        Self {
            severity: FileBufferDiagnosticSeverity::Error,
            message_diagnostic: file_buffer_message_diagnostic(&code, relative_path.as_deref()),
            code,
            relative_path,
            message: message.into(),
        }
    }
}

fn file_buffer_message_diagnostic(code: &str, relative_path: Option<&str>) -> LocalizedDiagnostic {
    let message_code = match code {
        "not_text_file" => "file-buffer-diagnostic-not-text",
        "open_failed" => "file-buffer-diagnostic-open-failed",
        "not_file" => "file-buffer-diagnostic-not-file",
        "file_too_large" => "file-buffer-diagnostic-file-too-large",
        "invalid_relative_path" => "file-buffer-diagnostic-invalid-path",
        "unsafe_project_path" => "file-buffer-diagnostic-unsafe-path",
        "unstable_during_read" => "file-buffer-diagnostic-unstable",
        "read_text_failed" => "file-buffer-diagnostic-read-failed",
        "max_files_reached" => "file-buffer-diagnostic-max-files",
        "max_total_bytes_reached" => "file-buffer-diagnostic-max-total-bytes",
        "saved_file_too_large" => "file-buffer-diagnostic-saved-file-too-large",
        _ => "file-buffer-diagnostic-generic",
    };
    let mut diagnostic = LocalizedDiagnostic::new(message_code);
    if let Some(path) = relative_path {
        diagnostic = diagnostic.with_argument("path", path);
    }
    diagnostic
}
