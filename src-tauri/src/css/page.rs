mod imports;
mod model;
mod paths;
mod stylesheet;
mod target;

#[cfg(test)]
mod tests;

pub use model::{PageCssTarget, PageCssWriteResult, ReusableCssWriteResult, WrittenProjectFile};
pub use paths::{page_css_href, page_scss_relative_path, reusable_scss_relative_path};
pub use stylesheet::{
    consumer_stylesheet_imports_reusable, plan_page_stylesheet_link_source,
    plan_page_stylesheet_link_writes_with_reader, prepare_page_stylesheet_source,
    prepare_reusable_consumer_stylesheet_source, remove_page_stylesheet_link,
};
pub use target::page_target_for_template;
