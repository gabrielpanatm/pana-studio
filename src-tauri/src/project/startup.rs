use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Runtime};

use crate::{
    deploy::run_zola_editor_check,
    kernel::{
        file_buffer_store::hash_bytes,
        themes::{ThemePack, ThemeRegistry},
        write_authority::{
            ProjectCreationAuthority, WriteAuthority, WriteCategory, WriteIntent,
            WriteOperationKind, WriteOwner, WritePolicy,
        },
    },
    zola_engine::EMBEDDED_ZOLA_VERSION,
};

use super::{manifest::project_disk_metadata_version_token, zola_project_root};

pub const STARTUP_FLOW_SCHEMA_VERSION: u32 = 1;
pub const STARTUP_CREATION_CATALOG_SCHEMA_VERSION: u32 = 1;
pub const STARTUP_CREATION_PLAN_SCHEMA_VERSION: u32 = 1;
pub const STARTUP_CREATION_RECEIPT_SCHEMA_VERSION: u32 = 1;

const MINIMAL_OPTION_ID: &str = "minimal";
const BASE_ZOLA_CONFIG: &str = r#"base_url = "http://127.0.0.1:1111"
title = "Proiect Pană Studio"
description = "Un proiect Zola inițializat cu Pană Studio."
default_language = "ro"
compile_sass = true
build_search_index = false
minify_html = false
generate_sitemap = true
generate_robots_txt = true

[markdown]
render_emoji = false
smart_punctuation = false
insert_anchor_links = "none"
lazy_async_image = false
github_alerts = false
bottom_footnotes = false
external_links_target_blank = false
external_links_no_follow = false
external_links_no_referrer = false

[extra]
"#;
const BASE_GITIGNORE: &str = "/public/\n.env\n";
const MINIMAL_SECTION: &str = "+++\ntitle = \"Acasă\"\nsort_by = \"weight\"\n+++\n\nBun venit în proiectul tău Pană Studio.\n";
const MINIMAL_TEMPLATE: &str = r#"<!doctype html>
<html lang="ro">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{{ section.title }}</title>
  </head>
  <body>
    <main>
      <h1>{{ section.title }}</h1>
      {{ section.content | safe }}
    </main>
  </body>
</html>
"#;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupStage {
    Idle,
    Inspecting,
    Ready,
    Planning,
    Creating,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupCandidateKind {
    ValidProject,
    EmptyDirectory,
    UnrecognizedDirectory,
    InvalidZolaProject,
    Inaccessible,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupDiagnostic {
    pub code: String,
    pub severity: StartupDiagnosticSeverity,
    pub message: String,
    pub detail: Option<String>,
}

impl StartupDiagnostic {
    fn info(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            severity: StartupDiagnosticSeverity::Info,
            message: message.into(),
            detail: None,
        }
    }

    fn warning(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            severity: StartupDiagnosticSeverity::Warning,
            message: message.into(),
            detail: None,
        }
    }

    fn error(code: &str, message: impl Into<String>, detail: Option<String>) -> Self {
        Self {
            code: code.to_string(),
            severity: StartupDiagnosticSeverity::Error,
            message: message.into(),
            detail,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupCandidateSnapshot {
    pub root: String,
    pub display_name: String,
    pub kind: StartupCandidateKind,
    pub snapshot_token: String,
    pub entry_count: usize,
    pub truncated: bool,
    pub diagnostics: Vec<StartupDiagnostic>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupFlowSnapshot {
    pub schema_version: u32,
    pub revision: u64,
    pub stage: StartupStage,
    pub candidate: Option<StartupCandidateSnapshot>,
    pub diagnostics: Vec<StartupDiagnostic>,
}

impl StartupFlowSnapshot {
    fn idle(revision: u64) -> Self {
        Self {
            schema_version: STARTUP_FLOW_SCHEMA_VERSION,
            revision,
            stage: StartupStage::Idle,
            candidate: None,
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupCreationKind {
    Minimal,
    Starter,
    ProjectTemplate,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupCreationOption {
    pub id: String,
    pub kind: StartupCreationKind,
    pub name: String,
    pub description: String,
    pub preview_data_url: Option<String>,
    pub compatibility_label: String,
    pub capabilities: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupCreationCatalog {
    pub schema_version: u32,
    pub registry_version: String,
    pub embedded_zola_version: String,
    pub expected_snapshot_token: String,
    pub options: Vec<StartupCreationOption>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartupCreationPlanRequest {
    pub expected_snapshot_token: String,
    pub option_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupCreationPlan {
    pub schema_version: u32,
    pub expected_snapshot_token: String,
    pub plan_token: String,
    pub project_root: String,
    pub option_id: String,
    pub option_kind: StartupCreationKind,
    pub option_name: String,
    pub affected_files: Vec<String>,
    pub total_bytes: u64,
    pub diagnostics: Vec<StartupDiagnostic>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartupCreationApplyRequest {
    pub expected_snapshot_token: String,
    pub expected_plan_token: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupCreationReceipt {
    pub schema_version: u32,
    pub project_root: String,
    pub option_id: String,
    pub plan_token: String,
    pub published_files: Vec<String>,
    pub validation: String,
    pub startup: StartupFlowSnapshot,
}

#[derive(Clone, Debug)]
struct StartupFlowState {
    snapshot: StartupFlowSnapshot,
    plan: Option<StartupCreationPlan>,
    inspection_manifest: Option<super::ProjectDiskManifest>,
    lifecycle_operation_id: Option<String>,
}

#[derive(Clone)]
pub struct StartupFlowRuntime {
    state: Arc<Mutex<StartupFlowState>>,
}

impl Default for StartupFlowRuntime {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(StartupFlowState {
                snapshot: StartupFlowSnapshot::idle(1),
                plan: None,
                inspection_manifest: None,
                lifecycle_operation_id: None,
            })),
        }
    }
}

impl StartupFlowRuntime {
    pub fn snapshot(&self) -> Result<StartupFlowSnapshot, String> {
        self.state
            .lock()
            .map(|state| state.snapshot.clone())
            .map_err(|_| "Starea Startup Flow este indisponibilă.".to_string())
    }

    pub fn reset(&self) -> Result<StartupFlowSnapshot, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "Starea Startup Flow este indisponibilă.".to_string())?;
        let revision = state.snapshot.revision.saturating_add(1);
        state.snapshot = StartupFlowSnapshot::idle(revision);
        state.plan = None;
        state.inspection_manifest = None;
        state.lifecycle_operation_id = None;
        Ok(state.snapshot.clone())
    }

    #[allow(dead_code)]
    pub fn inspect(&self, requested_root: &Path) -> Result<StartupFlowSnapshot, String> {
        self.inspect_for_operation(requested_root, None)
    }

    pub(crate) fn inspect_for_operation(
        &self,
        requested_root: &Path,
        lifecycle_operation_id: Option<String>,
    ) -> Result<StartupFlowSnapshot, String> {
        let revision = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| "Starea Startup Flow este indisponibilă.".to_string())?;
            let revision = state.snapshot.revision.saturating_add(1);
            state.snapshot = StartupFlowSnapshot {
                schema_version: STARTUP_FLOW_SCHEMA_VERSION,
                revision,
                stage: StartupStage::Inspecting,
                candidate: None,
                diagnostics: Vec::new(),
            };
            state.plan = None;
            state.inspection_manifest = None;
            state.lifecycle_operation_id = lifecycle_operation_id.clone();
            revision
        };

        let (snapshot, inspection_manifest) =
            match inspect_candidate_root_with_manifest(requested_root) {
                Ok((candidate, manifest)) => (
                    StartupFlowSnapshot {
                        schema_version: STARTUP_FLOW_SCHEMA_VERSION,
                        revision,
                        stage: StartupStage::Ready,
                        diagnostics: candidate.diagnostics.clone(),
                        candidate: Some(candidate),
                    },
                    Some(manifest),
                ),
                Err(diagnostic) => (
                    StartupFlowSnapshot {
                        schema_version: STARTUP_FLOW_SCHEMA_VERSION,
                        revision,
                        stage: StartupStage::Error,
                        candidate: Some(inaccessible_candidate(requested_root, &diagnostic)),
                        diagnostics: vec![diagnostic],
                    },
                    None,
                ),
            };

        let mut state = self
            .state
            .lock()
            .map_err(|_| "Starea Startup Flow este indisponibilă.".to_string())?;
        if state.snapshot.revision != revision {
            return Ok(state.snapshot.clone());
        }
        state.snapshot = snapshot.clone();
        state.inspection_manifest = inspection_manifest;
        Ok(snapshot)
    }

    pub fn require_valid_candidate(
        &self,
        expected_root: &Path,
        expected_snapshot_token: &str,
    ) -> Result<
        (
            StartupCandidateSnapshot,
            super::ProjectDiskManifest,
            Option<String>,
        ),
        String,
    > {
        let expected_root = expected_root
            .canonicalize()
            .map_err(|error| format!("Dosarul candidat nu mai poate fi rezolvat: {error}"))?;
        let state = self
            .state
            .lock()
            .map_err(|_| "Starea Startup Flow este indisponibilă.".to_string())?;
        let candidate = state
            .snapshot
            .candidate
            .as_ref()
            .ok_or_else(|| "Startup Flow nu are un dosar candidat inspectat.".to_string())?;
        if candidate.kind != StartupCandidateKind::ValidProject {
            return Err("Candidatul Startup nu este un proiect Zola valid.".to_string());
        }
        if candidate.snapshot_token != expected_snapshot_token {
            return Err("Tokenul inspecției Startup este stale.".to_string());
        }
        if Path::new(&candidate.root) != expected_root {
            return Err("Candidatul Startup aparține altui root canonic.".to_string());
        }
        let manifest = state.inspection_manifest.as_ref().ok_or_else(|| {
            "Startup Flow nu mai are manifestul inspecției candidate.".to_string()
        })?;
        if manifest.root != candidate.root {
            return Err("Manifestul Startup aparține altui root canonic.".to_string());
        }
        Ok((
            candidate.clone(),
            manifest.clone(),
            state.lifecycle_operation_id.clone(),
        ))
    }

    fn require_empty_candidate(
        &self,
        expected_snapshot_token: &str,
    ) -> Result<StartupCandidateSnapshot, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "Starea Startup Flow este indisponibilă.".to_string())?;
        let candidate = state
            .snapshot
            .candidate
            .as_ref()
            .ok_or_else(|| "Startup Flow nu are un dosar candidat.".to_string())?;
        if candidate.snapshot_token != expected_snapshot_token {
            return Err(
                "Snapshot-ul dosarului s-a schimbat; selectează din nou dosarul.".to_string(),
            );
        }
        if candidate.kind != StartupCandidateKind::EmptyDirectory {
            return Err(
                "Crearea proiectului este permisă numai pentru candidatul gol confirmat de Rust."
                    .to_string(),
            );
        }
        Ok(candidate.clone())
    }

    fn publish_plan(&self, plan: StartupCreationPlan) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "Starea Startup Flow este indisponibilă.".to_string())?;
        state.snapshot.stage = StartupStage::Planning;
        state.plan = Some(plan);
        Ok(())
    }

    fn require_plan(
        &self,
        request: &StartupCreationApplyRequest,
    ) -> Result<StartupCreationPlan, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "Starea Startup Flow este indisponibilă.".to_string())?;
        let plan = state
            .plan
            .as_ref()
            .ok_or_else(|| "Nu există un plan de creare Rust activ.".to_string())?;
        if plan.expected_snapshot_token != request.expected_snapshot_token
            || plan.plan_token != request.expected_plan_token
        {
            return Err("Planul de creare nu mai corespunde snapshot-ului confirmat.".to_string());
        }
        Ok(plan.clone())
    }

    fn set_stage(&self, stage: StartupStage) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "Starea Startup Flow este indisponibilă.".to_string())?;
        state.snapshot.stage = stage;
        Ok(())
    }

    fn publish_after_creation(
        &self,
        candidate: StartupCandidateSnapshot,
        manifest: super::ProjectDiskManifest,
    ) -> Result<StartupFlowSnapshot, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "Starea Startup Flow este indisponibilă.".to_string())?;
        let revision = state.snapshot.revision.saturating_add(1);
        state.snapshot = StartupFlowSnapshot {
            schema_version: STARTUP_FLOW_SCHEMA_VERSION,
            revision,
            stage: StartupStage::Ready,
            diagnostics: candidate.diagnostics.clone(),
            candidate: Some(candidate),
        };
        state.plan = None;
        state.inspection_manifest = Some(manifest);
        state.lifecycle_operation_id = None;
        Ok(state.snapshot.clone())
    }
}

#[derive(Clone)]
struct MaterializedProject {
    option_id: String,
    option_kind: StartupCreationKind,
    option_name: String,
    registry_version: String,
    files: BTreeMap<String, Vec<u8>>,
}

#[derive(Clone, Debug)]
struct PublishedFile {
    relative_path: String,
    version_token: String,
    content_hash: String,
}

#[derive(Clone, Debug)]
struct PublishedDirectory {
    relative_path: String,
}

pub fn read_creation_catalog<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &StartupFlowRuntime,
    expected_snapshot_token: &str,
) -> Result<StartupCreationCatalog, String> {
    runtime.require_empty_candidate(expected_snapshot_token)?;
    let registry = ThemeRegistry::load(app).map_err(|error| error.to_string())?;
    let theme_catalog = registry.snapshot(None)?;
    let mut options = vec![StartupCreationOption {
        id: MINIMAL_OPTION_ID.to_string(),
        kind: StartupCreationKind::Minimal,
        name: "Proiect minimal".to_string(),
        description: "Structură Zola curată, cu o singură pagină și fără temă.".to_string(),
        preview_data_url: None,
        compatibility_label: format!("Zola embedded {}", EMBEDDED_ZOLA_VERSION),
        capabilities: vec!["structură minimă".to_string(), "fără temă".to_string()],
    }];
    options.extend(theme_catalog.themes.into_iter().map(|theme| {
        let kind = creation_kind_for_theme(&theme.id, &theme.category);
        StartupCreationOption {
            id: creation_option_id(kind, &theme.id),
            kind,
            name: theme.name,
            description: theme.description,
            preview_data_url: Some(theme.preview_data_url),
            compatibility_label: format!(
                "Zola {} · testat {}",
                theme.compatibility.minimum, theme.compatibility.tested
            ),
            capabilities: theme.capabilities,
        }
    }));
    Ok(StartupCreationCatalog {
        schema_version: STARTUP_CREATION_CATALOG_SCHEMA_VERSION,
        registry_version: theme_catalog.registry_version,
        embedded_zola_version: EMBEDDED_ZOLA_VERSION.to_string(),
        expected_snapshot_token: expected_snapshot_token.to_string(),
        options,
    })
}

pub fn plan_creation<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &StartupFlowRuntime,
    request: StartupCreationPlanRequest,
) -> Result<StartupCreationPlan, String> {
    let candidate = runtime.require_empty_candidate(&request.expected_snapshot_token)?;
    let materialized = materialize_project(app, &request.option_id)?;
    let affected_files = materialized.files.keys().cloned().collect::<Vec<_>>();
    let total_bytes = materialized
        .files
        .values()
        .map(|bytes| bytes.len() as u64)
        .sum();
    let plan_token = creation_plan_token(&candidate, &materialized);
    let plan = StartupCreationPlan {
        schema_version: STARTUP_CREATION_PLAN_SCHEMA_VERSION,
        expected_snapshot_token: request.expected_snapshot_token,
        plan_token,
        project_root: candidate.root,
        option_id: materialized.option_id,
        option_kind: materialized.option_kind,
        option_name: materialized.option_name,
        affected_files,
        total_bytes,
        diagnostics: vec![StartupDiagnostic::info(
            "startup_creation_no_overwrite",
            "Rust va publica numai fișiere noi și va refuza orice suprascriere.",
        )],
    };
    runtime.publish_plan(plan.clone())?;
    Ok(plan)
}

pub fn apply_creation<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &StartupFlowRuntime,
    request: StartupCreationApplyRequest,
) -> Result<StartupCreationReceipt, String> {
    let plan = runtime.require_plan(&request)?;
    let planned_candidate = runtime.require_empty_candidate(&request.expected_snapshot_token)?;
    let live_candidate = inspect_candidate_root(Path::new(&plan.project_root))
        .map_err(|diagnostic| diagnostic.message)?;
    if live_candidate.kind != StartupCandidateKind::EmptyDirectory
        || live_candidate.snapshot_token != planned_candidate.snapshot_token
    {
        return Err(
            "Dosarul s-a schimbat după planificare; crearea a fost refuzată fără modificări."
                .to_string(),
        );
    }
    let materialized = materialize_project(app, &plan.option_id)?;
    if creation_plan_token(&live_candidate, &materialized) != plan.plan_token {
        return Err(
            "Catalogul sau conținutul planului s-a schimbat; confirmarea trebuie refăcută."
                .to_string(),
        );
    }

    runtime.set_stage(StartupStage::Creating)?;
    let root = PathBuf::from(&plan.project_root);
    let authority = ProjectCreationAuthority::capture(&root)?;
    authority.verify_path_binding()?;
    require_empty_root(&root)?;
    let journal = match publish_materialized_project(app, &authority, &root, &materialized.files) {
        Ok(journal) => journal,
        Err(failure) => {
            let rollback = rollback_publication(app, &authority, &root, &failure.journal);
            return Err(fail_creation(runtime, failure.error, rollback));
        }
    };
    let validation = match run_zola_editor_check(&root, &zola_project_root(&root)) {
        Ok(validation) => validation,
        Err(error) => {
            let rollback = rollback_publication(app, &authority, &root, &journal);
            return Err(fail_creation(runtime, error, rollback));
        }
    };
    if let Err(error) = authority.verify_path_binding() {
        let rollback = rollback_publication(app, &authority, &root, &journal);
        return Err(fail_creation(runtime, error, rollback));
    }

    let (candidate, inspection_manifest) = match inspect_candidate_root_with_manifest(&root) {
        Ok(inspection) => inspection,
        Err(diagnostic) => {
            let rollback = rollback_publication(app, &authority, &root, &journal);
            return Err(fail_creation(runtime, diagnostic.message, rollback));
        }
    };
    if candidate.kind != StartupCandidateKind::ValidProject {
        let rollback = rollback_publication(app, &authority, &root, &journal);
        return Err(fail_creation(
            runtime,
            "Proiectul creat a trecut validarea, dar inspecția finală nu l-a clasificat drept valid."
                .to_string(),
            rollback,
        ));
    }
    let published_files = journal
        .files
        .iter()
        .map(|entry| entry.relative_path.clone())
        .collect::<Vec<_>>();
    let startup = runtime.publish_after_creation(candidate, inspection_manifest)?;
    Ok(StartupCreationReceipt {
        schema_version: STARTUP_CREATION_RECEIPT_SCHEMA_VERSION,
        project_root: plan.project_root,
        option_id: plan.option_id,
        plan_token: plan.plan_token,
        published_files,
        validation,
        startup,
    })
}

fn inspect_candidate_root(root: &Path) -> Result<StartupCandidateSnapshot, StartupDiagnostic> {
    inspect_candidate_root_with_manifest(root).map(|(candidate, _)| candidate)
}

fn inspect_candidate_root_with_manifest(
    root: &Path,
) -> Result<(StartupCandidateSnapshot, super::ProjectDiskManifest), StartupDiagnostic> {
    let root = root.canonicalize().map_err(|error| {
        StartupDiagnostic::error(
            "startup_root_unavailable",
            "Dosarul selectat nu poate fi rezolvat.",
            Some(error.to_string()),
        )
    })?;
    let metadata = fs::symlink_metadata(&root).map_err(|error| {
        StartupDiagnostic::error(
            "startup_root_metadata_failed",
            "Dosarul selectat nu poate fi inspectat.",
            Some(error.to_string()),
        )
    })?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(StartupDiagnostic::error(
            "startup_root_not_directory",
            "Calea selectată nu este un dosar regulat.",
            None,
        ));
    }
    let root_version = project_disk_metadata_version_token(&metadata);
    let inspection = super::manifest::inspect_project_disk(&root).map_err(|error| {
        StartupDiagnostic::error(
            "startup_directory_read_failed",
            "Dosarul selectat nu poate fi citit complet.",
            Some(error),
        )
    })?;
    let display_name = root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| root.to_string_lossy().into_owned());
    let root_text = root.to_string_lossy().into_owned();

    if inspection.entry_count == 0 {
        let kind = StartupCandidateKind::EmptyDirectory;
        return Ok((
            StartupCandidateSnapshot {
                root: root_text.clone(),
                display_name,
                kind,
                snapshot_token: candidate_token(
                    &root_text,
                    &root_version,
                    kind,
                    &inspection.inventory_fingerprint,
                ),
                entry_count: 0,
                truncated: false,
                diagnostics: vec![StartupDiagnostic::info(
                    "startup_empty_directory",
                    "Dosarul este gol și poate primi un proiect nou.",
                )],
            },
            inspection.manifest,
        ));
    }

    let zola_root = zola_project_root(&root);
    let zola_config = regular_file(&zola_root.join("zola.toml"));
    let legacy_config = regular_file(&zola_root.join("config.toml"));
    let content = regular_directory(&zola_root.join("content"));
    let has_zola_markers = zola_config
        || legacy_config
        || content
        || ["templates", "themes", "sass", "static"]
            .iter()
            .any(|name| regular_directory(&zola_root.join(name)));

    let (kind, diagnostics) = if inspection.inventory_truncated || inspection.manifest.truncated {
        (
            StartupCandidateKind::InvalidZolaProject,
            vec![StartupDiagnostic::error(
                "startup_project_inventory_truncated",
                "Proiectul depășește limita inventarului autoritar și nu poate fi deschis în siguranță.",
                None,
            )],
        )
    } else if !has_zola_markers {
        (
            StartupCandidateKind::UnrecognizedDirectory,
            vec![StartupDiagnostic::warning(
                "startup_unrecognized_directory",
                "Dosarul conține fișiere, dar nu este un proiect Zola recunoscut. Nu va fi modificat.",
            )],
        )
    } else if zola_config == legacy_config {
        (
            StartupCandidateKind::InvalidZolaProject,
            vec![StartupDiagnostic::error(
                "startup_zola_config_ambiguous",
                if zola_config {
                    "Proiectul conține simultan zola.toml și config.toml."
                } else {
                    "Proiectul nu conține zola.toml sau config.toml."
                },
                None,
            )],
        )
    } else if !content {
        (
            StartupCandidateKind::InvalidZolaProject,
            vec![StartupDiagnostic::error(
                "startup_zola_content_missing",
                "Proiectul Zola nu conține directorul regulat content.",
                None,
            )],
        )
    } else if let Err(error) = validate_zola_config_syntax(&zola_root, zola_config) {
        (
            StartupCandidateKind::InvalidZolaProject,
            vec![StartupDiagnostic::error(
                "startup_zola_config_invalid",
                "Configurația Zola nu este TOML valid.",
                Some(error),
            )],
        )
    } else {
        (
            StartupCandidateKind::ValidProject,
            vec![StartupDiagnostic::info(
                "startup_zola_candidate_ready",
                format!(
                    "Structură Zola recunoscută; validarea canonică va folosi o singură construcție Preview cu motorul embedded {}.",
                    EMBEDDED_ZOLA_VERSION
                ),
            )],
        )
    };

    Ok((
        StartupCandidateSnapshot {
            root: root_text.clone(),
            display_name,
            kind,
            snapshot_token: candidate_token(
                &root_text,
                &root_version,
                kind,
                &inspection.inventory_fingerprint,
            ),
            entry_count: inspection.entry_count,
            truncated: inspection.inventory_truncated || inspection.manifest.truncated,
            diagnostics,
        },
        inspection.manifest,
    ))
}

fn validate_zola_config_syntax(zola_root: &Path, uses_zola_toml: bool) -> Result<(), String> {
    let path = zola_root.join(if uses_zola_toml {
        "zola.toml"
    } else {
        "config.toml"
    });
    let source = fs::read_to_string(&path).map_err(|error| {
        format!(
            "Configurația {} nu poate fi citită: {error}",
            path.display()
        )
    })?;
    source
        .parse::<toml_edit::DocumentMut>()
        .map(|_| ())
        .map_err(|error| format!("Configurația {} este invalidă: {error}", path.display()))
}

fn inaccessible_candidate(root: &Path, diagnostic: &StartupDiagnostic) -> StartupCandidateSnapshot {
    let root_text = root.to_string_lossy().into_owned();
    StartupCandidateSnapshot {
        display_name: root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| root_text.clone()),
        root: root_text.clone(),
        kind: StartupCandidateKind::Inaccessible,
        snapshot_token: candidate_token(
            &root_text,
            "unavailable",
            StartupCandidateKind::Inaccessible,
            &diagnostic.code,
        ),
        entry_count: 0,
        truncated: false,
        diagnostics: vec![diagnostic.clone()],
    }
}

fn candidate_token(
    root: &str,
    root_version: &str,
    kind: StartupCandidateKind,
    inventory_fingerprint: &str,
) -> String {
    digest_parts([
        STARTUP_FLOW_SCHEMA_VERSION.to_string(),
        root.to_string(),
        root_version.to_string(),
        format!("{kind:?}"),
        inventory_fingerprint.to_string(),
    ])
}

fn regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
        .unwrap_or(false)
}

fn regular_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
        .unwrap_or(false)
}

fn creation_kind_for_theme(id: &str, category: &str) -> StartupCreationKind {
    if id == "pana-studio" || category.eq_ignore_ascii_case("starter") {
        StartupCreationKind::Starter
    } else {
        StartupCreationKind::ProjectTemplate
    }
}

fn creation_option_id(kind: StartupCreationKind, theme_id: &str) -> String {
    match kind {
        StartupCreationKind::Minimal => MINIMAL_OPTION_ID.to_string(),
        StartupCreationKind::Starter => format!("starter:{theme_id}"),
        StartupCreationKind::ProjectTemplate => format!("template:{theme_id}"),
    }
}

fn materialize_project<R: Runtime>(
    app: &AppHandle<R>,
    option_id: &str,
) -> Result<MaterializedProject, String> {
    if option_id == MINIMAL_OPTION_ID {
        let settings_source = crate::commands::config::serialize_default_project_settings()?;
        let deploy_source =
            crate::deploy::serialize_deploy_settings(&crate::deploy::DeploySettings::default())?;
        let files = BTreeMap::from([
            (".gitignore".to_string(), BASE_GITIGNORE.as_bytes().to_vec()),
            (
                ".panastudio/settings.toml".to_string(),
                settings_source.into_bytes(),
            ),
            (
                ".panastudio/deploy.toml".to_string(),
                deploy_source.into_bytes(),
            ),
            (
                "content/_index.md".to_string(),
                MINIMAL_SECTION.as_bytes().to_vec(),
            ),
            (
                "templates/index.html".to_string(),
                MINIMAL_TEMPLATE.as_bytes().to_vec(),
            ),
            (
                "zola.toml".to_string(),
                BASE_ZOLA_CONFIG.as_bytes().to_vec(),
            ),
        ]);
        return Ok(MaterializedProject {
            option_id: option_id.to_string(),
            option_kind: StartupCreationKind::Minimal,
            option_name: "Proiect minimal".to_string(),
            registry_version: "minimal-v1".to_string(),
            files,
        });
    }

    let (declared_kind, theme_id) = option_id
        .split_once(':')
        .ok_or_else(|| format!("Opțiune de creare necunoscută: {option_id}"))?;
    let registry = ThemeRegistry::load(app).map_err(|error| error.to_string())?;
    let catalog = registry.snapshot(None)?;
    let pack = registry.require(theme_id)?;
    let option_kind = creation_kind_for_theme(&pack.manifest.id, &pack.manifest.category);
    let expected_prefix = match option_kind {
        StartupCreationKind::Starter => "starter",
        StartupCreationKind::ProjectTemplate => "template",
        StartupCreationKind::Minimal => unreachable!(),
    };
    if declared_kind != expected_prefix {
        return Err(format!(
            "Opțiunea `{option_id}` nu corespunde categoriei validate din catalog."
        ));
    }
    let files = materialize_theme_pack(pack)?;
    Ok(MaterializedProject {
        option_id: option_id.to_string(),
        option_kind,
        option_name: pack.manifest.display_name.clone(),
        registry_version: catalog.registry_version,
        files,
    })
}

fn materialize_theme_pack(pack: &ThemePack) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut files = BTreeMap::new();
    insert_materialized(&mut files, ".gitignore", BASE_GITIGNORE.as_bytes().to_vec())?;
    insert_materialized(
        &mut files,
        ".panastudio/settings.toml",
        crate::commands::config::serialize_default_project_settings()?.into_bytes(),
    )?;
    insert_materialized(
        &mut files,
        ".panastudio/deploy.toml",
        crate::deploy::serialize_deploy_settings(&crate::deploy::DeploySettings::default())?
            .into_bytes(),
    )?;
    let config =
        crate::zola_theme::set_active_theme_in_source(BASE_ZOLA_CONFIG, &pack.manifest.id)?;
    insert_materialized(&mut files, "zola.toml", config.into_bytes())?;
    for file in &pack.theme_files {
        let path = format!("themes/{}/{}", pack.manifest.id, file.relative_path);
        insert_materialized(&mut files, &path, file.bytes.clone())?;
    }
    for file in &pack.recipe_files {
        insert_materialized(&mut files, &file.relative_path, file.bytes.clone())?;
    }
    Ok(files)
}

fn insert_materialized(
    files: &mut BTreeMap<String, Vec<u8>>,
    relative_path: &str,
    bytes: Vec<u8>,
) -> Result<(), String> {
    validate_creation_relative_path(relative_path)?;
    if files.insert(relative_path.to_string(), bytes).is_some() {
        return Err(format!(
            "Catalogul de creare publică de două ori `{relative_path}`."
        ));
    }
    Ok(())
}

fn validate_creation_relative_path(relative_path: &str) -> Result<(), String> {
    let path = Path::new(relative_path);
    if path.is_absolute() || relative_path.contains('\\') {
        return Err(format!("Path de creare nesigur: {relative_path}"));
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(format!("Path de creare nesigur: {relative_path}"));
    }
    if path.components().next().is_none() {
        return Err("Path de creare gol.".to_string());
    }
    Ok(())
}

fn creation_plan_token(
    candidate: &StartupCandidateSnapshot,
    materialized: &MaterializedProject,
) -> String {
    let mut parts = vec![
        STARTUP_CREATION_PLAN_SCHEMA_VERSION.to_string(),
        candidate.snapshot_token.clone(),
        materialized.option_id.clone(),
        materialized.registry_version.clone(),
    ];
    parts.extend(
        materialized
            .files
            .iter()
            .map(|(path, bytes)| format!("{path}\0{}\0{}", bytes.len(), hash_bytes(bytes))),
    );
    digest_parts(parts)
}

fn digest_parts(parts: impl IntoIterator<Item = String>) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    format!("{:x}", hasher.finalize())
}

#[derive(Debug)]
struct PublicationJournal {
    files: Vec<PublishedFile>,
    directories: Vec<PublishedDirectory>,
}

#[derive(Debug)]
struct PublicationFailure {
    error: String,
    journal: PublicationJournal,
}

fn publish_materialized_project<R: Runtime>(
    app: &AppHandle<R>,
    authority: &ProjectCreationAuthority,
    root: &Path,
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<PublicationJournal, PublicationFailure> {
    let mut journal = PublicationJournal {
        files: Vec::new(),
        directories: Vec::new(),
    };
    let mut directories = BTreeSet::new();
    for relative_path in files.keys() {
        let mut parent = Path::new(relative_path).parent();
        while let Some(path) = parent {
            if path.as_os_str().is_empty() {
                break;
            }
            directories.insert(path.to_path_buf());
            parent = path.parent();
        }
    }
    let mut directories = directories.into_iter().collect::<Vec<_>>();
    directories.sort_by_key(|path| path.components().count());

    for relative_path in directories {
        let target = root.join(&relative_path);
        if target.exists() {
            return Err(PublicationFailure {
                error: format!(
                    "Publicarea a refuzat directorul existent {}.",
                    relative_path.display()
                ),
                journal,
            });
        }
        let intent = match creation_intent(
            authority,
            &target,
            &relative_path.to_string_lossy(),
            WriteOperationKind::CreateDirectory,
            WritePolicy::project_creation_lifecycle(),
        ) {
            Ok(intent) => intent,
            Err(error) => return Err(PublicationFailure { error, journal }),
        };
        if let Err(error) = WriteAuthority::new(app).create_directory_all(intent) {
            return Err(PublicationFailure {
                error: error.into_terminal_diagnostic(),
                journal,
            });
        }
        match fs::symlink_metadata(&target) {
            Ok(_) => journal.directories.push(PublishedDirectory {
                relative_path: relative_path.to_string_lossy().replace('\\', "/"),
            }),
            Err(error) => {
                return Err(PublicationFailure {
                    error: format!(
                        "Nu am putut jurnaliza directorul publicat {}: {error}",
                        relative_path.display()
                    ),
                    journal,
                })
            }
        }
    }

    for (relative_path, bytes) in files {
        let target = root.join(relative_path);
        if target.exists() {
            return Err(PublicationFailure {
                error: format!("Publicarea a refuzat fișierul existent {relative_path}."),
                journal,
            });
        }
        let intent = match creation_intent(
            authority,
            &target,
            relative_path,
            WriteOperationKind::WriteBytes,
            WritePolicy::project_creation_write(),
        ) {
            Ok(intent) => intent,
            Err(error) => return Err(PublicationFailure { error, journal }),
        };
        if let Err(error) = WriteAuthority::new(app).write_bytes(intent, bytes) {
            return Err(PublicationFailure {
                error: error.into_terminal_diagnostic(),
                journal,
            });
        }
        match fs::symlink_metadata(&target) {
            Ok(metadata) => journal.files.push(PublishedFile {
                relative_path: relative_path.clone(),
                version_token: project_disk_metadata_version_token(&metadata),
                content_hash: hash_bytes(bytes),
            }),
            Err(error) => {
                return Err(PublicationFailure {
                    error: format!(
                        "Nu am putut jurnaliza fișierul publicat {relative_path}: {error}"
                    ),
                    journal,
                })
            }
        }
    }
    Ok(journal)
}

fn creation_intent(
    authority: &ProjectCreationAuthority,
    target: &Path,
    relative_path: &str,
    operation: WriteOperationKind,
    policy: WritePolicy,
) -> Result<WriteIntent, String> {
    Ok(WriteIntent::new(
        WriteCategory::ProjectSourceWrite,
        WriteOwner::ProjectInitializer,
        operation,
        authority
            .target(target, format!("startup-creation/{relative_path}"))?
            .with_expected_absent(),
        policy,
        "Rust-first Startup project publication",
    ))
}

fn rollback_publication<R: Runtime>(
    app: &AppHandle<R>,
    authority: &ProjectCreationAuthority,
    root: &Path,
    journal: &PublicationJournal,
) -> Result<(), String> {
    let mut failures = Vec::new();
    for published in journal.files.iter().rev() {
        let target = root.join(&published.relative_path);
        let intent = WriteIntent::new(
            WriteCategory::ProjectSourceWrite,
            WriteOwner::ProjectInitializer,
            WriteOperationKind::RemoveFile,
            authority
                .target(
                    &target,
                    format!("startup-creation/rollback/{}", published.relative_path),
                )?
                .with_expected_present(
                    published.version_token.clone(),
                    Some(published.content_hash.clone()),
                ),
            WritePolicy::project_creation_lifecycle(),
            "Rust-first Startup exact file rollback",
        );
        if let Err(error) = WriteAuthority::new(app).remove_file_if_exists(intent) {
            failures.push(format!(
                "{}: {}",
                published.relative_path,
                error.diagnostic()
            ));
        }
    }

    let empty_tree_fingerprint = hash_bytes(&[]);
    for published in journal.directories.iter().rev() {
        let target = root.join(&published.relative_path);
        let metadata = match fs::symlink_metadata(&target) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                failures.push(format!("{}: {}", published.relative_path, error));
                continue;
            }
        };
        let intent = WriteIntent::new(
            WriteCategory::ProjectSourceWrite,
            WriteOwner::ProjectInitializer,
            WriteOperationKind::RemoveDirectoryTree,
            authority
                .target(
                    &target,
                    format!(
                        "startup-creation/rollback-directory/{}",
                        published.relative_path
                    ),
                )?
                .with_expected_present_tree(
                    project_disk_metadata_version_token(&metadata),
                    empty_tree_fingerprint.clone(),
                ),
            WritePolicy::project_creation_lifecycle(),
            "Rust-first Startup exact empty-directory rollback",
        );
        if let Err(error) = WriteAuthority::new(app).remove_directory_tree_if_exists(intent) {
            failures.push(format!(
                "{}: {}",
                published.relative_path,
                error.diagnostic()
            ));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Rollback-ul exact a păstrat intrări schimbate extern: {}",
            failures.join(" | ")
        ))
    }
}

fn format_creation_failure(error: String, rollback: Result<(), String>) -> String {
    match rollback {
        Ok(()) => format!(
            "Crearea proiectului a eșuat, iar publicațiile Pană Studio au fost retrase exact: {error}"
        ),
        Err(rollback_error) => format!(
            "Crearea proiectului a eșuat ({error}). {rollback_error}"
        ),
    }
}

fn fail_creation(
    runtime: &StartupFlowRuntime,
    error: String,
    rollback: Result<(), String>,
) -> String {
    let message = format_creation_failure(error, rollback);
    let _ = runtime.set_stage(StartupStage::Error);
    message
}

fn require_empty_root(root: &Path) -> Result<(), String> {
    if !root.is_dir() {
        return Err(format!(
            "Candidatul nu mai este un dosar: {}",
            root.display()
        ));
    }
    if fs::read_dir(root)
        .map_err(|error| format!("Nu am putut reverifica dosarul: {error}"))?
        .next()
        .is_some()
    {
        return Err(
            "Dosarul nu mai este gol; publicarea a fost refuzată fără suprascriere.".to_string(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_home::{ensure_app_home, TEST_APP_ENV_LOCK};
    use std::{
        env,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn detached_inspection_classifies_empty_unrecognized_invalid_and_valid() {
        let fixture = temp_dir("classifications");
        let empty = fixture.join("empty");
        let unrecognized = fixture.join("unrecognized");
        let invalid = fixture.join("invalid");
        let valid = fixture.join("valid");
        fs::create_dir_all(&empty).unwrap();
        fs::create_dir_all(&unrecognized).unwrap();
        fs::write(unrecognized.join("readme.txt"), "text").unwrap();
        fs::create_dir_all(invalid.join("content")).unwrap();
        fs::write(invalid.join("zola.toml"), "not = [valid").unwrap();
        create_minimal_fixture(&valid);

        assert_eq!(
            inspect_candidate_root(&empty).unwrap().kind,
            StartupCandidateKind::EmptyDirectory
        );
        assert_eq!(
            inspect_candidate_root(&unrecognized).unwrap().kind,
            StartupCandidateKind::UnrecognizedDirectory
        );
        assert_eq!(
            inspect_candidate_root(&invalid).unwrap().kind,
            StartupCandidateKind::InvalidZolaProject
        );
        assert_eq!(
            inspect_candidate_root(&valid).unwrap().kind,
            StartupCandidateKind::ValidProject
        );
        cleanup(fixture);
    }

    #[test]
    fn detached_inspection_accepts_unreachable_external_links() {
        let fixture = temp_dir("offline-external-links");
        let project = fixture.join("project");
        create_minimal_fixture(&project);
        fs::write(
            project.join("content/_index.md"),
            concat!(
                "+++\ntitle = \"Acasă\"\nsort_by = \"weight\"\n+++\n\n",
                "[Serviciu extern](http://127.0.0.1:9/offline)\n",
            ),
        )
        .unwrap();

        let candidate = inspect_candidate_root(&project).unwrap();

        assert_eq!(candidate.kind, StartupCandidateKind::ValidProject);
        cleanup(fixture);
    }

    #[test]
    fn stale_empty_snapshot_is_rejected_without_modifying_external_file() {
        let _lock = TEST_APP_ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let fixture = temp_dir("stale");
        let root = fixture.join("candidate");
        fs::create_dir_all(&root).unwrap();
        let _env = TestEnvGuard::from_root(&fixture.join("app-home"));
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        ensure_app_home(app.handle()).unwrap();
        let runtime = StartupFlowRuntime::default();
        let inspected = runtime.inspect(&root).unwrap();
        let token = inspected.candidate.unwrap().snapshot_token;
        let plan = plan_creation(
            app.handle(),
            &runtime,
            StartupCreationPlanRequest {
                expected_snapshot_token: token.clone(),
                option_id: "minimal".to_string(),
            },
        )
        .unwrap();

        fs::write(root.join("external.txt"), "external").unwrap();
        let error = apply_creation(
            app.handle(),
            &runtime,
            StartupCreationApplyRequest {
                expected_snapshot_token: token,
                expected_plan_token: plan.plan_token,
            },
        )
        .unwrap_err();

        assert!(error.contains("fără modificări"));
        assert_eq!(
            fs::read_to_string(root.join("external.txt")).unwrap(),
            "external"
        );
        assert!(!root.join("zola.toml").exists());
        cleanup(fixture);
    }

    #[test]
    fn exact_rollback_preserves_an_externally_changed_publication() {
        let _lock = TEST_APP_ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let fixture = temp_dir("rollback-external-change");
        let root = fixture.join("candidate");
        fs::create_dir_all(&root).unwrap();
        let _env = TestEnvGuard::from_root(&fixture.join("app-home"));
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        ensure_app_home(app.handle()).unwrap();
        let materialized = materialize_project(app.handle(), "minimal").unwrap();
        let authority = ProjectCreationAuthority::capture(&root).unwrap();
        let journal =
            publish_materialized_project(app.handle(), &authority, &root, &materialized.files)
                .unwrap();

        fs::write(root.join("zola.toml"), "external = true\n").unwrap();
        let error = rollback_publication(app.handle(), &authority, &root, &journal).unwrap_err();

        assert!(error.contains("schimbate extern"));
        assert_eq!(
            fs::read_to_string(root.join("zola.toml")).unwrap(),
            "external = true\n"
        );
        assert!(!root.join(".gitignore").exists());
        assert!(!root.join("content").exists());
        assert!(!root.join("templates").exists());
        cleanup(fixture);
    }

    #[test]
    fn every_creation_option_produces_a_valid_project() {
        let _lock = TEST_APP_ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let fixture = temp_dir("all-options");
        let _env = TestEnvGuard::from_root(&fixture.join("app-home"));
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        ensure_app_home(app.handle()).unwrap();

        for option_id in [
            "minimal",
            "starter:pana-studio",
            "template:nord",
            "template:cadru",
            "template:radacini",
        ] {
            let root = fixture.join(option_id.replace(':', "-"));
            fs::create_dir_all(&root).unwrap();
            let runtime = StartupFlowRuntime::default();
            let inspected = runtime.inspect(&root).unwrap();
            let token = inspected.candidate.unwrap().snapshot_token;
            let plan = plan_creation(
                app.handle(),
                &runtime,
                StartupCreationPlanRequest {
                    expected_snapshot_token: token.clone(),
                    option_id: option_id.to_string(),
                },
            )
            .unwrap();
            let receipt = apply_creation(
                app.handle(),
                &runtime,
                StartupCreationApplyRequest {
                    expected_snapshot_token: token,
                    expected_plan_token: plan.plan_token,
                },
            )
            .unwrap();
            assert_eq!(
                receipt.startup.candidate.unwrap().kind,
                StartupCandidateKind::ValidProject
            );
        }
        cleanup(fixture);
    }

    fn create_minimal_fixture(root: &Path) {
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(root.join("zola.toml"), BASE_ZOLA_CONFIG).unwrap();
        fs::write(root.join("content/_index.md"), MINIMAL_SECTION).unwrap();
        fs::write(root.join("templates/index.html"), MINIMAL_TEMPLATE).unwrap();
    }

    fn temp_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "pana-startup-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn cleanup(path: PathBuf) {
        let _ = fs::remove_dir_all(path);
    }

    struct TestEnvGuard {
        previous_values: Vec<(&'static str, Option<String>)>,
    }

    impl TestEnvGuard {
        fn from_root(root: &Path) -> Self {
            let bindings = [
                ("XDG_CONFIG_HOME", root.join("config")),
                ("XDG_DATA_HOME", root.join("data")),
                ("XDG_CACHE_HOME", root.join("cache")),
                ("XDG_STATE_HOME", root.join("state")),
            ];
            let previous_values = bindings
                .iter()
                .map(|(key, _)| (*key, env::var(key).ok()))
                .collect();
            for (key, path) in bindings {
                env::set_var(key, path);
            }
            Self { previous_values }
        }
    }

    impl Drop for TestEnvGuard {
        fn drop(&mut self) {
            for (key, value) in &self.previous_values {
                match value {
                    Some(value) => env::set_var(key, value),
                    None => env::remove_var(key),
                }
            }
        }
    }
}
