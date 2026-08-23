#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProjectCapacityPolicy {
    /// Maximum regular files represented by one authoritative disk manifest.
    pub max_tracked_files: usize,
    /// Maximum files plus directories inspected before Startup fails closed.
    pub max_disk_inventory_entries: usize,
    /// Maximum logical files plus directories exposed by bounded project scans.
    pub max_projected_entries: usize,
    /// Maximum text documents resident in one FileBufferStore.
    pub max_resident_text_documents: usize,
    /// Maximum bytes resident for one text document.
    pub max_text_document_bytes: u64,
    /// Maximum aggregate text bytes resident in one FileBufferStore.
    pub max_resident_text_bytes: u64,
}

/// The single Rust authority for project-capacity boundaries.
///
/// File inventory and resident text limits are deliberately separate even
/// where their current numeric values match. A project can contain binary
/// resources and directories that must remain visible without consuming a
/// resident text-document slot.
pub(crate) const PROJECT_CAPACITY: ProjectCapacityPolicy = ProjectCapacityPolicy {
    max_tracked_files: 1_000,
    max_disk_inventory_entries: 2_000,
    max_projected_entries: 2_000,
    max_resident_text_documents: 1_000,
    max_text_document_bytes: 2 * 1024 * 1024,
    max_resident_text_bytes: 24 * 1024 * 1024,
};

pub(crate) fn require_projected_entry_capacity(entry_count: usize) -> Result<(), String> {
    if entry_count > PROJECT_CAPACITY.max_projected_entries {
        return Err(format!(
            "Proiecția proiectului conține {entry_count} intrări, peste limita explicită de {}.",
            PROJECT_CAPACITY.max_projected_entries
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{require_projected_entry_capacity, PROJECT_CAPACITY};

    #[test]
    fn file_inventory_and_resident_text_limits_are_distinct_contract_fields() {
        assert_eq!(PROJECT_CAPACITY.max_tracked_files, 1_000);
        assert_eq!(PROJECT_CAPACITY.max_resident_text_documents, 1_000);
        assert_eq!(PROJECT_CAPACITY.max_disk_inventory_entries, 2_000);
        assert_eq!(PROJECT_CAPACITY.max_projected_entries, 2_000);
        assert_eq!(PROJECT_CAPACITY.max_resident_text_bytes, 24 * 1024 * 1024);
    }

    #[test]
    fn projected_entry_capacity_accepts_the_boundary_and_rejects_only_overflow() {
        assert!(require_projected_entry_capacity(1_999).is_ok());
        assert!(require_projected_entry_capacity(2_000).is_ok());
        assert!(require_projected_entry_capacity(2_001).is_err());
    }
}
