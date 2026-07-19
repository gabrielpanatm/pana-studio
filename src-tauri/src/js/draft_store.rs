use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::kernel::{observability::now_ms, project_session::ProjectSessionSnapshot};

use super::PageJsConfig;

pub const PAGE_JS_DRAFT_STORE_SCHEMA_VERSION: u32 = 2;
pub const PAGE_JS_DRAFT_MAX_TEMPLATE_PATH_BYTES: usize = 1024;
pub const PAGE_JS_DRAFT_MAX_SOURCE_BYTES: usize = 120;
pub const PAGE_JS_DRAFT_MAX_COALESCE_KEY_BYTES: usize = 160;
pub const PAGE_JS_DRAFT_MAX_TRANSACTION_ID_BYTES: usize = 256;

const DEFAULT_MAX_DRAFTS: usize = 128;
const DEFAULT_MAX_CONFIG_BYTES: usize = 2 * 1024 * 1024;
const DEFAULT_MAX_TOTAL_CONFIG_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageJsDraftStoreLimits {
    pub max_drafts: usize,
    pub max_config_bytes: usize,
    pub max_total_config_bytes: usize,
}

impl Default for PageJsDraftStoreLimits {
    fn default() -> Self {
        Self {
            max_drafts: DEFAULT_MAX_DRAFTS,
            max_config_bytes: DEFAULT_MAX_CONFIG_BYTES,
            max_total_config_bytes: DEFAULT_MAX_TOTAL_CONFIG_BYTES,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PageJsDraftStore {
    pub schema_version: u32,
    pub session_id: String,
    pub runtime_session_id: String,
    pub project_root: String,
    pub revision: u64,
    pub drafts: BTreeMap<String, PageJsDraftEntry>,
    limits: PageJsDraftStoreLimits,
    retained_config_bytes: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageJsDraftEntry {
    pub template_path: String,
    pub base: PageJsConfig,
    pub current: PageJsConfig,
    pub cachebust_assets: bool,
    pub source: String,
    pub coalesce_key: Option<String>,
    pub transaction_id: Option<String>,
    pub updated_at_ms: u128,
    pub revision: u64,
    pub base_config_bytes: usize,
    pub current_config_bytes: usize,
    pub retained_config_bytes: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageJsDraftStoreSnapshot {
    pub schema_version: u32,
    pub session_id: String,
    pub runtime_session_id: String,
    pub project_root: String,
    pub revision: u64,
    pub dirty_count: usize,
    pub retained_config_bytes: usize,
    pub limits: PageJsDraftStoreLimits,
    pub drafts: Vec<PageJsDraftEntry>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageJsDraftStageInput {
    pub template_path: String,
    pub expected_project_root: String,
    pub expected_session_id: String,
    pub base_config: PageJsConfig,
    pub current_config: PageJsConfig,
    // Kept in the staging contract for frontend compatibility and explicit Save policy.
    pub cachebust_assets: bool,
    pub source: Option<String>,
    pub coalesce_key: Option<String>,
    pub transaction_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PageJsDraftStageStatus {
    Staged,
    Cleared,
    Unchanged,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageJsDraftStageReceipt {
    pub schema_version: u32,
    pub status: PageJsDraftStageStatus,
    pub changed: bool,
    pub dirty: bool,
    pub template_path: String,
    pub revision: u64,
    pub entry_revision: Option<u64>,
    pub dirty_count: usize,
    pub retained_config_bytes: usize,
    pub project_root: String,
    pub runtime_session_id: String,
}

impl PageJsDraftStore {
    pub fn new(session: &ProjectSessionSnapshot) -> Self {
        Self::new_unchecked(session, PageJsDraftStoreLimits::default())
    }

    #[cfg(test)]
    pub fn with_limits(
        session: &ProjectSessionSnapshot,
        limits: PageJsDraftStoreLimits,
    ) -> Result<Self, String> {
        validate_limits(limits)?;
        Ok(Self::new_unchecked(session, limits))
    }

    fn new_unchecked(session: &ProjectSessionSnapshot, limits: PageJsDraftStoreLimits) -> Self {
        Self {
            schema_version: PAGE_JS_DRAFT_STORE_SCHEMA_VERSION,
            session_id: session.id.clone(),
            runtime_session_id: session.runtime_instance_id(),
            project_root: session.project_root.clone(),
            revision: 0,
            drafts: BTreeMap::new(),
            limits,
            retained_config_bytes: 0,
        }
    }

    pub fn dirty_count(&self) -> usize {
        self.drafts.len()
    }

    pub fn snapshot(&self) -> PageJsDraftStoreSnapshot {
        PageJsDraftStoreSnapshot {
            schema_version: self.schema_version,
            session_id: self.session_id.clone(),
            runtime_session_id: self.runtime_session_id.clone(),
            project_root: self.project_root.clone(),
            revision: self.revision,
            dirty_count: self.dirty_count(),
            retained_config_bytes: self.retained_config_bytes,
            limits: self.limits,
            drafts: self.drafts.values().cloned().collect(),
        }
    }

    pub fn stage(
        &mut self,
        input: PageJsDraftStageInput,
    ) -> Result<PageJsDraftStageReceipt, String> {
        let PageJsDraftStageInput {
            template_path,
            expected_project_root,
            expected_session_id,
            base_config,
            current_config,
            cachebust_assets,
            source,
            coalesce_key,
            transaction_id,
        } = input;
        self.require_identity(&expected_project_root, &expected_session_id)?;
        let template_path = normalize_template_path(&template_path)?;
        let source = normalize_source(source)?;
        let coalesce_key = normalize_optional_metadata(
            "coalesceKey",
            coalesce_key,
            PAGE_JS_DRAFT_MAX_COALESCE_KEY_BYTES,
        )?;
        let transaction_id = normalize_optional_metadata(
            "transactionId",
            transaction_id,
            PAGE_JS_DRAFT_MAX_TRANSACTION_ID_BYTES,
        )?;
        let base_config_bytes =
            serialized_config_bytes("baseConfig", &base_config, self.limits.max_config_bytes)?;
        let current_config_bytes = serialized_config_bytes(
            "currentConfig",
            &current_config,
            self.limits.max_config_bytes,
        )?;

        if current_config == base_config {
            return Ok(self.clear_internal(&template_path));
        }

        if let Some(entry) = self.drafts.get(&template_path) {
            if entry.base == base_config
                && entry.current == current_config
                && entry.cachebust_assets == cachebust_assets
                && entry.source == source
                && entry.coalesce_key == coalesce_key
                && entry.transaction_id == transaction_id
            {
                return Ok(self.receipt(
                    PageJsDraftStageStatus::Unchanged,
                    false,
                    true,
                    template_path,
                    Some(entry.revision),
                ));
            }
        } else if self.dirty_count() >= self.limits.max_drafts {
            return Err(format!(
                "PageJsDraftStore a refuzat draftul: limita de {} drafturi active a fost atinsă.",
                self.limits.max_drafts
            ));
        }

        let retained_config_bytes = base_config_bytes
            .checked_add(current_config_bytes)
            .ok_or_else(|| {
                "PageJsDraftStore a refuzat draftul: dimensiunea config-urilor a depășit usize."
                    .to_string()
            })?;
        let replaced_config_bytes = self
            .drafts
            .get(&template_path)
            .map(|entry| entry.retained_config_bytes)
            .unwrap_or_default();
        let next_total_config_bytes = self
            .retained_config_bytes
            .saturating_sub(replaced_config_bytes)
            .checked_add(retained_config_bytes)
            .ok_or_else(|| {
                "PageJsDraftStore a refuzat draftul: totalul config-urilor a depășit usize."
                    .to_string()
            })?;
        if next_total_config_bytes > self.limits.max_total_config_bytes {
            return Err(format!(
                "PageJsDraftStore a refuzat draftul: {} bytes reținuți ar depăși limita totală de {} bytes.",
                next_total_config_bytes, self.limits.max_total_config_bytes
            ));
        }

        self.revision = self.revision.saturating_add(1);
        let entry_revision = self.revision;
        let entry = PageJsDraftEntry {
            template_path: template_path.clone(),
            base: base_config,
            current: current_config,
            cachebust_assets,
            source,
            coalesce_key,
            transaction_id,
            updated_at_ms: now_ms(),
            revision: entry_revision,
            base_config_bytes,
            current_config_bytes,
            retained_config_bytes,
        };
        self.drafts.insert(template_path.clone(), entry);
        self.retained_config_bytes = next_total_config_bytes;

        Ok(self.receipt(
            PageJsDraftStageStatus::Staged,
            true,
            true,
            template_path,
            Some(entry_revision),
        ))
    }

    pub fn clear(
        &mut self,
        template_path: &str,
        expected_revision: Option<u64>,
    ) -> Result<PageJsDraftStageReceipt, String> {
        let template_path = normalize_template_path(template_path)?;
        if let (Some(expected), Some(entry)) = (expected_revision, self.drafts.get(&template_path))
        {
            if entry.revision != expected {
                return Ok(self.receipt(
                    PageJsDraftStageStatus::Unchanged,
                    false,
                    true,
                    template_path,
                    Some(entry.revision),
                ));
            }
        }
        Ok(self.clear_internal(&template_path))
    }

    pub fn require_identity(
        &self,
        expected_project_root: &str,
        expected_session_id: &str,
    ) -> Result<(), String> {
        if expected_project_root != self.project_root
            || expected_session_id != self.runtime_session_id
        {
            return Err(format!(
                "PageJsDraftStore a refuzat un task stale: așteptat root/session {expected_project_root}/{expected_session_id}, activ {}/{}.",
                self.project_root, self.runtime_session_id
            ));
        }
        Ok(())
    }

    fn clear_internal(&mut self, template_path: &str) -> PageJsDraftStageReceipt {
        let removed = self.drafts.remove(template_path);
        let changed = removed.is_some();
        if let Some(entry) = removed {
            self.retained_config_bytes = self
                .retained_config_bytes
                .saturating_sub(entry.retained_config_bytes);
            self.revision = self.revision.saturating_add(1);
        }
        self.receipt(
            if changed {
                PageJsDraftStageStatus::Cleared
            } else {
                PageJsDraftStageStatus::Unchanged
            },
            changed,
            false,
            template_path.to_string(),
            None,
        )
    }

    fn receipt(
        &self,
        status: PageJsDraftStageStatus,
        changed: bool,
        dirty: bool,
        template_path: String,
        entry_revision: Option<u64>,
    ) -> PageJsDraftStageReceipt {
        PageJsDraftStageReceipt {
            schema_version: PAGE_JS_DRAFT_STORE_SCHEMA_VERSION,
            status,
            changed,
            dirty,
            template_path,
            revision: self.revision,
            entry_revision,
            dirty_count: self.dirty_count(),
            retained_config_bytes: self.retained_config_bytes,
            project_root: self.project_root.clone(),
            runtime_session_id: self.runtime_session_id.clone(),
        }
    }
}

#[cfg(test)]
fn validate_limits(limits: PageJsDraftStoreLimits) -> Result<(), String> {
    if limits.max_drafts == 0 || limits.max_config_bytes == 0 || limits.max_total_config_bytes == 0
    {
        return Err(
            "PageJsDraftStore cere limite nenule pentru drafturi și bytes de config.".to_string(),
        );
    }
    Ok(())
}

fn serialized_config_bytes(
    label: &str,
    config: &PageJsConfig,
    max_config_bytes: usize,
) -> Result<usize, String> {
    let bytes = serde_json::to_vec(config)
        .map_err(|error| format!("PageJsDraftStore nu a putut serializa {label}: {error}"))?
        .len();
    if bytes > max_config_bytes {
        return Err(format!(
            "PageJsDraftStore a refuzat {label}: {bytes} bytes depășesc limita de {max_config_bytes} bytes."
        ));
    }
    Ok(bytes)
}

fn normalize_template_path(path: &str) -> Result<String, String> {
    let normalized = super::paths::normalize_template_path(path)?;
    if normalized.len() > PAGE_JS_DRAFT_MAX_TEMPLATE_PATH_BYTES {
        return Err(format!(
            "PageJsDraftStore a refuzat template path: {} bytes depășesc limita de {} bytes.",
            normalized.len(),
            PAGE_JS_DRAFT_MAX_TEMPLATE_PATH_BYTES
        ));
    }
    Ok(normalized)
}

fn normalize_source(source: Option<String>) -> Result<String, String> {
    let source = source
        .unwrap_or_else(|| "page_js.draft".to_string())
        .trim()
        .to_string();
    let source = if source.is_empty() {
        "page_js.draft".to_string()
    } else {
        source
    };
    if source.contains('\0') {
        return Err("PageJsDraftStore a refuzat source cu caracter nul.".to_string());
    }
    Ok(truncate_utf8_bytes(&source, PAGE_JS_DRAFT_MAX_SOURCE_BYTES))
}

fn normalize_optional_metadata(
    label: &str,
    value: Option<String>,
    max_bytes: usize,
) -> Result<Option<String>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.contains('\0') {
        return Err(format!(
            "PageJsDraftStore a refuzat {label} cu caracter nul."
        ));
    }
    if value.len() > max_bytes {
        return Err(format!(
            "PageJsDraftStore a refuzat {label}: {} bytes depășesc limita de {max_bytes} bytes.",
            value.len()
        ));
    }
    Ok(Some(value.to_string()))
}

fn truncate_utf8_bytes(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}

#[cfg(test)]
mod tests {
    use crate::js::PanaComponent;

    use super::*;

    fn session_at(opened_at_ms: u128) -> ProjectSessionSnapshot {
        ProjectSessionSnapshot {
            schema_version: 1,
            id: "session-test".to_string(),
            project_root: "/tmp/pana-test".to_string(),
            zola_root: "/tmp/pana-test/sursa".to_string(),
            session_dir: "/tmp/pana-test-session".to_string(),
            manifest_path: "/tmp/pana-test-session/session.json".to_string(),
            opened_at_ms,
            last_seen_at_ms: 1,
            root_fingerprint: crate::kernel::project_session::ProjectRootFingerprint {
                canonical_path: "/tmp/pana-test".to_string(),
                modified_ms: 1,
                size: 0,
                readonly: false,
                unix_device: None,
                unix_inode: None,
            },
            scan_summary: crate::kernel::project_session::ProjectSessionScanSummary {
                is_zola: true,
                is_empty: false,
                active_theme: None,
                file_count: 0,
                directory_count: 0,
            },
        }
    }

    fn session() -> ProjectSessionSnapshot {
        session_at(1)
    }

    fn dirty_config(id: &str) -> PageJsConfig {
        PageJsConfig {
            version: Some(1),
            components: vec![PanaComponent { id: id.to_string() }],
            motion: None,
        }
    }

    fn stage_input(template_path: &str, id: &str) -> PageJsDraftStageInput {
        let session = session();
        PageJsDraftStageInput {
            template_path: template_path.to_string(),
            expected_project_root: session.project_root.clone(),
            expected_session_id: session.runtime_instance_id(),
            base_config: PageJsConfig::default(),
            current_config: dirty_config(id),
            cachebust_assets: false,
            source: Some("test".to_string()),
            coalesce_key: Some("motion.timeline.stepTiming".to_string()),
            transaction_id: Some("tx-1".to_string()),
        }
    }

    #[test]
    fn stages_only_draft_state_without_save_plan() {
        let mut store = PageJsDraftStore::new(&session());
        let receipt = store
            .stage(stage_input("templates/index.html", "tabs"))
            .unwrap();

        assert_eq!(receipt.status, PageJsDraftStageStatus::Staged);
        assert!(receipt.dirty);
        assert_eq!(receipt.dirty_count, 1);
        let snapshot = store.snapshot();
        assert_eq!(snapshot.dirty_count, 1);
        assert_eq!(snapshot.drafts[0].template_path, "templates/index.html");
        assert_eq!(
            snapshot.drafts[0].coalesce_key.as_deref(),
            Some("motion.timeline.stepTiming")
        );
        let serialized = serde_json::to_value(snapshot).unwrap();
        assert!(serialized["drafts"][0].get("plan").is_none());
    }

    #[test]
    fn clears_existing_draft_when_config_matches_base() {
        let mut store = PageJsDraftStore::new(&session());
        store
            .stage(stage_input("templates/index.html", "tabs"))
            .unwrap();
        let receipt = store
            .stage(PageJsDraftStageInput {
                template_path: "templates/index.html".to_string(),
                expected_project_root: session().project_root,
                expected_session_id: session().runtime_instance_id(),
                base_config: PageJsConfig::default(),
                current_config: PageJsConfig::default(),
                cachebust_assets: false,
                source: None,
                coalesce_key: None,
                transaction_id: None,
            })
            .unwrap();

        assert_eq!(receipt.status, PageJsDraftStageStatus::Cleared);
        assert_eq!(receipt.dirty_count, 0);
        assert_eq!(store.dirty_count(), 0);
        assert_eq!(store.snapshot().retained_config_bytes, 0);
    }

    #[test]
    fn duplicate_stage_is_a_revision_preserving_no_op() {
        let mut store = PageJsDraftStore::new(&session());
        let first = store
            .stage(stage_input("templates/index.html", "tabs"))
            .unwrap();
        let duplicate = store
            .stage(stage_input("templates/index.html", "tabs"))
            .unwrap();

        assert_eq!(duplicate.status, PageJsDraftStageStatus::Unchanged);
        assert!(!duplicate.changed);
        assert!(duplicate.dirty);
        assert_eq!(duplicate.revision, first.revision);
        assert_eq!(duplicate.entry_revision, first.entry_revision);
        assert_eq!(store.dirty_count(), 1);
    }

    #[test]
    fn revision_guard_does_not_clear_a_newer_draft() {
        let mut store = PageJsDraftStore::new(&session());
        let first = store
            .stage(stage_input("templates/index.html", "tabs"))
            .unwrap();
        let second = store
            .stage(stage_input("templates/index.html", "accordion"))
            .unwrap();

        let stale_clear = store
            .clear("templates/index.html", first.entry_revision)
            .unwrap();
        assert_eq!(stale_clear.status, PageJsDraftStageStatus::Unchanged);
        assert!(!stale_clear.changed);
        assert!(stale_clear.dirty);
        assert_eq!(stale_clear.entry_revision, second.entry_revision);
        assert_eq!(
            store.snapshot().drafts[0].current.components[0].id,
            "accordion"
        );

        let current_clear = store
            .clear("templates/index.html", second.entry_revision)
            .unwrap();
        assert_eq!(current_clear.status, PageJsDraftStageStatus::Cleared);
        assert_eq!(store.dirty_count(), 0);
    }

    #[test]
    fn same_template_keeps_only_latest_config() {
        let mut store = PageJsDraftStore::new(&session());
        for id in ["tabs", "accordion"] {
            let mut input = stage_input("templates/index.html", id);
            input.transaction_id = None;
            store.stage(input).unwrap();
        }

        let snapshot = store.snapshot();
        assert_eq!(snapshot.dirty_count, 1);
        assert_eq!(snapshot.revision, 2);
        assert_eq!(snapshot.drafts[0].current.components[0].id, "accordion");
    }

    #[test]
    fn rejects_config_over_resource_limit_without_mutating_store() {
        let limits = PageJsDraftStoreLimits {
            max_drafts: 4,
            max_config_bytes: 128,
            max_total_config_bytes: 1024,
        };
        let mut store = PageJsDraftStore::with_limits(&session(), limits).unwrap();
        let error = store
            .stage(stage_input(
                "templates/index.html",
                &"component".repeat(128),
            ))
            .unwrap_err();

        assert!(error.contains("currentConfig"));
        assert!(error.contains("limita de 128 bytes"));
        assert_eq!(store.revision, 0);
        assert_eq!(store.dirty_count(), 0);
    }

    #[test]
    fn rejects_new_template_after_draft_count_limit() {
        let limits = PageJsDraftStoreLimits {
            max_drafts: 1,
            max_config_bytes: 1024,
            max_total_config_bytes: 4096,
        };
        let mut store = PageJsDraftStore::with_limits(&session(), limits).unwrap();
        store
            .stage(stage_input("templates/first.html", "tabs"))
            .unwrap();
        let error = store
            .stage(stage_input("templates/second.html", "accordion"))
            .unwrap_err();

        assert!(error.contains("limita de 1 drafturi"));
        assert_eq!(store.revision, 1);
        assert_eq!(store.dirty_count(), 1);
        assert_eq!(
            store.snapshot().drafts[0].template_path,
            "templates/first.html"
        );
    }

    #[test]
    fn rejects_task_from_another_project_session_without_mutating_store() {
        let mut store = PageJsDraftStore::new(&session());
        let mut input = stage_input("templates/index.html", "tabs");
        input.expected_project_root = "/tmp/another-project".to_string();

        let error = store.stage(input).unwrap_err();
        assert!(error.contains("task stale"));
        assert_eq!(store.revision, 0);
        assert_eq!(store.dirty_count(), 0);
    }

    #[test]
    fn rejects_task_from_previous_runtime_of_same_project_without_mutating_store() {
        let previous = session_at(1);
        let current = session_at(2);
        assert_eq!(previous.id, current.id);
        assert_eq!(previous.project_root, current.project_root);
        assert_ne!(
            previous.runtime_instance_id(),
            current.runtime_instance_id()
        );

        let mut store = PageJsDraftStore::new(&current);
        let mut input = stage_input("templates/index.html", "tabs");
        input.expected_session_id = previous.runtime_instance_id();

        let error = store.stage(input).unwrap_err();
        assert!(error.contains("task stale"));
        assert_eq!(store.revision, 0);
        assert_eq!(store.dirty_count(), 0);
    }

    #[test]
    fn stage_receipt_is_compact_and_contains_no_draft_payload() {
        let mut store = PageJsDraftStore::new(&session());
        let receipt = store
            .stage(stage_input("templates/index.html", "tabs"))
            .unwrap();
        let serialized = serde_json::to_string(&receipt).unwrap();

        assert!(serialized.len() < 512);
        assert_eq!(receipt.project_root, session().project_root);
        assert_eq!(receipt.runtime_session_id, session().runtime_instance_id());
        for forbidden in [
            "\"entry\"",
            "\"snapshot\"",
            "\"base\"",
            "\"current\"",
            "\"plan\"",
        ] {
            assert!(
                !serialized.contains(forbidden),
                "receipt contains {forbidden}"
            );
        }
    }
}
