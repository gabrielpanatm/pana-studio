use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use crate::{
    kernel::project_workspace::WorkspaceProjectionSnapshot,
    project::{AcceptedProjectDiskManifest, ProjectDiskManifest, ProjectDiskManifestEntry},
    project_model::{build_project_model_from_workspace_projection, model::ProjectModel},
};

/// In-memory source fixture for ProjectModel tests.
///
/// The directory exists only so production root-identity checks can
/// canonicalize it. ProjectModel and SourceGraph receive every semantic input
/// from the immutable WorkspaceProjectionSnapshot built by this fixture.
pub(crate) struct ProjectModelTestFixture {
    project_root: PathBuf,
    runtime_session_id: String,
    revision: u64,
    workspace_transaction_id: Option<String>,
    source_texts: HashMap<String, String>,
    resource_bytes: HashMap<String, Vec<u8>>,
    deleted_sources: HashSet<String>,
    changed_paths: HashSet<String>,
    accepted_entries: BTreeMap<String, u64>,
}

impl ProjectModelTestFixture {
    #[cfg(test)]
    pub(crate) fn new(project_root: impl Into<PathBuf>) -> Result<Self, String> {
        let project_root = project_root.into();
        fs::create_dir_all(&project_root).map_err(|error| {
            format!(
                "Fixture-ul ProjectModel nu a putut crea root-ul {}: {error}",
                project_root.display()
            )
        })?;
        let project_root = project_root.canonicalize().map_err(|error| {
            format!(
                "Fixture-ul ProjectModel nu a putut canoniza root-ul {}: {error}",
                project_root.display()
            )
        })?;
        let runtime_session_id = format!("project-model-test:{}", project_root.display());
        Ok(Self {
            project_root,
            runtime_session_id,
            revision: 0,
            workspace_transaction_id: None,
            source_texts: HashMap::new(),
            resource_bytes: HashMap::new(),
            deleted_sources: HashSet::new(),
            changed_paths: HashSet::new(),
            accepted_entries: BTreeMap::new(),
        })
    }

    pub(crate) fn standard_zola(
        project_root: impl Into<PathBuf>,
        template: impl Into<String>,
    ) -> Result<Self, String> {
        let mut fixture = Self::new(project_root)?;
        fixture.source("zola.toml", "base_url = \"http://example.test\"\n");
        fixture.source(
            "content/_index.md",
            "+++\ntitle = \"Acasă\"\ntemplate = \"index.html\"\n+++\n",
        );
        fixture.source("templates/index.html", template);
        Ok(fixture)
    }

    /// Captures an integration project's disk namespace at an explicit I/O
    /// boundary, then feeds the same immutable projection used by production.
    /// Use this only when the test also needs a real on-disk project (for
    /// example, an embedded Zola render); semantic unit tests use `source`.
    pub(crate) fn from_integration_disk_boundary(
        project_root: impl Into<PathBuf>,
    ) -> Result<Self, String> {
        let mut fixture = Self::new(project_root)?;
        let manifest = crate::project::read_project_disk_manifest(&fixture.project_root)?;
        for entry in manifest.files {
            fixture
                .accepted_entries
                .insert(entry.relative_path.clone(), entry.size);
            if let Ok(source) = fs::read_to_string(fixture.project_root.join(&entry.relative_path))
            {
                fixture.source_texts.insert(entry.relative_path, source);
            }
        }
        Ok(fixture)
    }

    pub(crate) fn root(&self) -> &Path {
        &self.project_root
    }

    /// Adds or replaces a clean source from the accepted workspace baseline.
    pub(crate) fn source(
        &mut self,
        relative_path: impl Into<String>,
        contents: impl Into<String>,
    ) -> &mut Self {
        let relative_path = normalized_test_path(relative_path.into());
        let contents = contents.into();
        self.accepted_entries
            .insert(relative_path.clone(), contents.len() as u64);
        self.source_texts.insert(relative_path.clone(), contents);
        self.deleted_sources.remove(&relative_path);
        self.changed_paths.remove(&relative_path);
        self
    }

    /// Projects a changed or newly created source without reading live disk.
    pub(crate) fn draft(
        &mut self,
        relative_path: impl Into<String>,
        contents: impl Into<String>,
    ) -> &mut Self {
        let relative_path = normalized_test_path(relative_path.into());
        self.source_texts
            .insert(relative_path.clone(), contents.into());
        self.deleted_sources.remove(&relative_path);
        self.changed_paths.insert(relative_path);
        self
    }

    pub(crate) fn delete(&mut self, relative_path: impl Into<String>) -> &mut Self {
        let relative_path = normalized_test_path(relative_path.into());
        self.source_texts.remove(&relative_path);
        self.changed_paths.remove(&relative_path);
        self.deleted_sources.insert(relative_path);
        self
    }

    pub(crate) fn staged_resource(
        &mut self,
        relative_path: impl Into<String>,
        bytes: impl Into<Vec<u8>>,
    ) -> &mut Self {
        let relative_path = normalized_test_path(relative_path.into());
        self.resource_bytes
            .insert(relative_path.clone(), bytes.into());
        self.deleted_sources.remove(&relative_path);
        self.changed_paths.insert(relative_path);
        self
    }

    pub(crate) fn accepted_resource(
        &mut self,
        relative_path: impl Into<String>,
        size: u64,
    ) -> &mut Self {
        let relative_path = normalized_test_path(relative_path.into());
        self.accepted_entries.insert(relative_path.clone(), size);
        self.deleted_sources.remove(&relative_path);
        self
    }

    pub(crate) fn revision(
        &mut self,
        revision: u64,
        transaction_id: Option<impl Into<String>>,
    ) -> &mut Self {
        self.revision = revision;
        self.workspace_transaction_id = transaction_id.map(Into::into);
        self
    }

    pub(crate) fn projection(&self) -> WorkspaceProjectionSnapshot {
        let project_root = self.project_root.to_string_lossy().to_string();
        let manifest = ProjectDiskManifest {
            root: project_root.clone(),
            files: self
                .accepted_entries
                .iter()
                .map(|(relative_path, size)| ProjectDiskManifestEntry {
                    relative_path: relative_path.clone(),
                    modified_ms: 0,
                    size: *size,
                    version_token: "project-model-test-fixture".to_string(),
                })
                .collect(),
            truncated: false,
            max_files: self.accepted_entries.len().max(1),
        };
        let accepted_disk = AcceptedProjectDiskManifest::new(
            self.runtime_session_id.clone(),
            project_root.clone(),
            manifest,
        )
        .expect("fixture-ul trebuie să producă o identitate disk coerentă");
        WorkspaceProjectionSnapshot {
            project_root,
            runtime_session_id: self.runtime_session_id.clone(),
            revision: self.revision,
            workspace_transaction_id: self.workspace_transaction_id.clone(),
            source_texts: self.source_texts.clone(),
            resource_bytes: self.resource_bytes.clone(),
            deleted_sources: self.deleted_sources.clone(),
            changed_paths: self.changed_paths.clone(),
            accepted_disk,
        }
    }

    pub(crate) fn build_model(&self) -> Result<ProjectModel, String> {
        build_project_model_from_workspace_projection(&self.project_root, &self.projection())
    }

    pub(crate) fn build_source_graph(
        &self,
    ) -> Result<crate::source_graph::model::SourceGraph, String> {
        crate::source_graph::build_source_graph_from_workspace_projection(
            &self.project_root,
            &self.projection(),
        )
    }
}

fn normalized_test_path(relative_path: String) -> String {
    let normalized = relative_path.replace('\\', "/");
    let path = Path::new(&normalized);
    assert!(
        !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, std::path::Component::Normal(_))),
        "fixture-ul ProjectModel a primit un path nesigur: {relative_path}"
    );
    normalized
}

#[cfg(test)]
mod tests {
    use std::{fs, time::SystemTime};

    use super::ProjectModelTestFixture;

    #[test]
    fn fixture_materializes_workspace_identity_and_overlays_without_disk_sources() {
        let root = std::env::temp_dir().join(format!(
            "pana-project-model-fixture-{}-{:?}",
            std::process::id(),
            SystemTime::now()
        ));
        let mut fixture =
            ProjectModelTestFixture::standard_zola(root, "<main>Baseline</main>\n").unwrap();
        fixture.source("content/removed.md", "+++\ntitle = \"Șters\"\n+++\n");
        fixture.draft("templates/index.html", "<main>Draft</main>\n");
        fixture.delete("content/removed.md");
        fixture.accepted_resource("static/accepted.png", 3);
        fixture.staged_resource("static/created.png", vec![1, 2, 3]);
        fixture.revision(7, Some("fixture-transaction"));

        let projection = fixture.projection();
        assert_eq!(projection.revision, 7);
        assert_eq!(
            projection.workspace_transaction_id.as_deref(),
            Some("fixture-transaction")
        );
        assert_eq!(
            projection.source_texts.get("templates/index.html"),
            Some(&"<main>Draft</main>\n".to_string())
        );
        assert!(projection.changed_paths.contains("templates/index.html"));
        assert!(projection.changed_paths.contains("static/created.png"));
        assert!(projection.deleted_sources.contains("content/removed.md"));
        assert_eq!(
            projection.resource_bytes.get("static/created.png"),
            Some(&vec![1, 2, 3])
        );
        assert!(projection
            .accepted_disk
            .manifest
            .files
            .iter()
            .any(|entry| entry.relative_path == "static/accepted.png"));

        let model = fixture.build_model().unwrap();
        assert!(model
            .files
            .iter()
            .any(|file| file.relative_path == "templates/index.html" && file.from_draft));

        fs::remove_dir_all(fixture.root()).unwrap();
    }
}
