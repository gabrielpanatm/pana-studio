use serde::Serialize;

use crate::kernel::write_authority::WriteReceipt;

pub const MAX_SCRATCH_TEXT_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_SCRATCH_NAMESPACE_BYTES: usize = 64;
pub(crate) const MAX_SCRATCH_KEY_BYTES: usize = 96;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScratchEntrySnapshot {
    pub namespace: String,
    pub key: String,
    pub relative_path: String,
    pub public_label: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScratchTextSnapshot {
    pub entry: ScratchEntrySnapshot,
    pub contents: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScratchMutationReceipt {
    pub entry: ScratchEntrySnapshot,
    pub write: WriteReceipt,
}
