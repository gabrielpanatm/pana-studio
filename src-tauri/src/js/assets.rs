const ANIME_JS_BYTES: &[u8] = include_bytes!("../../resources/anime.umd.min.js");

pub fn anime_js_bytes() -> &'static [u8] {
    ANIME_JS_BYTES
}
