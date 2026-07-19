#[cfg(target_os = "linux")]
mod platform {
    use std::{
        ffi::{OsStr, OsString},
        fs::File,
        io::{Read, Write},
        os::unix::ffi::{OsStrExt, OsStringExt},
    };

    use super::super::{
        model::{WalPhase, MAX_WAL_RECORDS, MAX_WAL_RECORD_BYTES, MAX_WAL_TOTAL_BYTES},
        paths::{WalRecordName, WAL_LOCK_FILE},
    };
    use crate::kernel::write_authority::root_authority::DirectoryAuthority;
    use rustix::{
        fd::OwnedFd,
        fs::{self, AtFlags, Dir, FileType, FlockOperation, Mode, OFlags, RenameFlags},
        io::Errno,
    };

    const LOCK_MODE: Mode = Mode::from_raw_mode(0o600);
    const RECORD_MODE: Mode = Mode::from_raw_mode(0o600);
    #[cfg(test)]
    thread_local! {
        static FAIL_RECORD_REMOVE_BEFORE_UNLINK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    #[cfg(test)]
    fn take_record_remove_failure() -> bool {
        FAIL_RECORD_REMOVE_BEFORE_UNLINK.with(|flag| flag.replace(false))
    }

    #[cfg(not(test))]
    const fn take_record_remove_failure() -> bool {
        false
    }

    /// Failure injection generic pentru frontiera terminală: recordul rămâne
    /// numit și durabil, iar `DurableWalGuard::{commit,abort_no_effect}` trebuie
    /// să publice bariera hot fără să pretindă că unlink-ul a avut loc.
    #[cfg(test)]
    pub(crate) fn with_record_remove_failure_before_unlink<T>(operation: impl FnOnce() -> T) -> T {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                FAIL_RECORD_REMOVE_BEFORE_UNLINK.with(|flag| flag.set(false));
            }
        }
        FAIL_RECORD_REMOVE_BEFORE_UNLINK.with(|flag| {
            assert!(
                !flag.replace(true),
                "WAL record remove failure hook already active"
            );
        });
        let _reset = Reset;
        operation()
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum WalLockMode {
        Exclusive,
    }

    pub(crate) struct WalFileLock {
        _descriptor: OwnedFd,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub(crate) struct WalRawEntry {
        pub file_name: String,
        pub bytes: Result<Vec<u8>, String>,
    }

    #[derive(Clone, Debug)]
    pub(crate) struct WalDirectory {
        authority: DirectoryAuthority,
    }

    impl WalDirectory {
        pub(crate) fn new(authority: DirectoryAuthority) -> Self {
            Self { authority }
        }

        pub(crate) fn authority(&self) -> &DirectoryAuthority {
            &self.authority
        }

        pub(crate) fn lock(&self, mode: WalLockMode) -> Result<WalFileLock, String> {
            let (descriptor, created) = open_or_create_lock(self.authority.directory())?;
            validate_regular_single_link(&descriptor, WAL_LOCK_FILE)?;
            if created {
                fs::fsync(&descriptor).map_err(|error| {
                    format!("WriteAuthority WAL lock file fsync a eșuat: {error}.")
                })?;
                sync_directory(self.authority.directory())?;
            }
            let operation = FlockOperation::LockExclusive;
            fs::flock(&descriptor, operation)
                .map_err(|error| format!("WriteAuthority WAL lock {mode:?} a eșuat: {error}."))?;
            Ok(WalFileLock {
                _descriptor: descriptor,
            })
        }

        pub(crate) fn prepare_record(
            &self,
            record_name: &WalRecordName,
            bytes: &[u8],
        ) -> Result<(), String> {
            if !matches!(record_name.phase, super::super::model::WalPhase::Preparing) {
                return Err("WriteAuthority WAL poate crea direct numai faza preparing.".into());
            }
            if bytes.is_empty() || bytes.len() > MAX_WAL_RECORD_BYTES {
                return Err(format!(
                    "WriteAuthority WAL refuză recordul de {} bytes.",
                    bytes.len()
                ));
            }
            let descriptor = fs::openat(
                self.authority.directory(),
                record_name.file_name.as_str(),
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                RECORD_MODE,
            )
            .map_err(|error| {
                format!(
                    "WriteAuthority WAL nu poate crea {} cu O_EXCL: {error}.",
                    record_name.file_name
                )
            })?;
            validate_regular_single_link(&descriptor, &record_name.file_name)?;
            let mut file = File::from(descriptor);
            if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
                drop(file);
                let cleanup = fs::unlinkat(
                    self.authority.directory(),
                    record_name.file_name.as_str(),
                    AtFlags::empty(),
                );
                let _ = sync_directory(self.authority.directory());
                return Err(format!(
                    "WriteAuthority WAL nu poate face durabil {}: {error}; cleanup={cleanup:?}.",
                    record_name.file_name
                ));
            }
            Ok(())
        }

        pub(crate) fn rename_phase(
            &self,
            current: &WalRecordName,
            next: &WalRecordName,
        ) -> Result<(), String> {
            if current.operation_id != next.operation_id || current.phase.next() != Some(next.phase)
            {
                return Err("WriteAuthority WAL a refuzat o tranziție de fază ne-monotonă.".into());
            }
            if !valid_filename_evidence_transition(current, next) {
                return Err(
                    "WriteAuthority WAL a refuzat pierderea sau schimbarea evidence-ului din filename la avansarea fazei."
                        .into(),
                );
            }
            fs::renameat_with(
                self.authority.directory(),
                current.file_name.as_str(),
                self.authority.directory(),
                next.file_name.as_str(),
                RenameFlags::NOREPLACE,
            )
            .map_err(|error| {
                format!(
                    "WriteAuthority WAL nu poate avansa {} -> {}: {error}.",
                    current.file_name, next.file_name
                )
            })?;
            sync_directory(self.authority.directory())
        }

        pub(crate) fn remove_record(&self, record_name: &WalRecordName) -> Result<(), String> {
            if take_record_remove_failure() {
                return Err(format!(
                    "WriteAuthority WAL failure injection înainte de unlink pentru {}.",
                    record_name.file_name
                ));
            }
            fs::unlinkat(
                self.authority.directory(),
                record_name.file_name.as_str(),
                AtFlags::empty(),
            )
            .map_err(|error| {
                format!(
                    "WriteAuthority WAL nu poate elimina {}: {error}.",
                    record_name.file_name
                )
            })?;
            sync_directory(self.authority.directory())
        }

        pub(crate) fn list_entries(&self) -> Result<Vec<WalRawEntry>, String> {
            let mut stream = Dir::read_from(self.authority.directory()).map_err(|error| {
                format!("WriteAuthority WAL nu poate enumera directorul: {error}.")
            })?;
            let mut names = Vec::<OsString>::new();
            while let Some(entry) = stream.read() {
                let entry = entry.map_err(|error| {
                    format!("WriteAuthority WAL enumerare întreruptă: {error}.")
                })?;
                let bytes = entry.file_name().to_bytes();
                if matches!(bytes, b"." | b"..") || bytes == WAL_LOCK_FILE.as_bytes() {
                    continue;
                }
                names.push(OsString::from_vec(bytes.to_vec()));
                if names.len() > MAX_WAL_RECORDS {
                    return Err(format!(
                        "WriteAuthority WAL depășește limita de {} recorduri.",
                        MAX_WAL_RECORDS
                    ));
                }
            }
            drop(stream);
            names.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));

            let mut total_bytes = 0_usize;
            let mut entries = Vec::with_capacity(names.len());
            for name in names {
                let display = String::from_utf8(name.as_bytes().to_vec())
                    .unwrap_or_else(|_| format!("non-utf8-{}", hex_name(name.as_bytes())));
                let bytes = self.read_bounded_entry(&name).and_then(|bytes| {
                    total_bytes = total_bytes.saturating_add(bytes.len());
                    if total_bytes > MAX_WAL_TOTAL_BYTES {
                        return Err(format!(
                            "WriteAuthority WAL depășește limita totală de {} bytes.",
                            MAX_WAL_TOTAL_BYTES
                        ));
                    }
                    Ok(bytes)
                });
                entries.push(WalRawEntry {
                    file_name: display,
                    bytes,
                });
            }
            Ok(entries)
        }

        pub(crate) fn has_record_entries(&self) -> Result<bool, String> {
            let mut stream = Dir::read_from(self.authority.directory()).map_err(|error| {
                format!("WriteAuthority WAL nu poate enumera directorul: {error}.")
            })?;
            while let Some(entry) = stream.read() {
                let entry = entry.map_err(|error| {
                    format!("WriteAuthority WAL enumerare întreruptă: {error}.")
                })?;
                let bytes = entry.file_name().to_bytes();
                if matches!(bytes, b"." | b"..") || bytes == WAL_LOCK_FILE.as_bytes() {
                    continue;
                }
                // Gate-ul rapid trebuie doar să dovedească dacă WAL-ul este
                // hot. Prima intrare oprește scanarea; scanarea detaliată
                // aplică separat limitele de recorduri și bytes.
                return Ok(true);
            }
            Ok(false)
        }

        fn read_bounded_entry(&self, name: &OsStr) -> Result<Vec<u8>, String> {
            self.read_bounded_entry_with_stat(name, false)
                .map(|(bytes, _)| bytes)
        }

        fn read_bounded_entry_with_stat(
            &self,
            name: &OsStr,
            require_mode_0600: bool,
        ) -> Result<(Vec<u8>, fs::Stat), String> {
            let descriptor = fs::openat(
                self.authority.directory(),
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|error| format!("WAL entry nu poate fi deschisă sigur: {error}."))?;
            let stat = validate_regular_single_link(&descriptor, &name.to_string_lossy())?;
            if require_mode_0600 && stat.st_mode & 0o7777 != 0o600 {
                return Err(format!(
                    "WAL entry {} nu are mode 0600.",
                    name.to_string_lossy()
                ));
            }
            if stat.st_size < 0 || stat.st_size as usize > MAX_WAL_RECORD_BYTES {
                return Err(format!(
                    "WAL entry {} are dimensiunea invalidă {}.",
                    name.to_string_lossy(),
                    stat.st_size
                ));
            }
            let mut bytes = Vec::with_capacity(stat.st_size as usize);
            let mut file = File::from(descriptor);
            std::io::Read::by_ref(&mut file)
                .take((MAX_WAL_RECORD_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|error| format!("WAL entry nu poate fi citită: {error}."))?;
            if bytes.len() > MAX_WAL_RECORD_BYTES {
                return Err("WAL entry a crescut peste limită în timpul citirii.".into());
            }
            let final_stat = fs::fstat(&file).map_err(|error| {
                format!(
                    "WAL entry {} final stat a eșuat: {error}.",
                    name.to_string_lossy()
                )
            })?;
            if stat.st_dev != final_stat.st_dev
                || stat.st_ino != final_stat.st_ino
                || stat.st_size != final_stat.st_size
                || stat.st_mtime != final_stat.st_mtime
                || stat.st_mtime_nsec != final_stat.st_mtime_nsec
                || stat.st_ctime != final_stat.st_ctime
                || stat.st_ctime_nsec != final_stat.st_ctime_nsec
            {
                return Err(format!(
                    "WAL entry {} s-a schimbat în timpul citirii.",
                    name.to_string_lossy()
                ));
            }
            Ok((bytes, final_stat))
        }
    }

    fn open_or_create_lock(parent: &OwnedFd) -> Result<(OwnedFd, bool), String> {
        let existing_flags = OFlags::RDWR | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
        match fs::openat(parent, WAL_LOCK_FILE, existing_flags, Mode::empty()) {
            Ok(descriptor) => Ok((descriptor, false)),
            Err(Errno::NOENT) => match fs::openat(
                parent,
                WAL_LOCK_FILE,
                existing_flags | OFlags::CREATE | OFlags::EXCL,
                LOCK_MODE,
            ) {
                Ok(descriptor) => Ok((descriptor, true)),
                Err(Errno::EXIST) => {
                    fs::openat(parent, WAL_LOCK_FILE, existing_flags, Mode::empty())
                        .map(|descriptor| (descriptor, false))
                        .map_err(|error| format!("WAL lock race nu poate fi recapturat: {error}."))
                }
                Err(error) => Err(format!("WAL lock nu poate fi creat: {error}.")),
            },
            Err(error) => Err(format!("WAL lock nu poate fi deschis: {error}.")),
        }
    }

    fn validate_regular_single_link(descriptor: &OwnedFd, label: &str) -> Result<fs::Stat, String> {
        let stat = fs::fstat(descriptor)
            .map_err(|error| format!("WriteAuthority WAL nu poate verifica {label}: {error}."))?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile || stat.st_nlink != 1 {
            return Err(format!(
                "WriteAuthority WAL refuză {label}: cere fișier regular cu exact un link."
            ));
        }
        Ok(stat)
    }

    fn sync_directory(directory: &OwnedFd) -> Result<(), String> {
        fs::fsync(directory)
            .map_err(|error| format!("WriteAuthority WAL directory fsync a eșuat: {error}."))
    }

    fn valid_filename_evidence_transition(current: &WalRecordName, next: &WalRecordName) -> bool {
        let current_has_no_evidence = current.append_stage_checkpoint.is_none()
            && current.copy_stage_checkpoint.is_none()
            && current.directory_stage_checkpoint.is_none()
            && current.symlink_stage_checkpoint.is_none()
            && current.external_stage_checkpoint.is_none()
            && current.external_operator_decision.is_none();
        let publishes_checkpoint = current.phase == WalPhase::Prepared
            && next.phase == WalPhase::AuxiliaryDurable
            && current_has_no_evidence
            && matches!(
                (
                    next.append_stage_checkpoint.is_some(),
                    next.copy_stage_checkpoint.is_some(),
                    next.directory_stage_checkpoint.is_some(),
                    next.symlink_stage_checkpoint.is_some(),
                    next.external_stage_checkpoint.is_some(),
                    next.external_operator_decision.is_some(),
                ),
                (true, false, false, false, false, false)
                    | (false, true, false, false, false, false)
                    | (false, false, true, false, false, false)
                    | (false, false, false, true, false, false)
                    | (false, false, false, false, true, false)
            );
        let preserves_filename_evidence = current.append_stage_checkpoint
            == next.append_stage_checkpoint
            && current.copy_stage_checkpoint == next.copy_stage_checkpoint
            && current.directory_stage_checkpoint == next.directory_stage_checkpoint
            && current.symlink_stage_checkpoint == next.symlink_stage_checkpoint
            && current.external_stage_checkpoint == next.external_stage_checkpoint
            && current.external_operator_decision == next.external_operator_decision;
        publishes_checkpoint || preserves_filename_evidence
    }

    fn hex_name(bytes: &[u8]) -> String {
        let mut encoded = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            use std::fmt::Write as _;
            let _ = write!(encoded, "{byte:02x}");
        }
        encoded
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::kernel::write_authority::recovery::paths::{
            WalDirectoryStageCheckpoint, WalSymlinkStageCheckpoint,
        };

        #[test]
        fn directory_checkpoint_must_be_published_once_and_preserved_exactly() {
            let prepared = WalRecordName::new("directory-transition", WalPhase::Prepared).unwrap();
            let checkpoint =
                WalDirectoryStageCheckpoint::new("a".repeat(32), "b".repeat(32)).unwrap();
            let auxiliary = WalRecordName::with_directory_stage_checkpoint(
                "directory-transition",
                WalPhase::AuxiliaryDurable,
                checkpoint,
            )
            .unwrap();
            assert!(valid_filename_evidence_transition(&prepared, &auxiliary));

            let effect = auxiliary.successor(WalPhase::EffectVisible).unwrap();
            assert!(valid_filename_evidence_transition(&auxiliary, &effect));

            let lost = WalRecordName::new("directory-transition", WalPhase::EffectVisible).unwrap();
            assert!(!valid_filename_evidence_transition(&auxiliary, &lost));
            let changed = WalRecordName::with_directory_stage_checkpoint(
                "directory-transition",
                WalPhase::EffectVisible,
                WalDirectoryStageCheckpoint::new("a".repeat(32), "c".repeat(32)).unwrap(),
            )
            .unwrap();
            assert!(!valid_filename_evidence_transition(&auxiliary, &changed));
        }

        #[test]
        fn symlink_checkpoint_must_be_published_once_and_preserved_exactly() {
            let prepared = WalRecordName::new("symlink-transition", WalPhase::Prepared).unwrap();
            let checkpoint =
                WalSymlinkStageCheckpoint::new("a".repeat(32), "b".repeat(32)).unwrap();
            let auxiliary = WalRecordName::with_symlink_stage_checkpoint(
                "symlink-transition",
                WalPhase::AuxiliaryDurable,
                checkpoint,
            )
            .unwrap();
            assert!(valid_filename_evidence_transition(&prepared, &auxiliary));

            let effect = auxiliary.successor(WalPhase::EffectVisible).unwrap();
            assert!(valid_filename_evidence_transition(&auxiliary, &effect));

            let lost = WalRecordName::new("symlink-transition", WalPhase::EffectVisible).unwrap();
            assert!(!valid_filename_evidence_transition(&auxiliary, &lost));
            let changed = WalRecordName::with_symlink_stage_checkpoint(
                "symlink-transition",
                WalPhase::EffectVisible,
                WalSymlinkStageCheckpoint::new("a".repeat(32), "c".repeat(32)).unwrap(),
            )
            .unwrap();
            assert!(!valid_filename_evidence_transition(&auxiliary, &changed));
        }
    }
}

#[cfg(target_os = "linux")]
pub(crate) use platform::*;

#[cfg(not(target_os = "linux"))]
mod unsupported {
    use crate::kernel::write_authority::root_authority::DirectoryAuthority;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum WalLockMode {
        Exclusive,
    }

    pub(crate) struct WalFileLock;

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub(crate) struct WalRawEntry {
        pub file_name: String,
        pub bytes: Result<Vec<u8>, String>,
    }

    #[derive(Clone, Debug)]
    pub(crate) struct WalDirectory;

    impl WalDirectory {
        pub(crate) fn new(_authority: DirectoryAuthority) -> Self {
            Self
        }

        pub(crate) fn lock(&self, _mode: WalLockMode) -> Result<WalFileLock, String> {
            Err("WriteAuthority WAL este fail-closed în afara Linux.".into())
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) use unsupported::*;
