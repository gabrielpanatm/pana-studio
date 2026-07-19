use crate::css::page::{
    model::PageCssTarget,
    paths::{page_css_href, page_scss_relative_path, supports_page_css},
};

pub fn page_target_for_template(
    template_path: Option<&str>,
    selector: &str,
    fallback_file: Option<&str>,
) -> PageCssTarget {
    let Some(template_path) = template_path.filter(|path| !path.trim().is_empty()) else {
        let file = fallback_file.unwrap_or("styles.css").to_string();
        return PageCssTarget {
            exists: false,
            file,
            selector: selector.to_string(),
            target_kind: "fallback".to_string(),
            linked: false,
            href: None,
            template_path: None,
            page_owned: false,
            reason: "Nu există template sursă pentru o foaie CSS de pagină.".to_string(),
        };
    };

    if !supports_page_css(template_path) {
        let file = fallback_file.unwrap_or("styles.css").to_string();
        return PageCssTarget {
            exists: false,
            file,
            selector: selector.to_string(),
            target_kind: "fallback".to_string(),
            linked: false,
            href: None,
            template_path: Some(template_path.to_string()),
            page_owned: false,
            reason: "Elementul vine dintr-un partial; folosesc fișierul CSS curent.".to_string(),
        };
    }

    let file = page_scss_relative_path(template_path);
    let href = page_css_href(template_path);
    PageCssTarget {
        exists: false,
        linked: false,
        file,
        selector: selector.to_string(),
        target_kind: "page".to_string(),
        href: Some(href),
        template_path: Some(template_path.to_string()),
        page_owned: true,
        reason: "Regula va fi creată în fișierul SCSS al paginii.".to_string(),
    }
}
