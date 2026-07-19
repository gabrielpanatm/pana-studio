use std::{
    env, fs,
    path::{Path, PathBuf},
};

use tauri::{AppHandle, Manager, Runtime};

const BUNDLED_ZOLA_RELATIVE_PATHS: [&str; 2] = ["binaries/zola", "src-tauri/binaries/zola"];

pub fn resolve_zola_binary_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let mut candidates = Vec::new();

    if cfg!(debug_assertions) {
        // Bundled binary first — avoids system wrappers (e.g. Flatpak) that
        // run in a sandbox and cannot access arbitrary working directories.
        candidates.push(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("binaries")
                .join("zola"),
        );

        candidates.push(PathBuf::from("/usr/bin/zola"));

        if let Some(path_binary) = find_zola_in_path() {
            candidates.push(path_binary);
        }
    } else {
        if let Ok(resource_dir) = app.path().resource_dir() {
            for relative_path in BUNDLED_ZOLA_RELATIVE_PATHS {
                candidates.push(resource_dir.join(relative_path));
            }
        }

        candidates.push(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("binaries")
                .join("zola"),
        );
    }

    let resolved = candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| {
            "Nu am gasit binarul Zola. Lipseste resursa bundled `src-tauri/binaries/zola`."
                .to_string()
        })?;

    require_executable(&resolved)?;
    Ok(resolved)
}

fn find_zola_in_path() -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|directory| directory.join("zola"))
        .find(|candidate| candidate.is_file())
}

fn require_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let metadata = fs::metadata(path).map_err(|error| {
            format!(
                "Nu am putut citi permisiunile pentru {}: {}",
                path.to_string_lossy(),
                error
            )
        })?;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(format!(
                "Binarul Zola {} nu are bit executabil. Resursa bundled trebuie livrată cu modul corect; Pană Studio nu modifică permisiunile binarelor de sistem sau PATH la runtime.",
                path.to_string_lossy()
            ));
        }
    }

    Ok(())
}
