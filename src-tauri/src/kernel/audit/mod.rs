mod builder;
mod model;

pub use builder::build_project_audit;
pub use model::{
    AuditCategory, AuditDiagnostic, AuditSeverity, AuditSummary, ProjectAuditSnapshot,
    PROJECT_AUDIT_SCHEMA_VERSION,
};
