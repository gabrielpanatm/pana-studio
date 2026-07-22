use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::Duration,
};
use tokio_util::sync::CancellationToken;

use super::artifact::resolve_artifact_root;

pub fn run_zola_build_cancellable(
    binary: &Path,
    project_root: &Path,
    zola_root: &Path,
    cancellation_token: &CancellationToken,
) -> Result<String, String> {
    // This containment check intentionally runs before the subprocess. Zola
    // must never receive an output_dir that escapes the active ProjectRoot.
    resolve_artifact_root(project_root, zola_root)?;
    let mut child = Command::new(binary)
        .arg("build")
        .current_dir(zola_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Nu am putut porni zola build: {}", e))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Zola build nu a expus stdout pentru captură.".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Zola build nu a expus stderr pentru captură.".to_string())?;
    let stdout_reader = thread::spawn(move || read_stream(stdout));
    let stderr_reader = thread::spawn(move || read_stream(stderr));
    let status = loop {
        if cancellation_token.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(
                "[publish_cancelled] Build-ul Zola a fost anulat de utilizator.".to_string(),
            );
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(25)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(format!("Nu am putut urmări procesul zola build: {error}"));
            }
        }
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| "Thread-ul stdout pentru zola build a eșuat.".to_string())??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "Thread-ul stderr pentru zola build a eșuat.".to_string())??;
    let log = command_log(&stdout, &stderr);

    if status.success() {
        Ok(format!("OK Build reusit\n{}", log))
    } else {
        Err(format!("Eroare build:\n{}", log))
    }
}

fn read_stream(mut stream: impl Read) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    stream
        .read_to_end(&mut output)
        .map_err(|error| format!("Nu am putut citi output-ul procesului Zola: {error}"))?;
    Ok(output)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    #[cfg(unix)]
    #[test]
    fn cancelled_build_terminates_the_zola_process_promptly() {
        use std::os::unix::fs::PermissionsExt;

        let root = unique_temp_dir("cancel-build");
        let zola_root = root.join("sursa");
        fs::create_dir_all(&zola_root).unwrap();
        fs::write(
            zola_root.join("config.toml"),
            "base_url = '/'\noutput_dir = '../export'\n",
        )
        .unwrap();
        let fake_zola = root.join("fake-zola");
        fs::write(&fake_zola, "#!/bin/sh\nexec sleep 30\n").unwrap();
        let mut permissions = fs::metadata(&fake_zola).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&fake_zola, permissions).unwrap();

        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let started = Instant::now();
        let error =
            run_zola_build_cancellable(&fake_zola, &root, &zola_root, &cancellation).unwrap_err();

        assert!(error.contains("[publish_cancelled]"));
        assert!(started.elapsed() < Duration::from_secs(2));
        cleanup(root);
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "panastudio-zola-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn cleanup(path: PathBuf) {
        let _ = fs::remove_dir_all(path);
    }
}
