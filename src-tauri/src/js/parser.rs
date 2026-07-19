use std::collections::{HashMap, HashSet};

use super::types::{PageJsConfig, PanaComponent};

pub fn parse_page_js(content: &str) -> PageJsConfig {
    let mut components: Vec<PanaComponent> = Vec::new();
    let mut component_ids: HashSet<String> = HashSet::new();
    let mut version: Option<u32> = None;
    let mut motion: Option<serde_json::Value> = None;

    for raw_line in content.lines() {
        let line = raw_line.trim();

        if let Some(rest) = line.strip_prefix("// @pana-motion ") {
            if let Ok(payload) = serde_json::from_str::<serde_json::Value>(rest) {
                version = payload
                    .get("version")
                    .and_then(serde_json::Value::as_u64)
                    .map(|value| value as u32)
                    .or(Some(1));
                motion = payload.get("motion").cloned();
            }
            continue;
        }

        if let Some(rest) = line.strip_prefix("// @pana-component ") {
            let params = parse_kv_params(rest);
            let id = params.get("id").cloned().unwrap_or_default();
            if !id.is_empty() && component_ids.insert(id.clone()) {
                components.push(PanaComponent { id });
            }
        }
    }

    PageJsConfig {
        version,
        components,
        motion,
    }
}

fn parse_kv_params(s: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for part in s.split_whitespace() {
        if let Some((key, value)) = part.split_once('=') {
            map.insert(key.to_string(), value.to_string());
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_motion_metadata() {
        let config = parse_page_js(
            r#"(function () {
  // @pana-motion {"version":1,"motion":{"schemaVersion":1,"animeVersion":"4.4.1","items":[{"id":"animation-a","type":"animation"}]}}
})();"#,
        );
        assert_eq!(config.version, Some(1));
        assert!(config.has_motion_items());
        assert!(config.uses_anime());
    }
}
