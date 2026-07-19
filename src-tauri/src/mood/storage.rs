use serde_json::Value;
use std::{
    collections::HashMap,
    fs,
    io::{Cursor, ErrorKind},
    path::{Path, PathBuf},
};
use tauri::{AppHandle, Runtime};

use crate::{
    kernel::{
        file_buffer_store::{
            read_project_disk_text_snapshot, FileBufferStoreLimits, ProjectDiskTextReadOutcome,
        },
        project_path::normalize_project_relative_path,
        write_authority::{
            WriteAuthority, WriteCategory, WriteIntent, WriteOperationKind, WriteOwner,
            WritePolicy, WriteTarget,
        },
    },
    project::resolve_project_write_path,
};

const MOOD_DIR: &str = "design";
const MOOD_FILE: &str = "mood-board.json";
const MOOD_BOARD_RELATIVE_PATH: &str = "design/mood-board.json";
const LEGACY_MOOD_BOARD_RELATIVE_PATH: &str = ".panastudio/mood-board.json";
const MAX_MOOD_BOARD_DOCUMENT_BYTES: u64 = 2 * 1024 * 1024;
const IMAGE_EXTENSIONS: &[&str] = &["avif", "gif", "jpeg", "jpg", "png", "svg", "webp"];
const MAX_RAW_PREVIEW_BYTES: u64 = 3 * 1024 * 1024;
const MAX_THUMBNAIL_SOURCE_BYTES: u64 = 80 * 1024 * 1024;
pub(crate) const MAX_MOOD_BOARD_ASSET_BYTES: u64 = 64 * 1024 * 1024;
pub(crate) const MAX_MOOD_BOARD_SVG_BYTES: usize = 4 * 1024 * 1024;
const MAX_EXPORT_SVG_BYTES: u64 = 512 * 1024;
const MAX_EXPORT_PROJECTION_BYTES: usize = 16 * 1024 * 1024;
const MAX_DECODED_IMAGE_EDGE: u32 = 8_192;
const MAX_DECODED_IMAGE_ALLOC_BYTES: u64 = 256 * 1024 * 1024;
const THUMBNAIL_MAX_EDGE: u32 = 420;
const EXPORT_PROJECTION_MAX_EDGE: u32 = 2_048;
const PALETTE_SAMPLE_MAX_EDGE: u32 = 96;

fn mood_board_path(root: &Path) -> PathBuf {
    root.join(MOOD_DIR).join(MOOD_FILE)
}

fn mood_board_document_limits() -> FileBufferStoreLimits {
    FileBufferStoreLimits {
        max_files: 1,
        max_file_bytes: MAX_MOOD_BOARD_DOCUMENT_BYTES,
        max_total_bytes: MAX_MOOD_BOARD_DOCUMENT_BYTES,
    }
}

fn validate_mood_board_document_envelope(board: &Value, label: &str) -> Result<(), String> {
    let document = board
        .as_object()
        .ok_or_else(|| format!("Mood board-ul {label} nu este un obiect JSON."))?;
    if document.get("version").and_then(Value::as_u64) != Some(2) {
        return Err(format!(
            "Mood board-ul {label} are o versiune absentă sau incompatibilă; este acceptată numai versiunea 2."
        ));
    }
    if document.get("updatedAt").and_then(Value::as_f64).is_none() {
        return Err(format!(
            "Mood board-ul {label} nu are un câmp updatedAt numeric valid."
        ));
    }
    let viewport = document
        .get("viewport")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("Mood board-ul {label} nu are un viewport valid."))?;
    if ["x", "y", "zoom"]
        .iter()
        .any(|field| viewport.get(*field).and_then(Value::as_f64).is_none())
    {
        return Err(format!(
            "Mood board-ul {label} are coordonate viewport invalide."
        ));
    }
    if !document.get("items").is_some_and(Value::is_array) {
        return Err(format!(
            "Mood board-ul {label} nu are o listă items validă."
        ));
    }
    Ok(())
}

fn read_mood_board_file(root: &Path, relative_path: &str) -> Result<Option<Value>, String> {
    let limits = mood_board_document_limits();
    let source = match read_project_disk_text_snapshot(root, relative_path, &limits) {
        ProjectDiskTextReadOutcome::Loaded(snapshot) => snapshot.text,
        ProjectDiskTextReadOutcome::Missing => return Ok(None),
        ProjectDiskTextReadOutcome::NotFile => {
            return Err(format!(
                "Mood board-ul {relative_path} nu este un fișier regulat."
            ));
        }
        ProjectDiskTextReadOutcome::Oversized(bytes) => {
            return Err(format!(
                "Mood board-ul {relative_path} are {bytes} bytes și depășește limita de {MAX_MOOD_BOARD_DOCUMENT_BYTES} bytes."
            ));
        }
        ProjectDiskTextReadOutcome::InvalidPath(message) => {
            return Err(format!(
                "Path-ul mood board-ului {relative_path} este invalid: {message}"
            ));
        }
        ProjectDiskTextReadOutcome::UnsafePath(message) => {
            return Err(format!(
                "Citirea mood board-ului {relative_path} a fost refuzată ca nesigură: {message}"
            ));
        }
        ProjectDiskTextReadOutcome::Unstable(message) => {
            return Err(format!(
                "Mood board-ul {relative_path} s-a modificat în timpul citirii: {message}"
            ));
        }
        ProjectDiskTextReadOutcome::Unreadable(message) => {
            return Err(format!(
                "Mood board-ul {relative_path} nu poate fi citit ca text UTF-8: {message}"
            ));
        }
    };
    let board = serde_json::from_str(&source)
        .map_err(|error| format!("Mood board-ul {relative_path} nu este JSON valid: {error}"))?;
    validate_mood_board_document_envelope(&board, relative_path)?;
    Ok(Some(board))
}

fn write_mood_board_file<R: Runtime>(
    app: &AppHandle<R>,
    root: &Path,
    path: &Path,
    board: &Value,
    expected_runtime_session_id: &str,
) -> Result<(), String> {
    validate_mood_board_document_envelope(board, MOOD_BOARD_RELATIVE_PATH)?;
    let body = serde_json::to_string_pretty(&board)
        .map_err(|error| format!("Nu am putut serializa mood board-ul: {}", error))?;
    let serialized_bytes = body.len().saturating_add(1);
    if u64::try_from(serialized_bytes).unwrap_or(u64::MAX) > MAX_MOOD_BOARD_DOCUMENT_BYTES {
        return Err(format!(
            "Mood board-ul depășește limita de {MAX_MOOD_BOARD_DOCUMENT_BYTES} bytes și nu poate fi salvat."
        ));
    }
    let intent = WriteIntent::new(
        WriteCategory::ProjectDesignWrite,
        WriteOwner::MoodBoard,
        WriteOperationKind::WriteText,
        WriteTarget::new(
            path.to_path_buf(),
            root.to_path_buf(),
            "project/design/mood-board.json",
        )
        .with_expected_runtime_session_id(expected_runtime_session_id),
        WritePolicy::project_design_state_save(),
        "Mood Board salvează starea design document.",
    );

    WriteAuthority::new(app)
        .write_text(intent, &format!("{body}\n"))
        .map(|_| ())
        .map_err(|error| error.into_terminal_diagnostic())?;

    Ok(())
}

pub fn read_mood_board(root: &Path) -> Result<Option<Value>, String> {
    // Canonical is authoritative whenever it exists. A corrupt, oversized or
    // unsafe canonical document must be surfaced, never hidden by a legacy
    // fallback that a later save could use to overwrite it.
    if let Some(board) = read_mood_board_file(root, MOOD_BOARD_RELATIVE_PATH)? {
        return Ok(Some(board));
    }
    // Legacy compatibility is deliberately read-only. Migration and cleanup
    // are disk mutations and require their own explicit transactional contract.
    read_mood_board_file(root, LEGACY_MOOD_BOARD_RELATIVE_PATH)
}

pub fn write_mood_board<R: Runtime>(
    app: &AppHandle<R>,
    root: &Path,
    board: Value,
    expected_runtime_session_id: &str,
) -> Result<Value, String> {
    let path = mood_board_path(root);
    write_mood_board_file(app, root, &path, &board, expected_runtime_session_id)?;
    Ok(board)
}

pub fn export_mood_board_svg_asset<R: Runtime>(
    app: &AppHandle<R>,
    root: &Path,
    relative_path: &str,
    svg: &str,
    expected_runtime_session_id: &str,
) -> Result<String, String> {
    let normalized_relative_path = normalize_project_relative_path(relative_path)?;
    validate_mood_board_asset_export_target(&normalized_relative_path, Some("svg"))?;
    validate_mood_board_svg_source(svg)?;
    let path = resolve_project_write_path(root, &normalized_relative_path)?;
    validate_mood_board_asset_is_new(&path, &normalized_relative_path)?;

    let intent = WriteIntent::new(
        WriteCategory::ProjectDesignWrite,
        WriteOwner::MoodBoard,
        WriteOperationKind::WriteBytes,
        WriteTarget::new(
            path,
            root.to_path_buf(),
            format!("project/{normalized_relative_path}"),
        )
        .with_expected_absent()
        .with_expected_runtime_session_id(expected_runtime_session_id),
        WritePolicy::project_design_asset_create(),
        "Mood Board exportă vectorul ca asset SVG create-only.",
    );
    WriteAuthority::new(app)
        .write_bytes(intent, svg.as_bytes())
        .map_err(|error| error.into_terminal_diagnostic())
        .map(|_| normalized_relative_path)
}

pub fn export_mood_board_binary_asset<R: Runtime>(
    app: &AppHandle<R>,
    root: &Path,
    relative_path: &str,
    bytes: &[u8],
    required_extension: Option<&str>,
    description: impl Into<String>,
    expected_runtime_session_id: &str,
) -> Result<String, String> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_MOOD_BOARD_ASSET_BYTES {
        return Err(format!(
            "Exportul Mood Board depășește limita de {} MiB per asset.",
            MAX_MOOD_BOARD_ASSET_BYTES / (1024 * 1024)
        ));
    }
    let normalized_relative_path = normalize_project_relative_path(relative_path)?;
    validate_mood_board_asset_export_target(&normalized_relative_path, required_extension)?;
    let path = resolve_project_write_path(root, &normalized_relative_path)?;
    validate_mood_board_asset_is_new(&path, &normalized_relative_path)?;

    let intent = WriteIntent::new(
        WriteCategory::ProjectDesignWrite,
        WriteOwner::MoodBoard,
        WriteOperationKind::WriteBytes,
        WriteTarget::new(
            path,
            root.to_path_buf(),
            format!("project/{normalized_relative_path}"),
        )
        .with_expected_absent()
        .with_expected_runtime_session_id(expected_runtime_session_id),
        WritePolicy::project_design_asset_create(),
        description,
    );
    WriteAuthority::new(app)
        .write_bytes(intent, bytes)
        .map_err(|error| error.into_terminal_diagnostic())
        .map(|_| normalized_relative_path)
}

fn validate_mood_board_asset_export_target(
    relative_path: &str,
    required_extension: Option<&str>,
) -> Result<(), String> {
    if !(relative_path.starts_with("design/imagini/")
        || relative_path.starts_with("resurse/imagini/"))
    {
        return Err(
            "Mood Board poate exporta imagini doar în design/imagini sau resurse/imagini."
                .to_string(),
        );
    }
    let extension = Path::new(relative_path)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| "Exportul Mood Board cere un fișier imagine cu extensie.".to_string())?;
    if let Some(required_extension) = required_extension {
        if extension != required_extension {
            return Err(format!(
                "Exportul Mood Board cere extensia .{required_extension} pentru acest format."
            ));
        }
    }
    if !IMAGE_EXTENSIONS.contains(&extension.as_str()) {
        return Err("Mood Board acceptă momentan doar fișiere imagine.".to_string());
    }
    Ok(())
}

fn validate_mood_board_asset_is_new(path: &Path, relative_path: &str) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(format!(
            "Mood Board a blocat exportul: {relative_path} este symlink."
        )),
        Ok(metadata) if metadata.is_dir() => Err(format!(
            "Mood Board a blocat exportul: {relative_path} este director."
        )),
        Ok(_) => Err(format!(
            "Mood Board a blocat suprascrierea imaginii existente {relative_path}. Șterge sau redenumește fișierul înainte de export."
        )),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "Nu am putut verifica exportul Mood Board {relative_path} înainte de scriere: {error}"
        )),
    }
}

fn validate_mood_board_svg_source(svg: &str) -> Result<(), String> {
    if svg.len() > MAX_MOOD_BOARD_SVG_BYTES {
        return Err(format!(
            "Exportul Mood Board SVG depășește limita de {} MiB.",
            MAX_MOOD_BOARD_SVG_BYTES / (1024 * 1024)
        ));
    }
    let trimmed = svg.trim_start();
    let header = trimmed
        .chars()
        .take(512)
        .collect::<String>()
        .to_ascii_lowercase();
    let body = trimmed.to_ascii_lowercase();
    let has_svg_start =
        header.starts_with("<svg") || (header.starts_with("<?xml") && header.contains("<svg"));
    if !has_svg_start || !body.contains("</svg>") {
        return Err(
            "Exportul Mood Board SVG a fost blocat: sursa nu pare să fie SVG complet.".to_string(),
        );
    }
    Ok(())
}

pub fn normalize_mood_board_image_relative_path(relative_path: &str) -> Result<String, String> {
    let normalized = normalize_project_relative_path(relative_path)?;
    let extension = Path::new(&normalized)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| "Fișierul nu pare să fie o imagine.".to_string())?;
    if !IMAGE_EXTENSIONS.contains(&extension.as_str()) {
        return Err("Mood Board acceptă momentan doar fișiere imagine.".to_string());
    }
    Ok(normalized)
}

pub fn normalize_mood_board_svg_source_relative_path(
    relative_path: &str,
) -> Result<String, String> {
    let normalized = normalize_mood_board_image_relative_path(relative_path)?;
    let is_svg = Path::new(&normalized)
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"));
    if !is_svg {
        return Err("Importul SVG editabil cere un fișier cu extensia .svg.".to_string());
    }
    Ok(normalized)
}

fn image_mime_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("avif") => "image/avif",
        Some("gif") => "image/gif",
        Some("jpeg") | Some("jpg") => "image/jpeg",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}

fn can_generate_thumbnail(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("jpeg") | Some("jpg") | Some("png") | Some("webp")
    )
}

fn can_extract_palette(path: &Path) -> bool {
    can_generate_thumbnail(path)
}

fn decode_mood_board_image(bytes: Vec<u8>, operation: &str) -> Result<image::DynamicImage, String> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| format!("Nu am putut detecta formatul imaginii: {}", error))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DECODED_IMAGE_EDGE);
    limits.max_image_height = Some(MAX_DECODED_IMAGE_EDGE);
    limits.max_alloc = Some(MAX_DECODED_IMAGE_ALLOC_BYTES);
    reader.limits(limits);
    reader.decode().map_err(|error| {
        format!("Nu am putut decoda imaginea pentru {operation} în limitele de resurse: {error}")
    })
}

fn generate_thumbnail_data_url(bytes: Vec<u8>) -> Result<String, String> {
    let image = decode_mood_board_image(bytes, "preview")?;

    let thumbnail = image.thumbnail(THUMBNAIL_MAX_EDGE, THUMBNAIL_MAX_EDGE);
    let mut bytes = Vec::new();
    let mut cursor = Cursor::new(&mut bytes);
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, 76);
    encoder
        .encode_image(&thumbnail)
        .map_err(|error| format!("Nu am putut encoda preview-ul imaginii: {}", error))?;

    Ok(format!("data:image/jpeg;base64,{}", encode_base64(&bytes)))
}

fn generate_export_projection_bytes(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    let image = decode_mood_board_image(bytes, "proiecția de export")?;
    let projection = image.thumbnail(EXPORT_PROJECTION_MAX_EDGE, EXPORT_PROJECTION_MAX_EDGE);
    let rgba = projection.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut encoded = Vec::new();
    image::codecs::webp::WebPEncoder::new_lossless(&mut encoded)
        .encode(rgba.as_raw(), width, height, image::ColorType::Rgba8.into())
        .map_err(|error| format!("Nu am putut encoda proiecția WebP pentru export: {error}"))?;
    if encoded.len() > MAX_EXPORT_PROJECTION_BYTES {
        return Err(format!(
            "Proiecția de export depășește limita de {} MiB.",
            MAX_EXPORT_PROJECTION_BYTES / (1024 * 1024)
        ));
    }
    Ok(encoded)
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);

    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);

        output.push(TABLE[(first >> 2) as usize] as char);
        output.push(TABLE[(((first & 0b0000_0011) << 4) | (second >> 4)) as usize] as char);

        if chunk.len() > 1 {
            output.push(TABLE[(((second & 0b0000_1111) << 2) | (third >> 6)) as usize] as char);
        } else {
            output.push('=');
        }

        if chunk.len() > 2 {
            output.push(TABLE[(third & 0b0011_1111) as usize] as char);
        } else {
            output.push('=');
        }
    }

    output
}

fn require_bounded_reader_result(
    bytes: Vec<u8>,
    max_bytes: u64,
    operation: &str,
) -> Result<Vec<u8>, String> {
    let observed = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if observed > max_bytes {
        return Err(format!(
            "Citirea Mood Board a returnat {observed} bytes pentru {operation}, peste limita de {max_bytes} bytes."
        ));
    }
    Ok(bytes)
}

pub fn read_mood_board_image_data_url_with_reader<F>(
    relative_path: &str,
    read: F,
) -> Result<String, String>
where
    F: FnOnce(u64, &str) -> Result<Vec<u8>, String>,
{
    let normalized = normalize_mood_board_image_relative_path(relative_path)?;
    let path = Path::new(&normalized);

    if can_generate_thumbnail(path) {
        let bytes = require_bounded_reader_result(
            read(MAX_THUMBNAIL_SOURCE_BYTES, "preview")?,
            MAX_THUMBNAIL_SOURCE_BYTES,
            "preview",
        )?;
        return generate_thumbnail_data_url(bytes);
    }

    let bytes = require_bounded_reader_result(
        read(MAX_RAW_PREVIEW_BYTES, "preview direct")?,
        MAX_RAW_PREVIEW_BYTES,
        "preview direct",
    )?;
    Ok(format!(
        "data:{};base64,{}",
        image_mime_type(path),
        encode_base64(&bytes)
    ))
}

pub fn read_mood_board_image_original_data_url_with_reader<F>(
    relative_path: &str,
    read: F,
) -> Result<String, String>
where
    F: FnOnce(u64, &str) -> Result<Vec<u8>, String>,
{
    let normalized = normalize_mood_board_image_relative_path(relative_path)?;
    let path = Path::new(&normalized);
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if extension == "svg" {
        let bytes = require_bounded_reader_result(
            read(MAX_EXPORT_SVG_BYTES, "proiecția SVG de export")?,
            MAX_EXPORT_SVG_BYTES,
            "proiecția SVG de export",
        )?;
        return Ok(format!(
            "data:image/svg+xml;base64,{}",
            encode_base64(&bytes)
        ));
    }
    if !can_generate_thumbnail(path) {
        return Err(
            "Proiecția de export este disponibilă pentru JPG, PNG, WebP și SVG bounded; GIF/AVIF original nu este trimis în WebView."
                .to_string(),
        );
    }
    let bytes = require_bounded_reader_result(
        read(MAX_THUMBNAIL_SOURCE_BYTES, "proiecția raster de export")?,
        MAX_THUMBNAIL_SOURCE_BYTES,
        "proiecția raster de export",
    )?;
    let projection = generate_export_projection_bytes(bytes)?;
    Ok(format!(
        "data:image/webp;base64,{}",
        encode_base64(&projection)
    ))
}

#[derive(Default)]
struct ColorBucket {
    count: u32,
    red: u64,
    green: u64,
    blue: u64,
}

fn color_score(red: u8, green: u8, blue: u8, count: u32) -> f32 {
    let max = red.max(green).max(blue) as f32 / 255.0;
    let min = red.min(green).min(blue) as f32 / 255.0;
    let saturation = if max <= f32::EPSILON {
        0.0
    } else {
        (max - min) / max
    };
    let luminance = (0.2126 * red as f32 + 0.7152 * green as f32 + 0.0722 * blue as f32) / 255.0;
    let middle_luminance = 1.0 - (luminance - 0.54).abs().min(0.54) / 0.54;
    count as f32 * (0.7 + saturation * 1.4 + middle_luminance * 0.45)
}

fn is_too_close_to_existing(red: u8, green: u8, blue: u8, colors: &[(u8, u8, u8)]) -> bool {
    colors.iter().any(|(r, g, b)| {
        let dr = i32::from(red) - i32::from(*r);
        let dg = i32::from(green) - i32::from(*g);
        let db = i32::from(blue) - i32::from(*b);
        dr * dr + dg * dg + db * db < 34 * 34
    })
}

pub fn extract_mood_board_image_palette_with_reader<F>(
    relative_path: &str,
    max_colors: usize,
    read: F,
) -> Result<Vec<String>, String>
where
    F: FnOnce(u64, &str) -> Result<Vec<u8>, String>,
{
    let normalized = normalize_mood_board_image_relative_path(relative_path)?;
    let path = Path::new(&normalized);
    if !can_extract_palette(path) {
        return Err("Paleta poate fi extrasă momentan doar din JPG, PNG și WebP.".to_string());
    }

    let bytes = require_bounded_reader_result(
        read(MAX_THUMBNAIL_SOURCE_BYTES, "extragerea paletei")?,
        MAX_THUMBNAIL_SOURCE_BYTES,
        "extragerea paletei",
    )?;
    let image = decode_mood_board_image(bytes, "extragerea paletei")?;
    let sample = image
        .thumbnail(PALETTE_SAMPLE_MAX_EDGE, PALETTE_SAMPLE_MAX_EDGE)
        .to_rgba8();

    let mut buckets: HashMap<(u8, u8, u8), ColorBucket> = HashMap::new();
    for pixel in sample.pixels() {
        let [red, green, blue, alpha] = pixel.0;
        if alpha < 160 {
            continue;
        }
        let key = (red >> 4, green >> 4, blue >> 4);
        let bucket = buckets.entry(key).or_default();
        bucket.count += 1;
        bucket.red += red as u64;
        bucket.green += green as u64;
        bucket.blue += blue as u64;
    }

    let mut candidates = buckets
        .into_values()
        .filter(|bucket| bucket.count >= 3)
        .map(|bucket| {
            let red = (bucket.red / bucket.count as u64) as u8;
            let green = (bucket.green / bucket.count as u64) as u8;
            let blue = (bucket.blue / bucket.count as u64) as u8;
            (
                red,
                green,
                blue,
                color_score(red, green, blue, bucket.count),
            )
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| right.3.total_cmp(&left.3));

    let mut picked: Vec<(u8, u8, u8)> = Vec::new();
    for (red, green, blue, _) in candidates {
        if is_too_close_to_existing(red, green, blue, &picked) {
            continue;
        }
        picked.push((red, green, blue));
        if picked.len() >= max_colors.max(1).min(12) {
            break;
        }
    }

    if picked.is_empty() {
        return Err("Nu am găsit culori relevante în imagine.".to_string());
    }

    Ok(picked
        .into_iter()
        .map(|(red, green, blue)| format!("#{red:02x}{green:02x}{blue:02x}"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{
        extract_mood_board_image_palette_with_reader, generate_export_projection_bytes,
        read_mood_board, read_mood_board_image_data_url_with_reader,
        read_mood_board_image_original_data_url_with_reader, validate_mood_board_svg_source,
        write_mood_board_file, EXPORT_PROJECTION_MAX_EDGE, MAX_EXPORT_PROJECTION_BYTES,
        MAX_EXPORT_SVG_BYTES, MAX_MOOD_BOARD_DOCUMENT_BYTES, MAX_MOOD_BOARD_SVG_BYTES,
        MAX_THUMBNAIL_SOURCE_BYTES, MOOD_BOARD_RELATIVE_PATH,
    };
    use std::{
        fs,
        io::Cursor,
        sync::atomic::{AtomicU64, Ordering},
    };

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn svg_export_is_bounded_before_write_authority() {
        let mut source = String::from("<svg>");
        source.push_str(&" ".repeat(MAX_MOOD_BOARD_SVG_BYTES));
        source.push_str("</svg>");
        let error = validate_mood_board_svg_source(&source).unwrap_err();
        assert!(error.contains("depășește limita"));
    }

    #[test]
    fn raster_export_projection_is_resampled_and_output_bounded() {
        let source = image::DynamicImage::new_rgba8(3_000, 32);
        let mut encoded_source = Cursor::new(Vec::new());
        source
            .write_to(&mut encoded_source, image::ImageFormat::Png)
            .unwrap();

        let projection = generate_export_projection_bytes(encoded_source.into_inner()).unwrap();
        assert!(projection.len() <= MAX_EXPORT_PROJECTION_BYTES);
        let decoded = image::load_from_memory(&projection).unwrap();
        assert!(decoded.width() <= EXPORT_PROJECTION_MAX_EDGE);
        assert!(decoded.height() <= EXPORT_PROJECTION_MAX_EDGE);
    }

    #[test]
    fn reader_driven_preview_selects_the_limit_and_transforms_raster_without_filesystem_access() {
        let png = encoded_test_png();
        let preview = read_mood_board_image_data_url_with_reader(
            "design/imagini/test.png",
            move |max_bytes, operation| {
                assert_eq!(max_bytes, MAX_THUMBNAIL_SOURCE_BYTES);
                assert_eq!(operation, "preview");
                Ok(png)
            },
        )
        .unwrap();
        assert!(preview.starts_with("data:image/jpeg;base64,"));

        let invalid =
            read_mood_board_image_data_url_with_reader("design/imagini/test.txt", |_, _| {
                panic!("extensia invalidă trebuie respinsă înaintea readerului")
            })
            .unwrap_err();
        assert!(invalid.contains("doar fișiere imagine"));
    }

    #[test]
    fn reader_driven_export_rejects_a_reader_that_breaks_the_selected_bound() {
        let error = read_mood_board_image_original_data_url_with_reader(
            "design/imagini/test.svg",
            |max_bytes, operation| {
                assert_eq!(max_bytes, MAX_EXPORT_SVG_BYTES);
                assert_eq!(operation, "proiecția SVG de export");
                Ok(vec![0; max_bytes as usize + 1])
            },
        )
        .unwrap_err();
        assert!(error.contains("peste limita"));

        let unsupported = read_mood_board_image_original_data_url_with_reader(
            "design/imagini/test.gif",
            |_, _| panic!("GIF export trebuie respins înaintea readerului"),
        )
        .unwrap_err();
        assert!(unsupported.contains("GIF/AVIF"));
    }

    #[test]
    fn reader_driven_export_and_palette_transform_captured_raster_bytes() {
        let png = encoded_test_png();
        let projection =
            read_mood_board_image_original_data_url_with_reader("resurse/imagini/test.png", {
                let png = png.clone();
                move |max_bytes, operation| {
                    assert_eq!(max_bytes, MAX_THUMBNAIL_SOURCE_BYTES);
                    assert_eq!(operation, "proiecția raster de export");
                    Ok(png)
                }
            })
            .unwrap();
        assert!(projection.starts_with("data:image/webp;base64,"));

        let colors = extract_mood_board_image_palette_with_reader(
            "resurse/imagini/test.png",
            2,
            move |max_bytes, operation| {
                assert_eq!(max_bytes, MAX_THUMBNAIL_SOURCE_BYTES);
                assert_eq!(operation, "extragerea paletei");
                Ok(png)
            },
        )
        .unwrap();
        assert!(!colors.is_empty());
        assert!(colors.len() <= 2);
        assert!(colors.iter().all(|color| color.starts_with('#')));
    }

    #[test]
    fn mood_document_prefers_canonical_without_mutating_canonical_or_legacy() {
        let root = unique_test_dir("canonical-priority");
        fs::create_dir_all(root.join("design")).unwrap();
        fs::create_dir_all(root.join(".panastudio")).unwrap();
        let canonical = valid_board("canonical");
        let legacy = valid_board("legacy");
        fs::write(root.join("design/mood-board.json"), &canonical).unwrap();
        fs::write(root.join(".panastudio/mood-board.json"), &legacy).unwrap();

        let board = read_mood_board(&root).unwrap().unwrap();
        assert_eq!(board["marker"], "canonical");
        assert_eq!(
            fs::read(root.join("design/mood-board.json")).unwrap(),
            canonical
        );
        assert_eq!(
            fs::read(root.join(".panastudio/mood-board.json")).unwrap(),
            legacy
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_mood_document_fallback_is_read_only() {
        let root = unique_test_dir("legacy-read-only");
        fs::create_dir_all(root.join(".panastudio")).unwrap();
        let legacy = valid_board("legacy-only");
        fs::write(root.join(".panastudio/mood-board.json"), &legacy).unwrap();

        let board = read_mood_board(&root).unwrap().unwrap();
        assert_eq!(board["marker"], "legacy-only");
        assert!(!root.join("design/mood-board.json").exists());
        assert_eq!(
            fs::read(root.join(".panastudio/mood-board.json")).unwrap(),
            legacy
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_canonical_mood_document_never_falls_back_to_legacy() {
        let root = unique_test_dir("invalid-canonical");
        fs::create_dir_all(root.join("design")).unwrap();
        fs::create_dir_all(root.join(".panastudio")).unwrap();
        fs::write(root.join("design/mood-board.json"), b"{broken").unwrap();
        fs::write(
            root.join(".panastudio/mood-board.json"),
            valid_board("must-not-fallback"),
        )
        .unwrap();

        let error = read_mood_board(&root).unwrap_err();
        assert!(error.contains(MOOD_BOARD_RELATIVE_PATH));
        assert!(error.contains("JSON valid"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn oversized_canonical_mood_document_never_falls_back_to_legacy() {
        let root = unique_test_dir("oversized-canonical");
        fs::create_dir_all(root.join("design")).unwrap();
        fs::create_dir_all(root.join(".panastudio")).unwrap();
        fs::File::create(root.join("design/mood-board.json"))
            .unwrap()
            .set_len(MAX_MOOD_BOARD_DOCUMENT_BYTES + 1)
            .unwrap();
        fs::write(
            root.join(".panastudio/mood-board.json"),
            valid_board("must-not-fallback"),
        )
        .unwrap();

        let error = read_mood_board(&root).unwrap_err();
        assert!(error.contains("depășește limita"));

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_canonical_mood_document_never_falls_back_to_legacy() {
        use std::os::unix::fs::symlink;

        let root = unique_test_dir("symlink-canonical");
        fs::create_dir_all(root.join("design")).unwrap();
        fs::create_dir_all(root.join(".panastudio")).unwrap();
        let outside = root.with_extension("outside.json");
        fs::write(&outside, valid_board("outside")).unwrap();
        symlink(&outside, root.join("design/mood-board.json")).unwrap();
        fs::write(
            root.join(".panastudio/mood-board.json"),
            valid_board("must-not-fallback"),
        )
        .unwrap();

        let error = read_mood_board(&root).unwrap_err();
        assert!(error.contains("nesigură"));

        fs::remove_dir_all(root).unwrap();
        fs::remove_file(outside).unwrap();
    }

    #[test]
    fn mood_document_write_rejects_a_payload_larger_than_the_read_budget_before_effect() {
        let root = unique_test_dir("oversized-write");
        fs::create_dir_all(root.join("design")).unwrap();
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build");
        let mut board: serde_json::Value =
            serde_json::from_slice(&valid_board("oversized")).unwrap();
        board["marker"] =
            serde_json::Value::String("x".repeat(MAX_MOOD_BOARD_DOCUMENT_BYTES as usize));

        let error = write_mood_board_file(
            app.handle(),
            &root,
            &root.join("design/mood-board.json"),
            &board,
            "test-session",
        )
        .unwrap_err();
        assert!(error.contains("depășește limita"));
        assert!(!root.join("design/mood-board.json").exists());

        drop(app);
        fs::remove_dir_all(root).unwrap();
    }

    fn valid_board(marker: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "version": 2,
            "updatedAt": 1,
            "viewport": { "x": 0, "y": 0, "zoom": 1 },
            "items": [],
            "marker": marker,
        }))
        .unwrap()
    }

    fn encoded_test_png() -> Vec<u8> {
        let image = image::RgbaImage::from_fn(16, 8, |x, _| {
            if x < 8 {
                image::Rgba([230, 35, 45, 255])
            } else {
                image::Rgba([35, 75, 225, 255])
            }
        });
        let mut encoded = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();
        encoded.into_inner()
    }

    fn unique_test_dir(label: &str) -> std::path::PathBuf {
        let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "pana-mood-storage-{}-{sequence}-{label}",
            std::process::id()
        ))
    }
}
