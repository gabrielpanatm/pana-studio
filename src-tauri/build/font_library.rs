use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

use serde::Deserialize;
use sha2::{Digest, Sha256};

#[path = "../src/fonts/metadata.rs"]
#[allow(dead_code)]
mod font_metadata;

const FONT_LIBRARY_SCHEMA_VERSION: u32 = 1;
const FONT_LIBRARY_MAX_BYTES: u64 = 5_500 * 1_024;
const EXPECTED_FAMILY_IDS: &[&str] = &[
    "inter",
    "roboto",
    "open-sans",
    "source-sans-3",
    "manrope",
    "montserrat",
    "dm-sans",
    "figtree",
    "plus-jakarta-sans",
    "work-sans",
    "space-grotesk",
    "atkinson-hyperlegible-next",
    "public-sans",
    "nunito-sans",
    "ibm-plex-sans",
    "geist",
    "source-serif-4",
    "literata",
    "lora",
    "crimson-pro",
    "fraunces",
    "newsreader",
    "playfair-display",
    "merriweather",
    "libre-baskerville",
    "roboto-slab",
    "jetbrains-mono",
    "source-code-pro",
    "roboto-mono",
    "inconsolata",
    "geist-mono",
    "oswald",
    "raleway",
    "comfortaa",
    "unbounded",
    "caveat",
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FontLibraryManifest {
    schema_version: u32,
    provider: String,
    catalog_url: String,
    retrieved_at: String,
    families: Vec<FontLibraryFamily>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FontLibraryFamily {
    id: String,
    family: String,
    category: String,
    last_modified: String,
    specimen_url: String,
    css_url: String,
    license: FontLibraryLicense,
    files: Vec<FontLibraryFile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FontLibraryLicense {
    identifier: String,
    file: String,
    source_url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FontLibraryFile {
    file: String,
    subset: String,
    unicode_range: String,
    source_url: String,
    sha256: String,
    size_bytes: u64,
}

struct ValidatedFontFile {
    manifest: FontLibraryFile,
    file_name: String,
    style: String,
    weight_start: u16,
    weight_end: u16,
    romanian_glyphs: Vec<char>,
}

pub fn generate(manifest_dir: &Path, output_path: PathBuf) {
    let root = manifest_dir.join("resources/font-library");
    let catalog_path = root.join("catalog.json");
    println!("cargo:rerun-if-changed={}", catalog_path.display());
    let catalog_bytes = fs::read(&catalog_path).unwrap_or_else(|error| {
        panic!(
            "failed to read embedded font catalog {}: {error}",
            catalog_path.display()
        )
    });
    let manifest: FontLibraryManifest = serde_json::from_slice(&catalog_bytes)
        .unwrap_or_else(|error| panic!("embedded font catalog is invalid JSON: {error}"));
    validate_manifest_header(&manifest);

    let mut total_bytes = catalog_bytes.len() as u64;
    let mut expected_files = BTreeSet::from(["catalog.json".to_string()]);
    let mut ids = BTreeSet::new();
    let mut generated =
        String::from("pub const EMBEDDED_FONT_LIBRARY: &[EmbeddedFontFamily] = &[\n");

    for mut family in manifest.families {
        assert!(
            ids.insert(family.id.clone()),
            "duplicate embedded font ID {}",
            family.id
        );
        validate_family_header(&family);
        let license_path = resolve_manifest_path(&root, &family.license.file, &family.id);
        assert!(
            expected_files.insert(family.license.file.clone()),
            "duplicate embedded font resource {}",
            family.license.file
        );
        println!("cargo:rerun-if-changed={}", license_path.display());
        let license_text = fs::read_to_string(&license_path).unwrap_or_else(|error| {
            panic!(
                "failed to read embedded font license {}: {error}",
                license_path.display()
            )
        });
        assert!(
            !license_text.trim().is_empty(),
            "embedded font {} has an empty license",
            family.family
        );
        total_bytes += license_text.len() as u64;

        let mut validated = Vec::new();
        let mut style_subsets = BTreeMap::<String, BTreeSet<String>>::new();
        let mut style_glyphs = BTreeMap::<String, BTreeSet<char>>::new();
        for file in std::mem::take(&mut family.files) {
            assert!(
                expected_files.insert(file.file.clone()),
                "duplicate embedded font resource {}",
                file.file
            );
            let absolute_path = resolve_manifest_path(&root, &file.file, &family.id);
            println!("cargo:rerun-if-changed={}", absolute_path.display());
            let bytes = fs::read(&absolute_path).unwrap_or_else(|error| {
                panic!(
                    "failed to read embedded font {}: {error}",
                    absolute_path.display()
                )
            });
            assert_eq!(
                bytes.len() as u64,
                file.size_bytes,
                "embedded font size drifted for {}",
                file.file
            );
            assert_eq!(
                sha256_hex(&bytes),
                file.sha256,
                "embedded font hash drifted for {}",
                file.file
            );
            font_metadata::validate_font_signature(&bytes, "woff2", &file.file)
                .unwrap_or_else(|error| panic!("{error}"));
            let metadata = font_metadata::parse_font_metadata(&bytes)
                .unwrap_or_else(|error| panic!("invalid embedded font {}: {error}", file.file));
            let internal_family = normalize_name(&metadata.family);
            let catalog_family = normalize_name(&family.family);
            assert!(
                internal_family == catalog_family
                    || internal_family.starts_with(&(catalog_family.clone() + " ")),
                "embedded font internal family mismatch for {}: {:?} versus {:?}",
                file.file,
                metadata.family,
                family.family
            );
            let range = metadata
                .weight_range
                .unwrap_or_else(|| panic!("embedded font {} is not variable on wght", file.file));
            assert!(
                metadata.axes.iter().any(|axis| axis.tag == "wght"),
                "embedded font {} is missing the wght axis",
                file.file
            );
            assert!(
                matches!(file.subset.as_str(), "latin" | "latin-ext"),
                "embedded font {} has unsupported subset {}",
                file.file,
                file.subset
            );
            assert!(
                !file.unicode_range.trim().is_empty(),
                "embedded font {} has no unicode-range",
                file.file
            );
            assert_https(&file.source_url, "font source");
            style_subsets
                .entry(metadata.style.clone())
                .or_default()
                .insert(file.subset.clone());
            style_glyphs
                .entry(metadata.style.clone())
                .or_default()
                .extend(metadata.romanian_glyphs.iter().copied());
            total_bytes += bytes.len() as u64;
            let file_name = absolute_path
                .file_name()
                .and_then(|value| value.to_str())
                .expect("embedded font file name must be UTF-8")
                .to_string();
            validated.push(ValidatedFontFile {
                manifest: file,
                file_name,
                style: metadata.style,
                weight_start: range.start,
                weight_end: range.end,
                romanian_glyphs: metadata.romanian_glyphs,
            });
        }
        assert!(
            !validated.is_empty(),
            "embedded family {} has no files",
            family.family
        );
        for (style, subsets) in &style_subsets {
            assert_eq!(
                subsets,
                &["latin".to_string(), "latin-ext".to_string()]
                    .into_iter()
                    .collect(),
                "embedded family {} style {} must contain Latin and Latin Extended exactly once",
                family.family,
                style
            );
            let glyphs = &style_glyphs[style];
            assert!(
                font_metadata::ROMANIAN_GLYPHS
                    .iter()
                    .all(|glyph| glyphs.contains(glyph)),
                "embedded family {} style {} does not cover every Romanian glyph",
                family.family,
                style
            );
        }
        render_family(&mut generated, &family, &validated);
    }

    let expected = EXPECTED_FAMILY_IDS.iter().copied().collect::<BTreeSet<_>>();
    let actual = ids.iter().map(String::as_str).collect::<BTreeSet<_>>();
    assert_eq!(actual, expected, "embedded font family set drifted");
    let actual_files = collect_relative_files(&root);
    assert_eq!(
        actual_files, expected_files,
        "embedded font directory contains missing or unreferenced resources"
    );
    assert!(
        total_bytes <= FONT_LIBRARY_MAX_BYTES,
        "embedded font library uses {total_bytes} bytes, above the {FONT_LIBRARY_MAX_BYTES}-byte budget"
    );
    generated.push_str("];\n");
    super::write_if_changed(&output_path, &generated);
}

fn collect_relative_files(root: &Path) -> BTreeSet<String> {
    fn visit(root: &Path, directory: &Path, files: &mut BTreeSet<String>) {
        for entry in fs::read_dir(directory)
            .unwrap_or_else(|error| panic!("failed to scan {}: {error}", directory.display()))
        {
            let entry = entry.expect("embedded font directory entry is unreadable");
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .unwrap_or_else(|error| panic!("failed to inspect {}: {error}", path.display()));
            assert!(
                !metadata.file_type().is_symlink(),
                "embedded font library contains a symlink: {}",
                path.display()
            );
            if metadata.is_dir() {
                visit(root, &path, files);
            } else {
                assert!(
                    metadata.is_file(),
                    "embedded font library contains a non-file resource: {}",
                    path.display()
                );
                let relative = path
                    .strip_prefix(root)
                    .expect("embedded font resource escaped its root")
                    .to_string_lossy()
                    .replace('\\', "/");
                assert!(
                    files.insert(relative.clone()),
                    "duplicate resource {relative}"
                );
            }
        }
    }

    let mut files = BTreeSet::new();
    visit(root, root, &mut files);
    files
}

fn validate_manifest_header(manifest: &FontLibraryManifest) {
    assert_eq!(
        manifest.schema_version, FONT_LIBRARY_SCHEMA_VERSION,
        "unsupported embedded font catalog schema"
    );
    assert_eq!(manifest.provider, "Google Fonts");
    assert_https(&manifest.catalog_url, "catalog URL");
    assert_eq!(manifest.retrieved_at, "2026-08-14");
    assert_eq!(manifest.families.len(), EXPECTED_FAMILY_IDS.len());
}

fn validate_family_header(family: &FontLibraryFamily) {
    assert!(
        valid_id(&family.id),
        "invalid embedded font ID {}",
        family.id
    );
    assert!(
        !family.family.trim().is_empty(),
        "empty embedded font family name"
    );
    assert!(
        matches!(
            family.category.as_str(),
            "sans-serif" | "serif" | "slab-serif" | "monospace" | "display" | "handwriting"
        ),
        "unsupported embedded font category {}",
        family.category
    );
    assert!(!family.last_modified.trim().is_empty());
    assert_https(&family.specimen_url, "specimen URL");
    assert_https(&family.css_url, "CSS URL");
    assert!(!family.license.identifier.trim().is_empty());
    assert_https(&family.license.source_url, "license URL");
}

fn render_family(output: &mut String, family: &FontLibraryFamily, files: &[ValidatedFontFile]) {
    output.push_str("    EmbeddedFontFamily {\n");
    output.push_str(&format!("        id: {:?},\n", family.id));
    output.push_str(&format!("        family: {:?},\n", family.family));
    output.push_str(&format!("        category: {:?},\n", family.category));
    output.push_str(&format!(
        "        last_modified: {:?},\n",
        family.last_modified
    ));
    output.push_str(&format!(
        "        specimen_url: {:?},\n",
        family.specimen_url
    ));
    output.push_str(&format!("        css_url: {:?},\n", family.css_url));
    output.push_str("        license: EmbeddedFontLicense {\n");
    output.push_str(&format!(
        "            identifier: {:?},\n",
        family.license.identifier
    ));
    output.push_str(&format!(
        "            relative_path: {:?},\n",
        family.license.file
    ));
    output.push_str(&format!(
        "            source_url: {:?},\n",
        family.license.source_url
    ));
    output.push_str(&format!(
        "            text: include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/resources/font-library/{}\")),\n",
        family.license.file
    ));
    output.push_str("        },\n        files: &[\n");
    for file in files {
        output.push_str("            EmbeddedFontFile {\n");
        output.push_str(&format!(
            "                relative_path: {:?},\n",
            file.manifest.file
        ));
        output.push_str(&format!(
            "                file_name: {:?},\n",
            file.file_name
        ));
        output.push_str(&format!(
            "                subset: {:?},\n",
            file.manifest.subset
        ));
        output.push_str(&format!(
            "                unicode_range: {:?},\n",
            file.manifest.unicode_range
        ));
        output.push_str(&format!(
            "                source_url: {:?},\n",
            file.manifest.source_url
        ));
        output.push_str(&format!(
            "                sha256: {:?},\n",
            file.manifest.sha256
        ));
        output.push_str(&format!(
            "                size_bytes: {},\n",
            file.manifest.size_bytes
        ));
        output.push_str(&format!("                style: {:?},\n", file.style));
        output.push_str(&format!(
            "                weight_start: {},\n",
            file.weight_start
        ));
        output.push_str(&format!(
            "                weight_end: {},\n",
            file.weight_end
        ));
        output.push_str("                romanian_glyphs: &[");
        for glyph in &file.romanian_glyphs {
            output.push_str(&format!("{:?},", glyph));
        }
        output.push_str("],\n");
        output.push_str(&format!(
            "                bytes: include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/resources/font-library/{}\")),\n",
            file.manifest.file
        ));
        output.push_str("            },\n");
    }
    output.push_str("        ],\n    },\n");
}

fn resolve_manifest_path(root: &Path, relative: &str, family_id: &str) -> PathBuf {
    let path = Path::new(relative);
    assert!(
        !path.is_absolute(),
        "embedded font path must be relative: {relative}"
    );
    assert!(
        path.components()
            .all(|component| matches!(component, Component::Normal(_))),
        "embedded font path contains traversal: {relative}"
    );
    assert_eq!(
        path.components()
            .next()
            .and_then(|component| component.as_os_str().to_str()),
        Some(family_id),
        "embedded font path escaped family directory: {relative}"
    );
    let absolute = root.join(path);
    let metadata = fs::symlink_metadata(&absolute).unwrap_or_else(|error| {
        panic!(
            "failed to inspect embedded font resource {}: {error}",
            absolute.display()
        )
    });
    assert!(
        !metadata.file_type().is_symlink(),
        "embedded font resource is a symlink: {relative}"
    );
    assert!(
        metadata.is_file(),
        "embedded font resource is not a file: {relative}"
    );
    absolute
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('-')
        && !id.ends_with('-')
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn normalize_name(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn assert_https(value: &str, label: &str) {
    assert!(
        value.starts_with("https://"),
        "embedded font {label} must use HTTPS: {value}"
    );
}
