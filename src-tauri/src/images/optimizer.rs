use std::{
    fs,
    panic::{catch_unwind, AssertUnwindSafe},
    path::{Path, PathBuf},
};

use image::{codecs::webp::WebPEncoder, imageops::FilterType, GenericImageView};
use tauri::{AppHandle, Runtime};

use crate::kernel::write_authority::{
    WriteAuthority, WriteAuthorityError, WriteCategory, WriteIntent, WriteOperationKind,
    WriteOwner, WritePolicy, WriteTarget,
};

use super::rewrite::rewrite_asset_references;
const MAX_IMAGE_DECODE_PIXELS: u64 = 80_000_000;

#[derive(Clone, Debug)]
pub struct ImageOptimizationOptions {
    pub max_dimension: u32,
    pub exclude_suffix: String,
    pub replace_only_if_smaller: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ImageOptimizationReport {
    pub scanned: usize,
    pub optimized: usize,
    pub skipped_excluded: usize,
    pub skipped_not_smaller: usize,
    pub failed: usize,
    pub bytes_before: u64,
    pub bytes_after: u64,
    pub replacements: Vec<(String, String)>,
    pub log: String,
}

impl ImageOptimizationReport {
    pub fn summary(&self) -> String {
        let saved = self.bytes_before.saturating_sub(self.bytes_after);
        format!(
            "Optimizare imagini: {} scanate, {} optimizate, {} excluse, {} păstrate mai mici, {} erori, {} salvați",
            self.scanned,
            self.optimized,
            self.skipped_excluded,
            self.skipped_not_smaller,
            self.failed,
            human_bytes(saved)
        )
    }
}

pub fn optimize_output_images<R: Runtime>(
    app: &AppHandle<R>,
    output_dir: &Path,
    options: &ImageOptimizationOptions,
    expected_runtime_session_id: &str,
) -> Result<ImageOptimizationReport, WriteAuthorityError> {
    if !output_dir.exists() {
        return Err(format!("Folderul output nu există: {}", output_dir.display()).into());
    }

    let mut report = ImageOptimizationReport::default();
    let image_paths = collect_image_paths(output_dir);

    for path in image_paths {
        report.scanned += 1;

        if should_exclude(&path, &options.exclude_suffix) {
            report.skipped_excluded += 1;
            continue;
        }

        match catch_unwind(AssertUnwindSafe(|| {
            optimize_one_image(app, output_dir, &path, options, expected_runtime_session_id)
        })) {
            Ok(Ok(Some(result))) => {
                report.optimized += 1;
                report.bytes_before += result.bytes_before;
                report.bytes_after += result.bytes_after;
                if result.old_rel != result.new_rel {
                    report.replacements.push((result.old_rel, result.new_rel));
                }
            }
            Ok(Ok(None)) => {
                report.skipped_not_smaller += 1;
            }
            Ok(Err(error @ WriteAuthorityError::RecoveryRequired(_))) => {
                // Efectul poate fi deja vizibil, iar WAL-ul interzice retry-ul.
                // Nu transforma recovery-ul într-o eroare tolerată per imagine:
                // callerul trebuie să oprească inclusiv deploy-ul outputului parțial.
                return Err(error);
            }
            Ok(Err(error @ WriteAuthorityError::Rejected(_))) => {
                report.failed += 1;
                report
                    .log
                    .push_str(&format!("eroare {}: {}\n", path.display(), error));
            }
            Err(_) => {
                report.failed += 1;
                report.log.push_str(&format!(
                    "eroare {}: optimizerul a întrerupt procesarea imaginii\n",
                    path.display()
                ));
            }
        }
    }

    if !report.replacements.is_empty() {
        rewrite_output_references(
            app,
            output_dir,
            &report.replacements,
            expected_runtime_session_id,
        )?;
    }

    Ok(report)
}

struct OptimizedImage {
    old_rel: String,
    new_rel: String,
    bytes_before: u64,
    bytes_after: u64,
}

fn optimize_one_image<R: Runtime>(
    app: &AppHandle<R>,
    output_dir: &Path,
    path: &Path,
    options: &ImageOptimizationOptions,
    expected_runtime_session_id: &str,
) -> Result<Option<OptimizedImage>, WriteAuthorityError> {
    ensure_decode_budget(path)?;
    let before = fs::metadata(path)
        .map_err(|e| format!("Nu am putut citi metadata: {}", e))?
        .len();
    let image = image::ImageReader::open(path)
        .map_err(|e| format!("Nu am putut deschide imaginea: {}", e))?
        .with_guessed_format()
        .map_err(|e| format!("Nu am putut detecta formatul: {}", e))?
        .decode()
        .map_err(|e| format!("Nu am putut decoda imaginea: {}", e))?;

    let resized = resize_if_needed(image, options.max_dimension.max(1));
    let rgba = resized.to_rgba8();
    let (width, height) = rgba.dimensions();
    let encoded = encode_lossless_webp(&rgba, width, height)?;

    if options.replace_only_if_smaller && encoded.len() as u64 >= before {
        return Ok(None);
    }

    let target = target_webp_path(path);
    let after = encoded.len() as u64;
    if options.replace_only_if_smaller && after >= before {
        return Ok(None);
    }

    if target != path && target.exists() {
        remove_output_file_if_exists(
            app,
            output_dir,
            &target,
            "Image Optimizer elimină WebP-ul output existent înainte de înlocuire.",
            expected_runtime_session_id,
        )?;
    }
    write_output_bytes(
        app,
        output_dir,
        &target,
        &encoded,
        expected_runtime_session_id,
    )?;
    if target != path {
        remove_output_file_if_exists(
            app,
            output_dir,
            path,
            "Image Optimizer elimină originalul output după conversia WebP.",
            expected_runtime_session_id,
        )?;
    }

    Ok(Some(OptimizedImage {
        old_rel: relative_output_path(output_dir, path)?,
        new_rel: relative_output_path(output_dir, &target)?,
        bytes_before: before,
        bytes_after: after,
    }))
}

fn ensure_decode_budget(path: &Path) -> Result<(), String> {
    let (width, height) = image::image_dimensions(path)
        .map_err(|e| format!("Nu am putut citi dimensiunile imaginii: {}", e))?;
    let pixels = image_pixels(width, height);
    if pixels > MAX_IMAGE_DECODE_PIXELS {
        return Err(format!(
            "Imagine prea mare pentru optimizer: {}x{} (limita este {} MP).",
            width,
            height,
            MAX_IMAGE_DECODE_PIXELS / 1_000_000
        ));
    }
    Ok(())
}

fn image_pixels(width: u32, height: u32) -> u64 {
    u64::from(width) * u64::from(height)
}

fn resize_if_needed(image: image::DynamicImage, max_dimension: u32) -> image::DynamicImage {
    let (width, height) = image.dimensions();
    let largest = width.max(height);
    if largest <= max_dimension {
        return image;
    }

    let ratio = max_dimension as f64 / largest as f64;
    let next_width = ((width as f64 * ratio).round() as u32).max(1);
    let next_height = ((height as f64 * ratio).round() as u32).max(1);
    image.resize_exact(next_width, next_height, FilterType::Lanczos3)
}

fn encode_lossless_webp(
    rgba: &image::RgbaImage,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    let mut encoded = Vec::new();
    WebPEncoder::new_lossless(&mut encoded)
        .encode(rgba.as_raw(), width, height, image::ColorType::Rgba8.into())
        .map_err(|e| format!("Nu am putut encoda WebP lossless: {}", e))?;
    Ok(encoded)
}

fn collect_image_paths(output_dir: &Path) -> Vec<PathBuf> {
    walkdir::WalkDir::new(output_dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| is_supported_image(path))
        .collect()
}

fn is_supported_image(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref(),
        Some("jpg" | "jpeg" | "png" | "webp")
    )
}

fn should_exclude(path: &Path, suffix: &str) -> bool {
    let suffix = suffix.trim();
    if suffix.is_empty() {
        return false;
    }
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.ends_with(suffix))
}

fn target_webp_path(path: &Path) -> PathBuf {
    path.with_extension("webp")
}

fn relative_output_path(output_dir: &Path, path: &Path) -> Result<String, String> {
    path.strip_prefix(output_dir)
        .map_err(|e| e.to_string())
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
}

fn rewrite_output_references<R: Runtime>(
    app: &AppHandle<R>,
    output_dir: &Path,
    replacements: &[(String, String)],
    expected_runtime_session_id: &str,
) -> Result<(), WriteAuthorityError> {
    for entry in walkdir::WalkDir::new(output_dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path();
        if !is_rewritable_text_file(path) {
            continue;
        }
        let source = fs::read_to_string(path)
            .map_err(|e| format!("Nu am putut citi fișierul output {}: {}", path.display(), e))?;
        let updated = rewrite_asset_references(&source, path, output_dir, replacements);
        if updated != source {
            write_output_text(app, output_dir, path, &updated, expected_runtime_session_id)?;
        }
    }
    Ok(())
}

fn write_output_bytes<R: Runtime>(
    app: &AppHandle<R>,
    output_dir: &Path,
    target: &Path,
    bytes: &[u8],
    expected_runtime_session_id: &str,
) -> Result<(), WriteAuthorityError> {
    let label = relative_output_path(output_dir, target)?;
    let intent = WriteIntent::new(
        WriteCategory::BuildOutputWrite,
        WriteOwner::ImageOptimizer,
        WriteOperationKind::WriteBytes,
        WriteTarget::new(
            target.to_path_buf(),
            output_dir.to_path_buf(),
            label.clone(),
        )
        .with_expected_runtime_session_id(expected_runtime_session_id),
        WritePolicy::build_output_atomic(),
        format!("Image Optimizer scrie WebP output {}.", label),
    );
    WriteAuthority::new(app).write_bytes(intent, bytes)?;
    Ok(())
}

fn write_output_text<R: Runtime>(
    app: &AppHandle<R>,
    output_dir: &Path,
    target: &Path,
    text: &str,
    expected_runtime_session_id: &str,
) -> Result<(), WriteAuthorityError> {
    let label = relative_output_path(output_dir, target)?;
    let intent = WriteIntent::new(
        WriteCategory::BuildOutputWrite,
        WriteOwner::ImageOptimizer,
        WriteOperationKind::WriteText,
        WriteTarget::new(
            target.to_path_buf(),
            output_dir.to_path_buf(),
            label.clone(),
        )
        .with_expected_runtime_session_id(expected_runtime_session_id),
        WritePolicy::build_output_atomic(),
        format!("Image Optimizer rescrie referințe output în {}.", label),
    );
    WriteAuthority::new(app).write_text(intent, text)?;
    Ok(())
}

fn remove_output_file_if_exists<R: Runtime>(
    app: &AppHandle<R>,
    output_dir: &Path,
    target: &Path,
    description: &str,
    expected_runtime_session_id: &str,
) -> Result<(), WriteAuthorityError> {
    let label = relative_output_path(output_dir, target)?;
    let intent = WriteIntent::new(
        WriteCategory::BuildOutputWrite,
        WriteOwner::ImageOptimizer,
        WriteOperationKind::RemoveFile,
        WriteTarget::new(target.to_path_buf(), output_dir.to_path_buf(), label)
            .with_expected_runtime_session_id(expected_runtime_session_id),
        WritePolicy::build_output_lifecycle(),
        description,
    );
    WriteAuthority::new(app).remove_file_if_exists(intent)?;
    Ok(())
}

fn is_rewritable_text_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref(),
        Some("html" | "css" | "js" | "json" | "xml" | "txt")
    )
}

fn human_bytes(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.2} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "linux")]
    use crate::kernel::write_authority::test_support::with_before_remove_leaf_target_durable_hook_for_test;
    use crate::{
        app_home::{ensure_app_home, TEST_APP_ENV_LOCK},
        kernel::write_authority::test_support::install_test_project_authority,
    };
    use std::{
        env,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn excludes_images_with_configured_suffix() {
        assert!(should_exclude(Path::new("hero-nr.jpg"), "-nr"));
        assert!(!should_exclude(Path::new("hero.jpg"), "-nr"));
        assert!(!should_exclude(Path::new("hero.jpg"), ""));
    }

    #[test]
    fn recognizes_supported_static_raster_formats_only() {
        assert!(is_supported_image(Path::new("a.jpg")));
        assert!(is_supported_image(Path::new("a.jpeg")));
        assert!(is_supported_image(Path::new("a.png")));
        assert!(is_supported_image(Path::new("a.webp")));
        assert!(!is_supported_image(Path::new("a.gif")));
        assert!(!is_supported_image(Path::new("a.svg")));
    }

    #[test]
    fn decode_budget_counts_pixels_before_decode() {
        assert!(image_pixels(8_000, 10_000) <= MAX_IMAGE_DECODE_PIXELS);
        assert!(image_pixels(8_001, 10_000) > MAX_IMAGE_DECODE_PIXELS);
    }

    #[test]
    fn optimizes_output_image_and_rewrites_html_without_touching_excluded_file() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_temp_dir();
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let output_dir = root.join("sursa/public");
        let images = output_dir.join("imagini");
        fs::create_dir_all(&images).unwrap();
        write_test_png(&images.join("hero.png"));
        write_test_png(&images.join("logo-nr.png"));
        fs::write(
            output_dir.join("index.html"),
            r#"<img src="/imagini/hero.png"><img src="/imagini/logo-nr.png">"#,
        )
        .unwrap();

        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");
        let app_handle = app.handle().clone();
        ensure_app_home(&app_handle).expect("test app home should be available");
        let session_dir = root.join("app-home/data/sessions/image-optimizer-test");
        fs::create_dir_all(&session_dir).unwrap();
        install_test_project_authority(
            &app_handle,
            "image-optimizer-test/runtime",
            &root,
            &session_dir,
        )
        .unwrap();

        let report = optimize_output_images(
            &app_handle,
            &output_dir,
            &ImageOptimizationOptions {
                max_dimension: 2,
                exclude_suffix: "-nr".to_string(),
                replace_only_if_smaller: false,
            },
            "image-optimizer-test/runtime",
        )
        .unwrap();

        assert_eq!(report.optimized, 1);
        assert_eq!(report.skipped_excluded, 1);
        assert!(!images.join("hero.png").exists());
        assert!(images.join("hero.webp").exists());
        assert!(images.join("logo-nr.png").exists());
        let html = fs::read_to_string(output_dir.join("index.html")).unwrap();
        assert!(html.contains("/imagini/hero.webp"));
        assert!(html.contains("/imagini/logo-nr.png"));

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn remove_recovery_aborts_optimizer_without_returning_a_partial_report() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_temp_dir();
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let output_dir = root.join("sursa/public");
        let images = output_dir.join("imagini");
        fs::create_dir_all(&images).unwrap();
        let original = images.join("hero.png");
        write_test_png(&original);

        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");
        let app_handle = app.handle().clone();
        ensure_app_home(&app_handle).expect("test app home should be available");
        let session_dir = root.join("app-home/data/sessions/image-optimizer-recovery-test");
        fs::create_dir_all(&session_dir).unwrap();
        install_test_project_authority(
            &app_handle,
            "image-optimizer-recovery-test/runtime",
            &root,
            &session_dir,
        )
        .unwrap();

        let recreated_original = original.clone();
        let result = with_before_remove_leaf_target_durable_hook_for_test(
            move || {
                fs::write(&recreated_original, b"external replacement").unwrap();
            },
            || {
                optimize_output_images(
                    &app_handle,
                    &output_dir,
                    &ImageOptimizationOptions {
                        max_dimension: 2,
                        exclude_suffix: String::new(),
                        replace_only_if_smaller: false,
                    },
                    "image-optimizer-recovery-test/runtime",
                )
            },
        );

        let WriteAuthorityError::RecoveryRequired(recovery) = result.unwrap_err() else {
            panic!("RemoveFile incert trebuie păstrat ca recovery tipizat");
        };
        assert!(recovery.retry_forbidden());
        assert_eq!(recovery.receipt.owner, WriteOwner::ImageOptimizer);
        assert_eq!(recovery.receipt.operation, WriteOperationKind::RemoveFile);
        assert_eq!(recovery.receipt.status, "recovery_required");
        assert!(images.join("hero.webp").exists());
        assert_eq!(fs::read(&original).unwrap(), b"external replacement");

        fs::remove_dir_all(root).unwrap();
    }

    fn unique_temp_dir() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "panastudio-image-test-{}-{stamp}",
            std::process::id()
        ))
    }

    fn write_test_png(path: &Path) {
        let mut image = image::RgbaImage::new(4, 4);
        for pixel in image.pixels_mut() {
            *pixel = image::Rgba([20, 120, 220, 255]);
        }
        image.save(path).unwrap();
    }

    struct TestEnvGuard {
        previous_values: Vec<(&'static str, Option<String>)>,
    }

    impl TestEnvGuard {
        fn from_root(root: &Path) -> Self {
            let bindings = [
                ("XDG_CONFIG_HOME", root.join("config")),
                ("XDG_DATA_HOME", root.join("data")),
                ("XDG_CACHE_HOME", root.join("cache")),
                ("XDG_STATE_HOME", root.join("state")),
            ];
            let previous_values = bindings
                .iter()
                .map(|(key, _)| (*key, env::var(key).ok()))
                .collect::<Vec<_>>();
            for (key, path) in bindings {
                env::set_var(key, path);
            }
            Self { previous_values }
        }
    }

    impl Drop for TestEnvGuard {
        fn drop(&mut self) {
            for (key, value) in &self.previous_values {
                if let Some(value) = value {
                    env::set_var(key, value);
                } else {
                    env::remove_var(key);
                }
            }
        }
    }
}
