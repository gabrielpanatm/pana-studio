#[path = "src/tauri_command_registry.rs"]
mod tauri_command_registry;

use std::{env, fs, path::Path};

use tauri_command_registry::{render_app_default_permission_toml, APP_COMMAND_NAMES};

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

    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(app_manifest))
        .expect("failed to build Pană Studio Tauri ACL manifest");
}
