use std::time::Instant;

use crate::project_model::ProjectModelIncrementalBuildReport;

use super::observability::{KernelEventKind, KernelLogEvent, KernelLogLevel};

pub(crate) const PERFORMANCE_SAMPLE_SCHEMA_VERSION: u32 = 3;

#[derive(Clone, Debug)]
pub(crate) struct ProjectModelPerformanceSample {
    pub(crate) build_mode: String,
    pub(crate) fallback_reason: Option<String>,
    pub(crate) duration_us: u64,
    pub(crate) clone_us: u64,
    pub(crate) template_parse_us: u64,
    pub(crate) component_graph_us: u64,
    pub(crate) block_graph_us: u64,
    pub(crate) content_model_us: u64,
    pub(crate) listing_items_us: u64,
    pub(crate) listing_items_reused: bool,
    pub(crate) dynamic_widget_us: u64,
    pub(crate) markdown_us: u64,
    pub(crate) node_index_us: u64,
    pub(crate) changed_path_count: usize,
    pub(crate) reused_nodes: usize,
    pub(crate) replaced_nodes: usize,
}

impl From<&ProjectModelIncrementalBuildReport> for ProjectModelPerformanceSample {
    fn from(report: &ProjectModelIncrementalBuildReport) -> Self {
        Self {
            build_mode: report.mode.label().to_string(),
            fallback_reason: report.fallback_reason.clone(),
            duration_us: report.duration_us,
            clone_us: report.model_clone_us,
            template_parse_us: report.template_parse_us,
            component_graph_us: report.component_graph_us,
            block_graph_us: report.block_graph_us,
            content_model_us: report.content_model_us,
            listing_items_us: report.listing_items_us,
            listing_items_reused: report.listing_items_reused,
            dynamic_widget_us: report.dynamic_widget_us,
            markdown_us: report.markdown_us,
            node_index_us: report.node_index_us,
            changed_path_count: report.changed_paths.len(),
            reused_nodes: report.reused_nodes,
            replaced_nodes: report.replaced_nodes,
        }
    }
}

pub(crate) fn performance_event(
    owner: &str,
    category: &str,
    operation: &str,
    variant: &str,
    target: Option<String>,
    total_us: u64,
) -> KernelLogEvent {
    let event = KernelLogEvent::new(
        KernelLogLevel::Info,
        KernelEventKind::PerformanceSampled,
        owner,
        category,
        operation,
        target,
        "Performance sample captured by the canonical Rust operation.",
        None,
    );
    with_performance_sample(event, operation, variant, total_us)
}

pub(crate) fn with_performance_sample(
    event: KernelLogEvent,
    operation: &str,
    variant: &str,
    total_us: u64,
) -> KernelLogEvent {
    event
        .with_attribute(
            "performanceSchemaVersion",
            PERFORMANCE_SAMPLE_SCHEMA_VERSION,
        )
        .with_attribute("performanceOperation", operation)
        .with_attribute("performanceVariant", variant)
        .with_attribute("performanceTotalUs", total_us)
}

pub(crate) fn with_project_model_sample(
    mut event: KernelLogEvent,
    sample: Option<&ProjectModelPerformanceSample>,
) -> KernelLogEvent {
    let Some(sample) = sample else {
        return event;
    };
    event = event
        .with_attribute("projectModelBuildMode", &sample.build_mode)
        .with_attribute("projectModelFallbackReason", sample.fallback_reason.clone())
        .with_attribute("projectModelBuildUs", sample.duration_us)
        .with_attribute("projectModelCloneUs", sample.clone_us)
        .with_attribute("projectModelTemplateParseUs", sample.template_parse_us)
        .with_attribute("projectModelComponentGraphUs", sample.component_graph_us)
        .with_attribute("projectModelBlockGraphUs", sample.block_graph_us)
        .with_attribute("projectModelContentModelUs", sample.content_model_us)
        .with_attribute("projectModelListingItemsUs", sample.listing_items_us)
        .with_attribute(
            "projectModelListingItemsReused",
            sample.listing_items_reused,
        )
        .with_attribute("projectModelDynamicWidgetUs", sample.dynamic_widget_us)
        .with_attribute("projectModelMarkdownUs", sample.markdown_us)
        .with_attribute("projectModelNodeIndexUs", sample.node_index_us)
        .with_attribute("projectModelChangedPathCount", sample.changed_path_count)
        .with_attribute("projectModelReusedNodes", sample.reused_nodes)
        .with_attribute("projectModelReplacedNodes", sample.replaced_nodes);
    event
}

pub(crate) fn project_model_performance_event(
    owner: &str,
    target: Option<String>,
    sample: &ProjectModelPerformanceSample,
) -> KernelLogEvent {
    with_project_model_sample(
        performance_event(
            owner,
            "performance",
            "project_model_build",
            &sample.build_mode,
            target,
            sample.duration_us,
        ),
        Some(sample),
    )
}

pub(crate) fn elapsed_us(started: Instant) -> u64 {
    started.elapsed().as_micros().min(u64::MAX as u128) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn performance_event_uses_one_machine_readable_schema() {
        let event = performance_event(
            "project_workspace",
            "performance",
            "html_edit",
            "attributes",
            Some("session-a".to_string()),
            1_250,
        );
        assert_eq!(event.kind, KernelEventKind::PerformanceSampled);
        assert_eq!(event.attributes["performanceSchemaVersion"], 3);
        assert_eq!(event.attributes["performanceOperation"], "html_edit");
        assert_eq!(event.attributes["performanceTotalUs"], 1_250);
    }

    #[test]
    fn project_model_event_is_a_dedicated_operation() {
        let sample = ProjectModelPerformanceSample {
            build_mode: "incremental".to_string(),
            fallback_reason: None,
            duration_us: 90,
            clone_us: 20,
            template_parse_us: 30,
            component_graph_us: 10,
            block_graph_us: 10,
            content_model_us: 5,
            listing_items_us: 0,
            listing_items_reused: true,
            dynamic_widget_us: 5,
            markdown_us: 5,
            node_index_us: 5,
            changed_path_count: 1,
            reused_nodes: 12,
            replaced_nodes: 2,
        };
        let event = project_model_performance_event("project_workspace", None, &sample);
        assert_eq!(
            event.attributes["performanceOperation"],
            "project_model_build"
        );
        assert_eq!(event.attributes["projectModelBuildMode"], "incremental");
    }
}
