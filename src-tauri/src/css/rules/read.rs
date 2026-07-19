use crate::css::rules::{
    declarations::parse_declarations, media::locate_media_block, model::CssProperty,
    selector::locate_rule_block,
};

pub fn find_class_in_sources(
    style_files: impl IntoIterator<Item = String>,
    selector: &str,
    mut read_source: impl FnMut(&str) -> Result<Option<String>, String>,
) -> Result<Option<(String, Vec<CssProperty>)>, String> {
    for relative_path in style_files {
        let Some(source) = read_source(&relative_path)? else {
            continue;
        };
        let rules = get_class_rules(&source, selector);
        if !rules.is_empty() {
            return Ok(Some((relative_path, rules)));
        }
    }
    Ok(None)
}

pub fn get_class_rules(source: &str, selector: &str) -> Vec<CssProperty> {
    let Some((_, _, content_start, content_end, _)) = locate_rule_block(source, selector) else {
        return Vec::new();
    };

    parse_declarations(&source[content_start..content_end])
        .into_iter()
        .map(|declaration| CssProperty {
            property: declaration.property,
            value: declaration.value,
        })
        .collect()
}

pub fn get_class_rules_in_media(
    source: &str,
    media_query: &str,
    selector: &str,
) -> Vec<CssProperty> {
    let Some((block_start, block_end)) = locate_media_block(source, media_query) else {
        return Vec::new();
    };

    get_class_rules(&source[block_start..block_end], selector)
}
