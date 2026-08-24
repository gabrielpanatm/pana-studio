pub(super) mod frontmatter;
pub(super) mod templates;

pub(super) use templates::{
    ensure_metadata_contracts, stage_remove_field_values, stage_remove_model_values,
    stage_rename_field_values, stage_rename_template_references, stage_replace_model_values,
};
