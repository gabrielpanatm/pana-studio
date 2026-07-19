use super::model::GeneratedAssetId;

const ANIME_JS_BYTES: &[u8] = include_bytes!("../../../resources/anime.umd.min.js");

#[derive(Clone, Copy, Debug)]
pub struct GeneratedAssetDefinition {
    pub label: &'static str,
    pub zola_relative_path: &'static str,
    pub project_relative_path: &'static str,
    pub bytes: &'static [u8],
}

pub fn generated_asset_definition(id: GeneratedAssetId) -> GeneratedAssetDefinition {
    match id {
        GeneratedAssetId::AnimeJsRuntime => GeneratedAssetDefinition {
            label: id.label(),
            zola_relative_path: "static/js/anime.min.js",
            project_relative_path: "sursa/static/js/anime.min.js",
            bytes: ANIME_JS_BYTES,
        },
    }
}
