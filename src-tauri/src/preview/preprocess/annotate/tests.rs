use super::{index::SourceIdIndex, inject::inject_tpl_src_on_line, preprocess_template};

fn index_with(entries: &[(&str, &str)]) -> SourceIdIndex {
    let mut index = SourceIdIndex::default();
    for (source_location, source_id) in entries {
        index
            .by_source_location
            .insert((*source_location).to_string(), (*source_id).to_string());
    }
    index
}

fn index_with_template(entries: &[(&str, &str)]) -> SourceIdIndex {
    let mut index = SourceIdIndex::default();
    for (source_location, source_id) in entries {
        index
            .by_template_source_location
            .insert((*source_location).to_string(), (*source_id).to_string());
    }
    index
}

#[test]
fn injects_on_simple_tag() {
    let index = index_with(&[("templates/index.html:5:1", "sg_div")]);
    let result = inject_tpl_src_on_line(
        r#"<div class="foo">"#,
        "templates/index.html",
        5,
        Some(&index),
    );
    assert_eq!(result, r#"<div class="foo" data-pana-source-id="sg_div">"#);
}

#[test]
fn injects_on_tag_without_class() {
    let index = index_with(&[("templates/index.html:3:1", "sg_div")]);
    let result = inject_tpl_src_on_line("<div>", "templates/index.html", 3, Some(&index));
    assert_eq!(result, r#"<div data-pana-source-id="sg_div">"#);
}

#[test]
fn skips_closing_tags() {
    let result = inject_tpl_src_on_line("</div>", "templates/index.html", 6, None);
    assert_eq!(result, "</div>");
}

#[test]
fn skips_tera_blocks() {
    let result = inject_tpl_src_on_line("{% if x %}", "templates/index.html", 1, None);
    assert_eq!(result, "{% if x %}");
}

#[test]
fn handles_tera_in_attribute() {
    let index = index_with(&[("templates/base.html:10:1", "sg_link")]);
    let result = inject_tpl_src_on_line(
        r#"<a href="{{ page.permalink }}" class="nav-link">"#,
        "templates/base.html",
        10,
        Some(&index),
    );
    assert!(result.contains("data-pana-source-id=\"sg_link\""));
    assert!(result.contains("href=\"{{ page.permalink }}\""));
}

#[test]
fn handles_mixed_tera_and_html_on_same_line() {
    let index = index_with(&[("templates/partials/nav.html:4:16", "sg_li")]);
    let result = inject_tpl_src_on_line(
        r#"{% if active %}<li class="active">{% endif %}"#,
        "templates/partials/nav.html",
        4,
        Some(&index),
    );
    assert!(result.contains("data-pana-source-id=\"sg_li\""));
    assert!(result.contains("{% if active %}"));
    assert!(result.contains("{% endif %}"));
}

#[test]
fn does_not_double_inject() {
    let line = r#"<div data-pana-source-id="sg_div">"#;
    let result = inject_tpl_src_on_line(line, "templates/index.html", 1, None);
    assert_eq!(result, line);
}

#[test]
fn injects_on_multiple_tags_same_line() {
    let index = index_with(&[
        ("templates/index.html:7:1", "sg_div"),
        ("templates/index.html:7:6", "sg_span"),
    ]);
    let result = inject_tpl_src_on_line(
        r#"<div><span class="x">"#,
        "templates/index.html",
        7,
        Some(&index),
    );
    let count = result.matches("data-pana-source-id").count();
    assert_eq!(count, 2);
    assert!(result.contains("data-pana-source-id=\"sg_div\""));
    assert!(result.contains("data-pana-source-id=\"sg_span\""));
}

#[test]
fn skips_void_tags() {
    let result = inject_tpl_src_on_line("<br>", "f", 1, None);
    assert_eq!(result, "<br>");
    let result2 = inject_tpl_src_on_line("<input type=\"text\">", "f", 1, None);
    assert_eq!(result2, "<input type=\"text\">");
}

#[test]
fn handles_self_closing() {
    let index = index_with(&[("templates/index.html:8:1", "sg_img")]);
    let result = inject_tpl_src_on_line(
        r#"<img class="hero" src="/img.jpg" />"#,
        "templates/index.html",
        8,
        Some(&index),
    );
    assert!(result.contains("data-pana-source-id=\"sg_img\""));
    assert!(result.ends_with("/>"));
}

#[test]
fn injects_source_id_when_index_matches() {
    let source = r#"<section class="hero"><h1>Titlu</h1></section>"#;
    let index = SourceIdIndex::for_template_source("templates/index.html", source);

    let result = preprocess_template(source, "templates/index.html", Some(&index));

    assert!(result.contains("data-pana-source-id=\"sg_"));
    assert_eq!(result.matches("data-pana-source-id").count(), 2);
}

#[test]
fn injects_template_source_id_for_html_inside_block() {
    let source = "{% block content %}\n<section class=\"hero\"></section>\n{% endblock %}";
    let index = SourceIdIndex::for_template_source("templates/index.html", source);

    let result = preprocess_template(source, "templates/index.html", Some(&index));

    assert!(result.contains("data-pana-source-id=\"sg_"));
    assert!(result.contains("data-pana-template-source-id=\"sg_"));
}

#[test]
fn wraps_empty_block_with_template_source_markers() {
    let source = "{% block content %}{% endblock %}";
    let index = SourceIdIndex::for_template_source("templates/index.html", source);

    let result = preprocess_template(source, "templates/index.html", Some(&index));

    assert!(result.contains("{% block content %}<!-- pana-template-source-start:sg_"));
    assert!(result.contains("<!-- pana-template-source-end:sg_"));
    assert!(result.contains("-->{% endblock %}"));
    assert!(result.contains("data-pana-empty-tera-slot=\"sg_"));
}

#[test]
fn skips_non_visual_blocks_when_wrapping_template_source_markers() {
    let source = "{% block scripts %}\n<script src=\"/app.js\"></script>\n{% endblock %}";
    let index = SourceIdIndex::for_template_source("templates/index.html", source);

    let result = preprocess_template(source, "templates/index.html", Some(&index));

    assert!(!result.contains("pana-template-source-start"));
    assert!(!result.contains("pana-template-source-end"));
    assert!(result.contains("<script src=\"/app.js\"></script>"));
}

#[test]
fn wraps_multiline_block_with_template_source_markers() {
    let source = "{% block content %}\n<section></section>\n{% endblock %}";
    let index = SourceIdIndex::for_template_source("templates/index.html", source);

    let result = preprocess_template(source, "templates/index.html", Some(&index));

    assert!(result.contains("{% block content %}<!-- pana-template-source-start:sg_"));
    assert!(result.contains("<!-- pana-template-source-end:sg_"));
    assert!(result.contains("-->{% endblock %}"));
}

#[test]
fn set_assignment_extends_matching_loop_gate_to_prelude() {
    let source = "{% block content %}\n{% set items = section.pages %}\n<section>\n{% for page in items %}\n<article></article>\n{% endfor %}\n</section>\n{% endblock %}";
    let index = SourceIdIndex::for_template_source("templates/index.html", source);

    let result = preprocess_template(source, "templates/index.html", Some(&index));
    let article_line = result
        .lines()
        .find(|line| line.contains("<article"))
        .expect("article line is present");
    let wrapper_line = result
        .lines()
        .find(|line| line.contains("<section"))
        .expect("wrapper line is present");
    let for_marker_id = result
        .split("{% set items = section.pages %}<!-- pana-template-source-start:")
        .nth(1)
        .and_then(|tail| tail.split(" -->").next())
        .expect("for prelude marker id is present");
    assert!(!result.contains("{% for page in items %}<!-- pana-template-source-start:"));
    let article_template_id = article_line
        .split("data-pana-template-source-id=\"")
        .nth(1)
        .and_then(|tail| tail.split('"').next())
        .expect("article template source id is present");
    let wrapper_template_id = wrapper_line
        .split("data-pana-template-source-id=\"")
        .nth(1)
        .and_then(|tail| tail.split('"').next())
        .expect("wrapper template source id is present");

    assert_eq!(article_template_id, for_marker_id);
    assert_eq!(wrapper_template_id, for_marker_id);
}

#[test]
fn unrelated_set_does_not_capture_following_loop_html() {
    let source = "{% block content %}\n{% set hero_title = section.title %}\n<section></section>\n{% for page in section.pages %}\n<article></article>\n{% endfor %}\n{% endblock %}";
    let index = SourceIdIndex::for_template_source("templates/index.html", source);

    let result = preprocess_template(source, "templates/index.html", Some(&index));

    assert!(
        !result.contains("{% set hero_title = section.title %}<!-- pana-template-source-start:")
    );
    assert!(result.contains("{% for page in section.pages %}<!-- pana-template-source-start:"));
    let for_marker_id = result
        .split("{% for page in section.pages %}<!-- pana-template-source-start:")
        .nth(1)
        .and_then(|tail| tail.split(" -->").next())
        .expect("for marker id is present");
    let section_line = result
        .lines()
        .find(|line| line.contains("<section"))
        .expect("section line is present");
    let section_template_id = section_line
        .split("data-pana-template-source-id=\"")
        .nth(1)
        .and_then(|tail| tail.split('"').next())
        .expect("section template source id is present");

    assert_ne!(section_template_id, for_marker_id);
}

#[test]
fn set_prelude_is_consumed_by_first_matching_loop() {
    let source = "{% block content %}\n{% set items = section.pages %}\n<section>\n{% for page in items %}\n<article class=\"first\"></article>\n{% endfor %}\n</section>\n<aside>\n{% for page in items %}\n<article class=\"second\"></article>\n{% endfor %}\n</aside>\n{% endblock %}";
    let index = SourceIdIndex::for_template_source("templates/index.html", source);

    let result = preprocess_template(source, "templates/index.html", Some(&index));
    let for_lines = result
        .lines()
        .filter(|line| line.contains("{% for page in items %}"))
        .collect::<Vec<_>>();

    assert_eq!(for_lines.len(), 2);
    assert!(!for_lines[0].contains("pana-template-source-start"));
    assert!(for_lines[1].contains("pana-template-source-start"));
    assert!(result.contains("{% set items = section.pages %}<!-- pana-template-source-start:"));
}

#[test]
fn plain_for_without_set_starts_at_for_marker() {
    let source = "{% block content %}\n<section></section>\n{% for page in section.pages %}\n<article></article>\n{% endfor %}\n{% endblock %}";
    let index = SourceIdIndex::for_template_source("templates/index.html", source);

    let result = preprocess_template(source, "templates/index.html", Some(&index));

    assert!(result.contains("{% for page in section.pages %}<!-- pana-template-source-start:"));
    let for_marker_id = result
        .split("{% for page in section.pages %}<!-- pana-template-source-start:")
        .nth(1)
        .and_then(|tail| tail.split(" -->").next())
        .expect("for marker id is present");
    let section_line = result
        .lines()
        .find(|line| line.contains("<section"))
        .expect("section line is present");
    let section_template_id = section_line
        .split("data-pana-template-source-id=\"")
        .nth(1)
        .and_then(|tail| tail.split('"').next())
        .expect("section template source id is present");
    assert_ne!(section_template_id, for_marker_id);
}

#[test]
fn does_not_treat_block_inside_partial_as_visual_tera_gate() {
    let source = "{% block content %}\n<section></section>\n{% endblock %}";
    let index = SourceIdIndex::for_template_source("templates/partials/cta.html", source);

    let result = preprocess_template(source, "templates/partials/cta.html", Some(&index));

    assert!(result.contains("data-pana-source-id=\"sg_"));
    assert!(!result.contains("data-pana-template-source-id"));
    assert!(!result.contains("pana-template-source-start"));
    assert!(!result.contains("pana-template-source-end"));
}

#[test]
fn wraps_include_with_template_source_markers() {
    let index = index_with_template(&[("templates/base.html:3:1", "sg_include")]);
    let result = inject_tpl_src_on_line(
        r#"{% include "partials/header.html" %}"#,
        "templates/base.html",
        3,
        Some(&index),
    );

    assert_eq!(
        result,
        r#"<!-- pana-template-source-start:sg_include -->{% include "partials/header.html" %}<!-- pana-template-source-end:sg_include -->"#
    );
}

#[test]
fn injects_source_id_for_theme_template_paths() {
    let source = r#"<main class="theme-main"><section></section></main>"#;
    let index = SourceIdIndex::for_template_source("themes/anemone/templates/index.html", source);

    let result = preprocess_template(source, "themes/anemone/templates/index.html", Some(&index));

    assert!(result.contains("data-pana-source-id=\"sg_"));
    assert_eq!(result.matches("data-pana-source-id").count(), 2);
}
