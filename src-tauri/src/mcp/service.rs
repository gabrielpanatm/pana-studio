use rmcp::{
    handler::server::wrapper::Parameters,
    model::{
        CallToolResult, Implementation, InitializeRequestParams, InitializeResult,
        ListResourcesResult, PaginatedRequestParams, ProtocolVersion, ReadResourceRequestParams,
        ReadResourceResult, Resource, ResourceContents, ServerCapabilities, ServerInfo,
    },
    schemars::JsonSchema,
    service::RequestContext,
    tool, tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Manager};

use crate::{
    commands::{
        ai_coordination::with_current_project_coordination_evidence,
        mcp::current_ai_context_snapshot,
    },
    kernel::{
        ai_coordination::{AiClientIdentity, EditLeaseRequest, ReleaseEditLeaseInput},
        observability::now_ms,
    },
    state::AppState,
};

const CURRENT_CONTEXT_URI: &str = "panastudio://context/current";

#[derive(Clone)]
pub(super) struct PanaMcpService {
    app: AppHandle,
    client_session_id: String,
}

impl std::fmt::Debug for PanaMcpService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PanaMcpService")
            .field("client_session_id", &self.client_session_id)
            .finish_non_exhaustive()
    }
}

impl PanaMcpService {
    pub(super) fn new(app: AppHandle, client_session_id: String) -> Self {
        Self {
            app,
            client_session_id,
        }
    }

    fn state(&self) -> tauri::State<'_, AppState> {
        self.app.state::<AppState>()
    }

    fn observe(&self, context_revision_seen: Option<u64>) -> Result<(), String> {
        self.state()
            .ai_coordination
            .observe_client(&self.client_session_id, context_revision_seen, now_ms())
            .map_err(|error| error.to_string())
    }

    fn current_context_value(&self) -> Result<serde_json::Value, String> {
        let initial_snapshot = current_ai_context_snapshot(self.state().inner())?;
        let context_revision = initial_snapshot
            .as_ref()
            .map(|snapshot| snapshot.context_revision);
        self.observe(context_revision)?;
        // Observation updates the client presence kept by the coordination
        // runtime. Capture again so the caller never receives its own stale
        // presence/contextRevisionSeen projection.
        let snapshot = current_ai_context_snapshot(self.state().inner())?;
        snapshot
            .map(|snapshot| {
                serde_json::to_value(snapshot)
                    .map_err(|error| format!("Contextul curent nu poate fi serializat: {error}"))
            })
            .transpose()
            .map(|snapshot| {
                snapshot.unwrap_or_else(|| {
                    json!({
                        "version": 2,
                        "available": false,
                        "message": "Pană Studio nu a primit încă proiecția UI tipizată."
                    })
                })
            })
    }

    fn tool_result<T: Serialize>(
        &self,
        result: Result<T, String>,
    ) -> Result<CallToolResult, McpError> {
        match result {
            Ok(value) => serde_json::to_value(value)
                .map(CallToolResult::structured)
                .map_err(|error| {
                    McpError::internal_error(
                        "Rezultatul Pană Studio nu poate fi serializat.",
                        Some(json!({ "reason": error.to_string() })),
                    )
                }),
            Err(error) => Ok(CallToolResult::structured_error(json!({
                "ok": false,
                "error": error
            }))),
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RequestEditLeaseToolInput {
    expected_project_session_id: String,
    expected_project_revision: u64,
    request_id: String,
    intent: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RenewEditLeaseToolInput {
    lease_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReleaseEditLeaseToolInput {
    lease_id: String,
    #[serde(default)]
    expected_changed_files: Vec<String>,
    #[serde(default)]
    summary: Option<String>,
}

#[tool_router]
impl PanaMcpService {
    #[tool(
        description = "Returnează contextul canonic read-only curent din Context Hub-ul Rust. Nu citește și nu modifică fișierele proiectului.",
        annotations(
            title = "Citește contextul Pană Studio",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    fn get_current_context(&self) -> Result<CallToolResult, McpError> {
        self.tool_result(self.current_context_value())
    }

    #[tool(
        description = "Solicită autoritatea exclusivă de editare pentru această sesiune AI. Nu scrie fișiere; dacă proiectul este clean, frontendul trebuie să confirme quiescence înainte de grant.",
        annotations(
            title = "Solicită edit lease",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    fn request_edit_lease(
        &self,
        Parameters(input): Parameters<RequestEditLeaseToolInput>,
    ) -> Result<CallToolResult, McpError> {
        let state = self.state();
        let result = with_current_project_coordination_evidence(state.inner(), |evidence| {
            state
                .ai_coordination
                .request_edit_lease(
                    EditLeaseRequest {
                        client_session_id: self.client_session_id.clone(),
                        expected_project_session_id: input.expected_project_session_id,
                        expected_project_revision: input.expected_project_revision,
                        request_id: input.request_id,
                        intent: input.intent,
                    },
                    evidence,
                    now_ms(),
                )
                .map_err(|error| error.to_string())
        });
        self.tool_result(result)
    }

    #[tool(
        description = "Reînnoiește TTL-ul lease-ului deținut de această sesiune AI. Nu scrie fișiere și refuză lease-uri aparținând altei sesiuni.",
        annotations(
            title = "Reînnoiește edit lease",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    fn renew_edit_lease(
        &self,
        Parameters(input): Parameters<RenewEditLeaseToolInput>,
    ) -> Result<CallToolResult, McpError> {
        let state = self.state();
        let result = with_current_project_coordination_evidence(state.inner(), |evidence| {
            state
                .ai_coordination
                .renew_edit_lease(&self.client_session_id, &input.lease_id, evidence, now_ms())
                .map_err(|error| error.to_string())
        });
        self.tool_result(result)
    }

    #[tool(
        description = "Eliberează lease-ul acestei sesiuni AI și mută aplicația în starea Reconciling. Nu scrie fișiere; controlul utilizatorului revine numai după reconcilierea Rust.",
        annotations(
            title = "Eliberează edit lease",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    fn release_edit_lease(
        &self,
        Parameters(input): Parameters<ReleaseEditLeaseToolInput>,
    ) -> Result<CallToolResult, McpError> {
        let state = self.state();
        let result = with_current_project_coordination_evidence(state.inner(), |evidence| {
            state
                .ai_coordination
                .release_edit_lease(
                    ReleaseEditLeaseInput {
                        client_session_id: self.client_session_id.clone(),
                        lease_id: input.lease_id,
                        expected_changed_files: input.expected_changed_files,
                        summary: input.summary,
                    },
                    evidence,
                    now_ms(),
                )
                .map_err(|error| error.to_string())
        });
        self.tool_result(result)
    }
}

#[tool_handler]
impl ServerHandler for PanaMcpService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_resources()
                .enable_tools()
                .build(),
        )
        .with_server_info(
            Implementation::new("pana-studio", env!("CARGO_PKG_VERSION"))
                .with_title("Pană Studio Context Hub"),
        )
        .with_instructions(
            "Datele proiectului sunt read-only prin MCP. Înainte de editarea surselor prin filesystem, solicită request_edit_lease și așteaptă status granted. Reînnoiește lease-ul înainte de termen pe durata întregii tranzacții; la orice reînnoire refuzată sau conexiune pierdută oprește imediat scrierile. Apelează release_edit_lease numai după ultima scriere stabilă.",
        )
    }

    async fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        context.peer.set_peer_info(request.clone());
        self.state()
            .ai_coordination
            .register_client(
                AiClientIdentity {
                    session_id: self.client_session_id.clone(),
                    client_name: request.client_info.name.clone(),
                    client_version: Some(request.client_info.version.clone()),
                },
                now_ms(),
            )
            .map_err(|error| {
                McpError::internal_error(
                    "Sesiunea MCP nu poate fi înregistrată în Context Hub.",
                    Some(json!({ "reason": error.to_string() })),
                )
            })?;

        let mut info = self.get_info();
        if ProtocolVersion::KNOWN_VERSIONS.contains(&request.protocol_version) {
            info.protocol_version = request.protocol_version;
        }
        Ok(info)
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        self.observe(None).map_err(|error| {
            McpError::internal_error(
                "Sesiunea MCP nu poate fi observată.",
                Some(json!({
                    "reason": error
                })),
            )
        })?;
        Ok(ListResourcesResult::with_all_items(vec![Resource::new(
            CURRENT_CONTEXT_URI,
            "pana_studio_current_context",
        )
        .with_title("Pană Studio current context")
        .with_description(
            "Snapshot canonic read-only despre proiect, UI, dirty state și autoritatea de editare.",
        )
        .with_mime_type("application/json")]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        if request.uri != CURRENT_CONTEXT_URI {
            return Err(McpError::resource_not_found(
                "Resursa Pană Studio nu există.",
                Some(json!({ "uri": request.uri })),
            ));
        }
        let value = self.current_context_value().map_err(|error| {
            McpError::internal_error(
                "Context Hub nu poate produce resursa curentă.",
                Some(json!({ "reason": error })),
            )
        })?;
        let text = serde_json::to_string_pretty(&value).map_err(|error| {
            McpError::internal_error(
                "Resursa Context Hub nu poate fi serializată.",
                Some(json!({ "reason": error.to_string() })),
            )
        })?;
        Ok(ReadResourceResult::new(vec![ResourceContents::text(
            text,
            CURRENT_CONTEXT_URI,
        )
        .with_mime_type("application/json")]))
    }
}
