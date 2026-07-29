use serde::{de::Error as _, Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::MotionDocument;

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
    #[serde(default, deserialize_with = "deserialize_motion")]
    pub motion: Option<MotionDocument>,
}

fn deserialize_motion<'de, D>(deserializer: D) -> Result<Option<MotionDocument>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    value
        .map(MotionDocument::from_value)
        .transpose()
        .map_err(D::Error::custom)
}

impl PageJsConfig {
    pub fn has_motion_items(&self) -> bool {
        self.motion
            .as_ref()
            .map(|motion| !motion.is_empty())
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

    #[test]
    fn page_js_config_migrates_legacy_motion_during_deserialization() {
        let legacy: PageJsConfig = serde_json::from_str(
            r#"{
              "version":1,
              "motion":{
                "schemaVersion":1,
                "items":[{
                  "id":"fade",
                  "type":"animation",
                  "name":"Fade",
                  "trigger":"load",
                  "target":{"mode":"dataAnim","dataAnim":"hero"},
                  "properties":[{
                    "id":"opacity",
                    "property":"opacity",
                    "value":{"mode":"fromTo","from":"0","to":"1"}
                  }],
                  "playback":{"duration":600}
                }]
              }
            }"#,
        )
        .expect("legacy config");

        let motion = legacy.motion.expect("migrated motion");
        assert_eq!(motion.schema_version, 2);
        assert_eq!(motion.interactions.len(), 1);
        assert_eq!(motion.interactions[0].id, "fade");
    }
}
