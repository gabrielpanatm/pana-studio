use serde::{Deserialize, Serialize};

use super::MotionDocument;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PageJsConfig {
    #[serde(default)]
    pub motion: Option<MotionDocument>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_js_config_contains_only_editorial_motion_state() {
        let config = PageJsConfig::default();
        let canonical = serde_json::to_string(&config).unwrap();
        assert_eq!(canonical, r#"{"motion":null}"#);
        assert!(!canonical.contains("blocks"));
        assert!(!canonical.contains("version"));
    }

    #[test]
    fn legacy_page_js_fields_are_rejected_instead_of_ignored() {
        assert!(serde_json::from_str::<PageJsConfig>(r#"{"version":2,"blocks":[]}"#).is_err());
    }
}
