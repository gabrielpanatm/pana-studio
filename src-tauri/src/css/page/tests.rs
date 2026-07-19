use std::path::Path;

use super::{
    imports::{relative_scss_import_path, variables_import_path},
    page_scss_relative_path,
    paths::supports_page_css,
    stylesheet::{ensure_page_css_block, remove_page_stylesheet_link},
};

#[test]
fn derives_page_scss_from_template() {
    assert_eq!(
        page_scss_relative_path("templates/index.html"),
        "sass/pagini/index.scss"
    );
    assert_eq!(
        page_scss_relative_path("templates/atelier/home_page.html"),
        "sass/pagini/atelier-home-page.scss"
    );
}

#[test]
fn derives_theme_page_scss_from_theme_template() {
    assert_eq!(
        page_scss_relative_path("themes/pana-studio/templates/index.html"),
        "themes/pana-studio/sass/pagini/index.scss"
    );
    assert_eq!(
        page_scss_relative_path("sursa/themes/pana-studio/templates/atelier/home_page.html"),
        "themes/pana-studio/sass/pagini/atelier-home-page.scss"
    );
}

#[test]
fn detects_theme_partials_as_not_page_owned_css() {
    assert!(!supports_page_css(
        "themes/pana-studio/templates/partials/header.html"
    ));
    assert!(supports_page_css("themes/pana-studio/templates/index.html"));
}

#[test]
fn adds_css_block_to_child_template() {
    let source = "{% extends \"base.html\" %}\n\n{% block content %}Hello{% endblock %}\n";
    let result = ensure_page_css_block(source, "/pagini/index.css", false);
    assert!(result.contains(
        "{% block css_pagina %}<link rel=\"stylesheet\" href=\"/pagini/index.css\">{% endblock %}"
    ));
    assert!(result.contains("{% block content %}Hello{% endblock %}"));
}

#[test]
fn appends_link_to_existing_css_block() {
    let source =
        "{% block css_pagina %}<link rel=\"stylesheet\" href=\"/pagini/old.css\">{% endblock %}";
    let result = ensure_page_css_block(source, "/pagini/index.css", false);
    assert!(result.contains("/pagini/old.css"));
    assert!(result.contains("/pagini/index.css"));
}

#[test]
fn writes_cachebusted_css_link_when_requested() {
    let source = "{% extends \"base.html\" %}\n\n{% block content %}Hello{% endblock %}\n";
    let result = ensure_page_css_block(source, "/pagini/index.css", true);
    assert!(result.contains("{{ get_url(path='pagini/index.css', cachebust=true) }}"));
}

#[test]
fn detects_existing_cachebusted_css_link() {
    let source = r#"{% block css_pagina %}<link rel="stylesheet" href="{{ get_url(path="pagini/index.css", cachebust=true) }}">{% endblock %}"#;
    let result = ensure_page_css_block(source, "/pagini/index.css", false);
    assert_eq!(result, source);
}

#[test]
fn removes_page_css_link_and_empty_block() {
    let source = r#"{% extends "base.html" %}
{% block css_pagina %}<link rel="stylesheet" href="/pagini/index.css">{% endblock %}
{% block content %}Hello{% endblock %}
"#;
    let result = remove_page_stylesheet_link(source, "/pagini/index.css");
    assert!(!result.contains("/pagini/index.css"));
    assert!(!result.contains("css_pagina"));
    assert!(result.contains("{% block content %}Hello{% endblock %}"));
}

#[test]
fn removes_page_css_link_but_keeps_super_block() {
    let source = r#"{% block css_pagina %}
{{ super() }}
<link rel="stylesheet" href="{{ get_url(path='pagini/index.css', cachebust=true) }}">
{% endblock %}"#;
    let result = remove_page_stylesheet_link(source, "/pagini/index.css");
    assert!(!result.contains("/pagini/index.css"));
    assert!(result.contains("{{ super() }}"));
    assert!(result.contains("css_pagina"));
}

#[test]
fn computes_relative_import_to_partial() {
    let result = relative_scss_import_path(
        Path::new("sass/pagini/index.scss"),
        Path::new("sass/css-framework/_variabile.scss"),
    );
    assert_eq!(result.as_deref(), Some("../css-framework/variabile"));
}

#[test]
fn computes_theme_relative_import_to_theme_variables() {
    let result = variables_import_path(
        "themes/pana-studio/sass/pagini/index.scss",
        ["themes/pana-studio/sass/css-framework/_variabile.scss".to_string()],
        Some("pana-studio"),
    );

    assert_eq!(result.as_deref(), Some("../css-framework/variabile"));
}

#[test]
fn local_page_stylesheet_can_import_active_theme_variables() {
    let result = variables_import_path(
        "sass/pagini/dada.scss",
        ["themes/pana-studio/sass/css-framework/_variabile.scss".to_string()],
        Some("pana-studio"),
    );

    assert_eq!(
        result.as_deref(),
        Some("../../themes/pana-studio/sass/css-framework/variabile")
    );
}
