use std::sync::Mutex;

use crate::kernel::ai_coordination::AiCoordinationSnapshot;

use super::model::{
    CanonicalAiContextSnapshot, ContextHubError, ContextHubPublication, ContextHubPublishReceipt,
    CONTEXT_HUB_SCHEMA_VERSION,
};

#[derive(Clone, Debug)]
struct PublishedContext {
    project_session_id: Option<String>,
    context_revision: u64,
    updated_at_ms: u128,
    ui_revision_seen: u64,
    core: super::model::AiContextCore,
}

#[derive(Default)]
struct ContextHubState {
    published: Option<PublishedContext>,
    next_context_revision: u64,
}

#[derive(Default)]
pub struct ContextHubRuntime {
    state: Mutex<ContextHubState>,
}

impl ContextHubRuntime {
    pub fn publish(
        &self,
        publication: ContextHubPublication,
        now_ms: u128,
    ) -> Result<ContextHubPublishReceipt, ContextHubError> {
        self.with_state(|state| {
            if let Some(current) = state.published.as_mut() {
                if current.project_session_id == publication.project_session_id {
                    if publication.ui_revision < current.ui_revision_seen {
                        return Err(ContextHubError::new(format!(
                            "Proiecția UI revizia {} este mai veche decât revizia {} acceptată de Context Hub.",
                            publication.ui_revision, current.ui_revision_seen
                        )));
                    }
                    if publication.ui_revision == current.ui_revision_seen
                        && current.core != publication.core
                    {
                        return Err(ContextHubError::new(
                            "Aceeași revizie UI nu poate publica două contexte semantice diferite.",
                        ));
                    }
                }
            }

            let changed = state
                .published
                .as_ref()
                .is_none_or(|current| current.core != publication.core);
            if changed {
                state.next_context_revision = state.next_context_revision.saturating_add(1).max(1);
                state.published = Some(PublishedContext {
                    project_session_id: publication.project_session_id,
                    context_revision: state.next_context_revision,
                    updated_at_ms: now_ms,
                    ui_revision_seen: publication.ui_revision,
                    core: publication.core,
                });
            } else if let Some(current) = state.published.as_mut() {
                current.ui_revision_seen = current.ui_revision_seen.max(publication.ui_revision);
            }

            let current = state.published.as_ref().ok_or_else(|| {
                ContextHubError::new("Context Hub nu a putut publica snapshotul curent.")
            })?;
            Ok(ContextHubPublishReceipt {
                changed,
                context_revision: current.context_revision,
                ui_revision_seen: current.ui_revision_seen,
                updated_at_ms: current.updated_at_ms,
            })
        })
    }

    pub fn snapshot(
        &self,
        coordination: AiCoordinationSnapshot,
    ) -> Result<Option<CanonicalAiContextSnapshot>, ContextHubError> {
        self.with_state(|state| {
            Ok(state
                .published
                .as_ref()
                .map(|published| CanonicalAiContextSnapshot {
                    version: CONTEXT_HUB_SCHEMA_VERSION,
                    context_revision: published.context_revision,
                    updated_at_ms: published.updated_at_ms,
                    ui_revision_seen: published.ui_revision_seen,
                    core: published.core.clone(),
                    coordination,
                }))
        })
    }

    fn with_state<T>(
        &self,
        operation: impl FnOnce(&mut ContextHubState) -> Result<T, ContextHubError>,
    ) -> Result<T, ContextHubError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ContextHubError::new("ContextHubRuntime mutex este compromis."))?;
        operation(&mut state)
    }
}
