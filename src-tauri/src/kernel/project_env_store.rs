use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use tauri::{AppHandle, Runtime};

use crate::{
    kernel::{
        file_buffer_store::hash_bytes,
        write_authority::{
            WriteAuthority, WriteCategory, WriteIntent, WriteOperationKind, WriteOwner,
            WritePolicy, WriteTarget,
        },
    },
    project::{read_project_disk_manifest, AcceptedProjectDiskManifest},
};

const PROJECT_ENV_PATH: &str = ".env";
const MAX_PROJECT_ENV_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Clone, Debug)]
struct EnvDiskSnapshot {
    source: String,
    version_token: String,
    hash: String,
}

pub struct ProjectEnvStore;

impl ProjectEnvStore {
    pub fn read_namespace(
        project_root: &Path,
        prefix: &str,
    ) -> Result<BTreeMap<String, String>, String> {
        validate_env_prefix(prefix)?;
        let source = read_env_disk_snapshot(project_root)?.map(|snapshot| snapshot.source);
        Ok(parse_env(source.as_deref().unwrap_or_default())
            .into_iter()
            .filter_map(|(key, value)| {
                key.strip_prefix(prefix)
                    .and_then(|suffix| suffix.strip_prefix("__"))
                    .map(|suffix| (suffix.to_string(), value))
            })
            .collect())
    }

    pub fn write_namespace<R: Runtime>(
        app: &AppHandle<R>,
        project_root: &Path,
        runtime_session_id: &str,
        accepted_disk: &AcceptedProjectDiskManifest,
        prefix: &str,
        values: Option<&BTreeMap<String, String>>,
    ) -> Result<AcceptedProjectDiskManifest, String> {
        validate_env_prefix(prefix)?;
        validate_namespace_values(values)?;
        require_env_git_safety(project_root)?;
        let project_root_text = project_root.to_string_lossy();
        accepted_disk.require_live_complete(
            runtime_session_id,
            project_root_text.as_ref(),
            project_root,
        )?;

        let before = read_env_disk_snapshot(project_root)?;
        let source = before
            .as_ref()
            .map(|snapshot| snapshot.source.as_str())
            .unwrap_or_default();
        let updated = replace_namespace(source, prefix, values);
        if updated == source {
            return Ok(accepted_disk.clone());
        }
        if updated.len() as u64 > MAX_PROJECT_ENV_BYTES {
            return Err(format!(
                "Fișierul .env rezultat depășește limita sigură de {MAX_PROJECT_ENV_BYTES} bytes."
            ));
        }

        let target = match before.as_ref() {
            Some(snapshot) => WriteTarget::new(
                project_root.join(PROJECT_ENV_PATH),
                project_root,
                "project/.env",
            )
            .with_expected_present(snapshot.version_token.clone(), Some(snapshot.hash.clone())),
            None => WriteTarget::new(
                project_root.join(PROJECT_ENV_PATH),
                project_root,
                "project/.env",
            )
            .with_expected_absent(),
        }
        .with_expected_runtime_session_id(runtime_session_id.to_string());
        let intent = WriteIntent::new(
            WriteCategory::ProjectSourceWrite,
            WriteOwner::ProjectEnvStore,
            WriteOperationKind::WriteText,
            target,
            WritePolicy::project_sensitive_atomic(),
            "Actualizare namespace credentiale deploy în .env",
        );
        WriteAuthority::new(app)
            .write_text(intent, &updated)
            .map_err(|error| error.into_terminal_diagnostic())?;

        let manifest = read_project_disk_manifest(project_root)?;
        accepted_disk.next(runtime_session_id, project_root_text.as_ref(), manifest)
    }
}

fn read_env_disk_snapshot(project_root: &Path) -> Result<Option<EnvDiskSnapshot>, String> {
    let path = project_root.join(PROJECT_ENV_PATH);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err("Fișierul .env trebuie să fie un fișier regulat, nu symlink.".to_string())
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Fișierul .env nu poate fi inspectat: {error}.")),
    }
    let file = match fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Fișierul .env nu poate fi deschis: {error}.")),
    };
    let metadata = file
        .metadata()
        .map_err(|error| format!("Fișierul .env nu poate fi inspectat: {error}."))?;
    if !metadata.is_file() {
        return Err("Fișierul .env trebuie să fie un fișier regulat, nu symlink.".to_string());
    }
    if metadata.len() > MAX_PROJECT_ENV_BYTES {
        return Err(format!(
            "Fișierul .env depășește limita sigură de {MAX_PROJECT_ENV_BYTES} bytes."
        ));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_PROJECT_ENV_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Fișierul .env nu poate fi citit: {error}."))?;
    if bytes.len() as u64 > MAX_PROJECT_ENV_BYTES {
        return Err(format!(
            "Fișierul .env depășește limita sigură de {MAX_PROJECT_ENV_BYTES} bytes."
        ));
    }
    let source = String::from_utf8(bytes.clone())
        .map_err(|_| "Fișierul .env trebuie să fie UTF-8 valid.".to_string())?;
    Ok(Some(EnvDiskSnapshot {
        source,
        version_token: crate::project::project_disk_metadata_version_token(&metadata),
        hash: hash_bytes(&bytes),
    }))
}

fn validate_namespace_values(values: Option<&BTreeMap<String, String>>) -> Result<(), String> {
    let Some(values) = values else {
        return Ok(());
    };
    for (suffix, value) in values {
        if suffix.is_empty()
            || suffix.len() > 64
            || suffix.starts_with('_')
            || suffix.ends_with('_')
            || suffix.contains("__")
            || !suffix
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Err(format!("Sufixul ENV {suffix} este invalid."));
        }
        if value.contains('\0') || value.len() as u64 > MAX_PROJECT_ENV_BYTES {
            return Err(format!("Valoarea ENV {suffix} depășește limita sigură."));
        }
    }
    Ok(())
}

pub fn validate_env_prefix(value: &str) -> Result<(), String> {
    const DEPLOY_NAMESPACE: &str = "PANA_DEPLOY_";
    let mut bytes = value.bytes();
    if !bytes.next().is_some_and(|byte| byte.is_ascii_uppercase())
        || !value.starts_with(DEPLOY_NAMESPACE)
        || value.len() <= DEPLOY_NAMESPACE.len()
        || value.len() > 96
        || value.ends_with('_')
        || value.contains("__")
        || !bytes.all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err("Prefixul ENV deploy este invalid.".to_string());
    }
    Ok(())
}

fn replace_namespace(
    source: &str,
    prefix: &str,
    values: Option<&BTreeMap<String, String>>,
) -> String {
    let namespace = format!("{prefix}__");
    let mut output = String::with_capacity(source.len().saturating_add(256));
    let mut inserted = false;
    for record in env_records(source) {
        let belongs = assignment_key(record).is_some_and(|key| key.starts_with(&namespace));
        if belongs {
            if !inserted {
                append_namespace(&mut output, prefix, values);
                inserted = true;
            }
            continue;
        }
        output.push_str(record);
    }
    if !inserted {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        append_namespace(&mut output, prefix, values);
    }
    output
}

fn append_namespace(output: &mut String, prefix: &str, values: Option<&BTreeMap<String, String>>) {
    let Some(values) = values else {
        return;
    };
    for (suffix, value) in values {
        output.push_str(prefix);
        output.push_str("__");
        output.push_str(suffix);
        output.push('=');
        output.push_str(&encode_env_value(value));
        output.push('\n');
    }
}

fn parse_env(source: &str) -> BTreeMap<String, String> {
    env_records(source)
        .into_iter()
        .filter_map(|record| {
            let key = assignment_key(record)?;
            let equals = record.find('=')?;
            Some((key.to_string(), decode_env_value(&record[equals + 1..])))
        })
        .collect()
}

/// Splits dotenv input into assignments without losing the original bytes.
/// Quoted multiline values stay in one record, which makes namespace removal
/// strict even for manually-authored `.env` files.
fn env_records(source: &str) -> Vec<&str> {
    let mut records = Vec::new();
    let mut start = 0usize;
    while start < source.len() {
        let physical_end = source[start..]
            .find('\n')
            .map(|offset| start + offset + 1)
            .unwrap_or(source.len());
        let first_line = &source[start..physical_end];
        let mut record_end = physical_end;

        if let Some((quote_offset, quote)) = assignment_value_quote(first_line) {
            let value_start = start + quote_offset + quote.len_utf8();
            if let Some(close_offset) = closing_quote_offset(&source[value_start..], quote) {
                let close = value_start + close_offset + quote.len_utf8();
                record_end = source[close..]
                    .find('\n')
                    .map(|offset| close + offset + 1)
                    .unwrap_or(source.len());
            } else {
                // Preserve an invalid unterminated assignment as one record.
                // If it belongs to the managed namespace, deleting the entire
                // tail is safer than leaving secret continuation bytes behind.
                record_end = source.len();
            }
        }

        records.push(&source[start..record_end]);
        start = record_end;
    }
    records
}

fn assignment_value_quote(line: &str) -> Option<(usize, char)> {
    assignment_key(line)?;
    let equals = line.find('=')?;
    let value = &line[equals + 1..];
    let whitespace = value.len().saturating_sub(value.trim_start().len());
    let quote_offset = equals + 1 + whitespace;
    match line[quote_offset..].chars().next()? {
        quote @ ('\'' | '"') => Some((quote_offset, quote)),
        _ => None,
    }
}

fn closing_quote_offset(value: &str, quote: char) -> Option<usize> {
    let mut escaped = false;
    for (offset, character) in value.char_indices() {
        if quote == '"' && escaped {
            escaped = false;
            continue;
        }
        if quote == '"' && character == '\\' {
            escaped = true;
            continue;
        }
        if character == quote {
            return Some(offset);
        }
    }
    None
}

fn assignment_key(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let trimmed = trimmed.strip_prefix("export ").unwrap_or(trimmed);
    if trimmed.starts_with('#') {
        return None;
    }
    let equals = trimmed.find('=')?;
    let key = trimmed[..equals].trim();
    if key.is_empty()
        || !key.bytes().enumerate().all(|(index, byte)| {
            byte == b'_' || byte.is_ascii_alphanumeric() && (index > 0 || !byte.is_ascii_digit())
        })
    {
        return None;
    }
    Some(key)
}

fn encode_env_value(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len().saturating_add(2));
    encoded.push('"');
    for character in value.chars() {
        match character {
            '\\' => encoded.push_str("\\\\"),
            '"' => encoded.push_str("\\\""),
            '\n' => encoded.push_str("\\n"),
            '\r' => encoded.push_str("\\r"),
            '\t' => encoded.push_str("\\t"),
            character => encoded.push(character),
        }
    }
    encoded.push('"');
    encoded
}

fn decode_env_value(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        let mut decoded = String::new();
        let mut escaped = false;
        for character in value[1..value.len() - 1].chars() {
            if escaped {
                match character {
                    'n' => decoded.push('\n'),
                    'r' => decoded.push('\r'),
                    't' => decoded.push('\t'),
                    '\\' => decoded.push('\\'),
                    '"' => decoded.push('"'),
                    other => {
                        decoded.push('\\');
                        decoded.push(other);
                    }
                }
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else {
                decoded.push(character);
            }
        }
        if escaped {
            decoded.push('\\');
        }
        return decoded;
    }
    if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        return value[1..value.len() - 1].to_string();
    }
    value
        .find(" #")
        .map(|index| value[..index].trim_end())
        .unwrap_or(value)
        .to_string()
}

fn require_env_git_safety(project_root: &Path) -> Result<(), String> {
    let tracked = git_status(project_root, &["ls-files", "--error-unmatch", "--", PROJECT_ENV_PATH])
        .ok_or_else(|| {
            "Git nu este disponibil; Pană Studio nu poate demonstra că .env este netracked. Salvarea credentialelor a fost blocată."
                .to_string()
        })?;
    if tracked {
        return Err(
            ".env este deja urmărit de Git. Elimină-l din index (git rm --cached .env) înainte de a salva credentiale."
                .to_string(),
        );
    }
    let ignored = git_status(
        project_root,
        &[
            "check-ignore",
            "--no-index",
            "--quiet",
            "--",
            PROJECT_ENV_PATH,
        ],
    );
    if ignored == Some(true) || root_gitignore_explicitly_ignores_env(project_root)? {
        return Ok(());
    }
    Err(
        ".env nu este ignorat de Git. Adaugă linia `.env` sau `/.env` în .gitignore și salvează proiectul înainte de credentiale."
            .to_string(),
    )
}

fn git_status(project_root: &Path, arguments: &[&str]) -> Option<bool> {
    let status = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()?;
    Some(status.success())
}

fn root_gitignore_explicitly_ignores_env(project_root: &Path) -> Result<bool, String> {
    let path: PathBuf = project_root.join(".gitignore");
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!(".gitignore nu poate fi citit: {error}.")),
    };
    Ok(source.lines().any(|line| {
        let line = line.trim();
        matches!(line, ".env" | "/.env")
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn namespace_rewrite_preserves_comments_unknowns_and_order() {
        let source = "# local\nOTHER='keep me'\nPANA_DEPLOY_A__TOKEN=old\nTAIL=yes\n";
        let values = BTreeMap::from([
            ("TOKEN".to_string(), "a\"b\\c\n".to_string()),
            ("USER".to_string(), "gabriel".to_string()),
        ]);
        let updated = replace_namespace(source, "PANA_DEPLOY_A", Some(&values));
        assert!(updated.starts_with("# local\nOTHER='keep me'\n"));
        assert!(updated.ends_with("TAIL=yes\n"));
        assert!(!updated.contains("=old"));
        let parsed = parse_env(&updated);
        assert_eq!(parsed["OTHER"], "keep me");
        assert_eq!(parsed["PANA_DEPLOY_A__TOKEN"], "a\"b\\c\n");
        assert_eq!(parsed["PANA_DEPLOY_A__USER"], "gabriel");
    }

    #[test]
    fn namespace_delete_is_strict_and_leaves_other_prefixes_untouched() {
        let source = "PANA_DEPLOY_A__USER=a\nPANA_DEPLOY_A__EXTRA=x\nPANA_DEPLOY_B__USER=b\n";
        assert_eq!(
            replace_namespace(source, "PANA_DEPLOY_A", None),
            "PANA_DEPLOY_B__USER=b\n"
        );
    }

    #[test]
    fn overlapping_target_prefixes_have_disjoint_namespaces() {
        let source = concat!(
            "PANA_DEPLOY_PRODUCTION__TOKEN=primary\n",
            "PANA_DEPLOY_PRODUCTION_2__TOKEN=secondary\n",
        );
        assert_eq!(
            replace_namespace(source, "PANA_DEPLOY_PRODUCTION", None),
            "PANA_DEPLOY_PRODUCTION_2__TOKEN=secondary\n"
        );
    }

    #[test]
    fn namespace_rewrite_removes_complete_multiline_assignments() {
        let source = concat!(
            "BEFORE=keep\n",
            "PANA_DEPLOY_A__PRIVATE_KEY=\"line one\nline two\"\n",
            "PANA_DEPLOY_B__TOKEN=keep-too\n",
        );
        let updated = replace_namespace(source, "PANA_DEPLOY_A", None);
        assert_eq!(updated, "BEFORE=keep\nPANA_DEPLOY_B__TOKEN=keep-too\n");
        assert!(!updated.contains("line two"));
    }

    #[test]
    fn parser_reads_multiline_quoted_values_without_exposing_following_assignments() {
        let source = "PANA_DEPLOY_A__KEY=\"line one\nline two\"\nOTHER=ok\n";
        let parsed = parse_env(source);
        assert_eq!(parsed["PANA_DEPLOY_A__KEY"], "line one\nline two");
        assert_eq!(parsed["OTHER"], "ok");
    }

    #[test]
    fn prefix_contract_is_namespaced_and_deterministic() {
        assert!(validate_env_prefix("PANA_DEPLOY_PRODUCTION_2").is_ok());
        for invalid in [
            "",
            "pana_deploy",
            "2_DEPLOY",
            "PANA__DEPLOY",
            "PANA-",
            "AWS_PRODUCTION",
            "PANA_DEPLOY_",
        ] {
            assert!(validate_env_prefix(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn git_safety_requires_ignore_and_blocks_tracked_env() {
        let unignored = unique_test_dir("unignored");
        fs::create_dir_all(&unignored).unwrap();
        assert!(require_env_git_safety(&unignored)
            .unwrap_err()
            .contains("nu este ignorat"));

        fs::write(unignored.join(".gitignore"), "/.env\n").unwrap();
        require_env_git_safety(&unignored).unwrap();
        fs::remove_dir_all(&unignored).unwrap();

        let tracked = unique_test_dir("tracked");
        fs::create_dir_all(&tracked).unwrap();
        fs::write(tracked.join(".gitignore"), "/.env\n").unwrap();
        fs::write(tracked.join(".env"), "PANA_DEPLOY_TEST__TOKEN=secret\n").unwrap();
        assert!(Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&tracked)
            .status()
            .unwrap()
            .success());
        assert!(Command::new("git")
            .args(["add", "--force", ".env"])
            .current_dir(&tracked)
            .status()
            .unwrap()
            .success());
        assert!(require_env_git_safety(&tracked)
            .unwrap_err()
            .contains("deja urmărit"));
        fs::remove_dir_all(&tracked).unwrap();
    }

    fn unique_test_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "panastudio-project-env-{label}-{}-{stamp}",
            TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
        ))
    }
}
