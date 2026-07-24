use crate::js::PageJsConfig;

pub(crate) const NATIVE_BLOCK_RUNTIME_SCRIPT: &str = include_str!("runtime.js");

pub(crate) fn install_native_block_runtime(config: &PageJsConfig) -> String {
    let config = serde_json::to_string(config).unwrap_or_else(|_| "{}".to_string());
    format!("{NATIVE_BLOCK_RUNTIME_SCRIPT}\nwindow.PanaBlockRuntime.installPageConfig({config});")
}

#[cfg(test)]
mod tests {
    use crate::js::{NativeBlockRuntimeEntry, PageJsConfig};

    use super::*;

    #[test]
    fn canonical_runtime_owns_all_six_native_providers_and_legacy_selectors() {
        for provider in [
            "counter",
            "accordion",
            "tabs",
            "dialog",
            "offcanvas",
            "nav-menu",
        ] {
            assert!(NATIVE_BLOCK_RUNTIME_SCRIPT.contains(&format!("register(\"{provider}\"")));
        }
        assert!(NATIVE_BLOCK_RUNTIME_SCRIPT.contains("data-pana-block"));
        assert!(NATIVE_BLOCK_RUNTIME_SCRIPT.contains("data-pana-component"));
        assert!(NATIVE_BLOCK_RUNTIME_SCRIPT.contains("PanaBlockRuntime"));
        assert!(NATIVE_BLOCK_RUNTIME_SCRIPT.contains("optionSignature"));
        assert!(NATIVE_BLOCK_RUNTIME_SCRIPT.contains("data-default-tab"));
        assert!(NATIVE_BLOCK_RUNTIME_SCRIPT.contains("data-close-outside"));
        assert!(NATIVE_BLOCK_RUNTIME_SCRIPT.contains("data-close-on-select"));
        assert!(!NATIVE_BLOCK_RUNTIME_SCRIPT.contains("PanaInteractiveRuntime"));
    }

    #[test]
    fn installation_serializes_the_canonical_blocks_field() {
        let config = PageJsConfig {
            blocks: vec![NativeBlockRuntimeEntry {
                id: "accordion".to_string(),
            }],
            ..PageJsConfig::default()
        };
        let script = install_native_block_runtime(&config);

        assert!(script.contains("window.PanaBlockRuntime.installPageConfig"));
        assert!(script.contains("\"blocks\":[{\"id\":\"accordion\"}]"));
        assert!(!script.contains("\"components\":"));
    }
}
