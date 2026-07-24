use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NativeBlockRuntimeEntry {
    pub id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PageJsConfig {
    #[serde(default)]
    pub version: Option<u32>,
    #[serde(default, alias = "components")]
    pub blocks: Vec<NativeBlockRuntimeEntry>,
    #[serde(default)]
    pub motion: Option<Value>,
}

impl PageJsConfig {
    pub fn has_motion_items(&self) -> bool {
        self.motion
            .as_ref()
            .and_then(|motion| motion.get("items"))
            .and_then(Value::as_array)
            .map(|items| !items.is_empty())
            .unwrap_or(false)
    }

    pub fn has_page_js(&self) -> bool {
        !self.blocks.is_empty() || self.has_motion_items()
    }

    pub fn uses_anime(&self) -> bool {
        self.has_motion_items()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_js_config_writes_blocks_and_reads_legacy_components() {
        let legacy: PageJsConfig = serde_json::from_str(
            r#"{"version":1,"components":[{"id":"accordion"}],"motion":null}"#,
        )
        .unwrap();
        assert_eq!(legacy.blocks[0].id, "accordion");

        let canonical = serde_json::to_string(&legacy).unwrap();
        assert!(canonical.contains(r#""blocks":[{"id":"accordion"}]"#));
        assert!(!canonical.contains(r#""components":"#));
    }
}
