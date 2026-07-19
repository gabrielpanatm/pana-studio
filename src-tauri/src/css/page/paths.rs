use crate::zola_theme::{
    page_css_href as resolve_page_css_href,
    page_scss_relative_path as resolve_page_scss_relative_path,
    style_root_for_page_stylesheet as resolve_style_root_for_page_stylesheet,
    supports_page_css as resolve_supports_page_css,
};

pub(super) fn style_root_for_page_stylesheet(relative_path: &str) -> String {
    resolve_style_root_for_page_stylesheet(relative_path)
}

pub fn page_scss_relative_path(template_path: &str) -> String {
    resolve_page_scss_relative_path(template_path)
}

pub fn page_css_href(template_path: &str) -> String {
    resolve_page_css_href(template_path)
}

pub(super) fn supports_page_css(template_path: &str) -> bool {
    resolve_supports_page_css(template_path)
}
