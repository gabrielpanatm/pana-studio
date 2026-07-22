use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(test)]
use std::fs;

use tokio_util::sync::CancellationToken;
use zola_site::{sass, Site};

use crate::kernel::write_authority::ZolaArtifactPublicationLease;
use crate::zola_engine::{
    with_zola_engine, zola_config_file, EMBEDDED_ZOLA_REVISION, EMBEDDED_ZOLA_VERSION,
};

use super::artifact::resolve_artifact_root;

static BUILD_GENERATION: AtomicU64 = AtomicU64::new(1);

pub fn run_zola_build_cancellable(
    project_root: &Path,
    zola_root: &Path,
    cancellation_token: &CancellationToken,
) -> Result<String, String> {
    let artifact_root = resolve_artifact_root(project_root, zola_root)?;
    cancellation_checkpoint(cancellation_token, "înainte de pregătirea build-ului")?;

    let staging_root = unique_sibling(&artifact_root, "build-staging")?;
    let publication = ZolaArtifactPublicationLease::capture(&artifact_root)?;
    let confirmed_artifact_root = match resolve_artifact_root(project_root, zola_root) {
        Ok(root) => root,
        Err(error) => {
            cleanup_private_generation(&publication, &staging_root);
            return Err(format!(
                "Politica output_dir a respins pregătirea build-ului: {error}"
            ));
        }
    };
    if confirmed_artifact_root != artifact_root {
        cleanup_private_generation(&publication, &staging_root);
        return Err(
            "output_dir s-a schimbat în timpul pregătirii build-ului; operația a fost blocată."
                .to_string(),
        );
    }
    publication.verify_path_binding()?;

    let build_result = with_zola_engine("build de producție", || {
        cancellation_checkpoint(cancellation_token, "înainte de încărcarea proiectului")?;
        let config_file = zola_config_file(zola_root)?;
        let mut site = Site::new(zola_root, &config_file).map_err(|error| {
            format!(
                "Zola embedded {EMBEDDED_ZOLA_VERSION} nu a putut încărca configurația: {error:#}"
            )
        })?;
        // Site::new defaults to BuildMode::Disk. Only the destination is
        // redirected to a private sibling generation until publication.
        site.set_output_path(&staging_root);
        site.load().map_err(|error| {
            format!(
                "Zola embedded {EMBEDDED_ZOLA_VERSION} nu a putut valida și încărca sursele: {error:#}"
            )
        })?;
        cancellation_checkpoint(cancellation_token, "după încărcarea proiectului")?;
        build_site_cooperatively(&site, cancellation_token)?;
        cancellation_checkpoint(cancellation_token, "după randarea artifactului")?;
        Ok(())
    });

    if let Err(error) = build_result {
        cleanup_private_generation(&publication, &staging_root);
        return Err(error);
    }
    cancellation_checkpoint(cancellation_token, "înainte de publicarea artifactului").map_err(
        |error| {
            cleanup_private_generation(&publication, &staging_root);
            error
        },
    )?;

    let confirmed_artifact_root = match resolve_artifact_root(project_root, zola_root) {
        Ok(root) => root,
        Err(error) => {
            cleanup_private_generation(&publication, &staging_root);
            return Err(format!(
                "Politica output_dir a respins publicarea după build: {error}"
            ));
        }
    };
    if confirmed_artifact_root != artifact_root {
        cleanup_private_generation(&publication, &staging_root);
        return Err(
            "output_dir s-a schimbat în timpul build-ului; generația staged nu a fost publicată."
                .to_string(),
        );
    }

    publication.verify_path_binding()?;
    let cleanup_warning = publication
        .publish_private_generation(&staging_root)
        .map_err(|error| error.into_terminal_diagnostic())?;
    let mut log = format!(
        "OK Build Zola embedded {EMBEDDED_ZOLA_VERSION} reușit\nRevizie motor: {EMBEDDED_ZOLA_REVISION}\nArtifact publicat atomic: {}",
        artifact_root.display()
    );
    if let Some(warning) = cleanup_warning {
        log.push_str("\nAvertisment: ");
        log.push_str(&warning);
    }
    Ok(log)
}

fn build_site_cooperatively(site: &Site, token: &CancellationToken) -> Result<(), String> {
    engine_phase(token, "curățarea generației staged", || site.clean())?;
    if let Some(theme) = &site.config.theme {
        let theme_path = site.base_path.join("themes").join(theme);
        if theme_path.join("sass").exists() {
            engine_phase(token, "compilarea Sass a temei", || {
                sass::compile_sass(&theme_path, &site.output_path)
            })?;
        }
    }
    if site.config.compile_sass {
        engine_phase(token, "compilarea Sass a proiectului", || {
            sass::compile_sass(&site.base_path, &site.output_path)
        })?;
    }
    if site.config.build_search_index {
        engine_phase(token, "construirea indexului de căutare", || {
            site.build_search_index()
        })?;
    }
    engine_phase(token, "randarea aliasurilor", || site.render_aliases())?;
    engine_phase(token, "randarea secțiunilor", || site.render_sections())?;
    engine_phase(token, "randarea paginilor independente", || {
        site.render_orphan_pages()
    })?;
    if site.config.generate_sitemap {
        engine_phase(token, "randarea sitemap-ului", || site.render_sitemap())?;
    }

    if site.config.generate_feeds {
        cancellation_checkpoint(token, "înainte de feed-ul limbii implicite")?;
        let library = site
            .library
            .read()
            .map_err(|_| "Biblioteca Zola este indisponibilă pentru feed.".to_string())?;
        let pages = if site.config.is_multilingual() {
            library
                .pages
                .values()
                .filter(|page| page.lang == site.config.default_language)
                .collect()
        } else {
            library.pages.values().collect()
        };
        site.render_feeds(pages, None, &site.config.default_language, |context| {
            context
        })
        .map_err(|error| embedded_phase_error("randarea feed-ului implicit", error))?;
    }
    for (code, language) in site.config.other_languages() {
        if !language.generate_feeds {
            continue;
        }
        cancellation_checkpoint(token, &format!("înainte de feed-ul limbii {code}"))?;
        let library = site
            .library
            .read()
            .map_err(|_| "Biblioteca Zola este indisponibilă pentru feed.".to_string())?;
        let pages = library
            .pages
            .values()
            .filter(|page| page.lang == *code)
            .collect();
        site.render_feeds(pages, Some(&PathBuf::from(code)), code, |context| context)
            .map_err(|error| embedded_phase_error(&format!("randarea feed-ului {code}"), error))?;
    }

    engine_phase(token, "randarea CSS-ului temelor de cod", || {
        site.render_themes_css()
    })?;
    engine_phase(token, "randarea paginii 404", || site.render_404())?;
    if site.config.generate_robots_txt {
        engine_phase(token, "randarea robots.txt", || site.render_robots())?;
    }
    engine_phase(token, "randarea taxonomiilor", || site.render_taxonomies())?;
    engine_phase(token, "procesarea imaginilor Zola", || {
        site.process_images()
    })?;
    engine_phase(token, "copierea resurselor statice", || {
        site.copy_static_directories()
    })?;
    cancellation_checkpoint(token, "după ultima fază de build")
}

fn engine_phase<E: std::fmt::Display>(
    token: &CancellationToken,
    phase: &str,
    execute: impl FnOnce() -> Result<(), E>,
) -> Result<(), String> {
    cancellation_checkpoint(token, &format!("înainte de {phase}"))?;
    execute().map_err(|error| embedded_phase_error(phase, error))?;
    cancellation_checkpoint(token, &format!("după {phase}"))
}

fn embedded_phase_error(phase: &str, error: impl std::fmt::Display) -> String {
    format!("Zola embedded {EMBEDDED_ZOLA_VERSION} a eșuat în faza «{phase}»: {error:#}")
}

pub fn run_zola_check(project_root: &Path, zola_root: &Path) -> Result<String, String> {
    // Validation and build deliberately share the exact output policy even
    // though check does not publish files.
    let artifact_root = resolve_artifact_root(project_root, zola_root)?;
    with_zola_engine("validare canonică", || {
        let config_file = zola_config_file(zola_root)?;
        let mut site = Site::new(zola_root, &config_file).map_err(|error| {
            format!(
                "Zola embedded {EMBEDDED_ZOLA_VERSION} nu a putut încărca configurația: {error:#}"
            )
        })?;
        site.config.enable_check_mode();
        site.load().map_err(|error| {
            format!("Zola embedded {EMBEDDED_ZOLA_VERSION} a respins sursele salvate: {error:#}")
        })?;
        Ok(())
    })?;

    Ok(format!(
        "OK Validare Zola embedded {EMBEDDED_ZOLA_VERSION} reușită\nSursă validată: fișierele salvate ale proiectului\nOutput configurat: {}",
        artifact_root.display()
    ))
}

fn cancellation_checkpoint(token: &CancellationToken, phase: &str) -> Result<(), String> {
    if token.is_cancelled() {
        return Err(format!(
            "[publish_cancelled] Build-ul Zola embedded a fost anulat {phase}; niciun artifact nou nu a fost publicat."
        ));
    }
    Ok(())
}

fn unique_sibling(artifact_root: &Path, kind: &str) -> Result<PathBuf, String> {
    let parent = artifact_root.parent().ok_or_else(|| {
        format!(
            "Artifactul {} nu are un director părinte sigur.",
            artifact_root.display()
        )
    })?;
    let generation = BUILD_GENERATION.fetch_add(1, Ordering::Relaxed);
    Ok(parent.join(format!(
        ".pana-studio-{kind}-{}-{generation}",
        std::process::id()
    )))
}

fn cleanup_private_generation(publication: &ZolaArtifactPublicationLease, path: &Path) {
    if let Err(error) = publication.discard_private_generation(path) {
        eprintln!(
            "[Pană Studio] Cleanup-ul generației private {} a eșuat: {error}",
            path.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn cancelled_build_preserves_the_published_artifact() {
        let root = fixture_root("cancel-preserves");
        create_minimal_site(&root, None);
        let artifact = root.join("public");
        fs::create_dir_all(&artifact).unwrap();
        fs::write(artifact.join("sentinel.txt"), "published").unwrap();
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        let error = run_zola_build_cancellable(&root, &root, &cancellation).unwrap_err();

        assert!(error.contains("[publish_cancelled]"));
        assert_eq!(
            fs::read_to_string(artifact.join("sentinel.txt")).unwrap(),
            "published"
        );
        cleanup(root);
    }

    #[test]
    fn embedded_build_replaces_default_output_with_sass_and_static_assets() {
        let root = fixture_root("default-output");
        create_minimal_site(&root, None);
        fs::create_dir_all(root.join("public")).unwrap();
        fs::write(root.join("public/stale.txt"), "stale").unwrap();

        let log = run_zola_build_cancellable(&root, &root, &CancellationToken::new()).unwrap();

        assert!(log.contains("Zola embedded 0.22.1"));
        assert!(root.join("public/index.html").is_file());
        assert!(root.join("public/site.css").is_file());
        assert!(root.join("public/asset.txt").is_file());
        assert!(!root.join("public/stale.txt").exists());
        cleanup(root);
    }

    #[test]
    fn embedded_build_publishes_to_configured_parent_output() {
        let fixture = fixture_root("parent-output");
        let project = fixture.join("project");
        fs::create_dir_all(&project).unwrap();
        create_minimal_site(&project, Some("../export"));

        run_zola_build_cancellable(&project, &project, &CancellationToken::new()).unwrap();

        assert!(fixture.join("export/index.html").is_file());
        assert!(!project.join("public").exists());
        cleanup(fixture);
    }

    #[test]
    fn embedded_build_publishes_to_configured_absolute_output() {
        let fixture = fixture_root("absolute-output");
        let project = fixture.join("project");
        let artifact = fixture.join("absolute-artifact");
        fs::create_dir_all(&project).unwrap();
        create_minimal_site(&project, Some(artifact.to_str().unwrap()));

        run_zola_build_cancellable(&project, &project, &CancellationToken::new()).unwrap();

        assert!(artifact.join("index.html").is_file());
        assert!(!project.join("public").exists());
        cleanup(fixture);
    }

    #[test]
    fn embedded_build_processes_images_requested_by_zola_templates() {
        let root = fixture_root("image-processing");
        create_minimal_site(&root, None);
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("icons/32x32.png"),
            root.join("static/pixel.png"),
        )
        .unwrap();
        fs::write(
            root.join("templates/index.html"),
            concat!(
                "{% set optimized = resize_image(path='pixel.png', width=1, height=1, op='fit') %}",
                "<!doctype html><html><body><img src='{{ optimized.url }}'></body></html>",
            ),
        )
        .unwrap();

        run_zola_build_cancellable(&root, &root, &CancellationToken::new()).unwrap();

        let html = fs::read_to_string(root.join("public/index.html")).unwrap();
        assert!(html.contains("processed_images"));
        assert!(root.join("public/processed_images").is_dir());
        assert!(fs::read_dir(root.join("public/processed_images"))
            .unwrap()
            .next()
            .is_some());
        cleanup(root);
    }

    #[test]
    fn cooperative_phase_observes_cancellation_after_the_current_engine_step() {
        let cancellation = CancellationToken::new();
        let cancellation_inside_step = cancellation.clone();
        let error = engine_phase(&cancellation, "test", move || {
            cancellation_inside_step.cancel();
            Ok::<(), String>(())
        })
        .unwrap_err();
        assert!(error.contains("[publish_cancelled]"));
        assert!(error.contains("după test"));
    }

    #[test]
    fn embedded_check_validates_saved_sources() {
        let root = fixture_root("check");
        create_minimal_site(&root, None);
        assert!(run_zola_check(&root, &root)
            .unwrap()
            .contains("fișierele salvate"));
        fs::write(root.join("templates/index.html"), "{{ broken(").unwrap();
        assert!(run_zola_check(&root, &root).is_err());
        cleanup(root);
    }

    fn create_minimal_site(root: &Path, output_dir: Option<&str>) {
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::create_dir_all(root.join("sass")).unwrap();
        fs::create_dir_all(root.join("static")).unwrap();
        let output = output_dir
            .map(|value| format!("output_dir = {value:?}\n"))
            .unwrap_or_default();
        fs::write(
            root.join("zola.toml"),
            format!("base_url = \"https://example.test\"\ncompile_sass = true\n{output}"),
        )
        .unwrap();
        fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = \"Acasă\"\n+++",
        )
        .unwrap();
        fs::write(
            root.join("templates/index.html"),
            "<!doctype html><html><body>{{ section.title }}</body></html>",
        )
        .unwrap();
        fs::write(
            root.join("sass/site.scss"),
            "$accent: #123456; body { color: $accent; }",
        )
        .unwrap();
        fs::write(root.join("static/asset.txt"), "asset").unwrap();
    }

    fn fixture_root(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "pana-embedded-zola-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn cleanup(path: PathBuf) {
        let _ = fs::remove_dir_all(path);
    }
}
