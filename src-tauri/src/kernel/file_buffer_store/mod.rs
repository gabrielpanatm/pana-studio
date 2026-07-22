mod bootstrap;
mod changeset;
mod classify;
mod external_reconcile;
mod hash;
mod model;
mod reader;
mod session_binding;
mod store;

pub use bootstrap::bootstrap_file_buffer_store;
pub use changeset::{
    FileBufferChangeCoordinateSpace, FileBufferChangeSetInput, FileBufferChangeSetResult,
    FileBufferTextChange,
};
pub(crate) use classify::language_for_relative_path;
pub(crate) use external_reconcile::{
    commit_clean_external_reconcile, plan_clean_external_reconcile,
    read_clean_external_reconcile_plan, CleanExternalReconcilePlan,
    CleanExternalReconcilePlanResult, CleanExternalReconcileReadResult,
};
pub use external_reconcile::{
    KernelExternalDiskProjectionHints, KernelExternalDiskReconcileDiagnostic,
    KernelExternalDiskReconcileInput, KernelExternalDiskReconcileItemOutcome,
    KernelExternalDiskReconcileItemReceipt, KernelExternalDiskReconcileReceipt,
    KernelExternalDiskReconcileStatus, KERNEL_EXTERNAL_DISK_RECONCILE_SCHEMA_VERSION,
};
pub(crate) use hash::{hash_bytes, hash_text};
pub(crate) use model::FileBufferDraft;
pub use model::{
    FileBufferBaseline, FileBufferDiagnostic, FileBufferDiagnosticSeverity, FileBufferEntry,
    FileBufferFileSnapshot, FileBufferMutationExpectation, FileBufferSaveProjection,
    FileBufferSaveSnapshot, FileBufferSaveStamp, FileBufferStore, FileBufferStoreLimits,
    FileBufferStoreSnapshot, FileBufferTextSnapshot, TextBufferLanguage, TextBufferRole,
};
pub(crate) use reader::{read_project_disk_text_snapshot, ProjectDiskTextReadOutcome};
pub use session_binding::{
    require_file_buffer_session_binding, FileBufferCommandReceipt, FileBufferRequestIdentity,
    FILE_BUFFER_IDENTITY_INVALID_CODE, FILE_BUFFER_STALE_SESSION_CODE,
    FILE_BUFFER_STORE_SESSION_MISMATCH_CODE,
};
pub use store::now_ms;

#[cfg(test)]
mod tests {
    use super::{
        changeset::{
            FileBufferChangeCoordinateSpace, FileBufferChangeSetInput, FileBufferTextChange,
        },
        hash::hash_text,
        model::{
            FileBufferEntry, FileBufferMutationExpectation, FileBufferStore, FileBufferStoreLimits,
            TextBufferLanguage, TextBufferRole,
        },
        store::{clean_baseline_for_test, FILE_BUFFER_DRAFT_CAS_CONFLICT_CODE},
    };

    #[test]
    fn draft_state_is_derived_from_baseline_hash() {
        let mut store = FileBufferStore::new(
            "session-1",
            "/tmp/project",
            1,
            FileBufferStoreLimits {
                max_files: 10,
                max_file_bytes: 1024,
                max_total_bytes: 4096,
            },
        );
        store.insert_loaded_file(FileBufferEntry {
            relative_path: "templates/index.html".to_string(),
            absolute_path: "/tmp/project/templates/index.html".to_string(),
            language: TextBufferLanguage::Html,
            role: TextBufferRole::Template,
            baseline: clean_baseline_for_test("<main>Base</main>"),
            baseline_text: "<main>Base</main>".to_string(),
            draft: None,
            revision: 1,
        });

        let dirty = store
            .set_draft("templates/index.html", "<main>Draft</main>".to_string(), 2)
            .unwrap();

        assert!(dirty.dirty);
        assert_eq!(dirty.current_hash, hash_text("<main>Draft</main>"));
        assert_eq!(store.snapshot().dirty_file_count, 1);

        let clean = store
            .set_draft("templates/index.html", "<main>Base</main>".to_string(), 3)
            .unwrap();

        assert!(!clean.dirty);
        assert_eq!(store.snapshot().dirty_file_count, 0);
    }

    #[test]
    fn changeset_applies_utf16_ranges_from_editor_transactions() {
        let mut store = test_store();
        store.insert_loaded_file(FileBufferEntry {
            relative_path: "templates/index.html".to_string(),
            absolute_path: "/tmp/project/templates/index.html".to_string(),
            language: TextBufferLanguage::Html,
            role: TextBufferRole::Template,
            baseline: clean_baseline_for_test("a😀b"),
            baseline_text: "a😀b".to_string(),
            draft: None,
            revision: 1,
        });

        let result = store
            .apply_changeset(
                FileBufferChangeSetInput {
                    relative_path: "templates/index.html".to_string(),
                    base_revision: Some(1),
                    base_hash: Some(hash_text("a😀b")),
                    coordinate_space: FileBufferChangeCoordinateSpace::Utf16,
                    source: Some("codemirror".to_string()),
                    changes: vec![FileBufferTextChange {
                        from: 1,
                        to: 3,
                        insert: "x".to_string(),
                    }],
                },
                2,
            )
            .unwrap();

        assert!(result.applied);
        assert_eq!(result.previous_revision, 1);
        assert_eq!(result.revision, 2);
        assert_eq!(store.text_for("templates/index.html").unwrap(), "axb");
        assert!(result.file.dirty);
    }

    #[test]
    fn changeset_applies_multiple_ranges_in_start_document_coordinates() {
        let mut store = test_store();
        store.insert_loaded_file(FileBufferEntry {
            relative_path: "templates/index.html".to_string(),
            absolute_path: "/tmp/project/templates/index.html".to_string(),
            language: TextBufferLanguage::Html,
            role: TextBufferRole::Template,
            baseline: clean_baseline_for_test("abcdef"),
            baseline_text: "abcdef".to_string(),
            draft: None,
            revision: 1,
        });

        store
            .apply_changeset(
                FileBufferChangeSetInput {
                    relative_path: "templates/index.html".to_string(),
                    base_revision: None,
                    base_hash: None,
                    coordinate_space: FileBufferChangeCoordinateSpace::Utf16,
                    source: Some("codemirror".to_string()),
                    changes: vec![
                        FileBufferTextChange {
                            from: 1,
                            to: 2,
                            insert: "X".to_string(),
                        },
                        FileBufferTextChange {
                            from: 4,
                            to: 6,
                            insert: "YZ".to_string(),
                        },
                    ],
                },
                2,
            )
            .unwrap();

        assert_eq!(store.text_for("templates/index.html").unwrap(), "aXcdYZ");
    }

    #[test]
    fn changeset_blocks_stale_revision_before_mutating_buffer() {
        let mut store = test_store();
        store.insert_loaded_file(FileBufferEntry {
            relative_path: "templates/index.html".to_string(),
            absolute_path: "/tmp/project/templates/index.html".to_string(),
            language: TextBufferLanguage::Html,
            role: TextBufferRole::Template,
            baseline: clean_baseline_for_test("base"),
            baseline_text: "base".to_string(),
            draft: None,
            revision: 4,
        });

        let error = store
            .apply_changeset(
                FileBufferChangeSetInput {
                    relative_path: "templates/index.html".to_string(),
                    base_revision: Some(3),
                    base_hash: None,
                    coordinate_space: FileBufferChangeCoordinateSpace::Utf16,
                    source: None,
                    changes: vec![FileBufferTextChange {
                        from: 0,
                        to: 4,
                        insert: "draft".to_string(),
                    }],
                },
                5,
            )
            .unwrap_err();

        assert!(error.contains("revizia așteptată"));
        assert_eq!(store.text_for("templates/index.html").unwrap(), "base");
    }

    #[test]
    fn changeset_blocks_stale_hash_before_mutating_buffer() {
        let mut store = test_store();
        store.insert_loaded_file(FileBufferEntry {
            relative_path: "templates/index.html".to_string(),
            absolute_path: "/tmp/project/templates/index.html".to_string(),
            language: TextBufferLanguage::Html,
            role: TextBufferRole::Template,
            baseline: clean_baseline_for_test("base"),
            baseline_text: "base".to_string(),
            draft: None,
            revision: 1,
        });
        store
            .set_draft("templates/index.html", "draft".to_string(), 2)
            .unwrap();

        let error = store
            .apply_changeset(
                FileBufferChangeSetInput {
                    relative_path: "templates/index.html".to_string(),
                    base_revision: Some(2),
                    base_hash: Some(hash_text("base")),
                    coordinate_space: FileBufferChangeCoordinateSpace::Utf16,
                    source: Some("codemirror".to_string()),
                    changes: vec![FileBufferTextChange {
                        from: 0,
                        to: 5,
                        insert: "other".to_string(),
                    }],
                },
                3,
            )
            .unwrap_err();

        assert!(error.contains("hash-ul de bază"));
        assert_eq!(store.text_for("templates/index.html").unwrap(), "draft");
    }

    #[test]
    fn full_draft_cas_blocks_stale_overwrite_after_concurrent_mutation() {
        let mut store = store_with_index_baseline("base");
        let expectation = FileBufferMutationExpectation {
            expected_revision: 1,
            expected_hash: hash_text("base"),
        };
        store
            .set_draft(
                "templates/index.html",
                "concurrent authority".to_string(),
                2,
            )
            .unwrap();

        let error = store
            .set_draft_if_current(
                "templates/index.html",
                "stale frontend".to_string(),
                &expectation,
                3,
            )
            .unwrap_err();

        assert!(error.contains(FILE_BUFFER_DRAFT_CAS_CONFLICT_CODE));
        assert_eq!(
            store.text_for("templates/index.html").unwrap(),
            "concurrent authority"
        );
        assert_eq!(
            store
                .text_snapshot("templates/index.html")
                .unwrap()
                .revision,
            2
        );
    }

    #[test]
    fn clear_draft_cas_blocks_stale_clear_after_concurrent_mutation() {
        let mut store = store_with_index_baseline("base");
        store
            .set_draft("templates/index.html", "owned draft".to_string(), 2)
            .unwrap();
        let expectation = FileBufferMutationExpectation {
            expected_revision: 2,
            expected_hash: hash_text("owned draft"),
        };
        store
            .set_draft("templates/index.html", "newer draft".to_string(), 3)
            .unwrap();

        let error = store
            .clear_draft_if_current("templates/index.html", &expectation)
            .unwrap_err();

        assert!(error.contains(FILE_BUFFER_DRAFT_CAS_CONFLICT_CODE));
        assert_eq!(
            store.text_for("templates/index.html").unwrap(),
            "newer draft"
        );
        assert_eq!(
            store
                .text_snapshot("templates/index.html")
                .unwrap()
                .revision,
            3
        );
    }

    #[test]
    fn full_draft_cas_retry_is_idempotent_after_lost_receipt() {
        let mut store = store_with_index_baseline("base");
        let expectation = FileBufferMutationExpectation {
            expected_revision: 1,
            expected_hash: hash_text("base"),
        };

        let first = store
            .set_draft_if_current(
                "templates/index.html",
                "desired".to_string(),
                &expectation,
                2,
            )
            .unwrap();
        let retry = store
            .set_draft_if_current(
                "templates/index.html",
                "desired".to_string(),
                &expectation,
                3,
            )
            .unwrap();

        assert_eq!(first.revision, 2);
        assert_eq!(retry.revision, first.revision);
        assert_eq!(retry.current_hash, first.current_hash);
    }

    #[test]
    fn clear_draft_cas_retry_is_idempotent_after_lost_receipt() {
        let mut store = store_with_index_baseline("base");
        let dirty = store
            .set_draft("templates/index.html", "draft".to_string(), 2)
            .unwrap();
        let expectation = FileBufferMutationExpectation {
            expected_revision: dirty.revision,
            expected_hash: dirty.current_hash,
        };

        let first = store
            .clear_draft_if_current("templates/index.html", &expectation)
            .unwrap();
        let retry = store
            .clear_draft_if_current("templates/index.html", &expectation)
            .unwrap();

        assert_eq!(first.revision, 3);
        assert_eq!(retry.revision, first.revision);
        assert!(!retry.has_draft);
        assert!(!retry.dirty);
    }

    #[test]
    fn changeset_clears_draft_when_text_returns_to_baseline() {
        let mut store = test_store();
        store.insert_loaded_file(FileBufferEntry {
            relative_path: "templates/index.html".to_string(),
            absolute_path: "/tmp/project/templates/index.html".to_string(),
            language: TextBufferLanguage::Html,
            role: TextBufferRole::Template,
            baseline: clean_baseline_for_test("base"),
            baseline_text: "base".to_string(),
            draft: None,
            revision: 1,
        });
        store
            .set_draft("templates/index.html", "draft".to_string(), 2)
            .unwrap();

        let result = store
            .apply_changeset(
                FileBufferChangeSetInput {
                    relative_path: "templates/index.html".to_string(),
                    base_revision: None,
                    base_hash: None,
                    coordinate_space: FileBufferChangeCoordinateSpace::Utf16,
                    source: None,
                    changes: vec![FileBufferTextChange {
                        from: 0,
                        to: 5,
                        insert: "base".to_string(),
                    }],
                },
                3,
            )
            .unwrap();

        assert!(!result.file.has_draft);
        assert!(!result.file.dirty);
        assert_eq!(store.snapshot().dirty_file_count, 0);
    }

    #[test]
    fn record_removed_file_drops_loaded_baseline() {
        let mut store = FileBufferStore::new(
            "session-1",
            "/tmp/project",
            1,
            FileBufferStoreLimits {
                max_files: 10,
                max_file_bytes: 1024,
                max_total_bytes: 4096,
            },
        );
        store.insert_loaded_file(FileBufferEntry {
            relative_path: "templates/new.html".to_string(),
            absolute_path: "/tmp/project/templates/new.html".to_string(),
            language: TextBufferLanguage::Html,
            role: TextBufferRole::Template,
            baseline: clean_baseline_for_test("<main>New</main>"),
            baseline_text: "<main>New</main>".to_string(),
            draft: None,
            revision: 1,
        });

        store.record_removed_file("templates/new.html").unwrap();

        assert!(store.text_for("templates/new.html").is_none());
        assert_eq!(store.snapshot().loaded_file_count, 0);
    }

    #[test]
    fn record_moved_entry_rekeys_loaded_file() {
        let mut store = test_store();
        store.insert_loaded_file(FileBufferEntry {
            relative_path: "templates/index.html".to_string(),
            absolute_path: "/tmp/project/templates/index.html".to_string(),
            language: TextBufferLanguage::Html,
            role: TextBufferRole::Template,
            baseline: clean_baseline_for_test("<main>Base</main>"),
            baseline_text: "<main>Base</main>".to_string(),
            draft: None,
            revision: 1,
        });

        store
            .record_moved_entry(
                "templates/index.html",
                "templates/archive/index.html",
                std::path::Path::new("/tmp/project"),
            )
            .unwrap();

        assert!(store.text_for("templates/index.html").is_none());
        assert!(store.text_for("templates/archive/index.html").is_some());
        let snapshot = store
            .snapshot()
            .files
            .into_iter()
            .find(|file| file.relative_path == "templates/archive/index.html")
            .unwrap();
        assert_eq!(
            snapshot.absolute_path,
            "/tmp/project/templates/archive/index.html"
        );
    }

    #[test]
    fn planned_moved_entry_paths_block_destination_baseline_collision() {
        let mut store = test_store();
        for relative_path in ["templates/index.html", "templates/archive/index.html"] {
            store.insert_loaded_file(FileBufferEntry {
                relative_path: relative_path.to_string(),
                absolute_path: format!("/tmp/project/{relative_path}"),
                language: TextBufferLanguage::Html,
                role: TextBufferRole::Template,
                baseline: clean_baseline_for_test("<main>Base</main>"),
                baseline_text: "<main>Base</main>".to_string(),
                draft: None,
                revision: 1,
            });
        }

        let error = store
            .planned_moved_entry_paths("templates/index.html", "templates/archive/index.html")
            .unwrap_err();

        assert!(error.contains("există deja baseline"));
    }

    #[test]
    fn record_trashed_entry_removes_nested_loaded_files() {
        let mut store = test_store();
        for relative_path in [
            "templates/sections/hero.html",
            "templates/sections/cards.html",
        ] {
            store.insert_loaded_file(FileBufferEntry {
                relative_path: relative_path.to_string(),
                absolute_path: format!("/tmp/project/{relative_path}"),
                language: TextBufferLanguage::Html,
                role: TextBufferRole::Template,
                baseline: clean_baseline_for_test("<section></section>"),
                baseline_text: "<section></section>".to_string(),
                draft: None,
                revision: 1,
            });
        }

        let touched = store.planned_trashed_entry_paths("templates/sections");
        let removed = store.record_trashed_entry("templates/sections");

        assert_eq!(
            touched,
            vec![
                "templates/sections",
                "templates/sections/cards.html",
                "templates/sections/hero.html",
            ]
        );
        assert_eq!(removed.len(), 2);
        assert!(store.text_for("templates/sections/hero.html").is_none());
        assert_eq!(store.snapshot().loaded_file_count, 0);
    }

    #[test]
    fn record_restored_entries_rehydrates_loaded_files() {
        let mut store = test_store();
        let entry = FileBufferEntry {
            relative_path: "templates/index.html".to_string(),
            absolute_path: "/old/project/templates/index.html".to_string(),
            language: TextBufferLanguage::Html,
            role: TextBufferRole::Template,
            baseline: clean_baseline_for_test("<main>Base</main>"),
            baseline_text: "<main>Base</main>".to_string(),
            draft: None,
            revision: 1,
        };

        store
            .record_restored_entries(&[entry], std::path::Path::new("/tmp/project"))
            .unwrap();

        let snapshot = store
            .snapshot()
            .files
            .into_iter()
            .find(|file| file.relative_path == "templates/index.html")
            .unwrap();
        assert_eq!(snapshot.absolute_path, "/tmp/project/templates/index.html");
        assert_eq!(snapshot.revision, 2);
    }

    fn test_store() -> FileBufferStore {
        FileBufferStore::new(
            "session-1",
            "/tmp/project",
            1,
            FileBufferStoreLimits {
                max_files: 10,
                max_file_bytes: 1024,
                max_total_bytes: 4096,
            },
        )
    }

    fn store_with_index_baseline(baseline: &str) -> FileBufferStore {
        let mut store = test_store();
        store.insert_loaded_file(FileBufferEntry {
            relative_path: "templates/index.html".to_string(),
            absolute_path: "/tmp/project/templates/index.html".to_string(),
            language: TextBufferLanguage::Html,
            role: TextBufferRole::Template,
            baseline: clean_baseline_for_test(baseline),
            baseline_text: baseline.to_string(),
            draft: None,
            revision: 1,
        });
        store
    }
}
