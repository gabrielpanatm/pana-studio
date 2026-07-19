use std::{
    net::TcpListener,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use axum::{
    body::Body,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use rmcp::transport::{
    streamable_http_server::session::local::LocalSessionManager, StreamableHttpServerConfig,
    StreamableHttpService,
};
use serde_json::json;
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;

use crate::{
    commands::mcp::current_ai_context_snapshot,
    mcp::service::PanaMcpService,
    state::{AppState, McpServerHandle},
};

use super::security::{access_tokens_equal, is_valid_access_token, ACCESS_TOKEN_HEADER};

pub const MCP_PORT: u16 = 48731;
pub const MCP_ENDPOINT: &str = "http://127.0.0.1:48731/mcp";

#[derive(Clone)]
struct HttpState {
    app: AppHandle,
    access_token: Arc<str>,
}

pub fn start_context_server(
    app: AppHandle,
    access_token: String,
) -> Result<McpServerHandle, String> {
    if !is_valid_access_token(&access_token) {
        return Err("Serverul MCP refuză un token de acces invalid.".to_string());
    }
    let listener = TcpListener::bind(("127.0.0.1", MCP_PORT))
        .map_err(|error| format!("Nu am putut porni serverul MCP Pană Studio: {error}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("Nu am putut pregăti listenerul MCP: {error}"))?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("pana-mcp-runtime")
        .enable_all()
        .build()
        .map_err(|error| format!("Nu am putut construi runtime-ul MCP: {error}"))?;

    let cancellation_token = CancellationToken::new();
    let server_cancellation = cancellation_token.clone();
    let thread = std::thread::Builder::new()
        .name("pana-mcp-server".to_string())
        .spawn(move || {
            runtime.block_on(async move {
                if let Err(error) =
                    run_server(listener, app, access_token.into(), server_cancellation).await
                {
                    eprintln!("[Pană Studio] Serverul MCP s-a oprit cu eroare: {error}");
                }
            });
        })
        .map_err(|error| format!("Nu am putut porni thread-ul MCP: {error}"))?;

    Ok(McpServerHandle {
        cancellation_token,
        thread: Some(thread),
    })
}

async fn run_server(
    listener: TcpListener,
    app: AppHandle,
    access_token: Arc<str>,
    cancellation_token: CancellationToken,
) -> Result<(), String> {
    let listener = tokio::net::TcpListener::from_std(listener)
        .map_err(|error| format!("Listenerul MCP nu poate fi adoptat de Tokio: {error}"))?;
    let next_session_nonce = Arc::new(AtomicU64::new(1));
    let factory_app = app.clone();
    let factory_nonce = next_session_nonce.clone();
    let config = StreamableHttpServerConfig::default()
        .with_cancellation_token(cancellation_token.clone())
        .with_allowed_hosts([
            "127.0.0.1",
            "127.0.0.1:48731",
            "localhost",
            "localhost:48731",
        ])
        .with_allowed_origins([
            "null",
            "tauri://localhost",
            "http://tauri.localhost",
            "https://tauri.localhost",
            "http://localhost:1420",
            "http://127.0.0.1:1420",
            "http://localhost:1430",
            "http://127.0.0.1:1430",
        ]);
    let service: StreamableHttpService<PanaMcpService, LocalSessionManager> =
        StreamableHttpService::new(
            move || {
                let nonce = factory_nonce.fetch_add(1, Ordering::Relaxed);
                let session_id = format!(
                    "mcp-{:032x}-{nonce:016x}",
                    crate::kernel::observability::now_ms()
                );
                Ok(PanaMcpService::new(factory_app.clone(), session_id))
            },
            Arc::new(LocalSessionManager::default()),
            config,
        );

    let http_state = HttpState { app, access_token };
    let router = Router::new()
        .route("/health", get(health))
        .route("/context", get(context))
        .nest_service("/mcp", service)
        .with_state(http_state.clone())
        .layer(DefaultBodyLimit::max(256 * 1024))
        .layer(middleware::from_fn_with_state(
            http_state,
            require_access_token,
        ));

    axum::serve(listener, router)
        .with_graceful_shutdown({
            let cancellation_token = cancellation_token.clone();
            async move { cancellation_token.cancelled_owned().await }
        })
        .await
        .map_err(|error| format!("Transportul Streamable HTTP a eșuat: {error}"))
}

async fn require_access_token(
    State(state): State<HttpState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let authorized = request
        .headers()
        .get(ACCESS_TOKEN_HEADER)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|candidate| access_tokens_equal(candidate, &state.access_token));
    if !authorized {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "ok": false, "error": "Autentificare MCP necesară." })),
        )
            .into_response();
    }
    next.run(request).await
}

async fn health(headers: HeaderMap, State(state): State<HttpState>) -> Response {
    if !safe_local_request(&headers) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "ok": false, "error": "Host sau Origin nepermis." })),
        )
            .into_response();
    }
    let coordination = state
        .app
        .state::<AppState>()
        .ai_coordination
        .snapshot(crate::kernel::observability::now_ms())
        .ok();
    Json(json!({
        "ok": true,
        "name": "pana-studio",
        "processId": std::process::id(),
        "sdk": "rmcp",
        "sdkVersion": "2.2.0",
        "mode": "read_only_data_with_ram_coordination",
        "coordinationRevision": coordination.map(|snapshot| snapshot.coordination_revision)
    }))
    .into_response()
}

async fn context(headers: HeaderMap, State(state): State<HttpState>) -> Response {
    if !safe_local_request(&headers) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Host sau Origin nepermis." })),
        )
            .into_response();
    }
    let value = current_ai_context_snapshot(state.app.state::<AppState>().inner())
        .ok()
        .flatten()
        .and_then(|snapshot| serde_json::to_value(snapshot).ok())
        .unwrap_or_else(|| {
            json!({
                "version": 2,
                "available": false,
                "message": "Pană Studio nu a publicat încă o proiecție UI tipizată."
            })
        });
    Json(value).into_response()
}

fn safe_local_request(headers: &HeaderMap) -> bool {
    let host_allowed = headers
        .get("host")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|host| {
            matches!(
                host,
                "127.0.0.1" | "127.0.0.1:48731" | "localhost" | "localhost:48731"
            )
        });
    if !host_allowed {
        return false;
    }
    headers
        .get("origin")
        .and_then(|value| value.to_str().ok())
        .is_none_or(|origin| {
            matches!(
                origin,
                "null"
                    | "tauri://localhost"
                    | "http://tauri.localhost"
                    | "https://tauri.localhost"
                    | "http://localhost:1420"
                    | "http://127.0.0.1:1420"
                    | "http://localhost:1430"
                    | "http://127.0.0.1:1430"
            )
        })
}
