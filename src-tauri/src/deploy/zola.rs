use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(test)]
use std::fs;

use tokio_util::sync::CancellationToken;
use zola_site::Site;

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
    run_zola_build_with_after_render(project_root, zola_root, cancellation_token, || {})
}

fn run_zola_build_with_after_render(
    project_root: &Path,
    zola_root: &Path,
    cancellation_token: &CancellationToken,
    after_render: impl FnOnce(),
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
        after_render();
        cancellation_checkpoint(cancellation_token, "după randarea artifactului")?;
        Ok(())
    });

    if let Err(error) = build_result {
        cleanup_private_generation(&publication, &staging_root);
        return Err(error);
    }
    cancellation_checkpoint(cancellation_token, "înainte de publicarea artifactului").inspect_err(
        |_| {
            cleanup_private_generation(&publication, &staging_root);
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
    // Zola 0.23 owns the canonical render queue. Cancellation remains
    // cooperative at the safe publication boundaries around that queue: the
    // private generation is never visible until the post-build checkpoint.
    engine_phase(token, "build-ul canonic Zola", || site.build())
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ZolaCheckPolicy {
    Canonical,
    EditorOffline,
}

pub fn run_zola_check(project_root: &Path, zola_root: &Path) -> Result<String, String> {
    run_zola_check_with_policy(project_root, zola_root, ZolaCheckPolicy::Canonical)
}

pub(crate) fn run_zola_editor_check(
    project_root: &Path,
    zola_root: &Path,
) -> Result<String, String> {
    run_zola_check_with_policy(project_root, zola_root, ZolaCheckPolicy::EditorOffline)
}

fn run_zola_check_with_policy(
    project_root: &Path,
    zola_root: &Path,
    policy: ZolaCheckPolicy,
) -> Result<String, String> {
    // Validation and build deliberately share the exact output policy even
    // though check does not publish files.
    let artifact_root = resolve_artifact_root(project_root, zola_root)?;
    let operation = match policy {
        ZolaCheckPolicy::Canonical => "validare canonică",
        ZolaCheckPolicy::EditorOffline => "validare locală pentru editor",
    };
    with_zola_engine(operation, || {
        let config_file = zola_config_file(zola_root)?;
        let mut site = Site::new(zola_root, &config_file).map_err(|error| {
            format!(
                "Zola embedded {EMBEDDED_ZOLA_VERSION} nu a putut încărca configurația: {error:#}"
            )
        })?;
        site.config.enable_check_mode();
        if policy == ZolaCheckPolicy::EditorOffline {
            // Opening and editing are local operations. They must still reject
            // invalid templates, content and internal links, but must never
            // depend on DNS, remote servers or the user's network connection.
            site.skip_external_links_check();
        }
        site.load().map_err(|error| {
            format!("Zola embedded {EMBEDDED_ZOLA_VERSION} a respins sursele salvate: {error:#}")
        })?;
        Ok(())
    })?;

    let scope = match policy {
        ZolaCheckPolicy::Canonical => "fișierele salvate ale proiectului",
        ZolaCheckPolicy::EditorOffline => {
            "fișierele locale ale proiectului (fără accesarea linkurilor externe)"
        }
    };
    Ok(format!(
        "OK Validare Zola embedded {EMBEDDED_ZOLA_VERSION} reușită\nSursă validată: {scope}\nOutput configurat: {}",
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
    fn cancellation_after_render_preserves_the_published_artifact() {
        let root = fixture_root("cancel-after-render-preserves");
        create_minimal_site(&root, None);
        let artifact = root.join("public");
        fs::create_dir_all(&artifact).unwrap();
        fs::write(artifact.join("sentinel.txt"), "published-before-build").unwrap();
        let cancellation = CancellationToken::new();
        let cancellation_after_render = cancellation.clone();

        let error = run_zola_build_with_after_render(&root, &root, &cancellation, move || {
            cancellation_after_render.cancel()
        })
        .unwrap_err();

        assert!(error.contains("[publish_cancelled]"));
        assert!(error.contains("după randarea artifactului"));
        assert_eq!(
            fs::read_to_string(artifact.join("sentinel.txt")).unwrap(),
            "published-before-build"
        );
        assert_private_generations_removed(&root);
        cleanup(root);
    }

    #[test]
    fn failed_build_preserves_the_published_artifact() {
        let root = fixture_root("failed-build-preserves");
        create_minimal_site(&root, None);
        let artifact = root.join("public");
        fs::create_dir_all(&artifact).unwrap();
        fs::write(artifact.join("sentinel.txt"), "published-before-error").unwrap();
        fs::write(root.join("templates/index.html"), "{{ broken(").unwrap();

        let error =
            run_zola_build_cancellable(&root, &root, &CancellationToken::new()).unwrap_err();

        assert!(
            error.contains(&format!("Zola embedded {EMBEDDED_ZOLA_VERSION}")),
            "eroare neașteptată: {error}"
        );
        assert_eq!(
            fs::read_to_string(artifact.join("sentinel.txt")).unwrap(),
            "published-before-error"
        );
        assert_private_generations_removed(&root);
        cleanup(root);
    }

    #[test]
    fn embedded_build_replaces_default_output_with_sass_and_static_assets() {
        let root = fixture_root("default-output");
        create_minimal_site(&root, None);
        fs::create_dir_all(root.join(".panastudio/motion/templates")).unwrap();
        fs::write(
            root.join(".panastudio/motion/templates/index.json"),
            r#"{"schemaVersion":1}"#,
        )
        .unwrap();
        fs::write(
            root.join(".env"),
            "PANA_DEPLOY_TEST__API_TOKEN=production-secret\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("public")).unwrap();
        fs::write(root.join("public/stale.txt"), "stale").unwrap();

        let log = run_zola_build_cancellable(&root, &root, &CancellationToken::new()).unwrap();

        assert!(log.contains(&format!("Zola embedded {EMBEDDED_ZOLA_VERSION}")));
        assert!(root.join("public/index.html").is_file());
        assert!(root.join("public/site.css").is_file());
        assert!(root.join("public/asset.txt").is_file());
        assert!(!root.join("public/.panastudio").exists());
        assert!(!root.join("public/.env").exists());
        assert!(!fs::read_to_string(root.join("public/index.html"))
            .unwrap()
            .contains("production-secret"));
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
    fn embedded_build_accepts_all_zola_023_editor_options() {
        let root = fixture_root("zola-023-editor-options");
        fs::create_dir_all(root.join("content/secret")).unwrap();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::create_dir_all(root.join("static")).unwrap();
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("icons/32x32.png"),
            root.join("static/pixel.png"),
        )
        .unwrap();
        fs::write(
            root.join("zola.toml"),
            r#"base_url = "https://example.test"
generate_feeds = true
feed_filenames = ["atom.xml"]
skip_content_templating = ["literal.md"]

[markdown.highlighting]
style = "inline"
theme = "github-dark"
data_attr_position = "pre"
"#,
        )
        .unwrap();
        fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = \"Acasă\"\ntemplate = \"index.html\"\n+++\n\n```rust\nfn main() {}\n```\n",
        )
        .unwrap();
        fs::write(
            root.join("content/literal.md"),
            "+++\ntitle = \"Literal\"\ntemplate = \"page.html\"\n+++\n\n{{ valoare_inexistentă }}\n",
        )
        .unwrap();
        fs::write(
            root.join("content/publicat.md"),
            "+++\ntitle = \"Publicat în feed\"\ndate = 2026-08-27\ntemplate = \"page.html\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("content/exclus.md"),
            "+++\ntitle = \"Exclus din feed\"\ndate = 2026-08-28\ninclude_in_feeds = false\ntemplate = \"page.html\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("content/secret/_index.md"),
            "+++\ntitle = \"Secret\"\nhidden = true\ntemplate = \"section.html\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("content/secret/mostenit.md"),
            "+++\ntitle = \"Moștenit\"\ntemplate = \"page.html\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("content/secret/vizibil.md"),
            "+++\ntitle = \"Vizibil\"\nhidden = false\ntemplate = \"page.html\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("templates/index.html"),
            concat!(
                "{% set optimized = resize_image(path='pixel.png', width=1, height=1, op='fit', filter='nearest') %}",
                "<!doctype html><html><body>{{ section.content | safe }}<img src='{{ optimized.url }}'></body></html>",
            ),
        )
        .unwrap();
        fs::write(
            root.join("templates/page.html"),
            "<!doctype html><html><body>{{ page.title }}|hidden={{ page.hidden }}|{{ page.content | safe }}</body></html>",
        )
        .unwrap();
        fs::write(
            root.join("templates/section.html"),
            "<!doctype html><html><body>{{ section.title }}|hidden={{ section.hidden }}</body></html>",
        )
        .unwrap();

        run_zola_check(&root, &root).unwrap();
        run_zola_build_cancellable(&root, &root, &CancellationToken::new()).unwrap();

        let public = root.join("public");
        let index = fs::read_to_string(public.join("index.html")).unwrap();
        let pre_start = index.find("<pre").expect("bloc de cod randat");
        let code_start = index[pre_start..]
            .find("<code")
            .map(|offset| pre_start + offset)
            .expect("element code randat");
        assert!(index[pre_start..code_start].contains("data-lang=\"rust\""));
        assert!(!index[code_start..].starts_with("<code data-lang=\"rust\""));
        assert!(public.join("processed_images").is_dir());

        let literal = fs::read_to_string(public.join("literal/index.html")).unwrap();
        assert!(literal.contains("valoare_inexistentă"));
        let feed = fs::read_to_string(public.join("atom.xml")).unwrap();
        assert!(feed.contains("Publicat în feed"));
        assert!(!feed.contains("Exclus din feed"));
        assert!(fs::read_to_string(public.join("secret/index.html"))
            .unwrap()
            .contains("hidden=true"));
        assert!(
            fs::read_to_string(public.join("secret/mostenit/index.html"))
                .unwrap()
                .contains("hidden=true")
        );
        assert!(fs::read_to_string(public.join("secret/vizibil/index.html"))
            .unwrap()
            .contains("hidden=false"));
        cleanup(root);
    }

    #[test]
    fn embedded_upgrade_baseline_covers_the_zola_feature_matrix() {
        let root = fixture_root("upgrade-feature-matrix");
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/projects/zola-upgrade-baseline");
        copy_tree(&source, &root);
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("icons/32x32.png"),
            root.join("static/pixel.png"),
        )
        .unwrap();

        run_zola_check(&root, &root).unwrap();
        run_zola_build_cancellable(&root, &root, &CancellationToken::new()).unwrap();

        let public = root.join("public");
        assert!(public.join("index.html").is_file(), "pagina root lipsește");
        assert!(
            public.join("en/index.html").is_file(),
            "pagina root en lipsește"
        );
        assert!(
            public.join("articole/index.html").is_file(),
            "secțiunea lipsește"
        );
        assert!(
            public.join("articole/page/2/index.html").is_file(),
            "paginarea lipsește"
        );
        assert!(
            public.join("tags/index.html").is_file(),
            "taxonomia lipsește"
        );
        assert!(public.join("site.css").is_file(), "CSS-ul Sass lipsește");
        assert!(
            public.join("giallo.css").is_file(),
            "CSS-ul de syntax highlighting trebuie generat în output"
        );
        assert!(
            public.join("processed_images").is_dir(),
            "imaginea procesată lipsește"
        );
        assert!(
            public.join("search_index.ro.js").is_file(),
            "search ro lipsește"
        );
        assert!(
            public.join("search_index.en.js").is_file(),
            "search en lipsește"
        );
        assert!(public.join("atom.xml").is_file(), "feed-ul ro lipsește");
        assert!(public.join("en/atom.xml").is_file(), "feed-ul en lipsește");
        assert!(
            public.join("articole/primul/diagrama.svg").is_file(),
            "asset-ul colocat lipsește"
        );
        assert_eq!(
            fs::read_to_string(public.join("marker.txt")).unwrap(),
            "baseline-static-asset\n"
        );
        cleanup(root);
    }

    #[test]
    fn embedded_engine_builds_every_bundled_starter() {
        let starters_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/project-starters");
        let mut starter_ids = fs::read_dir(&starters_root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        starter_ids.sort();
        assert_eq!(
            starter_ids,
            ["cadru", "minimal", "nord", "pana-studio", "radacini"]
        );

        for starter_id in starter_ids {
            let root = fixture_root(&format!("starter-{starter_id}"));
            copy_tree(&starters_root.join(&starter_id).join("project"), &root);

            run_zola_check(&root, &root)
                .unwrap_or_else(|error| panic!("starter {starter_id}: check eșuat: {error}"));
            run_zola_build_cancellable(&root, &root, &CancellationToken::new())
                .unwrap_or_else(|error| panic!("starter {starter_id}: build eșuat: {error}"));
            assert!(
                root.join("public/index.html").is_file(),
                "starter {starter_id}"
            );
            cleanup(root);
        }
    }

    #[test]
    fn embedded_engine_builds_the_native_tera2_index_zero_fixture() {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/projects/index-zero/sursa");
        let root = fixture_root("index-zero-tera2");
        copy_tree(&source, &root);

        run_zola_check(&root, &root)
            .unwrap_or_else(|error| panic!("index-zero: check eșuat: {error}"));
        run_zola_build_cancellable(&root, &root, &CancellationToken::new())
            .unwrap_or_else(|error| panic!("index-zero: build eșuat: {error}"));
        assert!(root.join("public/index.html").is_file());

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

    #[test]
    fn editor_check_skips_external_links_but_keeps_local_validation() {
        let root = fixture_root("editor-offline-check");
        create_minimal_site(&root, None);
        fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = \"Acasă\"\n+++\n\n[Serviciu extern](http://127.0.0.1:9/offline)",
        )
        .unwrap();

        let log = run_zola_editor_check(&root, &root).unwrap();

        assert!(log.contains("fără accesarea linkurilor externe"));
        fs::write(root.join("templates/index.html"), "{{ broken(").unwrap();
        assert!(run_zola_editor_check(&root, &root).is_err());
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

    fn copy_tree(source: &Path, destination: &Path) {
        fs::create_dir_all(destination).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let source_path = entry.path();
            let destination_path = destination.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&source_path, &destination_path);
            } else {
                fs::copy(&source_path, &destination_path).unwrap();
            }
        }
    }

    fn assert_private_generations_removed(root: &Path) {
        let leftovers = fs::read_dir(root)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(".pana-studio-build-staging-"))
            .collect::<Vec<_>>();
        assert!(
            leftovers.is_empty(),
            "generații private rămase: {leftovers:?}"
        );
    }
}
