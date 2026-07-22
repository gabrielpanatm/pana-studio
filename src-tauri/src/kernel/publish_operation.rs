use serde::Serialize;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PublishOperationKind {
    Build,
    Deploy,
}

#[derive(Clone)]
pub struct PublishOperationControl {
    pub operation_id: String,
    pub kind: PublishOperationKind,
    pub project_root: String,
    pub runtime_session_id: String,
    pub cancellation_token: CancellationToken,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishOperationCancelReceipt {
    pub schema_version: u32,
    pub operation_id: String,
    pub kind: PublishOperationKind,
    pub cancellation_requested: bool,
}
