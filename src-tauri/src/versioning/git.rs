use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[cfg(target_os = "linux")]
use std::os::unix::process::CommandExt;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_OUTPUT_LIMIT: usize = 4 * 1024 * 1024;
const GLOBAL_CREDENTIAL_CONFIG_LIMIT: usize = 256 * 1024;
const MAX_GLOBAL_CREDENTIAL_HELPERS: usize = 64;
pub const NETWORK_TIMEOUT: Duration = Duration::from_secs(5 * 60);
pub const NETWORK_OUTPUT_LIMIT: usize = 8 * 1024 * 1024;
pub const NETWORK_CANCELLED_ERROR: &str = "Operația Git de rețea a fost anulată.";

pub(crate) type ProgressCallback = Arc<dyn Fn(&[u8]) + Send + Sync + 'static>;
type GitOutputReader = JoinHandle<Result<(Vec<u8>, bool), String>>;

#[derive(Debug)]
pub struct GitCommandOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

impl GitCommandOutput {
    pub fn success(&self) -> bool {
        self.status.success()
    }

    pub fn stdout_text(&self) -> Result<String, String> {
        String::from_utf8(self.stdout.clone())
            .map_err(|_| "Git a returnat stdout care nu este UTF-8.".to_string())
    }

    pub fn stderr_lossy(&self) -> String {
        let mut diagnostic = String::from_utf8_lossy(&self.stderr).trim().to_string();
        if self.stderr_truncated {
            if !diagnostic.is_empty() {
                diagnostic.push(' ');
            }
            diagnostic.push_str("[stderr Git trunchiat la limita de siguranță]");
        }
        diagnostic
    }

    pub fn require_success(self, operation: &str) -> Result<Self, String> {
        if self.success() {
            return Ok(self);
        }
        let diagnostic = self.stderr_lossy();
        Err(if diagnostic.is_empty() {
            format!("{operation} a eșuat cu statusul {}.", self.status)
        } else {
            format!("{operation} a eșuat: {diagnostic}")
        })
    }
}

pub struct GitRunner {
    cwd: PathBuf,
}

impl GitRunner {
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self { cwd: cwd.into() }
    }

    pub fn run<I, S>(&self, args: I) -> Result<GitCommandOutput, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        self.run_bounded(
            args,
            None,
            DEFAULT_TIMEOUT,
            DEFAULT_OUTPUT_LIMIT,
            false,
            false,
            &[],
            None,
            None,
        )
    }

    pub fn run_with_input<I, S>(&self, args: I, input: &[u8]) -> Result<GitCommandOutput, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        self.run_bounded(
            args,
            Some(input),
            DEFAULT_TIMEOUT,
            DEFAULT_OUTPUT_LIMIT,
            false,
            false,
            &[],
            None,
            None,
        )
    }

    pub fn run_with_limit<I, S>(
        &self,
        args: I,
        output_limit: usize,
    ) -> Result<GitCommandOutput, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        self.run_bounded(
            args,
            None,
            DEFAULT_TIMEOUT,
            output_limit,
            false,
            false,
            &[],
            None,
            None,
        )
    }

    pub(crate) fn spawn_network<I, S>(
        &self,
        args: I,
        cancellation: Arc<AtomicBool>,
        progress: ProgressCallback,
    ) -> Result<RunningGitCommand, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        require_not_cancelled(&cancellation)?;
        let credential_helpers = self.global_credential_helpers()?;
        require_not_cancelled(&cancellation)?;
        self.spawn_bounded(
            args,
            None,
            NETWORK_TIMEOUT,
            NETWORK_OUTPUT_LIMIT,
            true,
            false,
            &credential_helpers,
            Some(cancellation),
            Some(progress),
        )
    }

    fn global_credential_helpers(&self) -> Result<Vec<(String, String)>, String> {
        let output = self.run_bounded(
            [
                "config",
                "--global",
                "--null",
                "--get-regexp",
                r"^credential(\..*)?\.helper$",
            ],
            None,
            DEFAULT_TIMEOUT,
            GLOBAL_CREDENTIAL_CONFIG_LIMIT,
            false,
            true,
            &[],
            None,
            None,
        )?;
        if !output.success() {
            if output.status.code() == Some(1) {
                return Ok(Vec::new());
            }
            return Err(
                "Credential helper-ele globale Git nu au putut fi citite în siguranță.".to_string(),
            );
        }
        if output.stdout_truncated {
            return Err(
                "Configurația globală a credential helper-elor Git depășește limita sigură."
                    .to_string(),
            );
        }
        let mut helpers = Vec::new();
        for record in output.stdout.split(|byte| *byte == 0) {
            if record.is_empty() {
                continue;
            }
            let record = std::str::from_utf8(record).map_err(|_| {
                "Configurația globală a credential helper-elor Git nu este UTF-8.".to_string()
            })?;
            let (key, value) = record.split_once('\n').ok_or_else(|| {
                "Configurația globală a credential helper-elor Git are format invalid.".to_string()
            })?;
            let normalized_key = key.to_ascii_lowercase();
            if !normalized_key.starts_with("credential.") && normalized_key != "credential.helper" {
                return Err("Git a returnat o cheie credential helper neașteptată.".to_string());
            }
            if !normalized_key.ends_with(".helper")
                || key.contains(['\0', '\n', '\r', '='])
                || value.contains('\0')
            {
                return Err("Un credential helper global Git are format invalid.".to_string());
            }
            helpers.push((key.to_string(), value.to_string()));
            if helpers.len() > MAX_GLOBAL_CREDENTIAL_HELPERS {
                return Err(
                    "Sunt configurate prea multe credential helper-e globale Git.".to_string(),
                );
            }
        }
        Ok(helpers)
    }

    // Security-sensitive Git execution keeps every sandbox/config/output policy explicit.
    #[allow(clippy::too_many_arguments)]
    fn run_bounded<I, S>(
        &self,
        args: I,
        input: Option<&[u8]>,
        timeout: Duration,
        output_limit: usize,
        allow_user_config: bool,
        inspect_global_config: bool,
        credential_helpers: &[(String, String)],
        cancellation: Option<Arc<AtomicBool>>,
        progress: Option<ProgressCallback>,
    ) -> Result<GitCommandOutput, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        self.spawn_bounded(
            args,
            input,
            timeout,
            output_limit,
            allow_user_config,
            inspect_global_config,
            credential_helpers,
            cancellation,
            progress,
        )?
        .wait()
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_bounded<I, S>(
        &self,
        args: I,
        input: Option<&[u8]>,
        timeout: Duration,
        output_limit: usize,
        allow_user_config: bool,
        inspect_global_config: bool,
        credential_helpers: &[(String, String)],
        cancellation: Option<Arc<AtomicBool>>,
        progress: Option<ProgressCallback>,
    ) -> Result<RunningGitCommand, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        if let Some(cancellation) = cancellation.as_ref() {
            require_not_cancelled(cancellation)?;
        }
        let mut command = Command::new("git");
        command
            .arg("-c")
            .arg("core.hooksPath=/dev/null")
            .arg("-c")
            .arg("core.fsmonitor=false")
            .arg("-c")
            .arg("core.pager=cat")
            .arg("-c")
            .arg("core.attributesFile=/dev/null")
            .arg("-c")
            .arg("core.excludesFile=/dev/null")
            .arg("-c")
            .arg("merge.default=text")
            .current_dir(&self.cwd)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_PAGER", "cat")
            .env("PAGER", "cat")
            .env("LC_ALL", "C")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            });

        // Nu permitem mediului procesului gazdă să redirecționeze repository-ul,
        // indexul, transportul, helper-ele sau configurația procesului Git.
        for variable in [
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_COMMON_DIR",
            "GIT_INDEX_FILE",
            "GIT_OBJECT_DIRECTORY",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            "GIT_NAMESPACE",
            "GIT_REPLACE_REF_BASE",
            "GIT_SHALLOW_FILE",
            "GIT_CONFIG_GLOBAL",
            "GIT_CONFIG_COUNT",
            "GIT_CONFIG_KEY_0",
            "GIT_CONFIG_VALUE_0",
            "GIT_CONFIG_PARAMETERS",
            "GIT_ALLOW_PROTOCOL",
            "GIT_PROTOCOL_FROM_USER",
            "GIT_EXEC_PATH",
            "GIT_SSH",
            "GIT_SSH_COMMAND",
            "GIT_PROXY_COMMAND",
            "GIT_ASKPASS",
            "SSH_ASKPASS",
            "SSH_ASKPASS_REQUIRE",
        ] {
            command.env_remove(variable);
        }

        // Operațiile locale sunt complet izolate de configurația globală.
        // Pentru fetch/push importăm numai credential helper-ele din scope-ul
        // global; restul configurației globale și toate helper-ele locale sunt
        // neutralizate. Inspecția globală este separată și rulează cu --global.
        if !inspect_global_config {
            command.env("GIT_CONFIG_GLOBAL", "/dev/null");
        }
        if allow_user_config {
            command
                .arg("-c")
                .arg("protocol.allow=never")
                .arg("-c")
                .arg("protocol.https.allow=always")
                .arg("-c")
                .arg("protocol.ssh.allow=always")
                .arg("-c")
                .arg("protocol.git.allow=always")
                .arg("-c")
                .arg("http.followRedirects=initial")
                .arg("-c")
                .arg("http.sslVerify=true")
                .arg("-c")
                .arg("http.extraHeader=")
                .arg("-c")
                .arg("http.cookieFile=/dev/null")
                .arg("-c")
                .arg("http.saveCookies=false")
                .arg("-c")
                .arg("core.sshCommand=ssh")
                .arg("-c")
                .arg("core.gitProxy=none")
                .arg("-c")
                .arg("core.askPass=/bin/false")
                .arg("-c")
                .arg("credential.interactive=never")
                .arg("-c")
                .arg("transfer.fsckObjects=true")
                .arg("-c")
                .arg("fetch.fsckObjects=true")
                .arg("-c")
                .arg("receive.fsckObjects=true")
                .arg("-c")
                .arg("fsck.skipList=/dev/null");
            // Configurația de autentificare rămâne în mediul procesului, nu în
            // argv/proces list. Intrarea goală elimină helper-ele locale, apoi
            // sunt reaplicate exclusiv helper-ele globale inspectate mai sus.
            command.env(
                "GIT_CONFIG_COUNT",
                (credential_helpers.len() + 1).to_string(),
            );
            command
                .env("GIT_CONFIG_KEY_0", "credential.helper")
                .env("GIT_CONFIG_VALUE_0", "");
            for (index, (key, value)) in credential_helpers.iter().enumerate() {
                let index = index + 1;
                command
                    .env(format!("GIT_CONFIG_KEY_{index}"), key)
                    .env(format!("GIT_CONFIG_VALUE_{index}"), value);
            }
        }
        command.args(args);

        // Fetch/Push primesc un grup de procese propriu, astfel încât anularea
        // și timeout-ul să oprească și ssh/credential/transport children care
        // ar putea păstra pipe-urile deschise după terminarea procesului Git.
        #[cfg(target_os = "linux")]
        if allow_user_config {
            command.process_group(0);
        }

        let mut child = command.spawn().map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                "Executabilul Git nu este disponibil în PATH.".to_string()
            } else {
                format!("Procesul Git nu a putut fi pornit: {error}")
            }
        })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Procesul Git nu a publicat stdout.".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "Procesul Git nu a publicat stderr.".to_string())?;
        let stdout_reader = thread::spawn(move || read_capped(stdout, output_limit));
        let stderr_reader = thread::spawn(move || {
            read_capped_with_progress(stderr, output_limit, progress.as_deref())
        });

        if let Some(input) = input {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| "Procesul Git nu a deschis stdin.".to_string())?;
            stdin
                .write_all(input)
                .map_err(|error| format!("Nu am putut trimite input către Git: {error}"))?;
        }

        Ok(RunningGitCommand {
            child,
            stdout_reader: Some(stdout_reader),
            stderr_reader: Some(stderr_reader),
            started: Instant::now(),
            timeout,
            cancellation,
            network_process_group: allow_user_config,
            reaped: false,
        })
    }
}

pub(crate) struct RunningGitCommand {
    child: std::process::Child,
    stdout_reader: Option<GitOutputReader>,
    stderr_reader: Option<GitOutputReader>,
    started: Instant,
    timeout: Duration,
    cancellation: Option<Arc<AtomicBool>>,
    network_process_group: bool,
    reaped: bool,
}

impl RunningGitCommand {
    pub(crate) fn wait(mut self) -> Result<GitCommandOutput, String> {
        let status = loop {
            match self
                .child
                .try_wait()
                .map_err(|error| format!("Nu am putut verifica procesul Git: {error}"))?
            {
                Some(status) => {
                    self.reaped = true;
                    break status;
                }
                None if self
                    .cancellation
                    .as_ref()
                    .is_some_and(|token| token.load(Ordering::SeqCst)) =>
                {
                    self.terminate_and_reap();
                    self.join_readers_discard();
                    return Err(NETWORK_CANCELLED_ERROR.to_string());
                }
                None if self.started.elapsed() < self.timeout => {
                    thread::sleep(Duration::from_millis(10));
                }
                None => {
                    self.terminate_and_reap();
                    self.join_readers_discard();
                    return Err(format!(
                        "Procesul Git a depășit limita de {} secunde și a fost oprit.",
                        self.timeout.as_secs()
                    ));
                }
            }
        };

        let (stdout, stdout_truncated) =
            join_output_reader(self.stdout_reader.take(), "Thread-ul stdout Git a eșuat.")?;
        let (stderr, stderr_truncated) =
            join_output_reader(self.stderr_reader.take(), "Thread-ul stderr Git a eșuat.")?;
        Ok(GitCommandOutput {
            status,
            stdout,
            stderr,
            stdout_truncated,
            stderr_truncated,
        })
    }

    fn terminate_and_reap(&mut self) {
        if self.reaped {
            return;
        }
        terminate_git_process(&mut self.child, self.network_process_group);
        let _ = self.child.wait();
        self.reaped = true;
    }

    fn join_readers_discard(&mut self) {
        if let Some(reader) = self.stdout_reader.take() {
            let _ = reader.join();
        }
        if let Some(reader) = self.stderr_reader.take() {
            let _ = reader.join();
        }
    }
}

impl Drop for RunningGitCommand {
    fn drop(&mut self) {
        self.terminate_and_reap();
        self.join_readers_discard();
    }
}

fn join_output_reader(
    reader: Option<GitOutputReader>,
    panic_diagnostic: &str,
) -> Result<(Vec<u8>, bool), String> {
    reader
        .ok_or_else(|| "Procesul Git nu mai deține reader-ul de output.".to_string())?
        .join()
        .map_err(|_| panic_diagnostic.to_string())?
}

fn require_not_cancelled(cancellation: &AtomicBool) -> Result<(), String> {
    if cancellation.load(Ordering::SeqCst) {
        return Err(NETWORK_CANCELLED_ERROR.to_string());
    }
    Ok(())
}

fn terminate_git_process(child: &mut std::process::Child, network_process_group: bool) {
    #[cfg(target_os = "linux")]
    if network_process_group {
        let pid = rustix::process::Pid::from_child(child);
        if rustix::process::kill_process_group(pid, rustix::process::Signal::KILL).is_ok() {
            return;
        }
    }
    let _ = child.kill();
}

fn read_capped(mut reader: impl Read, limit: usize) -> Result<(Vec<u8>, bool), String> {
    read_capped_with_progress(&mut reader, limit, None)
}

type GitProgressCallback = dyn Fn(&[u8]) + Send + Sync + 'static;

fn read_capped_with_progress(
    mut reader: impl Read,
    limit: usize,
    progress: Option<&GitProgressCallback>,
) -> Result<(Vec<u8>, bool), String> {
    let mut retained = Vec::with_capacity(limit.min(64 * 1024));
    let mut buffer = [0_u8; 16 * 1024];
    let mut truncated = false;
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("Nu am putut citi outputul Git: {error}"))?;
        if read == 0 {
            break;
        }
        if let Some(progress) = progress {
            progress(&buffer[..read]);
        }
        let remaining = limit.saturating_sub(retained.len());
        if remaining > 0 {
            retained.extend_from_slice(&buffer[..read.min(remaining)]);
        }
        if read > remaining {
            truncated = true;
        }
    }
    Ok((retained, truncated))
}

pub fn canonical_git_root(output: &str) -> Result<PathBuf, String> {
    let path = output.trim();
    if path.is_empty() {
        return Err("Git nu a returnat rădăcina repository-ului.".to_string());
    }
    Path::new(path)
        .canonicalize()
        .map_err(|error| format!("Rădăcina raportată de Git nu poate fi canonizată: {error}"))
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    #[test]
    fn pre_cancelled_network_command_is_rejected_before_spawn() {
        let cancellation = Arc::new(AtomicBool::new(true));
        let error = super::GitRunner::new(std::env::temp_dir())
            .spawn_network(["status"], cancellation, Arc::new(|_| {}))
            .err()
            .expect("pre-cancelled network command must not spawn");
        assert_eq!(error, super::NETWORK_CANCELLED_ERROR);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn network_termination_kills_the_complete_process_group() {
        use std::{
            os::unix::process::CommandExt,
            process::{Command, Stdio},
            thread,
            time::{Duration, Instant},
        };

        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("sleep 30")
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = command.spawn().unwrap();
        thread::sleep(Duration::from_millis(25));
        let started = Instant::now();
        super::terminate_git_process(&mut child, true);
        let status = child.wait().unwrap();
        assert!(!status.success());
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn running_network_command_observes_cancellation_token() {
        use std::{thread, time::Duration};

        let cancellation = Arc::new(AtomicBool::new(false));
        let running = super::GitRunner::new(std::env::temp_dir())
            .spawn_network(
                ["-c", "alias.pana-slow=!sleep 30", "pana-slow"],
                Arc::clone(&cancellation),
                Arc::new(|_| {}),
            )
            .unwrap();
        let canceller = thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            cancellation.store(true, Ordering::SeqCst);
        });

        let started = std::time::Instant::now();
        let error = running.wait().unwrap_err();
        canceller.join().unwrap();
        assert_eq!(error, super::NETWORK_CANCELLED_ERROR);
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
