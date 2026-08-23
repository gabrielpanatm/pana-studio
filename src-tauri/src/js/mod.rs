mod draft_store;
mod generator;
mod motion;
mod motion_compiler;
pub(crate) mod motion_model;
mod motion_source;
mod paths;
mod reader;
mod save_contract;
mod session_binding;
mod template;
mod types;

pub use draft_store::{
    PageJsDraftStageInput, PageJsDraftStageReceipt, PageJsDraftStore, PageJsDraftStoreSnapshot,
};
pub use generator::{generate_page_js, PageRuntimePlan};
pub(crate) use motion::{generate_motion_preview_payload, generate_motion_preview_runtime};
pub use motion_model::{
    MotionAction, MotionBehavior, MotionCustomCode, MotionDiagnostic, MotionDocument,
    MotionInteraction, MotionRuntimeContract, MotionTarget, MotionTargetKind,
};
pub use motion_source::{parse_motion_source, serialize_motion_source};
pub use paths::{js_relative_path, template_to_slug};
pub use paths::{motion_source_relative_path, template_path_from_motion_source};
pub use reader::read_page_motion_config;
pub use save_contract::plan_page_js_save_for_project;
pub(crate) use save_contract::{
    page_js_text_changes_from_plan, page_js_text_deletes_from_plan,
    plan_page_js_save_for_project_preserving_source,
};
pub use session_binding::{
    require_page_js_draft_session_identity, require_page_js_file_buffer_identity,
    PageJsCommandReceipt, PageJsRequestIdentity,
};
pub use template::{
    ensure_base_scripts_block, ensure_page_scripts_block, ensure_script_tags, extract_extends,
    page_scripts_html, remove_page_scripts_contract,
};
pub use types::PageJsConfig;
