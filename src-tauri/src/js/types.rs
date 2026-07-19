use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PanaComponent {
    pub id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PageJsConfig {
    #[serde(default)]
    pub version: Option<u32>,
    #[serde(default)]
    pub components: Vec<PanaComponent>,
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
        !self.components.is_empty() || self.has_motion_items()
    }

    pub fn uses_anime(&self) -> bool {
        self.has_motion_items()
    }
}
