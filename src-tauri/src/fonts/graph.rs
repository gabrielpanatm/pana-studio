use percent_encoding::percent_decode_str;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs,
    path::{Component, Path},
    sync::{Mutex, OnceLock},
};

use super::{
    font_file_sort_weight, font_format_label, font_roots, is_stylesheet_path,
    is_supported_font_file, is_text_optimized_font_file_name, local_font_file_from_bytes,
    managed_font_start_marker, normalize_font_family_name, parse_font_weight, FontCssRegistration,
    FontDeliveryKind, FontFaceFamily, FontFaceGraph, FontFaceIssue, FontFaceIssueSeverity,
    FontFaceSource, FontLicenseMetadata, FontOrigin, FontOwnership, FontRoot, FontRootCandidate,
    FontWeightRange, LocalFontFile, ROMANIAN_GLYPHS,
};
use crate::{kernel::file_buffer_store::hash_bytes, zola_theme::ZolaThemeResolver};

pub const FONT_FACE_GRAPH_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct DiskCacheKey {
    root: String,
    file: String,
    version: String,
}

#[derive(Clone, Debug)]
struct FontAsset {
    origin: FontOrigin,
    theme_name: Option<String>,
    file: LocalFontFile,
}

#[derive(Clone, Debug)]
struct ParsedFace {
    family: String,
    stylesheet: String,
    urls: Vec<String>,
    has_local_source: bool,
    weight: Option<u16>,
    weight_range: Option<FontWeightRange>,
    style: String,
    display: Option<String>,
    unicode_range: Option<String>,
    managed: bool,
}

#[derive(Default)]
struct FamilyBuilder {
    id: String,
    family: String,
    asset_indexes: BTreeSet<usize>,
    faces: Vec<FontFaceSource>,
    issues: Vec<FontFaceIssue>,
    stylesheets: BTreeSet<String>,
    display_modes: BTreeSet<String>,
    managed: bool,
    has_detected: bool,
    has_system: bool,
    has_external: bool,
    has_missing: bool,
}

static DISK_FONT_CACHE: OnceLock<Mutex<HashMap<DiskCacheKey, LocalFontFile>>> = OnceLock::new();
static FONT_METADATA_CACHE: OnceLock<Mutex<HashMap<String, LocalFontFile>>> = OnceLock::new();

pub fn build_font_face_graph<'a>(
    zola_root: &Path,
    sources: impl Iterator<Item = (&'a str, &'a str)>,
    staged_resources: impl Iterator<Item = (&'a str, &'a [u8])>,
    deleted_resources: impl Iterator<Item = &'a str>,
    disk_versions: impl Iterator<Item = (&'a str, &'a str)>,
) -> FontFaceGraph {
    let sources = sources
        .filter(|(path, _)| is_stylesheet_path(path) && !is_public_output(path))
        .collect::<Vec<_>>();
    let deleted = deleted_resources.collect::<HashSet<_>>();
    let versions = disk_versions.collect::<HashMap<_, _>>();
    let (mut roots, mut assets) = scan_disk_assets(zola_root, &versions);
    overlay_staged_assets(&mut roots, &mut assets, staged_resources);
    assets.retain(|asset| !deleted.contains(asset.file.file.as_str()));
    assets.sort_by(asset_order);

    let runtime_indexes = runtime_asset_indexes(&assets);
    let mut logical_assets = BTreeMap::<String, Vec<usize>>::new();
    for index in &runtime_indexes {
        logical_assets
            .entry(public_logical_path(&assets[*index].file.file))
            .or_default()
            .push(*index);
    }

    let parsed_faces = sources
        .iter()
        .flat_map(|(path, source)| parse_font_faces(path, source))
        .collect::<Vec<_>>();
    let mut referenced = HashSet::<usize>::new();
    let mut builders = BTreeMap::<String, FamilyBuilder>::new();

    for face in parsed_faces {
        let normalized_family = normalize_font_family_name(&face.family);
        if normalized_family.is_empty() {
            continue;
        }
        let builder = builders
            .entry(format!("css:{normalized_family}"))
            .or_insert_with(|| FamilyBuilder {
                id: format!("css:{normalized_family}"),
                family: face.family.clone(),
                ..FamilyBuilder::default()
            });
        builder.stylesheets.insert(face.stylesheet.clone());
        builder.managed |= face.managed;
        builder.has_detected |= !face.managed;
        if let Some(display) = face.display.as_ref().filter(|value| !value.is_empty()) {
            builder.display_modes.insert(display.clone());
        }

        let mut face_viable = face.has_local_source;
        if face.has_local_source {
            builder.has_system = true;
            builder.faces.push(font_face_source(
                &face,
                "local()",
                None,
                FontDeliveryKind::System,
                false,
            ));
            builder.issues.push(issue(
                "font_face_src_system",
                FontFaceIssueSeverity::Info,
                format!(
                    "Familia {} folosește local(); disponibilitatea depinde de sistemul utilizatorului.",
                    face.family
                ),
                None,
                Some(face.stylesheet.clone()),
            ));
        }
        if face.urls.is_empty() {
            if !face.has_local_source {
                builder.has_missing = true;
                builder.issues.push(issue(
                    "font_face_src_missing",
                    FontFaceIssueSeverity::Error,
                    format!(
                        "Familia {} are un @font-face fără o sursă src utilizabilă.",
                        face.family
                    ),
                    None,
                    Some(face.stylesheet.clone()),
                ));
            }
            continue;
        }

        for url in &face.urls {
            match classify_font_url(url, &face.stylesheet) {
                ClassifiedUrl::External => {
                    face_viable = true;
                    builder.has_external = true;
                    let reported_url = bounded_external_url(url);
                    builder.faces.push(font_face_source(
                        &face,
                        &reported_url,
                        None,
                        FontDeliveryKind::External,
                        false,
                    ));
                    builder.issues.push(issue(
                        "font_face_src_external",
                        FontFaceIssueSeverity::Info,
                        format!(
                            "Sursa externă {reported_url} este păstrată, dar binarul nu este inspectat local de Rust."
                        ),
                        None,
                        Some(face.stylesheet.clone()),
                    ));
                }
                ClassifiedUrl::Dynamic => {
                    builder.faces.push(font_face_source(
                        &face,
                        url,
                        None,
                        FontDeliveryKind::Missing,
                        true,
                    ));
                    builder.issues.push(issue(
                        "font_face_src_dynamic",
                        FontFaceIssueSeverity::Warning,
                        format!("Sursa dinamică {url} nu poate fi confirmată de Rust."),
                        None,
                        Some(face.stylesheet.clone()),
                    ));
                }
                ClassifiedUrl::Local(logical) => {
                    let candidates = logical_assets.get(&logical).cloned().unwrap_or_default();
                    if candidates.len() == 1 {
                        face_viable = true;
                        let index = candidates[0];
                        referenced.insert(index);
                        builder.asset_indexes.insert(index);
                        builder.faces.push(font_face_source(
                            &face,
                            url,
                            Some(assets[index].file.file.clone()),
                            FontDeliveryKind::Local,
                            false,
                        ));
                        validate_face_against_asset(builder, &face, &assets[index].file);
                    } else {
                        builder.faces.push(font_face_source(
                            &face,
                            url,
                            None,
                            FontDeliveryKind::Missing,
                            false,
                        ));
                        let (code, message) = if candidates.is_empty() {
                            (
                                "font_face_src_unresolved",
                                format!("Sursa {url} nu corespunde niciunui font activ din static/fonturi."),
                            )
                        } else {
                            (
                                "font_face_src_ambiguous",
                                format!("Sursa {url} corespunde mai multor resurse active."),
                            )
                        };
                        builder.issues.push(issue(
                            code,
                            FontFaceIssueSeverity::Error,
                            message,
                            None,
                            Some(face.stylesheet.clone()),
                        ));
                    }
                }
            }
        }
        builder.has_missing |= !face_viable;
    }

    for index in runtime_indexes {
        if referenced.contains(&index) {
            continue;
        }
        let asset = &assets[index];
        let family = asset
            .file
            .internal_family
            .clone()
            .unwrap_or_else(|| asset.file.file_name.clone());
        let key = if asset.file.internal_family.is_some() {
            format!(
                "orphan:{}:{}",
                origin_key(&asset.origin),
                normalize_font_family_name(&family)
            )
        } else {
            format!("orphan:{}:{}", origin_key(&asset.origin), asset.file.file)
        };
        let builder = builders
            .entry(key.clone())
            .or_insert_with(|| FamilyBuilder {
                id: key,
                family: family.clone(),
                has_missing: true,
                ..FamilyBuilder::default()
            });
        builder.asset_indexes.insert(index);
        builder.issues.push(issue(
            "font_file_without_face",
            FontFaceIssueSeverity::Error,
            format!(
                "{} nu este referit de nicio declarație @font-face.",
                asset.file.file_name
            ),
            Some(asset.file.file.clone()),
            None,
        ));
    }

    let mut families = builders
        .into_values()
        .map(|builder| finish_family(builder, &assets))
        .collect::<Vec<_>>();
    families.sort_by(|left, right| {
        left.family
            .to_lowercase()
            .cmp(&right.family.to_lowercase())
            .then_with(|| left.id.cmp(&right.id))
    });

    FontFaceGraph {
        schema_version: FONT_FACE_GRAPH_SCHEMA_VERSION,
        roots,
        families,
    }
}

fn scan_disk_assets(
    zola_root: &Path,
    versions: &HashMap<&str, &str>,
) -> (Vec<FontRoot>, Vec<FontAsset>) {
    let resolver = ZolaThemeResolver::for_root(zola_root);
    let candidates = font_roots(zola_root, &resolver);
    let mut roots = Vec::new();
    let mut assets = Vec::new();
    for root in candidates {
        let exists = fs::symlink_metadata(&root.absolute_path)
            .ok()
            .is_some_and(|metadata| {
                metadata.file_type().is_dir() && !metadata.file_type().is_symlink()
            });
        roots.push(FontRoot {
            relative_path: root.relative_path.clone(),
            origin: root.origin.clone(),
            theme_name: root.theme_name.clone(),
            exists,
        });
        if exists {
            collect_disk_assets(zola_root, &root, &root.absolute_path, versions, &mut assets);
        }
    }
    (roots, assets)
}

fn collect_disk_assets(
    zola_root: &Path,
    root: &FontRootCandidate,
    directory: &Path,
    versions: &HashMap<&str, &str>,
    assets: &mut Vec<FontAsset>,
) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_disk_assets(zola_root, root, &path, versions, assets);
            continue;
        }
        if !file_type.is_file() || !is_supported_font_file(&path) {
            continue;
        }
        let relative = path
            .strip_prefix(zola_root)
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/"));
        let version = versions.get(relative.as_str()).copied().unwrap_or_default();
        if let Some(file) = cached_disk_font(zola_root, &path, &relative, version) {
            assets.push(FontAsset {
                origin: root.origin.clone(),
                theme_name: root.theme_name.clone(),
                file,
            });
        }
    }
}

fn cached_disk_font(
    zola_root: &Path,
    path: &Path,
    relative: &str,
    version: &str,
) -> Option<LocalFontFile> {
    let key = DiskCacheKey {
        root: zola_root.to_string_lossy().into_owned(),
        file: relative.to_string(),
        version: version.to_string(),
    };
    if !version.is_empty() {
        if let Ok(cache) = DISK_FONT_CACHE.get_or_init(Default::default).lock() {
            if let Some(file) = cache.get(&key) {
                return Some(file.clone());
            }
        }
    }
    let metadata = fs::symlink_metadata(path).ok()?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return None;
    }
    let bytes = fs::read(path).ok()?;
    let file_name = path.file_name()?.to_string_lossy().into_owned();
    let extension = path.extension()?.to_string_lossy().to_ascii_lowercase();
    let file = cached_font_from_bytes(relative, &file_name, &extension, &bytes);
    if !version.is_empty() {
        if let Ok(mut cache) = DISK_FONT_CACHE.get_or_init(Default::default).lock() {
            if cache.len() > 4096 {
                cache.clear();
            }
            cache.insert(key, file.clone());
        }
    }
    Some(file)
}

fn cached_font_from_bytes(
    relative: &str,
    file_name: &str,
    extension: &str,
    bytes: &[u8],
) -> LocalFontFile {
    let content_hash = hash_bytes(bytes);
    if let Ok(cache) = FONT_METADATA_CACHE.get_or_init(Default::default).lock() {
        if let Some(cached) = cache.get(&content_hash) {
            let mut file = cached.clone();
            file.file = relative.to_string();
            file.file_name = file_name.to_string();
            file.extension = extension.to_string();
            file.format = font_format_label(extension).to_string();
            file.text_optimized = is_text_optimized_font_file_name(file_name);
            return file;
        }
    }
    let file = local_font_file_from_bytes(relative, file_name, extension, bytes);
    if let Ok(mut cache) = FONT_METADATA_CACHE.get_or_init(Default::default).lock() {
        if cache.len() > 4096 {
            cache.clear();
        }
        if file.internal_family.is_some() {
            cache.insert(content_hash, file.clone());
        }
    }
    file
}

fn overlay_staged_assets<'a>(
    roots: &mut [FontRoot],
    assets: &mut Vec<FontAsset>,
    staged: impl Iterator<Item = (&'a str, &'a [u8])>,
) {
    for (path, bytes) in staged {
        let file_path = Path::new(path);
        if !path.starts_with("static/fonturi/") || !is_supported_font_file(file_path) {
            continue;
        }
        let Some(file_name) = file_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
        else {
            continue;
        };
        let extension = file_path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let file = cached_font_from_bytes(path, &file_name, &extension, bytes);
        assets.retain(|asset| asset.file.file != path);
        assets.push(FontAsset {
            origin: FontOrigin::Local,
            theme_name: None,
            file,
        });
        if let Some(root) = roots
            .iter_mut()
            .find(|root| root.origin == FontOrigin::Local)
        {
            root.exists = true;
        }
    }
}

fn runtime_asset_indexes(assets: &[FontAsset]) -> Vec<usize> {
    let mut selected = BTreeMap::<String, usize>::new();
    for (index, asset) in assets.iter().enumerate() {
        let logical = public_logical_path(&asset.file.file);
        selected.entry(logical).or_insert(index);
    }
    selected.into_values().collect()
}

fn finish_family(builder: FamilyBuilder, assets: &[FontAsset]) -> FontFaceFamily {
    let mut files = builder
        .asset_indexes
        .iter()
        .map(|index| {
            let mut file = assets[*index].file.clone();
            if let Some(face) = builder
                .faces
                .iter()
                .find(|face| face.resolved_file.as_deref() == Some(file.file.as_str()))
            {
                file.declared_weight = face.weight;
                file.declared_weight_range = face.weight_range;
                file.declared_style = Some(face.style.clone());
                file.unicode_range = face.unicode_range.clone();
            }
            file
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| {
        font_file_sort_weight(left)
            .cmp(&font_file_sort_weight(right))
            .then_with(|| left.style.cmp(&right.style))
            .then_with(|| left.file_name.cmp(&right.file_name))
    });
    let mut directories = files
        .iter()
        .map(|file| asset_directory(&file.file))
        .collect::<Vec<_>>();
    directories.sort();
    directories.dedup();
    let origin = builder
        .asset_indexes
        .iter()
        .map(|index| assets[*index].origin.clone())
        .min()
        .unwrap_or(FontOrigin::External);
    let theme_name = builder
        .asset_indexes
        .iter()
        .find_map(|index| assets[*index].theme_name.clone());
    let delivery = if !files.is_empty() && !builder.has_missing {
        FontDeliveryKind::Local
    } else if files.is_empty() && builder.has_external && !builder.has_missing {
        FontDeliveryKind::External
    } else if files.is_empty() && builder.has_system && !builder.has_missing {
        FontDeliveryKind::System
    } else {
        FontDeliveryKind::Missing
    };
    let license = files
        .iter()
        .map(|file| file.license.clone())
        .find(|license| !license.is_empty())
        .unwrap_or_else(FontLicenseMetadata::default);
    let mut issues = builder.issues;
    let romanian_supported = (!files.is_empty()).then(|| {
        ROMANIAN_GLYPHS.iter().all(|required| {
            files
                .iter()
                .any(|file| file.romanian_glyphs.contains(required))
        })
    });
    if romanian_supported == Some(false) {
        issues.push(issue(
            "font_romanian_glyphs_missing",
            FontFaceIssueSeverity::Warning,
            format!(
                "Familia {} nu acoperă toate diacriticele românești din livrarea locală.",
                builder.family
            ),
            None,
            None,
        ));
    }
    diagnose_duplicate_binaries(&builder.family, &files, &mut issues);
    let stylesheets = builder.stylesheets.into_iter().collect::<Vec<_>>();
    let display_modes = builder.display_modes.into_iter().collect::<Vec<_>>();
    FontFaceFamily {
        id: builder.id,
        family: builder.family,
        directories,
        origin,
        theme_name,
        delivery,
        ownership: if builder.managed && !builder.has_detected {
            FontOwnership::Managed
        } else {
            FontOwnership::Detected
        },
        romanian_supported,
        files,
        faces: builder.faces,
        issues,
        license,
        registration: FontCssRegistration {
            registered: !stylesheets.is_empty(),
            managed: builder.managed,
            stylesheets,
            display_modes,
        },
    }
}

fn diagnose_duplicate_binaries(
    family: &str,
    files: &[LocalFontFile],
    issues: &mut Vec<FontFaceIssue>,
) {
    let mut hashes = HashMap::<&str, Vec<&LocalFontFile>>::new();
    for file in files {
        hashes.entry(&file.content_hash).or_default().push(file);
    }
    for duplicates in hashes.into_values().filter(|files| files.len() > 1) {
        issues.push(issue(
            "font_binary_duplicate",
            FontFaceIssueSeverity::Warning,
            format!(
                "Familia {family} referă același binar prin {} fișiere: {}.",
                duplicates.len(),
                duplicates
                    .iter()
                    .map(|file| file.file_name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            duplicates.first().map(|file| file.file.clone()),
            None,
        ));
    }
}

fn validate_face_against_asset(
    builder: &mut FamilyBuilder,
    face: &ParsedFace,
    file: &LocalFontFile,
) {
    if file.internal_family.is_none() {
        builder.issues.push(issue(
            "font_opentype_metadata_unreadable",
            FontFaceIssueSeverity::Error,
            format!(
                "Metadatele OpenType din {} nu au putut fi validate.",
                file.file_name
            ),
            Some(file.file.clone()),
            Some(face.stylesheet.clone()),
        ));
    }
    if file.internal_family.as_deref().is_some_and(|internal| {
        normalize_font_family_name(internal) != normalize_font_family_name(&face.family)
    }) {
        builder.issues.push(issue(
            "font_css_alias",
            FontFaceIssueSeverity::Info,
            format!(
                "Aliasul CSS {} livrează familia OpenType {}.",
                face.family,
                file.internal_family.as_deref().unwrap_or_default()
            ),
            Some(file.file.clone()),
            Some(face.stylesheet.clone()),
        ));
    }
    if let Some(declared) = face.weight {
        let compatible = file
            .weight_range
            .is_some_and(|range| declared >= range.start && declared <= range.end)
            || file.weight == Some(declared);
        if !compatible {
            builder.issues.push(issue(
                "font_weight_mismatch",
                FontFaceIssueSeverity::Warning,
                format!(
                    "{} este declarat cu greutatea {declared}, dar OpenType indică {}.",
                    file.file_name,
                    file.weight
                        .map(|weight| weight.to_string())
                        .unwrap_or_else(|| "un interval incompatibil".to_string())
                ),
                Some(file.file.clone()),
                Some(face.stylesheet.clone()),
            ));
        }
    }
    if let Some(declared) = face.weight_range {
        let compatible = file.weight_range.is_some_and(|internal| {
            declared.start >= internal.start && declared.end <= internal.end
        });
        if !compatible {
            builder.issues.push(issue(
                "font_weight_range_mismatch",
                FontFaceIssueSeverity::Warning,
                format!(
                    "{} este declarat {}–{}, dar fontul nu expune acest interval.",
                    file.file_name, declared.start, declared.end
                ),
                Some(file.file.clone()),
                Some(face.stylesheet.clone()),
            ));
        }
    }
    if file
        .style
        .as_deref()
        .is_some_and(|style| !style.eq_ignore_ascii_case(&face.style))
    {
        builder.issues.push(issue(
            "font_style_mismatch",
            FontFaceIssueSeverity::Warning,
            format!(
                "{} este declarat {}, dar OpenType indică {}.",
                file.file_name,
                face.style,
                file.style.as_deref().unwrap_or("necunoscut")
            ),
            Some(file.file.clone()),
            Some(face.stylesheet.clone()),
        ));
    }
}

fn parse_font_faces(path: &str, source: &str) -> Vec<ParsedFace> {
    let mut faces = Vec::new();
    let has_managed_markers = source.contains("/* pana-studio-font:");
    let mut managed_ranges = HashMap::<String, Vec<(usize, usize)>>::new();
    for block in font_face_blocks(source) {
        let declarations = parse_declarations(block);
        let Some(family) = declarations
            .get("font-family")
            .map(|value| unquote(value))
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let src = declarations.get("src").cloned().unwrap_or_default();
        let (weight, weight_range) = declarations
            .get("font-weight")
            .map(|value| parse_font_weight(value))
            .unwrap_or((Some(400), None));
        let block_offset = block.as_ptr() as usize - source.as_ptr() as usize;
        let managed = if has_managed_markers {
            managed_ranges
                .entry(normalize_font_family_name(&family))
                .or_insert_with(|| managed_font_ranges(source, &family))
                .iter()
                .any(|(start, end)| (*start..*end).contains(&block_offset))
        } else {
            false
        };
        faces.push(ParsedFace {
            managed,
            family,
            stylesheet: path.replace('\\', "/"),
            urls: extract_css_urls(&src),
            has_local_source: contains_css_function(&src, "local"),
            weight,
            weight_range,
            style: declarations
                .get("font-style")
                .map(|style| style.trim().to_ascii_lowercase())
                .filter(|style| !style.is_empty())
                .unwrap_or_else(|| "normal".to_string()),
            display: declarations
                .get("font-display")
                .map(|display| display.trim().to_ascii_lowercase())
                .filter(|display| !display.is_empty()),
            unicode_range: declarations
                .get("unicode-range")
                .map(|range| range.trim().to_string())
                .filter(|range| !range.is_empty()),
        });
    }
    faces
}

fn managed_font_ranges(source: &str, family: &str) -> Vec<(usize, usize)> {
    let start_marker = managed_font_start_marker(family);
    let end_marker = super::managed_font_end_marker(family);
    let mut cursor = 0usize;
    let mut ranges = Vec::new();
    while let Some(relative_start) = source[cursor..].find(&start_marker) {
        let start = cursor + relative_start + start_marker.len();
        let Some(relative_end) = source[start..].find(&end_marker) else {
            break;
        };
        let end = start + relative_end;
        ranges.push((start, end));
        cursor = end + end_marker.len();
    }
    ranges
}

fn font_face_blocks(source: &str) -> Vec<&str> {
    let bytes = source.as_bytes();
    let mut blocks = Vec::new();
    let mut cursor = 0usize;
    while let Some(start) = find_token_outside_css(source, cursor, "@font-face") {
        let Some(open) = find_char_outside_css(source, start + "@font-face".len(), b'{') else {
            break;
        };
        let mut state = CssState::default();
        let mut depth = 1usize;
        let mut index = open + 1;
        while index < bytes.len() {
            let byte = bytes[index];
            state.advance(bytes, &mut index);
            if state.literal() {
                continue;
            }
            if byte == b'{' {
                depth += 1;
            } else if byte == b'}' {
                depth -= 1;
                if depth == 0 {
                    blocks.push(&source[open + 1..index]);
                    cursor = index + 1;
                    break;
                }
            }
            index += 1;
        }
        if depth != 0 {
            break;
        }
    }
    blocks
}

fn parse_declarations(block: &str) -> BTreeMap<String, String> {
    let bytes = block.as_bytes();
    let mut declarations = BTreeMap::new();
    let mut state = CssState::default();
    let mut depth = 0usize;
    let mut start = 0usize;
    let mut index = 0usize;
    while index <= bytes.len() {
        if index == bytes.len() || (bytes[index] == b';' && !state.literal() && depth == 0) {
            let declaration = &block[start..index];
            if let Some((property, value)) = declaration.split_once(':') {
                let property = property.trim().to_ascii_lowercase();
                if !property.is_empty() {
                    declarations.insert(property, value.trim().to_string());
                }
            }
            start = index.saturating_add(1);
            index += 1;
            continue;
        }
        let byte = bytes[index];
        state.advance(bytes, &mut index);
        if !state.literal() {
            match byte {
                b'(' => depth += 1,
                b')' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        index += 1;
    }
    declarations
}

#[derive(Default)]
struct CssState {
    quote: Option<u8>,
    escaped: bool,
    comment: bool,
}

impl CssState {
    fn literal(&self) -> bool {
        self.quote.is_some() || self.comment
    }

    fn advance(&mut self, bytes: &[u8], index: &mut usize) {
        let byte = bytes[*index];
        if self.comment {
            if byte == b'*' && bytes.get(*index + 1) == Some(&b'/') {
                self.comment = false;
                *index += 1;
            }
            return;
        }
        if let Some(quote) = self.quote {
            if self.escaped {
                self.escaped = false;
            } else if byte == b'\\' {
                self.escaped = true;
            } else if byte == quote {
                self.quote = None;
            }
            return;
        }
        if byte == b'/' && bytes.get(*index + 1) == Some(&b'*') {
            self.comment = true;
            *index += 1;
        } else if matches!(byte, b'\'' | b'"') {
            self.quote = Some(byte);
        }
    }
}

fn find_token_outside_css(source: &str, from: usize, needle: &str) -> Option<usize> {
    let bytes = source.as_bytes();
    let needle_bytes = needle.as_bytes();
    let mut state = CssState::default();
    let mut index = from;
    while index + needle_bytes.len() <= bytes.len() {
        if !state.literal()
            && bytes[index..index + needle_bytes.len()].eq_ignore_ascii_case(needle_bytes)
        {
            return Some(index);
        }
        state.advance(bytes, &mut index);
        index += 1;
    }
    None
}

fn find_char_outside_css(source: &str, from: usize, target: u8) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut state = CssState::default();
    let mut index = from;
    while index < bytes.len() {
        if !state.literal() && bytes[index] == target {
            return Some(index);
        }
        state.advance(bytes, &mut index);
        index += 1;
    }
    None
}

fn extract_css_urls(value: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let mut cursor = 0usize;
    while let Some(start) = find_token_outside_css(value, cursor, "url") {
        let mut open = start + 3;
        while value
            .as_bytes()
            .get(open)
            .is_some_and(u8::is_ascii_whitespace)
        {
            open += 1;
        }
        if value.as_bytes().get(open) != Some(&b'(') {
            cursor = open;
            continue;
        }
        let body_start = open + 1;
        let Some(close) = find_closing_parenthesis(value, body_start) else {
            break;
        };
        let url = unquote(value[body_start..close].trim());
        if !url.is_empty() {
            urls.push(url);
        }
        cursor = close + 1;
    }
    urls
}

fn find_closing_parenthesis(value: &str, from: usize) -> Option<usize> {
    let bytes = value.as_bytes();
    let mut quote = None;
    let mut escaped = false;
    for (offset, byte) in bytes[from..].iter().copied().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        if byte == b'\\' {
            escaped = true;
            continue;
        }
        if matches!(byte, b'\'' | b'"') {
            if quote == Some(byte) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(byte);
            }
            continue;
        }
        if byte == b')' && quote.is_none() {
            return Some(from + offset);
        }
    }
    None
}

fn contains_css_function(value: &str, name: &str) -> bool {
    find_token_outside_css(value, 0, name).is_some_and(|start| {
        value.as_bytes()[start + name.len()..]
            .iter()
            .copied()
            .find(|byte| !byte.is_ascii_whitespace())
            == Some(b'(')
    })
}

enum ClassifiedUrl {
    Local(String),
    External,
    Dynamic,
}

fn classify_font_url(url: &str, stylesheet: &str) -> ClassifiedUrl {
    let trimmed = url.trim();
    if trimmed.contains("#{")
        || trimmed.contains("{{")
        || trimmed.contains("{%")
        || trimmed.contains('$')
    {
        return ClassifiedUrl::Dynamic;
    }
    let without_suffix = trimmed.split(['?', '#']).next().unwrap_or(trimmed);
    let Ok(decoded) = percent_decode_str(without_suffix).decode_utf8() else {
        return ClassifiedUrl::Dynamic;
    };
    let normalized = decoded.replace('\\', "/");
    let lower = normalized.to_ascii_lowercase();
    if lower.starts_with("data:")
        || lower.starts_with("blob:")
        || lower.starts_with("http:")
        || lower.starts_with("https:")
        || normalized.starts_with("//")
        || has_uri_scheme(&normalized)
    {
        return ClassifiedUrl::External;
    }
    normalize_logical_url(&normalized, stylesheet)
        .map(ClassifiedUrl::Local)
        .unwrap_or(ClassifiedUrl::Dynamic)
}

fn normalize_logical_url(url: &str, stylesheet: &str) -> Option<String> {
    let rooted = url.starts_with('/');
    let mut components = if rooted {
        Vec::new()
    } else {
        stylesheet_public_parent(stylesheet)
            .unwrap_or_default()
            .split('/')
            .filter(|segment| !segment.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    let value = url.trim_start_matches('/');
    let value = value.strip_prefix("static/").unwrap_or(value);
    for component in Path::new(value).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                components.pop()?;
            }
            Component::Normal(segment) => components.push(segment.to_string_lossy().into_owned()),
            _ => return None,
        }
    }
    (!components.is_empty()).then(|| components.join("/"))
}

fn stylesheet_public_parent(path: &str) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let logical = if let Some(path) = normalized.strip_prefix("static/") {
        path.to_string()
    } else if let Some((_, path)) = normalized.split_once("/static/") {
        path.to_string()
    } else if let Some(path) = normalized.strip_prefix("sass/") {
        path.to_string()
    } else if let Some((_, path)) = normalized.split_once("/sass/") {
        path.to_string()
    } else {
        return None;
    };
    logical
        .rsplit_once('/')
        .map(|(parent, _)| parent.to_string())
}

fn font_face_source(
    face: &ParsedFace,
    url: &str,
    resolved_file: Option<String>,
    delivery: FontDeliveryKind,
    dynamic: bool,
) -> FontFaceSource {
    FontFaceSource {
        stylesheet: face.stylesheet.clone(),
        url: url.to_string(),
        resolved_file,
        delivery,
        ownership: if face.managed {
            FontOwnership::Managed
        } else {
            FontOwnership::Detected
        },
        external: delivery == FontDeliveryKind::External,
        dynamic,
        weight: face.weight,
        weight_range: face.weight_range,
        style: face.style.clone(),
        display: face.display.clone(),
        unicode_range: face.unicode_range.clone(),
        managed: face.managed,
    }
}

fn issue(
    code: &str,
    severity: FontFaceIssueSeverity,
    message: String,
    file: Option<String>,
    stylesheet: Option<String>,
) -> FontFaceIssue {
    FontFaceIssue {
        code: code.to_string(),
        severity,
        message,
        file,
        stylesheet,
    }
}

fn asset_order(left: &FontAsset, right: &FontAsset) -> std::cmp::Ordering {
    origin_rank(&left.origin)
        .cmp(&origin_rank(&right.origin))
        .then_with(|| left.file.file.cmp(&right.file.file))
}

fn origin_rank(origin: &FontOrigin) -> u8 {
    match origin {
        FontOrigin::Bundled => 0,
        FontOrigin::Local => 0,
        FontOrigin::Theme => 1,
        FontOrigin::External => 2,
    }
}

fn origin_key(origin: &FontOrigin) -> &'static str {
    match origin {
        FontOrigin::Bundled => "bundled",
        FontOrigin::Local => "local",
        FontOrigin::Theme => "theme",
        FontOrigin::External => "external",
    }
}

fn public_logical_path(file: &str) -> String {
    let normalized = file.replace('\\', "/");
    normalized
        .strip_prefix("static/")
        .or_else(|| normalized.split_once("/static/").map(|(_, path)| path))
        .unwrap_or(&normalized)
        .trim_start_matches('/')
        .to_string()
}

fn asset_directory(file: &str) -> String {
    file.rsplit_once('/')
        .map(|(directory, _)| directory.to_string())
        .unwrap_or_default()
}

fn unquote(value: &str) -> String {
    value
        .trim()
        .trim_matches(|character| matches!(character, '\'' | '"'))
        .trim()
        .to_string()
}

fn has_uri_scheme(value: &str) -> bool {
    let Some((scheme, _)) = value.split_once(':') else {
        return false;
    };
    !scheme.is_empty()
        && scheme.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphabetic()
                || (index > 0 && (byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'.')))
        })
}

fn bounded_external_url(value: &str) -> String {
    if value.trim_start().to_ascii_lowercase().starts_with("data:") {
        return "data:…".to_string();
    }
    const MAX_CHARS: usize = 512;
    if value.chars().count() <= MAX_CHARS {
        return value.to_string();
    }
    value.chars().take(MAX_CHARS).collect::<String>() + "…"
}

fn is_public_output(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    normalized.starts_with("public/") || normalized.contains("/public/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        hint::black_box,
        path::PathBuf,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    fn fixture_root(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("pana-font-face-graph-{name}-{unique}"))
    }

    #[test]
    fn css_parser_ignores_comments_and_preserves_semicolons_inside_urls() {
        let source = r#"
          /* @font-face { font-family: 'False'; src: url('/x.woff2'); } */
          @font-face {
            font-family: 'Primary';
            src: url("data:font/woff2;base64,abc;def") format('woff2'),
                 url('/fonturi/inter.woff2?v=1#face') format('woff2');
            font-weight: 100 900;
            font-style: normal;
            font-display: swap;
          }
        "#;
        let faces = parse_font_faces("sass/site.scss", source);
        assert_eq!(faces.len(), 1);
        assert_eq!(faces[0].family, "Primary");
        assert_eq!(faces[0].urls.len(), 2);
        assert_eq!(
            faces[0].weight_range,
            Some(FontWeightRange {
                start: 100,
                end: 900
            })
        );
    }

    #[test]
    fn ownership_is_bound_to_the_exact_managed_marker_range() {
        let source = r#"
          /* pana-studio-font:primary:start */
          @font-face { font-family: 'Primary'; src: url('/fonturi/managed.woff2'); }
          /* pana-studio-font:primary:end */
          @font-face { font-family: 'Primary'; src: url('/fonturi/detected.woff2'); }
        "#;
        let faces = parse_font_faces("sass/site.scss", source);
        assert_eq!(faces.len(), 2);
        assert!(faces[0].managed);
        assert!(!faces[1].managed);
    }

    #[test]
    fn canonical_css_identity_preserves_meaningful_punctuation() {
        let css = r#"
          @font-face { font-family: 'AB'; src: url('https://cdn.example/ab.woff2'); }
          @font-face { font-family: 'A-B'; src: url('https://cdn.example/a-b.woff2'); }
        "#;
        let graph = build_font_face_graph(
            Path::new("/path/that/does/not/exist"),
            [("sass/site.scss", css)].into_iter(),
            std::iter::empty(),
            std::iter::empty(),
            std::iter::empty(),
        );
        assert!(graph.families.iter().any(|family| family.id == "css:ab"));
        assert!(graph.families.iter().any(|family| family.id == "css:a-b"));
    }

    #[test]
    fn normalizes_root_relative_percent_encoded_and_relative_urls() {
        assert!(matches!(
            classify_font_url("/fonturi/Inter%20Var.woff2?v=1#x", "sass/site.scss"),
            ClassifiedUrl::Local(path) if path == "fonturi/Inter Var.woff2"
        ));
        assert!(matches!(
            classify_font_url("../fonturi/inter.woff2", "static/css/site.css"),
            ClassifiedUrl::Local(path) if path == "fonturi/inter.woff2"
        ));
        assert!(matches!(
            classify_font_url("https://fonts.example/inter.woff2", "sass/site.scss"),
            ClassifiedUrl::External
        ));
        assert!(matches!(
            classify_font_url("#{$font-path}/inter.woff2", "sass/site.scss"),
            ClassifiedUrl::Dynamic
        ));
    }

    #[test]
    fn two_css_families_in_one_directory_are_not_merged() {
        let root = fixture_root("aliases");
        fs::create_dir_all(root.join("static/fonturi")).expect("font dir");
        let inter = include_bytes!(
            "../../resources/project-starters/cadru/project/static/fonturi/inter-400-700-latin-ext.woff2"
        );
        let poppins = include_bytes!(
            "../../resources/project-starters/cadru/project/static/fonturi/poppins-600-latin-ext.woff2"
        );
        fs::write(root.join("static/fonturi/inter.woff2"), inter).expect("inter");
        fs::write(root.join("static/fonturi/poppins.woff2"), poppins).expect("poppins");
        let css = r#"
          @font-face { font-family: 'Primary'; src: url('/fonturi/inter.woff2'); font-weight: 100 900; font-display: swap; }
          @font-face { font-family: 'Display'; src: url('/fonturi/poppins.woff2'); font-weight: 600; font-display: swap; }
        "#;
        let graph = build_font_face_graph(
            &root,
            [("sass/site.scss", css)].into_iter(),
            std::iter::empty(),
            std::iter::empty(),
            std::iter::empty(),
        );
        assert_eq!(graph.families.len(), 2);
        assert_eq!(graph.families[0].family, "Display");
        assert_eq!(graph.families[1].family, "Primary");
        assert!(graph
            .families
            .iter()
            .all(|family| family.registration.registered));
        assert!(graph.families.iter().all(|family| family.files.len() == 1));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn one_css_family_can_resolve_files_from_multiple_directories() {
        let root = fixture_root("multi-directory");
        fs::create_dir_all(root.join("static/fonturi/regular")).expect("regular dir");
        fs::create_dir_all(root.join("static/fonturi/bold")).expect("bold dir");
        let regular = include_bytes!(
            "../../resources/project-starters/cadru/project/static/fonturi/poppins-600-latin-ext.woff2"
        );
        let bold = include_bytes!(
            "../../resources/project-starters/cadru/project/static/fonturi/poppins-700-latin-ext.woff2"
        );
        fs::write(root.join("static/fonturi/regular/display.woff2"), regular).expect("regular");
        fs::write(root.join("static/fonturi/bold/display.woff2"), bold).expect("bold");
        let css = r#"
          @font-face { font-family: 'Display'; src: url('/fonturi/regular/display.woff2'); font-weight: 600; font-display: swap; }
          @font-face { font-family: 'Display'; src: url('/fonturi/bold/display.woff2'); font-weight: 700; font-display: swap; }
        "#;
        let graph = build_font_face_graph(
            &root,
            [("sass/site.scss", css)].into_iter(),
            std::iter::empty(),
            std::iter::empty(),
            std::iter::empty(),
        );
        assert_eq!(graph.families.len(), 1);
        assert_eq!(graph.families[0].id, "css:display");
        assert_eq!(graph.families[0].files.len(), 2);
        assert_eq!(graph.families[0].directories.len(), 2);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn local_asset_overrides_the_active_theme_at_the_same_public_url() {
        let root = fixture_root("theme-override");
        fs::create_dir_all(root.join("static/fonturi")).expect("local dir");
        fs::create_dir_all(root.join("themes/demo/static/fonturi")).expect("theme dir");
        fs::write(root.join("config.toml"), "theme = 'demo'\n").expect("config");
        fs::write(
            root.join("static/fonturi/primary.woff2"),
            include_bytes!(
                "../../resources/project-starters/cadru/project/static/fonturi/inter-400-700-latin-ext.woff2"
            ),
        )
        .expect("local font");
        fs::write(
            root.join("themes/demo/static/fonturi/primary.woff2"),
            include_bytes!(
                "../../resources/project-starters/cadru/project/static/fonturi/poppins-600-latin-ext.woff2"
            ),
        )
        .expect("theme font");
        let css = "@font-face { font-family: 'Primary'; src: url('/fonturi/primary.woff2'); font-weight: 400 700; font-display: swap; }";
        let graph = build_font_face_graph(
            &root,
            [("sass/site.scss", css)].into_iter(),
            std::iter::empty(),
            std::iter::empty(),
            std::iter::empty(),
        );
        let primary = graph
            .families
            .iter()
            .find(|family| family.id == "css:primary")
            .expect("Primary");
        assert_eq!(primary.origin, FontOrigin::Local);
        assert_eq!(primary.files.len(), 1);
        assert_eq!(primary.files[0].file, "static/fonturi/primary.woff2");
        assert_eq!(primary.files[0].internal_family.as_deref(), Some("Inter"));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn variable_and_italic_descriptors_are_validated_against_opentype() {
        let inter = include_bytes!(
            "../../resources/project-starters/cadru/project/static/fonturi/inter-400-700-latin-ext.woff2"
        );
        let poppins = include_bytes!(
            "../../resources/project-starters/cadru/project/static/fonturi/poppins-600-latin-ext.woff2"
        );
        let css = r#"
          @font-face { font-family: 'Primary'; src: url('/fonturi/inter.woff2'); font-weight: 100 900; font-style: normal; font-display: swap; }
          @font-face { font-family: 'Display Italic'; src: url('/fonturi/poppins.woff2'); font-weight: 600; font-style: italic; font-display: swap; }
        "#;
        let graph = build_font_face_graph(
            Path::new("/path/that/does/not/exist"),
            [("sass/site.scss", css)].into_iter(),
            [
                ("static/fonturi/inter.woff2", inter.as_slice()),
                ("static/fonturi/poppins.woff2", poppins.as_slice()),
            ]
            .into_iter(),
            std::iter::empty(),
            std::iter::empty(),
        );
        let primary = graph
            .families
            .iter()
            .find(|family| family.id == "css:primary")
            .expect("variable Primary");
        assert_eq!(
            primary.files[0].declared_weight_range,
            Some(FontWeightRange {
                start: 100,
                end: 900
            })
        );
        assert!(primary.files[0].axes.iter().any(|axis| axis.tag == "wght"));
        let italic = graph
            .families
            .iter()
            .find(|family| family.id == "css:display italic")
            .expect("italic alias");
        assert!(italic
            .issues
            .iter()
            .any(|issue| issue.code == "font_style_mismatch"));
    }

    #[test]
    fn duplicate_binary_content_is_reported_across_distinct_paths() {
        let inter = include_bytes!(
            "../../resources/project-starters/cadru/project/static/fonturi/inter-400-700-latin-ext.woff2"
        );
        let css = r#"
          @font-face { font-family: 'Primary'; src: url('/fonturi/regular.woff2'); font-weight: 400; }
          @font-face { font-family: 'Primary'; src: url('/fonturi/bold.woff2'); font-weight: 700; }
        "#;
        let graph = build_font_face_graph(
            Path::new("/path/that/does/not/exist"),
            [("sass/site.scss", css)].into_iter(),
            [
                ("static/fonturi/regular.woff2", inter.as_slice()),
                ("static/fonturi/bold.woff2", inter.as_slice()),
            ]
            .into_iter(),
            std::iter::empty(),
            std::iter::empty(),
        );
        assert!(graph.families[0]
            .issues
            .iter()
            .any(|issue| issue.code == "font_binary_duplicate"));
    }

    #[test]
    fn system_external_dynamic_and_missing_sources_have_distinct_delivery() {
        let css = r#"
          @font-face { font-family: 'System Face'; src: local('Arial'); font-display: swap; }
          @font-face { font-family: 'Remote Face'; src: url('https://cdn.example/font.woff2'); font-display: swap; }
          @font-face { font-family: 'Dynamic Face'; src: url('#{$font-path}/font.woff2'); font-display: swap; }
          @font-face { font-family: 'Missing Face'; font-display: swap; }
        "#;
        let graph = build_font_face_graph(
            Path::new("/path/that/does/not/exist"),
            [("sass/site.scss", css)].into_iter(),
            std::iter::empty(),
            std::iter::empty(),
            std::iter::empty(),
        );
        let delivery = graph
            .families
            .iter()
            .map(|family| (family.family.as_str(), family.delivery))
            .collect::<HashMap<_, _>>();
        assert_eq!(delivery["System Face"], FontDeliveryKind::System);
        assert_eq!(delivery["Remote Face"], FontDeliveryKind::External);
        assert_eq!(delivery["Dynamic Face"], FontDeliveryKind::Missing);
        assert_eq!(delivery["Missing Face"], FontDeliveryKind::Missing);
    }

    #[test]
    fn unresolved_fallback_does_not_hide_a_viable_face_source() {
        let inter = include_bytes!(
            "../../resources/project-starters/cadru/project/static/fonturi/inter-400-700-latin-ext.woff2"
        );
        let css = "@font-face { font-family: 'Primary'; src: url('/fonturi/missing.woff2') format('woff2'), url('/fonturi/inter.woff2') format('woff2'); font-display: swap; }";
        let graph = build_font_face_graph(
            Path::new("/path/that/does/not/exist"),
            [("sass/site.scss", css)].into_iter(),
            [("static/fonturi/inter.woff2", inter.as_slice())].into_iter(),
            std::iter::empty(),
            std::iter::empty(),
        );
        assert_eq!(graph.families[0].delivery, FontDeliveryKind::Local);
        assert!(graph.families[0]
            .issues
            .iter()
            .any(|issue| issue.code == "font_face_src_unresolved"));
    }

    #[test]
    fn bundled_starter_font_contracts_match_opentype_and_cover_romanian() {
        let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/project-starters");
        for pack in ["cadru", "nord", "pana-studio", "radacini"] {
            let root = resources.join(pack).join("project");
            let stylesheet = "sass/css-framework/_baza.scss";
            let source = fs::read_to_string(root.join(stylesheet)).expect("starter stylesheet");
            let graph = build_font_face_graph(
                &root,
                [(stylesheet, source.as_str())].into_iter(),
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
            );
            assert!(
                graph.families.iter().all(|family| family
                    .issues
                    .iter()
                    .all(|issue| issue.severity == FontFaceIssueSeverity::Info)),
                "{pack} has a warning/error: {:?}",
                graph
                    .families
                    .iter()
                    .flat_map(|family| family.issues.iter())
                    .map(|issue| (&issue.code, &issue.message))
                    .collect::<Vec<_>>()
            );
            assert!(
                graph
                    .families
                    .iter()
                    .all(|family| family.romanian_supported == Some(true)),
                "{pack} contains a font without Romanian glyph coverage"
            );
        }
    }

    fn benchmark_graph(faces: usize, rounds: usize) -> Duration {
        let mut source = String::with_capacity(faces * 160);
        for index in 0..faces {
            source.push_str(&format!(
                "@font-face{{font-family:'Benchmark';src:url('https://cdn.example/{index}.woff2');font-weight:400;font-style:normal;font-display:swap}}"
            ));
        }
        let run = || {
            black_box(build_font_face_graph(
                Path::new("/path/that/does/not/exist"),
                [("sass/benchmark.scss", source.as_str())].into_iter(),
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
            ));
        };
        run();
        let mut samples = (0..rounds)
            .map(|_| {
                let started = Instant::now();
                run();
                started.elapsed()
            })
            .collect::<Vec<_>>();
        samples.sort_unstable();
        samples[samples.len() / 2]
    }

    #[test]
    #[ignore = "manual release performance budget"]
    fn benchmark_font_face_graph_100_and_1000_faces() {
        const BASELINE_1000: Duration = Duration::from_nanos(4_332_023);
        let hundred = benchmark_graph(100, 31);
        let thousand = benchmark_graph(1_000, 31);
        let normalized_ratio = thousand.as_secs_f64() / hundred.as_secs_f64() / 10.0;
        eprintln!(
            "FontFaceGraph release baseline: 100={hundred:?}, 1000={thousand:?}, normalized={normalized_ratio:.3}"
        );
        assert!(
            normalized_ratio <= 1.10,
            "indexing exceeded the 10% near-linear budget: {normalized_ratio:.3}"
        );
        assert!(
            thousand <= BASELINE_1000.mul_f64(1.10),
            "1000 faces regressed more than 10% from {BASELINE_1000:?}: {thousand:?}"
        );
    }

    fn collect_stylesheets(root: &Path, directory: &Path, output: &mut Vec<(String, String)>) {
        let Ok(entries) = fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().and_then(|name| name.to_str()) != Some("public") {
                    collect_stylesheets(root, &path, output);
                }
                continue;
            }
            if !is_stylesheet_path(&path.to_string_lossy()) {
                continue;
            }
            let Ok(source) = fs::read_to_string(&path) else {
                continue;
            };
            let relative = path
                .strip_prefix(root)
                .expect("project stylesheet")
                .to_string_lossy()
                .replace('\\', "/");
            output.push((relative, source));
        }
    }

    #[test]
    #[ignore = "manual 11111d acceptance evidence"]
    fn real_project_reports_css_aliases_without_false_missing_faces() {
        let root = PathBuf::from(
            std::env::var("PANA_FONT_GRAPH_PROJECT")
                .expect("set PANA_FONT_GRAPH_PROJECT to the 11111d Zola root"),
        );
        let mut documents = Vec::new();
        collect_stylesheets(&root, &root, &mut documents);
        let graph = build_font_face_graph(
            &root,
            documents
                .iter()
                .map(|(path, source)| (path.as_str(), source.as_str())),
            std::iter::empty(),
            std::iter::empty(),
            std::iter::empty(),
        );
        let primary = graph
            .families
            .iter()
            .find(|family| family.id == "css:primary")
            .expect("Primary CSS alias");
        let display = graph
            .families
            .iter()
            .find(|family| family.id == "css:display")
            .expect("Display CSS alias");
        assert_eq!(primary.delivery, FontDeliveryKind::Local);
        assert_eq!(display.delivery, FontDeliveryKind::Local);
        assert_eq!(primary.files.len(), 3);
        assert_eq!(display.files.len(), 2);
        assert!(primary
            .files
            .iter()
            .all(|file| file.internal_family.as_deref() == Some("Inter")));
        assert!(display.files.iter().all(|file| file
            .internal_family
            .as_deref()
            .is_some_and(|family| family.starts_with("Poppins"))));
        assert!(primary
            .registration
            .display_modes
            .contains(&"swap".to_string()));
        assert!(display
            .registration
            .display_modes
            .contains(&"swap".to_string()));
        assert!(graph.families.iter().all(|family| family
            .issues
            .iter()
            .all(|issue| issue.code != "font_face_src_unresolved")));
        assert!(primary
            .issues
            .iter()
            .any(|issue| issue.code == "font_binary_duplicate"));
        assert!(display
            .issues
            .iter()
            .any(|issue| issue.code == "font_weight_mismatch"));

        let roles = crate::fonts::read_font_roles(
            documents
                .iter()
                .map(|(path, source)| (path.as_str(), source.as_str())),
            &graph.families,
        );
        assert_eq!(
            roles[0].delivery,
            crate::fonts::roles::FontRoleDeliveryKind::Local
        );
        assert_eq!(
            roles[1].delivery,
            crate::fonts::roles::FontRoleDeliveryKind::Local
        );
        assert!(roles[2]
            .diagnostic
            .as_deref()
            .is_some_and(|message| message.contains("$font-ui")));
        let diagnostics = crate::fonts::font_delivery_diagnostics(&graph.families, &roles);
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "font_face_missing"));
    }
}
