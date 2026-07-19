use std::{
    collections::HashMap,
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use super::{write::upsert_css_rule, *};

fn temp_rules_root(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("panastudio-css-rules-test-{name}-{unique}"))
}

#[test]
fn extracts_properties_from_rule() {
    let scss = ".btn-primary {\n  background-color: $color-primary;\n  color: $text-inverse;\n}\n";
    let props = get_class_rules(scss, ".btn-primary");
    assert_eq!(props.len(), 2);
    assert_eq!(props[0].property, "background-color");
    assert_eq!(props[0].value, "$color-primary");
    assert_eq!(props[1].property, "color");
    assert_eq!(props[1].value, "$text-inverse");
}

#[test]
fn returns_empty_for_unknown_selector() {
    let scss = ".btn { color: red; }";
    let props = get_class_rules(scss, ".btn-primary");
    assert!(props.is_empty());
}

#[test]
fn preserves_clamp_values() {
    let scss = ".hero-title {\n  font-size: clamp(1.875rem, 1.35rem + 2.2vw, 3rem);\n}\n";
    let props = get_class_rules(scss, ".hero-title");
    assert_eq!(props.len(), 1);
    assert_eq!(props[0].value, "clamp(1.875rem, 1.35rem + 2.2vw, 3rem)");
}

#[test]
fn extracts_multiple_properties_from_single_line_rule() {
    let scss = ".grid { display: grid; grid-template-columns: auto auto atuo; }\n";
    let props = get_class_rules(scss, ".grid");

    assert_eq!(props.len(), 2);
    assert_eq!(props[0].property, "display");
    assert_eq!(props[0].value, "grid");
    assert_eq!(props[1].property, "grid-template-columns");
    assert_eq!(props[1].value, "auto auto atuo");
}

#[test]
fn project_style_search_finds_rule_outside_scanned_list() {
    let root = temp_rules_root("project-style-search");
    let style_path = root.join("themes/pana-studio/sass/pagini/despre.scss");
    fs::create_dir_all(style_path.parent().unwrap()).unwrap();
    fs::write(
        &style_path,
        ".ps-section-test { display: grid; backdrop-filter: blur(12px); }\n",
    )
    .unwrap();

    let result = find_class_in_sources(
        ["themes/pana-studio/sass/pagini/despre.scss".to_string()],
        ".ps-section-test",
        |relative_path| {
            std::fs::read_to_string(root.join(relative_path))
                .map(Some)
                .map_err(|error| error.to_string())
        },
    )
    .unwrap()
    .unwrap();

    assert_eq!(result.0, "themes/pana-studio/sass/pagini/despre.scss");
    assert!(result
        .1
        .iter()
        .any(|prop| prop.property == "display" && prop.value == "grid"));
    assert!(result
        .1
        .iter()
        .any(|prop| prop.property == "backdrop-filter" && prop.value == "blur(12px)"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn display_value_is_not_polluted_by_later_declarations() {
    let scss = ".layout { display: grid; gap: clamp(1rem, 2vw, 2rem); }\n";
    let props = get_class_rules(scss, ".layout");
    let display = props
        .iter()
        .find(|prop| prop.property == "display")
        .unwrap();

    assert_eq!(display.value, "grid");
}

#[test]
fn appends_new_rule_when_selector_not_found() {
    let css = "body { color: red; }\n";
    let mut props = HashMap::new();
    props.insert("font-size".to_string(), "20px".to_string());
    let result = upsert_css_rule(css, ".button", &props);
    assert!(result.contains(".button {"));
    assert!(result.contains("font-size: 20px;"));
    assert!(result.contains("body { color: red; }"));
}

#[test]
fn updates_existing_property_in_block() {
    let css = ".button {\n  color: red;\n}\n";
    let mut props = HashMap::new();
    props.insert("color".to_string(), "blue".to_string());
    let result = upsert_css_rule(css, ".button", &props);
    assert!(result.contains("color: blue;"));
    assert!(!result.contains("color: red;"));
}

#[test]
fn updates_the_existing_grouped_rule_instead_of_creating_a_duplicate_selector() {
    let css = ".card,\n.hero { color: red; }\n";
    let mut props = HashMap::new();
    props.insert("color".to_string(), "blue".to_string());
    let result = upsert_css_rule(css, ".hero", &props);

    assert_eq!(result.matches(".hero").count(), 1);
    assert!(result.contains(".card,\n.hero"));
    assert!(result.contains("color: blue;"));
}

#[test]
fn appends_new_property_to_existing_block() {
    let css = ".button {\n  color: red;\n}\n";
    let mut props = HashMap::new();
    props.insert("font-size".to_string(), "20px".to_string());
    let result = upsert_css_rule(css, ".button", &props);
    assert!(result.contains("color: red;"));
    assert!(result.contains("font-size: 20px;"));
}

#[test]
fn updates_single_line_rule_without_losing_other_declarations() {
    let css = ".grid { display: block; gap: 1rem; }\n";
    let mut props = HashMap::new();
    props.insert("display".to_string(), "grid".to_string());
    props.insert(
        "grid-template-columns".to_string(),
        "auto auto atuo".to_string(),
    );

    let result = upsert_css_rule(css, ".grid", &props);

    assert!(result.contains("display: grid;"));
    assert!(result.contains("gap: 1rem;"));
    assert!(result.contains("grid-template-columns: auto auto atuo;"));
}

#[test]
fn empty_value_removes_existing_property() {
    let css = ".button {\n  color: red;\n  line-height: 1.5;\n}\n";
    let mut props = HashMap::new();
    props.insert("line-height".to_string(), String::new());
    let result = upsert_css_rule(css, ".button", &props);

    assert!(result.contains("color: red;"));
    assert!(!result.contains("line-height: ;"));
    assert!(!result.contains("line-height: 1.5;"));
}

#[test]
fn removing_property_trims_outer_blank_lines_inside_rule() {
    let css = ".button {\n\n  color: red;\n  line-height: 1.5;\n\n}\n";
    let mut props = HashMap::new();
    props.insert("line-height".to_string(), String::new());
    let result = upsert_css_rule(css, ".button", &props);

    assert_eq!(result, ".button {\n  color: red;\n}\n");
}

#[test]
fn empty_value_removes_rule_when_no_declarations_remain() {
    let css = ".button {\n  line-height: 1.5;\n}\n\n.card {\n  color: red;\n}\n";
    let mut props = HashMap::new();
    props.insert("line-height".to_string(), "   ".to_string());
    let result = upsert_css_rule(css, ".button", &props);

    assert!(!result.contains(".button"));
    assert!(result.contains(".card {\n  color: red;\n}"));
}

#[test]
fn empty_value_does_not_create_new_empty_rule() {
    let css = ".card {\n  color: red;\n}\n";
    let mut props = HashMap::new();
    props.insert("line-height".to_string(), String::new());
    let result = upsert_css_rule_desktop(css, ".button", &props);

    assert_eq!(result, css);
}

#[test]
fn updates_existing_rule_without_gluing_to_previous_block() {
    let css = ".hero-centered {\n  text-align: center;\n}.hero-title {\n  text-align: center;\n}\n";
    let mut props = HashMap::new();
    props.insert("text-align".to_string(), "right".to_string());
    let result = upsert_css_rule(css, ".hero-title", &props);

    assert!(result.contains(".hero-centered {\n  text-align: center;\n}\n.hero-title {"));
    assert!(!result.contains("}.hero-title"));
    assert!(result.contains("text-align: right;"));
}

#[test]
fn does_not_match_partial_selector() {
    let css = ".button-large { color: red; }\n";
    let mut props = HashMap::new();
    props.insert("color".to_string(), "blue".to_string());
    let result = upsert_css_rule(css, ".button", &props);
    assert!(result.contains(".button-large { color: red; }"));
    assert!(result.contains(".button {"));
}

#[test]
fn desktop_new_rule_inserts_before_media() {
    let css = ".hero { font-size: 2rem; }\n\n@media (max-width: 1024px) {\n  .hero { font-size: 1.5rem; }\n}\n";
    let mut props = HashMap::new();
    props.insert("color".to_string(), "red".to_string());
    let result = upsert_css_rule_desktop(css, ".btn", &props);
    let media_pos = result.find("@media").unwrap();
    let btn_pos = result.find(".btn {").unwrap();
    assert!(
        btn_pos < media_pos,
        "new desktop rule must appear before @media"
    );
    assert!(result.contains(".btn {"));
    assert!(result.contains("color: red;"));
}

#[test]
fn desktop_existing_rule_updates_in_place() {
    let css = ".btn {\n  color: red;\n}\n\n@media (max-width: 1024px) {\n  .btn { font-size: 1rem; }\n}\n";
    let mut props = HashMap::new();
    props.insert("color".to_string(), "blue".to_string());
    let result = upsert_css_rule_desktop(css, ".btn", &props);
    assert!(!result.contains("color: red;"));
    assert!(result.contains("color: blue;"));
    assert!(result.contains("@media (max-width: 1024px)"));
    assert!(result.contains("font-size: 1rem;"));
}

#[test]
fn mobile_block_created_after_tablet() {
    let css =
        ".hero { color: red; }\n\n@media (max-width: 1024px) {\n  .hero { color: blue; }\n}\n";
    let mut props = HashMap::new();
    props.insert("color".to_string(), "green".to_string());
    let result = upsert_css_rule_in_media_ordered(css, "768px", 768, ".hero", &props);
    let tablet_pos = result.find("@media (max-width: 1024px)").unwrap();
    let mobile_pos = result.find("@media (max-width: 768px)").unwrap();
    assert!(
        tablet_pos < mobile_pos,
        "tablet block must appear before mobile block"
    );
    assert!(result.contains("color: green;"));
}

#[test]
fn variable_media_block_uses_numeric_order() {
    let css = ".hero { color: red; }\n\n@media (max-width: $bp-mobil) {\n  .hero { font-size: 1rem; }\n}\n";
    let mut props = HashMap::new();
    props.insert("text-align".to_string(), "right".to_string());
    let result = upsert_css_rule_in_media_ordered(css, "$bp-mobil", 768, ".hero", &props);

    assert!(result.contains("@media (max-width: $bp-mobil)"));
    assert!(!result.contains("@media (max-width: 768px)"));
    assert!(result.contains("text-align: right;"));
}

#[test]
fn media_insert_position_handles_utf8_comments() {
    let css = "// _COMPONENTE.SCSS — Componente UI\n\n.hero { color: red; }\n";
    let mut props = HashMap::new();
    props.insert("color".to_string(), "blue".to_string());

    let result = upsert_css_rule_in_media_ordered(css, "1024px", 1024, ".hero", &props);

    assert!(result.contains("@media (max-width: 1024px)"));
    assert!(result.contains("color: blue;"));
}

#[test]
fn reads_rule_from_matching_media_block() {
    let css = ".hero { color: red; }\n\n@media (max-width: 1024px) {\n  .hero {\n    color: blue;\n    font-size: 2rem;\n  }\n}\n";
    let props = get_class_rules_in_media(css, "1024px", ".hero");

    assert_eq!(props.len(), 2);
    assert!(props
        .iter()
        .any(|prop| prop.property == "color" && prop.value == "blue"));
    assert!(props
        .iter()
        .any(|prop| prop.property == "font-size" && prop.value == "2rem"));
}

#[test]
fn media_scanner_ignores_braces_and_media_text_inside_strings_or_comments() {
    let css = r#"/* @media (max-width: 768px) { .hero { color: black; } } */
.icon { content: "}"; }
@media (max-width: 768px) {
  .icon { background-image: url("data:image/svg+xml,{x}"); }
  .hero { color: green; }
}
"#;
    let props = get_class_rules_in_media(css, "768px", ".hero");

    assert_eq!(props.len(), 1);
    assert_eq!(props[0].property, "color");
    assert_eq!(props[0].value, "green");
}

#[test]
fn tablet_block_created_before_mobile() {
    let css =
        ".hero { color: red; }\n\n@media (max-width: 768px) {\n  .hero { color: green; }\n}\n";
    let mut props = HashMap::new();
    props.insert("font-size".to_string(), "1.5rem".to_string());
    let result = upsert_css_rule_in_media_ordered(css, "1024px", 1024, ".hero", &props);
    let tablet_pos = result.find("@media (max-width: 1024px)").unwrap();
    let mobile_pos = result.find("@media (max-width: 768px)").unwrap();
    assert!(
        tablet_pos < mobile_pos,
        "newly created tablet block must appear before existing mobile block"
    );
    assert!(result.contains("font-size: 1.5rem;"));
}

#[test]
fn pseudo_class_inserted_after_base() {
    let css = ".btn {\n  color: red;\n}\n\n@media (max-width: 1024px) {\n  .btn { font-size: 14px; }\n}\n";
    let mut props = HashMap::new();
    props.insert("color".to_string(), "blue".to_string());
    let result = upsert_css_rule_desktop(css, ".btn:hover", &props);

    let base_pos = result.find(".btn {").unwrap();
    let hover_pos = result.find(".btn:hover {").unwrap();
    let media_pos = result.find("@media").unwrap();

    assert!(base_pos < hover_pos, ".btn:hover must come after .btn");
    assert!(hover_pos < media_pos, ".btn:hover must come before @media");
    assert!(result.contains("color: blue;"));
    assert!(
        result.contains(".btn {\n  color: red;\n}\n\n.btn:hover {\n  color: blue;\n}\n\n@media")
    );
    assert!(!result.contains("}\n\n\n.btn:hover"));
    assert!(!result.contains("}\n\n\n@media"));
}

#[test]
fn pseudo_class_in_media_inserted_after_base() {
    let css =
        ".btn { color: red; }\n\n@media (max-width: 768px) {\n  .btn { font-size: 12px; }\n}\n";
    let mut props = HashMap::new();
    props.insert("opacity".to_string(), "0.8".to_string());
    let result = upsert_css_rule_in_media_ordered(css, "768px", 768, ".btn:hover", &props);

    let media_block = result.find("@media (max-width: 768px)").unwrap();
    let hover_in_media = result[media_block..].find(".btn:hover").unwrap();
    let btn_in_media = result[media_block..].find(".btn {").unwrap();

    assert!(
        btn_in_media < hover_in_media,
        ".btn:hover must come after .btn inside @media"
    );
    assert!(result.contains("opacity: 0.8;"));
}
