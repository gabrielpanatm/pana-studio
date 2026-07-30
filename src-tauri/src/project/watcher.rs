use serde::Serialize;

pub const PROJECT_DISK_CHANGED_EVENT: &str = "pana-project-disk-changed";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDiskChangeNotice {
    pub schema_version: u32,
    pub project_root: String,
    pub runtime_session_id: String,
    pub watch_generation: u64,
    pub watch_revision: u64,
    pub changed_paths: Vec<String>,
    pub overflowed: bool,
}

#[cfg(target_os = "linux")]
mod platform {
    use std::{
        collections::{BTreeMap, BTreeSet},
        path::{Component, Path, PathBuf},
        sync::{
            atomic::{AtomicBool, AtomicU64, Ordering},
            mpsc::{self, Receiver, RecvTimeoutError, Sender},
            Arc, Mutex,
        },
        thread::{self, JoinHandle},
        time::Duration,
    };

    use nix::sys::inotify::{AddWatchFlags, InitFlags, Inotify, WatchDescriptor};
    use tauri::{AppHandle, Emitter, Runtime};
    use walkdir::WalkDir;

    use super::{ProjectDiskChangeNotice, PROJECT_DISK_CHANGED_EVENT};
    use crate::project::scope::is_derived_or_internal_dir;

    const WATCH_DEBOUNCE: Duration = Duration::from_millis(240);
    const WATCH_SCHEMA_VERSION: u32 = 1;
    const MAX_BATCH_PATHS: usize = 1_000;
    static WATCH_GENERATION: AtomicU64 = AtomicU64::new(1);

    const WATCH_MASK: AddWatchFlags = AddWatchFlags::IN_CLOSE_WRITE
        .union(AddWatchFlags::IN_ATTRIB)
        .union(AddWatchFlags::IN_CREATE)
        .union(AddWatchFlags::IN_DELETE)
        .union(AddWatchFlags::IN_MOVED_FROM)
        .union(AddWatchFlags::IN_MOVED_TO)
        .union(AddWatchFlags::IN_DELETE_SELF)
        .union(AddWatchFlags::IN_MOVE_SELF)
        .union(AddWatchFlags::IN_ONLYDIR)
        .union(AddWatchFlags::IN_DONT_FOLLOW);

    enum WatchMessage {
        Changed {
            relative_path: Option<String>,
            overflowed: bool,
        },
        Stop,
    }

    pub struct ProjectDiskWatchHandle {
        project_root: PathBuf,
        runtime_session_id: String,
        watch_generation: u64,
        stop: Arc<AtomicBool>,
        inotify: Arc<Inotify>,
        watches: Arc<Mutex<BTreeMap<WatchDescriptor, PathBuf>>>,
        sender: Sender<WatchMessage>,
        listener: Option<JoinHandle<()>>,
        publisher: Option<JoinHandle<()>>,
    }

    impl ProjectDiskWatchHandle {
        pub fn start<R: Runtime>(
            app: AppHandle<R>,
            project_root: PathBuf,
            runtime_session_id: String,
        ) -> Result<Self, String> {
            let metadata = std::fs::symlink_metadata(&project_root).map_err(|error| {
                format!(
                    "Watcher-ul nu poate inspecta rădăcina {}: {error}",
                    project_root.display()
                )
            })?;
            if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
                return Err("Watcher-ul discului cere o rădăcină reală, non-symlink.".to_string());
            }
            let inotify = Arc::new(
                Inotify::init(InitFlags::IN_CLOEXEC)
                    .map_err(|error| format!("Watcher-ul inotify nu poate porni: {error}"))?,
            );
            let watches = Arc::new(Mutex::new(BTreeMap::new()));
            install_recursive_watches(&inotify, &watches, &project_root, &project_root)?;

            let watch_generation = WATCH_GENERATION.fetch_add(1, Ordering::Relaxed);
            let stop = Arc::new(AtomicBool::new(false));
            let (sender, receiver) = mpsc::channel();
            let publisher = spawn_publisher(
                app,
                receiver,
                project_root.clone(),
                runtime_session_id.clone(),
                watch_generation,
            )?;
            let listener = spawn_listener(
                Arc::clone(&inotify),
                Arc::clone(&watches),
                Arc::clone(&stop),
                sender.clone(),
                project_root.clone(),
            )?;

            Ok(Self {
                project_root,
                runtime_session_id,
                watch_generation,
                stop,
                inotify,
                watches,
                sender,
                listener: Some(listener),
                publisher: Some(publisher),
            })
        }

        pub fn matches(&self, project_root: &Path, runtime_session_id: &str) -> bool {
            self.project_root == project_root && self.runtime_session_id == runtime_session_id
        }

        pub fn watch_generation(&self) -> u64 {
            self.watch_generation
        }

        pub fn stop(mut self) {
            self.stop_inner();
        }

        fn stop_inner(&mut self) {
            if self.stop.swap(true, Ordering::AcqRel) {
                return;
            }
            let _ = self.sender.send(WatchMessage::Stop);
            if let Ok(mut watches) = self.watches.lock() {
                for descriptor in watches.keys().copied().collect::<Vec<_>>() {
                    let _ = self.inotify.rm_watch(descriptor);
                }
                watches.clear();
            }
            if let Some(listener) = self.listener.take() {
                let _ = listener.join();
            }
            if let Some(publisher) = self.publisher.take() {
                let _ = publisher.join();
            }
        }
    }

    impl Drop for ProjectDiskWatchHandle {
        fn drop(&mut self) {
            self.stop_inner();
        }
    }

    fn spawn_listener(
        inotify: Arc<Inotify>,
        watches: Arc<Mutex<BTreeMap<WatchDescriptor, PathBuf>>>,
        stop: Arc<AtomicBool>,
        sender: Sender<WatchMessage>,
        project_root: PathBuf,
    ) -> Result<JoinHandle<()>, String> {
        thread::Builder::new()
            .name("pana-project-disk-watch".to_string())
            .spawn(move || loop {
                let events = match inotify.read_events() {
                    Ok(events) => events,
                    Err(error) => {
                        if !stop.load(Ordering::Acquire) {
                            let _ = sender.send(WatchMessage::Changed {
                                relative_path: None,
                                overflowed: true,
                            });
                            eprintln!("[Pană Studio] Watcher-ul inotify s-a oprit: {error}");
                        }
                        break;
                    }
                };
                if stop.load(Ordering::Acquire) {
                    break;
                }
                for event in events {
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    if event.mask.contains(AddWatchFlags::IN_IGNORED) {
                        if let Ok(mut registry) = watches.lock() {
                            registry.remove(&event.wd);
                        }
                        continue;
                    }
                    let parent = watches
                        .lock()
                        .ok()
                        .and_then(|registry| registry.get(&event.wd).cloned());
                    let Some(parent) = parent else {
                        let _ = sender.send(WatchMessage::Changed {
                            relative_path: None,
                            overflowed: true,
                        });
                        continue;
                    };
                    let path = event
                        .name
                        .as_ref()
                        .map_or_else(|| parent.clone(), |name| parent.join(name));
                    let relative_path = watched_relative_path(&project_root, &path);
                    if relative_path
                        .as_deref()
                        .is_some_and(is_ignored_relative_path)
                    {
                        continue;
                    }
                    if event.mask.contains(AddWatchFlags::IN_ISDIR)
                        && (event.mask.contains(AddWatchFlags::IN_CREATE)
                            || event.mask.contains(AddWatchFlags::IN_MOVED_TO))
                    {
                        if let Err(error) =
                            install_recursive_watches(&inotify, &watches, &project_root, &path)
                        {
                            eprintln!(
                                "[Pană Studio] Watcher-ul nu poate adăuga directorul {}: {error}",
                                path.display()
                            );
                            let _ = sender.send(WatchMessage::Changed {
                                relative_path: None,
                                overflowed: true,
                            });
                        }
                    }
                    let _ = sender.send(WatchMessage::Changed {
                        relative_path,
                        overflowed: false,
                    });
                }
            })
            .map_err(|error| format!("Thread-ul watcher-ului de disc nu poate porni: {error}"))
    }

    fn spawn_publisher<R: Runtime>(
        app: AppHandle<R>,
        receiver: Receiver<WatchMessage>,
        project_root: PathBuf,
        runtime_session_id: String,
        watch_generation: u64,
    ) -> Result<JoinHandle<()>, String> {
        thread::Builder::new()
            .name("pana-project-disk-watch-debounce".to_string())
            .spawn(move || {
                let mut watch_revision = 0_u64;
                while let Ok(message) = receiver.recv() {
                    let WatchMessage::Changed {
                        relative_path,
                        overflowed,
                    } = message
                    else {
                        break;
                    };
                    let mut paths = BTreeSet::new();
                    let mut overflowed_batch = overflowed;
                    if let Some(path) = relative_path {
                        paths.insert(path);
                    }
                    loop {
                        match receiver.recv_timeout(WATCH_DEBOUNCE) {
                            Ok(WatchMessage::Changed {
                                relative_path,
                                overflowed,
                            }) => {
                                overflowed_batch |= overflowed;
                                if let Some(path) = relative_path {
                                    if paths.len() < MAX_BATCH_PATHS {
                                        paths.insert(path);
                                    } else {
                                        overflowed_batch = true;
                                    }
                                }
                            }
                            Ok(WatchMessage::Stop) | Err(RecvTimeoutError::Disconnected) => return,
                            Err(RecvTimeoutError::Timeout) => break,
                        }
                    }
                    watch_revision = watch_revision.saturating_add(1);
                    let notice = ProjectDiskChangeNotice {
                        schema_version: WATCH_SCHEMA_VERSION,
                        project_root: project_root.to_string_lossy().to_string(),
                        runtime_session_id: runtime_session_id.clone(),
                        watch_generation,
                        watch_revision,
                        changed_paths: paths.into_iter().collect(),
                        overflowed: overflowed_batch,
                    };
                    if let Err(error) = app.emit(PROJECT_DISK_CHANGED_EVENT, notice) {
                        eprintln!(
                            "[Pană Studio] Evenimentul watcher-ului de disc nu a putut fi emis: {error}"
                        );
                    }
                }
            })
            .map_err(|error| format!("Thread-ul debounce al watcher-ului nu poate porni: {error}"))
    }

    fn install_recursive_watches(
        inotify: &Inotify,
        watches: &Mutex<BTreeMap<WatchDescriptor, PathBuf>>,
        project_root: &Path,
        subtree_root: &Path,
    ) -> Result<(), String> {
        if !subtree_root.starts_with(project_root) {
            return Err("Watcher-ul a refuzat un subtree în afara proiectului.".to_string());
        }
        for entry in WalkDir::new(subtree_root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| {
                entry.depth() == 0
                    || !entry
                        .file_name()
                        .to_str()
                        .is_some_and(is_derived_or_internal_dir)
            })
        {
            let entry = entry.map_err(|error| {
                format!(
                    "Watcher-ul nu poate parcurge {}: {error}",
                    subtree_root.display()
                )
            })?;
            if !entry.file_type().is_dir() || entry.file_type().is_symlink() {
                continue;
            }
            let path = entry.path().to_path_buf();
            let descriptor = inotify
                .add_watch(&path, WATCH_MASK)
                .map_err(|error| format!("inotify_add_watch {}: {error}", path.display()))?;
            watches
                .lock()
                .map_err(|_| "Registrul watcher-ului este compromis.".to_string())?
                .insert(descriptor, path);
        }
        Ok(())
    }

    fn watched_relative_path(project_root: &Path, path: &Path) -> Option<String> {
        let relative = path.strip_prefix(project_root).ok()?;
        if relative.as_os_str().is_empty() {
            return None;
        }
        if !relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        {
            return None;
        }
        Some(relative.to_string_lossy().replace('\\', "/"))
    }

    fn is_ignored_relative_path(relative_path: &str) -> bool {
        Path::new(relative_path).components().any(|component| {
            matches!(
                component,
                Component::Normal(name)
                    if name.to_str().is_some_and(is_derived_or_internal_dir)
            )
        })
    }

    #[cfg(test)]
    mod tests {
        use std::{
            fs,
            sync::mpsc,
            time::{Duration, Instant},
        };

        use tauri::Listener;

        use super::*;

        #[test]
        fn watcher_paths_stay_relative_and_ignore_derived_subtrees() {
            let root = Path::new("/tmp/project");
            assert_eq!(
                watched_relative_path(root, Path::new("/tmp/project/templates/index.html")),
                Some("templates/index.html".to_string())
            );
            assert_eq!(
                watched_relative_path(root, Path::new("/tmp/other/index.html")),
                None
            );
            assert!(is_ignored_relative_path("public/index.html"));
            assert!(is_ignored_relative_path("nested/node_modules/pkg.js"));
            assert!(!is_ignored_relative_path("templates/index.html"));
        }

        #[test]
        fn watcher_publishes_a_debounced_change_and_stops_without_polling() {
            let root = std::env::temp_dir().join(format!(
                "pana-project-watcher-{}-{}",
                std::process::id(),
                WATCH_GENERATION.load(Ordering::Relaxed)
            ));
            fs::create_dir_all(root.join("templates")).unwrap();
            let app = tauri::test::mock_builder()
                .build(tauri::test::mock_context(tauri::test::noop_assets()))
                .unwrap();
            let (sender, receiver) = mpsc::channel();
            let listener = app.listen(PROJECT_DISK_CHANGED_EVENT, move |event| {
                let _ = sender.send(event.payload().to_string());
            });
            let watcher = ProjectDiskWatchHandle::start(
                app.handle().clone(),
                root.clone(),
                "session:watcher-test".to_string(),
            )
            .unwrap();

            fs::write(root.join("templates/index.html"), "<main>test</main>").unwrap();
            let payload = receiver
                .recv_timeout(Duration::from_secs(3))
                .expect("watcher-ul trebuie să emită după schimbarea fișierului");
            let notice: serde_json::Value = serde_json::from_str(&payload).unwrap();
            assert_eq!(notice["runtimeSessionId"], "session:watcher-test");
            assert_eq!(notice["changedPaths"][0], "templates/index.html");
            assert_eq!(notice["overflowed"], false);

            let stop_started = Instant::now();
            watcher.stop();
            assert!(
                stop_started.elapsed() < Duration::from_secs(2),
                "oprirea watcher-ului blocant nu trebuie să aștepte polling"
            );
            app.unlisten(listener);
            fs::remove_dir_all(root).unwrap();
        }
    }
}

#[cfg(target_os = "linux")]
pub use platform::ProjectDiskWatchHandle;

#[cfg(not(target_os = "linux"))]
pub struct ProjectDiskWatchHandle;

#[cfg(not(target_os = "linux"))]
impl ProjectDiskWatchHandle {
    pub fn start<R: tauri::Runtime>(
        _app: tauri::AppHandle<R>,
        _project_root: std::path::PathBuf,
        _runtime_session_id: String,
    ) -> Result<Self, String> {
        Err("Watcher-ul nativ al proiectului este indisponibil pe această platformă.".to_string())
    }

    pub fn matches(&self, _project_root: &std::path::Path, _runtime_session_id: &str) -> bool {
        false
    }

    pub fn watch_generation(&self) -> u64 {
        0
    }

    pub fn stop(self) {}
}
