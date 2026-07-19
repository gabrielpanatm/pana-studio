mod fingerprint;
mod lifecycle;
mod manifest;
mod model;

pub use fingerprint::fingerprint_project_root;
pub use lifecycle::{
    open_project_session, persist_project_session_open, prepare_project_session,
    record_project_session_opened,
};
pub use model::{ProjectRootFingerprint, ProjectSessionScanSummary, ProjectSessionSnapshot};
