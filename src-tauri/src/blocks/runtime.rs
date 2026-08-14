use std::collections::HashSet;

use serde::Serialize;

pub(crate) const NATIVE_BLOCK_RUNTIME_CORE_SCRIPT: &str = include_str!("runtime.js");

#[derive(Clone, Copy)]
struct NativeBlockRuntimeProvider {
    id: &'static str,
    script: &'static str,
}

const NATIVE_BLOCK_RUNTIME_PROVIDERS: &[NativeBlockRuntimeProvider] = &[
    NativeBlockRuntimeProvider {
        id: "counter",
        script: include_str!("runtime/counter.js"),
    },
    NativeBlockRuntimeProvider {
        id: "accordion",
        script: include_str!("runtime/accordion.js"),
    },
    NativeBlockRuntimeProvider {
        id: "tabs",
        script: include_str!("runtime/tabs.js"),
    },
    NativeBlockRuntimeProvider {
        id: "slider",
        script: include_str!("runtime/slider.js"),
    },
    NativeBlockRuntimeProvider {
        id: "dialog",
        script: include_str!("runtime/dialog.js"),
    },
    NativeBlockRuntimeProvider {
        id: "offcanvas",
        script: include_str!("runtime/offcanvas.js"),
    },
    NativeBlockRuntimeProvider {
        id: "nav-menu",
        script: include_str!("runtime/nav_menu.js"),
    },
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeBlockRuntimePlan {
    provider_ids: Vec<&'static str>,
}

impl NativeBlockRuntimePlan {
    pub(crate) fn from_template_source(source: &str) -> Self {
        let mut diagnostics = Vec::new();
        let requested = super::contract::block_ids_in_template_source(source, &mut diagnostics);
        let provider_ids = NATIVE_BLOCK_RUNTIME_PROVIDERS
            .iter()
            .filter(|provider| requested.contains(provider.id))
            .map(|provider| provider.id)
            .collect();
        Self { provider_ids }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.provider_ids.is_empty()
    }

    pub(crate) fn provider_ids(&self) -> &[&'static str] {
        &self.provider_ids
    }
}

#[derive(Serialize)]
struct NativeBlockRuntimeConfig<'a> {
    blocks: Vec<NativeBlockRuntimeConfigEntry<'a>>,
}

#[derive(Serialize)]
struct NativeBlockRuntimeConfigEntry<'a> {
    id: &'a str,
}

pub(crate) fn render_native_block_runtime(plan: &NativeBlockRuntimePlan) -> String {
    if plan.is_empty() {
        return String::new();
    }

    let selected = plan.provider_ids.iter().copied().collect::<HashSet<_>>();
    let config = NativeBlockRuntimeConfig {
        blocks: plan
            .provider_ids
            .iter()
            .map(|id| NativeBlockRuntimeConfigEntry { id })
            .collect(),
    };
    let encoded = serde_json::to_string(&config).unwrap_or_else(|_| r#"{"blocks":[]}"#.to_string());
    let mut output = String::from("(function(){\n");
    output.push_str(NATIVE_BLOCK_RUNTIME_CORE_SCRIPT);
    output.push('\n');
    for provider in NATIVE_BLOCK_RUNTIME_PROVIDERS {
        if selected.contains(provider.id) {
            output.push_str(provider.script);
            output.push('\n');
        }
    }
    output.push_str("window.PanaBlockRuntime.installPageConfig(");
    output.push_str(&encoded);
    output.push_str(");\n})();");
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::native::{native_block_provider_definitions, NativeBlockKind};

    fn template(ids: &[&str]) -> String {
        ids.iter()
            .map(|id| format!(r#"<div data-pana-block="{id}"></div>"#))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn provider_fragments_follow_the_native_registry_order_exactly() {
        let registry_ids = native_block_provider_definitions()
            .iter()
            .filter(|definition| definition.kind == NativeBlockKind::Js)
            .map(|definition| definition.id)
            .collect::<Vec<_>>();
        let runtime_ids = NATIVE_BLOCK_RUNTIME_PROVIDERS
            .iter()
            .map(|provider| provider.id)
            .collect::<Vec<_>>();

        assert_eq!(runtime_ids, registry_ids);
    }

    #[test]
    fn motion_only_plan_has_no_block_runtime() {
        let plan = NativeBlockRuntimePlan::from_template_source("<main></main>");

        assert!(plan.is_empty());
        assert!(render_native_block_runtime(&plan).is_empty());
    }

    #[test]
    fn plan_deduplicates_filters_and_orders_providers_by_registry() {
        let plan = NativeBlockRuntimePlan::from_template_source(&template(&[
            "slider",
            "accordion",
            "slider",
            "unknown",
        ]));

        assert_eq!(plan.provider_ids(), &["accordion", "slider"]);
        let script = render_native_block_runtime(&plan);
        assert_eq!(script.matches("PANA BLOCK RUNTIME CORE").count(), 1);
        assert_eq!(script.matches("PANA BLOCK PROVIDER: accordion").count(), 1);
        assert_eq!(script.matches("PANA BLOCK PROVIDER: slider").count(), 1);
        assert!(!script.contains("PANA BLOCK PROVIDER: counter"));
        assert!(!script.contains("PANA BLOCK PROVIDER: tabs"));
        assert!(!script.contains("__panaMotionV2Config"));
        assert!(!script.contains("\"motion\""));
    }

    #[test]
    fn plan_reads_real_html_attributes_not_comments_or_script_text() {
        let plan = NativeBlockRuntimePlan::from_template_source(
            r#"<!-- <div data-pana-block="slider"></div> -->
<script>var example = '<div data-pana-block="tabs"></div>';</script>
<section data-pana-block="accordion"></section>"#,
        );

        assert_eq!(plan.provider_ids(), &["accordion"]);
    }

    #[test]
    fn every_provider_fragment_is_independently_selectable() {
        for provider in NATIVE_BLOCK_RUNTIME_PROVIDERS {
            let plan = NativeBlockRuntimePlan::from_template_source(&template(&[provider.id]));
            let script = render_native_block_runtime(&plan);
            assert_eq!(plan.provider_ids(), &[provider.id]);
            assert_eq!(
                script
                    .matches(&format!("PANA BLOCK PROVIDER: {}", provider.id))
                    .count(),
                1
            );
            assert_eq!(script.matches("PANA BLOCK PROVIDER:").count(), 1);
            assert!(script.contains(&format!(r#""id":"{}""#, provider.id)));
        }
    }
}
