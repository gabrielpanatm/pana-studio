use std::{
    fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
};

use super::model::WriteTarget;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct BoundaryRules {
    allow_boundary_root: bool,
    allow_leaf_symlink: bool,
}

impl BoundaryRules {
    pub(super) const WRITE: Self = Self {
        allow_boundary_root: false,
        allow_leaf_symlink: false,
    };

    pub(super) const CREATE_DIRECTORY: Self = Self {
        allow_boundary_root: true,
        allow_leaf_symlink: false,
    };

    pub(super) const UNLINK_LEAF: Self = Self {
        allow_boundary_root: false,
        allow_leaf_symlink: true,
    };

    pub(super) const RENAME_SOURCE: Self = Self {
        allow_boundary_root: false,
        allow_leaf_symlink: true,
    };
}

pub(super) fn validate_target_boundary(
    target: &WriteTarget,
    rules: BoundaryRules,
) -> Result<(), String> {
    let path = target.path.as_path();
    let boundary_root = target.boundary_root.as_path();

    if path.as_os_str().is_empty() || boundary_root.as_os_str().is_empty() {
        return Err(boundary_error(
            target,
            "target-ul și boundary-ul trebuie să fie căi ne-goale",
        ));
    }

    if !path.is_absolute() || !boundary_root.is_absolute() {
        return Err(boundary_error(
            target,
            "target-ul și boundary-ul trebuie să fie căi absolute",
        ));
    }

    validate_boundary_shape(target)?;

    let relative = path.strip_prefix(boundary_root).map_err(|_| {
        boundary_error(
            target,
            "target-ul nu este descendent lexical al boundary-ului declarat",
        )
    })?;

    if relative.as_os_str().is_empty() && !rules.allow_boundary_root {
        return Err(boundary_error(
            target,
            "operația nu poate folosi chiar rădăcina autorității ca target",
        ));
    }

    for component in relative.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(boundary_error(
                target,
                "calea relativă conține componente non-canonice sau de traversare",
            ));
        }
    }

    let resolved_boundary = resolve_existing_and_missing(boundary_root).map_err(|reason| {
        boundary_error(
            target,
            &format!("boundary-ul nu poate fi rezolvat fail-closed: {reason}"),
        )
    })?;

    validate_relative_components(target, &resolved_boundary, relative, rules)
}

fn validate_boundary_shape(target: &WriteTarget) -> Result<(), String> {
    for component in target.boundary_root.components() {
        match component {
            Component::Normal(_) | Component::RootDir | Component::Prefix(_) => {}
            Component::CurDir | Component::ParentDir => {
                return Err(boundary_error(
                    target,
                    "boundary-ul declarat conține componente relative sau de traversare",
                ));
            }
        }
    }
    Ok(())
}

fn validate_relative_components(
    target: &WriteTarget,
    resolved_boundary: &Path,
    relative: &Path,
    rules: BoundaryRules,
) -> Result<(), String> {
    let components = relative.components().collect::<Vec<_>>();
    let mut resolved = resolved_boundary.to_path_buf();

    for (index, component) in components.iter().enumerate() {
        let Component::Normal(part) = component else {
            return Err(boundary_error(
                target,
                "calea relativă conține o componentă nesigură",
            ));
        };
        let is_leaf = index + 1 == components.len();
        let candidate = resolved.join(part);

        match fs::symlink_metadata(&candidate) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                if is_leaf && rules.allow_leaf_symlink {
                    resolved.push(part);
                    continue;
                }
                return Err(boundary_error(
                    target,
                    &format!(
                        "componenta {} este symlink și operația nu are voie să o urmeze",
                        candidate.display()
                    ),
                ));
            }
            Ok(metadata) if !is_leaf && !metadata.is_dir() => {
                return Err(boundary_error(
                    target,
                    &format!(
                        "strămoșul {} există, dar nu este director",
                        candidate.display()
                    ),
                ));
            }
            Ok(_) => resolved.push(part),
            Err(error) if error.kind() == ErrorKind::NotFound => {
                resolved.push(part);
                for remaining in components.iter().skip(index + 1) {
                    let Component::Normal(part) = remaining else {
                        return Err(boundary_error(
                            target,
                            "suffix-ul inexistent conține o componentă nesigură",
                        ));
                    };
                    resolved.push(part);
                }
                break;
            }
            Err(error) => {
                return Err(boundary_error(
                    target,
                    &format!(
                        "componenta {} nu poate fi verificată: {error}",
                        candidate.display()
                    ),
                ));
            }
        }
    }

    if !resolved.starts_with(resolved_boundary) {
        return Err(boundary_error(
            target,
            "rezolvarea filesystem a target-ului iese din boundary",
        ));
    }

    Ok(())
}

fn resolve_existing_and_missing(path: &Path) -> Result<PathBuf, String> {
    let components = path.components().collect::<Vec<_>>();
    let mut resolved = PathBuf::new();
    for (index, component) in components.iter().enumerate() {
        match component {
            Component::Prefix(prefix) => resolved.push(prefix.as_os_str()),
            Component::RootDir => resolved.push(component.as_os_str()),
            Component::CurDir | Component::ParentDir => {
                return Err("boundary-ul conține o componentă relativă nesigură".to_string());
            }
            Component::Normal(part) => {
                let candidate = resolved.join(part);
                match fs::symlink_metadata(&candidate) {
                    Ok(metadata) if metadata.file_type().is_symlink() => {
                        return Err(format!(
                            "{} este symlink în boundary-ul declarat",
                            candidate.display()
                        ));
                    }
                    Ok(metadata) if !metadata.is_dir() => {
                        return Err(format!(
                            "{} există în boundary, dar nu este director",
                            candidate.display()
                        ));
                    }
                    Ok(_) => resolved.push(part),
                    Err(error) if error.kind() == ErrorKind::NotFound => {
                        resolved.push(part);
                        for remaining in components.iter().skip(index + 1) {
                            let Component::Normal(part) = remaining else {
                                return Err(
                                    "suffix-ul boundary-ului conține o componentă nesigură"
                                        .to_string(),
                                );
                            };
                            resolved.push(part);
                        }
                        break;
                    }
                    Err(error) => {
                        return Err(format!(
                            "{} nu poate fi verificat: {error}",
                            candidate.display()
                        ));
                    }
                }
            }
        }
    }

    if resolved.as_os_str().is_empty() {
        return Err("calea nu are un anchor filesystem rezolvabil".to_string());
    }

    Ok(resolved)
}

fn boundary_error(target: &WriteTarget, reason: &str) -> String {
    format!(
        "Scriere blocată: {} nu respectă autoritatea {}: {}.",
        target.path.display(),
        target.boundary_root.display(),
        reason
    )
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{validate_target_boundary, BoundaryRules};
    use crate::kernel::write_authority::WriteTarget;

    #[test]
    fn allows_normal_child_with_missing_boundary_and_parents() {
        let root = unique_test_dir("missing-boundary");
        fs::create_dir_all(&root).unwrap();
        let boundary = root.join("sessions/project-id");
        let target = boundary.join("journals/nested/record.json");

        validate_target_boundary(
            &WriteTarget::new(target, boundary, "missing-boundary/record"),
            BoundaryRules::WRITE,
        )
        .unwrap();

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn allows_create_directory_for_boundary_root() {
        let root = unique_test_dir("create-boundary-root");
        fs::create_dir_all(&root).unwrap();
        let boundary = root.join("preview");

        validate_target_boundary(
            &WriteTarget::new(boundary.clone(), boundary, "preview/root"),
            BoundaryRules::CREATE_DIRECTORY,
        )
        .unwrap();

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn blocks_boundary_root_for_write_and_remove() {
        let root = unique_test_dir("boundary-root-target");
        fs::create_dir_all(&root).unwrap();
        let target = WriteTarget::new(root.clone(), root.clone(), "root");

        assert!(validate_target_boundary(&target, BoundaryRules::WRITE).is_err());
        assert!(validate_target_boundary(&target, BoundaryRules::UNLINK_LEAF).is_err());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn blocks_sibling_prefix() {
        let root = unique_test_dir("sibling-prefix");
        fs::create_dir_all(&root).unwrap();
        let boundary = root.join("project");
        let target = root.join("project-copy/file.txt");

        assert!(validate_target_boundary(
            &WriteTarget::new(target, boundary, "sibling"),
            BoundaryRules::WRITE,
        )
        .is_err());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn blocks_relative_target_and_boundary() {
        let target = WriteTarget::new(
            PathBuf::from("project/file.txt"),
            PathBuf::from("project"),
            "relative/file.txt",
        );

        let error = validate_target_boundary(&target, BoundaryRules::WRITE).unwrap_err();

        assert!(error.contains("căi absolute"));
    }

    #[test]
    fn blocks_parent_dir_in_missing_suffix() {
        let root = unique_test_dir("parent-dir");
        fs::create_dir_all(&root).unwrap();
        let boundary = root.join("preview");
        let target = boundary.join("templates/new/../../../escaped.html");

        let error = validate_target_boundary(
            &WriteTarget::new(target, boundary, "preview/traversal"),
            BoundaryRules::WRITE,
        )
        .unwrap_err();

        assert!(error.contains("traversare"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn blocks_parent_dir_in_declared_boundary() {
        let root = unique_test_dir("parent-dir-boundary");
        fs::create_dir_all(&root).unwrap();
        let boundary = root.join("namespace/../authority");
        let target = boundary.join("record.json");

        let error = validate_target_boundary(
            &WriteTarget::new(target, boundary, "unsafe-boundary/record"),
            BoundaryRules::WRITE,
        )
        .unwrap_err();

        assert!(error.contains("boundary-ul declarat"));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn blocks_symlink_ancestor_even_when_descendants_are_missing() {
        use std::os::unix::fs::symlink;

        let root = unique_test_dir("symlink-ancestor");
        let boundary = root.join("project");
        let outside = root.join("outside");
        fs::create_dir_all(&boundary).unwrap();
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, boundary.join("link")).unwrap();

        let error = validate_target_boundary(
            &WriteTarget::new(
                boundary.join("link/new/file.txt"),
                boundary.clone(),
                "project/link/new/file.txt",
            ),
            BoundaryRules::WRITE,
        )
        .unwrap_err();

        assert!(error.contains("symlink"));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn blocks_dangling_symlink_leaf_for_write() {
        use std::os::unix::fs::symlink;

        let root = unique_test_dir("dangling-leaf");
        let boundary = root.join("project");
        fs::create_dir_all(&boundary).unwrap();
        symlink(
            root.join("outside/missing.txt"),
            boundary.join("journal.jsonl"),
        )
        .unwrap();

        assert!(validate_target_boundary(
            &WriteTarget::new(
                boundary.join("journal.jsonl"),
                boundary.clone(),
                "project/journal.jsonl",
            ),
            BoundaryRules::WRITE,
        )
        .is_err());

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn allows_symlink_leaf_only_for_unlink() {
        use std::os::unix::fs::symlink;

        let root = unique_test_dir("unlink-leaf");
        let boundary = root.join("project");
        let outside = root.join("outside.txt");
        fs::create_dir_all(&boundary).unwrap();
        fs::write(&outside, "outside").unwrap();
        symlink(&outside, boundary.join("link")).unwrap();

        validate_target_boundary(
            &WriteTarget::new(boundary.join("link"), boundary.clone(), "project/link"),
            BoundaryRules::UNLINK_LEAF,
        )
        .unwrap();

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn blocks_symlink_loop_in_declared_boundary() {
        use std::os::unix::fs::symlink;

        let root = unique_test_dir("boundary-symlink-loop");
        fs::create_dir_all(&root).unwrap();
        symlink(root.join("loop-b"), root.join("loop-a")).unwrap();
        symlink(root.join("loop-a"), root.join("loop-b")).unwrap();
        let boundary = root.join("loop-a/authority");

        let error = validate_target_boundary(
            &WriteTarget::new(
                boundary.join("record.json"),
                boundary,
                "loop-boundary/record",
            ),
            BoundaryRules::WRITE,
        )
        .unwrap_err();

        assert!(error.contains("symlink"));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn blocks_boundary_below_symlink_to_outside() {
        use std::os::unix::fs::symlink;

        let root = unique_test_dir("boundary-symlink-outside");
        let safe = root.join("safe");
        let outside = root.join("outside");
        fs::create_dir_all(&safe).unwrap();
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, safe.join("link")).unwrap();
        let boundary = safe.join("link/missing-namespace");

        let error = validate_target_boundary(
            &WriteTarget::new(
                boundary.join("record.json"),
                boundary,
                "symlink-boundary/record",
            ),
            BoundaryRules::WRITE,
        )
        .unwrap_err();

        assert!(error.contains("symlink"));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn create_directory_blocks_symlink_boundary_root() {
        use std::os::unix::fs::symlink;

        let root = unique_test_dir("create-symlink-boundary-root");
        let outside = root.join("outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let boundary = root.join("authority-link");
        symlink(&outside, &boundary).unwrap();

        let error = validate_target_boundary(
            &WriteTarget::new(boundary.clone(), boundary, "symlink-boundary/create-root"),
            BoundaryRules::CREATE_DIRECTORY,
        )
        .unwrap_err();

        assert!(error.contains("symlink"));
        fs::remove_dir_all(root).unwrap();
    }

    fn unique_test_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pana-studio-boundary-{label}-{nanos}"))
    }
}
