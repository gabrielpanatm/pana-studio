use std::{
    path::{Path, PathBuf},
    sync::atomic::Ordering,
};

use super::{
    capability, require_durable_maintenance_effect,
    root_authority::{DirectoryAuthority, DirectoryAuthorityScope, WriteAuthorityRuntime},
    CapabilityMaintenanceError, WriteTarget, PREVIEW_PROJECTION_GENERATION_SEQUENCE,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PreviewProjectionPublicationStats {
    pub logical_publications: u32,
    pub durability_operations: u32,
    pub materialized_entries: usize,
    pub materialized_bytes: u64,
}

pub(crate) struct PreviewProjectionGeneration {
    parent_authority: DirectoryAuthority,
    staging_authority: Option<DirectoryAuthority>,
    staging_root: PathBuf,
    publication_root: PathBuf,
    operation_id: String,
    materialized_entries: usize,
    materialized_bytes: u64,
}

pub(crate) struct PreviewProjectionPublication {
    pub stats: PreviewProjectionPublicationStats,
    retirement: Option<PreviewProjectionRetirement>,
}

impl PreviewProjectionPublication {
    pub(crate) fn retire_previous(self) -> Result<u32, String> {
        self.retirement
            .map(PreviewProjectionRetirement::retire)
            .transpose()
            .map(|operations| operations.unwrap_or_default())
    }
}

struct PreviewProjectionRetirement {
    parent_authority: DirectoryAuthority,
    retired_root: PathBuf,
    operation_id: String,
}

impl PreviewProjectionRetirement {
    fn retire(self) -> Result<u32, String> {
        let target = WriteTarget::new(
            &self.retired_root,
            self.parent_authority.root_path(),
            "preview/projection/retired",
        )
        .bind_authority(self.parent_authority)?;
        let effect = capability::remove_rebuildable_directory_if_exists(
            &target,
            &format!("{}-retire", self.operation_id),
        )?;
        require_durable_maintenance_effect(effect)
            // Removal quarantines the old name and then removes the quarantine.
            // Each visible namespace transition synchronizes the sealed parent.
            .map(|effect| if effect.changed { 2 } else { 0 })
            .map_err(|error| error.to_string())
    }
}

impl PreviewProjectionGeneration {
    pub(crate) fn begin(
        runtime: &WriteAuthorityRuntime,
        session_root: &Path,
        publication_root: &Path,
    ) -> Result<Self, String> {
        if publication_root.parent() != Some(session_root)
            || publication_root.file_name().and_then(|name| name.to_str()) != Some("source")
        {
            return Err(format!(
                "Generația Preview a refuzat rădăcina publică {} în afara sesiunii {}.",
                publication_root.display(),
                session_root.display()
            ));
        }
        let parent_authority = runtime.capture_preview_cache_descendant_authority(
            session_root,
            "preview/projection/session",
        )?;
        let sequence = PREVIEW_PROJECTION_GENERATION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let operation_id = format!("preview-projection-{}-{sequence}", std::process::id());
        let staging_root = session_root.join(format!(".source-staging-{operation_id}"));
        capability::create_private_rebuildable_directory(
            &parent_authority,
            &staging_root,
            "preview/projection/staging",
        )?;
        let staging_authority = match capability::capture_descendant_authority(
            &parent_authority,
            &staging_root,
            "preview/projection/staging",
            DirectoryAuthorityScope::ApplicationPreviewCache,
        ) {
            Ok(authority) => authority,
            Err(error) => {
                let cleanup = cleanup_private_generation(
                    parent_authority.clone(),
                    &staging_root,
                    &operation_id,
                );
                return Err(match cleanup {
                    Ok(_) => error,
                    Err(cleanup_error) => format!(
                        "{error} Cleanup-ul generației private eșuate a eșuat: {cleanup_error}"
                    ),
                });
            }
        };

        Ok(Self {
            parent_authority,
            staging_authority: Some(staging_authority),
            staging_root,
            publication_root: publication_root.to_path_buf(),
            operation_id,
            materialized_entries: 0,
            materialized_bytes: 0,
        })
    }

    pub(crate) fn create_directory(&mut self, relative_path: &Path) -> Result<(), String> {
        if relative_path.as_os_str().is_empty() {
            return Ok(());
        }
        capability::create_rebuildable_generation_directory(
            self.staging_authority()?,
            relative_path,
            "preview/projection/staging-directory",
        )?;
        self.materialized_entries = self.materialized_entries.saturating_add(1);
        Ok(())
    }

    pub(crate) fn write_text(
        &mut self,
        relative_path: &Path,
        contents: &str,
    ) -> Result<(), String> {
        self.write_bytes(relative_path, contents.as_bytes())
    }

    pub(crate) fn write_bytes(
        &mut self,
        relative_path: &Path,
        contents: &[u8],
    ) -> Result<(), String> {
        if relative_path.as_os_str().is_empty() {
            return Err("Generația Preview refuză scrierea în rădăcină.".to_string());
        }
        capability::write_rebuildable_generation_file(
            self.staging_authority()?,
            relative_path,
            contents,
            "preview/projection/staging-file",
        )?;
        self.materialized_entries = self.materialized_entries.saturating_add(1);
        self.materialized_bytes = self
            .materialized_bytes
            .checked_add(contents.len() as u64)
            .ok_or_else(|| "Generația Preview a depășit contorul de bytes.".to_string())?;
        Ok(())
    }

    pub(crate) fn discard(mut self) -> Result<(), String> {
        self.staging_authority.take();
        cleanup_private_generation(
            self.parent_authority,
            &self.staging_root,
            &self.operation_id,
        )
        .map(|_| ())
    }

    pub(crate) fn publish(mut self) -> Result<PreviewProjectionPublication, String> {
        let staging_authority = self
            .staging_authority
            .as_ref()
            .ok_or_else(|| "Generația Preview nu mai are authority staging.".to_string())?;
        capability::verify_directory_authority_path(staging_authority)?;
        capability::seal_rebuildable_generation(
            staging_authority,
            "preview/projection/staging-seal",
        )?;

        let source = WriteTarget::new(
            &self.staging_root,
            self.parent_authority.root_path(),
            "preview/projection/staging",
        )
        .bind_authority(self.parent_authority.clone())?;
        let destination = WriteTarget::new(
            &self.publication_root,
            self.parent_authority.root_path(),
            "preview/projection/published",
        )
        .bind_authority(self.parent_authority.clone())?;
        let effect = capability::publish_rebuildable_directory(&source, &destination)?;
        require_durable_maintenance_effect(effect)
            .map_err(CapabilityMaintenanceError::into_terminal_diagnostic)?;
        self.staging_authority.take();

        let retirement = if capability::is_real_directory_leaf(
            &self.parent_authority,
            &self.staging_root,
            "preview/projection/retired",
        )? {
            Some(PreviewProjectionRetirement {
                parent_authority: self.parent_authority,
                retired_root: self.staging_root,
                operation_id: self.operation_id,
            })
        } else {
            None
        };
        Ok(PreviewProjectionPublication {
            stats: PreviewProjectionPublicationStats {
                logical_publications: 1,
                // Preview is a process-scoped, rebuildable cache. Its file
                // bytes need in-process completeness, not cross-process crash
                // recovery. One fsync seals the private namespace metadata;
                // the atomic name exchange then fsyncs the session parent.
                durability_operations: 2,
                materialized_entries: self.materialized_entries,
                materialized_bytes: self.materialized_bytes,
            },
            retirement,
        })
    }

    fn staging_authority(&self) -> Result<&DirectoryAuthority, String> {
        self.staging_authority
            .as_ref()
            .ok_or_else(|| "Generația Preview a fost deja închisă.".to_string())
    }
}

fn cleanup_private_generation(
    parent_authority: DirectoryAuthority,
    staging_root: &Path,
    operation_id: &str,
) -> Result<bool, String> {
    let target = WriteTarget::new(
        staging_root,
        parent_authority.root_path(),
        "preview/projection/private-cleanup",
    )
    .bind_authority(parent_authority)?;
    let effect = capability::remove_rebuildable_directory_if_exists(&target, operation_id)?;
    require_durable_maintenance_effect(effect)
        .map(|effect| effect.changed)
        .map_err(|error| error.to_string())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::kernel::write_authority::ApplicationAuthorityPaths;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn complete_generation_is_published_once_and_retired_after_release() {
        let fixture = fixture_root("publish");
        let runtime = runtime_for(&fixture);
        let session_root = fixture.join("cache/preview/project/editor/session-test");
        fs::create_dir_all(&session_root).unwrap();
        let publication_root = session_root.join("source");
        fs::create_dir_all(publication_root.join("templates")).unwrap();
        fs::write(
            publication_root.join("templates/old.html"),
            "old generation",
        )
        .unwrap();

        let mut generation =
            PreviewProjectionGeneration::begin(&runtime, &session_root, &publication_root).unwrap();
        generation
            .write_text(
                Path::new("templates/index.html"),
                "<main data-pana-source>new generation</main>",
            )
            .unwrap();
        generation
            .write_bytes(Path::new("static/logo.bin"), b"binary")
            .unwrap();
        let publication = generation.publish().unwrap();

        assert_eq!(publication.stats.logical_publications, 1);
        assert_eq!(publication.stats.durability_operations, 2);
        assert_eq!(publication.stats.materialized_entries, 2);
        assert_eq!(
            fs::read_to_string(publication_root.join("templates/index.html")).unwrap(),
            "<main data-pana-source>new generation</main>"
        );
        assert!(!publication_root.join("templates/old.html").exists());
        assert_eq!(private_generation_count(&session_root), 1);

        assert_eq!(publication.retire_previous().unwrap(), 2);
        assert_eq!(private_generation_count(&session_root), 0);
        fs::remove_dir_all(fixture).unwrap();
    }

    #[test]
    fn descriptor_writer_refuses_parent_escape_and_symlink_ancestors() {
        use std::os::unix::fs::symlink;

        let fixture = fixture_root("symlink");
        let runtime = runtime_for(&fixture);
        let session_root = fixture.join("cache/preview/project/editor/session-test");
        fs::create_dir_all(&session_root).unwrap();
        let publication_root = session_root.join("source");
        let mut generation =
            PreviewProjectionGeneration::begin(&runtime, &session_root, &publication_root).unwrap();

        assert!(generation
            .write_text(Path::new("../outside.html"), "blocked")
            .is_err());
        let outside = fixture.join("outside");
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, generation.staging_root.join("templates")).unwrap();
        assert!(generation
            .write_text(Path::new("templates/index.html"), "blocked")
            .is_err());
        assert!(!outside.join("index.html").exists());

        fs::remove_file(generation.staging_root.join("templates")).unwrap();
        generation.discard().unwrap();
        fs::remove_dir_all(fixture).unwrap();
    }

    fn runtime_for(root: &Path) -> WriteAuthorityRuntime {
        let runtime = WriteAuthorityRuntime::default();
        runtime
            .install_application_home(ApplicationAuthorityPaths {
                config_dir: root.join("config"),
                data_dir: root.join("data"),
                cache_dir: root.join("cache"),
                log_dir: root.join("logs"),
                projects_config_dir: root.join("config/projects"),
                mcp_dir: root.join("config/mcp"),
                sessions_dir: root.join("data/sessions"),
                kernel_dir: root.join("data/kernel"),
                write_authority_wal_dir: root.join("data/kernel/write-authority-wal"),
                scratch_dir: root.join("cache/scratch"),
                preview_cache_dir: root.join("cache/preview"),
                app_logs_dir: root.join("logs/app"),
            })
            .unwrap();
        runtime
    }

    fn private_generation_count(session_root: &Path) -> usize {
        fs::read_dir(session_root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".source-staging-preview-projection-")
            })
            .count()
    }

    fn fixture_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "pana-preview-generation-{label}-{}-{nonce}",
            std::process::id()
        ))
    }
}
