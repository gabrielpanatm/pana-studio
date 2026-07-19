mod imports;
mod model;
mod paths;
mod stylesheet;
mod target;

#[cfg(test)]
mod tests;

pub use model::{PageCssTarget, PageCssWriteResult, WrittenProjectFile};
pub use paths::{page_css_href, page_scss_relative_path};
pub use stylesheet::{
    plan_page_stylesheet_link_source, plan_page_stylesheet_link_writes_with_reader,
    prepare_page_stylesheet_source, remove_page_stylesheet_link,
};
pub use target::page_target_for_template;
