#[path = "src/tauri_command_registry.rs"]
mod tauri_command_registry;

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use tauri_command_registry::{render_app_default_permission_toml, APP_COMMAND_NAMES};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocaleManifest {
    locale: String,
    native_name: String,
    direction: String,
    contributors: Vec<String>,
}

fn write_if_changed(path: &Path, content: &str) {
    if matches!(fs::read_to_string(path), Ok(existing) if existing == content) {
        return;
    }

    fs::write(path, content).unwrap_or_else(|error| {
        panic!(
            "failed to write generated Tauri application permission {}: {error}",
            path.display()
        )
    });
}

fn main() {
    println!("cargo:rerun-if-changed=src/tauri_command_registry.rs");

    let out_dir = env::var_os("OUT_DIR").expect("Cargo did not provide OUT_DIR");
    let app_default_permission =
        Path::new(&out_dir).join("pana-studio-app-default-permission.toml");
    write_if_changed(
        &app_default_permission,
        &render_app_default_permission_toml(),
    );

    let permission_pattern: &'static str = Box::leak(
        app_default_permission
            .to_string_lossy()
            .into_owned()
            .into_boxed_str(),
    );
    let app_manifest = tauri_build::AppManifest::new()
        .commands(APP_COMMAND_NAMES)
        .permissions_path_pattern(permission_pattern);

    generate_embedded_locale_catalog(Path::new(&out_dir).join("pana-studio-locales.rs"));

    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(app_manifest))
        .expect("failed to build Pană Studio Tauri ACL manifest");
}

fn generate_embedded_locale_catalog(output_path: PathBuf) {
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("Cargo did not provide CARGO_MANIFEST_DIR"),
    );
    let locales_root = manifest_dir.join("../locales");
    println!("cargo:rerun-if-changed={}", locales_root.display());
    let mut locale_directories = fs::read_dir(&locales_root)
        .unwrap_or_else(|error| {
            panic!(
                "failed to read localization root {}: {error}",
                locales_root.display()
            )
        })
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    locale_directories.sort();
    let mut generated = String::from("pub const EMBEDDED_LOCALES: &[EmbeddedLocale] = &[\n");
    for directory in locale_directories {
        let directory_name = directory
            .file_name()
            .and_then(|name| name.to_str())
            .expect("locale directory must be valid UTF-8");
        let manifest_path = directory.join("manifest.json");
        println!("cargo:rerun-if-changed={}", manifest_path.display());
        let manifest: LocaleManifest =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap_or_else(|error| {
                panic!("failed to read {}: {error}", manifest_path.display())
            }))
            .unwrap_or_else(|error| {
                panic!(
                    "invalid locale manifest {}: {error}",
                    manifest_path.display()
                )
            });
        assert_eq!(
            manifest.locale, directory_name,
            "locale manifest id must match its directory"
        );
        assert!(
            matches!(manifest.direction.as_str(), "ltr" | "rtl"),
            "locale direction must be ltr or rtl"
        );
        assert!(
            !manifest.contributors.is_empty(),
            "locale manifest must credit at least one contributor"
        );
        let mut resources = fs::read_dir(&directory)
            .expect("failed to read locale directory")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("ftl"))
            .collect::<Vec<_>>();
        resources.sort();
        generated.push_str("    EmbeddedLocale {\n");
        generated.push_str(&format!("        locale: {:?},\n", manifest.locale));
        generated.push_str(&format!(
            "        native_name: {:?},\n",
            manifest.native_name
        ));
        generated.push_str(&format!("        direction: {:?},\n", manifest.direction));
        generated.push_str("        contributors: &[\n");
        for contributor in manifest.contributors {
            generated.push_str(&format!("            {:?},\n", contributor));
        }
        generated.push_str("        ],\n        resources: &[\n");
        for resource in resources {
            println!("cargo:rerun-if-changed={}", resource.display());
            let file_name = resource
                .file_name()
                .and_then(|name| name.to_str())
                .expect("FTL resource name must be valid UTF-8");
            let domain = resource
                .file_stem()
                .and_then(|name| name.to_str())
                .expect("FTL domain must be valid UTF-8");
            generated.push_str("            EmbeddedFluentResource {\n");
            generated.push_str(&format!("                domain: {:?},\n", domain));
            generated.push_str(&format!(
                "                source: include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../locales/{}/{}\")),\n",
                directory_name, file_name
            ));
            generated.push_str("            },\n");
        }
        generated.push_str("        ],\n    },\n");
    }
    generated.push_str("];\n");
    write_if_changed(&output_path, &generated);
}
