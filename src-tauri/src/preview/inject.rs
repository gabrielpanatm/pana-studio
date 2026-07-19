use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use sha2::{Digest, Sha256};
use tauri_utils::html::{parse, serialize_node, NodeRef};

use crate::{
    preview::CanvasProjectionIdentity,
    project_model::html_editor_schema::{
        has_active_script_scheme, is_forbidden_attribute_name, is_forbidden_element,
        is_forbidden_meta_http_equiv,
    },
};

const MAX_PREVIEW_HTML_BYTES: usize = 8 * 1024 * 1024;
#[cfg(test)]
const TEST_PREVIEW_PROXY_PORT: u16 = 4173;

const BRIDGE_SCRIPT: &str = concat!(
    include_str!("bridge/00_bootstrap.js"),
    "var HTML_EDITOR_SCHEMA = ",
    include_str!("../../../src/lib/html/editor-schema.json"),
    ";\n",
    include_str!("bridge/01_dom_structure.js"),
    include_str!("bridge/02_css_inspection.js"),
    include_str!("bridge/03_overlay_geometry.js"),
    include_str!("bridge/04_html_selection.js"),
    include_str!("bridge/05_template_gate.js"),
    include_str!("bridge/06_empty_zones.js"),
    include_str!("bridge/06_preview_hover.js"),
    include_str!("bridge/07_drag_drop.js"),
    include_str!("bridge/08_inspector_shell.js"),
    include_str!("bridge/09_design_safe_surface.js"),
    include_str!("bridge/10_canvas_patch.js"),
    include_str!("bridge/11_document_sync.js"),
    include_str!("bridge/12_messages_events.js"),
    include_str!("bridge/13_boot.js"),
);
const INTERACTIVE_RUNTIME_SCRIPT: &str = include_str!("interactive_runtime.js");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PreviewHtmlSurface {
    Editor,
    Visitor,
    Interactive,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedDesignSafeHtml {
    pub editor: String,
    pub visitor: String,
    pub interactive: String,
}

pub(crate) fn bind_canvas_identity_to_editor_html(
    prepared: &mut PreparedDesignSafeHtml,
    identity: &CanvasProjectionIdentity,
) -> Result<(), String> {
    let document = parse(prepared.editor.clone());
    let html = document
        .select_first("html")
        .map_err(|_| "Documentul Design Safe nu are root pentru identitatea Canvas.".to_string())?;
    let mut attributes = html.attributes.borrow_mut();
    attributes.insert(
        "data-pana-canvas-project-root",
        identity.project_root.clone(),
    );
    attributes.insert(
        "data-pana-canvas-runtime-session-id",
        identity.runtime_session_id.clone(),
    );
    attributes.insert(
        "data-pana-canvas-workspace-revision",
        identity.workspace_revision.to_string(),
    );
    attributes.insert(
        "data-pana-canvas-transaction-id",
        identity.transaction_id.clone(),
    );
    drop(attributes);
    prepared.editor = String::from_utf8(serialize_node(&document)).map_err(|error| {
        format!("Documentul Design Safe nu a putut lega identitatea Canvas: {error}")
    })?;
    Ok(())
}

impl PreviewHtmlSurface {
    fn allows_internal_bridge(self) -> bool {
        matches!(self, Self::Editor)
    }

    fn content_security_policy(self, preview_proxy_port: u16) -> String {
        let script_policy = match self {
            Self::Editor => {
                let hash = bridge_csp_hash();
                format!(
                    "script-src 'sha256-{hash}'; script-src-elem 'sha256-{hash}'; script-src-attr 'none'"
                )
            }
            Self::Visitor => {
                "script-src 'none'; script-src-elem 'none'; script-src-attr 'none'".to_string()
            }
            Self::Interactive => format!(
                "script-src http://127.0.0.1:{preview_proxy_port} 'unsafe-inline' 'unsafe-eval' blob:; \
                 script-src-elem http://127.0.0.1:{preview_proxy_port} 'unsafe-inline' blob:; \
                 script-src-attr 'unsafe-inline'"
            ),
        };
        let preview_asset_origin = format!("http://127.0.0.1:{preview_proxy_port}");

        // HTML/CSS remains renderable, including authored <style> nodes and
        // style attributes. The iframe deliberately has an opaque sandbox
        // origin, so `'self'` alone is not sufficient in WebKitGTK for linked
        // assets. Authorize only this exact loopback proxy origin, never every
        // localhost port. `unsafe-inline` is intentionally confined to the
        // style directives; script execution is restricted to the exact
        // SHA-256 bridge hash (or completely disabled for Visitor).
        let runtime_policy = match self {
            Self::Interactive => format!(
                "connect-src {preview_asset_origin}; worker-src {preview_asset_origin} blob:;"
            ),
            Self::Editor | Self::Visitor => "connect-src 'none'; worker-src 'none';".to_string(),
        };
        format!(
            "default-src 'none'; {script_policy}; \
             style-src 'self' {preview_asset_origin} 'unsafe-inline'; \
             style-src-elem 'self' {preview_asset_origin} 'unsafe-inline'; \
             style-src-attr 'unsafe-inline'; \
             img-src 'self' {preview_asset_origin} data:; \
             font-src 'self' {preview_asset_origin} data:; \
             media-src 'self' {preview_asset_origin} data:; \
             {runtime_policy} child-src 'none'; frame-src 'none'; \
             object-src 'none'; manifest-src 'none'; base-uri 'none'; form-action 'none'; \
             navigate-to 'none'"
        )
    }
}

/// Sanitizes and annotates a rendered Zola document once, at candidate-build
/// time. The persistent server can then answer requests without reparsing the
/// document and without consulting mutable renderer state.
pub(crate) fn prepare_design_safe_html(
    html: &str,
    preview_revision: &str,
) -> Result<PreparedDesignSafeHtml, String> {
    if html.len() > MAX_PREVIEW_HTML_BYTES {
        return Err(format!(
            "Documentul HTML de preview depășește limita de {} bytes.",
            MAX_PREVIEW_HTML_BYTES
        ));
    }
    if preview_revision.trim().is_empty() {
        return Err("Documentul Preview persistent cere o revizie nenulă.".to_string());
    }
    Ok(PreparedDesignSafeHtml {
        editor: sanitize_design_safe_document(
            html,
            PreviewHtmlSurface::Editor,
            Some(preview_revision),
        )?,
        visitor: sanitize_design_safe_document(
            html,
            PreviewHtmlSurface::Visitor,
            Some(preview_revision),
        )?,
        interactive: prepare_interactive_document(html, preview_revision)?,
    })
}

fn prepare_interactive_document(html: &str, preview_revision: &str) -> Result<String, String> {
    let document = parse(html.to_string());
    remove_authored_content_security_policy(&document);
    revision_local_resource_urls(&document, preview_revision);
    let html = document.select_first("html").map_err(|_| {
        "Documentul Interactive Preview nu are element html normalizat.".to_string()
    })?;
    html.attributes
        .borrow_mut()
        .insert("data-pana-preview-revision", preview_revision.to_string());
    append_interactive_runtime(&document)?;
    String::from_utf8(serialize_node(&document)).map_err(|error| {
        format!("Documentul Interactive Preview nu a putut fi serializat: {error}")
    })
}

fn remove_authored_content_security_policy(document: &NodeRef) {
    let nodes = document
        .select("meta[http-equiv]")
        .ok()
        .into_iter()
        .flatten()
        .filter(|node| {
            node.attributes
                .borrow()
                .get("http-equiv")
                .is_some_and(|value| value.eq_ignore_ascii_case("content-security-policy"))
        })
        .map(|node| node.as_node().clone())
        .collect::<Vec<_>>();
    for node in nodes {
        node.detach();
    }
}

fn append_interactive_runtime(document: &NodeRef) -> Result<(), String> {
    let runtime_document = parse(format!(
        "<!doctype html><html><body><script id=\"pana-interactive-runtime\">{INTERACTIVE_RUNTIME_SCRIPT}</script></body></html>"
    ));
    let runtime = runtime_document
        .select_first("script#pana-interactive-runtime")
        .map_err(|_| "Runtime-ul Interactive Preview nu a putut fi construit.".to_string())?
        .as_node()
        .clone();
    if runtime.text_contents() != INTERACTIVE_RUNTIME_SCRIPT {
        return Err("Parserul HTML a modificat runtime-ul Interactive Preview.".to_string());
    }
    runtime.detach();
    let body = document
        .select_first("body")
        .map_err(|_| "Documentul Interactive Preview nu are body normalizat.".to_string())?;
    body.as_node().append(runtime);
    Ok(())
}

pub(crate) fn build_prepared_design_safe_response(
    status_line: &str,
    content_type: &str,
    html: &str,
    surface: PreviewHtmlSurface,
    preview_port: u16,
) -> Result<Vec<u8>, String> {
    build_html_response(
        status_line.to_string(),
        vec![("Content-Type".to_string(), content_type.to_string())],
        html.to_string(),
        &surface.content_security_policy(preview_port),
    )
}

fn sanitize_design_safe_document(
    html: &str,
    surface: PreviewHtmlSurface,
    preview_revision: Option<&str>,
) -> Result<String, String> {
    let document = parse(html.to_string());
    sanitize_document_tree(&document);
    if let Some(preview_revision) = preview_revision {
        revision_local_resource_urls(&document, preview_revision);
        let html = document
            .select_first("html")
            .map_err(|_| "Documentul Design Safe nu are element html normalizat.".to_string())?;
        html.attributes
            .borrow_mut()
            .insert("data-pana-preview-revision", preview_revision.to_string());
    }
    if surface.allows_internal_bridge() {
        append_internal_bridge(&document)?;
    }
    String::from_utf8(serialize_node(&document)).map_err(|error| {
        format!(
            "Documentul Design Safe nu a putut fi serializat ca UTF-8: {}",
            error
        )
    })
}

fn revision_local_resource_urls(document: &NodeRef, preview_revision: &str) {
    for node in document.descendants() {
        let Some(element) = node.as_element() else {
            continue;
        };
        let element_name = element.name.local.as_ref().to_ascii_lowercase();
        let mut attributes = element.attributes.borrow_mut();
        for attribute_name in revisioned_url_attributes(&element_name) {
            let Some(value) = attributes.get(*attribute_name).map(str::to_string) else {
                continue;
            };
            let revised = if *attribute_name == "srcset" {
                revise_srcset(&value, preview_revision)
            } else {
                revise_resource_url(&value, preview_revision)
            };
            attributes.insert(*attribute_name, revised);
        }
    }
}

fn revisioned_url_attributes(element: &str) -> &'static [&'static str] {
    match element {
        "link" => &["href"],
        "script" => &["src"],
        "img" | "source" => &["src", "srcset"],
        "video" => &["src", "poster"],
        "audio" | "track" | "input" => &["src"],
        _ => &[],
    }
}

fn revise_srcset(value: &str, preview_revision: &str) -> String {
    value
        .split(',')
        .map(|candidate| {
            let candidate = candidate.trim();
            let mut parts = candidate.splitn(2, char::is_whitespace);
            let url = parts.next().unwrap_or_default();
            let descriptor = parts.next().unwrap_or_default().trim();
            let revised = revise_resource_url(url, preview_revision);
            if descriptor.is_empty() {
                revised
            } else {
                format!("{revised} {descriptor}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn revise_resource_url(value: &str, preview_revision: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('#')
        || trimmed.starts_with("data:")
        || trimmed.starts_with("blob:")
        || trimmed.starts_with("mailto:")
        || trimmed.starts_with("tel:")
        || (trimmed.contains("://")
            && !trimmed.starts_with("http://127.0.0.1:")
            && !trimmed.starts_with("http://localhost:"))
    {
        return value.to_string();
    }
    let separator = if trimmed.contains('?') { '&' } else { '?' };
    format!("{trimmed}{separator}__pana_preview_revision={preview_revision}")
}

fn sanitize_document_tree(document: &NodeRef) {
    let mut nodes_to_detach = Vec::new();

    for node in document.descendants() {
        let Some(element) = node.as_element() else {
            continue;
        };
        let element_name = element.name.local.as_ref().to_ascii_lowercase();

        if is_forbidden_element(&element_name) || is_navigation_meta(&node) {
            nodes_to_detach.push(node.clone());
            continue;
        }

        let mut attributes = element.attributes.borrow_mut();
        attributes.map.retain(|name, attribute| {
            !is_forbidden_attribute(&element_name, name.local.as_ref(), attribute.value.as_str())
        });
    }

    for node in nodes_to_detach {
        node.detach();
    }
}

fn is_navigation_meta(node: &NodeRef) -> bool {
    let Some(element) = node.as_element() else {
        return false;
    };
    if !element.name.local.as_ref().eq_ignore_ascii_case("meta") {
        return false;
    }
    let attributes = element.attributes.borrow();
    attributes
        .get("http-equiv")
        .map(str::trim)
        .is_some_and(is_forbidden_meta_http_equiv)
}

fn is_forbidden_attribute(_element: &str, name: &str, value: &str) -> bool {
    is_forbidden_attribute_name(name) || has_active_script_scheme(value)
}

fn append_internal_bridge(document: &NodeRef) -> Result<(), String> {
    let bridge_document = parse(format!(
        "<!doctype html><html><body><script id=\"pana-studio-bridge\">{BRIDGE_SCRIPT}</script></body></html>"
    ));
    let bridge = bridge_document
        .select_first("script#pana-studio-bridge")
        .map_err(|_| "Bridge-ul intern Design Safe nu a putut fi construit.".to_string())?
        .as_node()
        .clone();
    if bridge.text_contents() != BRIDGE_SCRIPT {
        return Err("Parserul HTML a modificat textul bridge-ului Design Safe.".to_string());
    }
    bridge.detach();

    let body = document
        .select_first("body")
        .map_err(|_| "Documentul Design Safe nu are un element body normalizat.".to_string())?;
    body.as_node().append(bridge);
    Ok(())
}

fn bridge_csp_hash() -> String {
    BASE64_STANDARD.encode(Sha256::digest(BRIDGE_SCRIPT.as_bytes()))
}

fn build_html_response(
    status_line: String,
    response_headers: Vec<(String, String)>,
    html: String,
    content_security_policy: &str,
) -> Result<Vec<u8>, String> {
    let content_type = response_headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
        .map(|(_, value)| value.clone())
        .unwrap_or_else(|| "text/html; charset=utf-8".to_string());
    let body = html.into_bytes();

    let mut headers = Vec::new();
    headers.push(status_line);
    headers.push(format!("Content-Type: {content_type}"));
    headers.push(format!("Content-Length: {}", body.len()));
    headers.push("Cache-Control: no-store".to_string());
    headers.push("X-Content-Type-Options: nosniff".to_string());
    headers.push("Referrer-Policy: no-referrer".to_string());
    headers.push(format!(
        "Content-Security-Policy: {content_security_policy}"
    ));
    headers.push("Connection: close".to_string());
    headers.push(String::new());
    headers.push(String::new());

    let mut bytes = headers.join("\r\n").into_bytes();
    bytes.extend_from_slice(&body);
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
    use sha2::{Digest, Sha256};
    use tauri_utils::html::parse;

    use crate::preview::CanvasProjectionIdentity;

    use super::{
        bind_canvas_identity_to_editor_html, bridge_csp_hash, build_prepared_design_safe_response,
        prepare_design_safe_html, PreviewHtmlSurface, BRIDGE_SCRIPT, INTERACTIVE_RUNTIME_SCRIPT,
        MAX_PREVIEW_HTML_BYTES, TEST_PREVIEW_PROXY_PORT,
    };

    fn response_headers(response: &[u8]) -> &str {
        let split = response
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("response headers");
        std::str::from_utf8(&response[..split]).expect("UTF-8 response headers")
    }

    #[test]
    fn editor_design_safe_keeps_exactly_the_internal_bridge_and_css() {
        let source = r#"<!doctype html><html><head>
            <style>.card { color: red }</style>
            <link rel="stylesheet" href="/site.css">
            <script src="/app.js"></script>
            <ScRiPt type="module">window.projectModule = true</ScRiPt>
          </head><body style="display:grid" onload="while(true){}">
            <main class="card">OK</main>
            <svg><script>window.svgScript = true</script></svg>
          </body></html>"#;

        let prepared = prepare_design_safe_html(source, "editor-security").unwrap();
        let body = prepared.editor;
        let document = parse(body.clone());
        let scripts = document
            .select("script")
            .expect("script selector")
            .collect::<Vec<_>>();

        assert_eq!(scripts.len(), 1);
        let bridge = &scripts[0];
        assert_eq!(
            bridge.attributes.borrow().get("id"),
            Some("pana-studio-bridge")
        );
        assert_eq!(bridge.as_node().text_contents(), BRIDGE_SCRIPT);
        assert!(body.contains(".card { color: red }"));
        assert!(body.contains("style=\"display:grid\""));
        assert!(body.contains("href=\"/site.css?__pana_preview_revision=editor-security\""));
        assert!(!body.contains("/app.js"));
        assert!(!body.contains("projectModule"));
        assert!(!body.contains("svgScript"));
        assert!(!body.to_ascii_lowercase().contains("onload="));
    }

    #[test]
    fn csp_authorizes_only_the_exact_bridge_hash_for_scripts() {
        let prepared =
            prepare_design_safe_html("<html><body><main>OK</main></body></html>", "editor-csp")
                .unwrap();
        let outgoing = build_prepared_design_safe_response(
            "HTTP/1.1 200 OK",
            "text/html; charset=utf-8",
            &prepared.editor,
            PreviewHtmlSurface::Editor,
            TEST_PREVIEW_PROXY_PORT,
        )
        .unwrap();
        let headers = response_headers(&outgoing);
        let expected_hash = bridge_csp_hash();
        let expected_digest = Sha256::digest(BRIDGE_SCRIPT.as_bytes());

        assert_eq!(expected_hash, BASE64_STANDARD.encode(expected_digest));
        assert!(headers.contains(&format!("script-src 'sha256-{expected_hash}'")));
        assert!(headers.contains(&format!("script-src-elem 'sha256-{expected_hash}'")));
        assert!(headers.contains("script-src-attr 'none'"));
        assert!(!headers.contains("unsafe-eval"));
        let script_directive = PreviewHtmlSurface::Editor
            .content_security_policy(TEST_PREVIEW_PROXY_PORT)
            .split(';')
            .find(|directive| directive.trim_start().starts_with("script-src "))
            .expect("script-src directive")
            .trim()
            .to_string();
        assert_eq!(
            script_directive,
            format!("script-src 'sha256-{expected_hash}'")
        );
    }

    #[test]
    fn sandboxed_design_safe_csp_authorizes_only_the_exact_preview_asset_origin() {
        let preview_proxy_port = 43_123;
        let prepared =
            prepare_design_safe_html("<html><body>Visitor</body></html>", "visitor-csp").unwrap();
        let outgoing = build_prepared_design_safe_response(
            "HTTP/1.1 200 OK",
            "text/html; charset=utf-8",
            &prepared.visitor,
            PreviewHtmlSurface::Visitor,
            preview_proxy_port,
        )
        .unwrap();
        let headers = response_headers(&outgoing);
        let asset_origin = format!("http://127.0.0.1:{preview_proxy_port}");

        for directive_name in [
            "style-src ",
            "style-src-elem ",
            "img-src ",
            "font-src ",
            "media-src ",
        ] {
            let directive = headers
                .split(';')
                .find(|directive| directive.trim_start().starts_with(directive_name))
                .unwrap_or_else(|| panic!("missing {directive_name} directive"));
            assert!(directive.contains(&asset_origin), "{directive}");
        }

        let script_directive = headers
            .split(';')
            .find(|directive| directive.trim_start().starts_with("script-src "))
            .expect("script-src directive");
        assert!(!script_directive.contains(&asset_origin));
        assert!(!headers.contains("http://127.0.0.1:*"));
        assert!(!headers.contains("http://localhost:*"));
    }

    #[test]
    fn visitor_design_safe_contains_zero_scripts() {
        let source = r#"<html><body><script>alert(1)</script><main>Visitor</main></body></html>"#;
        let prepared = prepare_design_safe_html(source, "visitor-zero-script").unwrap();
        let body = prepared.visitor;
        let document = parse(body.clone());
        let outgoing = build_prepared_design_safe_response(
            "HTTP/1.1 200 OK",
            "text/html; charset=utf-8",
            &body,
            PreviewHtmlSurface::Visitor,
            TEST_PREVIEW_PROXY_PORT,
        )
        .unwrap();

        assert_eq!(document.select("script").expect("selector").count(), 0);
        assert!(body.contains("Visitor"));
        assert!(response_headers(&outgoing).contains("script-src 'none'"));
        assert!(!body.contains("pana-studio-bridge"));
    }

    #[test]
    fn interactive_surface_preserves_project_js_in_an_opaque_non_privileged_runtime() {
        let source = r#"<!doctype html><html><head>
            <meta http-equiv="Content-Security-Policy" content="script-src *">
            <script src="/page.js"></script>
        </head><body><button onclick="window.clicked=true">Run</button>
            <script>window.projectInline=true</script>
        </body></html>"#;
        let prepared = prepare_design_safe_html(source, "interactive-7").unwrap();

        assert!(!prepared.editor.contains("window.projectInline"));
        assert!(prepared.interactive.contains("window.projectInline"));
        let interactive_document = parse(prepared.interactive.clone());
        assert_eq!(
            interactive_document
                .select_first("button")
                .unwrap()
                .attributes
                .borrow()
                .get("onclick"),
            Some("window.clicked=true")
        );
        assert!(prepared
            .interactive
            .contains("/page.js?__pana_preview_revision=interactive-7"));
        assert!(prepared.interactive.contains(INTERACTIVE_RUNTIME_SCRIPT));
        assert_eq!(
            interactive_document
                .select("meta[http-equiv]")
                .unwrap()
                .count(),
            0
        );

        let response = build_prepared_design_safe_response(
            "HTTP/1.1 200 OK",
            "text/html; charset=utf-8",
            &prepared.interactive,
            PreviewHtmlSurface::Interactive,
            TEST_PREVIEW_PROXY_PORT,
        )
        .unwrap();
        let headers = response_headers(&response);
        assert!(headers.contains(&format!(
            "script-src http://127.0.0.1:{}",
            TEST_PREVIEW_PROXY_PORT
        )));
        assert!(headers.contains("connect-src http://127.0.0.1:"));
        assert!(!headers.contains("allow-same-origin"));
    }

    #[test]
    fn confirmed_upstream_revision_is_injected_on_the_normalized_html_root() {
        let prepared =
            prepare_design_safe_html("<main>Rendered Tera output</main>", "generation-42").unwrap();
        let document = parse(prepared.editor);
        let html = document.select_first("html").expect("normalized html root");

        assert_eq!(
            html.attributes.borrow().get("data-pana-preview-revision"),
            Some("generation-42")
        );
    }

    #[test]
    fn editor_document_binds_the_exact_canvas_identity_without_exposing_it_to_visitor() {
        let mut prepared =
            prepare_design_safe_html("<html><body><main>Canvas</main></body></html>", "preview-7")
                .unwrap();
        let identity = CanvasProjectionIdentity {
            project_root: "/project".to_string(),
            runtime_session_id: "runtime-7".to_string(),
            workspace_revision: 7,
            transaction_id: "canvas-transaction-7".to_string(),
            preview_revision: "preview-7".to_string(),
        };
        bind_canvas_identity_to_editor_html(&mut prepared, &identity).unwrap();

        let editor = parse(prepared.editor);
        let html = editor.select_first("html").unwrap();
        let attributes = html.attributes.borrow();
        assert_eq!(
            attributes.get("data-pana-canvas-project-root"),
            Some("/project")
        );
        assert_eq!(
            attributes.get("data-pana-canvas-runtime-session-id"),
            Some("runtime-7")
        );
        assert_eq!(
            attributes.get("data-pana-canvas-workspace-revision"),
            Some("7")
        );
        assert_eq!(
            attributes.get("data-pana-canvas-transaction-id"),
            Some("canvas-transaction-7")
        );
        drop(attributes);

        let scripts = editor.select("script").unwrap().collect::<Vec<_>>();
        assert_eq!(scripts.len(), 1);
        assert_eq!(scripts[0].as_node().text_contents(), BRIDGE_SCRIPT);

        assert!(!prepared.visitor.contains("data-pana-canvas-transaction-id"));
        assert!(!prepared.visitor.contains("data-pana-canvas-project-root"));
    }

    #[test]
    fn internal_bridge_has_no_arbitrary_javascript_or_anime_execution_path() {
        assert!(!BRIDGE_SCRIPT.contains("set-live-js"));
        assert!(!BRIDGE_SCRIPT.contains("motion-timeline-preview-command"));
        assert!(!BRIDGE_SCRIPT.contains("/__pana/anime.min.js"));
        assert!(!BRIDGE_SCRIPT.contains("createElement(\"script\")"));
        assert!(!BRIDGE_SCRIPT.contains("eval("));
        assert!(!BRIDGE_SCRIPT.contains("new Function"));
    }

    #[test]
    fn internal_bridge_reapplies_design_safe_policy_to_live_dom_inputs() {
        assert!(BRIDGE_SCRIPT.contains("function sanitizeDesignSafeTree(root)"));
        assert!(BRIDGE_SCRIPT.contains("sanitizeDesignSafeTree(nextDocument)"));
        assert!(BRIDGE_SCRIPT.contains("sanitizeDesignSafeTree(template.content)"));
        assert!(BRIDGE_SCRIPT.contains("designSafeAttributeAllowed(selected, normalized, value)"));
        assert!(!BRIDGE_SCRIPT.contains("designSafeAttributeAllowed(renderedHtmlSelectionElement"));
        assert!(BRIDGE_SCRIPT.contains("designSafeElementAllowedName(normalizedTag)"));
        assert_eq!(BRIDGE_SCRIPT.matches("sanitizeDesignSafeTree(").count(), 4);
    }

    #[test]
    fn internal_bridge_consumes_shared_html_schema_and_latest_wins_attribute_epochs() {
        assert!(BRIDGE_SCRIPT.contains("var HTML_EDITOR_SCHEMA ="));
        assert!(BRIDGE_SCRIPT.contains("HTML_EDITOR_SCHEMA.designSafe"));
        assert!(BRIDGE_SCRIPT.contains("var draftEpoch = Number(data.draftEpoch)"));
        assert!(BRIDGE_SCRIPT.contains("activeLiveAttributeDraft.draftEpoch >= draftEpoch"));
        assert!(BRIDGE_SCRIPT.contains("draftEpoch < activeLiveAttributeDraft.draftEpoch"));
    }

    #[test]
    fn internal_bridge_defines_structure_sync_before_boot_uses_it() {
        let definition = BRIDGE_SCRIPT
            .find("function syncStructure()")
            .expect("bridge-ul trebuie să definească sincronizarea structurii");
        let boot_call = BRIDGE_SCRIPT
            .rfind("syncStructure();")
            .expect("boot-ul trebuie să publice structura inițială");

        assert!(definition < boot_call);
        assert!(BRIDGE_SCRIPT.contains("post(\"structure\", { sections: collectPageSections() });"));
    }

    #[test]
    fn internal_bridge_accepts_commands_only_from_the_mounted_parent_frame() {
        assert!(BRIDGE_SCRIPT.contains(
            "window.addEventListener(\"message\", function (event) {\n    // The Design Safe document has a single trusted controller"
        ));
        assert!(BRIDGE_SCRIPT.contains("if (event.source !== window.parent)"));
        assert!(BRIDGE_SCRIPT.contains("data.source !== SOURCE_APP"));
    }

    #[test]
    fn synthetic_dom_events_cannot_enter_bridge_gesture_paths() {
        assert!(BRIDGE_SCRIPT.contains("function isTrustedPreviewGesture(event)"));
        assert!(BRIDGE_SCRIPT.contains("event.isTrusted === true"));

        for guarded_path in [
            "function handlePreviewPointerDown(event) {\n    if (!isTrustedPreviewGesture(event)) return;",
            "function handlePreviewPointerMove(event) {\n    if (!isTrustedPreviewGesture(event)) return;",
            "function handlePreviewHoverPointerMove(event) {\n    if (!isTrustedPreviewGesture(event)) return;",
            "function handlePreviewPointerUp(event) {\n    if (!isTrustedPreviewGesture(event)) return;",
            "function handlePreviewShortcut(event) {\n    if (!isTrustedPreviewGesture(event)) return;",
            "function openPreviewContextMenuFromEvent(event) {\n    if (!isTrustedPreviewGesture(event)) return;",
        ] {
            assert!(
                BRIDGE_SCRIPT.contains(guarded_path),
                "missing trusted-event gate for {guarded_path}"
            );
        }

        // Template edit, Delete/Backspace, pointerdown feedback and selection
        // click use anonymous listeners; keep all of them behind the same
        // bridge-local gate as well.
        assert!(
            BRIDGE_SCRIPT
                .matches("if (!isTrustedPreviewGesture(event)) return;")
                .count()
                >= 10
        );
        assert!(!BRIDGE_SCRIPT.contains("data.isTrusted"));
        assert!(!BRIDGE_SCRIPT.contains("payload.isTrusted"));
    }

    #[test]
    fn active_html_and_navigation_surfaces_are_removed_parser_first() {
        let source = r#"<html><head>
            <base href="https://attacker.invalid/">
            <meta http-equiv="refresh" content="0;url=https://attacker.invalid/">
            <meta http-equiv="Content-Security-Policy" content="script-src * 'unsafe-inline'">
          </head><body>
            <a HREF="&#x6a;ava&#x73;cript:alert(1)" TARGET="_top" ping="/track">go</a>
            <a id="safe-link" href="/despre">Despre</a>
            <form action="/submit"><button formaction="/other">send</button></form>
            <iframe srcdoc="<script>while(true){}</script>"></iframe>
            <object data="/plugin"></object><embed src="/plugin"><applet></applet>
            <img src="/ok.png" ONERROR="alert(1)">
          </body></html>"#;
        let prepared = prepare_design_safe_html(source, "active-surface-security").unwrap();
        let document = parse(prepared.editor);

        for selector in [
            "base",
            "meta[http-equiv]",
            "iframe",
            "object",
            "embed",
            "applet",
        ] {
            assert_eq!(
                document.select(selector).expect("selector").count(),
                0,
                "{selector} must be absent"
            );
        }
        let anchor = document.select_first("a").expect("anchor");
        let anchor_attributes = anchor.attributes.borrow();
        assert!(anchor_attributes.get("href").is_none());
        assert!(anchor_attributes.get("target").is_none());
        assert!(anchor_attributes.get("ping").is_none());
        drop(anchor_attributes);
        let safe_anchor = document.select_first("a#safe-link").expect("safe anchor");
        assert_eq!(safe_anchor.attributes.borrow().get("href"), Some("/despre"));
        let form = document.select_first("form").expect("form");
        assert!(form.attributes.borrow().get("action").is_none());
        let button = document.select_first("button").expect("button");
        assert!(button.attributes.borrow().get("formaction").is_none());
        let image = document.select_first("img").expect("image");
        assert_eq!(
            image.attributes.borrow().get("src"),
            Some("/ok.png?__pana_preview_revision=active-surface-security")
        );
        assert!(image.attributes.borrow().get("onerror").is_none());
    }

    #[test]
    fn malformed_and_mixed_namespace_scripts_do_not_survive() {
        let source = r#"<html><body>
            <SCRIPT SRC=/one.js></SCRIPT>
            <svg xmlns="http://www.w3.org/2000/svg"><ScRiPt href="/two.js" /></svg>
            <math><script>window.mathScript = true</script></math>
            <script type=module>window.unclosed = true
          </body></html>"#;
        let prepared = prepare_design_safe_html(source, "malformed-security").unwrap();
        let document = parse(prepared.visitor.clone());

        assert_eq!(document.select("script").expect("selector").count(), 0);
        assert!(!prepared.visitor.contains("/one.js"));
        assert!(!prepared.visitor.contains("/two.js"));
        assert!(!prepared.visitor.contains("mathScript"));
        assert!(!prepared.visitor.contains("unclosed"));
    }

    #[test]
    fn oversized_html_is_rejected_before_parser_and_injection_allocations() {
        let html = "a".repeat(MAX_PREVIEW_HTML_BYTES + 1);
        let error = prepare_design_safe_html(&html, "oversized").unwrap_err();
        assert!(error.contains("depășește limita"), "{error}");
    }
}
