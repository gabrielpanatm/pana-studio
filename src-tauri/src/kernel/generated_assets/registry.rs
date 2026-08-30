use std::collections::BTreeSet;

const ANIME_JS_BYTES: &[u8] = include_bytes!("../../../resources/anime.umd.min.js");

#[derive(Clone, Copy, Debug)]
pub struct EmbeddedAnimeModule {
    pub relative_path: &'static str,
    pub source: &'static str,
    pub dependencies: &'static [&'static str],
}

include!(concat!(env!("OUT_DIR"), "/pana-studio-anime-modules.rs"));

pub fn anime_esm_public_root() -> String {
    format!("js/vendor/animejs-{EMBEDDED_ANIME_VERSION}")
}

pub fn anime_esm_project_root() -> String {
    format!("static/{}", anime_esm_public_root())
}

pub fn anime_esm_public_module_path(module_path: &str) -> String {
    format!("/{}/{module_path}", anime_esm_public_root())
}

pub fn anime_esm_project_license_path() -> String {
    format!("{}/LICENSE.md", anime_esm_project_root())
}

#[derive(Clone, Debug)]
pub struct AnimeModuleAssetDefinition {
    pub module_path: &'static str,
    pub project_path: String,
    pub source: &'static str,
}

pub fn anime_module(module_path: &str) -> Option<&'static EmbeddedAnimeModule> {
    EMBEDDED_ANIME_MODULES
        .binary_search_by_key(&module_path, |module| module.relative_path)
        .ok()
        .map(|index| &EMBEDDED_ANIME_MODULES[index])
}

pub fn anime_module_dependency_closure(
    entry_modules: impl IntoIterator<Item = &'static str>,
) -> Result<BTreeSet<&'static str>, String> {
    let mut required = BTreeSet::new();
    let mut pending = entry_modules.into_iter().collect::<Vec<_>>();
    while let Some(module_path) = pending.pop() {
        if !required.insert(module_path) {
            continue;
        }
        let module = anime_module(module_path).ok_or_else(|| {
            format!("Catalogul Anime.js nu conține modulul necesar {module_path}.")
        })?;
        pending.extend(module.dependencies.iter().copied());
    }
    Ok(required)
}

pub fn anime_modules_referenced_by_source(source: &str) -> BTreeSet<&'static str> {
    let prefix = format!("/{}/", anime_esm_public_root());
    let mut modules = BTreeSet::new();
    for (start, _) in source.match_indices(&prefix) {
        let Some(quote) = source[..start].chars().next_back() else {
            continue;
        };
        if !matches!(quote, '\'' | '"' | '`') {
            continue;
        }
        let path_start = start + prefix.len();
        let mut escaped = false;
        let mut path_end = None;
        for (offset, character) in source[path_start..].char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            if character == '\\' {
                escaped = true;
                continue;
            }
            if character == quote {
                path_end = Some(path_start + offset);
                break;
            }
        }
        let Some(path_end) = path_end else {
            continue;
        };
        let module_path = source[path_start..path_end]
            .split(['?', '#'])
            .next()
            .unwrap_or_default();
        if let Some(module) = anime_module(module_path) {
            modules.insert(module.relative_path);
        }
    }
    modules
}

pub fn anime_module_assets() -> impl Iterator<Item = AnimeModuleAssetDefinition> {
    let project_root = anime_esm_project_root();
    EMBEDDED_ANIME_MODULES
        .iter()
        .map(move |module| AnimeModuleAssetDefinition {
            module_path: module.relative_path,
            project_path: format!("{project_root}/{}", module.relative_path),
            source: module.source,
        })
}

#[derive(Clone, Copy, Debug)]
pub struct AnimeJsRuntimeContract {
    pub version: &'static str,
    pub bytes: &'static [u8],
}

pub const ANIME_JS_RUNTIME_CONTRACT: AnimeJsRuntimeContract = AnimeJsRuntimeContract {
    version: EMBEDDED_ANIME_VERSION,
    bytes: ANIME_JS_BYTES,
};

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::zola_engine::{EMBEDDED_ZOLA_REVISION, EMBEDDED_ZOLA_VERSION};

    use super::{
        anime_module, anime_module_dependency_closure, anime_modules_referenced_by_source,
        ANIME_JS_RUNTIME_CONTRACT, EMBEDDED_ANIME_MODULES,
    };

    #[test]
    fn vendored_anime_runtime_matches_the_declared_contract() {
        let source = std::str::from_utf8(ANIME_JS_RUNTIME_CONTRACT.bytes)
            .expect("Anime.js runtime must be UTF-8");
        assert!(source.contains(&format!("@version v{}", ANIME_JS_RUNTIME_CONTRACT.version)));
        assert!(source.contains(&format!(
            "version:\"{}\"",
            ANIME_JS_RUNTIME_CONTRACT.version
        )));
    }

    #[test]
    fn third_party_notice_matches_the_declared_contract() {
        let notice = include_str!("../../../../THIRD_PARTY_NOTICES.md");
        assert!(notice.contains(&format!("## Zola {EMBEDDED_ZOLA_VERSION}")));
        assert!(notice.contains(&format!("- revizie sursă: `{EMBEDDED_ZOLA_REVISION}`;")));
        assert!(notice.contains(&format!(
            "## Anime.js {}",
            ANIME_JS_RUNTIME_CONTRACT.version
        )));
        assert!(notice.contains(&format!(
            "- versiune: `{}`;",
            ANIME_JS_RUNTIME_CONTRACT.version
        )));
    }

    #[test]
    fn simple_timeline_closure_is_bounded_and_excludes_the_full_umd_runtime() {
        let modules = anime_module_dependency_closure(["timeline/index.js"]).unwrap();
        let bytes = modules
            .iter()
            .map(|path| anime_module(path).unwrap().source.len())
            .sum::<usize>();

        assert_eq!(modules.len(), 21);
        assert!(bytes < 45 * 1024, "closure Anime.js simplă: {bytes} bytes");
        assert!(bytes < ANIME_JS_RUNTIME_CONTRACT.bytes.len() / 2);
        assert!(!modules.contains("index.js"));
    }

    #[test]
    fn every_embedded_module_dependency_resolves_inside_the_catalog() {
        for module in EMBEDDED_ANIME_MODULES {
            for dependency in module.dependencies {
                assert!(
                    anime_module(dependency).is_some(),
                    "{} imports missing module {dependency}",
                    module.relative_path
                );
            }
        }
    }

    #[test]
    fn authored_page_source_retains_only_literal_managed_anime_imports() {
        let source = format!(
            "import {{ createTimeline }} from '/{}/timeline/index.js';\n\
             void import(\"/{}/utils/stagger.js?v=1\");\n\
             const unrelated='/js/vendor/animejs-0.0.0/index.js';",
            super::anime_esm_public_root(),
            super::anime_esm_public_root(),
        );
        assert_eq!(
            anime_modules_referenced_by_source(&source),
            BTreeSet::from(["timeline/index.js", "utils/stagger.js"])
        );
    }
}
