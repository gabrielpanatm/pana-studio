use std::path::PathBuf;

use serde::Serialize;

use crate::source_graph::model::{
    SourceCapabilities, SourceGraph, SourceNodeKind, SourceOrigin, SourceRange,
};

#[derive(Clone)]
pub struct ProjectModel {
    pub project_root: PathBuf,
    pub zola_root: PathBuf,
    pub revision: String,
    pub files: Vec<ProjectModelFile>,
    pub source_graph: SourceGraph,
    pub tera_graph: TeraGraph,
    pub diagnostics: Vec<ProjectModelDiagnostic>,
}

impl ProjectModel {
    pub fn snapshot(&self) -> ProjectModelSnapshot {
        ProjectModelSnapshot {
            project_root: self.project_root.to_string_lossy().to_string(),
            zola_root: self.zola_root.to_string_lossy().to_string(),
            revision: self.revision.clone(),
            files: self.files.iter().map(ProjectModelFile::summary).collect(),
            source_graph: self.source_graph.clone(),
            tera_graph: self.tera_graph.clone(),
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
    pub from_draft: bool,
}

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

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectModelSnapshot {
    pub project_root: String,
    pub zola_root: String,
    pub revision: String,
    pub files: Vec<ProjectModelFileSummary>,
    pub source_graph: SourceGraph,
    pub tera_graph: TeraGraph,
    pub diagnostics: Vec<ProjectModelDiagnostic>,
}

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
    pub message: String,
    pub file: Option<String>,
    pub range: Option<SourceRange>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectModelDiagnosticSeverity {
    Warning,
    Error,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeraGraph {
    pub templates: Vec<TeraGraphTemplate>,
    pub nodes: Vec<TeraGraphNode>,
    pub relations: Vec<TeraGraphRelation>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeraGraphTemplate {
    pub file: String,
    pub name: String,
    pub origin: SourceOrigin,
    pub theme_name: Option<String>,
    pub is_partial: bool,
    pub source_graph_template_id: String,
    pub source_graph_node_id: String,
    pub root_node_id: String,
    pub extends: Option<String>,
    pub includes: Vec<String>,
    pub imports: Vec<String>,
    pub blocks: Vec<String>,
    pub macros: Vec<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeraGraphNode {
    pub id: String,
    pub kind: SourceNodeKind,
    pub file: String,
    pub label: String,
    pub target: Option<String>,
    pub range: Option<SourceRange>,
    pub parent: Option<String>,
    pub children: Vec<String>,
    pub capabilities: SourceCapabilities,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeraGraphRelation {
    pub id: String,
    pub from: String,
    pub to: String,
    pub kind: TeraGraphRelationKind,
    pub label: String,
}

#[derive(Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TeraGraphRelationKind {
    Contains,
    Extends,
    Includes,
    Imports,
    DefinesBlock,
    DefinesMacro,
}
