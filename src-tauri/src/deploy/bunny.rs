use reqwest::{blocking::Client, Url};
use std::{path::Path, time::Duration};

use super::{
    artifact::{build_deploy_artifact_manifest, DeployArtifactFile, DeployArtifactManifest},
    env::{env_require, read_env_from_root},
};

const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

pub fn deploy_project_to_bunny(
    project_root: &Path,
    zola_root: &Path,
    env_root: &Path,
) -> Result<String, String> {
    let client = bunny_client()?;
    let transport = ReqwestBunnyTransport { client };
    deploy_project_with_transport(
        project_root,
        zola_root,
        env_root,
        &transport,
        |credentials| BunnyEndpoints::production(&credentials.region),
    )
}

fn deploy_project_with_transport<T, F>(
    project_root: &Path,
    zola_root: &Path,
    env_root: &Path,
    transport: &T,
    endpoints: F,
) -> Result<String, String>
where
    T: BunnyTransport,
    F: FnOnce(&BunnyCredentials) -> Result<BunnyEndpoints, String>,
{
    // Ordering is a safety contract: the complete bounded/no-follow artifact
    // must exist in memory before the transport can receive an upload call.
    let manifest = build_deploy_artifact_manifest(project_root, zola_root)?;
    let credentials = BunnyCredentials::from_root(env_root)?;
    let endpoints = endpoints(&credentials)?;
    upload_manifest_and_purge(transport, endpoints, credentials, manifest)
}

#[derive(Debug)]
struct BunnyCredentials {
    zone: String,
    storage_key: String,
    region: String,
    pull_zone_id: String,
    cdn_key: String,
}

impl BunnyCredentials {
    fn from_root(root: &Path) -> Result<Self, String> {
        let env = read_env_from_root(root)?;
        Ok(Self {
            zone: env_require(&env, "BUNNY_STORAGE_ZONE")?,
            storage_key: env_require(&env, "BUNNY_STORAGE_KEY")?,
            region: env
                .get("BUNNY_STORAGE_REGION")
                .cloned()
                .unwrap_or_else(|| "de".to_string()),
            pull_zone_id: env_require(&env, "BUNNY_PULL_ZONE_ID")?,
            cdn_key: env_require(&env, "BUNNY_CDN_API_KEY")?,
        })
    }
}

#[derive(Clone, Debug)]
struct BunnyEndpoints {
    storage_base: Url,
    api_base: Url,
}

impl BunnyEndpoints {
    fn production(region: &str) -> Result<Self, String> {
        let host = storage_host(region)?;
        Ok(Self {
            storage_base: Url::parse(&format!("https://{host}/"))
                .map_err(|error| format!("Endpointul Bunny Storage este invalid: {error}."))?,
            api_base: Url::parse("https://api.bunny.net/")
                .map_err(|error| format!("Endpointul Bunny CDN este invalid: {error}."))?,
        })
    }
}

fn bunny_client() -> Result<Client, String> {
    Client::builder()
        .connect_timeout(HTTP_CONNECT_TIMEOUT)
        .timeout(HTTP_REQUEST_TIMEOUT)
        .build()
        .map_err(|error| format!("Clientul HTTP Bunny nu poate fi inițializat: {error}."))
}

trait BunnyTransport {
    #[allow(clippy::too_many_arguments)]
    fn upload(
        &self,
        url: Url,
        access_key: &str,
        content_type: &'static str,
        checksum: &str,
        bytes: Vec<u8>,
    ) -> Result<(), String>;

    fn purge(&self, url: Url, access_key: &str) -> Result<(), String>;
}

struct ReqwestBunnyTransport {
    client: Client,
}

impl BunnyTransport for ReqwestBunnyTransport {
    fn upload(
        &self,
        url: Url,
        access_key: &str,
        content_type: &'static str,
        checksum: &str,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        let response = self
            .client
            .put(url)
            .header("AccessKey", access_key)
            .header("Content-Type", content_type)
            .header("Checksum", checksum)
            .body(bytes)
            .send()
            .map_err(|error| format!("request-ul HTTP a eșuat: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "Bunny Storage a răspuns HTTP {}",
                response.status()
            ));
        }
        Ok(())
    }

    fn purge(&self, url: Url, access_key: &str) -> Result<(), String> {
        let response = self
            .client
            .post(url)
            .header("AccessKey", access_key)
            .header("Content-Length", "0")
            .send()
            .map_err(|error| format!("request-ul HTTP a eșuat: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("Bunny CDN a răspuns HTTP {}", response.status()));
        }
        Ok(())
    }
}

fn upload_manifest_and_purge<T: BunnyTransport>(
    transport: &T,
    endpoints: BunnyEndpoints,
    credentials: BunnyCredentials,
    manifest: DeployArtifactManifest,
) -> Result<String, String> {
    let total_files = manifest.files.len();
    let total_bytes = manifest.total_bytes;
    let artifact_root = manifest.root.display().to_string();
    let mut uploaded = 0usize;
    let mut log = String::new();

    for file in manifest.files {
        let remote_path = file.relative_path.clone();
        let url = storage_file_url(&endpoints.storage_base, &credentials.zone, &file)?;
        let content_type = mime_for_extension(Path::new(&file.relative_path));
        transport
            .upload(
                url,
                &credentials.storage_key,
                content_type,
                &file.sha256_uppercase,
                file.bytes,
            )
            .map_err(|error| {
                format!(
                    "Deploy Bunny oprit după {uploaded}/{total_files} uploaduri la {remote_path}: {error}. Cache-ul CDN nu a fost purjat."
                )
            })?;
        uploaded += 1;
        log.push_str(&format!("upload {remote_path}\n"));
    }

    purge_cdn_cache(
        transport,
        &endpoints.api_base,
        &credentials.pull_zone_id,
        &credentials.cdn_key,
    )?;
    log.push_str("CDN cache purged\n");
    Ok(format!(
        "Deploy complet: {uploaded} fișiere / {total_bytes} bytes din {artifact_root}; checksum SHA-256 verificat, purge CDN confirmat.\n\n{log}"
    ))
}

fn storage_file_url(
    storage_base: &Url,
    zone: &str,
    file: &DeployArtifactFile,
) -> Result<Url, String> {
    let mut url = storage_base.clone();
    let mut segments = url
        .path_segments_mut()
        .map_err(|_| "Endpointul Bunny Storage nu poate primi segmente de path.".to_string())?;
    segments.pop_if_empty().push(zone);
    for segment in file.relative_path.split('/') {
        segments.push(segment);
    }
    drop(segments);
    Ok(url)
}

fn purge_cdn_cache<T: BunnyTransport>(
    transport: &T,
    api_base: &Url,
    pull_zone_id: &str,
    cdn_key: &str,
) -> Result<(), String> {
    let mut url = api_base.clone();
    let mut segments = url
        .path_segments_mut()
        .map_err(|_| "Endpointul Bunny CDN nu poate primi segmente de path.".to_string())?;
    segments
        .pop_if_empty()
        .push("pullzone")
        .push(pull_zone_id)
        .push("purgeCache");
    drop(segments);

    transport.purge(url, cdn_key).map_err(|error| {
            format!(
                "Uploadurile au reușit, dar purge-ul CDN a eșuat: {error}. Deploy-ul nu este confirmat complet."
            )
        })?;
    Ok(())
}

fn storage_host(region: &str) -> Result<String, String> {
    let normalized = region.trim().to_ascii_lowercase();
    if !normalized
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err("BUNNY_STORAGE_REGION conține caractere invalide.".to_string());
    }
    Ok(match normalized.as_str() {
        "" | "de" => "storage.bunnycdn.com".to_string(),
        value => format!("{value}.storage.bunnycdn.com"),
    })
}

fn mime_for_extension(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("json") => "application/json",
        Some("xml") => "application/xml",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("txt") => "text/plain; charset=utf-8",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::Digest;
    use std::{
        cell::{Cell, RefCell},
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[derive(Default)]
    struct FakeTransport {
        uploads: RefCell<Vec<(String, String, Vec<u8>)>>,
        purge_calls: Cell<usize>,
        fail_upload: bool,
        fail_purge: bool,
    }

    impl BunnyTransport for FakeTransport {
        fn upload(
            &self,
            url: Url,
            _access_key: &str,
            _content_type: &'static str,
            checksum: &str,
            bytes: Vec<u8>,
        ) -> Result<(), String> {
            self.uploads
                .borrow_mut()
                .push((url.to_string(), checksum.to_string(), bytes));
            if self.fail_upload {
                Err("upload injectat eșuat".to_string())
            } else {
                Ok(())
            }
        }

        fn purge(&self, _url: Url, _access_key: &str) -> Result<(), String> {
            self.purge_calls.set(self.purge_calls.get() + 1);
            if self.fail_purge {
                Err("purge injectat eșuat".to_string())
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn checksum_header_value_is_uppercase_sha256() {
        let checksum = sha2::Sha256::digest(b"abc")
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<String>();
        assert_eq!(
            checksum,
            "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"
        );
    }

    #[test]
    fn storage_url_encodes_zone_and_artifact_segments() {
        let file = DeployArtifactFile {
            relative_path: "assets/a b.css".to_string(),
            bytes: Vec::new(),
            sha256_uppercase: String::new(),
        };
        let url = storage_file_url(
            &Url::parse("https://storage.bunnycdn.com/").unwrap(),
            "zone/name",
            &file,
        )
        .unwrap();
        assert_eq!(
            url.as_str(),
            "https://storage.bunnycdn.com/zone%2Fname/assets/a%20b.css"
        );
    }

    #[test]
    fn invalid_region_cannot_change_storage_host() {
        assert!(storage_host("de/path").is_err());
        assert_eq!(storage_host("DE").unwrap(), "storage.bunnycdn.com");
        assert_eq!(storage_host("ny").unwrap(), "ny.storage.bunnycdn.com");
    }

    #[cfg(unix)]
    #[test]
    fn artifact_preflight_failure_makes_zero_transport_calls() {
        use std::os::unix::fs::symlink;

        let root = deploy_fixture("zero-request");
        fs::create_dir_all(root.join("outside")).unwrap();
        symlink(root.join("outside"), root.join("export")).unwrap();
        let transport = FakeTransport::default();

        let error =
            deploy_project_with_transport(&root, &root.join("sursa"), &root, &transport, |_| {
                Ok(test_endpoints())
            })
            .unwrap_err();

        assert!(error.contains("symlink"));
        assert!(transport.uploads.borrow().is_empty());
        assert_eq!(transport.purge_calls.get(), 0);
        cleanup(root);
    }

    #[test]
    fn upload_failure_is_terminal_and_skips_purge() {
        let root = deploy_fixture("upload-failure");
        fs::create_dir_all(root.join("export")).unwrap();
        fs::write(root.join("export/index.html"), "payload").unwrap();
        let transport = FakeTransport {
            fail_upload: true,
            ..FakeTransport::default()
        };

        let error =
            deploy_project_with_transport(&root, &root.join("sursa"), &root, &transport, |_| {
                Ok(test_endpoints())
            })
            .unwrap_err();

        assert!(error.contains("nu a fost purjat"));
        assert_eq!(transport.uploads.borrow().len(), 1);
        assert_eq!(transport.purge_calls.get(), 0);
        cleanup(root);
    }

    #[test]
    fn successful_manifest_sends_uppercase_checksum_then_purges_once() {
        let root = deploy_fixture("checksum-purge");
        fs::create_dir_all(root.join("export")).unwrap();
        fs::write(root.join("export/index.html"), "abc").unwrap();
        let transport = FakeTransport::default();

        let result =
            deploy_project_with_transport(&root, &root.join("sursa"), &root, &transport, |_| {
                Ok(test_endpoints())
            })
            .unwrap();

        let uploads = transport.uploads.borrow();
        assert_eq!(uploads.len(), 1);
        assert_eq!(
            uploads[0].1,
            "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"
        );
        assert_eq!(uploads[0].2, b"abc");
        assert_eq!(transport.purge_calls.get(), 1);
        assert!(result.contains("purge CDN confirmat"));
        drop(uploads);
        cleanup(root);
    }

    #[test]
    fn purge_failure_is_terminal_after_successful_uploads() {
        let root = deploy_fixture("purge-failure");
        fs::create_dir_all(root.join("export")).unwrap();
        fs::write(root.join("export/index.html"), "payload").unwrap();
        let transport = FakeTransport {
            fail_purge: true,
            ..FakeTransport::default()
        };

        let error =
            deploy_project_with_transport(&root, &root.join("sursa"), &root, &transport, |_| {
                Ok(test_endpoints())
            })
            .unwrap_err();

        assert!(error.contains("nu este confirmat complet"));
        assert_eq!(transport.uploads.borrow().len(), 1);
        assert_eq!(transport.purge_calls.get(), 1);
        cleanup(root);
    }

    fn deploy_fixture(label: &str) -> PathBuf {
        let root = unique_temp_dir(label);
        fs::create_dir_all(root.join("sursa")).unwrap();
        fs::write(
            root.join("sursa/zola.toml"),
            "base_url = '/'\noutput_dir = '../export'\n",
        )
        .unwrap();
        fs::write(
            root.join(".env"),
            "BUNNY_STORAGE_ZONE=zone\nBUNNY_STORAGE_KEY=storage-key\nBUNNY_PULL_ZONE_ID=42\nBUNNY_CDN_API_KEY=cdn-key\n",
        )
        .unwrap();
        root.canonicalize().unwrap()
    }

    fn test_endpoints() -> BunnyEndpoints {
        BunnyEndpoints {
            storage_base: Url::parse("https://storage.invalid/").unwrap(),
            api_base: Url::parse("https://api.invalid/").unwrap(),
        }
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "panastudio-bunny-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn cleanup(path: PathBuf) {
        let _ = fs::remove_dir_all(path);
    }
}
