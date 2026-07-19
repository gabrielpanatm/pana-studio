use serde::{Deserialize, Serialize};

use super::hash::hash_text;
use super::model::FileBufferFileSnapshot;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileBufferChangeSetInput {
    pub relative_path: String,
    #[serde(default)]
    pub base_revision: Option<u64>,
    #[serde(default)]
    pub base_hash: Option<String>,
    #[serde(default = "default_coordinate_space")]
    pub coordinate_space: FileBufferChangeCoordinateSpace,
    #[serde(default)]
    pub source: Option<String>,
    pub changes: Vec<FileBufferTextChange>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileBufferTextChange {
    pub from: usize,
    pub to: usize,
    pub insert: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FileBufferChangeCoordinateSpace {
    Utf16,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileBufferChangeSetResult {
    pub relative_path: String,
    pub source: Option<String>,
    pub previous_revision: u64,
    pub revision: u64,
    pub previous_hash: String,
    pub current_hash: String,
    pub change_count: usize,
    pub applied: bool,
    pub file: FileBufferFileSnapshot,
}

pub(crate) struct AppliedTextChangeSet {
    pub text: String,
    pub previous_hash: String,
    pub current_hash: String,
    pub applied: bool,
}

fn default_coordinate_space() -> FileBufferChangeCoordinateSpace {
    FileBufferChangeCoordinateSpace::Utf16
}

pub(crate) fn apply_text_changes(
    current_text: &str,
    changes: &[FileBufferTextChange],
    coordinate_space: FileBufferChangeCoordinateSpace,
) -> Result<AppliedTextChangeSet, String> {
    let previous_hash = hash_text(current_text);
    if changes.is_empty() {
        return Ok(AppliedTextChangeSet {
            text: current_text.to_string(),
            previous_hash: previous_hash.clone(),
            current_hash: previous_hash,
            applied: false,
        });
    }

    let mut ranges = changes
        .iter()
        .map(|change| byte_range_for_change(current_text, change, coordinate_space))
        .collect::<Result<Vec<_>, _>>()?;
    ranges.sort_by(|left, right| {
        left.from
            .cmp(&right.from)
            .then_with(|| left.to.cmp(&right.to))
    });

    let mut previous_to = 0usize;
    for range in &ranges {
        if range.from < previous_to {
            return Err(
                "FileBufferStore a refuzat change-set-ul: range-urile text se suprapun."
                    .to_string(),
            );
        }
        previous_to = range.to;
    }

    let mut next_text = current_text.to_string();
    for range in ranges.iter().rev() {
        next_text.replace_range(range.from..range.to, &range.insert);
    }
    let current_hash = hash_text(&next_text);
    let applied = current_hash != previous_hash;

    Ok(AppliedTextChangeSet {
        text: next_text,
        previous_hash,
        current_hash,
        applied,
    })
}

struct ByteTextChange {
    from: usize,
    to: usize,
    insert: String,
}

fn byte_range_for_change(
    text: &str,
    change: &FileBufferTextChange,
    coordinate_space: FileBufferChangeCoordinateSpace,
) -> Result<ByteTextChange, String> {
    if change.from > change.to {
        return Err(format!(
            "FileBufferStore a refuzat change-set-ul: range invalid {}..{}.",
            change.from, change.to
        ));
    }

    match coordinate_space {
        FileBufferChangeCoordinateSpace::Utf16 => {
            let from = utf16_offset_to_byte_index(text, change.from)?;
            let to = utf16_offset_to_byte_index(text, change.to)?;
            Ok(ByteTextChange {
                from,
                to,
                insert: change.insert.clone(),
            })
        }
    }
}

fn utf16_offset_to_byte_index(text: &str, offset: usize) -> Result<usize, String> {
    let mut utf16_units = 0usize;
    for (byte_index, character) in text.char_indices() {
        if utf16_units == offset {
            return Ok(byte_index);
        }
        let next_units = utf16_units + character.len_utf16();
        if offset < next_units {
            return Err(format!(
                "FileBufferStore a refuzat change-set-ul: offsetul UTF-16 {offset} cade în interiorul unui caracter."
            ));
        }
        utf16_units = next_units;
    }

    if utf16_units == offset {
        return Ok(text.len());
    }

    Err(format!(
        "FileBufferStore a refuzat change-set-ul: offsetul UTF-16 {offset} depășește documentul."
    ))
}
