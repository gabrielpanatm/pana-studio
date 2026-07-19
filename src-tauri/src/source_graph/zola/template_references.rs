use crate::source_graph::zola::{
    internal_content_path, parse_zola_path_calls, static_asset_reference, TeraZolaPathFunction,
};

#[derive(Default)]
pub(crate) struct ZolaTemplateReferences {
    pub(crate) get_pages: Vec<String>,
    pub(crate) get_sections: Vec<String>,
    pub(crate) internal_links: Vec<String>,
    pub(crate) asset_urls: Vec<String>,
    pub(crate) asset_hashes: Vec<String>,
    pub(crate) data_loads: Vec<String>,
    pub(crate) image_metadata: Vec<String>,
    pub(crate) image_resizes: Vec<String>,
}

pub(crate) fn extract_zola_template_references(source: &str) -> ZolaTemplateReferences {
    let mut references = ZolaTemplateReferences::default();
    for call in parse_zola_path_calls(source) {
        match call.function {
            TeraZolaPathFunction::GetPage => {
                push_unique(&mut references.get_pages, Some(call.path))
            }
            TeraZolaPathFunction::GetSection => {
                push_unique(&mut references.get_sections, Some(call.path))
            }
            TeraZolaPathFunction::GetUrl => {
                if let Some(content_path) = internal_content_path(&call.path) {
                    push_unique(&mut references.internal_links, Some(content_path));
                } else {
                    push_unique(
                        &mut references.asset_urls,
                        static_asset_reference(&call.path),
                    );
                }
            }
            TeraZolaPathFunction::GetHash => push_unique(
                &mut references.asset_hashes,
                static_asset_reference(&call.path),
            ),
            TeraZolaPathFunction::LoadData => push_unique(
                &mut references.data_loads,
                local_load_data_reference(&call.path),
            ),
            TeraZolaPathFunction::GetImageMetadata => push_unique(
                &mut references.image_metadata,
                static_asset_reference(&call.path),
            ),
            TeraZolaPathFunction::ResizeImage => push_unique(
                &mut references.image_resizes,
                static_asset_reference(&call.path),
            ),
        }
    }
    references
}

fn push_unique(values: &mut Vec<String>, value: Option<String>) {
    let Some(value) = value.filter(|value| !value.trim().is_empty()) else {
        return;
    };
    if !values.contains(&value) {
        values.push(value);
    }
}

fn local_load_data_reference(path: &str) -> Option<String> {
    let normalized = path.trim().replace('\\', "/");
    if normalized.is_empty()
        || normalized.starts_with("http://")
        || normalized.starts_with("https://")
        || normalized.starts_with("//")
    {
        return None;
    }
    Some(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_zola_template_references_by_domain() {
        let source = r#"{% set post = get_page(path="blog/post.md") %}
{% set blog = get_section(path="blog/_index.md") %}
<a href="{{ get_url(path="@/blog/post.md") }}">Post</a>
<link href="{{ get_url(path="css/site.css") }}">
<script integrity="{{ get_hash(path="static/js/app.js") }}"></script>
{% set data = load_data(path="static/data/catalog.json") %}
{% set page_data = load_data(path="@/blog/post.md") %}
{% set meta = get_image_metadata(path="static/img/hero.png") %}
{% set resized = resize_image(path="static/img/hero.png", width=640, op="fit_width") %}
"#;

        let references = extract_zola_template_references(source);

        assert_eq!(references.get_pages, vec!["blog/post.md"]);
        assert_eq!(references.get_sections, vec!["blog/_index.md"]);
        assert_eq!(references.internal_links, vec!["blog/post.md"]);
        assert_eq!(references.asset_urls, vec!["css/site.css"]);
        assert_eq!(references.asset_hashes, vec!["static/js/app.js"]);
        assert_eq!(
            references.data_loads,
            vec!["static/data/catalog.json", "@/blog/post.md"]
        );
        assert_eq!(references.image_metadata, vec!["static/img/hero.png"]);
        assert_eq!(references.image_resizes, vec!["static/img/hero.png"]);
    }
}
