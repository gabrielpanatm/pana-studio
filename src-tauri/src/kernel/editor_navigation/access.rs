use super::*;

/// Resolves the semantic node exposed by every editor surface for the current
/// edit scope. Descendants of a closed boundary are projected as that nearest
/// boundary until its Rust-issued grant is presented.
pub(crate) fn editor_navigation_access_node<'a>(
    snapshot: &'a EditorNavigationSnapshot,
    requested_node_id: &str,
    authorized_edit_scope_id: Option<&str>,
) -> Option<&'a EditorNavigationNode> {
    let requested = editor_navigation_node(snapshot, requested_node_id)?;
    let Some(required_scope_id) = requested.capabilities.requires_edit_scope_id.as_deref() else {
        return Some(requested);
    };
    if authorized_edit_scope_id == Some(required_scope_id) {
        return Some(requested);
    }
    editor_navigation_node(snapshot, required_scope_id)
        .filter(|candidate| candidate.kind == EditorNavigationNodeKind::Boundary)
        .or(Some(requested))
}
