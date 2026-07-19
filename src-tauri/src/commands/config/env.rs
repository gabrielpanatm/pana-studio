use std::collections::HashMap;

pub(super) fn parse_env(source: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(eq) = line.find('=') {
            let key = line[..eq].trim();
            let val = line[eq + 1..].trim().trim_matches('"').trim_matches('\'');
            if !key.is_empty() {
                map.insert(key.to_string(), val.to_string());
            }
        }
    }
    map
}

pub(super) fn upsert_env(source: &str, vars: &HashMap<String, String>) -> String {
    let mut result = String::new();
    let mut written: HashMap<String, bool> = HashMap::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            result.push_str(line);
            result.push('\n');
            continue;
        }
        if let Some(eq) = trimmed.find('=') {
            let key = trimmed[..eq].trim();
            if let Some(val) = vars.get(key) {
                result.push_str(&format!("{}={}\n", key, val));
                written.insert(key.to_string(), true);
                continue;
            }
        }
        result.push_str(line);
        result.push('\n');
    }

    for (key, val) in vars {
        if !written.contains_key(key.as_str()) {
            result.push_str(&format!("{}={}\n", key, val));
        }
    }

    result
}
