use std::{
    fs,
    io::ErrorKind,
    io::Read,
    path::{Path, PathBuf},
};

use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime};
use toml_edit::{value, DocumentMut, InlineTable, Item, Table, Value};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

use crate::kernel::{
    observability::now_ms,
    write_authority::{
        CodexConfigLease, WriteAuthority, WriteAuthorityError, WriteCategory, WriteIntent,
        WriteOperationKind, WriteOwner, WritePolicy,
    },
};

use super::{
    security::{
        access_tokens_equal, generate_access_token, is_valid_access_token, ACCESS_TOKEN_HEADER,
    },
    MCP_ENDPOINT,
};

const MAX_CODEX_CONFIG_BYTES: u64 = 4 * 1024 * 1024;
const CODEX_SERVER_KEY: &str = "pana-studio";
const CODEX_ACCESS_TOKEN_HEADER: &str = "X-Pana-Studio-Token";
const STDIO_ONLY_KEYS: [&str; 6] = [
    "command",
    "args",
    "env",
    "env_vars",
    "cwd",
    "experimental_environment",
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexMcpStatus {
    pub config_path: String,
    pub config_exists: bool,
    pub configured: bool,
    pub authenticated: bool,
    pub secure_permissions: bool,
    pub configured_url: Option<String>,
    pub expected_url: String,
}

pub fn read_codex_status<R: Runtime>(app: &AppHandle<R>) -> Result<CodexMcpStatus, String> {
    let config_path = codex_config_path()?;
    let source = read_existing_codex_config_for_update(&config_path)?;
    let access_token = current_access_token(app)?;
    let secure_permissions = codex_config_permissions_are_private(&config_path, source.is_some());
    Ok(status_from_source(
        &config_path,
        source.as_deref(),
        &access_token,
        secure_permissions,
    ))
}

pub fn load_or_generate_access_token() -> Result<String, String> {
    let configured = codex_config_path()
        .ok()
        .and_then(|path| {
            let source = read_existing_codex_config_for_update(&path)
                .ok()
                .flatten()?;
            codex_config_permissions_are_private(&path, true).then_some(source)
        })
        .and_then(|source| read_codex_server_access_token(&source))
        .filter(|token| is_valid_access_token(token));
    configured.map_or_else(generate_access_token, Ok)
}

pub fn configure_codex<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<CodexMcpStatus, WriteAuthorityError> {
    let access_token = current_access_token(app)?;
    let config_path = codex_config_path()?;
    let boundary = config_path
        .parent()
        .ok_or_else(|| "Nu am putut determina folderul Codex.".to_string())?
        .to_path_buf();
    let lease = CodexConfigLease::capture(&boundary).map_err(|error| {
        format!(
            "Configurația Codex nu poate fi actualizată fără un director .codex existent și stabil: {error}"
        )
    })?;

    let existing_source = read_existing_codex_config_for_update(&config_path)?;
    if !codex_config_permissions_are_private(&config_path, existing_source.is_some()) {
        return Err(format!(
            "Configurația Codex {} este accesibilă altor utilizatori. Setează permisiunile la 0600 înainte ca Pană Studio să scrie tokenul MCP.",
            config_path.display()
        )
        .into());
    }
    let source = existing_source.clone().unwrap_or_default();

    let updated = upsert_codex_server_config(&source, &access_token)?;
    if existing_source.as_deref() == Some(updated.as_str()) {
        return Ok(status_from_source(
            &config_path,
            existing_source.as_deref(),
            &access_token,
            true,
        ));
    }

    let backup = existing_source
        .as_ref()
        .map(|source| {
            Ok::<_, String>((
                lease.target(
                    backup_path_for(&config_path),
                    "external:~/.codex/config.toml backup",
                )?,
                source.as_str(),
            ))
        })
        .transpose()?;
    let intent = WriteIntent::new(
        WriteCategory::ExternalIntegrationWrite,
        WriteOwner::CodexMcp,
        WriteOperationKind::ExternalConfigUpdate,
        lease.target(config_path.clone(), "external:~/.codex/config.toml")?,
        WritePolicy::external_config_update(),
        "Codex MCP actualizează configurația externă Codex cu backup înainte de scriere.",
    );
    WriteAuthority::new(app)
        .external_config_update(intent, &updated, backup)
        .map(|_| ())?;

    Ok(status_from_source(
        &config_path,
        Some(&updated),
        &access_token,
        true,
    ))
}

fn current_access_token<R: Runtime>(app: &AppHandle<R>) -> Result<String, String> {
    app.state::<crate::state::AppState>()
        .mcp_access_token
        .lock()
        .map_err(|_| "Tokenul MCP din RAM este indisponibil.".to_string())?
        .clone()
        .ok_or_else(|| "Serverul MCP nu și-a inițializat încă tokenul de acces.".to_string())
}

fn status_from_source(
    config_path: &Path,
    source: Option<&str>,
    expected_access_token: &str,
    secure_permissions: bool,
) -> CodexMcpStatus {
    let configured_url = source.and_then(read_codex_server_url);
    let authenticated = source
        .and_then(read_codex_server_access_token)
        .is_some_and(|token| access_tokens_equal(&token, expected_access_token));
    let configured = secure_permissions
        && authenticated
        && source
            .and_then(|value| value.parse::<DocumentMut>().ok())
            .is_some_and(|document| {
                validate_codex_http_server(&document, expected_access_token).is_ok()
            });
    CodexMcpStatus {
        config_path: config_path.to_string_lossy().to_string(),
        config_exists: source.is_some(),
        configured,
        authenticated,
        secure_permissions,
        configured_url,
        expected_url: MCP_ENDPOINT.to_string(),
    }
}

fn codex_config_path() -> Result<PathBuf, String> {
    codex_config_path_from_locations(
        std::env::var_os("CODEX_HOME").map(PathBuf::from),
        std::env::var_os("HOME").map(PathBuf::from),
        std::env::var_os("USERPROFILE").map(PathBuf::from),
    )
}

fn codex_config_path_from_locations(
    codex_home: Option<PathBuf>,
    home: Option<PathBuf>,
    user_profile: Option<PathBuf>,
) -> Result<PathBuf, String> {
    if let Some(codex_home) = codex_home {
        if codex_home.as_os_str().is_empty() {
            return Err("CODEX_HOME este setat, dar gol.".to_string());
        }
        return Ok(codex_home.join("config.toml"));
    }
    let home = home
        .or(user_profile)
        .ok_or_else(|| "Nu am putut detecta folderul home pentru Codex.".to_string())?;
    Ok(home.join(".codex").join("config.toml"))
}

fn read_existing_codex_config_for_update(path: &Path) -> Result<Option<String>, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(format!(
            "Config-ul Codex {} este symlink; Pană Studio nu îl modifică automat.",
            path.display()
        )),
        Ok(metadata) if metadata.is_dir() => Err(format!(
            "Config-ul Codex {} este director; Pană Studio nu îl poate modifica.",
            path.display()
        )),
        Ok(metadata) => {
            if !metadata.is_file() {
                return Err(format!(
                    "Config-ul Codex {} nu este fișier regular; Pană Studio nu îl citește automat.",
                    path.display()
                ));
            }
            if metadata.len() > MAX_CODEX_CONFIG_BYTES {
                return Err(format!(
                    "Config-ul Codex {} depășește limita de {} bytes.",
                    path.display(),
                    MAX_CODEX_CONFIG_BYTES
                ));
            }
            let file = fs::File::open(path).map_err(|error| {
                format!(
                    "Nu am putut deschide config-ul Codex {}: {}",
                    path.display(),
                    error
                )
            })?;
            let opened = file.metadata().map_err(|error| {
                format!(
                    "Nu am putut reverifica config-ul Codex {}: {}",
                    path.display(),
                    error
                )
            })?;
            if !opened.is_file() || opened.len() > MAX_CODEX_CONFIG_BYTES {
                return Err(format!(
                    "Config-ul Codex {} s-a schimbat ori depășește limita în timpul deschiderii.",
                    path.display()
                ));
            }
            #[cfg(unix)]
            if metadata.dev() != opened.dev() || metadata.ino() != opened.ino() {
                return Err(format!(
                    "Config-ul Codex {} a fost înlocuit în timpul capturii.",
                    path.display()
                ));
            }
            let mut bytes = Vec::with_capacity(opened.len() as usize);
            file.take(MAX_CODEX_CONFIG_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|error| {
                    format!(
                        "Nu am putut citi bounded config-ul Codex {}: {}",
                        path.display(),
                        error
                    )
                })?;
            if bytes.len() as u64 > MAX_CODEX_CONFIG_BYTES {
                return Err(format!(
                    "Config-ul Codex {} a crescut peste limita de {} bytes în timpul citirii.",
                    path.display(),
                    MAX_CODEX_CONFIG_BYTES
                ));
            }
            String::from_utf8(bytes).map(Some).map_err(|error| {
                format!(
                    "Config-ul Codex {} nu este UTF-8 valid: {error}",
                    path.display()
                )
            })
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "Nu am putut verifica config-ul Codex {}: {}",
            path.display(),
            error
        )),
    }
}

fn codex_config_permissions_are_private(path: &Path, exists: bool) -> bool {
    if !exists {
        return true;
    }
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return false;
    }
    #[cfg(unix)]
    {
        metadata.mode() & 0o077 == 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn backup_path_for(config_path: &Path) -> PathBuf {
    config_path.with_file_name(format!(
        "config.toml.pana-studio-{}-{}.bak",
        std::process::id(),
        now_ms()
    ))
}

fn read_codex_server_url(source: &str) -> Option<String> {
    let document = source.parse::<DocumentMut>().ok()?;
    codex_server_table(&document)?
        .get("url")?
        .as_str()
        .map(ToOwned::to_owned)
}

fn read_codex_server_access_token(source: &str) -> Option<String> {
    let document = source.parse::<DocumentMut>().ok()?;
    codex_server_table(&document)?
        .get("http_headers")?
        .as_table_like()?
        .get(CODEX_ACCESS_TOKEN_HEADER)?
        .as_str()
        .map(ToOwned::to_owned)
}

fn upsert_codex_server_config(source: &str, access_token: &str) -> Result<String, String> {
    if !is_valid_access_token(access_token) {
        return Err(
            "Tokenul MCP din RAM are un format invalid; config-ul Codex nu este modificat."
                .to_string(),
        );
    }
    let mut document = if source.trim().is_empty() {
        DocumentMut::new()
    } else {
        source.parse::<DocumentMut>().map_err(|error| {
            format!(
                "Configurația Codex existentă nu este TOML valid și nu poate fi modificată sigur: {error}"
            )
        })?
    };

    if document.get("mcp_servers").is_none() {
        document["mcp_servers"] = Item::Table(Table::new());
    }
    let mcp_servers = document
        .get_mut("mcp_servers")
        .and_then(Item::as_table_like_mut)
        .ok_or_else(|| {
            "Configurația Codex are `mcp_servers` cu alt tip decât tabel; actualizarea este blocată."
                .to_string()
        })?;
    if mcp_servers.get(CODEX_SERVER_KEY).is_none() {
        mcp_servers.insert(CODEX_SERVER_KEY, Item::Table(Table::new()));
    }
    let server = mcp_servers
        .get_mut(CODEX_SERVER_KEY)
        .and_then(Item::as_table_like_mut)
        .ok_or_else(|| {
            "Configurația Codex are serverul `pana-studio` cu alt tip decât tabel; actualizarea este blocată."
                .to_string()
        })?;

    // Documentația Codex separă transporturile: `command` și familia sa sunt
    // STDIO, iar `url` este Streamable HTTP. Conversia nu păstrează o a doua
    // autoritate de transport ambiguă în aceeași secțiune.
    for key in STDIO_ONLY_KEYS {
        server.remove(key);
    }
    match server.get_mut("url") {
        Some(item) if item.as_str() == Some(MCP_ENDPOINT) => {}
        Some(item) => {
            let decor = item.as_value().map(|current| current.decor().clone());
            let mut replacement = Value::from(MCP_ENDPOINT);
            if let Some(decor) = decor {
                *replacement.decor_mut() = decor;
            }
            *item = Item::Value(replacement);
        }
        None => {
            server.insert("url", value(MCP_ENDPOINT));
        }
    }
    match server.get_mut("enabled") {
        Some(item) if item.as_bool() == Some(true) => {}
        Some(item) => {
            let decor = item.as_value().map(|current| current.decor().clone());
            let mut replacement = Value::from(true);
            if let Some(decor) = decor {
                *replacement.decor_mut() = decor;
            }
            *item = Item::Value(replacement);
        }
        None => {
            server.insert("enabled", value(true));
        }
    }

    if server.get("http_headers").is_none() {
        server.insert(
            "http_headers",
            Item::Value(Value::InlineTable(InlineTable::new())),
        );
    }
    let headers = server
        .get_mut("http_headers")
        .and_then(Item::as_table_like_mut)
        .ok_or_else(|| {
            "Configurația Codex are `http_headers` cu alt tip decât map; actualizarea este blocată."
                .to_string()
        })?;
    match headers.get_mut(CODEX_ACCESS_TOKEN_HEADER) {
        Some(item) if item.as_str() == Some(access_token) => {}
        Some(item) => {
            let decor = item.as_value().map(|current| current.decor().clone());
            let mut replacement = Value::from(access_token);
            if let Some(decor) = decor {
                *replacement.decor_mut() = decor;
            }
            *item = Item::Value(replacement);
        }
        None => {
            headers.insert(CODEX_ACCESS_TOKEN_HEADER, value(access_token));
        }
    }

    let updated = document.to_string();
    if updated.len() as u64 > MAX_CODEX_CONFIG_BYTES {
        return Err(format!(
            "Configurația Codex rezultată depășește limita de {MAX_CODEX_CONFIG_BYTES} bytes."
        ));
    }
    let reparsed = updated.parse::<DocumentMut>().map_err(|error| {
        format!("Editorul semantic Codex a produs TOML invalid și a blocat scrierea: {error}")
    })?;
    validate_codex_http_server(&reparsed, access_token)?;
    Ok(updated)
}

fn codex_server_table(document: &DocumentMut) -> Option<&dyn toml_edit::TableLike> {
    document
        .get("mcp_servers")?
        .as_table_like()?
        .get(CODEX_SERVER_KEY)?
        .as_table_like()
}

fn validate_codex_http_server(
    document: &DocumentMut,
    expected_access_token: &str,
) -> Result<(), String> {
    let server = codex_server_table(document).ok_or_else(|| {
        "Configurația Codex rezultată nu conține tabelul pana-studio.".to_string()
    })?;
    let url = server
        .get("url")
        .and_then(Item::as_str)
        .ok_or_else(|| "Configurația Codex rezultată nu are un URL HTTP textual.".to_string())?;
    if url != MCP_ENDPOINT {
        return Err("Configurația Codex rezultată nu păstrează endpointul MCP planificat.".into());
    }
    if server
        .get("enabled")
        .is_some_and(|item| item.as_bool() != Some(true))
    {
        return Err(
            "Configurația Codex pana-studio este dezactivată sau are `enabled` invalid.".into(),
        );
    }
    for key in STDIO_ONLY_KEYS {
        if server.contains_key(key) {
            return Err(format!(
                "Configurația Codex rezultată amestecă transportul HTTP cu cheia STDIO `{key}`."
            ));
        }
    }
    let configured_token = server
        .get("http_headers")
        .and_then(Item::as_table_like)
        .and_then(|headers| headers.get(CODEX_ACCESS_TOKEN_HEADER))
        .and_then(Item::as_str)
        .ok_or_else(|| {
            format!("Configurația Codex rezultată nu conține headerul {ACCESS_TOKEN_HEADER}.")
        })?;
    if !access_tokens_equal(configured_token, expected_access_token) {
        return Err("Configurația Codex rezultată nu conține tokenul MCP curent.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    const TEST_ACCESS_TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[test]
    fn codex_status_is_derived_without_a_post_commit_disk_read() {
        let path = PathBuf::from("/home/test/.codex/config.toml");
        let source = format!(
            "[mcp_servers.pana-studio]\n\"url\" = \"{MCP_ENDPOINT}\"\nhttp_headers = {{ \"{CODEX_ACCESS_TOKEN_HEADER}\" = \"{TEST_ACCESS_TOKEN}\" }}\n"
        );

        let status = status_from_source(&path, Some(&source), TEST_ACCESS_TOKEN, true);

        assert!(status.config_exists);
        assert!(status.configured);
        assert!(status.authenticated);
        assert_eq!(status.configured_url.as_deref(), Some(MCP_ENDPOINT));
    }

    #[test]
    fn codex_status_rejects_ambiguous_http_and_stdio_transport() {
        let path = PathBuf::from("/home/test/.codex/config.toml");
        let source = format!(
            "[mcp_servers.pana-studio]\nurl = \"{MCP_ENDPOINT}\"\ncommand = \"node\"\nargs = [\"server.js\"]\n"
        );

        let status = status_from_source(&path, Some(&source), TEST_ACCESS_TOKEN, true);

        assert!(!status.configured);
        assert_eq!(status.configured_url.as_deref(), Some(MCP_ENDPOINT));
    }

    #[test]
    fn codex_toml_update_preserves_adjacent_table_boundary() {
        let source = "[mcp_servers.pana-studio]\nurl = \"https://old.invalid/mcp\"\n[other]\nenabled = true\n";

        let updated = upsert_codex_server_config(source, TEST_ACCESS_TOKEN).unwrap();
        let document = updated.parse::<DocumentMut>().unwrap();

        validate_codex_http_server(&document, TEST_ACCESS_TOKEN).unwrap();
        assert_eq!(
            document
                .get("other")
                .and_then(Item::as_table_like)
                .and_then(|table| table.get("enabled"))
                .and_then(Item::as_bool),
            Some(true)
        );
    }

    #[test]
    fn codex_toml_update_replaces_quoted_url_semantically_once() {
        let source = "[mcp_servers.\"pana-studio\"]\n\"url\" = \"https://old.invalid/mcp\"\n";

        let updated = upsert_codex_server_config(source, TEST_ACCESS_TOKEN).unwrap();
        let document = updated.parse::<DocumentMut>().unwrap();
        let server = codex_server_table(&document).unwrap();

        assert_eq!(server.iter().filter(|(key, _)| *key == "url").count(), 1);
        assert_eq!(server.get("url").and_then(Item::as_str), Some(MCP_ENDPOINT));
    }

    #[test]
    fn codex_toml_update_is_byte_stable_when_http_contract_is_already_exact() {
        let source = format!(
            "[mcp_servers.pana-studio]\nurl = \"{MCP_ENDPOINT}\" # keep this comment\nenabled = true\n"
        );

        let configured = upsert_codex_server_config(&source, TEST_ACCESS_TOKEN).unwrap();

        let updated = upsert_codex_server_config(&configured, TEST_ACCESS_TOKEN).unwrap();

        assert_eq!(updated, configured);
    }

    #[test]
    fn codex_toml_update_preserves_url_comment_when_value_changes() {
        let source =
            "[mcp_servers.pana-studio]\nurl = \"https://old.invalid/mcp\" # endpoint comment\n";

        let updated = upsert_codex_server_config(source, TEST_ACCESS_TOKEN).unwrap();

        assert!(updated.contains("# endpoint comment"), "{updated}");
        assert_eq!(
            read_codex_server_url(&updated).as_deref(),
            Some(MCP_ENDPOINT)
        );
    }

    #[test]
    fn codex_toml_http_conversion_removes_stdio_authority_and_preserves_common_keys() {
        let source = r#"[mcp_servers.pana-studio]
command = "node"
args = ["server.js"]
env_vars = ["TOKEN"]
cwd = "/tmp"
experimental_environment = "remote"
enabled = false

[mcp_servers.pana-studio.env]
TOKEN = "secret"
"#;

        let updated = upsert_codex_server_config(source, TEST_ACCESS_TOKEN).unwrap();
        let document = updated.parse::<DocumentMut>().unwrap();
        let server = codex_server_table(&document).unwrap();

        for key in STDIO_ONLY_KEYS {
            assert!(!server.contains_key(key), "stdio key survived: {key}");
        }
        assert_eq!(server.get("enabled").and_then(Item::as_bool), Some(true));
        assert_eq!(server.get("url").and_then(Item::as_str), Some(MCP_ENDPOINT));
    }

    #[test]
    fn codex_status_rejects_exact_url_when_server_is_disabled() {
        let path = PathBuf::from("/home/test/.codex/config.toml");
        let source =
            format!("[mcp_servers.pana-studio]\nurl = \"{MCP_ENDPOINT}\"\nenabled = false\n");

        let status = status_from_source(&path, Some(&source), TEST_ACCESS_TOKEN, true);

        assert!(!status.configured);
        assert_eq!(status.configured_url.as_deref(), Some(MCP_ENDPOINT));
    }

    #[test]
    fn codex_toml_http_conversion_supports_inline_server_tables() {
        let source =
            "mcp_servers = { pana-studio = { command = \"node\", args = [\"server.js\"], enabled = true } }\n";

        let updated = upsert_codex_server_config(source, TEST_ACCESS_TOKEN).unwrap();
        let document = updated.parse::<DocumentMut>().unwrap();
        let server = codex_server_table(&document).unwrap();

        assert!(!server.contains_key("command"));
        assert!(!server.contains_key("args"));
        assert_eq!(server.get("enabled").and_then(Item::as_bool), Some(true));
        assert_eq!(server.get("url").and_then(Item::as_str), Some(MCP_ENDPOINT));
    }

    #[test]
    fn codex_toml_update_accepts_crlf_and_keeps_document_valid() {
        let source = "title = \"config\"\r\n\r\n[mcp_servers.pana-studio]\r\nurl = \"https://old.invalid/mcp\"\r\n[other]\r\nvalue = 7\r\n";

        let updated = upsert_codex_server_config(source, TEST_ACCESS_TOKEN).unwrap();
        let document = updated.parse::<DocumentMut>().unwrap();

        validate_codex_http_server(&document, TEST_ACCESS_TOKEN).unwrap();
        assert_eq!(
            document
                .get("other")
                .and_then(Item::as_table_like)
                .and_then(|table| table.get("value"))
                .and_then(Item::as_integer),
            Some(7)
        );
    }

    #[test]
    fn codex_toml_update_rejects_invalid_existing_document_before_write() {
        let error =
            upsert_codex_server_config("[broken\nvalue = true\n", TEST_ACCESS_TOKEN).unwrap_err();

        assert!(error.contains("nu este TOML valid"), "{error}");
    }

    #[test]
    fn codex_home_is_the_authoritative_config_root() {
        let path = codex_config_path_from_locations(
            Some(PathBuf::from("/tmp/codex-profile")),
            Some(PathBuf::from("/home/ignored")),
            None,
        )
        .unwrap();

        assert_eq!(path, PathBuf::from("/tmp/codex-profile/config.toml"));
    }

    #[test]
    fn codex_token_update_preserves_unrelated_http_headers() {
        let source = format!(
            "[mcp_servers.pana-studio]\nurl = \"{MCP_ENDPOINT}\"\nhttp_headers = {{ \"X-Existing\" = \"keep\", \"{CODEX_ACCESS_TOKEN_HEADER}\" = \"{}\" }}\n",
            "b".repeat(43)
        );

        let updated = upsert_codex_server_config(&source, TEST_ACCESS_TOKEN).unwrap();
        let document = updated.parse::<DocumentMut>().unwrap();
        let headers = codex_server_table(&document)
            .unwrap()
            .get("http_headers")
            .unwrap()
            .as_table_like()
            .unwrap();

        assert_eq!(
            headers.get("X-Existing").and_then(Item::as_str),
            Some("keep")
        );
        assert_eq!(
            headers
                .get(CODEX_ACCESS_TOKEN_HEADER)
                .and_then(Item::as_str),
            Some(TEST_ACCESS_TOKEN)
        );
    }

    #[test]
    fn malformed_http_headers_block_the_semantic_update() {
        let source = format!(
            "[mcp_servers.pana-studio]\nurl = \"{MCP_ENDPOINT}\"\nhttp_headers = \"not-a-map\"\n"
        );

        let error = upsert_codex_server_config(&source, TEST_ACCESS_TOKEN).unwrap_err();

        assert!(error.contains("http_headers"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn codex_token_requires_private_existing_config_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let root = unique_test_dir("codex-private-mode");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("config.toml");
        fs::write(&path, "title = \"test\"\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!codex_config_permissions_are_private(&path, true));

        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(codex_config_permissions_are_private(&path, true));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn codex_config_reader_rejects_oversized_regular_file_before_allocation() {
        let root = unique_test_dir("codex-config-limit");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("config.toml");
        let file = fs::File::create(&path).unwrap();
        file.set_len(MAX_CODEX_CONFIG_BYTES + 1).unwrap();

        let error = read_existing_codex_config_for_update(&path).unwrap_err();

        assert!(error.contains("depășește limita"));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn codex_config_reader_never_follows_symlink() {
        use std::os::unix::fs::symlink;

        let root = unique_test_dir("codex-config-symlink");
        fs::create_dir_all(&root).unwrap();
        let outside = root.join("outside.toml");
        let path = root.join("config.toml");
        fs::write(&outside, "secret = true\n").unwrap();
        symlink(&outside, &path).unwrap();

        let error = read_existing_codex_config_for_update(&path).unwrap_err();

        assert!(error.contains("este symlink"));
        fs::remove_dir_all(root).unwrap();
    }

    fn unique_test_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pana-studio-{label}-{nanos}"))
    }
}
