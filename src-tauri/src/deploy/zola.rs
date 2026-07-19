use std::{
    path::Path,
    process::{Command, Stdio},
};

use super::artifact::resolve_artifact_root;

pub fn run_zola_build(
    binary: &Path,
    project_root: &Path,
    zola_root: &Path,
) -> Result<String, String> {
    // This containment check intentionally runs before the subprocess. Zola
    // must never receive an output_dir that escapes the active ProjectRoot.
    resolve_artifact_root(project_root, zola_root)?;
    let output = Command::new(binary)
        .arg("build")
        .current_dir(zola_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Nu am putut porni zola build: {}", e))?;

    let log = command_log(&output.stdout, &output.stderr);

    if output.status.success() {
        Ok(format!("OK Build reusit\n{}", log))
    } else {
        Err(format!("Eroare build:\n{}", log))
    }
}

pub fn run_zola_check(
    binary: &Path,
    project_root: &Path,
    zola_root: &Path,
) -> Result<String, String> {
    resolve_artifact_root(project_root, zola_root)?;
    let output = Command::new(binary)
        .arg("check")
        .current_dir(zola_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Nu am putut porni zola check: {}", e))?;

    let log = command_log(&output.stdout, &output.stderr);

    if output.status.success() {
        Ok(format!("OK Zola check reușit\n{}", log))
    } else {
        Err(format!("Eroare zola check:\n{}", log))
    }
}

fn command_log(stdout: &[u8], stderr: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(stdout).to_string();
    let stderr = String::from_utf8_lossy(stderr).to_string();
    format!("{}{}", stdout, stderr).trim().to_string()
}
