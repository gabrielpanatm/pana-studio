use std::{collections::HashMap, fs, path::Path};

pub(super) fn read_env_from_root(root: &Path) -> Result<HashMap<String, String>, String> {
    let path = root.join(".env");
    if !path.exists() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "pana-deploy-env-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn project_env_never_falls_back_to_parent_credentials() {
        let parent = temp_dir("no-parent-fallback");
        let project = parent.join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(parent.join(".env"), "BUNNY_STORAGE_KEY=parent-secret\n").unwrap();

        let error = read_env_from_root(&project).unwrap_err();
        assert!(error.contains("nu a fost găsit"));

        fs::write(project.join(".env"), "BUNNY_STORAGE_KEY=project-secret\n").unwrap();
        let env = read_env_from_root(&project).unwrap();
        assert_eq!(env.get("BUNNY_STORAGE_KEY").unwrap(), "project-secret");

        fs::remove_dir_all(parent).unwrap();
    }
}
