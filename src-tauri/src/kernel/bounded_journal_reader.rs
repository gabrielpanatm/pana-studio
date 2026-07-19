use std::{
    fs::File,
    io::{BufRead, BufReader, Read},
    path::{Component, Path, PathBuf},
};

#[cfg(target_os = "linux")]
use rustix::fs::{self as rustix_fs, FileType, FlockOperation, Mode, OFlags};

pub(crate) const APPEND_JOURNAL_MAX_LINE_BYTES: usize = 256 * 1024;
pub(crate) const APPEND_JOURNAL_MAX_SCAN_BYTES: u64 = 16 * 1024 * 1024;
pub(crate) const APPEND_JOURNAL_MAX_LINES: u64 = 100_000;

#[derive(Clone, Copy, Debug)]
pub(crate) struct BoundedJournalReadLimits {
    pub max_line_bytes: usize,
    pub max_scan_bytes: u64,
    pub max_lines: u64,
}

pub(crate) const APPEND_JOURNAL_READ_LIMITS: BoundedJournalReadLimits = BoundedJournalReadLimits {
    max_line_bytes: APPEND_JOURNAL_MAX_LINE_BYTES,
    max_scan_bytes: APPEND_JOURNAL_MAX_SCAN_BYTES,
    max_lines: APPEND_JOURNAL_MAX_LINES,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BoundedJournalReadOutcome {
    Missing,
    Present,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BoundedRegularFileSnapshot {
    pub bytes: Vec<u8>,
    pub version_token: String,
}

pub(crate) fn read_bounded_regular_file_snapshot(
    path: &Path,
    file_label: &str,
    max_bytes: u64,
) -> Result<Option<BoundedRegularFileSnapshot>, String> {
    #[cfg(target_os = "linux")]
    {
        let Some((parent_lock, mut file, initial_file_state)) =
            open_journal_with_shared_parent_lock(path, file_label)?
        else {
            return Ok(None);
        };
        let expected_size = u64::try_from(initial_file_state.size).map_err(|_| {
            format!(
                "{file_label} {} a fost blocat: fișierul are dimensiune negativă.",
                path.display()
            )
        })?;
        if expected_size > max_bytes {
            return Err(format!(
                "{file_label} {} a fost blocat: fișierul are {expected_size} bytes, peste limita de {max_bytes} bytes.",
                path.display()
            ));
        }
        let capacity = usize::try_from(expected_size).map_err(|_| {
            format!(
                "{file_label} {} a fost blocat: dimensiunea nu încape în memoria adresabilă.",
                path.display()
            )
        })?;
        let mut bytes = Vec::with_capacity(capacity);
        file.by_ref()
            .take(max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| {
                format!(
                    "{file_label} {} nu poate fi citit bounded: {error}",
                    path.display()
                )
            })?;
        if bytes.len() as u64 != expected_size {
            return Err(format!(
                "{file_label} {} s-a schimbat în timpul citirii bounded.",
                path.display()
            ));
        }
        validate_journal_postflight(path, file_label, &parent_lock, &file, initial_file_state)?;
        return Ok(Some(BoundedRegularFileSnapshot {
            bytes,
            version_token: initial_file_state.version_token(),
        }));
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = max_bytes;
        Err(format!(
            "{file_label} {} a fost blocat: stable bounded file read este fail-closed în afara Linux.",
            path.display()
        ))
    }
}

/// Streams a UTF-8 line journal without ever allocating more than one bounded
/// line. Resource-limit breaches are terminal: returning a partial journal as
/// authoritative recovery evidence would make later classification unsafe.
pub(crate) fn read_bounded_journal_lines(
    path: &Path,
    journal_label: &str,
    limits: BoundedJournalReadLimits,
    mut consume: impl FnMut(u64, &str),
) -> Result<BoundedJournalReadOutcome, String> {
    #[cfg(target_os = "linux")]
    {
        let Some((parent_lock, file, initial_file_state)) =
            open_journal_with_shared_parent_lock(path, journal_label)?
        else {
            return Ok(BoundedJournalReadOutcome::Missing);
        };
        let file = consume_bounded_journal_file(file, path, journal_label, limits, &mut consume)?;
        validate_journal_postflight(path, journal_label, &parent_lock, &file, initial_file_state)?;
        return Ok(BoundedJournalReadOutcome::Present);
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = (&mut consume, limits);
        Err(format!(
            "{journal_label} {} a fost blocat: stable journal read este fail-closed în afara Linux.",
            path.display()
        ))
    }
}

fn consume_bounded_journal_file(
    file: File,
    path: &Path,
    journal_label: &str,
    limits: BoundedJournalReadLimits,
    consume: &mut impl FnMut(u64, &str),
) -> Result<File, String> {
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut line_number = 0_u64;
    let mut scanned_bytes = 0_u64;

    loop {
        line.clear();
        let bytes = reader
            .by_ref()
            .take(limits.max_line_bytes.saturating_add(1) as u64)
            .read_line(&mut line)
            .map_err(|error| {
                format!(
                    "{journal_label} {} nu poate citi linia {} ca UTF-8: {error}",
                    path.display(),
                    line_number.saturating_add(1)
                )
            })?;
        if bytes == 0 {
            break;
        }

        line_number = line_number.saturating_add(1);
        if bytes > limits.max_line_bytes {
            return Err(format!(
                "{journal_label} {} a fost blocat: linia {line_number} depășește limita de {} bytes.",
                path.display(),
                limits.max_line_bytes
            ));
        }
        if !line.ends_with('\n') {
            return Err(format!(
                "{journal_label} {} a fost blocat: linia {line_number} nu este terminată cu LF și poate fi un append parțial.",
                path.display()
            ));
        }
        if line_number > limits.max_lines {
            return Err(format!(
                "{journal_label} {} a fost blocat: numărul liniilor depășește limita de {}.",
                path.display(),
                limits.max_lines
            ));
        }

        scanned_bytes = scanned_bytes.saturating_add(bytes as u64);
        if scanned_bytes > limits.max_scan_bytes {
            return Err(format!(
                "{journal_label} {} a fost blocat: scanarea depășește limita totală de {} bytes.",
                path.display(),
                limits.max_scan_bytes
            ));
        }

        consume(line_number, &line);
    }
    Ok(reader.into_inner())
}

pub(crate) fn read_bounded_journal_text(
    path: &Path,
    journal_label: &str,
    limits: BoundedJournalReadLimits,
) -> Result<Option<String>, String> {
    let mut text = String::new();
    let outcome = read_bounded_journal_lines(path, journal_label, limits, |_, line| {
        text.push_str(line);
    })?;
    match outcome {
        BoundedJournalReadOutcome::Missing => Ok(None),
        BoundedJournalReadOutcome::Present => Ok(Some(text)),
    }
}

#[cfg(target_os = "linux")]
pub(crate) struct BoundedJournalExclusiveParentLock {
    directory: rustix::fd::OwnedFd,
    parent_path: PathBuf,
}

#[cfg(target_os = "linux")]
pub(crate) fn lock_bounded_journal_parent_exclusive(
    journal_path: &Path,
    journal_label: &str,
) -> Result<BoundedJournalExclusiveParentLock, String> {
    let parent_path = journal_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| {
            format!(
                "{journal_label} {} a fost blocat: targetul nu are parent pentru lock exclusiv.",
                journal_path.display()
            )
        })?
        .to_path_buf();
    let directory =
        open_absolute_directory_no_symlinks(&parent_path, journal_label)?.ok_or_else(|| {
            format!(
                "{journal_label} {} a fost blocat: parentul lipsește pentru lock exclusiv.",
                journal_path.display()
            )
        })?;
    rustix_fs::flock(&directory, FlockOperation::LockExclusive).map_err(|error| {
        format!(
            "{journal_label} {} a fost blocat: stable parent exclusive lock a eșuat: {error}.",
            journal_path.display()
        )
    })?;
    Ok(BoundedJournalExclusiveParentLock {
        directory,
        parent_path,
    })
}

#[cfg(target_os = "linux")]
pub(crate) fn read_bounded_journal_text_under_exclusive_lock(
    lock: &BoundedJournalExclusiveParentLock,
    path: &Path,
    journal_label: &str,
    limits: BoundedJournalReadLimits,
) -> Result<Option<String>, String> {
    if path.parent() != Some(lock.parent_path.as_path()) {
        return Err(format!(
            "{journal_label} {} a fost blocat: lock-ul exclusiv aparține altui parent.",
            path.display()
        ));
    }
    let Some((file, initial_file_state)) =
        open_journal_leaf_from_locked_parent(&lock.directory, path, journal_label)?
    else {
        validate_missing_journal_postflight(path, journal_label, &lock.directory)?;
        return Ok(None);
    };
    let mut text = String::new();
    let file = consume_bounded_journal_file(file, path, journal_label, limits, &mut |_, line| {
        text.push_str(line)
    })?;
    validate_journal_postflight(
        path,
        journal_label,
        &lock.directory,
        &file,
        initial_file_state,
    )?;
    Ok(Some(text))
}

#[cfg(not(target_os = "linux"))]
pub(crate) struct BoundedJournalExclusiveParentLock;

#[cfg(not(target_os = "linux"))]
pub(crate) fn lock_bounded_journal_parent_exclusive(
    _journal_path: &Path,
    _journal_label: &str,
) -> Result<BoundedJournalExclusiveParentLock, String> {
    Err("Stable journal parent lock este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn read_bounded_journal_text_under_exclusive_lock(
    _lock: &BoundedJournalExclusiveParentLock,
    _path: &Path,
    _journal_label: &str,
    _limits: BoundedJournalReadLimits,
) -> Result<Option<String>, String> {
    Err("Stable journal parent read este fail-closed în afara Linux.".into())
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct JournalFileState {
    device: u64,
    inode: u64,
    size: i64,
    mode: u32,
    links: u64,
    modified_seconds: i64,
    modified_nanoseconds: u64,
    changed_seconds: i64,
    changed_nanoseconds: u64,
}

#[cfg(target_os = "linux")]
impl JournalFileState {
    fn from_stat(stat: &rustix_fs::Stat) -> Self {
        Self {
            device: stat.st_dev,
            inode: stat.st_ino,
            size: stat.st_size,
            mode: stat.st_mode,
            links: stat.st_nlink,
            modified_seconds: stat.st_mtime,
            modified_nanoseconds: stat.st_mtime_nsec,
            changed_seconds: stat.st_ctime,
            changed_nanoseconds: stat.st_ctime_nsec,
        }
    }

    fn version_token(self) -> String {
        format!(
            "unix:{}:{}:{}:{}:{}:{}:{}:{}",
            self.device,
            self.inode,
            self.size,
            self.modified_seconds,
            self.modified_nanoseconds,
            self.changed_seconds,
            self.changed_nanoseconds,
            self.mode,
        )
    }
}

#[cfg(target_os = "linux")]
fn open_journal_with_shared_parent_lock(
    path: &Path,
    journal_label: &str,
) -> Result<Option<(rustix::fd::OwnedFd, File, JournalFileState)>, String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let Some(directory) = open_absolute_directory_no_symlinks(parent, journal_label)? else {
        // Append v2 interzice crearea implicită a parentului. Dacă namespace-ul
        // sesiunii nu există încă, journal-ul nu poate exista la acest punct în
        // timp; primul writer va trebui să creeze parentul înainte de planificare.
        return Ok(None);
    };
    rustix_fs::flock(&directory, FlockOperation::LockShared).map_err(|error| {
        format!(
            "{journal_label} {} a fost blocat: stable parent read lock a eșuat: {error}.",
            path.display()
        )
    })?;
    let Some((file, state)) =
        open_journal_leaf_from_locked_parent(&directory, path, journal_label)?
    else {
        validate_missing_journal_postflight(path, journal_label, &directory)?;
        return Ok(None);
    };
    Ok(Some((directory, file, state)))
}

#[cfg(target_os = "linux")]
fn open_journal_leaf_from_locked_parent(
    directory: &rustix::fd::OwnedFd,
    path: &Path,
    journal_label: &str,
) -> Result<Option<(File, JournalFileState)>, String> {
    let leaf = path.file_name().ok_or_else(|| {
        format!(
            "{journal_label} {} a fost blocat: targetul nu are leaf.",
            path.display()
        )
    })?;
    let descriptor = match rustix_fs::openat(
        directory,
        leaf,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(descriptor) => descriptor,
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(error) => {
            return Err(format!(
                "{journal_label} {} a fost blocat: leaf-ul nu poate fi deschis no-follow sub parentul blocat: {error}.",
                path.display()
            ));
        }
    };
    let stat = rustix_fs::fstat(&descriptor).map_err(|error| {
        format!(
            "{journal_label} {} a fost blocat: metadata leaf-ului nu poate fi verificată: {error}.",
            path.display()
        )
    })?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile || stat.st_nlink != 1 {
        return Err(format!(
            "{journal_label} {} a fost blocat: leaf-ul trebuie să fie fișier regulat cu un singur link.",
            path.display()
        ));
    }
    Ok(Some((
        File::from(descriptor),
        JournalFileState::from_stat(&stat),
    )))
}

#[cfg(target_os = "linux")]
fn open_absolute_directory_no_symlinks(
    path: &Path,
    journal_label: &str,
) -> Result<Option<rustix::fd::OwnedFd>, String> {
    if !path.is_absolute() {
        return Err(format!(
            "{journal_label} {} a fost blocat: stable journal read cere parent absolut.",
            path.display()
        ));
    }
    let mut directory = rustix_fs::open(
        Path::new("/"),
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| format!("{journal_label} nu poate captura rădăcina filesystem: {error}."))?;
    for component in path.components() {
        let leaf = match component {
            Component::RootDir => continue,
            Component::Normal(component) => component,
            Component::CurDir | Component::ParentDir | Component::Prefix(_) => {
                return Err(format!(
                    "{journal_label} {} a fost blocat: parentul conține o componentă lexicală interzisă.",
                    path.display()
                ));
            }
        };
        directory = match rustix_fs::openat(
            &directory,
            leaf,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(next) => next,
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(error) => {
                return Err(format!(
                    "{journal_label} {} a fost blocat: traversarea fd-relative no-follow a parentului a eșuat la {:?}: {error}.",
                    path.display(),
                    leaf
                ));
            }
        };
    }
    Ok(Some(directory))
}

#[cfg(target_os = "linux")]
fn validate_journal_postflight(
    path: &Path,
    journal_label: &str,
    locked_parent: &rustix::fd::OwnedFd,
    file: &File,
    initial_file_state: JournalFileState,
) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| {
        format!(
            "{journal_label} {} a fost blocat: targetul nu are parent la postflight.",
            path.display()
        )
    })?;
    let leaf = path.file_name().ok_or_else(|| {
        format!(
            "{journal_label} {} a fost blocat: targetul nu are leaf la postflight.",
            path.display()
        )
    })?;
    let final_file_stat = rustix_fs::fstat(file).map_err(|error| {
        format!(
            "{journal_label} {} a fost blocat: fstat postflight a eșuat: {error}.",
            path.display()
        )
    })?;
    if JournalFileState::from_stat(&final_file_stat) != initial_file_state {
        return Err(format!(
            "{journal_label} {} a fost blocat: fișierul s-a schimbat în timpul citirii.",
            path.display()
        ));
    }

    let Some(recaptured_parent) = open_absolute_directory_no_symlinks(parent, journal_label)?
    else {
        return Err(format!(
            "{journal_label} {} a fost blocat: parentul a dispărut în timpul citirii.",
            path.display()
        ));
    };
    let locked_parent_stat = rustix_fs::fstat(locked_parent).map_err(|error| {
        format!(
            "{journal_label} {} a fost blocat: parent fstat postflight a eșuat: {error}.",
            path.display()
        )
    })?;
    let recaptured_parent_stat = rustix_fs::fstat(&recaptured_parent).map_err(|error| {
        format!(
            "{journal_label} {} a fost blocat: parent recapture fstat a eșuat: {error}.",
            path.display()
        )
    })?;
    if locked_parent_stat.st_dev != recaptured_parent_stat.st_dev
        || locked_parent_stat.st_ino != recaptured_parent_stat.st_ino
    {
        return Err(format!(
            "{journal_label} {} a fost blocat: path-ul nu mai numește parentul blocat.",
            path.display()
        ));
    }

    let named = rustix_fs::openat(
        &recaptured_parent,
        leaf,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| {
        format!(
            "{journal_label} {} a fost blocat: leaf-ul nu mai poate fi recapturat la postflight: {error}.",
            path.display()
        )
    })?;
    let named_stat = rustix_fs::fstat(&named).map_err(|error| {
        format!(
            "{journal_label} {} a fost blocat: leaf fstat postflight a eșuat: {error}.",
            path.display()
        )
    })?;
    if JournalFileState::from_stat(&named_stat) != initial_file_state {
        return Err(format!(
            "{journal_label} {} a fost blocat: leaf-ul numit nu mai este fișierul citit.",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn validate_missing_journal_postflight(
    path: &Path,
    journal_label: &str,
    locked_parent: &rustix::fd::OwnedFd,
) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| {
        format!(
            "{journal_label} {} a fost blocat: targetul missing nu are parent la postflight.",
            path.display()
        )
    })?;
    let leaf = path.file_name().ok_or_else(|| {
        format!(
            "{journal_label} {} a fost blocat: targetul missing nu are leaf la postflight.",
            path.display()
        )
    })?;
    let Some(recaptured_parent) = open_absolute_directory_no_symlinks(parent, journal_label)?
    else {
        return Err(format!(
            "{journal_label} {} a fost blocat: parentul a dispărut înainte de confirmarea rezultatului Missing.",
            path.display()
        ));
    };
    let locked_parent_stat = rustix_fs::fstat(locked_parent).map_err(|error| {
        format!(
            "{journal_label} {} a fost blocat: parent fstat pentru Missing a eșuat: {error}.",
            path.display()
        )
    })?;
    let recaptured_parent_stat = rustix_fs::fstat(&recaptured_parent).map_err(|error| {
        format!(
            "{journal_label} {} a fost blocat: parent recapture pentru Missing a eșuat: {error}.",
            path.display()
        )
    })?;
    if locked_parent_stat.st_dev != recaptured_parent_stat.st_dev
        || locked_parent_stat.st_ino != recaptured_parent_stat.st_ino
    {
        return Err(format!(
            "{journal_label} {} a fost blocat: path-ul nu mai numește parentul în care leaf-ul lipsea.",
            path.display()
        ));
    }
    match rustix_fs::openat(
        &recaptured_parent,
        leaf,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Err(rustix::io::Errno::NOENT) => Ok(()),
        Ok(_) => Err(format!(
            "{journal_label} {} a fost blocat: leaf-ul a apărut înainte de confirmarea rezultatului Missing.",
            path.display()
        )),
        Err(error) => Err(format!(
            "{journal_label} {} a fost blocat: revalidarea leaf-ului Missing a eșuat: {error}.",
            path.display()
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::{read_bounded_journal_lines, BoundedJournalReadLimits, BoundedJournalReadOutcome};

    #[test]
    fn reader_rejects_a_line_before_allocating_past_the_bound() {
        let path = temp_file("oversized-line");
        fs::write(&path, "x".repeat(17)).unwrap();
        let error =
            read_bounded_journal_lines(&path, "Test Journal", limits(), |_, _| {}).unwrap_err();

        assert!(error.contains("linia 1"));
        assert!(error.contains("limita de 16 bytes"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn reader_rejects_total_bytes_instead_of_returning_a_partial_journal() {
        let path = temp_file("oversized-total");
        fs::write(&path, "1234567\n1234567\n").unwrap();
        let mut consumed = 0_u64;
        let error = read_bounded_journal_lines(&path, "Test Journal", limits(), |_, _| {
            consumed += 1;
        })
        .unwrap_err();

        assert_eq!(consumed, 1);
        assert!(error.contains("limita totală de 12 bytes"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn reader_rejects_excess_lines() {
        let path = temp_file("oversized-count");
        fs::write(&path, "{}\n{}\n{}\n").unwrap();
        let error =
            read_bounded_journal_lines(&path, "Test Journal", limits(), |_, _| {}).unwrap_err();

        assert!(error.contains("numărul liniilor"));
        assert!(error.contains("limita de 2"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn reader_rejects_unterminated_last_line_before_callback() {
        let path = temp_file("unterminated-line");
        fs::write(&path, "{}").unwrap();
        let mut consumed = 0_u64;

        let error = read_bounded_journal_lines(&path, "Test Journal", limits(), |_, _| {
            consumed = consumed.saturating_add(1);
        })
        .unwrap_err();

        assert_eq!(consumed, 0);
        assert!(error.contains("nu este terminată cu LF"));
        let _ = fs::remove_file(path);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn reader_waits_for_append_exclusive_parent_lock_without_creating_a_lock_file() {
        use std::{sync::mpsc, thread, time::Duration};

        use rustix::fs::{self as rustix_fs, FlockOperation, Mode, OFlags};

        let root = temp_directory("shared-parent-lock");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("journal.jsonl");
        fs::write(&path, "{}\n").unwrap();
        let exclusive = rustix_fs::open(
            &root,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();
        rustix_fs::flock(&exclusive, FlockOperation::LockExclusive).unwrap();

        let (started_tx, started_rx) = mpsc::channel();
        let (finished_tx, finished_rx) = mpsc::channel();
        let reader_path = path.clone();
        let handle = thread::spawn(move || {
            started_tx.send(()).unwrap();
            let result =
                read_bounded_journal_lines(&reader_path, "Test Journal", limits(), |_, _| {});
            finished_tx.send(result).unwrap();
        });

        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(matches!(
            finished_rx.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));

        drop(exclusive);
        finished_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap();
        handle.join().unwrap();
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn exclusive_reader_guard_blocks_a_competing_append_parent_lock() {
        use std::{sync::mpsc, thread, time::Duration};

        use rustix::fs::{self as rustix_fs, FlockOperation, Mode, OFlags};

        let root = temp_directory("exclusive-parent-lock");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let path = root.join("journal.jsonl");
        fs::write(&path, "{}\n").unwrap();
        let guard = super::lock_bounded_journal_parent_exclusive(&path, "Test Journal").unwrap();

        let (started_tx, started_rx) = mpsc::channel();
        let (finished_tx, finished_rx) = mpsc::channel();
        let writer_root = root.clone();
        let handle = thread::spawn(move || {
            let directory = rustix_fs::open(
                &writer_root,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .unwrap();
            started_tx.send(()).unwrap();
            rustix_fs::flock(&directory, FlockOperation::LockExclusive).unwrap();
            finished_tx.send(()).unwrap();
        });

        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(matches!(
            finished_rx.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        drop(guard);
        finished_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        handle.join().unwrap();
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn exclusive_reader_guard_rejects_a_journal_under_another_parent() {
        let first = temp_directory("exclusive-wrong-parent-first");
        let second = temp_directory("exclusive-wrong-parent-second");
        let _ = fs::remove_dir_all(&first);
        let _ = fs::remove_dir_all(&second);
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();
        let first_path = first.join("journal.jsonl");
        let second_path = second.join("journal.jsonl");
        fs::write(&first_path, "{}\n").unwrap();
        fs::write(&second_path, "{}\n").unwrap();
        let guard =
            super::lock_bounded_journal_parent_exclusive(&first_path, "Test Journal").unwrap();

        let error = super::read_bounded_journal_text_under_exclusive_lock(
            &guard,
            &second_path,
            "Test Journal",
            limits(),
        )
        .unwrap_err();

        assert!(error.contains("altui parent"));
        let _ = fs::remove_dir_all(first);
        let _ = fs::remove_dir_all(second);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn missing_leaf_is_decided_only_after_shared_lock_and_sees_create_before_unlock() {
        use std::{sync::mpsc, thread, time::Duration};

        use rustix::fs::{self as rustix_fs, FlockOperation, Mode, OFlags};

        let root = temp_directory("shared-parent-lock-create");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let path = root.join("journal.jsonl");
        let exclusive = rustix_fs::open(
            &root,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();
        rustix_fs::flock(&exclusive, FlockOperation::LockExclusive).unwrap();

        let (started_tx, started_rx) = mpsc::channel();
        let (finished_tx, finished_rx) = mpsc::channel();
        let reader_path = path.clone();
        let handle = thread::spawn(move || {
            started_tx.send(()).unwrap();
            let mut consumed = Vec::new();
            let result =
                read_bounded_journal_lines(&reader_path, "Test Journal", limits(), |_, line| {
                    consumed.push(line.to_string())
                });
            finished_tx.send((result, consumed)).unwrap();
        });

        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(matches!(
            finished_rx.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        fs::write(&path, "{}\n").unwrap();
        drop(exclusive);

        let (result, consumed) = finished_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(result.unwrap(), BoundedJournalReadOutcome::Present);
        assert_eq!(consumed, vec!["{}\n"]);
        handle.join().unwrap();
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn missing_session_parent_is_a_clean_missing_journal() {
        let root = temp_directory("missing-session-parent");
        let _ = fs::remove_dir_all(&root);
        let path = root.join("sessions/session-id/transactions.jsonl");
        let mut consumed = 0_u64;

        let outcome = read_bounded_journal_lines(&path, "Test Journal", limits(), |_, _| {
            consumed = consumed.saturating_add(1);
        })
        .unwrap();

        assert_eq!(outcome, BoundedJournalReadOutcome::Missing);
        assert_eq!(consumed, 0);
        assert!(!root.exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn intermediate_parent_symlink_is_rejected_without_consuming_external_journal() {
        use std::os::unix::fs::symlink;

        let root = temp_directory("intermediate-parent-symlink");
        let outside = temp_directory("intermediate-parent-symlink-outside");
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&outside);
        fs::create_dir_all(root.join("data")).unwrap();
        fs::create_dir_all(outside.join("session-id")).unwrap();
        fs::write(outside.join("session-id/transactions.jsonl"), "{}\n").unwrap();
        symlink(&outside, root.join("data/sessions")).unwrap();
        let path = root.join("data/sessions/session-id/transactions.jsonl");
        let mut consumed = 0_u64;

        let error = read_bounded_journal_lines(&path, "Test Journal", limits(), |_, _| {
            consumed = consumed.saturating_add(1);
        })
        .unwrap_err();

        assert_eq!(consumed, 0);
        assert!(error.contains("fd-relative no-follow"));
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn non_directory_parent_is_an_error_not_a_missing_journal() {
        let root = temp_directory("non-directory-parent");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("session-id"), "not-a-directory").unwrap();
        let path = root.join("session-id/transactions.jsonl");

        let error =
            read_bounded_journal_lines(&path, "Test Journal", limits(), |_, _| {}).unwrap_err();

        assert!(error.contains("fd-relative no-follow"));
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn named_leaf_replacement_during_read_fails_postflight() {
        let root = temp_directory("named-leaf-replacement");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let path = root.join("journal.jsonl");
        let moved = root.join("journal.original.jsonl");
        fs::write(&path, "{}\n").unwrap();
        let callback_path = path.clone();
        let callback_moved = moved.clone();

        let error = read_bounded_journal_lines(&path, "Test Journal", limits(), move |_, _| {
            fs::rename(&callback_path, &callback_moved).unwrap();
            fs::write(&callback_path, "{}\n").unwrap();
        })
        .unwrap_err();

        assert!(
            error.contains("s-a schimbat")
                || error.contains("leaf-ul numit nu mai este fișierul citit"),
            "{error}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn missing_leaf_postflight_rejects_public_parent_replacement() {
        use rustix::fs::{self as rustix_fs, FlockOperation};

        let root = temp_directory("missing-parent-replacement");
        let moved = temp_directory("missing-parent-replacement-moved");
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&moved);
        fs::create_dir_all(&root).unwrap();
        let path = root.join("journal.jsonl");
        let locked_parent = super::open_absolute_directory_no_symlinks(&root, "Test Journal")
            .unwrap()
            .unwrap();
        rustix_fs::flock(&locked_parent, FlockOperation::LockShared).unwrap();
        fs::rename(&root, &moved).unwrap();
        fs::create_dir_all(&root).unwrap();
        fs::write(&path, "{}\n").unwrap();

        let error =
            super::validate_missing_journal_postflight(&path, "Test Journal", &locked_parent)
                .unwrap_err();

        assert!(error.contains("nu mai numește parentul"));
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(moved);
    }

    fn limits() -> BoundedJournalReadLimits {
        BoundedJournalReadLimits {
            max_line_bytes: 16,
            max_scan_bytes: 12,
            max_lines: 2,
        }
    }

    fn temp_file(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "pana-bounded-journal-{label}-{}",
            std::process::id()
        ))
    }

    fn temp_directory(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "pana-bounded-journal-dir-{label}-{}",
            std::process::id()
        ))
    }
}
