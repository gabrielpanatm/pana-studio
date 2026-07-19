use crate::kernel::project_workspace::WorkspaceTextMutationInput;

pub const SOURCE_GRAPH_REWRITE_SCHEMA_VERSION: u32 = 1;
pub const SOURCE_GRAPH_REWRITE_WORKSPACE_TARGET: &str = "project/source-graph/references";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceGraphRewriteOperation {
    Move,
    Rename,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceGraphRewriteStatus {
    Planned,
    NoOp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceGraphRewriteSeverity {
    Info,
    Warning,
    Blocked,
}

#[derive(Clone, Debug)]
pub struct SourceGraphReferenceRewritePlan {
    pub schema_version: u32,
    pub operation: SourceGraphRewriteOperation,
    pub status: SourceGraphRewriteStatus,
    pub source_relative_path: String,
    pub destination_relative_path: String,
    pub rewritten_references: Vec<SourceGraphReferenceRewrite>,
    pub touched_files: Vec<String>,
    pub diagnostics: Vec<SourceGraphRewriteDiagnostic>,
    pub workspace_mutation: Option<WorkspaceTextMutationInput>,
}

#[derive(Clone, Debug)]
pub struct SourceGraphReferenceRewrite {
    pub relative_path: String,
    pub target_relative_path: String,
    pub relation_kind: String,
    pub old_reference: String,
    pub new_reference: String,
    pub range_start: usize,
    pub range_end: usize,
}

#[derive(Clone, Debug)]
pub struct SourceGraphRewriteDiagnostic {
    pub severity: SourceGraphRewriteSeverity,
    pub code: String,
    pub relative_path: Option<String>,
    pub message: String,
}

impl SourceGraphRewriteDiagnostic {
    pub fn info(
        code: impl Into<String>,
        relative_path: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: SourceGraphRewriteSeverity::Info,
            code: code.into(),
            relative_path,
            message: message.into(),
        }
    }

    pub fn warning(
        code: impl Into<String>,
        relative_path: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: SourceGraphRewriteSeverity::Warning,
            code: code.into(),
            relative_path,
            message: message.into(),
        }
    }

    pub fn blocked(
        code: impl Into<String>,
        relative_path: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: SourceGraphRewriteSeverity::Blocked,
            code: code.into(),
            relative_path,
            message: message.into(),
        }
    }
}
