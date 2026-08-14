use crate::blocks::{render_native_block_runtime, NativeBlockRuntimePlan};
use std::collections::BTreeSet;

use super::motion::{generate_motion_js, MotionExecutionPlan};
use super::types::PageJsConfig;

#[derive(Clone, Debug, PartialEq)]
pub struct PageRuntimePlan {
    block_runtime: NativeBlockRuntimePlan,
    motion: Option<MotionExecutionPlan>,
}

impl PageRuntimePlan {
    pub fn from_sources(template_source: &str, config: &PageJsConfig) -> Self {
        Self {
            block_runtime: NativeBlockRuntimePlan::from_template_source(template_source),
            motion: MotionExecutionPlan::from_editor_config(config),
        }
    }

    pub fn has_runtime(&self) -> bool {
        !self.block_runtime.is_empty() || self.motion.is_some()
    }

    pub fn has_motion(&self) -> bool {
        self.motion.is_some()
    }

    pub fn anime_entry_modules(&self) -> BTreeSet<&'static str> {
        self.motion
            .as_ref()
            .map(MotionExecutionPlan::features)
            .map(super::motion_compiler::MotionFeatureSet::anime_entry_modules)
            .unwrap_or_default()
    }
}

pub fn generate_page_js(plan: &PageRuntimePlan) -> String {
    let mut out = String::new();
    let block_runtime = render_native_block_runtime(&plan.block_runtime);
    if !block_runtime.is_empty() {
        out.push_str(&block_runtime);
        out.push('\n');
    }
    out.push_str("\n(function () {\n");
    out.push_str("  var _panaStarted=false;\n");
    out.push_str("  function _panaRun(){if(_panaStarted)return;_panaStarted=true;\n");

    let motion_js = generate_motion_js(plan.motion.as_ref());
    if !motion_js.is_empty() {
        out.push_str(&motion_js);
        out.push_str("\n\n");
    }

    out.push_str("  }\n");
    out.push_str("  if (document.readyState === \"loading\") document.addEventListener(\"DOMContentLoaded\", _panaRun, { once: true }); else _panaRun();\n");
    out.push_str("})();\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::js::{MotionDocument, MotionRuntimeContract};

    fn generate(template: &str, config: &PageJsConfig) -> String {
        generate_page_js(&PageRuntimePlan::from_sources(template, config))
    }

    #[test]
    fn generated_page_js_runs_after_dom_is_already_ready() {
        let js = generate("<main></main>", &PageJsConfig::default());
        assert!(js.contains("document.readyState === \"loading\""));
        assert!(js.contains("else _panaRun();"));
    }

    #[test]
    fn generated_page_js_has_runtime_but_no_editor_metadata() {
        let config = PageJsConfig {
            motion: Some(
                MotionDocument::from_value(serde_json::json!({
                    "schemaVersion": 2,
                    "animeVersion": MotionRuntimeContract::current().anime_version,
                    "interactions": [{
                        "id": "animation-a",
                        "name": "Hero",
                        "trigger": { "type": "load" },
                        "triggerTarget": { "kind": "element", "dataAnim": "hero" },
                        "actions": [{
                            "type": "animate",
                            "id": "fade",
                            "name": "Fade",
                            "target": { "kind": "element", "dataAnim": "hero" },
                            "properties": [{
                                "id": "opacity",
                                "name": "opacity",
                                "category": "style",
                                "from": { "kind": "number", "value": "0" },
                                "to": { "kind": "number", "value": "1" }
                            }]
                        }]
                    }]
                }))
                .expect("strict Motion document"),
            ),
        };
        let js = generate("<h1 data-anim=\"hero\">Hero</h1>", &config);
        assert!(!js.contains("@pana-motion"));
        assert!(js.contains("import(\"/js/vendor/animejs-4.4.1/timeline/index.js\")"));
        assert!(js.contains("\"type\":\"animate\""));
        assert!(!js.contains("PANA MOTION RUNTIME"));
        assert!(!js.contains("PanaBlockRuntime"));
        assert!(!js.contains("PANA BLOCK RUNTIME CORE"));
        assert!(!js.contains("PANA BLOCK PROVIDER:"));
        assert!(!js.contains("__panaMotionV2Config"));
        assert!(!js.contains("schemaVersion"));
        assert!(!js.contains("animeVersion"));
        assert!(!js.contains("Hero"));
        assert!(!js.contains("Fade"));
        assert!(!js.contains("PanaMotionRuntime"));
        assert!(!js.contains("__pana_motion_mode"));
        assert!(js.len() < 14 * 1024);
    }

    #[test]
    fn disabled_last_interaction_does_not_keep_public_motion_assets_alive() {
        let config = PageJsConfig {
            motion: Some(
                MotionDocument::from_value(serde_json::json!({
                    "schemaVersion": 2,
                    "animeVersion": MotionRuntimeContract::current().anime_version,
                    "interactions": [{
                        "id": "disabled",
                        "enabled": false,
                        "trigger": { "type": "load" },
                        "triggerTarget": { "kind": "document" },
                        "actions": [{
                            "type": "animate",
                            "id": "fade",
                            "target": { "kind": "document" },
                            "properties": [{
                                "id": "opacity",
                                "name": "opacity",
                                "category": "style",
                                "from": { "kind": "number", "value": "0" },
                                "to": { "kind": "number", "value": "1" }
                            }]
                        }]
                    }]
                }))
                .unwrap(),
            ),
        };
        let plan = PageRuntimePlan::from_sources("<main></main>", &config);
        assert!(!plan.has_motion());
        assert!(!plan.has_runtime());
        assert!(plan.anime_entry_modules().is_empty());
    }

    #[test]
    fn generated_page_js_uses_the_canonical_block_runtime() {
        let js = generate(
            r#"<section data-pana-block="accordion"></section>"#,
            &PageJsConfig::default(),
        );

        assert!(js.contains("window.PanaBlockRuntime"));
        assert!(js.contains("installPageConfig"));
        assert!(!js.contains("_panaMotionPreview"));
        assert!(js.contains("\"id\":\"accordion\""));
        assert!(!js.contains("@pana-block"));
        assert!(!js.contains("// @pana-component"));
        assert!(!js.contains("window.PanaInteractiveRuntime"));
        assert_eq!(js.matches("register(\"accordion\"").count(), 1);
        assert_eq!(js.matches("PANA BLOCK PROVIDER:").count(), 1);
        assert!(!js.contains("PANA BLOCK PROVIDER: slider"));
        assert!(!js.contains("__panaMotionV2Config"));
    }

    #[test]
    fn canonical_runtime_has_accessible_lifecycle_and_cleanup() {
        let js = generate(
            concat!(
                r#"<span data-pana-block="counter"></span>"#,
                r#"<div data-pana-block="tabs"></div>"#,
                r#"<dialog data-pana-block="dialog"></dialog>"#,
                r#"<aside data-pana-block="offcanvas"></aside>"#,
                r#"<nav data-pana-block="nav-menu"></nav>"#,
            ),
            &PageJsConfig::default(),
        );

        assert!(js.contains("IntersectionObserver"));
        assert!(js.contains("cancelAnimationFrame"));
        assert!(js.contains("removeEventListener"));
        assert!(js.contains("media.removeListener"));
        assert!(js.contains("aria-controls"));
        assert!(js.contains("aria-expanded"));
        assert!(js.contains("aria-modal"));
        assert!(js.contains("ArrowRight"));
        assert!(js.contains("Escape"));
    }
}
