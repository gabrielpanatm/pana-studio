mod draft_store;
mod generator;
mod motion;
mod parser;
mod paths;
mod reader;
mod save_contract;
mod scan;
mod session_binding;
mod template;
mod types;

pub use draft_store::{
    PageJsDraftStageInput, PageJsDraftStageReceipt, PageJsDraftStore, PageJsDraftStoreSnapshot,
};
pub use generator::generate_page_js;
pub use parser::parse_page_js;
pub use paths::{js_relative_path, template_to_slug};
pub use reader::{read_page_data_anims, read_page_js_config};
pub use save_contract::plan_page_js_save_for_project;
pub(crate) use save_contract::{page_js_text_changes_from_plan, page_js_text_deletes_from_plan};
pub use scan::extract_data_anims;
pub use session_binding::{
    require_page_js_draft_session_identity, require_page_js_file_buffer_identity,
    PageJsCommandReceipt, PageJsRequestIdentity,
};
pub use template::{
    ensure_base_scripts_block, ensure_page_scripts_block, ensure_script_tags, extract_extends,
    page_scripts_html, remove_page_scripts_contract,
};
pub use types::{PageJsConfig, PanaComponent};
