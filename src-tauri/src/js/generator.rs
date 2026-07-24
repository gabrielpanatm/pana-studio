use crate::blocks::install_native_block_runtime;

use super::motion::generate_motion_js;
use super::types::PageJsConfig;

pub fn generate_page_js(config: &PageJsConfig) -> String {
    let mut out = block_metadata_comments(config);
    out.push_str(&install_native_block_runtime(config));
    out.push_str("\n(function () {\n");
    if let Some(metadata) = motion_metadata_comment(config) {
        out.push_str(&metadata);
        out.push('\n');
    }
    out.push_str("  var _panaStarted=false;\n");
    out.push_str("  function _panaRun(){if(_panaStarted)return;_panaStarted=true;\n");
    out.push_str("  var _an=window.anime||{},animate=_an.animate||function(){},stagger=_an.stagger||function(){return 0;},onScroll=_an.onScroll||function(){return null;};\n");

    let motion_js = generate_motion_js(config);
    if !motion_js.is_empty() {
        out.push_str(&motion_js);
        out.push_str("\n\n");
    }

    out.push_str("  }\n");
    out.push_str("  if (document.readyState === \"loading\") document.addEventListener(\"DOMContentLoaded\", _panaRun, { once: true }); else _panaRun();\n");
    out.push_str("})();\n");
    out
}

fn block_metadata_comments(config: &PageJsConfig) -> String {
    let mut output = String::new();
    for block in &config.blocks {
        if let Ok(payload) = serde_json::to_string(&serde_json::json!({ "id": block.id })) {
            output.push_str("// @pana-block ");
            output.push_str(&payload);
            output.push('\n');
        }
    }
    output
}

fn motion_metadata_comment(config: &PageJsConfig) -> Option<String> {
    if !config.has_motion_items() {
        return None;
    }
    let payload = serde_json::json!({
        "version": config.version.unwrap_or(1),
        "motion": config.motion,
    });
    let encoded = serde_json::to_string(&payload).ok()?;
    Some(format!("  // @pana-motion {}", encoded))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::js::NativeBlockRuntimeEntry;

    #[test]
    fn generated_page_js_runs_after_dom_is_already_ready() {
        let js = generate_page_js(&PageJsConfig::default());
        assert!(js.contains("document.readyState === \"loading\""));
        assert!(js.contains("else _panaRun();"));
    }

    #[test]
    fn generated_page_js_embeds_motion_metadata_when_present() {
        let config = PageJsConfig {
            version: Some(1),
            motion: Some(serde_json::json!({
                "schemaVersion": 1,
                "animeVersion": "4.4.1",
                "items": [{ "id": "animation-a", "type": "animation" }]
            })),
            ..PageJsConfig::default()
        };
        let js = generate_page_js(&config);
        assert!(js.contains("// @pana-motion "));
        assert!(js.contains("MOTION STUDIO"));
        assert!(js.contains("\"type\":\"animation\""));
    }

    #[test]
    fn generated_page_js_uses_the_canonical_block_runtime() {
        let config = PageJsConfig {
            blocks: vec![NativeBlockRuntimeEntry {
                id: "accordion".to_string(),
            }],
            ..PageJsConfig::default()
        };
        let js = generate_page_js(&config);

        assert!(js.contains("window.PanaBlockRuntime"));
        assert!(js.contains("installPageConfig"));
        assert!(js.contains("\"id\":\"accordion\""));
        assert!(js.contains("// @pana-block {\"id\":\"accordion\"}"));
        assert!(!js.contains("// @pana-component"));
        assert!(!js.contains("window.PanaInteractiveRuntime"));
        assert_eq!(js.matches("register(\"accordion\"").count(), 1);
    }

    #[test]
    fn canonical_runtime_has_accessible_lifecycle_and_cleanup() {
        let js = generate_page_js(&PageJsConfig {
            blocks: vec![
                NativeBlockRuntimeEntry {
                    id: "counter".to_string(),
                },
                NativeBlockRuntimeEntry {
                    id: "tabs".to_string(),
                },
                NativeBlockRuntimeEntry {
                    id: "dialog".to_string(),
                },
                NativeBlockRuntimeEntry {
                    id: "offcanvas".to_string(),
                },
                NativeBlockRuntimeEntry {
                    id: "nav-menu".to_string(),
                },
            ],
            ..PageJsConfig::default()
        });

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
