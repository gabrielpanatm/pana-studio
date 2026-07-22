use std::fs;
use std::path::{Component, Path, PathBuf};

const MUTATING_PATTERNS: &[&str] = &[
    "fs::write",
    "std::fs::write",
    "fs::create_dir_all",
    "std::fs::create_dir_all",
    "fs::rename",
    "std::fs::rename",
    "fs::remove_file",
    "std::fs::remove_file",
    "fs::remove_dir_all",
    "std::fs::remove_dir_all",
    "fs::remove_dir",
    "std::fs::remove_dir",
    "fs::copy",
    "std::fs::copy",
    "fs::set_permissions",
    "std::fs::set_permissions",
    ".set_permissions(",
    "File::create",
    "OpenOptions::new",
];

const LOW_LEVEL_CAPABILITY_PRIMITIVES: &[&str] = &[
    "openat",
    "openat2",
    "mkdirat",
    "unlinkat",
    "renameat",
    "renameat_with",
    "symlinkat",
];

const CAPABILITY_BACKEND_FILE: &str = "kernel/write_authority/capability.rs";
const CAPABILITY_LIFECYCLE_BACKEND_FILE: &str =
    "kernel/write_authority/capability/platform/lifecycle.rs";
const CAPABILITY_COPY_BACKEND_FILE: &str = "kernel/write_authority/capability/platform/copy.rs";
const CAPABILITY_COPY_RECOVERY_BACKEND_FILE: &str =
    "kernel/write_authority/capability/platform/copy/recovery.rs";
const CAPABILITY_DIRECTORY_BACKEND_FILE: &str =
    "kernel/write_authority/capability/platform/directory.rs";
const CAPABILITY_DIRECTORY_RECOVERY_BACKEND_FILE: &str =
    "kernel/write_authority/capability/platform/directory/recovery.rs";
const CAPABILITY_SYMLINK_BACKEND_FILE: &str =
    "kernel/write_authority/capability/platform/symlink.rs";
const CAPABILITY_SYMLINK_RECOVERY_BACKEND_FILE: &str =
    "kernel/write_authority/capability/platform/symlink/recovery.rs";
const CAPABILITY_ANONYMOUS_FILE_BACKEND_FILE: &str =
    "kernel/write_authority/capability/platform/anonymous_file.rs";
const CAPABILITY_APPEND_BACKEND_FILE: &str = "kernel/write_authority/capability/platform/append.rs";
const CAPABILITY_APPEND_RECOVERY_BACKEND_FILE: &str =
    "kernel/write_authority/capability/platform/append/recovery.rs";
const CAPABILITY_EXTERNAL_CONFIG_BACKEND_FILE: &str =
    "kernel/write_authority/capability/platform/external_config.rs";
const CAPABILITY_EXTERNAL_CONFIG_SNAPSHOT_BACKEND_FILE: &str =
    "kernel/write_authority/capability/platform/external_config/snapshot.rs";
const CAPABILITY_EXTERNAL_CONFIG_RECOVERY_BACKEND_FILE: &str =
    "kernel/write_authority/capability/platform/external_config/recovery.rs";
const CAPABILITY_EXTERNAL_CONFIG_RECOVERY_SNAPSHOT_BACKEND_FILE: &str =
    "kernel/write_authority/capability/platform/external_config/recovery/snapshot.rs";
const CAPABILITY_RENAME_BACKEND_FILE: &str = "kernel/write_authority/capability/platform/rename.rs";
const CAPABILITY_REMOVE_BACKEND_FILE: &str = "kernel/write_authority/capability/platform/remove.rs";
const CAPABILITY_REMOVE_TREE_BACKEND_FILE: &str =
    "kernel/write_authority/capability/platform/remove_tree.rs";
const CAPABILITY_REMOVE_TREE_RECOVERY_BACKEND_FILE: &str =
    "kernel/write_authority/capability/platform/remove_tree/recovery.rs";
const CAPABILITY_REMOVE_TREE_SNAPSHOT_BACKEND_FILE: &str =
    "kernel/write_authority/capability/platform/remove_tree/snapshot.rs";
const CAPABILITY_REMOVE_TREE_TRAVERSAL_BACKEND_FILE: &str =
    "kernel/write_authority/capability/platform/remove_tree/traversal.rs";
const TREE_FINGERPRINT_BACKEND_FILE: &str = "kernel/write_authority/tree_fingerprint.rs";
const WAL_IO_BACKEND_FILE: &str = "kernel/write_authority/recovery/wal_io.rs";
const COMPLIANCE_SCANNER_FILE: &str = "kernel/write_authority/compliance.rs";
const CAPABILITY_ADAPTER_DEFINITION_FILE: &str = "kernel/write_authority/mod.rs";
const ROOT_AUTHORITY_DEFINITION_FILE: &str = "kernel/write_authority/root_authority.rs";
const BOUNDED_JOURNAL_READER_FILE: &str = "kernel/bounded_journal_reader.rs";

// This is deliberately separate from the write-capability/WAL trust-base.
// The reader owns stable, no-follow journal reads and may only use this small
// read-only rustix surface. It is intentionally not added to
// DECLARED_WRITE_RUNTIME_FILES, so the mutation scanner still audits it.
const READ_ONLY_LOW_LEVEL_TRUST_FILES: &[&str] = &[BOUNDED_JOURNAL_READER_FILE];
const READ_ONLY_RUSTIX_FS_IMPORT: &str =
    "userustix::fs::{selfasrustix_fs,FileType,FlockOperation,Mode,OFlags};";
const READ_ONLY_RUSTIX_FS_PRIMITIVES: &[&str] = &["open", "openat", "flock", "fstat", "Stat"];
const READ_ONLY_OFLAG_MEMBERS: &[&str] =
    &["RDONLY", "DIRECTORY", "NOFOLLOW", "NONBLOCK", "CLOEXEC"];
const READ_ONLY_FLOCK_MEMBERS: &[&str] = &["LockShared", "LockExclusive"];
const READ_ONLY_MODE_MEMBERS: &[&str] = &["empty"];
const READ_ONLY_FILE_TYPE_MEMBERS: &[&str] = &["from_raw_mode", "RegularFile"];

const SCOPED_AUTHORITY_ISSUERS: &[(&str, &[&str])] = &[
    ("ProjectBootstrapLease::capture", &["project/init.rs"]),
    ("CodexConfigLease::capture", &["mcp/codex.rs"]),
];

const MAINTENANCE_CAPABILITY_ADAPTERS: &[(&str, &[&str])] = &[
    (
        "capability_append_observability_file",
        &["kernel/observability/mod.rs"],
    ),
    (
        "capability_lock_observability_file",
        &[
            "kernel/observability/mod.rs",
            "kernel/observability/reader.rs",
        ],
    ),
    (
        "capability_remove_observability_file",
        &["kernel/observability/retention.rs"],
    ),
    (
        "capability_rename_observability_file",
        &["kernel/observability/retention.rs"],
    ),
    (
        "capability_capture_subprocess_directory",
        &["project/init.rs"],
    ),
    ("ZolaArtifactPublicationLease", &["deploy/zola.rs"]),
];

const DECLARED_WRITE_RUNTIME_FILES: &[&str] = &[
    "kernel/write_authority/capability.rs",
    "kernel/write_authority/capability/platform/anonymous_file.rs",
    "kernel/write_authority/capability/platform/append.rs",
    "kernel/write_authority/capability/platform/append/recovery.rs",
    "kernel/write_authority/capability/platform/copy.rs",
    "kernel/write_authority/capability/platform/copy/recovery.rs",
    "kernel/write_authority/capability/platform/directory.rs",
    "kernel/write_authority/capability/platform/directory/recovery.rs",
    "kernel/write_authority/capability/platform/symlink.rs",
    "kernel/write_authority/capability/platform/symlink/recovery.rs",
    "kernel/write_authority/capability/platform/external_config.rs",
    "kernel/write_authority/capability/platform/external_config/snapshot.rs",
    "kernel/write_authority/capability/platform/external_config/recovery.rs",
    "kernel/write_authority/capability/platform/external_config/recovery/snapshot.rs",
    "kernel/write_authority/capability/platform/lifecycle.rs",
    "kernel/write_authority/capability/platform/rename.rs",
    "kernel/write_authority/capability/platform/remove.rs",
    "kernel/write_authority/capability/platform/remove_tree.rs",
    "kernel/write_authority/capability/platform/remove_tree/recovery.rs",
    "kernel/write_authority/capability/platform/remove_tree/snapshot.rs",
    "kernel/write_authority/capability/platform/remove_tree/traversal.rs",
    "kernel/write_authority/tree_fingerprint.rs",
    "kernel/write_authority/compliance.rs",
    "kernel/write_authority/recovery/wal_io.rs",
];

// BLK-P0-004 removed the former Application Home and observability ambient
// exceptions. Keep the empty inventory executable so a future exception must
// be introduced deliberately and will fail the test below until documented.
const LEGACY_AMBIENT_RUNTIME_EXCEPTIONS: &[&str] = &[];

const ALLOWED_POLICY_LITERAL_FILES: &[&str] = &[
    "kernel/write_authority/model.rs",
    "kernel/write_authority/compliance.rs",
];

#[test]
fn runtime_filesystem_mutations_stay_in_declared_authorities() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source_root = manifest_dir.join("src");
    let mut violations = Vec::new();

    scan_dir(&source_root, &source_root, &mut violations);

    assert!(
        violations.is_empty(),
        "runtime filesystem mutations must go through declared write authorities:\n{}",
        violations.join("\n")
    );
}

#[test]
fn low_level_filesystem_primitives_stay_in_capability_backend() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source_root = manifest_dir.join("src");
    let mut violations = Vec::new();

    scan_low_level_primitive_dir(&source_root, &source_root, &mut violations);

    assert!(
        violations.is_empty(),
        "low-level rustix filesystem primitives are restricted to the declared capability/WAL trust-base (root {CAPABILITY_BACKEND_FILE}):\n{}",
        violations.join("\n")
    );
}

#[test]
fn maintenance_capability_adapters_have_only_bounded_callsites() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source_root = manifest_dir.join("src");
    let mut violations = Vec::new();

    scan_maintenance_adapter_dir(&source_root, &source_root, &mut violations);

    assert!(
        violations.is_empty(),
        "bootstrap/observability capability adapters have bounded callsites:\n{}",
        violations.join("\n")
    );
}

#[test]
fn scoped_authority_issuers_have_only_bounded_runtime_callsites() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source_root = manifest_dir.join("src");
    let mut violations = Vec::new();

    scan_scoped_authority_issuer_dir(&source_root, &source_root, &mut violations);

    assert!(
        violations.is_empty(),
        "sealed filesystem authority issuers have bounded runtime callsites:\n{}",
        violations.join("\n")
    );
}

#[test]
fn write_policies_are_named_in_write_policy_model() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source_root = manifest_dir.join("src");
    let mut violations = Vec::new();

    scan_policy_literal_dir(&source_root, &source_root, &mut violations);

    assert!(
        violations.is_empty(),
        "runtime write policies must be named constructors on WritePolicy, not inline literals:\n{}",
        violations.join("\n")
    );
}

fn scan_dir(source_root: &Path, dir: &Path, violations: &mut Vec<String>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("read source directory {}: {error}", dir.display()));

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("read source entry: {error}"));
        let path = entry.path();
        if path.is_dir() {
            scan_dir(source_root, &path, violations);
            continue;
        }

        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }

        if is_test_file(source_root, &path) || is_allowed_runtime_file(source_root, &path) {
            continue;
        }

        scan_file(source_root, &path, violations);
    }
}

fn scan_low_level_primitive_dir(source_root: &Path, dir: &Path, violations: &mut Vec<String>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("read source directory {}: {error}", dir.display()));

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("read source entry: {error}"));
        let path = entry.path();
        if path.is_dir() {
            scan_low_level_primitive_dir(source_root, &path, violations);
            continue;
        }

        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }

        if is_test_file(source_root, &path)
            || is_compliance_scanner_file(source_root, &path)
            || is_low_level_backend_file(source_root, &path)
        {
            continue;
        }

        if is_read_only_low_level_trust_file(source_root, &path) {
            scan_read_only_low_level_file(source_root, &path, violations);
            continue;
        }

        scan_low_level_primitive_file(source_root, &path, violations);
    }
}

fn scan_maintenance_adapter_dir(source_root: &Path, dir: &Path, violations: &mut Vec<String>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("read source directory {}: {error}", dir.display()));

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("read source entry: {error}"));
        let path = entry.path();
        if path.is_dir() {
            scan_maintenance_adapter_dir(source_root, &path, violations);
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("rs")
            || is_test_file(source_root, &path)
            || is_compliance_scanner_file(source_root, &path)
        {
            continue;
        }

        let relative = relative_path(source_root, &path);
        let contents = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read source file {}: {error}", path.display()));
        for (adapter, allowed_files) in MAINTENANCE_CAPABILITY_ADAPTERS {
            if contents.contains(adapter)
                && relative != CAPABILITY_ADAPTER_DEFINITION_FILE
                && !allowed_files.iter().any(|allowed| relative == *allowed)
            {
                violations.push(format!(
                    "{relative} references maintenance adapter `{adapter}` outside {:?}",
                    allowed_files
                ));
            }
        }
    }
}

fn scan_scoped_authority_issuer_dir(source_root: &Path, dir: &Path, violations: &mut Vec<String>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("read source directory {}: {error}", dir.display()));

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("read source entry: {error}"));
        let path = entry.path();
        if path.is_dir() {
            scan_scoped_authority_issuer_dir(source_root, &path, violations);
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("rs")
            || is_test_file(source_root, &path)
            || is_compliance_scanner_file(source_root, &path)
            || is_allowed_file(source_root, &path, &[ROOT_AUTHORITY_DEFINITION_FILE])
        {
            continue;
        }

        let relative = relative_path(source_root, &path);
        let contents = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read source file {}: {error}", path.display()));
        let runtime_source = without_whitespace(&source_without_cfg_test_blocks(&contents));
        for (issuer, allowed_files) in SCOPED_AUTHORITY_ISSUERS {
            if runtime_source.contains(issuer)
                && !allowed_files.iter().any(|allowed| relative == *allowed)
            {
                violations.push(format!(
                    "{relative} calls sealed issuer `{issuer}` outside {:?}",
                    allowed_files
                ));
            }
        }
    }
}

fn scan_policy_literal_dir(source_root: &Path, dir: &Path, violations: &mut Vec<String>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("read source directory {}: {error}", dir.display()));

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("read source entry: {error}"));
        let path = entry.path();
        if path.is_dir() {
            scan_policy_literal_dir(source_root, &path, violations);
            continue;
        }

        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }

        if is_test_file(source_root, &path)
            || is_allowed_file(source_root, &path, ALLOWED_POLICY_LITERAL_FILES)
        {
            continue;
        }

        scan_policy_literal_file(source_root, &path, violations);
    }
}

fn scan_file(source_root: &Path, path: &Path, violations: &mut Vec<String>) {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read source file {}: {error}", path.display()));

    let mut pending_cfg_test = false;
    let mut skipping_cfg_test_block = false;
    let mut cfg_test_depth = 0isize;

    for (index, line) in contents.lines().enumerate() {
        if skipping_cfg_test_block {
            cfg_test_depth += brace_delta(line);
            if cfg_test_depth <= 0 {
                skipping_cfg_test_block = false;
                cfg_test_depth = 0;
            }
            continue;
        }

        let trimmed = line.trim();
        if pending_cfg_test {
            if trimmed.is_empty() || trimmed.starts_with("#[") {
                continue;
            }

            if trimmed.contains('{') {
                skipping_cfg_test_block = true;
                cfg_test_depth = brace_delta(line);
                if cfg_test_depth <= 0 {
                    skipping_cfg_test_block = false;
                    cfg_test_depth = 0;
                }
                pending_cfg_test = false;
                continue;
            }

            pending_cfg_test = !trimmed.ends_with(';');
            continue;
        }

        if is_cfg_test_attribute(trimmed) {
            pending_cfg_test = true;
            continue;
        }

        if let Some(pattern) = mutating_pattern(trimmed) {
            violations.push(format!(
                "{}:{} uses `{}`: {}",
                relative_path(source_root, path),
                index + 1,
                pattern,
                trimmed
            ));
        }
    }
}

fn scan_low_level_primitive_file(source_root: &Path, path: &Path, violations: &mut Vec<String>) {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read source file {}: {error}", path.display()));

    let mut pending_cfg_test = false;
    let mut skipping_cfg_test_block = false;
    let mut cfg_test_depth = 0isize;
    let mut pending_use: Option<(usize, String)> = None;

    for (index, line) in contents.lines().enumerate() {
        if skipping_cfg_test_block {
            cfg_test_depth += brace_delta(line);
            if cfg_test_depth <= 0 {
                skipping_cfg_test_block = false;
                cfg_test_depth = 0;
            }
            continue;
        }

        let trimmed = line.trim();
        if pending_cfg_test {
            if trimmed.is_empty() || trimmed.starts_with("#[") {
                continue;
            }

            if trimmed.contains('{') {
                skipping_cfg_test_block = true;
                cfg_test_depth = brace_delta(line);
                if cfg_test_depth <= 0 {
                    skipping_cfg_test_block = false;
                    cfg_test_depth = 0;
                }
                pending_cfg_test = false;
                continue;
            }

            pending_cfg_test = !trimmed.ends_with(';');
            continue;
        }

        if is_cfg_test_attribute(trimmed) {
            pending_cfg_test = true;
            continue;
        }

        let code = line_before_comment(trimmed);
        if let Some((start_line, statement)) = pending_use.as_mut() {
            statement.push(' ');
            statement.push_str(code);
            if code.contains(';') {
                if let Some(primitive) = low_level_import_primitive(statement) {
                    violations.push(format!(
                        "{}:{} imports low-level `{}` outside {}: {}",
                        relative_path(source_root, path),
                        *start_line,
                        primitive,
                        CAPABILITY_BACKEND_FILE,
                        statement.trim()
                    ));
                }
                pending_use = None;
            }
            continue;
        }

        if starts_use_statement(code) {
            if code.contains(';') {
                if let Some(primitive) = low_level_import_primitive(code) {
                    violations.push(format!(
                        "{}:{} imports low-level `{}` outside {}: {}",
                        relative_path(source_root, path),
                        index + 1,
                        primitive,
                        CAPABILITY_BACKEND_FILE,
                        code
                    ));
                }
            } else {
                pending_use = Some((index + 1, code.to_string()));
            }
            continue;
        }

        if let Some(primitive) = low_level_call_primitive(code) {
            violations.push(format!(
                "{}:{} uses low-level `{}` outside {}: {}",
                relative_path(source_root, path),
                index + 1,
                primitive,
                CAPABILITY_BACKEND_FILE,
                code
            ));
        }
    }
}

fn scan_read_only_low_level_file(source_root: &Path, path: &Path, violations: &mut Vec<String>) {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read source file {}: {error}", path.display()));
    let runtime_source = source_without_cfg_test_blocks(&contents);
    let relative = relative_path(source_root, path);
    let mut pending_use: Option<(usize, String)> = None;

    for (index, line) in runtime_source.lines().enumerate() {
        let code = line_before_comment(line.trim());
        if let Some((start_line, statement)) = pending_use.as_mut() {
            statement.push(' ');
            statement.push_str(code);
            if code.contains(';') {
                validate_read_only_low_level_import(&relative, *start_line, statement, violations);
                pending_use = None;
            }
            continue;
        }

        if starts_use_statement(code) {
            if code.contains(';') {
                validate_read_only_low_level_import(&relative, index + 1, code, violations);
            } else {
                pending_use = Some((index + 1, code.to_string()));
            }
            continue;
        }

        validate_qualified_members(
            &relative,
            index + 1,
            code,
            "rustix_fs::",
            READ_ONLY_RUSTIX_FS_PRIMITIVES,
            "rustix::fs primitive",
            violations,
        );
        validate_qualified_members(
            &relative,
            index + 1,
            code,
            "rustix::fs::",
            READ_ONLY_RUSTIX_FS_PRIMITIVES,
            "rustix::fs primitive",
            violations,
        );
        validate_qualified_members(
            &relative,
            index + 1,
            code,
            "OFlags::",
            READ_ONLY_OFLAG_MEMBERS,
            "OFlags member",
            violations,
        );
        validate_qualified_members(
            &relative,
            index + 1,
            code,
            "FlockOperation::",
            READ_ONLY_FLOCK_MEMBERS,
            "flock operation",
            violations,
        );
        validate_qualified_members(
            &relative,
            index + 1,
            code,
            "Mode::",
            READ_ONLY_MODE_MEMBERS,
            "Mode member",
            violations,
        );
        validate_qualified_members(
            &relative,
            index + 1,
            code,
            "FileType::",
            READ_ONLY_FILE_TYPE_MEMBERS,
            "FileType member",
            violations,
        );
    }

    if let Some((start_line, statement)) = pending_use {
        violations.push(format!(
            "{relative}:{start_line} has unterminated use statement in read-only trust-base: {}",
            statement.trim()
        ));
    }
}

fn validate_read_only_low_level_import(
    relative: &str,
    line: usize,
    statement: &str,
    violations: &mut Vec<String>,
) {
    let Some(import_body) = use_statement_body(statement) else {
        return;
    };
    if !contains_identifier(import_body, "rustix") || !contains_identifier(import_body, "fs") {
        return;
    }

    let compact = without_whitespace(statement);
    if compact != READ_ONLY_RUSTIX_FS_IMPORT {
        violations.push(format!(
            "{relative}:{line} imports rustix::fs outside the exact read-only surface: {}",
            statement.trim()
        ));
    }
}

fn validate_qualified_members(
    relative: &str,
    line: usize,
    code: &str,
    prefix: &str,
    allowed: &[&str],
    kind: &str,
    violations: &mut Vec<String>,
) {
    let compact = without_whitespace(code);
    let mut remainder = compact.as_str();

    while let Some(start) = remainder.find(prefix) {
        let after_prefix = &remainder[start + prefix.len()..];
        let member = after_prefix
            .chars()
            .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
            .collect::<String>();
        if member.is_empty() || !allowed.iter().any(|allowed| member == *allowed) {
            violations.push(format!(
                "{relative}:{line} uses disallowed {kind} `{prefix}{member}` in read-only trust-base: {}",
                code.trim()
            ));
        }
        remainder = after_prefix.get(member.len()..).unwrap_or_default();
    }
}

fn scan_policy_literal_file(source_root: &Path, path: &Path, violations: &mut Vec<String>) {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read source file {}: {error}", path.display()));

    let mut pending_cfg_test = false;
    let mut skipping_cfg_test_block = false;
    let mut cfg_test_depth = 0isize;

    for (index, line) in contents.lines().enumerate() {
        if skipping_cfg_test_block {
            cfg_test_depth += brace_delta(line);
            if cfg_test_depth <= 0 {
                skipping_cfg_test_block = false;
                cfg_test_depth = 0;
            }
            continue;
        }

        let trimmed = line.trim();
        if pending_cfg_test {
            if trimmed.is_empty() || trimmed.starts_with("#[") {
                continue;
            }

            if trimmed.contains('{') {
                skipping_cfg_test_block = true;
                cfg_test_depth = brace_delta(line);
                if cfg_test_depth <= 0 {
                    skipping_cfg_test_block = false;
                    cfg_test_depth = 0;
                }
                pending_cfg_test = false;
                continue;
            }

            pending_cfg_test = !trimmed.ends_with(';');
            continue;
        }

        if is_cfg_test_attribute(trimmed) {
            pending_cfg_test = true;
            continue;
        }

        if trimmed.starts_with("WritePolicy {") {
            violations.push(format!(
                "{}:{} uses inline `WritePolicy`: {}",
                relative_path(source_root, path),
                index + 1,
                trimmed
            ));
        }
    }
}

fn is_cfg_test_attribute(trimmed: &str) -> bool {
    trimmed.starts_with("#[cfg(") && trimmed.contains("test")
}

fn source_without_cfg_test_blocks(contents: &str) -> String {
    let mut runtime_source = String::new();
    let mut pending_cfg_test = false;
    let mut skipping_cfg_test_block = false;
    let mut cfg_test_depth = 0isize;

    for line in contents.lines() {
        if skipping_cfg_test_block {
            cfg_test_depth += brace_delta(line);
            if cfg_test_depth <= 0 {
                skipping_cfg_test_block = false;
                cfg_test_depth = 0;
            }
            continue;
        }

        let trimmed = line.trim();
        if pending_cfg_test {
            if trimmed.is_empty() || trimmed.starts_with("#[") {
                continue;
            }
            if trimmed.contains('{') {
                skipping_cfg_test_block = true;
                cfg_test_depth = brace_delta(line);
                if cfg_test_depth <= 0 {
                    skipping_cfg_test_block = false;
                    cfg_test_depth = 0;
                }
                pending_cfg_test = false;
                continue;
            }
            pending_cfg_test = !trimmed.ends_with(';');
            continue;
        }

        if is_cfg_test_attribute(trimmed) {
            pending_cfg_test = true;
            continue;
        }

        runtime_source.push_str(line_before_comment(line));
        runtime_source.push('\n');
    }

    runtime_source
}

fn mutating_pattern(trimmed: &str) -> Option<&'static str> {
    MUTATING_PATTERNS
        .iter()
        .copied()
        .find(|pattern| trimmed.contains(pattern))
}

fn low_level_call_primitive(code: &str) -> Option<&'static str> {
    let compact = without_whitespace(code);
    LOW_LEVEL_CAPABILITY_PRIMITIVES
        .iter()
        .copied()
        .find(|primitive| {
            contains_qualified_primitive(&compact, primitive)
                || contains_identifier_call(&compact, primitive)
        })
        .or_else(|| compact.contains("rustix::fs::").then_some("rustix::fs API"))
}

fn contains_qualified_primitive(text: &str, primitive: &str) -> bool {
    let qualified = format!("rustix::fs::{primitive}");
    text.match_indices(&qualified).any(|(start, _)| {
        let after = start + qualified.len();
        !text
            .as_bytes()
            .get(after)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    })
}

fn low_level_import_primitive(statement: &str) -> Option<&'static str> {
    let import_body = use_statement_body(statement)?;
    if !contains_identifier(import_body, "rustix") || !contains_identifier(import_body, "fs") {
        return None;
    }

    LOW_LEVEL_CAPABILITY_PRIMITIVES
        .iter()
        .copied()
        .find(|primitive| contains_identifier(import_body, primitive))
        .or_else(|| imports_rustix_fs_wildcard(import_body).then_some("rustix::fs::*"))
        .or(Some("rustix::fs API"))
}

fn imports_rustix_fs_wildcard(import_body: &str) -> bool {
    let compact = without_whitespace(import_body);
    compact.contains("rustix::fs::*")
        || wildcard_in_group_after(&compact, "rustix::fs::{")
        || compact
            .strip_prefix("rustix::{")
            .is_some_and(|group| group.contains("fs::*") || wildcard_in_group_after(group, "fs::{"))
}

fn wildcard_in_group_after(value: &str, prefix: &str) -> bool {
    value.find(prefix).is_some_and(|start| {
        value[start + prefix.len()..]
            .split('}')
            .next()
            .is_some_and(|group| group.contains('*'))
    })
}

fn contains_identifier_call(text: &str, identifier: &str) -> bool {
    text.match_indices(identifier).any(|(start, _)| {
        identifier_boundaries_match(text, start, identifier.len())
            && text.as_bytes().get(start + identifier.len()) == Some(&b'(')
    })
}

fn contains_identifier(text: &str, identifier: &str) -> bool {
    text.match_indices(identifier)
        .any(|(start, _)| identifier_boundaries_match(text, start, identifier.len()))
}

fn identifier_boundaries_match(text: &str, start: usize, length: usize) -> bool {
    let bytes = text.as_bytes();
    let before_is_identifier = start
        .checked_sub(1)
        .and_then(|index| bytes.get(index))
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_');
    let after_is_identifier = bytes
        .get(start + length)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_');
    !before_is_identifier && !after_is_identifier
}

fn without_whitespace(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn starts_use_statement(code: &str) -> bool {
    let trimmed = code.trim_start();
    trimmed.starts_with("use ") || (trimmed.starts_with("pub") && trimmed.find(" use ").is_some())
}

fn use_statement_body(statement: &str) -> Option<&str> {
    let statement = statement.trim_start();
    if let Some(body) = statement.strip_prefix("use ") {
        return Some(body);
    }
    if statement.starts_with("pub") {
        return statement
            .find(" use ")
            .map(|index| &statement[index + " use ".len()..]);
    }
    None
}

fn line_before_comment(line: &str) -> &str {
    line.split_once("//")
        .map(|(code, _)| code.trim_end())
        .unwrap_or(line)
}

fn is_test_file(source_root: &Path, path: &Path) -> bool {
    let relative = path.strip_prefix(source_root).unwrap_or(path);
    relative.components().any(|component| {
        matches!(
            component,
            Component::Normal(value) if value == "tests" || value == "tests.rs"
        )
    })
}

fn is_low_level_backend_file(source_root: &Path, path: &Path) -> bool {
    is_allowed_file(
        source_root,
        path,
        &[
            CAPABILITY_BACKEND_FILE,
            CAPABILITY_ANONYMOUS_FILE_BACKEND_FILE,
            CAPABILITY_APPEND_BACKEND_FILE,
            CAPABILITY_APPEND_RECOVERY_BACKEND_FILE,
            CAPABILITY_COPY_BACKEND_FILE,
            CAPABILITY_COPY_RECOVERY_BACKEND_FILE,
            CAPABILITY_DIRECTORY_BACKEND_FILE,
            CAPABILITY_DIRECTORY_RECOVERY_BACKEND_FILE,
            CAPABILITY_SYMLINK_BACKEND_FILE,
            CAPABILITY_SYMLINK_RECOVERY_BACKEND_FILE,
            CAPABILITY_EXTERNAL_CONFIG_BACKEND_FILE,
            CAPABILITY_EXTERNAL_CONFIG_SNAPSHOT_BACKEND_FILE,
            CAPABILITY_EXTERNAL_CONFIG_RECOVERY_BACKEND_FILE,
            CAPABILITY_EXTERNAL_CONFIG_RECOVERY_SNAPSHOT_BACKEND_FILE,
            CAPABILITY_LIFECYCLE_BACKEND_FILE,
            CAPABILITY_RENAME_BACKEND_FILE,
            CAPABILITY_REMOVE_BACKEND_FILE,
            CAPABILITY_REMOVE_TREE_BACKEND_FILE,
            CAPABILITY_REMOVE_TREE_RECOVERY_BACKEND_FILE,
            CAPABILITY_REMOVE_TREE_SNAPSHOT_BACKEND_FILE,
            CAPABILITY_REMOVE_TREE_TRAVERSAL_BACKEND_FILE,
            TREE_FINGERPRINT_BACKEND_FILE,
            WAL_IO_BACKEND_FILE,
        ],
    )
}

fn is_compliance_scanner_file(source_root: &Path, path: &Path) -> bool {
    is_allowed_file(source_root, path, &[COMPLIANCE_SCANNER_FILE])
}

fn is_read_only_low_level_trust_file(source_root: &Path, path: &Path) -> bool {
    is_allowed_file(source_root, path, READ_ONLY_LOW_LEVEL_TRUST_FILES)
}

fn is_allowed_runtime_file(source_root: &Path, path: &Path) -> bool {
    is_allowed_file(source_root, path, DECLARED_WRITE_RUNTIME_FILES)
        || is_allowed_file(source_root, path, LEGACY_AMBIENT_RUNTIME_EXCEPTIONS)
}

fn is_allowed_file(source_root: &Path, path: &Path, allowed_files: &[&str]) -> bool {
    let relative = relative_path(source_root, path);
    allowed_files.iter().any(|allowed| relative == *allowed)
}

fn relative_path(source_root: &Path, path: &Path) -> String {
    path.strip_prefix(source_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn brace_delta(line: &str) -> isize {
    line.chars().fold(0, |depth, character| match character {
        '{' => depth + 1,
        '}' => depth - 1,
        _ => depth,
    })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        is_low_level_backend_file, low_level_call_primitive, low_level_import_primitive,
        scan_low_level_primitive_file, source_without_cfg_test_blocks,
        DECLARED_WRITE_RUNTIME_FILES, LEGACY_AMBIENT_RUNTIME_EXCEPTIONS,
    };

    #[test]
    fn low_level_scanner_detects_qualified_unqualified_and_imported_primitives() {
        assert_eq!(
            low_level_call_primitive("rustix::fs::openat(&dir, path, flags, mode)?;"),
            Some("openat")
        );
        assert_eq!(
            low_level_call_primitive("let resolver = rustix::fs::openat2;"),
            Some("openat2")
        );
        assert_eq!(
            low_level_call_primitive("renameat_with (old, from, new, to, flags)?;"),
            Some("renameat_with")
        );
        assert_eq!(
            low_level_import_primitive("use rustix::fs::{OFlags, openat as open_child};"),
            Some("openat")
        );
        assert_eq!(
            low_level_import_primitive("use rustix::{fd::OwnedFd, fs::{Mode, mkdirat, unlinkat}};"),
            Some("mkdirat")
        );
        assert_eq!(
            low_level_import_primitive("use rustix::fs::*;"),
            Some("rustix::fs::*")
        );
        assert_eq!(
            low_level_import_primitive("use rustix::fs::{OFlags, *};"),
            Some("rustix::fs::*")
        );
        assert_eq!(
            low_level_import_primitive("pub(in crate::kernel) use rustix::fs::unlinkat;"),
            Some("unlinkat")
        );
        assert_eq!(
            low_level_import_primitive("use rustix::{fs::OFlags, io::*};"),
            Some("rustix::fs API")
        );
        assert_eq!(low_level_call_primitive("open_file(path)?;"), None);
    }

    #[test]
    fn low_level_scanner_ignores_cfg_test_blocks_but_reports_runtime_imports() {
        let root = unique_test_dir("low-level-scanner");
        let source_root = root.join("src");
        let path = source_root.join("runtime.rs");
        fs::create_dir_all(&source_root).unwrap();
        fs::write(
            &path,
            r#"
#[cfg(test)]
mod tests {
    use rustix::fs::openat;
    fn invokes_test_only_primitive() { let _ = openat(dir, "file", flags, mode); }
}

use rustix::fs::{
    Mode,
    symlinkat,
};
"#,
        )
        .unwrap();

        let mut violations = Vec::new();
        scan_low_level_primitive_file(&source_root, &path, &mut violations);

        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("symlinkat"));
        assert!(violations[0].contains("runtime.rs:8"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn only_capability_family_and_wal_trust_base_are_allowlisted() {
        let source_root = PathBuf::from("/workspace/src");

        assert!(is_low_level_backend_file(
            &source_root,
            &source_root.join("kernel/write_authority/capability.rs")
        ));
        assert!(is_low_level_backend_file(
            &source_root,
            &source_root.join("kernel/write_authority/capability/platform/lifecycle.rs")
        ));
        assert!(is_low_level_backend_file(
            &source_root,
            &source_root.join("kernel/write_authority/capability/platform/copy.rs")
        ));
        assert!(is_low_level_backend_file(
            &source_root,
            &source_root.join("kernel/write_authority/capability/platform/copy/recovery.rs")
        ));
        assert!(is_low_level_backend_file(
            &source_root,
            &source_root.join(super::CAPABILITY_DIRECTORY_BACKEND_FILE)
        ));
        assert!(is_low_level_backend_file(
            &source_root,
            &source_root.join(super::CAPABILITY_DIRECTORY_RECOVERY_BACKEND_FILE)
        ));
        assert!(is_low_level_backend_file(
            &source_root,
            &source_root.join(super::CAPABILITY_SYMLINK_BACKEND_FILE)
        ));
        assert!(is_low_level_backend_file(
            &source_root,
            &source_root.join(super::CAPABILITY_SYMLINK_RECOVERY_BACKEND_FILE)
        ));
        assert!(is_low_level_backend_file(
            &source_root,
            &source_root.join("kernel/write_authority/capability/platform/anonymous_file.rs")
        ));
        assert!(is_low_level_backend_file(
            &source_root,
            &source_root.join(super::CAPABILITY_APPEND_BACKEND_FILE)
        ));
        assert!(is_low_level_backend_file(
            &source_root,
            &source_root.join(super::CAPABILITY_APPEND_RECOVERY_BACKEND_FILE)
        ));
        assert!(is_low_level_backend_file(
            &source_root,
            &source_root.join("kernel/write_authority/capability/platform/external_config.rs")
        ));
        assert!(is_low_level_backend_file(
            &source_root,
            &source_root
                .join("kernel/write_authority/capability/platform/external_config/snapshot.rs")
        ));
        assert!(is_low_level_backend_file(
            &source_root,
            &source_root
                .join("kernel/write_authority/capability/platform/external_config/recovery.rs")
        ));
        assert!(is_low_level_backend_file(
            &source_root,
            &source_root.join(
                "kernel/write_authority/capability/platform/external_config/recovery/snapshot.rs"
            )
        ));
        assert!(is_low_level_backend_file(
            &source_root,
            &source_root.join("kernel/write_authority/capability/platform/rename.rs")
        ));
        assert!(is_low_level_backend_file(
            &source_root,
            &source_root.join("kernel/write_authority/capability/platform/remove.rs")
        ));
        assert!(is_low_level_backend_file(
            &source_root,
            &source_root.join("kernel/write_authority/capability/platform/remove_tree.rs")
        ));
        assert!(is_low_level_backend_file(
            &source_root,
            &source_root.join("kernel/write_authority/capability/platform/remove_tree/recovery.rs")
        ));
        assert!(is_low_level_backend_file(
            &source_root,
            &source_root.join("kernel/write_authority/capability/platform/remove_tree/snapshot.rs")
        ));
        assert!(is_low_level_backend_file(
            &source_root,
            &source_root
                .join("kernel/write_authority/capability/platform/remove_tree/traversal.rs")
        ));
        assert!(is_low_level_backend_file(
            &source_root,
            &source_root.join("kernel/write_authority/tree_fingerprint.rs")
        ));
        assert!(is_low_level_backend_file(
            &source_root,
            &source_root.join("kernel/write_authority/recovery/wal_io.rs")
        ));
        assert!(!is_low_level_backend_file(
            &source_root,
            &source_root.join("kernel/write_authority/authority.rs")
        ));
        assert!(!is_low_level_backend_file(
            &source_root,
            &source_root.join("kernel/observability/mod.rs")
        ));
    }

    #[test]
    fn directory_direct_backend_has_no_temporary_or_destructive_namespace_protocol() {
        let source_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        let runtime =
            fs::read_to_string(source_root.join(super::CAPABILITY_DIRECTORY_BACKEND_FILE)).unwrap();
        let recovery =
            fs::read_to_string(source_root.join(super::CAPABILITY_DIRECTORY_RECOVERY_BACKEND_FILE))
                .unwrap();
        assert!(runtime.contains("fs::mkdirat("));
        for forbidden in [
            "renameat",
            "RenameFlags",
            "unlinkat",
            "remove_dir",
            "rmdir",
            "temp_leaf",
        ] {
            assert!(
                !runtime.contains(forbidden),
                "Directory direct runtime conține primitiva interzisă {forbidden}"
            );
            assert!(
                !recovery.contains(forbidden),
                "Directory direct recovery conține primitiva interzisă {forbidden}"
            );
        }
    }

    #[test]
    fn symlink_direct_backend_has_one_create_and_no_cleanup_namespace_protocol() {
        let source_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        let runtime = source_without_cfg_test_blocks(
            &fs::read_to_string(source_root.join(super::CAPABILITY_SYMLINK_BACKEND_FILE)).unwrap(),
        );
        let recovery = source_without_cfg_test_blocks(
            &fs::read_to_string(source_root.join(super::CAPABILITY_SYMLINK_RECOVERY_BACKEND_FILE))
                .unwrap(),
        );
        assert_eq!(runtime.matches("fs::symlinkat(").count(), 1);
        for forbidden in [
            "renameat",
            "RenameFlags",
            "unlinkat",
            "fs::linkat(",
            "std::fs::rename",
            "std::fs::remove_file",
            "std::fs::remove_dir",
            "std::fs::remove_dir_all",
            "fs::rename",
            "fs::remove_file",
            "fs::remove_dir",
            "fs::remove_dir_all",
            "rmdir",
            "temp_leaf",
        ] {
            assert!(
                !runtime.contains(forbidden),
                "Symlink direct runtime conține primitiva interzisă {forbidden}"
            );
            assert!(
                !recovery.contains(forbidden),
                "Symlink direct recovery conține primitiva interzisă {forbidden}"
            );
        }
    }

    #[test]
    fn ambient_bootstrap_and_observability_exceptions_are_eliminated() {
        assert!(LEGACY_AMBIENT_RUNTIME_EXCEPTIONS.is_empty());
        assert!(DECLARED_WRITE_RUNTIME_FILES.contains(&"kernel/write_authority/capability.rs"));
        assert!(DECLARED_WRITE_RUNTIME_FILES
            .contains(&"kernel/write_authority/capability/platform/anonymous_file.rs"));
        assert!(DECLARED_WRITE_RUNTIME_FILES
            .contains(&"kernel/write_authority/capability/platform/append.rs"));
        assert!(DECLARED_WRITE_RUNTIME_FILES
            .contains(&"kernel/write_authority/capability/platform/append/recovery.rs"));
        assert!(DECLARED_WRITE_RUNTIME_FILES
            .contains(&"kernel/write_authority/capability/platform/copy/recovery.rs"));
        assert!(DECLARED_WRITE_RUNTIME_FILES
            .contains(&"kernel/write_authority/capability/platform/directory.rs"));
        assert!(DECLARED_WRITE_RUNTIME_FILES
            .contains(&"kernel/write_authority/capability/platform/directory/recovery.rs"));
        assert!(DECLARED_WRITE_RUNTIME_FILES
            .contains(&"kernel/write_authority/capability/platform/symlink.rs"));
        assert!(DECLARED_WRITE_RUNTIME_FILES
            .contains(&"kernel/write_authority/capability/platform/symlink/recovery.rs"));
        assert!(DECLARED_WRITE_RUNTIME_FILES
            .contains(&"kernel/write_authority/capability/platform/lifecycle.rs"));
        assert!(DECLARED_WRITE_RUNTIME_FILES
            .contains(&"kernel/write_authority/capability/platform/external_config.rs"));
        assert!(DECLARED_WRITE_RUNTIME_FILES
            .contains(&"kernel/write_authority/capability/platform/external_config/snapshot.rs"));
        assert!(DECLARED_WRITE_RUNTIME_FILES
            .contains(&"kernel/write_authority/capability/platform/external_config/recovery.rs"));
        assert!(DECLARED_WRITE_RUNTIME_FILES.contains(
            &"kernel/write_authority/capability/platform/external_config/recovery/snapshot.rs"
        ));
        assert!(DECLARED_WRITE_RUNTIME_FILES
            .contains(&"kernel/write_authority/capability/platform/rename.rs"));
        assert!(DECLARED_WRITE_RUNTIME_FILES
            .contains(&"kernel/write_authority/capability/platform/remove.rs"));
        assert!(DECLARED_WRITE_RUNTIME_FILES
            .contains(&"kernel/write_authority/capability/platform/remove_tree.rs"));
        assert!(DECLARED_WRITE_RUNTIME_FILES
            .contains(&"kernel/write_authority/capability/platform/remove_tree/recovery.rs"));
        assert!(DECLARED_WRITE_RUNTIME_FILES
            .contains(&"kernel/write_authority/capability/platform/remove_tree/snapshot.rs"));
        assert!(DECLARED_WRITE_RUNTIME_FILES
            .contains(&"kernel/write_authority/capability/platform/remove_tree/traversal.rs"));
        assert!(
            DECLARED_WRITE_RUNTIME_FILES.contains(&"kernel/write_authority/tree_fingerprint.rs")
        );
        assert!(DECLARED_WRITE_RUNTIME_FILES.contains(&"kernel/write_authority/recovery/wal_io.rs"));
        assert!(!DECLARED_WRITE_RUNTIME_FILES.contains(&"kernel/write_authority/authority.rs"));
        assert!(!DECLARED_WRITE_RUNTIME_FILES.contains(&"app_home.rs"));
    }

    fn unique_test_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pana-studio-{label}-{nanos}"))
    }
}
