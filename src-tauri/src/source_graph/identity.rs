use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
};

use crate::source_graph::model::{SourceNodeKind, SourceRelationKind};

pub(crate) fn source_node_id(
    file: &str,
    kind: &SourceNodeKind,
    label: &str,
    start: Option<usize>,
    end: Option<usize>,
) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    file.hash(&mut hasher);
    node_kind_key(kind).hash(&mut hasher);
    label.hash(&mut hasher);
    start.hash(&mut hasher);
    end.hash(&mut hasher);
    format!("sg_{:016x}", hasher.finish())
}

/// Assigns deterministic semantic identities without coupling them to byte
/// offsets. The occurrence is local to the same file/kind/label signature, so
/// inserting unrelated source before a node does not invalidate its identity.
#[derive(Default)]
pub(crate) struct SourceIdentityAssigner {
    occurrences: HashMap<(String, &'static str, String), usize>,
}

impl SourceIdentityAssigner {
    pub(crate) fn next(&mut self, file: &str, kind: &SourceNodeKind, label: &str) -> String {
        let key = (file.to_string(), node_kind_key(kind), label.to_string());
        let occurrence = self.occurrences.entry(key).or_default();
        let id = stable_source_node_id(file, kind, label, *occurrence);
        *occurrence += 1;
        id
    }
}

pub(crate) fn stable_source_node_id(
    file: &str,
    kind: &SourceNodeKind,
    label: &str,
    occurrence: usize,
) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    "pana-source-node-v2".hash(&mut hasher);
    file.hash(&mut hasher);
    node_kind_key(kind).hash(&mut hasher);
    label.hash(&mut hasher);
    occurrence.hash(&mut hasher);
    format!("sg_{:016x}", hasher.finish())
}

pub(crate) fn source_relation_id(
    from: &str,
    to: &str,
    kind: &SourceRelationKind,
    label: &str,
) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    from.hash(&mut hasher);
    to.hash(&mut hasher);
    relation_kind_key(kind).hash(&mut hasher);
    label.hash(&mut hasher);
    format!("rel_{:016x}", hasher.finish())
}

fn node_kind_key(kind: &SourceNodeKind) -> &'static str {
    match kind {
        SourceNodeKind::Page => "page",
        SourceNodeKind::Template => "template",
        SourceNodeKind::Partial => "partial",
        SourceNodeKind::Style => "style",
        SourceNodeKind::Script => "script",
        SourceNodeKind::Asset => "asset",
        SourceNodeKind::DataFile => "data_file",
        SourceNodeKind::DataTable => "data_table",
        SourceNodeKind::DataArray => "data_array",
        SourceNodeKind::DataValue => "data_value",
        SourceNodeKind::DataComment => "data_comment",
        SourceNodeKind::ConfigFile => "config_file",
        SourceNodeKind::Html => "html",
        SourceNodeKind::BlockMarker => "block_marker",
        SourceNodeKind::MacroCall => "macro_call",
        SourceNodeKind::FunctionCall => "function_call",
        SourceNodeKind::Shortcode => "shortcode",
        SourceNodeKind::Extends => "extends",
        SourceNodeKind::Block => "block",
        SourceNodeKind::Include => "include",
        SourceNodeKind::Import => "import",
        SourceNodeKind::Macro => "macro",
        SourceNodeKind::For => "for",
        SourceNodeKind::If => "if",
        SourceNodeKind::Elif => "elif",
        SourceNodeKind::Else => "else",
        SourceNodeKind::Set => "set",
        SourceNodeKind::SetGlobal => "set_global",
        SourceNodeKind::Filter => "filter",
        SourceNodeKind::Break => "break",
        SourceNodeKind::Continue => "continue",
        SourceNodeKind::Super => "super",
        SourceNodeKind::TeraVariable => "tera_variable",
        SourceNodeKind::TeraComment => "tera_comment",
        SourceNodeKind::Raw => "raw",
        SourceNodeKind::Tera => "tera",
    }
}

fn relation_kind_key(kind: &SourceRelationKind) -> &'static str {
    match kind {
        SourceRelationKind::PageTemplate => "page_template",
        SourceRelationKind::SectionPageTemplate => "section_page_template",
        SourceRelationKind::GetsPage => "gets_page",
        SourceRelationKind::GetsSection => "gets_section",
        SourceRelationKind::InternalContentLink => "internal_content_link",
        SourceRelationKind::AssetUrl => "asset_url",
        SourceRelationKind::AssetHash => "asset_hash",
        SourceRelationKind::DataLoad => "data_load",
        SourceRelationKind::DataFileLoad => "data_file_load",
        SourceRelationKind::ContentDataLoad => "content_data_load",
        SourceRelationKind::ImageMetadata => "image_metadata",
        SourceRelationKind::ImageResize => "image_resize",
        SourceRelationKind::Extends => "extends",
        SourceRelationKind::Includes => "includes",
        SourceRelationKind::Imports => "imports",
        SourceRelationKind::DefinesBlock => "defines_block",
        SourceRelationKind::OverridesBlock => "overrides_block",
        SourceRelationKind::UsesStyle => "uses_style",
        SourceRelationKind::UsesScript => "uses_script",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_ids_ignore_source_offsets_and_count_only_equal_signatures() {
        let mut first = SourceIdentityAssigner::default();
        let title = first.next("templates/index.html", &SourceNodeKind::Html, "<h1 .title>");
        let paragraph = first.next("templates/index.html", &SourceNodeKind::Html, "<p>");

        let mut after_unrelated_insert = SourceIdentityAssigner::default();
        let _unrelated =
            after_unrelated_insert.next("templates/index.html", &SourceNodeKind::Html, "<aside>");
        let shifted_title = after_unrelated_insert.next(
            "templates/index.html",
            &SourceNodeKind::Html,
            "<h1 .title>",
        );
        let shifted_paragraph =
            after_unrelated_insert.next("templates/index.html", &SourceNodeKind::Html, "<p>");

        assert_eq!(title, shifted_title);
        assert_eq!(paragraph, shifted_paragraph);
        assert_ne!(
            first.next("templates/index.html", &SourceNodeKind::Html, "<p>",),
            paragraph
        );
    }
}
