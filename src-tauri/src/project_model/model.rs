use std::{collections::HashSet, path::PathBuf};

use serde::Serialize;

use crate::localization::LocalizedDiagnostic;
use crate::source_graph::model::{SourceGraph, SourceRange};

#[derive(Clone)]
pub struct ProjectModel {
    pub project_root: PathBuf,
    pub zola_root: PathBuf,
    pub revision: String,
    pub files: Vec<ProjectModelFile>,
    /// Complete path namespace captured by the immutable workspace projection.
    /// Semantic validators must use this authority instead of consulting the
    /// live project disk.
    pub(crate) workspace_paths: HashSet<String>,
    pub source_graph: SourceGraph,
    pub diagnostics: Vec<ProjectModelDiagnostic>,
}

#[cfg(test)]
impl ProjectModel {
    pub fn snapshot(&self) -> ProjectModelSnapshot {
        ProjectModelSnapshot {
            project_root: self.project_root.to_string_lossy().to_string(),
            zola_root: self.zola_root.to_string_lossy().to_string(),
            revision: self.revision.clone(),
            files: self.files.iter().map(ProjectModelFile::summary).collect(),
            source_graph: self.source_graph.clone(),
            diagnostics: self.diagnostics.clone(),
        }
    }
}

#[derive(Clone)]
pub struct ProjectModelFile {
    pub relative_path: String,
    pub kind: ProjectModelFileKind,
    pub contents: String,
    pub size_bytes: usize,
    pub revision: String,
    pub(crate) source_hash: String,
    pub from_draft: bool,
}

#[cfg(test)]
impl ProjectModelFile {
    fn summary(&self) -> ProjectModelFileSummary {
        ProjectModelFileSummary {
            relative_path: self.relative_path.clone(),
            kind: self.kind.clone(),
            size_bytes: self.size_bytes,
            revision: self.revision.clone(),
            from_draft: self.from_draft,
        }
    }
}

#[cfg(test)]
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectModelSnapshot {
    pub project_root: String,
    pub zola_root: String,
    pub revision: String,
    pub files: Vec<ProjectModelFileSummary>,
    pub source_graph: SourceGraph,
    pub diagnostics: Vec<ProjectModelDiagnostic>,
}

#[cfg(test)]
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectModelFileSummary {
    pub relative_path: String,
    pub kind: ProjectModelFileKind,
    pub size_bytes: usize,
    pub revision: String,
    pub from_draft: bool,
}

#[derive(Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProjectModelFileKind {
    Config,
    Content,
    Template,
    Style,
    Script,
    Data,
    StaticText,
    OtherText,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectModelDiagnostic {
    pub severity: ProjectModelDiagnosticSeverity,
    pub diagnostic: LocalizedDiagnostic,
    pub file: Option<String>,
    pub range: Option<SourceRange>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectModelDiagnosticSeverity {
    Warning,
    Error,
}
