use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    fs,
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, RwLock,
    },
    thread::JoinHandle,
    time::Duration,
};

use percent_encoding::percent_decode_str;
use relative_path::RelativePathBuf;

use crate::preview::inject::{
    build_prepared_design_safe_response, PreparedDesignSafeHtml, PreviewHtmlSurface,
};
use crate::preview::{
    CanvasProjectionIdentity, CanvasProjectionPhase, CanvasProjectionPlan,
    CanvasProjectionTransaction, PreviewPhaseReceipt,
};

const MAX_CONCURRENT_CONNECTIONS: usize = 8;
const MAX_REQUEST_HEADER_BYTES: usize = 32 * 1024;
const CLIENT_READ_TIMEOUT: Duration = Duration::from_secs(3);
const CLIENT_WRITE_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_STAGED_GENERATIONS: usize = 8;
const MAX_RETIRED_GENERATIONS: usize = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RenderedPreviewContent {
    Html(PreparedDesignSafeHtml),
    Text { body: Vec<u8>, content_type: String },
}

#[derive(Clone, Debug)]
pub(crate) struct ActivePreviewGeneration {
    pub project_root: String,
    pub runtime_session_id: String,
    pub workspace_revision: u64,
    pub preview_revision: String,
    pub canvas_transaction: CanvasProjectionTransaction,
    pub content: HashMap<String, RenderedPreviewContent>,
    /// Vizualizările individuale de template aparțin strict acestei generații.
    /// Registrul este separat de site-ul canonic: publicarea unui Workbench nu
    /// poate înlocui sau falsifica ruta unei pagini Zola.
    pub workbench_content: Arc<RwLock<HashMap<String, RenderedPreviewContent>>>,
    pub assets_root: PathBuf,
}

impl ActivePreviewGeneration {
    pub fn owner_matches(&self, project_root: &str, runtime_session_id: &str) -> bool {
        self.project_root == project_root && self.runtime_session_id == runtime_session_id
    }
}

#[derive(Default)]
pub(crate) struct PreviewGenerationRegistry {
    active: Option<Arc<ActivePreviewGeneration>>,
    staged: BTreeMap<String, Arc<ActivePreviewGeneration>>,
    retired: VecDeque<Arc<ActivePreviewGeneration>>,
}

pub(crate) struct PreviewPhaseTransition {
    pub generation: Arc<ActivePreviewGeneration>,
    pub previous_active: Option<Arc<ActivePreviewGeneration>>,
    pub discarded: bool,
}

impl PreviewGenerationRegistry {
    pub(crate) fn active(&self) -> Option<Arc<ActivePreviewGeneration>> {
        self.active.clone()
    }

    fn resolve_workspace_revision(
        &self,
        project_root: &str,
        runtime_session_id: &str,
        workspace_revision: u64,
    ) -> Option<Arc<ActivePreviewGeneration>> {
        // A staged generation is the exact document the browser still has to
        // verify. Prefer it over an active generation with the same workspace
        // revision (for example after a forced reprojection).
        self.staged
            .values()
            .find(|generation| {
                generation.owner_matches(project_root, runtime_session_id)
                    && generation.workspace_revision == workspace_revision
            })
            .cloned()
            .or_else(|| {
                self.active
                    .as_ref()
                    .filter(|generation| {
                        generation.owner_matches(project_root, runtime_session_id)
                            && generation.workspace_revision == workspace_revision
                    })
                    .cloned()
            })
    }

    fn resolve(&self, preview_revision: Option<&str>) -> Option<Arc<ActivePreviewGeneration>> {
        let Some(preview_revision) = preview_revision else {
            return self.active();
        };
        self.active
            .as_ref()
            .filter(|generation| generation.preview_revision == preview_revision)
            .cloned()
            .or_else(|| {
                self.staged
                    .values()
                    .find(|generation| generation.preview_revision == preview_revision)
                    .cloned()
            })
    }

    fn resolve_identity(
        &self,
        identity: &CanvasProjectionIdentity,
    ) -> Option<Arc<ActivePreviewGeneration>> {
        self.active
            .as_ref()
            .filter(|generation| generation.canvas_transaction.identity == *identity)
            .cloned()
            .or_else(|| {
                self.staged
                    .get(&identity.transaction_id)
                    .filter(|generation| generation.canvas_transaction.identity == *identity)
                    .cloned()
            })
    }

    fn stage(
        &mut self,
        candidate: Arc<ActivePreviewGeneration>,
    ) -> Vec<Arc<ActivePreviewGeneration>> {
        let owner_root = candidate.project_root.clone();
        let owner_session = candidate.runtime_session_id.clone();
        let transaction_id = candidate.canvas_transaction.identity.transaction_id.clone();
        let stale_keys = self
            .staged
            .iter()
            .filter(|(_, generation)| {
                generation.owner_matches(&owner_root, &owner_session)
                    && generation.canvas_transaction.identity.transaction_id != transaction_id
            })
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        let mut evicted = stale_keys
            .into_iter()
            .filter_map(|key| self.staged.remove(&key))
            .collect::<Vec<_>>();
        self.staged.insert(transaction_id, candidate);
        while self.staged.len() > MAX_STAGED_GENERATIONS {
            let Some(key) = self.staged.keys().next().cloned() else {
                break;
            };
            if let Some(generation) = self.staged.remove(&key) {
                evicted.push(generation);
            }
        }
        evicted
    }

    fn publish(
        &mut self,
        candidate: Arc<ActivePreviewGeneration>,
    ) -> Option<Arc<ActivePreviewGeneration>> {
        self.staged
            .remove(&candidate.canvas_transaction.identity.transaction_id);
        let previous = self.active.replace(candidate);
        if let Some(previous) = previous.as_ref() {
            self.retired.push_back(Arc::clone(previous));
        }
        while self.retired.len() > MAX_RETIRED_GENERATIONS {
            self.retired.pop_front();
        }
        previous
    }

    fn acknowledge_phase(
        &mut self,
        receipt: &PreviewPhaseReceipt,
    ) -> Result<PreviewPhaseTransition, String> {
        let transaction_id = &receipt.identity.transaction_id;
        let candidate = self.staged.get(transaction_id).cloned().ok_or_else(|| {
            format!(
                "Canvas Runtime nu mai are candidatul staged pentru tranzacția {transaction_id}."
            )
        })?;
        let mut updated = (*candidate).clone();
        updated.canvas_transaction = candidate.canvas_transaction.accept_phase_receipt(receipt)?;

        if receipt.phase == CanvasProjectionPhase::Failed {
            let failed = Arc::new(updated);
            self.staged.remove(transaction_id);
            return Ok(PreviewPhaseTransition {
                generation: failed,
                previous_active: None,
                discarded: true,
            });
        }

        if receipt.phase == CanvasProjectionPhase::StyledReady {
            let canonical_elapsed = updated
                .canvas_transaction
                .phase_timings_ms
                .get("styledReady")
                .copied()
                .unwrap_or_default();
            updated
                .canvas_transaction
                .transition_to(CanvasProjectionPhase::CanonicalVerified, canonical_elapsed)?;
            let canonical = Arc::new(updated);
            self.staged.remove(transaction_id);
            let previous_active = self.publish(Arc::clone(&canonical));
            return Ok(PreviewPhaseTransition {
                generation: canonical,
                previous_active,
                discarded: false,
            });
        }

        let staged = Arc::new(updated);
        self.staged
            .insert(transaction_id.to_string(), Arc::clone(&staged));
        Ok(PreviewPhaseTransition {
            generation: staged,
            previous_active: None,
            discarded: false,
        })
    }

    fn clear(&mut self) {
        self.active = None;
        self.staged.clear();
        self.retired.clear();
    }
}

pub(crate) type ActivePreviewStore = Arc<RwLock<PreviewGenerationRegistry>>;

pub(crate) struct PersistentPreviewServer {
    port: u16,
    stop_flag: Arc<AtomicBool>,
    active: ActivePreviewStore,
    thread: Option<JoinHandle<()>>,
}

impl PersistentPreviewServer {
    pub fn start() -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|error| format!("Nu am putut porni serverul Preview persistent: {error}"))?;
        let port = listener
            .local_addr()
            .map_err(|error| format!("Nu am putut citi portul Preview persistent: {error}"))?
            .port();
        listener.set_nonblocking(true).map_err(|error| {
            format!("Nu am putut configura serverul Preview persistent: {error}")
        })?;
        let stop_flag = Arc::new(AtomicBool::new(false));
        let active = Arc::new(RwLock::new(PreviewGenerationRegistry::default()));
        let thread = spawn_server(listener, Arc::clone(&stop_flag), Arc::clone(&active), port)?;
        Ok(Self {
            port,
            stop_flag,
            active,
            thread: Some(thread),
        })
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn active(&self) -> Result<Option<Arc<ActivePreviewGeneration>>, String> {
        self.active
            .read()
            .map_err(|_| "Snapshot-ul activ Preview este indisponibil.".to_string())
            .map(|registry| registry.active())
    }

    pub fn generation_for_workspace_revision(
        &self,
        project_root: &str,
        runtime_session_id: &str,
        workspace_revision: u64,
    ) -> Result<Option<Arc<ActivePreviewGeneration>>, String> {
        self.active
            .read()
            .map_err(|_| "Snapshot-ul Preview persistent este indisponibil.".to_string())
            .map(|registry| {
                registry.resolve_workspace_revision(
                    project_root,
                    runtime_session_id,
                    workspace_revision,
                )
            })
    }

    pub fn canvas_plan_for_identity(
        &self,
        identity: &CanvasProjectionIdentity,
    ) -> Result<Option<CanvasProjectionPlan>, String> {
        self.active
            .read()
            .map_err(|_| "Snapshot-ul Preview persistent este indisponibil.".to_string())
            .map(|registry| {
                registry
                    .resolve_identity(identity)
                    .map(|generation| generation.canvas_transaction.plan())
            })
    }

    pub fn stage(
        &self,
        candidate: Arc<ActivePreviewGeneration>,
    ) -> Result<Vec<Arc<ActivePreviewGeneration>>, String> {
        let mut registry = self
            .active
            .write()
            .map_err(|_| "Nu am putut stage-ui generația Preview persistentă.".to_string())?;
        Ok(registry.stage(candidate))
    }

    pub fn acknowledge_phase(
        &self,
        receipt: &PreviewPhaseReceipt,
    ) -> Result<PreviewPhaseTransition, String> {
        self.active
            .write()
            .map_err(|_| "Nu am putut avansa faza generației Preview persistente.".to_string())?
            .acknowledge_phase(receipt)
    }

    pub fn clear(&self) {
        if let Ok(mut active) = self.active.write() {
            active.clear();
        }
    }

    pub fn stop(mut self) {
        self.stop_inner();
    }

    fn stop_inner(&mut self) {
        self.clear();
        self.stop_flag.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for PersistentPreviewServer {
    fn drop(&mut self) {
        self.stop_inner();
    }
}

fn spawn_server(
    listener: TcpListener,
    stop_flag: Arc<AtomicBool>,
    active: ActivePreviewStore,
    port: u16,
) -> Result<JoinHandle<()>, String> {
    std::thread::Builder::new()
        .name("pana-persistent-preview".to_string())
        .spawn(move || {
            let active_connections = Arc::new(AtomicUsize::new(0));
            while !stop_flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        if active_connections
                            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                                (count < MAX_CONCURRENT_CONNECTIONS).then_some(count + 1)
                            })
                            .is_err()
                        {
                            write_overload(stream);
                            continue;
                        }
                        let active = Arc::clone(&active);
                        let connections = Arc::clone(&active_connections);
                        if std::thread::Builder::new()
                            .name("pana-persistent-preview-request".to_string())
                            .spawn(move || {
                                let _lease = ConnectionLease(connections);
                                handle_connection(stream, &active, port);
                            })
                            .is_err()
                        {
                            active_connections.fetch_sub(1, Ordering::AcqRel);
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(15));
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(15)),
                }
            }
        })
        .map_err(|error| format!("Nu am putut porni thread-ul Preview persistent: {error}"))
}

struct ConnectionLease(Arc<AtomicUsize>);

impl Drop for ConnectionLease {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

fn handle_connection(mut stream: TcpStream, active: &ActivePreviewStore, port: u16) {
    let _ = stream.set_read_timeout(Some(CLIENT_READ_TIMEOUT));
    let _ = stream.set_write_timeout(Some(CLIENT_WRITE_TIMEOUT));
    let response = serve_request(&mut stream, active, port).unwrap_or_else(error_response);
    let _ = stream.write_all(&response);
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Both);
}

fn serve_request(
    reader: &mut impl Read,
    active: &ActivePreviewStore,
    port: u16,
) -> Result<Vec<u8>, String> {
    let request = read_request(reader)?;
    let (method, request_target) = parse_request_line(&request)
        .ok_or_else(|| "Cererea Preview persistent nu are request-line valid.".to_string())?;
    if !matches!(method.as_str(), "GET" | "HEAD") {
        return Ok(plain_response(
            "HTTP/1.1 405 Method Not Allowed",
            "text/plain; charset=utf-8",
            b"Method Not Allowed",
            method == "HEAD",
        ));
    }

    let head_only = method == "HEAD";
    let surface = request_surface(&request_target);
    let public_target = public_request_target(&request_target);
    let decoded = percent_decode_str(public_target.split('?').next().unwrap_or("/"))
        .decode_utf8()
        .map_err(|_| "Path-ul Preview persistent nu este UTF-8 valid.".to_string())?;
    let content_key = zola_content_key(&decoded)?;
    let requested_revision = requested_preview_revision(&request_target)?.or_else(|| {
        request_header(&request, "referer")
            .and_then(|referer| requested_preview_revision(referer).ok().flatten())
    });
    let generation = active
        .read()
        .map_err(|_| "Snapshot-ul Preview persistent este indisponibil.".to_string())?
        .resolve(requested_revision.as_deref());
    let Some(generation) = generation else {
        return Ok(plain_response(
            "HTTP/1.1 503 Service Unavailable",
            "text/plain; charset=utf-8",
            b"Preview not published",
            head_only,
        ));
    };
    if surface == PreviewHtmlSurface::Interactive {
        let requested_transaction = requested_canvas_transaction(&request_target)?;
        if requested_transaction.as_deref()
            != Some(
                generation
                    .canvas_transaction
                    .identity
                    .transaction_id
                    .as_str(),
            )
        {
            return Ok(plain_response(
                "HTTP/1.1 409 Conflict",
                "text/plain; charset=utf-8",
                b"Interactive Canvas identity mismatch",
                head_only,
            ));
        }
    }
    let exact_revision_lease =
        requested_revision.as_deref() == Some(generation.preview_revision.as_str());

    if decoded.starts_with("/__pana_workbench/") {
        let workbench = generation
            .workbench_content
            .read()
            .map_err(|_| "Registrul Template Workbench este indisponibil.".to_string())?;
        if let Some(content) = workbench.get(decoded.as_ref()) {
            return render_content(content, "HTTP/1.1 200 OK", surface, port, head_only);
        }
        return Ok(plain_response(
            "HTTP/1.1 404 Not Found",
            "text/plain; charset=utf-8",
            b"Template Workbench view not published for this preview revision",
            head_only,
        ));
    }

    if let Some(content) = generation.content.get(&content_key) {
        return render_content(content, "HTTP/1.1 200 OK", surface, port, head_only);
    }

    if let Some(asset) = read_asset(&generation.assets_root, &decoded)? {
        let etag = exact_revision_lease
            .then(|| resource_etag(&generation, &decoded))
            .flatten();
        return Ok(asset_response(
            &asset.content_type,
            &asset.body,
            head_only,
            etag.as_deref(),
            request_header(&request, "if-none-match"),
        ));
    }

    if let Some(content) = generation.content.get("404.html") {
        return render_content(content, "HTTP/1.1 404 Not Found", surface, port, head_only);
    }
    Ok(plain_response(
        "HTTP/1.1 404 Not Found",
        "text/plain; charset=utf-8",
        b"Not Found",
        head_only,
    ))
}

fn requested_preview_revision(request_target: &str) -> Result<Option<String>, String> {
    requested_internal_identity(request_target, "__pana_preview_revision", "previewRevision")
}

fn requested_canvas_transaction(request_target: &str) -> Result<Option<String>, String> {
    requested_internal_identity(
        request_target,
        "__pana_canvas_transaction",
        "canvasTransactionId",
    )
}

fn requested_internal_identity(
    request_target: &str,
    expected_name: &str,
    public_label: &str,
) -> Result<Option<String>, String> {
    let Some(query) = request_target.split_once('?').map(|(_, query)| query) else {
        return Ok(None);
    };
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        if name != expected_name {
            continue;
        }
        let decoded = percent_decode_str(value)
            .decode_utf8()
            .map_err(|_| format!("{public_label} din URL nu este UTF-8 valid."))?;
        if decoded.is_empty()
            || decoded.len() > 256
            || decoded.chars().any(|character| {
                !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | ':'))
            })
        {
            return Err(format!("{public_label} din URL este invalid."));
        }
        return Ok(Some(decoded.into_owned()));
    }
    Ok(None)
}

fn request_header<'a>(request: &'a [u8], expected_name: &str) -> Option<&'a str> {
    let request = std::str::from_utf8(request).ok()?;
    request.lines().skip(1).find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case(expected_name)
            .then_some(value.trim())
    })
}

fn render_content(
    content: &RenderedPreviewContent,
    status: &str,
    surface: PreviewHtmlSurface,
    port: u16,
    head_only: bool,
) -> Result<Vec<u8>, String> {
    let response = match content {
        RenderedPreviewContent::Html(html) => build_prepared_design_safe_response(
            status,
            "text/html; charset=utf-8",
            match surface {
                PreviewHtmlSurface::Editor => &html.editor,
                PreviewHtmlSurface::Visitor => &html.visitor,
                PreviewHtmlSurface::Interactive => &html.interactive,
            },
            surface,
            port,
        )?,
        RenderedPreviewContent::Text { body, content_type } => {
            plain_response(status, content_type, body, false)
        }
    };
    Ok(if head_only {
        response_without_body(response)
    } else {
        response
    })
}

struct AssetResponse {
    body: Vec<u8>,
    content_type: String,
}

fn read_asset(root: &Path, decoded_path: &str) -> Result<Option<AssetResponse>, String> {
    let relative = safe_asset_relative_path(decoded_path)?;
    if relative.as_os_str().is_empty() {
        return Ok(None);
    }
    let canonical_root = root.canonicalize().map_err(|error| {
        format!(
            "Directorul de asset-uri Preview {} este indisponibil: {error}",
            root.display()
        )
    })?;
    let requested = root.join(relative);
    let mut canonical = match requested.canonicalize() {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "Nu am putut rezolva asset-ul Preview {}: {error}",
                requested.display()
            ))
        }
    };
    if !canonical.starts_with(&canonical_root) {
        return Err("Preview-ul persistent a refuzat un asset în afara generației.".to_string());
    }
    if canonical.is_dir() {
        canonical.push("index.html");
    }
    if !canonical.is_file() {
        return Ok(None);
    }
    let body = fs::read(&canonical).map_err(|error| {
        format!(
            "Nu am putut citi asset-ul Preview {}: {error}",
            canonical.display()
        )
    })?;
    let content_type = mime_guess::from_path(&canonical)
        .first_or_octet_stream()
        .essence_str()
        .to_string();
    Ok(Some(AssetResponse { body, content_type }))
}

fn safe_asset_relative_path(decoded: &str) -> Result<PathBuf, String> {
    let trimmed = decoded.trim_start_matches('/');
    let path = Path::new(trimmed);
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err("Preview-ul persistent a refuzat un path de asset nesigur.".to_string());
        }
    }
    Ok(path.to_path_buf())
}

fn zola_content_key(decoded_path: &str) -> Result<String, String> {
    if !decoded_path.starts_with('/') {
        return Err("Path-ul Preview persistent trebuie să fie absolut HTTP.".to_string());
    }
    let mut path = RelativePathBuf::new();
    for component in decoded_path.split('/') {
        path.push(component);
    }
    Ok(path.as_str().to_string())
}

fn request_surface(target: &str) -> PreviewHtmlSurface {
    let query = target.split_once('?').map(|(_, query)| query).unwrap_or("");
    if query.split('&').any(is_interactive_query) {
        PreviewHtmlSurface::Interactive
    } else if query.split('&').any(is_visitor_query) {
        PreviewHtmlSurface::Visitor
    } else {
        PreviewHtmlSurface::Editor
    }
}

fn public_request_target(target: &str) -> String {
    let Some((route, query)) = target.split_once('?') else {
        return target.to_string();
    };
    let query = query
        .split('&')
        .filter(|part| !is_internal_query(part))
        .collect::<Vec<_>>()
        .join("&");
    if query.is_empty() {
        route.to_string()
    } else {
        format!("{route}?{query}")
    }
}

fn is_visitor_query(part: &str) -> bool {
    matches!(
        part,
        "__pana_plain" | "__pana_plain=1" | "__pana_plain=true" | "__pana_view=visitor"
    )
}

fn is_interactive_query(part: &str) -> bool {
    matches!(
        part,
        "__pana_interactive" | "__pana_interactive=1" | "__pana_view=interactive"
    )
}

fn is_internal_query(part: &&str) -> bool {
    if is_visitor_query(part) || is_interactive_query(part) || *part == "__pana_view=design-safe" {
        return true;
    }
    matches!(
        part.split_once('=').map(|(name, _)| name).unwrap_or(part),
        "__pana_preview_revision"
            | "__pana_canvas_transaction"
            | "__pana_reload"
            | "__pana_interactive_restart"
    )
}

fn read_request(reader: &mut impl Read) -> Result<Vec<u8>, String> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("Nu am putut citi cererea Preview: {error}"))?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        if request.len() > MAX_REQUEST_HEADER_BYTES {
            return Err("Headerul cererii Preview este prea mare.".to_string());
        }
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    Ok(request)
}

fn parse_request_line(request: &[u8]) -> Option<(String, String)> {
    let first = String::from_utf8_lossy(request).lines().next()?.to_string();
    let mut parts = first.split_whitespace();
    let method = parts.next()?;
    let target = parts.next()?;
    let version = parts.next()?;
    if parts.next().is_some()
        || !version.starts_with("HTTP/")
        || !target.starts_with('/')
        || target.contains(['\r', '\n'])
    {
        return None;
    }
    Some((method.to_string(), target.to_string()))
}

fn plain_response(status: &str, content_type: &str, body: &[u8], head_only: bool) -> Vec<u8> {
    let headers = format!(
        "{status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let mut response = headers.into_bytes();
    if !head_only {
        response.extend_from_slice(body);
    }
    response
}

fn resource_etag(generation: &ActivePreviewGeneration, decoded_path: &str) -> Option<String> {
    let normalized = format!("/{}", decoded_path.trim_start_matches('/'));
    generation
        .canvas_transaction
        .resources
        .entries
        .iter()
        .find(|entry| entry.url == normalized)
        .map(|entry| format!("\"{}\"", entry.content_hash))
}

fn asset_response(
    content_type: &str,
    body: &[u8],
    head_only: bool,
    etag: Option<&str>,
    if_none_match: Option<&str>,
) -> Vec<u8> {
    let Some(etag) = etag else {
        return plain_response("HTTP/1.1 200 OK", content_type, body, head_only);
    };
    let not_modified = if_none_match.is_some_and(|header| {
        header
            .split(',')
            .map(str::trim)
            .any(|candidate| candidate == "*" || candidate == etag)
    });
    let status = if not_modified {
        "HTTP/1.1 304 Not Modified"
    } else {
        "HTTP/1.1 200 OK"
    };
    let content_length = if not_modified { 0 } else { body.len() };
    let headers = format!(
        "{status}\r\nContent-Type: {content_type}\r\nContent-Length: {content_length}\r\nETag: {etag}\r\nCache-Control: public, max-age=31536000, immutable\r\nX-Content-Type-Options: nosniff\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n"
    );
    let mut response = headers.into_bytes();
    if !not_modified && !head_only {
        response.extend_from_slice(body);
    }
    response
}

fn response_without_body(mut response: Vec<u8>) -> Vec<u8> {
    if let Some(index) = response.windows(4).position(|window| window == b"\r\n\r\n") {
        response.truncate(index + 4);
    }
    response
}

fn error_response(message: String) -> Vec<u8> {
    plain_response(
        "HTTP/1.1 500 Internal Server Error",
        "text/plain; charset=utf-8",
        message.as_bytes(),
        false,
    )
}

fn write_overload(mut stream: TcpStream) {
    let response = plain_response(
        "HTTP/1.1 503 Service Unavailable",
        "text/plain; charset=utf-8",
        b"Preview busy",
        false,
    );
    let _ = stream.write_all(&response);
    let _ = stream.shutdown(Shutdown::Both);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request_preview(active: &ActivePreviewStore, target: &str) -> String {
        let request = format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1:43210\r\n\r\n");
        let mut reader = std::io::Cursor::new(request.into_bytes());
        String::from_utf8(serve_request(&mut reader, active, 43210).unwrap()).unwrap()
    }

    #[test]
    fn route_keys_follow_zola_memory_map_semantics() {
        assert_eq!(zola_content_key("/").unwrap(), "");
        assert_eq!(zola_content_key("/despre/").unwrap(), "despre/");
        assert_eq!(zola_content_key("/atom.xml").unwrap(), "atom.xml");
    }

    #[test]
    fn internal_queries_never_reach_project_routes() {
        assert_eq!(
            public_request_target(
                "/despre/?x=1&__pana_view=visitor&__pana_preview_revision=r1&__pana_canvas_transaction=tx1&__pana_reload=4"
            ),
            "/despre/?x=1"
        );
        assert_eq!(
            request_surface("/?__pana_plain=1"),
            PreviewHtmlSurface::Visitor
        );
        assert_eq!(
            request_surface("/?__pana_view=interactive"),
            PreviewHtmlSurface::Interactive
        );
        assert_eq!(
            requested_canvas_transaction(
                "/?__pana_view=interactive&__pana_canvas_transaction=canvas_tx_123"
            )
            .unwrap()
            .as_deref(),
            Some("canvas_tx_123")
        );
        assert!(requested_canvas_transaction(
            "/?__pana_view=interactive&__pana_canvas_transaction=../foreign"
        )
        .is_err());
    }

    #[test]
    fn interactive_surface_requires_exact_canvas_transaction() {
        let active: ActivePreviewStore =
            Arc::new(RwLock::new(PreviewGenerationRegistry::default()));
        let transaction = CanvasProjectionTransaction::test_fixture(11, "preview-11");
        let expected_transaction = transaction.identity.transaction_id.clone();
        let prepared = crate::preview::inject::prepare_design_safe_html(
            "<!doctype html><html><body><script>window.projectJs=true</script></body></html>",
            "preview-11",
        )
        .unwrap();
        let mut content = HashMap::new();
        content.insert("".to_string(), RenderedPreviewContent::Html(prepared));
        active
            .write()
            .unwrap()
            .stage(Arc::new(ActivePreviewGeneration {
                project_root: "/project".to_string(),
                runtime_session_id: "runtime".to_string(),
                workspace_revision: 11,
                preview_revision: "preview-11".to_string(),
                canvas_transaction: transaction,
                content,
                workbench_content: Arc::new(RwLock::new(HashMap::new())),
                assets_root: std::env::temp_dir(),
            }));

        let missing = request_preview(
            &active,
            "/?__pana_view=interactive&__pana_preview_revision=preview-11",
        );
        assert!(missing.starts_with("HTTP/1.1 409 Conflict"));
        let foreign = request_preview(
            &active,
            "/?__pana_view=interactive&__pana_preview_revision=preview-11&__pana_canvas_transaction=foreign",
        );
        assert!(foreign.starts_with("HTTP/1.1 409 Conflict"));
        let exact = request_preview(
            &active,
            &format!(
                "/?__pana_view=interactive&__pana_preview_revision=preview-11&__pana_canvas_transaction={expected_transaction}"
            ),
        );
        assert!(exact.starts_with("HTTP/1.1 200 OK"));
    }

    #[test]
    fn workbench_route_is_revision_scoped_and_preserves_preview_surfaces() {
        let active: ActivePreviewStore =
            Arc::new(RwLock::new(PreviewGenerationRegistry::default()));
        let transaction = CanvasProjectionTransaction::test_fixture(17, "preview-17");
        let expected_transaction = transaction.identity.transaction_id.clone();
        let prepared = crate::preview::inject::prepare_design_safe_html(
            concat!(
                "<!doctype html><html><head>",
                "<link rel=\"stylesheet\" href=\"/site.css\">",
                "</head><body>",
                "<main data-pana-workbench-active-source=\"source-partial\">Partial</main>",
                "<script src=\"/site.js\"></script>",
                "</body></html>",
            ),
            "preview-17",
        )
        .unwrap();
        let route = "/__pana_workbench/source-partial/".to_string();
        let workbench_content = Arc::new(RwLock::new(HashMap::from([(
            route.clone(),
            RenderedPreviewContent::Html(prepared),
        )])));
        active
            .write()
            .unwrap()
            .stage(Arc::new(ActivePreviewGeneration {
                project_root: "/project".to_string(),
                runtime_session_id: "runtime".to_string(),
                workspace_revision: 17,
                preview_revision: "preview-17".to_string(),
                canvas_transaction: transaction,
                content: HashMap::new(),
                workbench_content,
                assets_root: std::env::temp_dir(),
            }));

        let missing_revision = request_preview(&active, &route);
        assert!(missing_revision.starts_with("HTTP/1.1 503 Service Unavailable"));

        let stale_revision = request_preview(
            &active,
            &format!("{route}?__pana_preview_revision=preview-16"),
        );
        assert!(stale_revision.starts_with("HTTP/1.1 503 Service Unavailable"));

        let editor = request_preview(
            &active,
            &format!("{route}?__pana_preview_revision=preview-17"),
        );
        assert!(editor.starts_with("HTTP/1.1 200 OK"));
        assert!(editor.contains("/site.css"));
        assert!(editor.contains("data-pana-workbench-active-source"));
        assert!(!editor.contains("/site.js"));

        let interactive = request_preview(
            &active,
            &format!(
                "{route}?__pana_view=interactive&__pana_preview_revision=preview-17&__pana_canvas_transaction={expected_transaction}"
            ),
        );
        assert!(interactive.starts_with("HTTP/1.1 200 OK"));
        assert!(interactive.contains("/site.css"));
        assert!(interactive.contains("/site.js"));
    }

    #[test]
    fn active_generation_is_replaced_as_one_arc() {
        let active: ActivePreviewStore =
            Arc::new(RwLock::new(PreviewGenerationRegistry::default()));
        let generation = |revision: u64| {
            Arc::new(ActivePreviewGeneration {
                project_root: "/project".to_string(),
                runtime_session_id: "runtime".to_string(),
                workspace_revision: revision,
                preview_revision: format!("preview-{revision}"),
                canvas_transaction: CanvasProjectionTransaction::test_fixture(
                    revision,
                    &format!("preview-{revision}"),
                ),
                content: HashMap::new(),
                workbench_content: Arc::new(RwLock::new(HashMap::new())),
                assets_root: std::env::temp_dir(),
            })
        };
        let acknowledge_all =
            |registry: &mut PreviewGenerationRegistry,
             generation: &Arc<ActivePreviewGeneration>| {
                let mut transition = None;
                for (phase, timings) in [
                    (
                        CanvasProjectionPhase::ResourcesReady,
                        BTreeMap::from([("resourcesReady".to_string(), 1)]),
                    ),
                    (
                        CanvasProjectionPhase::Committed,
                        BTreeMap::from([
                            ("resourcesReady".to_string(), 1),
                            ("committed".to_string(), 2),
                        ]),
                    ),
                    (
                        CanvasProjectionPhase::StyledReady,
                        BTreeMap::from([
                            ("resourcesReady".to_string(), 1),
                            ("committed".to_string(), 2),
                            ("styledReady".to_string(), 3),
                        ]),
                    ),
                ] {
                    transition = Some(
                        registry
                            .acknowledge_phase(&PreviewPhaseReceipt {
                                schema_version:
                                    crate::preview::canvas::CANVAS_PROJECTION_SCHEMA_VERSION,
                                identity: generation.canvas_transaction.identity.clone(),
                                phase,
                                phase_timings_ms: timings,
                                diagnostic: None,
                            })
                            .unwrap(),
                    );
                }
                transition.unwrap()
            };
        let first = generation(1);
        let second = generation(2);
        let mut registry = active.write().unwrap();
        registry.stage(Arc::clone(&first));
        assert!(acknowledge_all(&mut registry, &first)
            .previous_active
            .is_none());
        registry.stage(Arc::clone(&second));
        let previous = acknowledge_all(&mut registry, &second)
            .previous_active
            .unwrap();
        assert_eq!(previous.workspace_revision, 1);
        assert_eq!(registry.active.as_ref().unwrap().workspace_revision, 2);
    }

    #[test]
    fn failed_candidate_is_discarded_without_replacing_the_active_generation() {
        let generation = |revision: u64| {
            Arc::new(ActivePreviewGeneration {
                project_root: "/project".to_string(),
                runtime_session_id: "runtime".to_string(),
                workspace_revision: revision,
                preview_revision: format!("preview-{revision}"),
                canvas_transaction: CanvasProjectionTransaction::test_fixture(
                    revision,
                    &format!("preview-{revision}"),
                ),
                content: HashMap::new(),
                workbench_content: Arc::new(RwLock::new(HashMap::new())),
                assets_root: std::env::temp_dir(),
            })
        };
        let mut registry = PreviewGenerationRegistry::default();
        let active = generation(1);
        registry.active = Some(Arc::clone(&active));
        let candidate = generation(2);
        registry.stage(Arc::clone(&candidate));

        let transition = registry
            .acknowledge_phase(&PreviewPhaseReceipt {
                schema_version: crate::preview::canvas::CANVAS_PROJECTION_SCHEMA_VERSION,
                identity: candidate.canvas_transaction.identity.clone(),
                phase: CanvasProjectionPhase::Failed,
                phase_timings_ms: BTreeMap::from([("failed".to_string(), 4)]),
                diagnostic: Some("stylesheet failed".to_string()),
            })
            .unwrap();

        assert!(transition.discarded);
        assert_eq!(
            transition.generation.canvas_transaction.phase,
            CanvasProjectionPhase::Failed
        );
        assert!(registry.staged.is_empty());
        assert_eq!(registry.active.as_ref().unwrap().workspace_revision, 1);
    }

    #[test]
    fn staged_generation_is_resolved_only_by_exact_preview_revision() {
        let mut registry = PreviewGenerationRegistry::default();
        let generation = Arc::new(ActivePreviewGeneration {
            project_root: "/project".to_string(),
            runtime_session_id: "runtime".to_string(),
            workspace_revision: 9,
            preview_revision: "preview-9".to_string(),
            canvas_transaction: CanvasProjectionTransaction::test_fixture(9, "preview-9"),
            content: HashMap::new(),
            workbench_content: Arc::new(RwLock::new(HashMap::new())),
            assets_root: std::env::temp_dir(),
        });
        registry.stage(Arc::clone(&generation));
        assert!(registry.resolve(None).is_none());
        assert!(registry.resolve(Some("preview-8")).is_none());
        assert_eq!(
            registry
                .resolve(Some("preview-9"))
                .unwrap()
                .workspace_revision,
            9
        );

        let exact = generation.canvas_transaction.identity.clone();
        assert!(registry.resolve_identity(&exact).is_some());
        for foreign in [
            CanvasProjectionIdentity {
                project_root: "/foreign".to_string(),
                ..exact.clone()
            },
            CanvasProjectionIdentity {
                runtime_session_id: "foreign-runtime".to_string(),
                ..exact.clone()
            },
            CanvasProjectionIdentity {
                workspace_revision: exact.workspace_revision + 1,
                ..exact.clone()
            },
            CanvasProjectionIdentity {
                transaction_id: "canvas_foreign".to_string(),
                ..exact.clone()
            },
            CanvasProjectionIdentity {
                preview_revision: "preview-foreign".to_string(),
                ..exact.clone()
            },
        ] {
            assert!(registry.resolve_identity(&foreign).is_none());
        }
    }

    #[test]
    fn unsafe_asset_paths_are_rejected_before_filesystem_access() {
        for path in ["/../secret", "/./asset.css"] {
            assert!(safe_asset_relative_path(path).is_err(), "{path}");
        }
        assert_eq!(
            safe_asset_relative_path("/css/site.css").unwrap(),
            PathBuf::from("css/site.css")
        );
    }

    #[test]
    fn revisioned_assets_are_immutable_and_support_conditional_requests() {
        let etag = "\"sha256-deadbeef\"";
        let first = String::from_utf8(asset_response(
            "text/css",
            b"body{}",
            false,
            Some(etag),
            None,
        ))
        .unwrap();
        assert!(first.starts_with("HTTP/1.1 200 OK"));
        assert!(first.contains("Cache-Control: public, max-age=31536000, immutable"));
        assert!(first.contains("ETag: \"sha256-deadbeef\""));
        assert!(first.ends_with("body{}"));

        let cached = String::from_utf8(asset_response(
            "text/css",
            b"body{}",
            false,
            Some(etag),
            Some(etag),
        ))
        .unwrap();
        assert!(cached.starts_with("HTTP/1.1 304 Not Modified"));
        assert!(cached.contains("Content-Length: 0"));
        assert!(!cached.ends_with("body{}"));

        let unversioned =
            String::from_utf8(asset_response("text/css", b"body{}", false, None, None)).unwrap();
        assert!(unversioned.contains("Cache-Control: no-store"));
        assert!(!unversioned.contains("ETag:"));
    }
}
