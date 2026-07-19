use std::hash::{Hash, Hasher};

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
        SourceNodeKind::Html => "html",
        SourceNodeKind::Extends => "extends",
        SourceNodeKind::Block => "block",
        SourceNodeKind::Include => "include",
        SourceNodeKind::Import => "import",
        SourceNodeKind::Macro => "macro",
        SourceNodeKind::For => "for",
        SourceNodeKind::If => "if",
        SourceNodeKind::Set => "set",
        SourceNodeKind::With => "with",
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
