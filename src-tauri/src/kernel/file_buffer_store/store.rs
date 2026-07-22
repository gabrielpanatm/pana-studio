use std::{path::Path, time::UNIX_EPOCH};

use crate::kernel::project_session::ProjectSessionSnapshot;

use super::{
    changeset::{apply_text_changes, FileBufferChangeSetInput, FileBufferChangeSetResult},
    classify::language_for_relative_path,
    hash::hash_text,
    model::{
        FileBufferDiagnostic, FileBufferDraft, FileBufferEntry, FileBufferFileSnapshot,
        FileBufferMutationExpectation, FileBufferSaveProjection, FileBufferSaveSnapshot,
        FileBufferSaveStamp, FileBufferStore, FileBufferStoreLimits, FileBufferStoreSnapshot,
        FileBufferTextSnapshot, TextBufferLanguage, TextBufferRole,
        FILE_BUFFER_STORE_SCHEMA_VERSION,
    },
    reader::{baseline_from_text_path, project_path},
};

pub const FILE_BUFFER_DRAFT_CAS_CONFLICT_CODE: &str = "file_buffer_draft_cas_conflict";
pub const FILE_BUFFER_DRAFT_CAS_INVALID_CODE: &str = "file_buffer_draft_cas_invalid";
pub const FILE_BUFFER_SAVE_CAS_CONFLICT_CODE: &str = "file_buffer_save_cas_conflict";

impl FileBufferStore {
    pub fn new(
        session_id: impl Into<String>,
        project_root: impl Into<String>,
        loaded_at_ms: u128,
        limits: FileBufferStoreLimits,
    ) -> Self {
        let session_id = session_id.into();
        Self {
            schema_version: FILE_BUFFER_STORE_SCHEMA_VERSION,
            runtime_session_id: session_id.clone(),
            session_id,
            project_root: project_root.into(),
            loaded_at_ms,
            files: Default::default(),
            diagnostics: Vec::new(),
            limits,
        }
    }

    pub fn for_project_session(
        session: &ProjectSessionSnapshot,
        loaded_at_ms: u128,
        limits: FileBufferStoreLimits,
    ) -> Self {
        Self {
            schema_version: FILE_BUFFER_STORE_SCHEMA_VERSION,
            session_id: session.id.clone(),
            runtime_session_id: session.runtime_instance_id(),
            project_root: session.project_root.clone(),
            loaded_at_ms,
            files: Default::default(),
            diagnostics: Vec::new(),
            limits,
        }
    }

    pub fn insert_loaded_file(&mut self, entry: FileBufferEntry) {
        self.files.insert(entry.relative_path.clone(), entry);
    }

    /// Adds a text resource that exists only in the current editor session.
    /// Its empty baseline is synthetic; ProjectWorkspace separately records
    /// that Save must create the path rather than overwrite an accepted file.
    pub fn stage_new_text_file(
        &mut self,
        relative_path: &str,
        contents: String,
        updated_at_ms: u128,
    ) -> Result<FileBufferFileSnapshot, String> {
        if self.files.contains_key(relative_path) {
            return Err(format!(
                "FileBufferStore urmărește deja resursa {relative_path}."
            ));
        }
        if self.files.len() >= self.limits.max_files {
            return Err(format!(
                "FileBufferStore nu poate crea {relative_path}: limita de {} fișiere a fost atinsă.",
                self.limits.max_files
            ));
        }
        if contents.len() as u64 > self.limits.max_file_bytes {
            return Err(format!(
                "FileBufferStore nu poate crea {relative_path}: {} bytes depășesc limita de {} bytes.",
                contents.len(), self.limits.max_file_bytes
            ));
        }
        let total_bytes = self
            .files
            .values()
            .map(FileBufferEntry::current_bytes)
            .sum::<u64>()
            .saturating_add(contents.len() as u64);
        if total_bytes > self.limits.max_total_bytes {
            return Err(format!(
                "FileBufferStore nu poate crea {relative_path}: totalul ar depăși limita de {} bytes.",
                self.limits.max_total_bytes
            ));
        }

        let absolute_path = project_path(Path::new(&self.project_root), relative_path)?;
        let language =
            language_for_relative_path(relative_path).unwrap_or(TextBufferLanguage::Plain);
        let baseline_text = String::new();
        let baseline = super::model::FileBufferBaseline {
            hash: hash_text(&baseline_text),
            modified_ms: 0,
            size: 0,
            readonly: false,
        };
        let draft = Some(FileBufferDraft {
            hash: hash_text(&contents),
            bytes: contents.len() as u64,
            text: contents,
            updated_at_ms,
        });
        let entry = FileBufferEntry {
            relative_path: relative_path.to_string(),
            absolute_path: absolute_path.to_string_lossy().into_owned(),
            language,
            role: role_for_session_resource(relative_path, language),
            baseline,
            baseline_text,
            draft,
            revision: 1,
        };
        let snapshot = entry.snapshot();
        self.files.insert(relative_path.to_string(), entry);
        Ok(snapshot)
    }

    pub fn add_diagnostic(&mut self, diagnostic: FileBufferDiagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn text_snapshot(&self, relative_path: &str) -> Option<FileBufferTextSnapshot> {
        let entry = self.files.get(relative_path)?;
        let text = entry.current_text().to_string();
        Some(FileBufferTextSnapshot {
            relative_path: entry.relative_path.clone(),
            hash: hash_text(&text),
            bytes: text.len() as u64,
            dirty: entry.is_dirty(),
            revision: entry.revision,
            text,
        })
    }

    pub fn text_for(&self, relative_path: &str) -> Option<String> {
        self.files
            .get(relative_path)
            .map(|entry| entry.current_text().to_string())
    }

    pub fn save_stamp_for(&self, relative_path: &str) -> Option<FileBufferSaveStamp> {
        self.files
            .get(relative_path)
            .map(FileBufferEntry::save_stamp)
    }

    pub fn capture_save_snapshot(&self, relative_path: &str) -> Option<FileBufferSaveSnapshot> {
        let entry = self.files.get(relative_path)?;
        Some(FileBufferSaveSnapshot {
            relative_path: entry.relative_path.clone(),
            contents: entry.current_text().to_string(),
            stamp: entry.save_stamp(),
        })
    }

    pub fn capture_dirty_save_snapshot(
        &self,
        relative_path: &str,
    ) -> Option<FileBufferSaveSnapshot> {
        self.capture_save_snapshot(relative_path)
            .filter(|snapshot| snapshot.stamp.dirty)
    }

    pub fn require_save_stamp_current(
        &self,
        relative_path: &str,
        expected: &FileBufferSaveStamp,
    ) -> Result<FileBufferSaveStamp, String> {
        let current = self.save_stamp_for(relative_path).ok_or_else(|| {
            format!(
                "[{FILE_BUFFER_SAVE_CAS_CONFLICT_CODE}] Save a devenit stale pentru {relative_path}: bufferul nu mai este urmărit."
            )
        })?;
        if &current != expected {
            return Err(format!(
                "[{FILE_BUFFER_SAVE_CAS_CONFLICT_CODE}] Save a devenit stale pentru {relative_path}: expected revision/hash/bytes/dirty {}/{}/{}/{}, current {}/{}/{}/{}.",
                expected.revision,
                expected.hash,
                expected.bytes,
                expected.dirty,
                current.revision,
                current.hash,
                current.bytes,
                current.dirty,
            ));
        }
        Ok(current)
    }

    pub fn baseline_text_for(&self, relative_path: &str) -> Option<String> {
        self.files
            .get(relative_path)
            .map(|entry| entry.baseline_text.clone())
    }

    pub fn set_draft(
        &mut self,
        relative_path: &str,
        contents: String,
        updated_at_ms: u128,
    ) -> Result<FileBufferFileSnapshot, String> {
        let entry = self
            .files
            .get_mut(relative_path)
            .ok_or_else(|| format!("FileBufferStore nu are baseline pentru {relative_path}."))?;
        let hash = hash_text(&contents);
        entry.draft = Some(FileBufferDraft {
            bytes: contents.len() as u64,
            text: contents,
            hash,
            updated_at_ms,
        });
        entry.revision = entry.revision.saturating_add(1);
        Ok(entry.snapshot())
    }

    pub fn clear_draft(&mut self, relative_path: &str) -> Result<FileBufferFileSnapshot, String> {
        let entry = self
            .files
            .get_mut(relative_path)
            .ok_or_else(|| format!("FileBufferStore nu are baseline pentru {relative_path}."))?;
        entry.draft = None;
        entry.revision = entry.revision.saturating_add(1);
        Ok(entry.snapshot())
    }

    pub fn set_draft_if_current(
        &mut self,
        relative_path: &str,
        contents: String,
        expectation: &FileBufferMutationExpectation,
        updated_at_ms: u128,
    ) -> Result<FileBufferFileSnapshot, String> {
        if contents.len() as u64 > self.limits.max_file_bytes {
            return Err(format!(
                "FileBufferStore a refuzat draftul complet pentru {relative_path}: are {} bytes, peste limita de {} bytes.",
                contents.len(),
                self.limits.max_file_bytes,
            ));
        }

        let entry = self
            .files
            .get_mut(relative_path)
            .ok_or_else(|| format!("FileBufferStore nu are baseline pentru {relative_path}."))?;
        require_valid_file_buffer_mutation_expectation(relative_path, expectation)?;

        // A retry whose first response was lost is safe only when its desired state
        // is already authoritative. It must not create another revision.
        if entry.current_text() == contents {
            return Ok(entry.snapshot());
        }

        require_file_buffer_mutation_expectation(entry, relative_path, expectation)?;
        let hash = hash_text(&contents);
        entry.draft = Some(FileBufferDraft {
            bytes: contents.len() as u64,
            text: contents,
            hash,
            updated_at_ms,
        });
        entry.revision = entry.revision.saturating_add(1);
        Ok(entry.snapshot())
    }

    pub fn clear_draft_if_current(
        &mut self,
        relative_path: &str,
        expectation: &FileBufferMutationExpectation,
    ) -> Result<FileBufferFileSnapshot, String> {
        let entry = self
            .files
            .get_mut(relative_path)
            .ok_or_else(|| format!("FileBufferStore nu are baseline pentru {relative_path}."))?;
        require_valid_file_buffer_mutation_expectation(relative_path, expectation)?;

        // Clearing an already clean entry is an idempotent retry, not a mutation.
        if entry.draft.is_none() {
            return Ok(entry.snapshot());
        }

        require_file_buffer_mutation_expectation(entry, relative_path, expectation)?;
        entry.draft = None;
        entry.revision = entry.revision.saturating_add(1);
        Ok(entry.snapshot())
    }

    pub fn apply_changeset(
        &mut self,
        input: FileBufferChangeSetInput,
        updated_at_ms: u128,
    ) -> Result<FileBufferChangeSetResult, String> {
        let relative_path = input.relative_path.trim().to_string();
        if relative_path.is_empty() {
            return Err("FileBufferStore a refuzat change-set-ul: path gol.".to_string());
        }
        let max_file_bytes = self.limits.max_file_bytes;

        let entry = self
            .files
            .get_mut(&relative_path)
            .ok_or_else(|| format!("FileBufferStore nu are baseline pentru {relative_path}."))?;

        if let Some(base_revision) = input.base_revision {
            if base_revision != entry.revision {
                return Err(format!(
                    "FileBufferStore a refuzat change-set-ul pentru {relative_path}: revizia așteptată {base_revision}, revizia curentă {}.",
                    entry.revision
                ));
            }
        }

        let current_text = entry.current_text().to_string();
        let current_hash = hash_text(&current_text);
        if let Some(base_hash) = input.base_hash.as_deref() {
            if base_hash != current_hash {
                return Err(format!(
                    "FileBufferStore a refuzat change-set-ul pentru {relative_path}: hash-ul de bază nu mai corespunde bufferului curent."
                ));
            }
        }

        let previous_revision = entry.revision;
        let applied = apply_text_changes(&current_text, &input.changes, input.coordinate_space)?;

        if applied.text.len() as u64 > max_file_bytes {
            return Err(format!(
                "FileBufferStore a refuzat change-set-ul pentru {relative_path}: draftul rezultat are {} bytes, peste limita de {} bytes.",
                applied.text.len(),
                max_file_bytes
            ));
        }

        if applied.applied {
            if applied.current_hash == entry.baseline.hash {
                entry.draft = None;
            } else {
                entry.draft = Some(FileBufferDraft {
                    bytes: applied.text.len() as u64,
                    text: applied.text,
                    hash: applied.current_hash.clone(),
                    updated_at_ms,
                });
            }
            entry.revision = entry.revision.saturating_add(1);
        }

        Ok(FileBufferChangeSetResult {
            relative_path,
            source: input.source,
            previous_revision,
            revision: entry.revision,
            previous_hash: applied.previous_hash,
            current_hash: entry.current_hash(),
            change_count: input.changes.len(),
            applied: applied.applied,
            file: entry.snapshot(),
        })
    }

    pub fn record_saved_text(
        &mut self,
        relative_path: &str,
        contents: String,
    ) -> Result<(), String> {
        if contents.len() as u64 > self.limits.max_file_bytes {
            self.files.remove(relative_path);
            self.add_diagnostic(FileBufferDiagnostic::warning(
                "saved_file_too_large",
                Some(relative_path.to_string()),
                format!(
                    "Fișierul salvat are {} bytes, peste limita FileBufferStore de {} bytes.",
                    contents.len(),
                    self.limits.max_file_bytes
                ),
            ));
            return Ok(());
        }

        let path = project_path(Path::new(&self.project_root), relative_path)?;
        let baseline = baseline_from_text_path(&path, &contents)?;
        if let Some(entry) = self.files.get_mut(relative_path) {
            entry.baseline = baseline;
            entry.baseline_text = contents;
            entry.draft = None;
            entry.revision = entry.revision.saturating_add(1);
            return Ok(());
        }

        let language =
            language_for_relative_path(relative_path).unwrap_or(TextBufferLanguage::Plain);
        self.files.insert(
            relative_path.to_string(),
            FileBufferEntry {
                relative_path: relative_path.to_string(),
                absolute_path: path.to_string_lossy().to_string(),
                language,
                role: TextBufferRole::Other,
                baseline,
                baseline_text: contents,
                draft: None,
                revision: 1,
            },
        );
        Ok(())
    }

    /// Projects a committed disk Save back into FileBufferStore under the
    /// exact stamp captured before the disk effect.
    ///
    /// A newer dirty draft is not a projection failure: the committed payload
    /// becomes the new baseline and that draft is retained on top. Any other
    /// mismatch is fail-closed because its provenance cannot be demonstrated.
    pub fn record_saved_text_if_current(
        &mut self,
        relative_path: &str,
        contents: String,
        expected: Option<&FileBufferSaveStamp>,
    ) -> Result<FileBufferSaveProjection, String> {
        if contents.len() as u64 > self.limits.max_file_bytes {
            return Err(format!(
                "FileBufferStore nu poate proiecta Save-ul pentru {relative_path}: payloadul comis are {} bytes, peste limita de {} bytes.",
                contents.len(),
                self.limits.max_file_bytes,
            ));
        }

        let path = project_path(Path::new(&self.project_root), relative_path)?;
        let baseline = baseline_from_text_path(&path, &contents)?;
        let before = self.save_stamp_for(relative_path);

        if let Some(expected) = expected {
            match self.files.get_mut(relative_path) {
                Some(entry) if entry.save_stamp() == *expected => {
                    entry.baseline = baseline;
                    entry.baseline_text = contents;
                    entry.draft = None;
                    entry.revision = entry.revision.saturating_add(1);
                    return Ok(FileBufferSaveProjection {
                        before,
                        after: entry.save_stamp(),
                        retained_newer_draft: false,
                    });
                }
                Some(entry)
                    if entry.revision > expected.revision
                        && entry.draft.is_some()
                        && entry.is_dirty() =>
                {
                    let newer_draft = entry
                        .draft
                        .take()
                        .expect("guarded newer FileBuffer draft must exist");
                    entry.baseline = baseline;
                    entry.baseline_text = contents;
                    entry.draft = if newer_draft.hash == entry.baseline.hash {
                        None
                    } else {
                        Some(newer_draft)
                    };
                    entry.revision = entry.revision.saturating_add(1);
                    return Ok(FileBufferSaveProjection {
                        before,
                        after: entry.save_stamp(),
                        retained_newer_draft: entry.is_dirty(),
                    });
                }
                Some(entry) => {
                    let current = entry.save_stamp();
                    return Err(format!(
                        "[{FILE_BUFFER_SAVE_CAS_CONFLICT_CODE}] Save-ul pentru {relative_path} a fost comis pe disk, dar proiecția FileBufferStore nu poate demonstra un draft mai nou: expected revision/hash/bytes/dirty {}/{}/{}/{}, current {}/{}/{}/{}.",
                        expected.revision,
                        expected.hash,
                        expected.bytes,
                        expected.dirty,
                        current.revision,
                        current.hash,
                        current.bytes,
                        current.dirty,
                    ));
                }
                None => {
                    return Err(format!(
                        "[{FILE_BUFFER_SAVE_CAS_CONFLICT_CODE}] Save-ul pentru {relative_path} a fost comis pe disk, dar bufferul capturat nu mai există la proiecție."
                    ));
                }
            }
        }

        if let Some(entry) = self.files.get_mut(relative_path) {
            entry.baseline = baseline;
            entry.baseline_text = contents;
            entry.draft = None;
            entry.revision = entry.revision.saturating_add(1);
            return Ok(FileBufferSaveProjection {
                before,
                after: entry.save_stamp(),
                retained_newer_draft: false,
            });
        }

        let language =
            language_for_relative_path(relative_path).unwrap_or(TextBufferLanguage::Plain);
        self.files.insert(
            relative_path.to_string(),
            FileBufferEntry {
                relative_path: relative_path.to_string(),
                absolute_path: path.to_string_lossy().to_string(),
                language,
                role: TextBufferRole::Other,
                baseline,
                baseline_text: contents,
                draft: None,
                revision: 1,
            },
        );
        let after = self
            .save_stamp_for(relative_path)
            .expect("inserted FileBuffer entry must expose a Save stamp");
        Ok(FileBufferSaveProjection {
            before,
            after,
            retained_newer_draft: false,
        })
    }

    pub fn record_removed_file(&mut self, relative_path: &str) -> Result<(), String> {
        self.files
            .remove(relative_path)
            .map(|_| ())
            .ok_or_else(|| format!("FileBufferStore nu are baseline pentru {relative_path}."))
    }

    pub fn tracked_paths_for_entry(&self, source_relative_path: &str) -> Vec<String> {
        let prefix = format!("{source_relative_path}/");
        let mut paths = self
            .files
            .keys()
            .filter(|path| path.as_str() == source_relative_path || path.starts_with(&prefix))
            .cloned()
            .collect::<Vec<_>>();
        paths.sort();
        paths
    }

    pub fn planned_trashed_entry_paths(&self, source_relative_path: &str) -> Vec<String> {
        let mut touched = vec![source_relative_path.to_string()];
        touched.extend(self.tracked_paths_for_entry(source_relative_path));
        touched.sort();
        touched.dedup();
        touched
    }

    pub fn record_trashed_entry(&mut self, source_relative_path: &str) -> Vec<FileBufferEntry> {
        let paths = self.tracked_paths_for_entry(source_relative_path);
        let mut removed = Vec::new();
        for path in paths {
            if let Some(entry) = self.files.remove(&path) {
                removed.push(entry);
            }
        }
        removed
    }

    pub fn record_restored_entries(
        &mut self,
        entries: &[FileBufferEntry],
        project_root: &Path,
    ) -> Result<(), String> {
        for entry in entries {
            if self.files.contains_key(&entry.relative_path) {
                return Err(format!(
                    "FileBufferStore a blocat restore: există deja baseline pentru {}.",
                    entry.relative_path
                ));
            }
        }

        for entry in entries {
            let mut restored = entry.clone();
            restored.absolute_path = project_path(project_root, &restored.relative_path)?
                .to_string_lossy()
                .to_string();
            restored.revision = restored.revision.saturating_add(1);
            self.files.insert(restored.relative_path.clone(), restored);
        }

        Ok(())
    }

    pub fn planned_moved_entry_paths(
        &self,
        source_relative_path: &str,
        destination_relative_path: &str,
    ) -> Result<Vec<String>, String> {
        let mappings = self.move_mappings(source_relative_path, destination_relative_path)?;
        let mut touched = vec![
            source_relative_path.to_string(),
            destination_relative_path.to_string(),
        ];
        for (from, to) in mappings {
            touched.push(from);
            touched.push(to);
        }
        touched.sort();
        touched.dedup();
        Ok(touched)
    }

    pub fn record_moved_entry(
        &mut self,
        source_relative_path: &str,
        destination_relative_path: &str,
        project_root: &Path,
    ) -> Result<Vec<String>, String> {
        let mappings = self.move_mappings(source_relative_path, destination_relative_path)?;
        let moved_paths = mappings
            .iter()
            .map(|(_, destination)| destination.clone())
            .collect::<Vec<_>>();

        let mut moved_entries = Vec::new();
        for (from, to) in mappings {
            let entry = self
                .files
                .remove(&from)
                .ok_or_else(|| format!("FileBufferStore nu are baseline pentru mutarea {from}."))?;
            moved_entries.push((to, entry));
        }
        for (to, mut entry) in moved_entries {
            entry.relative_path = to.clone();
            entry.absolute_path = project_path(project_root, &to)?
                .to_string_lossy()
                .to_string();
            entry.revision = entry.revision.saturating_add(1);
            self.files.insert(to, entry);
        }

        Ok(moved_paths)
    }

    fn move_mappings(
        &self,
        source_relative_path: &str,
        destination_relative_path: &str,
    ) -> Result<Vec<(String, String)>, String> {
        let source_prefix = format!("{source_relative_path}/");
        let mut mappings = self
            .files
            .keys()
            .filter_map(|path| {
                if path == source_relative_path {
                    return Some((path.clone(), destination_relative_path.to_string()));
                }
                path.strip_prefix(&source_prefix)
                    .map(|rest| (path.clone(), format!("{destination_relative_path}/{rest}")))
            })
            .collect::<Vec<_>>();
        mappings.sort_by(|left, right| left.0.cmp(&right.0));

        let moved_sources = mappings
            .iter()
            .map(|(from, _)| from.as_str())
            .collect::<std::collections::HashSet<_>>();
        for (_, to) in &mappings {
            if self.files.contains_key(to) && !moved_sources.contains(to.as_str()) {
                return Err(format!(
                    "FileBufferStore a blocat move: există deja baseline pentru destinația {to}."
                ));
            }
        }

        Ok(mappings)
    }

    pub fn snapshot(&self) -> FileBufferStoreSnapshot {
        let files = self
            .files
            .values()
            .map(FileBufferEntry::snapshot)
            .collect::<Vec<_>>();
        let dirty_file_count = files.iter().filter(|file| file.dirty).count();
        let total_loaded_bytes = self
            .files
            .values()
            .map(|entry| entry.baseline_text.len() as u64)
            .sum();
        FileBufferStoreSnapshot {
            schema_version: self.schema_version,
            session_id: self.session_id.clone(),
            runtime_session_id: self.runtime_session_id.clone(),
            project_root: self.project_root.clone(),
            loaded_at_ms: self.loaded_at_ms,
            file_count: files.len() + self.diagnostics.len(),
            loaded_file_count: files.len(),
            skipped_file_count: self.diagnostics.len(),
            dirty_file_count,
            total_loaded_bytes,
            limits: self.limits.clone(),
            files,
            diagnostics: self.diagnostics.clone(),
        }
    }
}

fn role_for_session_resource(relative_path: &str, language: TextBufferLanguage) -> TextBufferRole {
    if relative_path.starts_with("templates/") {
        return TextBufferRole::Template;
    }
    if relative_path.starts_with("content/") {
        return TextBufferRole::Page;
    }
    if matches!(language, TextBufferLanguage::Css | TextBufferLanguage::Scss) {
        return TextBufferRole::Style;
    }
    if language == TextBufferLanguage::JavaScript {
        return TextBufferRole::Script;
    }
    if matches!(
        language,
        TextBufferLanguage::Toml | TextBufferLanguage::Json | TextBufferLanguage::Yaml
    ) {
        return TextBufferRole::Config;
    }
    TextBufferRole::Other
}

fn require_file_buffer_mutation_expectation(
    entry: &FileBufferEntry,
    relative_path: &str,
    expectation: &FileBufferMutationExpectation,
) -> Result<(), String> {
    let current_hash = entry.current_hash();
    if expectation.expected_revision != entry.revision || expectation.expected_hash != current_hash
    {
        return Err(format!(
            "[{FILE_BUFFER_DRAFT_CAS_CONFLICT_CODE}] FileBufferStore a refuzat mutația CAS pentru {relative_path}: așteptat revision/hash {}/{}, curent {}/{}.",
            expectation.expected_revision,
            expectation.expected_hash,
            entry.revision,
            current_hash,
        ));
    }
    Ok(())
}

fn require_valid_file_buffer_mutation_expectation(
    relative_path: &str,
    expectation: &FileBufferMutationExpectation,
) -> Result<(), String> {
    if expectation.expected_hash.len() != 16
        || !expectation
            .expected_hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "[{FILE_BUFFER_DRAFT_CAS_INVALID_CODE}] FileBufferStore cere un expectedHash FNV-1a valid pentru {relative_path}."
        ));
    }
    Ok(())
}

impl FileBufferEntry {
    pub fn current_text(&self) -> &str {
        self.draft
            .as_ref()
            .map(|draft| draft.text.as_str())
            .unwrap_or(&self.baseline_text)
    }

    pub fn current_hash(&self) -> String {
        self.draft
            .as_ref()
            .map(|draft| draft.hash.clone())
            .unwrap_or_else(|| self.baseline.hash.clone())
    }

    pub fn current_bytes(&self) -> u64 {
        self.draft
            .as_ref()
            .map(|draft| draft.bytes)
            .unwrap_or(self.baseline_text.len() as u64)
    }

    pub fn is_dirty(&self) -> bool {
        self.draft
            .as_ref()
            .map(|draft| draft.hash != self.baseline.hash)
            .unwrap_or(false)
    }

    pub fn save_stamp(&self) -> FileBufferSaveStamp {
        FileBufferSaveStamp {
            revision: self.revision,
            hash: self.current_hash(),
            bytes: self.current_bytes(),
            dirty: self.is_dirty(),
        }
    }

    pub fn snapshot(&self) -> FileBufferFileSnapshot {
        FileBufferFileSnapshot {
            relative_path: self.relative_path.clone(),
            absolute_path: self.absolute_path.clone(),
            language: self.language,
            role: self.role,
            baseline: self.baseline.clone(),
            has_draft: self.draft.is_some(),
            dirty: self.is_dirty(),
            current_hash: self.current_hash(),
            current_bytes: self.current_bytes(),
            revision: self.revision,
        }
    }
}

impl FileBufferDraft {
    #[allow(dead_code)]
    pub fn updated_at_ms(&self) -> u128 {
        self.updated_at_ms
    }
}

pub fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
pub fn clean_baseline_for_test(text: &str) -> super::model::FileBufferBaseline {
    super::model::FileBufferBaseline {
        hash: hash_text(text),
        modified_ms: 0,
        size: text.len() as u64,
        readonly: false,
    }
}

#[cfg(test)]
mod save_cas_tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::{clean_baseline_for_test, hash_text, FILE_BUFFER_SAVE_CAS_CONFLICT_CODE};
    use crate::kernel::file_buffer_store::{
        FileBufferEntry, FileBufferStore, FileBufferStoreLimits, TextBufferLanguage, TextBufferRole,
    };

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);
    const RELATIVE_PATH: &str = "templates/index.html";

    #[test]
    fn dirty_save_snapshot_binds_revision_hash_bytes_and_dirty_state() {
        let (root, mut store) = store_with_disk_baseline("baseline");
        store
            .set_draft(RELATIVE_PATH, "captured draft".to_string(), 2)
            .unwrap();

        let snapshot = store.capture_dirty_save_snapshot(RELATIVE_PATH).unwrap();

        assert_eq!(snapshot.relative_path, RELATIVE_PATH);
        assert_eq!(snapshot.contents, "captured draft");
        assert_eq!(snapshot.stamp.revision, 2);
        assert_eq!(snapshot.stamp.hash, hash_text("captured draft"));
        assert_eq!(snapshot.stamp.bytes, 14);
        assert!(snapshot.stamp.dirty);
        store
            .require_save_stamp_current(RELATIVE_PATH, &snapshot.stamp)
            .unwrap();

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn post_disk_projection_clears_only_the_exact_captured_draft() {
        let (root, mut store) = store_with_disk_baseline("baseline");
        store
            .set_draft(RELATIVE_PATH, "captured draft".to_string(), 2)
            .unwrap();
        let snapshot = store.capture_dirty_save_snapshot(RELATIVE_PATH).unwrap();
        fs::write(root.join(RELATIVE_PATH), &snapshot.contents).unwrap();

        let projection = store
            .record_saved_text_if_current(
                RELATIVE_PATH,
                snapshot.contents.clone(),
                Some(&snapshot.stamp),
            )
            .unwrap();

        assert_eq!(projection.before, Some(snapshot.stamp));
        assert!(!projection.retained_newer_draft);
        assert!(!projection.after.dirty);
        assert_eq!(projection.after.revision, 3);
        assert_eq!(
            store.text_for(RELATIVE_PATH).as_deref(),
            Some("captured draft")
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn post_disk_projection_rebases_and_retains_a_newer_dirty_draft() {
        let (root, mut store) = store_with_disk_baseline("baseline");
        store
            .set_draft(RELATIVE_PATH, "captured draft".to_string(), 2)
            .unwrap();
        let snapshot = store.capture_dirty_save_snapshot(RELATIVE_PATH).unwrap();
        store
            .set_draft(RELATIVE_PATH, "newer draft".to_string(), 3)
            .unwrap();
        fs::write(root.join(RELATIVE_PATH), &snapshot.contents).unwrap();

        let projection = store
            .record_saved_text_if_current(RELATIVE_PATH, snapshot.contents, Some(&snapshot.stamp))
            .unwrap();

        assert!(projection.retained_newer_draft);
        assert!(projection.after.dirty);
        assert_eq!(projection.after.revision, 4);
        assert_eq!(projection.after.hash, hash_text("newer draft"));
        assert_eq!(
            store.text_for(RELATIVE_PATH).as_deref(),
            Some("newer draft")
        );
        assert_eq!(
            store.baseline_text_for(RELATIVE_PATH).as_deref(),
            Some("captured draft")
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stale_or_unexplained_stamp_is_refused_without_erasing_current_state() {
        let (root, mut store) = store_with_disk_baseline("baseline");
        store
            .set_draft(RELATIVE_PATH, "captured draft".to_string(), 2)
            .unwrap();
        let snapshot = store.capture_dirty_save_snapshot(RELATIVE_PATH).unwrap();
        store.clear_draft(RELATIVE_PATH).unwrap();
        fs::write(root.join(RELATIVE_PATH), &snapshot.contents).unwrap();

        let before = store.save_stamp_for(RELATIVE_PATH).unwrap();
        let preflight = store
            .require_save_stamp_current(RELATIVE_PATH, &snapshot.stamp)
            .unwrap_err();
        let projection = store
            .record_saved_text_if_current(RELATIVE_PATH, snapshot.contents, Some(&snapshot.stamp))
            .unwrap_err();

        assert!(preflight.contains(FILE_BUFFER_SAVE_CAS_CONFLICT_CODE));
        assert!(projection.contains(FILE_BUFFER_SAVE_CAS_CONFLICT_CODE));
        assert_eq!(store.save_stamp_for(RELATIVE_PATH).unwrap(), before);
        assert_eq!(store.text_for(RELATIVE_PATH).as_deref(), Some("baseline"));

        fs::remove_dir_all(root).unwrap();
    }

    fn store_with_disk_baseline(baseline: &str) -> (PathBuf, FileBufferStore) {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "pana-file-buffer-save-cas-{}-{counter}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(root.join(RELATIVE_PATH), baseline).unwrap();
        let root = root.canonicalize().unwrap();
        let mut store = FileBufferStore::new(
            "session-save-cas",
            root.to_string_lossy(),
            1,
            FileBufferStoreLimits {
                max_files: 10,
                max_file_bytes: 4096,
                max_total_bytes: 8192,
            },
        );
        store.insert_loaded_file(FileBufferEntry {
            relative_path: RELATIVE_PATH.to_string(),
            absolute_path: root.join(RELATIVE_PATH).to_string_lossy().to_string(),
            language: TextBufferLanguage::Html,
            role: TextBufferRole::Template,
            baseline: clean_baseline_for_test(baseline),
            baseline_text: baseline.to_string(),
            draft: None,
            revision: 1,
        });
        (root, store)
    }
}
