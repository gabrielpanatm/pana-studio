use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Condvar, Mutex,
    },
    thread::JoinHandle,
    time::Duration,
};

use percent_encoding::percent_decode_str;
use relative_path::RelativePathBuf;
use serde::Serialize;

const MAX_CONCURRENT_CONNECTIONS: usize = 16;
const MAX_REQUEST_HEADER_BYTES: usize = 32 * 1024;
const CLIENT_READ_TIMEOUT: Duration = Duration::from_secs(3);
const CLIENT_WRITE_TIMEOUT: Duration = Duration::from_secs(10);
const EVENT_HEARTBEAT: Duration = Duration::from_secs(15);
const INTERNAL_PREFIX: &str = "/__pana_source/";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SourceBrowserContent {
    Html(String),
    Text { body: Vec<u8>, content_type: String },
}

#[derive(Clone, Debug)]
pub(crate) struct SourceBrowserGeneration {
    pub project_root: String,
    pub runtime_session_id: String,
    pub disk_generation: u64,
    pub content: HashMap<String, SourceBrowserContent>,
    pub assets_root: PathBuf,
}

impl SourceBrowserGeneration {
    pub fn owner_matches(&self, project_root: &str, runtime_session_id: &str) -> bool {
        self.project_root == project_root && self.runtime_session_id == runtime_session_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SourceBrowserPublicationStatus {
    Empty,
    Building {
        disk_generation: u64,
    },
    Ready {
        disk_generation: u64,
    },
    Failed {
        disk_generation: u64,
        diagnostic: String,
    },
}

#[derive(Clone)]
struct SourceBrowserSnapshot {
    sequence: u64,
    active: Option<Arc<SourceBrowserGeneration>>,
    status: SourceBrowserPublicationStatus,
    closed: bool,
}

struct SourceBrowserRegistryState {
    sequence: u64,
    active: Option<Arc<SourceBrowserGeneration>>,
    status: SourceBrowserPublicationStatus,
    closed: bool,
}

impl Default for SourceBrowserRegistryState {
    fn default() -> Self {
        Self {
            sequence: 0,
            active: None,
            status: SourceBrowserPublicationStatus::Empty,
            closed: false,
        }
    }
}

#[derive(Default)]
struct SourceBrowserRegistry {
    state: Mutex<SourceBrowserRegistryState>,
    changed: Condvar,
}

impl SourceBrowserRegistry {
    fn snapshot(&self) -> Result<SourceBrowserSnapshot, String> {
        self.state
            .lock()
            .map_err(|_| "Source Browser registry este indisponibil.".to_string())
            .map(|state| Self::snapshot_from(&state))
    }

    fn snapshot_from(state: &SourceBrowserRegistryState) -> SourceBrowserSnapshot {
        SourceBrowserSnapshot {
            sequence: state.sequence,
            active: state.active.clone(),
            status: state.status.clone(),
            closed: state.closed,
        }
    }

    fn mutate(
        &self,
        update: impl FnOnce(&mut SourceBrowserRegistryState),
    ) -> Result<SourceBrowserSnapshot, String> {
        let snapshot = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| "Source Browser registry este indisponibil.".to_string())?;
            update(&mut state);
            state.sequence = state.sequence.saturating_add(1);
            Self::snapshot_from(&state)
        };
        self.changed.notify_all();
        Ok(snapshot)
    }

    fn mark_building(&self, disk_generation: u64) -> Result<(), String> {
        self.mutate(|state| {
            state.status = SourceBrowserPublicationStatus::Building { disk_generation };
        })?;
        Ok(())
    }

    fn publish(
        &self,
        generation: Arc<SourceBrowserGeneration>,
    ) -> Result<Option<Arc<SourceBrowserGeneration>>, String> {
        let mut previous = None;
        self.mutate(|state| {
            previous = state.active.replace(Arc::clone(&generation));
            state.status = SourceBrowserPublicationStatus::Ready {
                disk_generation: generation.disk_generation,
            };
        })?;
        Ok(previous)
    }

    fn publish_failure(&self, disk_generation: u64, diagnostic: String) -> Result<(), String> {
        self.mutate(|state| {
            state.status = SourceBrowserPublicationStatus::Failed {
                disk_generation,
                diagnostic,
            };
        })?;
        Ok(())
    }

    fn wait_after(&self, sequence: u64) -> Result<SourceBrowserSnapshot, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "Source Browser registry este indisponibil.".to_string())?;
        let (state, _) = self
            .changed
            .wait_timeout_while(state, EVENT_HEARTBEAT, |state| {
                !state.closed && state.sequence == sequence
            })
            .map_err(|_| "Source Browser registry nu a putut aștepta o generație.".to_string())?;
        Ok(Self::snapshot_from(&state))
    }

    fn close(&self) {
        let _ = self.mutate(|state| {
            state.closed = true;
            state.active = None;
            state.status = SourceBrowserPublicationStatus::Empty;
        });
    }
}

pub(crate) struct SourceBrowserServer {
    port: u16,
    stop_flag: Arc<AtomicBool>,
    registry: Arc<SourceBrowserRegistry>,
    thread: Option<JoinHandle<()>>,
}

impl SourceBrowserServer {
    pub fn start() -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|error| format!("Nu am putut porni Source Browser server: {error}"))?;
        let port = listener
            .local_addr()
            .map_err(|error| format!("Nu am putut citi portul Source Browser: {error}"))?
            .port();
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("Nu am putut configura Source Browser server: {error}"))?;
        let stop_flag = Arc::new(AtomicBool::new(false));
        let registry = Arc::new(SourceBrowserRegistry::default());
        let thread = spawn_server(
            listener,
            Arc::clone(&stop_flag),
            Arc::clone(&registry),
            port,
        )?;
        Ok(Self {
            port,
            stop_flag,
            registry,
            thread: Some(thread),
        })
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn active(&self) -> Result<Option<Arc<SourceBrowserGeneration>>, String> {
        self.registry.snapshot().map(|snapshot| snapshot.active)
    }

    pub fn mark_building(&self, disk_generation: u64) -> Result<(), String> {
        self.registry.mark_building(disk_generation)
    }

    pub fn publish(
        &self,
        generation: Arc<SourceBrowserGeneration>,
    ) -> Result<Option<Arc<SourceBrowserGeneration>>, String> {
        self.registry.publish(generation)
    }

    pub fn publish_failure(&self, disk_generation: u64, diagnostic: String) -> Result<(), String> {
        self.registry.publish_failure(disk_generation, diagnostic)
    }

    pub fn stop(mut self) {
        self.stop_inner();
    }

    fn stop_inner(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        self.registry.close();
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for SourceBrowserServer {
    fn drop(&mut self) {
        self.stop_inner();
    }
}

fn spawn_server(
    listener: TcpListener,
    stop_flag: Arc<AtomicBool>,
    registry: Arc<SourceBrowserRegistry>,
    port: u16,
) -> Result<JoinHandle<()>, String> {
    std::thread::Builder::new()
        .name("pana-source-browser".to_string())
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
                        let registry = Arc::clone(&registry);
                        let stop_flag = Arc::clone(&stop_flag);
                        let connections = Arc::clone(&active_connections);
                        if std::thread::Builder::new()
                            .name("pana-source-browser-request".to_string())
                            .spawn(move || {
                                let _lease = ConnectionLease(connections);
                                handle_connection(stream, &registry, &stop_flag, port);
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
        .map_err(|error| format!("Nu am putut porni thread-ul Source Browser: {error}"))
}

struct ConnectionLease(Arc<AtomicUsize>);

impl Drop for ConnectionLease {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

fn handle_connection(
    mut stream: TcpStream,
    registry: &Arc<SourceBrowserRegistry>,
    stop_flag: &Arc<AtomicBool>,
    port: u16,
) {
    let _ = stream.set_read_timeout(Some(CLIENT_READ_TIMEOUT));
    let _ = stream.set_write_timeout(Some(CLIENT_WRITE_TIMEOUT));
    let request = match read_request(&mut stream) {
        Ok(request) => request,
        Err(error) => {
            let _ = stream.write_all(&error_response(error));
            return;
        }
    };
    if !host_is_allowed(&request, port) {
        let _ = stream.write_all(&plain_response(
            "HTTP/1.1 421 Misdirected Request",
            "text/plain; charset=utf-8",
            b"Invalid Source Browser host",
            false,
            None,
        ));
        return;
    }
    let Some((method, target)) = parse_request_line(&request) else {
        let _ = stream.write_all(&error_response(
            "Cererea Source Browser nu are request-line valid.".to_string(),
        ));
        return;
    };
    if method == "GET" && target.split('?').next() == Some("/__pana_source/events") {
        serve_events(&mut stream, registry, stop_flag);
        return;
    }
    let response = serve_request(&request, registry, port).unwrap_or_else(error_response);
    let _ = stream.write_all(&response);
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Both);
}

fn serve_request(
    request: &[u8],
    registry: &SourceBrowserRegistry,
    port: u16,
) -> Result<Vec<u8>, String> {
    let (method, request_target) = parse_request_line(request)
        .ok_or_else(|| "Cererea Source Browser nu are request-line valid.".to_string())?;
    if !matches!(method.as_str(), "GET" | "HEAD") {
        return Ok(plain_response(
            "HTTP/1.1 405 Method Not Allowed",
            "text/plain; charset=utf-8",
            b"Method Not Allowed",
            method == "HEAD",
            None,
        ));
    }
    let head_only = method == "HEAD";
    let route = request_target.split('?').next().unwrap_or("/");
    if route == "/__pana_source/reload.js" {
        let generation = query_u64(&request_target, "generation").unwrap_or_default();
        let script = reload_script(generation, port);
        return Ok(plain_response(
            "HTTP/1.1 200 OK",
            "text/javascript; charset=utf-8",
            script.as_bytes(),
            head_only,
            None,
        ));
    }
    if route == "/__pana_source/status" {
        let snapshot = registry.snapshot()?;
        let body = serde_json::to_vec(&event_payload(&snapshot))
            .map_err(|error| format!("Status Source Browser invalid: {error}"))?;
        return Ok(plain_response(
            "HTTP/1.1 200 OK",
            "application/json; charset=utf-8",
            &body,
            head_only,
            None,
        ));
    }
    if route.starts_with(INTERNAL_PREFIX) {
        return Ok(plain_response(
            "HTTP/1.1 404 Not Found",
            "text/plain; charset=utf-8",
            b"Not Found",
            head_only,
            None,
        ));
    }

    let decoded = percent_decode_str(route)
        .decode_utf8()
        .map_err(|_| "Path-ul Source Browser nu este UTF-8 valid.".to_string())?;
    let content_key = zola_content_key(&decoded)?;
    let snapshot = registry.snapshot()?;
    let Some(generation) = snapshot.active else {
        return Ok(unavailable_response(&snapshot.status, head_only));
    };
    let canonical_content_key = if decoded != "/"
        && !decoded.ends_with('/')
        && !decoded.starts_with("//")
        && !decoded.contains('\\')
    {
        let candidate = format!("{content_key}/");
        generation
            .content
            .get(&candidate)
            .is_some_and(|content| matches!(content, SourceBrowserContent::Html(_)))
            .then_some(candidate)
    } else {
        None
    };
    let resolved_content_key = canonical_content_key.as_deref().unwrap_or(&content_key);
    if let SourceBrowserPublicationStatus::Failed {
        disk_generation,
        diagnostic,
    } = &snapshot.status
    {
        if *disk_generation > generation.disk_generation
            && generation
                .content
                .get(resolved_content_key)
                .is_some_and(|content| matches!(content, SourceBrowserContent::Html(_)))
        {
            return Ok(build_error_response(
                *disk_generation,
                diagnostic,
                head_only,
            ));
        }
    }

    if canonical_content_key.is_some() {
        return Ok(temporary_redirect_response(
            &directory_redirect_location(&request_target),
            head_only,
            generation.disk_generation,
        ));
    }
    if let Some(content) = generation.content.get(resolved_content_key) {
        return Ok(render_content(
            content,
            "HTTP/1.1 200 OK",
            generation.disk_generation,
            head_only,
        ));
    }
    if let Some(asset) = read_asset(&generation.assets_root, &decoded)? {
        return Ok(plain_response(
            "HTTP/1.1 200 OK",
            &asset.content_type,
            &asset.body,
            head_only,
            Some(generation.disk_generation),
        ));
    }
    if let Some(content) = generation.content.get("404.html") {
        return Ok(render_content(
            content,
            "HTTP/1.1 404 Not Found",
            generation.disk_generation,
            head_only,
        ));
    }
    Ok(plain_response(
        "HTTP/1.1 404 Not Found",
        "text/plain; charset=utf-8",
        b"Not Found",
        head_only,
        Some(generation.disk_generation),
    ))
}

fn serve_events(stream: &mut TcpStream, registry: &SourceBrowserRegistry, stop_flag: &AtomicBool) {
    let headers = b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream; charset=utf-8\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: keep-alive\r\n\r\n";
    if stream.write_all(headers).is_err() || stream.flush().is_err() {
        return;
    }
    let mut sequence = u64::MAX;
    while !stop_flag.load(Ordering::SeqCst) {
        let snapshot = if sequence == u64::MAX {
            registry.snapshot()
        } else {
            registry.wait_after(sequence)
        };
        let Ok(snapshot) = snapshot else {
            return;
        };
        if snapshot.closed {
            return;
        }
        if snapshot.sequence == sequence {
            if stream.write_all(b": keep-alive\n\n").is_err() || stream.flush().is_err() {
                return;
            }
            continue;
        }
        sequence = snapshot.sequence;
        let Ok(json) = serde_json::to_string(&event_payload(&snapshot)) else {
            return;
        };
        let event = format!("event: state\nid: {sequence}\ndata: {json}\n\n");
        if stream.write_all(event.as_bytes()).is_err() || stream.flush().is_err() {
            return;
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceBrowserEvent<'a> {
    sequence: u64,
    status: &'static str,
    disk_generation: Option<u64>,
    active_disk_generation: Option<u64>,
    diagnostic: Option<&'a str>,
}

fn event_payload(snapshot: &SourceBrowserSnapshot) -> SourceBrowserEvent<'_> {
    let active_disk_generation = snapshot
        .active
        .as_ref()
        .map(|generation| generation.disk_generation);
    match &snapshot.status {
        SourceBrowserPublicationStatus::Empty => SourceBrowserEvent {
            sequence: snapshot.sequence,
            status: "empty",
            disk_generation: None,
            active_disk_generation,
            diagnostic: None,
        },
        SourceBrowserPublicationStatus::Building { disk_generation } => SourceBrowserEvent {
            sequence: snapshot.sequence,
            status: "building",
            disk_generation: Some(*disk_generation),
            active_disk_generation,
            diagnostic: None,
        },
        SourceBrowserPublicationStatus::Ready { disk_generation } => SourceBrowserEvent {
            sequence: snapshot.sequence,
            status: "ready",
            disk_generation: Some(*disk_generation),
            active_disk_generation,
            diagnostic: None,
        },
        SourceBrowserPublicationStatus::Failed {
            disk_generation,
            diagnostic,
        } => SourceBrowserEvent {
            sequence: snapshot.sequence,
            status: "failed",
            disk_generation: Some(*disk_generation),
            active_disk_generation,
            diagnostic: Some(diagnostic),
        },
    }
}

fn render_content(
    content: &SourceBrowserContent,
    status: &str,
    disk_generation: u64,
    head_only: bool,
) -> Vec<u8> {
    match content {
        SourceBrowserContent::Html(html) => {
            let body = inject_reload_client(html, disk_generation);
            plain_response(
                status,
                "text/html; charset=utf-8",
                body.as_bytes(),
                head_only,
                Some(disk_generation),
            )
        }
        SourceBrowserContent::Text { body, content_type } => {
            plain_response(status, content_type, body, head_only, Some(disk_generation))
        }
    }
}

fn inject_reload_client(html: &str, disk_generation: u64) -> String {
    let marker = "/__pana_source/reload.js";
    if html.contains(marker) {
        return html.to_string();
    }
    let tag = format!(
        "<script src=\"{marker}?generation={disk_generation}\" defer data-pana-source-reload></script>"
    );
    let lower = html.to_ascii_lowercase();
    let insertion = lower
        .rfind("</body>")
        .or_else(|| lower.rfind("</html>"))
        .unwrap_or(html.len());
    let mut result = String::with_capacity(html.len() + tag.len());
    result.push_str(&html[..insertion]);
    result.push_str(&tag);
    result.push_str(&html[insertion..]);
    result
}

fn reload_script(disk_generation: u64, port: u16) -> String {
    format!(
        r##"(() => {{
  const currentGeneration = {disk_generation};
  const overlayId = "pana-source-browser-state";
  const removeOverlay = () => document.getElementById(overlayId)?.remove();
  const showOverlay = (message, failed = false) => {{
    let node = document.getElementById(overlayId);
    if (!node) {{
      node = document.createElement("div");
      node.id = overlayId;
      node.setAttribute("role", failed ? "alert" : "status");
      Object.assign(node.style, {{ position: "fixed", left: "16px", bottom: "16px", zIndex: "2147483647", maxWidth: "min(520px, calc(100vw - 32px))", padding: "10px 14px", borderRadius: "8px", font: "13px/1.45 system-ui, sans-serif", color: "#fff", background: failed ? "#9f1d27" : "#17231f", boxShadow: "0 8px 28px rgba(0,0,0,.28)" }});
      document.documentElement.appendChild(node);
    }}
    node.style.background = failed ? "#9f1d27" : "#17231f";
    node.textContent = message;
  }};
  const events = new EventSource("http://127.0.0.1:{port}/__pana_source/events");
  events.addEventListener("state", (event) => {{
    let state;
    try {{ state = JSON.parse(event.data); }} catch {{ return; }}
    const target = Number(state.diskGeneration || 0);
    if (state.status === "ready" && target > currentGeneration) {{
      events.close();
      location.reload();
      return;
    }}
    if (state.status === "building" && target > currentGeneration) {{
      showOverlay("Pană Studio actualizează versiunea salvată…");
      return;
    }}
    if (state.status === "failed" && target >= currentGeneration) {{
      showOverlay("Build Zola eșuat: " + (state.diagnostic || "eroare necunoscută"), true);
      return;
    }}
    removeOverlay();
  }});
}})();"##
    )
}

fn unavailable_response(status: &SourceBrowserPublicationStatus, head_only: bool) -> Vec<u8> {
    match status {
        SourceBrowserPublicationStatus::Failed {
            disk_generation,
            diagnostic,
        } => build_error_response(*disk_generation, diagnostic, head_only),
        SourceBrowserPublicationStatus::Building { disk_generation } => plain_response(
            "HTTP/1.1 503 Service Unavailable",
            "text/plain; charset=utf-8",
            format!("Source Browser construiește generația {disk_generation}.").as_bytes(),
            head_only,
            None,
        ),
        SourceBrowserPublicationStatus::Empty | SourceBrowserPublicationStatus::Ready { .. } => {
            plain_response(
                "HTTP/1.1 503 Service Unavailable",
                "text/plain; charset=utf-8",
                "Source Browser nu are o generație publicată.".as_bytes(),
                head_only,
                None,
            )
        }
    }
}

fn build_error_response(disk_generation: u64, diagnostic: &str, head_only: bool) -> Vec<u8> {
    let body = format!(
        "<!doctype html><html lang=\"ro\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><title>Build Zola eșuat</title><body><main><h1>Build Zola eșuat</h1><p>Generația salvată {disk_generation} nu a putut fi randată.</p><pre>{}</pre></main></body></html>",
        escape_html(diagnostic)
    );
    plain_response(
        "HTTP/1.1 503 Service Unavailable",
        "text/html; charset=utf-8",
        body.as_bytes(),
        head_only,
        Some(disk_generation),
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
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
            "Directorul de resurse Source Browser {} este indisponibil: {error}",
            root.display()
        )
    })?;
    let requested = root.join(relative);
    let mut canonical = match requested.canonicalize() {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "Nu am putut rezolva resursa Source Browser {}: {error}",
                requested.display()
            ))
        }
    };
    if !canonical.starts_with(&canonical_root) {
        return Err("Source Browser a refuzat o resursă din afara generației.".to_string());
    }
    if canonical.is_dir() {
        canonical.push("index.html");
    }
    if !canonical.is_file() {
        return Ok(None);
    }
    let body = fs::read(&canonical).map_err(|error| {
        format!(
            "Nu am putut citi resursa Source Browser {}: {error}",
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
            return Err("Source Browser a refuzat un path de resursă nesigur.".to_string());
        }
    }
    Ok(path.to_path_buf())
}

fn zola_content_key(decoded_path: &str) -> Result<String, String> {
    if !decoded_path.starts_with('/') {
        return Err("Path-ul Source Browser trebuie să fie absolut HTTP.".to_string());
    }
    let mut path = RelativePathBuf::new();
    for component in decoded_path.split('/') {
        path.push(component);
    }
    Ok(path.as_str().to_string())
}

fn query_u64(target: &str, expected_name: &str) -> Option<u64> {
    target
        .split_once('?')
        .map(|(_, query)| query)
        .into_iter()
        .flat_map(|query| query.split('&'))
        .find_map(|part| {
            let (name, value) = part.split_once('=')?;
            (name == expected_name)
                .then(|| value.parse::<u64>().ok())
                .flatten()
        })
}

fn read_request(reader: &mut impl Read) -> Result<Vec<u8>, String> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("Nu am putut citi cererea Source Browser: {error}"))?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        if request.len() > MAX_REQUEST_HEADER_BYTES {
            return Err("Headerul cererii Source Browser este prea mare.".to_string());
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

fn request_header<'a>(request: &'a [u8], expected_name: &str) -> Option<&'a str> {
    let request = std::str::from_utf8(request).ok()?;
    request.lines().skip(1).find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case(expected_name)
            .then_some(value.trim())
    })
}

fn host_is_allowed(request: &[u8], port: u16) -> bool {
    request_header(request, "host").is_some_and(|host| {
        host.eq_ignore_ascii_case(&format!("127.0.0.1:{port}"))
            || host.eq_ignore_ascii_case(&format!("localhost:{port}"))
    })
}

fn plain_response(
    status: &str,
    content_type: &str,
    body: &[u8],
    head_only: bool,
    disk_generation: Option<u64>,
) -> Vec<u8> {
    let generation_header = disk_generation
        .map(|generation| format!("X-Pana-Disk-Generation: {generation}\r\n"))
        .unwrap_or_default();
    let headers = format!(
        "{status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n{generation_header}X-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let mut response = headers.into_bytes();
    if !head_only {
        response.extend_from_slice(body);
    }
    response
}

fn directory_redirect_location(request_target: &str) -> String {
    match request_target.split_once('?') {
        Some((path, query)) => format!("{path}/?{query}"),
        None => format!("{request_target}/"),
    }
}

fn temporary_redirect_response(location: &str, head_only: bool, disk_generation: u64) -> Vec<u8> {
    let body = format!("Temporary Redirect: {location}");
    let headers = format!(
        "HTTP/1.1 307 Temporary Redirect\r\nLocation: {location}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Pana-Disk-Generation: {disk_generation}\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let mut response = headers.into_bytes();
    if !head_only {
        response.extend_from_slice(body.as_bytes());
    }
    response
}

fn error_response(message: String) -> Vec<u8> {
    plain_response(
        "HTTP/1.1 500 Internal Server Error",
        "text/plain; charset=utf-8",
        message.as_bytes(),
        false,
        None,
    )
}

fn write_overload(mut stream: TcpStream) {
    let response = plain_response(
        "HTTP/1.1 503 Service Unavailable",
        "text/plain; charset=utf-8",
        b"Source Browser busy",
        false,
        None,
    );
    let _ = stream.write_all(&response);
    let _ = stream.shutdown(Shutdown::Both);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generation(disk_generation: u64, html: &str) -> Arc<SourceBrowserGeneration> {
        Arc::new(SourceBrowserGeneration {
            project_root: "/project".to_string(),
            runtime_session_id: "runtime".to_string(),
            disk_generation,
            content: HashMap::from([(
                "".to_string(),
                SourceBrowserContent::Html(html.to_string()),
            )]),
            assets_root: std::env::temp_dir(),
        })
    }

    fn generation_with_pages(disk_generation: u64) -> Arc<SourceBrowserGeneration> {
        Arc::new(SourceBrowserGeneration {
            project_root: "/project".to_string(),
            runtime_session_id: "runtime".to_string(),
            disk_generation,
            content: HashMap::from([
                (
                    "".to_string(),
                    SourceBrowserContent::Html("<html><body>Acasă</body></html>".to_string()),
                ),
                (
                    "despre/".to_string(),
                    SourceBrowserContent::Html("<html><body>Despre</body></html>".to_string()),
                ),
                (
                    "servicii/".to_string(),
                    SourceBrowserContent::Html("<html><body>Servicii</body></html>".to_string()),
                ),
            ]),
            assets_root: std::env::temp_dir(),
        })
    }

    fn request(registry: &SourceBrowserRegistry, target: &str) -> String {
        let raw = format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1:43210\r\n\r\n");
        String::from_utf8(serve_request(raw.as_bytes(), registry, 43210).unwrap()).unwrap()
    }

    #[test]
    fn published_generation_is_swapped_atomically_and_keeps_reload_runtime_out_of_source() {
        let registry = SourceBrowserRegistry::default();
        let source = "<!doctype html><html><body><p>disk one</p><script>window.projectJs=true</script></body></html>";
        registry.publish(generation(1, source)).unwrap();
        let first = request(&registry, "/");
        assert!(first.contains("disk one"));
        assert!(first.contains("window.projectJs=true"));
        assert!(first.contains("/__pana_source/reload.js?generation=1"));
        assert!(!source.contains("__pana_source"));

        registry.mark_building(2).unwrap();
        let while_building = request(&registry, "/");
        assert!(while_building.contains("disk one"));
        registry
            .publish(generation(
                2,
                "<!doctype html><html><body><p>disk two</p></body></html>",
            ))
            .unwrap();
        let second = request(&registry, "/");
        assert!(second.contains("disk two"));
        assert!(!second.contains("disk one"));
        assert!(second.contains("X-Pana-Disk-Generation: 2"));
    }

    #[test]
    fn failed_new_generation_never_claims_the_last_good_document() {
        let registry = SourceBrowserRegistry::default();
        registry
            .publish(generation(3, "<html><body>good</body></html>"))
            .unwrap();
        registry
            .publish_failure(4, "template invalid <boom>".to_string())
            .unwrap();
        let response = request(&registry, "/");
        assert!(response.starts_with("HTTP/1.1 503 Service Unavailable"));
        assert!(response.contains("Generația salvată 4"));
        assert!(response.contains("&lt;boom&gt;"));
        assert!(!response.contains(">good<"));
    }

    #[test]
    fn internal_routes_are_reserved_and_asset_paths_fail_closed() {
        let registry = SourceBrowserRegistry::default();
        registry.publish(generation(1, "<html></html>")).unwrap();
        assert!(request(&registry, "/__pana_source/foreign").starts_with("HTTP/1.1 404 Not Found"));
        assert!(safe_asset_relative_path("/../secret").is_err());
        assert_eq!(zola_content_key("/despre/").unwrap(), "despre/");
    }

    #[test]
    fn page_routes_without_trailing_slash_redirect_to_the_zola_canonical_route() {
        let registry = SourceBrowserRegistry::default();
        registry.publish(generation_with_pages(7)).unwrap();

        let despre = request(&registry, "/despre?mod=preview");
        assert!(despre.starts_with("HTTP/1.1 307 Temporary Redirect"));
        assert!(despre.contains("Location: /despre/?mod=preview\r\n"));
        assert!(despre.contains("Cache-Control: no-store"));
        assert!(despre.contains("X-Pana-Disk-Generation: 7"));

        let canonical_despre = request(&registry, "/despre/");
        assert!(canonical_despre.starts_with("HTTP/1.1 200 OK"));
        assert!(canonical_despre.contains(">Despre<"));

        let servicii = request(&registry, "/servicii");
        assert!(servicii.starts_with("HTTP/1.1 307 Temporary Redirect"));
        assert!(servicii.contains("Location: /servicii/\r\n"));
        assert!(request(&registry, "/servicii/").starts_with("HTTP/1.1 200 OK"));
        assert!(request(&registry, "/contact").starts_with("HTTP/1.1 404 Not Found"));
    }
}
