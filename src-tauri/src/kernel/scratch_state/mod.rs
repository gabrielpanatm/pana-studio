mod model;
mod store;

pub use model::{
    ScratchEntrySnapshot, ScratchMutationReceipt, ScratchTextSnapshot, MAX_SCRATCH_TEXT_BYTES,
};
pub use store::{read_scratch_text, remove_scratch_entry, write_scratch_text};
