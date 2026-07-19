use std::{collections::HashMap, fs, path::Path};

pub(super) fn read_env_from_root(root: &Path) -> Result<HashMap<String, String>, String> {
    let path = root.join(".env");
    if !path.exists() {
        let parent_path = root.parent().map(|p| p.join(".env"));
        if let Some(pp) = parent_path {
            if pp.exists() {
                let source = fs::read_to_string(&pp).map_err(|e| e.to_string())?;
                return Ok(parse_env(&source));
            }
        }
        return Err(
            "Fișierul .env nu a fost găsit.\nAdaugă credentialele Bunny în tab-ul Deploy."
                .to_string(),
        );
    }
    let source = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    Ok(parse_env(&source))
}

pub(super) fn env_require(env: &HashMap<String, String>, key: &str) -> Result<String, String> {
    env.get(key)
        .filter(|v| !v.is_empty())
        .cloned()
        .ok_or_else(|| format!("Lipsă credential: {}. Completează în tab-ul Deploy.", key))
}

fn parse_env(source: &str) -> HashMap<String, String> {
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
