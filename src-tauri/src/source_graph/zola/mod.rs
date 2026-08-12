mod content;
mod data_files;
mod paths;
mod runtime;
mod template_references;
mod templates;
mod tera_calls;

pub(crate) use content::{
    find_zola_frontmatter_template_literal, normalize_zola_content_reference,
    parse_zola_content_frontmatter, resolve_zola_page_template, resolve_zola_section_page_template,
    rewrite_zola_content_load_reference, rewrite_zola_content_reference,
    zola_content_load_reference, zola_content_page_kind, zola_content_project_file_reference,
    zola_content_reference_for_relation, zola_content_url, zola_frontmatter_range,
    zola_frontmatter_template_for_key,
};
pub(crate) use data_files::{
    conventional_zola_data_file, editable_local_toml_path, resolve_zola_load_data_file,
    ResolvedZolaDataFile, ZolaDataResolutionContext,
};
pub(crate) use paths::{
    data_file_reference_keys, internal_content_path, local_static_asset_project_file_reference,
    local_zola_data_project_file_reference, normalize_static_asset_reference,
    normalize_zola_data_file_reference, rewrite_zola_data_file_reference,
    rewrite_zola_static_asset_reference, static_asset_logical_path, static_asset_reference,
    static_asset_reference_from_style, static_asset_reference_keys,
    zola_data_file_reference_for_rewrite, zola_static_asset_reference_for_rewrite,
};
pub(crate) use runtime::{
    collect_zola_runtime_uses, ZolaTeraRuntimeAvailability, ZolaTeraRuntimeKind,
    PINNED_ZOLA_REVISION,
};
pub(crate) use template_references::extract_zola_template_references;
pub(crate) use templates::{
    local_zola_template_project_file_reference, normalize_zola_template_reference,
    rewrite_zola_template_reference, zola_template_name_for_path, zola_template_reference_keys,
};
pub(crate) use tera_calls::{
    count_dynamic_zola_load_data_paths, parse_zola_path_calls, zola_path_function_for_relation,
    TeraZolaPathFunction,
};
