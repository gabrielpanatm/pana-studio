use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc, RwLock,
    },
    thread,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use sha2::{Digest, Sha256};
use tauri::{AppHandle, Runtime};
use tauri_utils::html::{parse, serialize_node};
use tera::Context;
use zola_site::{BuildMode, Site, SITE_CONTENT};

use crate::{
    js::{
        generate_motion_preview_payload, js_relative_path, parse_motion_source,
        template_path_from_motion_source,
    },
    kernel::{
        project_workspace::WorkspaceProjectionSnapshot,
        write_authority::{
            PendingProjectAuthority, WriteAuthority, WriteCategory, WriteIntent,
            WriteOperationKind, WriteOwner, WritePolicy, WriteTarget,
        },
    },
    preview::{
        inject::{
            bind_canvas_identity_to_editor_html, bind_canvas_identity_to_initial_preview_html,
            prepare_design_safe_html_with_motion_payload,
            prepare_initial_preview_html_with_motion_payload, PreviewResourceVersions,
        },
        preprocess::{
            create_persistent_preview_artifact_root, persistent_project_workspace_session_root,
            remove_persistent_preview_artifact_root, remove_persistent_preview_session,
            sync_persistent_project_workspace, PersistentProjectionManifest,
            PersistentProjectionUpdate,
        },
        server::{ActivePreviewGeneration, PersistentPreviewServer, RenderedPreviewContent},
        CanvasGraph, CanvasProjectionTransaction, CanvasResourceManifest, PreviewImpact,
        PreviewPhaseReceipt,
    },
    project_model::{
        build_project_model_from_workspace_projection,
        model::ProjectModel,
        template_workbench::{TemplateWorkbenchPlan, TemplateWorkbenchRenderMode},
    },
    zola_engine::{with_zola_engine, zola_config_file, EMBEDDED_ZOLA_VERSION},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PersistentPreviewOwner {
    pub project_root: String,
    pub runtime_session_id: String,
}

impl PersistentPreviewOwner {
    pub fn new(project_root: impl Into<String>, runtime_session_id: impl Into<String>) -> Self {
        Self {
            project_root: project_root.into(),
            runtime_session_id: runtime_session_id.into(),
        }
    }

    fn matches_generation(&self, generation: &ActivePreviewGeneration) -> bool {
        generation.owner_matches(&self.project_root, &self.runtime_session_id)
    }
}

pub(crate) struct PersistentPreviewCandidate {
    generation: Arc<ActivePreviewGeneration>,
    project_model: Arc<ProjectModel>,
    pub projected_paths: Vec<String>,
    pub projection_publication: crate::kernel::write_authority::PreviewProjectionPublicationStats,
    pub timings: PersistentPreviewCandidateTimings,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PersistentPreviewCandidateTimings {
    pub total_ms: u64,
    pub source_publication_ms: u64,
    pub artifact_setup_ms: u64,
    pub project_model_build_ms: u64,
    pub project_model_join_wait_ms: u64,
    pub project_model_cache_hit: bool,
    pub zola_render_ms: u64,
    pub rendered_content_clone_ms: u64,
    pub content_prepare_ms: u64,
    pub canvas_graph_ms: u64,
    pub resource_manifest_ms: u64,
    pub canvas_transaction_ms: u64,
    pub source_retirement_ms: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct TemplateWorkbenchPublication {
    pub route: String,
    pub preview_url: String,
    pub reuse_token: String,
    pub workspace_revision: u64,
    pub preview_revision: String,
    pub canvas_plan: crate::preview::CanvasProjectionPlan,
    pub cache_reused: bool,
    pub timings: TemplateWorkbenchPublicationTimings,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TemplateWorkbenchReuseConfirmation {
    pub route: String,
    pub preview_url: String,
    pub workspace_revision: u64,
    pub preview_revision: String,
    pub canvas_transaction_id: String,
    pub reuse_token: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TemplateWorkbenchReuseQuery<'a> {
    pub workspace_revision: u64,
    pub template_path: &'a str,
    pub preferred_page_path: Option<&'a str>,
    pub preferred_route: Option<&'a str>,
    pub preferred_component_name: Option<&'a str>,
    pub reuse_token: &'a str,
    pub expected_preview_revision: &'a str,
    pub expected_canvas_transaction_id: &'a str,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct TemplateWorkbenchPublicationTimings {
    pub total_us: u64,
    pub render_us: u64,
    pub graph_us: u64,
    pub prepare_us: u64,
}

impl PersistentPreviewCandidate {
    pub fn canvas_plan(&self) -> crate::preview::CanvasProjectionPlan {
        self.generation.canvas_transaction.plan()
    }

    pub fn project_model_arc(&self) -> Arc<ProjectModel> {
        self.project_model.clone()
    }
}

pub(crate) struct PersistentZolaPreviewEngine {
    owner: PersistentPreviewOwner,
    zola_root: PathBuf,
    session_root: PathBuf,
    projection_manifest: Option<PersistentProjectionManifest>,
    site: Option<Site>,
    raw_content: HashMap<String, String>,
    server: Option<PersistentPreviewServer>,
    retired: Vec<Arc<ActivePreviewGeneration>>,
}

impl PersistentZolaPreviewEngine {
    pub fn start<R: Runtime>(
        app: &AppHandle<R>,
        zola_root: &Path,
        owner: PersistentPreviewOwner,
    ) -> Result<Self, String> {
        let zola_root = zola_root
            .canonicalize()
            .unwrap_or_else(|_| zola_root.to_path_buf());
        let session_root =
            persistent_project_workspace_session_root(app, &zola_root, &owner.runtime_session_id)?;
        // Clear only this runtime session's private residue. A provisional
        // opening of the same project must not invalidate the still-active
        // session before the lifecycle commit point.
        remove_persistent_preview_session(app, &zola_root, &session_root)?;
        let server = PersistentPreviewServer::start()?;
        Ok(Self {
            owner,
            zola_root,
            session_root,
            projection_manifest: None,
            site: None,
            raw_content: HashMap::new(),
            server: Some(server),
            retired: Vec::new(),
        })
    }

    pub fn owner_matches(&self, owner: &PersistentPreviewOwner) -> bool {
        self.owner == *owner
    }

    pub fn url(&self) -> Result<String, String> {
        self.server
            .as_ref()
            .map(PersistentPreviewServer::url)
            .ok_or_else(|| "Serverul Preview persistent a fost oprit.".to_string())
    }

    pub fn active_generation(&self) -> Result<Option<Arc<ActivePreviewGeneration>>, String> {
        self.server
            .as_ref()
            .ok_or_else(|| "Serverul Preview persistent a fost oprit.".to_string())?
            .active()
    }

    pub fn active_matches_revision(&self, workspace_revision: u64) -> Result<bool, String> {
        Ok(self.active_generation()?.is_some_and(|generation| {
            self.owner.matches_generation(&generation)
                && generation.workspace_revision == workspace_revision
        }))
    }

    pub fn generation_for_workspace_revision(
        &self,
        workspace_revision: u64,
    ) -> Result<Option<Arc<ActivePreviewGeneration>>, String> {
        self.server
            .as_ref()
            .ok_or_else(|| "Serverul Preview persistent a fost oprit.".to_string())?
            .generation_for_workspace_revision(
                &self.owner.project_root,
                &self.owner.runtime_session_id,
                workspace_revision,
            )
    }

    pub fn canvas_plan_for_identity(
        &self,
        identity: &crate::preview::CanvasProjectionIdentity,
    ) -> Result<Option<crate::preview::CanvasProjectionPlan>, String> {
        self.server
            .as_ref()
            .ok_or_else(|| "Serverul Preview persistent a fost oprit.".to_string())?
            .canvas_plan_for_identity(identity)
    }

    pub(crate) fn generation_for_canvas_identity(
        &self,
        identity: &crate::preview::CanvasProjectionIdentity,
    ) -> Result<Option<Arc<ActivePreviewGeneration>>, String> {
        self.server
            .as_ref()
            .ok_or_else(|| "Serverul Preview persistent a fost oprit.".to_string())?
            .generation_for_canvas_identity(identity)
    }

    /// Confirms a previously materialized Workbench projection without
    /// rebuilding ProjectWorkspace/ProjectModel or serializing their plans.
    /// Every field belongs to the Rust-issued generation and cache entry; a
    /// mismatch is a cache miss and must fall back to full publication.
    pub fn confirm_template_workbench_reuse(
        &self,
        query: TemplateWorkbenchReuseQuery<'_>,
    ) -> Result<Option<TemplateWorkbenchReuseConfirmation>, String> {
        let Some(generation) = self.generation_for_workspace_revision(query.workspace_revision)?
        else {
            return Ok(None);
        };
        if generation.preview_revision != query.expected_preview_revision
            || generation.canvas_transaction.identity.transaction_id
                != query.expected_canvas_transaction_id
        {
            return Ok(None);
        }
        let workbench_content = generation
            .workbench_content
            .read()
            .map_err(|_| "Registrul Context de template este indisponibil.".to_string())?;
        let Some((route, projection)) = workbench_content.iter().find(|(_, projection)| {
            projection.reuse_token == query.reuse_token
                && projection.template_path == query.template_path
                && projection.selected_page_path.as_deref() == query.preferred_page_path
                && projection.selected_route.as_deref() == query.preferred_route
                && projection.selected_component_name.as_deref() == query.preferred_component_name
        }) else {
            return Ok(None);
        };
        let preview_url = format!(
            "{}{}?__pana_preview_revision={}&__pana_canvas_transaction={}",
            self.url()?,
            route,
            generation.preview_revision,
            generation.canvas_transaction.identity.transaction_id,
        );
        Ok(Some(TemplateWorkbenchReuseConfirmation {
            route: route.clone(),
            preview_url,
            workspace_revision: query.workspace_revision,
            preview_revision: generation.preview_revision.clone(),
            canvas_transaction_id: generation
                .canvas_transaction
                .identity
                .transaction_id
                .clone(),
            reuse_token: projection.reuse_token.clone(),
        }))
    }

    /// Randă template-ul ales în motorul Zola deja încărcat și îl publică în
    /// generația exactă a projection-ului. Generația poate fi încă staged: astfel
    /// Workbench-ul montat poate confirma chiar candidatul canonic, fără să
    /// revină temporar la pagina site-ului sau la generația precedentă.
    pub fn publish_template_workbench_view(
        &mut self,
        projection: &WorkspaceProjectionSnapshot,
        model: &ProjectModel,
        plan: &TemplateWorkbenchPlan,
    ) -> Result<TemplateWorkbenchPublication, String> {
        let started = Instant::now();
        self.require_projection_owner(projection)?;
        if model.project_root != Path::new(&projection.project_root) {
            return Err(
                "Context de template a refuzat un ProjectModel din alt proiect.".to_string(),
            );
        }
        let generation = self
            .server
            .as_ref()
            .ok_or_else(|| "Serverul Preview persistent a fost oprit.".to_string())?
            .generation_for_workspace_revision(
                &self.owner.project_root,
                &self.owner.runtime_session_id,
                projection.revision,
            )?
            .ok_or_else(|| {
                format!(
                    "Context de template nu găsește generația Preview exactă pentru revizia {}.",
                    projection.revision
                )
            })?;
        let route = template_workbench_route(&plan.active_template.source_id);
        let context_key = template_workbench_projection_key(plan);
        let cached_reuse_token = generation
            .workbench_content
            .read()
            .map_err(|_| "Registrul Context de template este indisponibil.".to_string())?
            .get(&route)
            .filter(|projection| projection.context_key == context_key)
            .map(|projection| projection.reuse_token.clone());
        let preview_url = format!(
            "{}{}?__pana_preview_revision={}&__pana_canvas_transaction={}",
            self.url()?,
            route,
            generation.preview_revision,
            generation.canvas_transaction.identity.transaction_id
        );
        if let Some(reuse_token) = cached_reuse_token {
            let timings = TemplateWorkbenchPublicationTimings {
                total_us: elapsed_us(started),
                ..TemplateWorkbenchPublicationTimings::default()
            };
            #[cfg(debug_assertions)]
            eprintln!(
                "[Pană Studio][perf] template_projection source={} cache_hit=true total_us={}",
                plan.active_template.file, timings.total_us,
            );
            return Ok(TemplateWorkbenchPublication {
                route,
                preview_url,
                reuse_token,
                workspace_revision: projection.revision,
                preview_revision: generation.preview_revision.clone(),
                canvas_plan: generation.canvas_transaction.plan(),
                cache_reused: true,
                timings,
            });
        }
        let site = self.site.as_ref().ok_or_else(|| {
            "Motorul Zola embedded nu are site activ pentru Workbench.".to_string()
        })?;
        let render_started = Instant::now();
        let (rendered, _canvas_route) = with_zola_engine("Context de template Preview", || {
            render_template_workbench_document(site, &self.raw_content, model, plan)
        })?;
        let render_us = elapsed_us(render_started);
        let graph_started = Instant::now();
        let annotated = CanvasGraph::annotate_rendered_document(model, &route, &rendered)?;
        let graph = CanvasGraph::from_rendered_documents(
            model,
            projection.revision,
            &generation.preview_revision,
            [(route.as_str(), annotated.as_str())],
        )?;
        let graph_us = elapsed_us(graph_started);
        let prepare_started = Instant::now();
        let resource_versions = PreviewResourceVersions::from_entries(
            generation
                .canvas_transaction
                .resources
                .entries
                .iter()
                .map(|entry| (entry.url.clone(), entry.content_hash.clone())),
        );
        let motion_payloads = motion_preview_payload_catalog(model)?;
        let motion_payload = rendered_motion_preview_payload(&annotated, &motion_payloads)?;
        let mut prepared = prepare_design_safe_html_with_motion_payload(
            &annotated,
            &generation.preview_revision,
            &route,
            &resource_versions,
            motion_payload,
        )?;
        bind_canvas_identity_to_editor_html(
            &mut prepared,
            &generation.canvas_transaction.identity,
        )?;
        let prepare_us = elapsed_us(prepare_started);
        let reuse_token = template_workbench_reuse_token(&generation, &route, &context_key);

        generation
            .workbench_content
            .write()
            .map_err(|_| "Registrul Context de template este indisponibil.".to_string())?
            .insert(
                route.clone(),
                crate::preview::server::TemplateWorkbenchProjection {
                    context_key,
                    reuse_token: reuse_token.clone(),
                    template_path: plan.active_template.file.clone(),
                    selected_page_path: plan
                        .selected_context
                        .as_ref()
                        .map(|context| context.page_file.clone()),
                    selected_route: plan
                        .selected_route
                        .as_ref()
                        .map(|context| context.url.clone()),
                    selected_component_name: plan.active_component_name.clone(),
                    content: RenderedPreviewContent::Html(prepared),
                    graph,
                },
            );
        let timings = TemplateWorkbenchPublicationTimings {
            total_us: elapsed_us(started),
            render_us,
            graph_us,
            prepare_us,
        };
        #[cfg(debug_assertions)]
        eprintln!(
            "[Pană Studio][perf] template_projection source={} cache_hit=false render_us={} graph_us={} prepare_us={} total_us={}",
            plan.active_template.file,
            timings.render_us,
            timings.graph_us,
            timings.prepare_us,
            timings.total_us,
        );
        Ok(TemplateWorkbenchPublication {
            route,
            preview_url,
            reuse_token,
            workspace_revision: projection.revision,
            preview_revision: generation.preview_revision.clone(),
            canvas_plan: generation.canvas_transaction.plan(),
            cache_reused: false,
            timings,
        })
    }

    pub fn render_candidate_with_project_model<R: Runtime>(
        &mut self,
        app: &AppHandle<R>,
        projection: &WorkspaceProjectionSnapshot,
        project_model: Arc<ProjectModel>,
        project_model_cache_hit: bool,
    ) -> Result<PersistentPreviewCandidate, String> {
        self.render_candidate_with_project_model_and_pending_authority(
            app,
            projection,
            Some(project_model),
            None,
            Some(project_model_cache_hit),
        )
    }

    pub(crate) fn render_candidate_with_pending_project_authority<R: Runtime>(
        &mut self,
        app: &AppHandle<R>,
        projection: &WorkspaceProjectionSnapshot,
        pending_project_authority: &PendingProjectAuthority,
    ) -> Result<PersistentPreviewCandidate, String> {
        self.render_candidate_with_project_model_and_pending_authority(
            app,
            projection,
            None,
            Some(pending_project_authority),
            None,
        )
    }

    fn render_candidate_with_project_model_and_pending_authority<R: Runtime>(
        &mut self,
        app: &AppHandle<R>,
        projection: &WorkspaceProjectionSnapshot,
        project_model: Option<Arc<ProjectModel>>,
        pending_project_authority: Option<&PendingProjectAuthority>,
        project_model_cache_hit: Option<bool>,
    ) -> Result<PersistentPreviewCandidate, String> {
        self.render_candidate_with_canonical_model(
            app,
            projection,
            project_model,
            pending_project_authority,
            project_model_cache_hit,
        )
    }

    fn render_candidate_with_canonical_model<R: Runtime>(
        &mut self,
        app: &AppHandle<R>,
        projection: &WorkspaceProjectionSnapshot,
        project_model: Option<Arc<ProjectModel>>,
        pending_project_authority: Option<&PendingProjectAuthority>,
        project_model_cache_hit: Option<bool>,
    ) -> Result<PersistentPreviewCandidate, String> {
        let candidate_started = Instant::now();
        let mut timings = PersistentPreviewCandidateTimings {
            project_model_cache_hit: project_model_cache_hit.unwrap_or(project_model.is_some()),
            ..PersistentPreviewCandidateTimings::default()
        };
        self.require_projection_owner(projection)?;
        self.collect_retired(app);
        // Preview annotation consumes the same canonical SourceGraph as every
        // mutation command. Resolve it before materialization so a parser-local
        // reconstruction can never become a second identity authority.
        let project_model = if let Some(project_model) = project_model {
            project_model
        } else {
            let model_started = Instant::now();
            let model_root = PathBuf::from(&projection.project_root);
            let model = build_project_model_from_workspace_projection(&model_root, projection)?;
            timings.project_model_build_ms = elapsed_ms(model_started);
            Arc::new(model)
        };

        let source_publication_started = Instant::now();
        let (mut update, source_publication) = match sync_persistent_project_workspace(
            app,
            &self.zola_root,
            &self.session_root,
            self.projection_manifest.as_ref(),
            projection,
            &project_model.source_graph,
            pending_project_authority,
        ) {
            Ok(update) => update,
            Err(error) => {
                // Pre-publication failures preserve the active source name.
                // A publication error may be effect-visible but uncertain, so
                // the next attempt must never trust the previous manifest.
                self.projection_manifest = None;
                self.site = None;
                self.raw_content.clear();
                return Err(error);
            }
        };
        timings.source_publication_ms = elapsed_ms(source_publication_started);
        self.projection_manifest = Some(update.manifest.clone());

        let result = (|| {
            let preview_revision = next_preview_revision(projection.revision);
            let artifact_setup_started = Instant::now();
            let artifact_root = create_persistent_preview_artifact_root(
                app,
                &self.session_root,
                &preview_revision,
            )?;
            timings.artifact_setup_ms = elapsed_ms(artifact_setup_started);
            let (generation, project_model) = match self.render_zola_generation(
                app,
                &update,
                &artifact_root,
                projection,
                &preview_revision,
                project_model,
                &mut timings,
            ) {
                Ok(result) => result,
                Err(error) => {
                    let cleanup = remove_persistent_preview_artifact_root(
                        app,
                        &self.session_root,
                        &artifact_root,
                    );
                    return Err(match cleanup {
                        Ok(()) => error,
                        Err(cleanup_error) => {
                            format!("{error} Cleanup candidat eșuat: {cleanup_error}")
                        }
                    });
                }
            };
            Ok((generation, project_model))
        })();
        let source_retirement_started = Instant::now();
        match source_publication.retire_previous() {
            Ok(retirement_operations) => {
                update.publication_stats.durability_operations = update
                    .publication_stats
                    .durability_operations
                    .saturating_add(retirement_operations);
            }
            Err(error) => {
                eprintln!(
                    "[Pană Studio] Retirement-ul generației sursă Preview a eșuat; teardown-ul sesiunii va reîncerca: {error}"
                );
            }
        }
        timings.source_retirement_ms = elapsed_ms(source_retirement_started);
        if result.is_err() {
            self.projection_manifest = None;
            self.site = None;
            self.raw_content.clear();
        }
        let (generation, project_model) = result?;
        timings.total_ms = elapsed_ms(candidate_started);

        Ok(PersistentPreviewCandidate {
            generation: Arc::new(generation),
            project_model,
            projected_paths: update.projected_paths,
            projection_publication: update.publication_stats,
            timings,
        })
    }

    pub fn stage_candidate<R: Runtime>(
        &mut self,
        app: &AppHandle<R>,
        candidate: PersistentPreviewCandidate,
    ) -> Result<Arc<ActivePreviewGeneration>, String> {
        if !self.owner.matches_generation(&candidate.generation) {
            return Err("Candidatul Canvas aparține altei sesiuni.".to_string());
        }
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| "Serverul Preview persistent a fost oprit.".to_string())?;
        let generation = candidate.generation;
        self.retired.extend(server.stage(Arc::clone(&generation))?);
        self.collect_retired(app);
        Ok(generation)
    }

    pub fn acknowledge_candidate_phase<R: Runtime>(
        &mut self,
        app: &AppHandle<R>,
        receipt: &PreviewPhaseReceipt,
    ) -> Result<Arc<ActivePreviewGeneration>, String> {
        if receipt.identity.project_root != self.owner.project_root
            || receipt.identity.runtime_session_id != self.owner.runtime_session_id
        {
            return Err("ACK-ul Canvas aparține altei sesiuni Preview.".to_string());
        }
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| "Serverul Preview persistent a fost oprit.".to_string())?;
        let transition = server.acknowledge_phase(receipt)?;
        if let Some(previous) = transition.previous_active {
            self.retired.push(previous);
        }
        if transition.discarded {
            self.retired.push(Arc::clone(&transition.generation));
        }
        self.collect_retired(app);
        Ok(transition.generation)
    }

    pub fn discard_candidate<R: Runtime>(
        &mut self,
        app: &AppHandle<R>,
        candidate: PersistentPreviewCandidate,
    ) -> Result<(), String> {
        if candidate.generation.inherited_assets.is_some() {
            drop(candidate);
            return Ok(());
        }
        let artifact_root = candidate.generation.assets_root.clone();
        drop(candidate);
        remove_persistent_preview_artifact_root(app, &self.session_root, &artifact_root)
    }

    pub fn stop<R: Runtime>(mut self, app: &AppHandle<R>) -> Result<(), String> {
        if let Some(server) = self.server.take() {
            server.stop();
        }
        self.retired.clear();
        remove_persistent_preview_session(app, &self.zola_root, &self.session_root)
    }

    // Rendering keeps projection evidence, optional model work and timing output independently typed.
    #[allow(clippy::too_many_arguments)]
    fn render_zola_generation<R: Runtime>(
        &mut self,
        app: &AppHandle<R>,
        update: &PersistentProjectionUpdate,
        artifact_root: &Path,
        projection: &WorkspaceProjectionSnapshot,
        preview_revision: &str,
        project_model: Arc<ProjectModel>,
        timings: &mut PersistentPreviewCandidateTimings,
    ) -> Result<(ActivePreviewGeneration, Arc<ProjectModel>), String> {
        let base_url = self.url()?;
        let previous_generation = self.active_generation()?;
        let impact = projection_render_impact(
            update,
            self.site.is_some() && previous_generation.is_some(),
            !self.raw_content.is_empty(),
        );
        let mut generation_assets_root = artifact_root.to_path_buf();
        let mut inherited_assets = None;
        let zola_render_started = Instant::now();
        let rendered = with_zola_engine("randare Preview persistentă", || match impact {
            ProjectionRenderImpact::Full => build_new_official_zola_site(
                &update.projection_root,
                artifact_root,
                &base_url,
                projection.revision,
                DraftRenderPolicy::Include,
            )
            .map(|(site, rendered)| {
                self.site = Some(site);
                rendered
            }),
            ProjectionRenderImpact::Reload => self
                .site
                .as_mut()
                .ok_or_else(|| "Motorul Zola persistent nu are site activ.".to_string())
                .and_then(|site| {
                    render_incremental_template_generation(
                        site,
                        artifact_root,
                        &base_url,
                        projection.revision,
                    )
                }),
            ProjectionRenderImpact::Reuse => {
                let previous = previous_generation.clone().ok_or_else(|| {
                    "Motorul Preview nu are o generație publicată pentru reutilizare.".to_string()
                })?;
                remove_persistent_preview_artifact_root(app, &self.session_root, artifact_root)?;
                generation_assets_root = previous.assets_root.clone();
                inherited_assets = Some(previous);
                Ok(self.raw_content.clone())
            }
        });
        timings.zola_render_ms = elapsed_ms(zola_render_started);
        let rendered = match rendered {
            Ok(rendered) => rendered,
            Err(error) => {
                // Zola's incremental APIs mutate Site before returning. A
                // failed candidate must never be reused as the basis of a
                // later revision; the published generation remains intact.
                self.site = None;
                self.raw_content.clear();
                return Err(error);
            }
        };
        let rendered_content_clone_started = Instant::now();
        self.raw_content = rendered.clone();
        timings.rendered_content_clone_ms = elapsed_ms(rendered_content_clone_started);
        let model = &project_model;
        let resource_manifest_started = Instant::now();
        let resources = if let Some(previous) = inherited_assets.as_ref() {
            let mut reused = previous.canvas_transaction.resources.clone();
            reused.preview_revision = preview_revision.to_string();
            reused
        } else {
            CanvasResourceManifest::from_revisioned_artifact_root(
                preview_revision,
                &generation_assets_root,
                |target, contents| {
                    let intent = WriteIntent::new(
                        WriteCategory::PreviewWorkspaceWrite,
                        WriteOwner::Preview,
                        WriteOperationKind::WriteBytes,
                        WriteTarget::new(
                            target.to_path_buf(),
                            generation_assets_root.clone(),
                            "preview/revisioned-stylesheet",
                        ),
                        WritePolicy::preview_workspace_atomic(),
                        "Versionare URL resurse CSS Preview",
                    );
                    WriteAuthority::new(app)
                        .write_bytes(intent, contents)
                        .map(|_| ())
                        .map_err(|error| error.into_terminal_diagnostic())
                },
            )?
        };
        let resource_manifest_ms = elapsed_ms(resource_manifest_started);
        let resource_versions = PreviewResourceVersions::from_entries(
            resources
                .entries
                .iter()
                .map(|entry| (entry.url.clone(), entry.content_hash.clone())),
        );
        let content_prepare_started = Instant::now();
        let content =
            prepare_generation_content(model, rendered, preview_revision, &resource_versions)?;
        let content_prepare_ms = elapsed_ms(content_prepare_started);
        let rendered_documents = content
            .iter()
            .filter_map(|(content_key, rendered)| match rendered {
                RenderedPreviewContent::Html(html) => Some((
                    canvas_route_for_content_key(content_key),
                    html.editor.as_str(),
                )),
                RenderedPreviewContent::InitialHtml(html) => Some((
                    canvas_route_for_content_key(content_key),
                    html.visitor.as_str(),
                )),
                RenderedPreviewContent::Text { .. } => None,
            })
            .collect::<Vec<_>>();
        let canvas_graph_started = Instant::now();
        let graph = CanvasGraph::from_rendered_documents(
            model,
            projection.revision,
            preview_revision,
            rendered_documents
                .iter()
                .map(|(route, html)| (route.as_str(), *html)),
        )?;
        let canvas_graph_ms = elapsed_ms(canvas_graph_started);
        timings.content_prepare_ms = content_prepare_ms;
        timings.canvas_graph_ms = canvas_graph_ms;
        timings.resource_manifest_ms = resource_manifest_ms;
        let canvas_transaction_started = Instant::now();
        let canvas_transaction = CanvasProjectionTransaction::prepared(
            &self.owner.project_root,
            &self.owner.runtime_session_id,
            projection.revision,
            preview_revision,
            projection.workspace_transaction_id.clone(),
            PreviewImpact::from_projected_paths(&update.projected_paths, update.baseline_rebuilt),
            graph,
            resources,
        )?;
        let content =
            bind_canvas_identity_to_generation_content(content, &canvas_transaction.identity)?;
        timings.canvas_transaction_ms = elapsed_ms(canvas_transaction_started);

        Ok((
            ActivePreviewGeneration {
                project_root: self.owner.project_root.clone(),
                runtime_session_id: self.owner.runtime_session_id.clone(),
                workspace_revision: projection.revision,
                preview_revision: preview_revision.to_string(),
                canvas_transaction,
                content,
                workbench_content: Arc::new(RwLock::new(HashMap::new())),
                assets_root: generation_assets_root,
                inherited_assets,
            },
            project_model,
        ))
    }

    fn collect_retired<R: Runtime>(&mut self, app: &AppHandle<R>) {
        let mut retained = Vec::new();
        for generation in self.retired.drain(..) {
            if Arc::strong_count(&generation) == 1 {
                let root = generation.assets_root.clone();
                let owns_assets = generation.inherited_assets.is_none();
                drop(generation);
                if owns_assets
                    && remove_persistent_preview_artifact_root(app, &self.session_root, &root)
                        .is_err()
                {
                    // Cleanup is derived and retryable. Keep no stale authority;
                    // session teardown removes the whole bounded cache tree.
                }
            } else {
                retained.push(generation);
            }
        }
        self.retired = retained;
    }

    fn require_projection_owner(
        &self,
        projection: &WorkspaceProjectionSnapshot,
    ) -> Result<(), String> {
        if projection.project_root != self.owner.project_root
            || projection.runtime_session_id != self.owner.runtime_session_id
        {
            return Err(format!(
                "Motorul Preview refuză projection-ul altei sesiuni: primit {}/{}, activ {}/{}.",
                projection.project_root,
                projection.runtime_session_id,
                self.owner.project_root,
                self.owner.runtime_session_id
            ));
        }
        Ok(())
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u64::MAX as u128) as u64
}

fn elapsed_us(started: Instant) -> u64 {
    started.elapsed().as_micros().min(u64::MAX as u128) as u64
}

fn template_workbench_projection_key(plan: &TemplateWorkbenchPlan) -> String {
    let page_context = plan
        .selected_context
        .as_ref()
        .map(|context| format!("{}:{}", context.page_id, context.page_file))
        .unwrap_or_default();
    let route_context = plan
        .selected_route
        .as_ref()
        .map(|context| context.url.as_str())
        .unwrap_or_default();
    format!(
        "{}\u{1f}{}\u{1f}{:?}\u{1f}{}\u{1f}{}\u{1f}{}",
        plan.project_model_revision,
        plan.active_template.source_id,
        plan.render_mode,
        page_context,
        route_context,
        plan.active_component_name.as_deref().unwrap_or_default(),
    )
}

fn template_workbench_reuse_token(
    generation: &ActivePreviewGeneration,
    route: &str,
    context_key: &str,
) -> String {
    let mut digest = Sha256::new();
    for field in [
        generation.project_root.as_str(),
        generation.runtime_session_id.as_str(),
        generation.preview_revision.as_str(),
        generation
            .canvas_transaction
            .identity
            .transaction_id
            .as_str(),
        route,
        context_key,
    ] {
        digest.update((field.len() as u64).to_le_bytes());
        digest.update(field.as_bytes());
    }
    digest.update(generation.workspace_revision.to_le_bytes());
    format!("sha256:{:x}", digest.finalize())
}

fn render_template_workbench_document(
    site: &Site,
    canonical_content: &HashMap<String, String>,
    model: &ProjectModel,
    plan: &TemplateWorkbenchPlan,
) -> Result<(String, String), String> {
    if plan.render_mode == TemplateWorkbenchRenderMode::CanonicalRoute {
        let route = plan
            .selected_route
            .as_ref()
            .map(|context| context.url.as_str())
            .ok_or_else(|| {
                "Contextul canonic Workbench nu conține ruta verificată de ProjectModel."
                    .to_string()
            })?;
        let document = canonical_document_for_route(canonical_content, route).ok_or_else(|| {
            format!("Preview-ul canonic al reviziei curente nu conține ruta «{route}».")
        })?;
        return Ok((document.to_string(), route.to_string()));
    }

    let (mut context, context_route) = template_workbench_context(site, plan)?;
    if !plan.render_context.canonical_truth {
        install_controlled_workbench_fixture(&mut context);
    }
    let active_template_name = engine_template_name(
        &plan.active_template.name,
        plan.active_template.theme_name.as_deref(),
    );

    let rendered = match plan.render_mode {
        TemplateWorkbenchRenderMode::ListingItemScenario => {
            render_listing_item_scenario(site, &active_template_name, context, plan)?
        }
        TemplateWorkbenchRenderMode::ComponentScenario => {
            let component_name = active_workbench_component_name(model, plan)?;
            render_component_scenario(&site.tera, &active_template_name, &component_name)?
        }
        TemplateWorkbenchRenderMode::IncludedTemplate if consumer_render_is_required(plan) => {
            let consumer = plan.selected_context.as_ref().ok_or_else(|| {
                "Context de template nu are consumator pentru partialul selectat.".to_string()
            })?;
            let root = model
                .source_graph
                .templates
                .iter()
                .find(|template| template.node_id == consumer.root_template_source_id)
                .ok_or_else(|| {
                    format!(
                        "Template-ul rădăcină {} nu mai există în ProjectModel.",
                        consumer.root_template_file
                    )
                })?;
            let root_name = engine_template_name(&root.name, root.theme_name.as_deref());
            let direct_context = context.clone();
            let consumer_document =
                render_zola_template(site, &root_name, context, root.theme_name.is_some())?;
            match extract_template_owned_fragment(
                &consumer_document,
                &plan.active_template.file,
                model,
            ) {
                Ok(fragment) => fragment,
                Err(extraction_error) => render_zola_template(
                    site,
                    &active_template_name,
                    direct_context,
                    plan.active_template.theme_name.is_some(),
                )
                .map_err(|direct_error| {
                    format!(
                        "{extraction_error} Randarea directă de rezervă a eșuat la rândul ei: {direct_error}"
                    )
                })?,
            }
        }
        _ => render_zola_template(
            site,
            &active_template_name,
            context,
            plan.active_template.theme_name.is_some(),
        )?,
    };

    let route =
        context_route.unwrap_or_else(|| template_workbench_route(&plan.active_template.source_id));
    if classify_workbench_rendered_html(&rendered) == WorkbenchRenderedShape::FullDocument {
        validate_complete_workbench_document(&rendered)?;
        return Ok((rendered, route));
    }

    let canonical = canonical_document_for_route(canonical_content, &route)
        .or_else(|| canonical_content.get("").map(String::as_str));
    Ok((
        mount_workbench_fragment(
            canonical,
            &rendered,
            &plan.active_template.source_id,
            &plan.active_template.file,
        )?,
        route,
    ))
}

fn render_listing_item_scenario(
    site: &Site,
    template_name: &str,
    context: Context,
    plan: &TemplateWorkbenchPlan,
) -> Result<String, String> {
    let page_file = plan
        .selected_context
        .as_ref()
        .map(|consumer| normalized_content_file(&consumer.page_file))
        .ok_or_else(|| {
            "Scenariul Listing Item nu are articolul real ales pentru Preview.".to_string()
        })?;
    let harness = format!(
        "{{% set item = get_page(path=\"{}\") %}}\n{{% include \"{}\" %}}",
        escape_workbench_tera_string(&page_file),
        escape_workbench_tera_string(template_name),
    );
    let mut tera = site.tera.clone();
    let harness_name = "__pana_template_workbench_listing_item.html";
    tera.add_raw_template(harness_name, &harness)
        .map_err(|error| format!("Scenariul Listing Item nu a putut fi compilat: {error}"))?;
    tera.render(harness_name, &context)
        .map_err(|error| format!("Scenariul Listing Item a eșuat: {error}"))
}

fn escape_workbench_tera_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn engine_template_name(name: &str, theme_name: Option<&str>) -> String {
    theme_name
        .map(|theme| format!("{theme}/templates/{name}"))
        .unwrap_or_else(|| name.to_string())
}

fn render_zola_template(
    site: &Site,
    template_name: &str,
    context: Context,
    theme_template: bool,
) -> Result<String, String> {
    let _ = theme_template;
    site.tera
        .render(template_name, &context)
        .map_err(|error| format!("Context de template nu a putut randa «{template_name}»: {error}"))
}

fn consumer_render_is_required(plan: &TemplateWorkbenchPlan) -> bool {
    plan.selected_context.as_ref().is_some_and(|consumer| {
        consumer.dependency_path.iter().any(|step| {
            matches!(
                step.kind,
                crate::project_model::template_workbench::TemplateWorkbenchDependencyKind::Includes
            )
        })
    })
}

fn extract_template_owned_fragment(
    rendered_document: &str,
    active_file: &str,
    model: &ProjectModel,
) -> Result<String, String> {
    let active_file = normalize_workbench_project_file(active_file);
    let owned_ids = model
        .source_graph
        .nodes
        .iter()
        .filter(|node| {
            node.kind == crate::source_graph::model::SourceNodeKind::Html
                && normalize_workbench_project_file(&node.file) == active_file
        })
        .map(|node| node.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    if owned_ids.is_empty() {
        return Err(format!(
            "Context de template nu a găsit noduri HTML provenite din «{active_file}» în SourceGraph."
        ));
    }

    let document = parse(rendered_document.to_string());
    let mut roots = Vec::new();
    for element in document
        .select("[data-pana-source-id]")
        .map_err(|_| "Selectorul de proveniență Workbench este invalid.".to_string())?
    {
        let source_id = element
            .attributes
            .borrow()
            .get("data-pana-source-id")
            .map(str::to_string);
        if !source_id
            .as_deref()
            .is_some_and(|id| owned_ids.contains(id))
        {
            continue;
        }
        let mut ancestor = element.as_node().parent();
        let mut has_owned_ancestor = false;
        while let Some(node) = ancestor {
            if let Some(parent_element) = node.as_element() {
                if parent_element
                    .attributes
                    .borrow()
                    .get("data-pana-source-id")
                    .is_some_and(|id| owned_ids.contains(id))
                {
                    has_owned_ancestor = true;
                    break;
                }
            }
            ancestor = node.parent();
        }
        if !has_owned_ancestor {
            roots.push(element.as_node().clone());
        }
    }
    if roots.is_empty() {
        return Err(format!(
            "Template-ul «{active_file}» a fost evaluat, dar nu a produs un fragment HTML propriu în contextul consumatorului."
        ));
    }
    let mut fragment = String::new();
    for root in roots {
        fragment.push_str(
            &String::from_utf8(serialize_node(&root)).map_err(|error| {
                format!("Fragmentul Workbench nu a putut fi serializat: {error}")
            })?,
        );
    }
    Ok(fragment)
}

fn normalize_workbench_project_file(file: &str) -> String {
    file.trim().trim_start_matches('/').replace('\\', "/")
}

fn render_component_scenario(
    source_tera: &tera::Tera,
    template_name: &str,
    component_name: &str,
) -> Result<String, String> {
    let definition = source_tera
        .get_component_definition(component_name)
        .ok_or_else(|| {
            format!(
                "Componenta «{component_name}» din template-ul «{template_name}» nu există în registrul Tera 2 activ."
            )
        })?;
    // Tera 2 validates a component call context as its argument list. Page and
    // site globals belong to template rendering and must not be smuggled in as
    // undeclared component arguments.
    let mut arguments = Context::new();
    for argument in definition.args() {
        if !argument.is_required() {
            continue;
        }
        arguments.insert(
            argument.name().to_string(),
            &controlled_component_argument(argument.name(), argument.arg_type()),
        );
    }
    source_tera
        .render_component(
            component_name,
            &arguments,
            Some("<p>Conținut demonstrativ</p>"),
            true,
        )
        .map_err(|error| format!("Scenariul componentei «{component_name}» a eșuat: {error}"))
}

fn active_workbench_component_name(
    model: &ProjectModel,
    plan: &TemplateWorkbenchPlan,
) -> Result<String, String> {
    if let Some(component_name) = plan.active_component_name.as_ref() {
        return Ok(component_name.clone());
    }
    model
        .source_graph
        .templates
        .iter()
        .find(|template| template.node_id == plan.active_template.source_id)
        .and_then(|template| template.component_definitions.first())
        .map(|definition| definition.name.clone())
        .ok_or_else(|| {
            format!(
                "Template-ul «{}» nu definește nicio componentă apelabilă.",
                plan.active_template.name
            )
        })
}

fn controlled_component_argument(
    name: &str,
    argument_type: Option<tera::ComponentArgType>,
) -> serde_json::Value {
    match argument_type {
        Some(tera::ComponentArgType::Bool) => return serde_json::json!(true),
        Some(tera::ComponentArgType::Integer) => return serde_json::json!(3),
        Some(tera::ComponentArgType::Float | tera::ComponentArgType::Number) => {
            return serde_json::json!(3.5);
        }
        Some(tera::ComponentArgType::Array) => {
            return serde_json::json!(["Exemplu unu", "Exemplu doi"]);
        }
        Some(tera::ComponentArgType::Map) => {
            return serde_json::json!({"title": "Exemplu", "url": "#"});
        }
        Some(tera::ComponentArgType::Bytes) => return serde_json::json!([80, 97, 110, 97]),
        Some(tera::ComponentArgType::String) | None => {}
        Some(_) => {}
    }
    let normalized = name.to_ascii_lowercase();
    if normalized.contains("items") || normalized.contains("pages") || normalized.contains("cards")
    {
        return serde_json::json!([
            {"title": "Exemplu unu", "text": "Conținut demonstrativ", "url": "#"},
            {"title": "Exemplu doi", "text": "Conținut demonstrativ", "url": "#"}
        ]);
    }
    if normalized.contains("item") || normalized.contains("page") || normalized.contains("card") {
        return serde_json::json!({
            "title": "Exemplu",
            "text": "Conținut demonstrativ",
            "description": "Descriere demonstrativă",
            "url": "#",
            "permalink": "#"
        });
    }
    if normalized.starts_with("is_")
        || normalized.starts_with("has_")
        || normalized.contains("enabled")
    {
        return serde_json::json!(true);
    }
    if normalized.contains("count") || normalized.contains("limit") || normalized.contains("index")
    {
        return serde_json::json!(3);
    }
    if normalized.contains("url") || normalized.contains("href") || normalized.contains("link") {
        return serde_json::json!("#");
    }
    serde_json::json!("Exemplu")
}

fn install_controlled_workbench_fixture(context: &mut Context) {
    let page = serde_json::json!({
        "title": "Pagină demonstrativă",
        "description": "Context controlat Context de template",
        "content": "<p>Conținut demonstrativ</p>",
        "permalink": "#",
        "path": "/exemplu/",
        "slug": "exemplu",
        "extra": {},
        "taxonomies": {},
        "assets": []
    });
    let section = serde_json::json!({
        "title": "Secțiune demonstrativă",
        "description": "Context controlat Context de template",
        "content": "<p>Conținut demonstrativ</p>",
        "permalink": "#",
        "path": "/",
        "pages": [page.clone()],
        "subsections": [],
        "extra": {},
        "assets": []
    });
    context.insert("page", &page);
    context.insert("section", &section);
    context.insert("pana_workbench_fixture", &true);
}

fn template_workbench_context(
    site: &Site,
    plan: &TemplateWorkbenchPlan,
) -> Result<(Context, Option<String>), String> {
    let selected_file = plan
        .selected_context
        .as_ref()
        .map(|consumer| normalized_content_file(&consumer.page_file));
    let library = &site.library;

    if let Some(selected_file) = selected_file.as_deref() {
        if let Some((page_path, page)) = library
            .pages
            .iter()
            .find(|(_, page)| normalized_content_file(&page.file.relative) == selected_file)
        {
            let mut context = Context::new();
            let config = site.cache.configs.get(&page.lang).ok_or_else(|| {
                format!(
                    "Cache-ul Zola nu conține configurația limbii «{}».",
                    page.lang
                )
            })?;
            let cached_page = site
                .cache
                .pages
                .get(page_path)
                .ok_or_else(|| format!("Cache-ul Zola nu conține pagina «{selected_file}»."))?;
            context.insert_value("config", config.clone());
            context.insert("current_url", &page.permalink);
            context.insert("current_path", &page.path);
            context.insert("zola_version", EMBEDDED_ZOLA_VERSION);
            context.insert_value("page", cached_page.value.clone());
            context.insert("lang", &page.lang);
            return Ok((context, Some(page.path.clone())));
        }
        if let Some((section_path, section)) = library
            .sections
            .iter()
            .find(|(_, section)| normalized_content_file(&section.file.relative) == selected_file)
        {
            let mut context = Context::new();
            let config = site.cache.configs.get(&section.lang).ok_or_else(|| {
                format!(
                    "Cache-ul Zola nu conține configurația limbii «{}».",
                    section.lang
                )
            })?;
            let cached_section =
                site.cache.sections.get(section_path).ok_or_else(|| {
                    format!("Cache-ul Zola nu conține secțiunea «{selected_file}».")
                })?;
            context.insert_value("config", config.clone());
            context.insert("current_url", &section.permalink);
            context.insert("current_path", &section.path);
            context.insert("zola_version", EMBEDDED_ZOLA_VERSION);
            context.insert_value("section", cached_section.value.clone());
            context.insert("lang", &section.lang);
            return Ok((context, Some(section.path.clone())));
        }
        return Err(format!(
            "Contextul consumator «{selected_file}» nu există în biblioteca motorului Zola pentru această revizie."
        ));
    }

    let lang = site.config.default_language.clone();
    let mut context = Context::new();
    let config = site
        .cache
        .configs
        .get(&lang)
        .ok_or_else(|| format!("Cache-ul Zola nu conține configurația limbii «{lang}»."))?;
    context.insert_value("config", config.clone());
    context.insert("current_url", &site.config.base_url);
    context.insert("current_path", "/");
    context.insert("zola_version", EMBEDDED_ZOLA_VERSION);
    context.insert("lang", &lang);
    Ok((context, None))
}

fn normalized_content_file(path: &str) -> String {
    let path = path.trim().trim_start_matches('/').replace('\\', "/");
    path.strip_prefix("content/")
        .or_else(|| path.strip_prefix("content/"))
        .unwrap_or(&path)
        .to_string()
}

fn canonical_document_for_route<'a>(
    content: &'a HashMap<String, String>,
    route: &str,
) -> Option<&'a str> {
    let path = route
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(route)
        .trim();
    let key = if path == "/" {
        String::new()
    } else {
        path.trim_matches('/').to_string()
    };
    content.get(&key).map(String::as_str)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkbenchRenderedShape {
    FullDocument,
    Fragment,
}

fn classify_workbench_rendered_html(mut html: &str) -> WorkbenchRenderedShape {
    if let Some(without_bom) = html.strip_prefix('\u{feff}') {
        html = without_bom;
    }
    loop {
        html = html.trim_start_matches(char::is_whitespace);
        let Some(comment) = html.strip_prefix("<!--") else {
            break;
        };
        let Some(comment_end) = comment.find("-->") else {
            break;
        };
        html = &comment[comment_end + "-->".len()..];
    }

    if html
        .get(.."<!doctype".len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("<!doctype"))
        || html
            .get(.."<html".len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("<html"))
    {
        WorkbenchRenderedShape::FullDocument
    } else {
        WorkbenchRenderedShape::Fragment
    }
}

fn validate_complete_workbench_document(html: &str) -> Result<(), String> {
    let document = parse(html.to_string());
    document
        .select_first("html")
        .map_err(|_| "Documentul complet Workbench nu are rădăcină html.".to_string())?;
    document
        .select_first("head")
        .map_err(|_| "Documentul complet Workbench nu are head normalizat.".to_string())?;
    document
        .select_first("body")
        .map_err(|_| "Documentul complet Workbench nu are body normalizat.".to_string())?;
    let body_owned_head_node = document
        .select(
            "body link[rel~='stylesheet'], body link[rel~='preload'], body base, body title, body meta[charset], body meta[name], body meta[property], body meta[http-equiv]",
        )
        .map_err(|_| "Validatorul structurii documentului Workbench este invalid.".to_string())?
        .next();
    if body_owned_head_node.is_some() {
        return Err(
            "Documentul complet Workbench conține o resursă head în body și a fost refuzat."
                .to_string(),
        );
    }
    Ok(())
}

fn mount_workbench_fragment(
    canonical: Option<&str>,
    fragment: &str,
    source_id: &str,
    source_file: &str,
) -> Result<String, String> {
    if classify_workbench_rendered_html(fragment) == WorkbenchRenderedShape::FullDocument {
        return Err(
            "Context de template a refuzat montarea unui document HTML complet ca fragment."
                .to_string(),
        );
    }
    let shell = canonical.unwrap_or(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"></head><body></body></html>",
    );
    let document = parse(shell.to_string());
    let body = document
        .select_first("body")
        .map_err(|_| "Documentul gazdă Workbench nu are body.".to_string())?;
    for child in body.as_node().children().collect::<Vec<_>>() {
        child.detach();
    }
    {
        let mut attributes = body.attributes.borrow_mut();
        attributes.insert("data-pana-workbench-active-source", source_id.to_string());
        attributes.insert("data-pana-workbench-active-file", source_file.to_string());
    }

    // The fragment root does not exist as an HTML element in the user's
    // source, therefore its editable extent is carried by the same provenance
    // marker protocol used by ordinary Tera boundaries. CanvasGraph and the
    // bridge can now project a persistent append surface without inventing a
    // wrapper that would ever be written to disk.
    let fragment_document = parse(format!(
        "<!doctype html><html><body><div data-pana-workbench-mount><!-- pana-template-source-start:{source_id} -->{fragment}<!-- pana-template-source-end:{source_id} --></div></body></html>"
    ));
    let mount = fragment_document
        .select_first("[data-pana-workbench-mount]")
        .map_err(|_| "Context de template nu a putut normaliza fragmentul randat.".to_string())?;
    for child in mount.as_node().children().collect::<Vec<_>>() {
        child.detach();
        body.as_node().append(child);
    }
    String::from_utf8(serialize_node(&document))
        .map_err(|error| format!("Context de template nu a putut serializa documentul: {error}"))
}

fn template_workbench_route(source_id: &str) -> String {
    let safe = source_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("/__pana_workbench/{safe}/")
}

fn canvas_route_for_content_key(content_key: &str) -> String {
    if content_key.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", content_key.trim_start_matches('/'))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProjectionRenderImpact {
    Full,
    Reload,
    Reuse,
}

fn projection_render_impact(
    update: &PersistentProjectionUpdate,
    has_site: bool,
    has_rendered_content: bool,
) -> ProjectionRenderImpact {
    if update.baseline_rebuilt || !has_site || !has_rendered_content {
        return ProjectionRenderImpact::Full;
    }
    if update.projected_paths.is_empty() {
        return ProjectionRenderImpact::Reuse;
    }
    for project_relative in &update.projected_paths {
        let relative = project_relative.as_str();
        if relative == "config.toml"
            || relative == "zola.toml"
            || relative.starts_with("content/")
            || relative.starts_with("themes/")
            || !(relative.starts_with("templates/")
                || relative.starts_with("sass/")
                || relative.starts_with("static/"))
        {
            return ProjectionRenderImpact::Full;
        }
    }
    ProjectionRenderImpact::Reload
}

#[cfg(test)]
fn render_official_zola_memory(
    projection_root: &Path,
    artifact_root: &Path,
    base_url: &str,
    workspace_revision: u64,
) -> Result<HashMap<String, String>, String> {
    with_zola_engine("randare Preview în memorie", || {
        build_new_official_zola_site(
            projection_root,
            artifact_root,
            base_url,
            workspace_revision,
            DraftRenderPolicy::Exclude,
        )
        .map(|(_, rendered)| rendered)
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DraftRenderPolicy {
    Include,
    Exclude,
}

fn build_new_official_zola_site(
    projection_root: &Path,
    artifact_root: &Path,
    base_url: &str,
    workspace_revision: u64,
    draft_policy: DraftRenderPolicy,
) -> Result<(Site, HashMap<String, String>), String> {
    clear_site_content()?;
    let config_file = zola_config_file(projection_root)?;
    let mut site = Site::new(projection_root, config_file).map_err(|error| {
        format!("Zola {EMBEDDED_ZOLA_VERSION} nu a putut încărca proiecția reviziei {workspace_revision}: {error:#}")
    })?;
    site.enable_serve_mode(BuildMode::Memory);
    // Embedded preview is an offline editor operation. Keep this explicit even
    // though Zola serve mode currently avoids check-mode network validation.
    site.skip_external_links_check();
    if draft_policy == DraftRenderPolicy::Include {
        // Editare sigură projects the full authoring workspace. Draft visibility
        // is an editor concern and must not change the production-like Source
        // Browser generation rendered from accepted disk state.
        site.include_drafts();
    }
    site.set_base_url(base_url.to_string());
    site.set_output_path(artifact_root);
    site.load().map_err(|error| {
        format!(
            "Zola {EMBEDDED_ZOLA_VERSION} nu a putut încărca conținutul reviziei {workspace_revision}: {error:#}"
        )
    })?;
    site.build().map_err(|error| {
        format!(
            "Zola {EMBEDDED_ZOLA_VERSION} nu a putut randă revizia {workspace_revision}: {error:#}"
        )
    })?;
    let rendered = capture_site_content()?;
    Ok((site, rendered))
}

pub(crate) fn render_official_zola_disk_generation(
    zola_root: &Path,
    artifact_root: &Path,
    base_url: &str,
    disk_generation: u64,
) -> Result<HashMap<String, String>, String> {
    with_zola_engine("randare Source Browser", || {
        build_new_official_zola_site(
            zola_root,
            artifact_root,
            base_url,
            disk_generation,
            DraftRenderPolicy::Exclude,
        )
        .map(|(_, rendered)| rendered)
    })
}

fn capture_site_content() -> Result<HashMap<String, String>, String> {
    SITE_CONTENT
        .read()
        .map_err(|_| "Zola SITE_CONTENT este indisponibil după randare.".to_string())
        .map(|rendered| {
            rendered
                .iter()
                .map(|(path, body)| (path.as_str().to_string(), body.clone()))
                .collect()
        })
}

fn clear_site_content() -> Result<(), String> {
    SITE_CONTENT
        .write()
        .map_err(|_| "Zola SITE_CONTENT este indisponibil înainte de randare.".to_string())?
        .clear();
    Ok(())
}

fn render_incremental_template_generation(
    site: &mut Site,
    artifact_root: &Path,
    base_url: &str,
    workspace_revision: u64,
) -> Result<HashMap<String, String>, String> {
    site.set_base_url(base_url.to_string());
    site.set_output_path(artifact_root);
    clear_site_content()?;
    site.reload_templates().map_err(|error| {
        format!(
            "Zola {EMBEDDED_ZOLA_VERSION} nu a putut reîncărca template-urile reviziei {workspace_revision}: {error}"
        )
    })?;
    capture_site_content()
}

fn prepare_rendered_content(
    extension: Option<&str>,
    body: &str,
    preview_revision: &str,
    document_route: &str,
    resource_versions: &PreviewResourceVersions,
    motion_preview_payload: Option<&str>,
) -> Result<RenderedPreviewContent, String> {
    let content_type = match extension {
        Some("xml") => Some("text/xml; charset=utf-8"),
        Some("json") => Some("application/json; charset=utf-8"),
        Some("txt") => Some("text/plain; charset=utf-8"),
        _ => None,
    };
    match content_type {
        Some(content_type) => Ok(RenderedPreviewContent::Text {
            body: body.as_bytes().to_vec(),
            content_type: content_type.to_string(),
        }),
        None => Ok(RenderedPreviewContent::InitialHtml(
            prepare_initial_preview_html_with_motion_payload(
                body,
                preview_revision,
                document_route,
                resource_versions,
                motion_preview_payload,
            )?,
        )),
    }
}

fn prepare_generation_content(
    model: &ProjectModel,
    rendered: HashMap<String, String>,
    preview_revision: &str,
    resource_versions: &PreviewResourceVersions,
) -> Result<HashMap<String, RenderedPreviewContent>, String> {
    let mut entries = rendered.into_iter().collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    let canvas_annotator = CanvasGraph::document_annotator(model);
    let motion_payloads = motion_preview_payload_catalog(model)?;
    parallel_preview_map(&entries, |path, body| {
        let document_route = canvas_route_for_content_key(path);
        let extension = Path::new(path).extension().and_then(|value| value.to_str());
        let prepared_body = if matches!(extension, Some("xml" | "json" | "txt")) {
            body.to_string()
        } else {
            canvas_annotator.annotate(&document_route, body)?
        };
        let motion_preview_payload = if matches!(extension, Some("xml" | "json" | "txt")) {
            None
        } else {
            rendered_motion_preview_payload(&prepared_body, &motion_payloads)?
        };
        prepare_rendered_content(
            extension,
            &prepared_body,
            preview_revision,
            &document_route,
            resource_versions,
            motion_preview_payload,
        )
    })
}

fn motion_preview_payload_catalog(model: &ProjectModel) -> Result<HashMap<String, String>, String> {
    let mut catalog = HashMap::new();
    for file in &model.files {
        let Some(template_path) = template_path_from_motion_source(&file.relative_path) else {
            continue;
        };
        let config = parse_motion_source(&file.contents)
            .map_err(|error| format!("{}: {error}", file.relative_path))?;
        let Some(payload) = generate_motion_preview_payload(&config) else {
            continue;
        };
        let project_path = js_relative_path(&template_path);
        let public_path = format!(
            "/{}",
            project_path
                .strip_prefix("static/")
                .unwrap_or(&project_path)
        );
        if catalog.insert(public_path.clone(), payload).is_some() {
            return Err(format!(
                "Motion Preview a detectat o coliziune între template-uri pentru {public_path}."
            ));
        }
    }
    Ok(catalog)
}

fn rendered_motion_preview_payload<'a>(
    rendered_html: &str,
    catalog: &'a HashMap<String, String>,
) -> Result<Option<&'a str>, String> {
    let mut matches = catalog
        .iter()
        .filter(|(public_path, _)| {
            crate::zola_links::template_contains_asset_path(
                rendered_html,
                public_path.trim_start_matches('/'),
            )
        })
        .map(|(_, payload)| payload.as_str());
    let selected = matches.next();
    if selected.is_some() && matches.next().is_some() {
        return Err(
            "Motion Preview a detectat mai multe runtime-uri de pagină în același document randat."
                .to_string(),
        );
    }
    Ok(selected)
}

fn bind_canvas_identity_to_generation_content(
    content: HashMap<String, RenderedPreviewContent>,
    identity: &crate::preview::CanvasProjectionIdentity,
) -> Result<HashMap<String, RenderedPreviewContent>, String> {
    let mut entries = content.into_iter().collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    parallel_preview_map(&entries, |_path, rendered| {
        let mut rendered = rendered.clone();
        match &mut rendered {
            RenderedPreviewContent::Html(html) => {
                bind_canvas_identity_to_editor_html(html, identity)?;
            }
            RenderedPreviewContent::InitialHtml(html) => {
                bind_canvas_identity_to_initial_preview_html(html, identity)?;
            }
            RenderedPreviewContent::Text { .. } => {}
        }
        Ok(rendered)
    })
}

fn parallel_preview_map<Input, Output>(
    entries: &[(String, Input)],
    operation: impl Fn(&str, &Input) -> Result<Output, String> + Sync,
) -> Result<HashMap<String, Output>, String>
where
    Input: Sync,
    Output: Send,
{
    if entries.is_empty() {
        return Ok(HashMap::new());
    }
    let worker_count = thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .div_ceil(2)
        .clamp(1, 6)
        .min(entries.len());
    if worker_count == 1 {
        return entries
            .iter()
            .map(|(path, input)| operation(path, input).map(|output| (path.clone(), output)))
            .collect();
    }

    let next_index = AtomicUsize::new(0);
    let (sender, receiver) = mpsc::channel();
    thread::scope(|scope| {
        for _ in 0..worker_count {
            let sender = sender.clone();
            let operation = &operation;
            let next_index = &next_index;
            scope.spawn(move || loop {
                let index = next_index.fetch_add(1, Ordering::Relaxed);
                let Some((path, input)) = entries.get(index) else {
                    break;
                };
                if sender.send((index, operation(path, input))).is_err() {
                    break;
                }
            });
        }
        drop(sender);
    });

    let mut ordered = (0..entries.len())
        .map(|_| None)
        .collect::<Vec<Option<Result<Output, String>>>>();
    for (index, result) in receiver {
        ordered[index] = Some(result);
    }
    let mut output = HashMap::with_capacity(entries.len());
    for ((path, _), result) in entries.iter().zip(ordered) {
        let result = result.ok_or_else(|| {
            format!("Procesarea paralelă Preview nu a publicat rezultatul pentru ruta `{path}`.")
        })??;
        output.insert(path.clone(), result);
    }
    Ok(output)
}

fn next_preview_revision(workspace_revision: u64) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| format!("{}-{}", duration.as_secs(), duration.subsec_nanos()))
        .unwrap_or_else(|_| "0-0".to_string());
    format!("workspace-{workspace_revision}-{timestamp}")
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, HashMap, HashSet},
        env, fs,
    };

    use crate::{
        app_home::{ensure_app_home, TEST_APP_ENV_LOCK},
        kernel::write_authority::{
            test_support::install_test_project_authority, WriteAuthorityRuntime,
        },
        preview::read_http_document,
        project::{read_project_disk_manifest, AcceptedProjectDiskManifest},
        project_model::test_support::ProjectModelTestFixture,
        zola_engine::acquire_zola_engine_for_test,
    };

    use super::*;

    #[test]
    fn parallel_preview_map_preserves_every_route_and_propagates_errors() {
        let entries = (0..24)
            .map(|index| (format!("route-{index:02}"), index))
            .collect::<Vec<_>>();
        let mapped =
            parallel_preview_map(&entries, |path, value| Ok(format!("{path}:{}", value * 2)))
                .unwrap();

        assert_eq!(mapped.len(), entries.len());
        assert_eq!(
            mapped.get("route-07").map(String::as_str),
            Some("route-07:14")
        );

        let error = parallel_preview_map(&entries, |path, value| {
            if *value == 11 {
                Err(format!("failed:{path}"))
            } else {
                Ok(*value)
            }
        })
        .unwrap_err();
        assert_eq!(error, "failed:route-11");
    }

    #[test]
    fn component_scenario_calls_real_component_with_controlled_required_arguments() {
        let mut tera = tera::Tera::default();
        tera.add_raw_template(
            "components/foreign.html",
            "{% component alpha() %}<aside>Componentă străină</aside>{% endcomponent alpha %}",
        )
        .unwrap();
        tera.add_raw_template(
            "components/card.html",
            concat!(
                "{% component card(title: string, visible: bool=true) %}",
                "{% if visible %}<article class=\"card\">{{ title }}</article>{% endif %}",
                "{% endcomponent card %}",
            ),
        )
        .unwrap();

        let rendered = render_component_scenario(&tera, "components/card.html", "card").unwrap();

        assert!(rendered.contains("<article class=\"card\">Exemplu</article>"));
        assert!(!rendered.contains("Componentă străină"));
    }

    #[test]
    fn workbench_renders_page_partial_orphan_and_component_with_declared_contexts() {
        let fixture = parity_fixture("template-workbench-render-matrix");
        let project = fixture.join("project");
        let zola_root = project.to_path_buf();
        let artifacts = fixture.join("artifacts");
        create_workbench_render_project(&zola_root);
        let model = ProjectModelTestFixture::from_integration_disk_boundary(&project)
            .unwrap()
            .build_model()
            .unwrap();
        let _render_guard = acquire_zola_engine_for_test();
        let (site, canonical) = build_new_official_zola_site(
            &zola_root,
            &artifacts,
            "http://127.0.0.1:41888",
            1,
            DraftRenderPolicy::Include,
        )
        .unwrap();

        let index_plan = crate::project_model::template_workbench::resolve_template_workbench_plan(
            &model,
            &crate::project_model::template_workbench::TemplateWorkbenchPlanInput {
                template_path: "templates/index.html".to_string(),
                preferred_page_path: None,
                preferred_route: None,
                preferred_component_name: None,
            },
        )
        .unwrap();
        let (index_html, index_route) =
            render_template_workbench_document(&site, &canonical, &model, &index_plan).unwrap();
        assert_eq!(index_route, "/");
        assert!(index_plan.render_context.canonical_truth);
        assert!(index_html.contains("<main class=\"layout\">"));
        assert!(index_html.contains("<article class=\"card\">Acasă</article>"));

        let base_plan = crate::project_model::template_workbench::resolve_template_workbench_plan(
            &model,
            &crate::project_model::template_workbench::TemplateWorkbenchPlanInput {
                template_path: "templates/base.html".to_string(),
                preferred_page_path: None,
                preferred_route: None,
                preferred_component_name: None,
            },
        )
        .unwrap();
        let (base_html, base_route) =
            render_template_workbench_document(&site, &canonical, &model, &base_plan).unwrap();
        assert_eq!(
            base_plan.render_mode,
            TemplateWorkbenchRenderMode::IncludedTemplate
        );
        assert_eq!(base_route, "/");
        assert!(!base_html.contains("data-pana-workbench-active-file"));
        let base_document = parse(base_html.clone());
        assert!(base_document
            .select_first("head link[rel~='stylesheet']")
            .is_ok());
        assert!(base_document
            .select_first("head link[rel~='preload'][as='font']")
            .is_ok());
        assert_eq!(
            base_document
                .select("body link[rel~='stylesheet'], body link[rel~='preload']")
                .unwrap()
                .count(),
            0
        );
        let prepared_base =
            crate::preview::inject::prepare_design_safe_html(&base_html, "base-1").unwrap();
        let prepared_base_document = parse(prepared_base.editor);
        assert!(prepared_base_document
            .select_first("head link[rel~='preload'][as='font']")
            .is_ok());
        assert_eq!(
            prepared_base_document
                .select("body link[rel~='stylesheet'], body link[rel~='preload']")
                .unwrap()
                .count(),
            0
        );

        let partial_plan =
            crate::project_model::template_workbench::resolve_template_workbench_plan(
                &model,
                &crate::project_model::template_workbench::TemplateWorkbenchPlanInput {
                    template_path: "templates/partials/wrapper.html".to_string(),
                    preferred_page_path: None,
                    preferred_route: None,
                    preferred_component_name: None,
                },
            )
            .unwrap();
        let (partial_html, _) =
            render_template_workbench_document(&site, &canonical, &model, &partial_plan).unwrap();
        assert!(partial_plan.render_context.canonical_truth);
        assert!(partial_html.contains("<section class=\"wrapper\">"));
        assert!(partial_html.contains("<article class=\"card\">Acasă</article>"));
        assert!(!partial_html.contains("<main class=\"layout\">"));
        let prepared_partial =
            crate::preview::inject::prepare_design_safe_html(&partial_html, "workbench-partial")
                .unwrap();
        assert!(prepared_partial.editor.contains("/site.css"));
        assert!(!prepared_partial.editor.contains("/site.js"));
        assert!(prepared_partial.interactive.contains("/site.js"));

        let orphan_plan =
            crate::project_model::template_workbench::resolve_template_workbench_plan(
                &model,
                &crate::project_model::template_workbench::TemplateWorkbenchPlanInput {
                    template_path: "templates/orphan.html".to_string(),
                    preferred_page_path: None,
                    preferred_route: None,
                    preferred_component_name: None,
                },
            )
            .unwrap();
        let (orphan_html, _) =
            render_template_workbench_document(&site, &canonical, &model, &orphan_plan).unwrap();
        assert!(!orphan_plan.render_context.canonical_truth);
        assert!(orphan_html.contains("<aside>Pagină demonstrativă</aside>"));

        let component_plan =
            crate::project_model::template_workbench::resolve_template_workbench_plan(
                &model,
                &crate::project_model::template_workbench::TemplateWorkbenchPlanInput {
                    template_path: "templates/components/card.html".to_string(),
                    preferred_page_path: None,
                    preferred_route: None,
                    preferred_component_name: None,
                },
            )
            .unwrap();
        let (component_html, _) =
            render_template_workbench_document(&site, &canonical, &model, &component_plan).unwrap();
        assert!(!component_plan.render_context.canonical_truth);
        assert!(component_html.contains("<strong class=\"component-card\">Exemplu</strong>"));

        drop(site);
        drop(_render_guard);
        fs::remove_dir_all(fixture).unwrap();
    }

    #[test]
    fn workbench_serves_the_exact_canonical_taxonomy_documents() {
        let fixture = parity_fixture("template-workbench-taxonomy-routes");
        let project = fixture.join("project");
        let artifacts = fixture.join("artifacts");
        create_taxonomy_workbench_project(&project);
        let model = ProjectModelTestFixture::from_integration_disk_boundary(&project)
            .unwrap()
            .build_model()
            .unwrap();
        let _render_guard = acquire_zola_engine_for_test();
        let (site, canonical) = build_new_official_zola_site(
            &project,
            &artifacts,
            "http://127.0.0.1:41889",
            1,
            DraftRenderPolicy::Include,
        )
        .unwrap();
        let list_plan = crate::project_model::template_workbench::resolve_template_workbench_plan(
            &model,
            &crate::project_model::template_workbench::TemplateWorkbenchPlanInput {
                template_path: "templates/tags/list.html".to_string(),
                preferred_page_path: None,
                preferred_route: Some("/tags/".to_string()),
                preferred_component_name: None,
            },
        )
        .unwrap();
        let (list_html, list_route) =
            render_template_workbench_document(&site, &canonical, &model, &list_plan).unwrap();
        assert_eq!(list_route, "/tags/");
        assert!(list_html.contains("class=\"taxonomy-list\""));
        assert_eq!(
            list_html,
            canonical_document_for_route(&canonical, "/tags/")
                .unwrap()
                .to_string()
        );

        let term_plan = crate::project_model::template_workbench::resolve_template_workbench_plan(
            &model,
            &crate::project_model::template_workbench::TemplateWorkbenchPlanInput {
                template_path: "templates/tags/single.html".to_string(),
                preferred_page_path: None,
                preferred_route: Some("/tags/rust/".to_string()),
                preferred_component_name: None,
            },
        )
        .unwrap();
        let (term_html, term_route) =
            render_template_workbench_document(&site, &canonical, &model, &term_plan).unwrap();
        assert_eq!(term_route, "/tags/rust/");
        assert!(term_html.contains("class=\"taxonomy-term\""));
        assert_eq!(
            term_html,
            canonical_document_for_route(&canonical, "/tags/rust/")
                .unwrap()
                .to_string()
        );

        drop(site);
        drop(_render_guard);
        fs::remove_dir_all(fixture).unwrap();
    }

    #[test]
    fn workbench_fragment_extraction_keeps_only_top_level_nodes_owned_by_active_source() {
        let fixture = parity_fixture("template-workbench-owned-fragment");
        let project = fixture.join("project");
        let zola_root = project.to_path_buf();
        create_workbench_render_project(&zola_root);
        let model = ProjectModelTestFixture::from_integration_disk_boundary(&project)
            .unwrap()
            .build_model()
            .unwrap();
        let article = model
            .source_graph
            .nodes
            .iter()
            .find(|node| {
                node.file.ends_with("templates/partials/card.html")
                    && node.kind == crate::source_graph::model::SourceNodeKind::Html
                    && node.label.starts_with("<article")
            })
            .unwrap();
        let rendered = format!(
            "<!doctype html><html><body><header data-pana-source-id=\"foreign\">Shell</header><article class=\"card\" data-pana-source-id=\"{}\"><span data-pana-source-id=\"{}\">Owned child</span></article></body></html>",
            article.id, article.id
        );

        let fragment =
            extract_template_owned_fragment(&rendered, "templates/partials/card.html", &model)
                .unwrap();

        assert!(fragment.contains("class=\"card\""));
        assert!(fragment.contains("Owned child"));
        assert!(!fragment.contains("Shell"));
        assert_eq!(fragment.matches("<article").count(), 1);
        fs::remove_dir_all(fixture).unwrap();
    }

    #[test]
    fn workbench_fragment_mount_keeps_an_outer_source_boundary_when_empty() {
        let html = mount_workbench_fragment(
            None,
            "",
            "sg_listing_item_root",
            "templates/listing-items/card.html",
        )
        .unwrap();

        let start = html
            .find("<!-- pana-template-source-start:sg_listing_item_root -->")
            .expect("fragment start marker");
        let end = html
            .find("<!-- pana-template-source-end:sg_listing_item_root -->")
            .expect("fragment end marker");
        assert!(start < end);
        assert!(
            html.contains("data-pana-workbench-active-file=\"templates/listing-items/card.html\"")
        );
        assert!(!html.contains("data-pana-workbench-mount"));
    }

    #[test]
    fn workbench_render_shape_is_content_driven_and_ignores_safe_prefixes() {
        assert_eq!(
            classify_workbench_rendered_html(
                "\u{feff}  <!-- generated -->\n<!DOCTYPE html><html><head></head><body></body></html>",
            ),
            WorkbenchRenderedShape::FullDocument
        );
        assert_eq!(
            classify_workbench_rendered_html(
                "\n<!-- source --> <HTML lang=\"ro\"><head></head><body></body></HTML>",
            ),
            WorkbenchRenderedShape::FullDocument
        );
        assert_eq!(
            classify_workbench_rendered_html("<section>Fragment</section>"),
            WorkbenchRenderedShape::Fragment
        );
    }

    #[test]
    fn workbench_fragment_mount_refuses_a_complete_html_document() {
        let error = mount_workbench_fragment(
            None,
            "<!doctype html><html><head><link rel=\"preload\" href=\"/font.woff2\" as=\"font\"></head><body></body></html>",
            "sg_complete_document",
            "templates/document.html",
        )
        .unwrap_err();

        assert!(error.contains("document HTML complet"), "{error}");
    }

    fn stage_and_confirm<R: Runtime>(
        engine: &mut PersistentZolaPreviewEngine,
        app: &AppHandle<R>,
        candidate: PersistentPreviewCandidate,
    ) -> Arc<ActivePreviewGeneration> {
        let identity = candidate.generation.canvas_transaction.identity.clone();
        engine.stage_candidate(app, candidate).unwrap();
        confirm_staged(engine, app, identity)
    }

    fn confirm_staged<R: Runtime>(
        engine: &mut PersistentZolaPreviewEngine,
        app: &AppHandle<R>,
        identity: crate::preview::CanvasProjectionIdentity,
    ) -> Arc<ActivePreviewGeneration> {
        let schema_version = crate::preview::canvas::CANVAS_PROJECTION_SCHEMA_VERSION;
        let mut generation = None;
        for (phase, timings) in [
            (
                crate::preview::canvas::CanvasProjectionPhase::ResourcesReady,
                BTreeMap::from([("resourcesReady".to_string(), 1)]),
            ),
            (
                crate::preview::canvas::CanvasProjectionPhase::Committed,
                BTreeMap::from([
                    ("resourcesReady".to_string(), 1),
                    ("committed".to_string(), 2),
                ]),
            ),
            (
                crate::preview::canvas::CanvasProjectionPhase::StyledReady,
                BTreeMap::from([
                    ("resourcesReady".to_string(), 1),
                    ("committed".to_string(), 2),
                    ("styledReady".to_string(), 3),
                ]),
            ),
        ] {
            generation = Some(
                engine
                    .acknowledge_candidate_phase(
                        app,
                        &PreviewPhaseReceipt {
                            schema_version,
                            identity: identity.clone(),
                            phase,
                            phase_timings_ms: timings,
                            diagnostic: None,
                        },
                    )
                    .unwrap(),
            );
        }
        generation.unwrap()
    }

    #[test]
    fn zola_memory_content_types_match_official_serve_defaults() {
        let resources = PreviewResourceVersions::default();
        assert!(matches!(
            prepare_rendered_content(
                Some("xml"),
                "<xml/>",
                "r1",
                "/feed.xml",
                &resources,
                None,
            )
                .unwrap(),
            RenderedPreviewContent::Text { content_type, .. } if content_type.starts_with("text/xml")
        ));
        assert!(matches!(
            prepare_rendered_content(
                None,
                "<!doctype html><html><body></body></html>",
                "r1",
                "/",
                &resources,
                None,
            )
            .unwrap(),
            RenderedPreviewContent::InitialHtml(_)
        ));
    }

    #[test]
    fn workspace_preview_includes_drafts_while_disk_rendering_excludes_them() {
        let fixture = parity_fixture("draft-render-policy");
        let project = fixture.join("project");
        let workspace_output = fixture.join("workspace-output");
        let disk_output = fixture.join("disk-output");
        create_parity_project(&project);
        fs::create_dir_all(&workspace_output).unwrap();
        fs::create_dir_all(&disk_output).unwrap();
        fs::write(
            project.join("content/despre.md"),
            r#"+++
title = "Despre noi"
template = "despre.html"
draft = true
+++
Conținut draft vizibil în editor.
"#,
        )
        .unwrap();
        fs::write(
            project.join("templates/despre.html"),
            r#"<!doctype html><html lang="ro"><body><main>{{ page.content | safe }}</main></body></html>"#,
        )
        .unwrap();

        let _render = acquire_zola_engine_for_test();
        let (_, workspace_rendered) = build_new_official_zola_site(
            &project,
            &workspace_output,
            "https://preview.pana.invalid",
            1,
            DraftRenderPolicy::Include,
        )
        .unwrap();
        let (_, disk_rendered) = build_new_official_zola_site(
            &project,
            &disk_output,
            "https://preview.pana.invalid",
            1,
            DraftRenderPolicy::Exclude,
        )
        .unwrap();
        drop(_render);

        assert!(
            workspace_rendered.contains_key("despre"),
            "generația workspace nu conține ruta draft: {:?}",
            workspace_rendered.keys().collect::<Vec<_>>()
        );
        assert!(
            !disk_rendered.contains_key("despre") && !disk_rendered.contains_key("despre/"),
            "generația de pe disc a publicat ruta draft"
        );

        fs::remove_dir_all(fixture).unwrap();
    }

    #[test]
    fn preview_revisions_are_workspace_scoped_and_safe_for_cache_paths() {
        let revision = next_preview_revision(42);
        assert!(revision.starts_with("workspace-42-"));
        assert!(revision
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-'));
    }

    #[test]
    fn projection_impact_uses_the_exact_delta_not_the_whole_dirty_workspace() {
        let update = |paths: &[&str], baseline_rebuilt: bool| PersistentProjectionUpdate {
            projection_root: PathBuf::from("/projection"),
            manifest: PersistentProjectionManifest::default(),
            projected_paths: paths.iter().map(|path| (*path).to_string()).collect(),
            baseline_rebuilt,
            publication_stats: Default::default(),
        };
        assert_eq!(
            projection_render_impact(&update(&["templates/index.html"], false), true, true),
            ProjectionRenderImpact::Reload
        );
        assert_eq!(
            projection_render_impact(&update(&["sass/pages/index.scss"], false), true, true),
            ProjectionRenderImpact::Reload
        );
        assert_eq!(
            projection_render_impact(
                &update(&["templates/layout.html", "sass/pages/index.scss"], false),
                true,
                true,
            ),
            ProjectionRenderImpact::Reload
        );
        assert_eq!(
            projection_render_impact(
                &update(&["templates/layout.html", "static/site.js"], false),
                true,
                true,
            ),
            ProjectionRenderImpact::Reload
        );
        assert_eq!(
            projection_render_impact(&update(&["content/about.md"], false), true, true),
            ProjectionRenderImpact::Full
        );
        assert_eq!(
            projection_render_impact(&update(&[], true), true, true),
            ProjectionRenderImpact::Full
        );
        assert_eq!(
            projection_render_impact(&update(&[], false), true, true),
            ProjectionRenderImpact::Reuse
        );
    }

    #[test]
    fn embedded_memory_renderer_matches_fresh_embedded_disk_generation() {
        let fixture = parity_fixture("official-render-parity");
        let project = fixture.join("project");
        let embedded_output = fixture.join("embedded-output");
        let disk_output = fixture.join("disk-output");
        create_parity_project(&project);
        fs::create_dir_all(&embedded_output).unwrap();

        let base_url = "https://preview.pana.invalid";
        let embedded = render_official_zola_memory(&project, &embedded_output, base_url, 7)
            .expect("embedded Zola build");

        run_fresh_embedded_disk_build(&project, &disk_output, base_url);

        assert!(!embedded.is_empty());
        for (route, body) in embedded {
            let disk_path = disk_path_for_memory_route(&disk_output, &route);
            assert_eq!(
                body.as_bytes(),
                fs::read(&disk_path)
                    .unwrap_or_else(|error| panic!("{}: {error}", disk_path.display())),
                "rendered route differs: {route}"
            );
        }
        for relative in ["site.css", "asset.txt"] {
            assert_eq!(
                fs::read(embedded_output.join(relative)).unwrap(),
                fs::read(disk_output.join(relative)).unwrap(),
                "derived/static asset differs: {relative}"
            );
        }

        fs::remove_dir_all(fixture).unwrap();
    }

    #[test]
    fn upgrade_baseline_fixture_keeps_preview_and_disk_generation_in_parity() {
        let fixture = parity_fixture("upgrade-baseline-parity");
        let project = fixture.join("project");
        let preview_output = fixture.join("preview-output");
        let disk_output = fixture.join("disk-output");
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/projects/zola-upgrade-baseline");
        copy_test_tree(&source, &project);
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("icons/32x32.png"),
            project.join("static/pixel.png"),
        )
        .unwrap();
        fs::create_dir_all(&preview_output).unwrap();

        let base_url = "https://preview.pana.invalid";
        let rendered = render_official_zola_memory(&project, &preview_output, base_url, 1)
            .expect("fixture baseline randat în Preview");
        run_fresh_embedded_disk_build(&project, &disk_output, base_url);

        for expected_route in ["", "en", "articole", "articole/page/2"] {
            assert!(
                rendered.contains_key(expected_route),
                "ruta Preview lipsește: {expected_route}; rute: {:?}",
                rendered.keys().collect::<Vec<_>>()
            );
        }
        assert_rendered_matches_disk(&rendered, &disk_output);
        for relative in [
            "site.css",
            "giallo.css",
            "marker.txt",
            "search_index.ro.js",
            "search_index.en.js",
            "articole/primul/diagrama.svg",
        ] {
            assert_eq!(
                fs::read(preview_output.join(relative)).unwrap(),
                fs::read(disk_output.join(relative)).unwrap(),
                "resursa derivată diferă: {relative}"
            );
        }

        fs::remove_dir_all(fixture).unwrap();
    }

    #[test]
    fn every_bundled_starter_renders_with_the_embedded_preview() {
        let starters_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/project-starters");
        let mut starter_ids = fs::read_dir(&starters_root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        starter_ids.sort();
        assert_eq!(
            starter_ids,
            ["cadru", "minimal", "nord", "pana-studio", "radacini"]
        );

        for starter_id in starter_ids {
            let fixture = parity_fixture(&format!("starter-{starter_id}"));
            let project = fixture.join("project");
            let preview_output = fixture.join("preview-output");
            copy_test_tree(&starters_root.join(&starter_id).join("project"), &project);
            fs::create_dir_all(&preview_output).unwrap();

            let rendered = render_official_zola_memory(
                &project,
                &preview_output,
                "https://preview.pana.invalid",
                1,
            )
            .unwrap_or_else(|error| panic!("starter {starter_id}: Preview eșuat: {error}"));
            let home = rendered
                .get("")
                .unwrap_or_else(|| panic!("starter {starter_id}: ruta homepage lipsește"));
            assert!(home.contains("<html"), "starter {starter_id}");

            fs::remove_dir_all(fixture).unwrap();
        }
    }

    #[test]
    fn retained_site_template_and_sass_updates_keep_fresh_disk_parity() {
        let fixture = parity_fixture("persistent-site-parity");
        let project = fixture.join("project");
        let first_output = fixture.join("first-output");
        let template_output = fixture.join("template-output");
        let sass_output = fixture.join("sass-output");
        let template_fresh = fixture.join("template-fresh");
        let sass_fresh = fixture.join("sass-fresh");
        create_parity_project(&project);
        fs::create_dir_all(&first_output).unwrap();
        fs::create_dir_all(&template_output).unwrap();
        fs::create_dir_all(&sass_output).unwrap();
        let base_url = "https://preview.pana.invalid";

        let _render = acquire_zola_engine_for_test();
        let (mut site, _) = build_new_official_zola_site(
            &project,
            &first_output,
            base_url,
            1,
            DraftRenderPolicy::Include,
        )
        .unwrap();
        fs::write(
            project.join("templates/index.html"),
            r#"<!doctype html>
<html lang="ro"><head><meta charset="utf-8"><title>{{ config.title }} · {{ section.title }}</title><link rel="stylesheet" href="{{ get_url(path='site.css') }}"></head><body><main data-revision="template-2">{{ section.content | safe }}</main><a href="{{ get_url(path='asset.txt') }}">asset</a></body></html>
"#,
        )
        .unwrap();
        let template_rendered =
            render_incremental_template_generation(&mut site, &template_output, base_url, 2)
                .unwrap();

        fs::write(
            project.join("sass/site.scss"),
            "$accent: #a32952; body { color: $accent; main { display: flex; } }\n",
        )
        .unwrap();
        let sass_rendered =
            render_incremental_template_generation(&mut site, &sass_output, base_url, 3).unwrap();
        drop(_render);

        run_fresh_embedded_disk_build(&project, &template_fresh, base_url);
        assert_rendered_matches_disk(&template_rendered, &template_fresh);
        run_fresh_embedded_disk_build(&project, &sass_fresh, base_url);
        assert_rendered_matches_disk(&sass_rendered, &sass_fresh);
        assert_eq!(
            fs::read(sass_output.join("site.css")).unwrap(),
            fs::read(sass_fresh.join("site.css")).unwrap()
        );
        assert_eq!(
            fs::read(sass_output.join("asset.txt")).unwrap(),
            fs::read(sass_fresh.join("asset.txt")).unwrap()
        );

        fs::remove_dir_all(fixture).unwrap();
    }

    #[test]
    fn retained_site_mixed_template_and_asset_update_keeps_fresh_disk_parity() {
        let fixture = parity_fixture("persistent-site-mixed-parity");
        let project = fixture.join("project");
        let first_output = fixture.join("first-output");
        let mixed_output = fixture.join("mixed-output");
        let mixed_fresh = fixture.join("mixed-fresh");
        create_parity_project(&project);
        fs::create_dir_all(&first_output).unwrap();
        fs::create_dir_all(&mixed_output).unwrap();
        let base_url = "https://preview.pana.invalid";

        let _render = acquire_zola_engine_for_test();
        let (mut site, _) = build_new_official_zola_site(
            &project,
            &first_output,
            base_url,
            1,
            DraftRenderPolicy::Include,
        )
        .unwrap();
        fs::write(
            project.join("templates/index.html"),
            r#"<!doctype html>
<html lang="ro"><head><meta charset="utf-8"><title>{{ config.title }} · {{ section.title }}</title><link rel="stylesheet" href="{{ get_url(path='site.css') }}"></head><body><main data-revision="mixed-2">{{ section.content | safe }}</main><a href="{{ get_url(path='asset.txt') }}">asset</a></body></html>
"#,
        )
        .unwrap();
        fs::write(
            project.join("sass/site.scss"),
            "$accent: #a32952; body { color: $accent; main { display: flex; } }\n",
        )
        .unwrap();
        let mixed_rendered =
            render_incremental_template_generation(&mut site, &mixed_output, base_url, 2).unwrap();
        drop(_render);

        run_fresh_embedded_disk_build(&project, &mixed_fresh, base_url);
        assert_rendered_matches_disk(&mixed_rendered, &mixed_fresh);
        assert_eq!(
            fs::read(mixed_output.join("site.css")).unwrap(),
            fs::read(mixed_fresh.join("site.css")).unwrap()
        );
        assert_eq!(
            fs::read(mixed_output.join("asset.txt")).unwrap(),
            fs::read(mixed_fresh.join("asset.txt")).unwrap()
        );

        fs::remove_dir_all(fixture).unwrap();
    }

    #[test]
    #[ignore = "requires a real loopback socket for the persistent HTTP server"]
    fn runtime_engine_publishes_exact_revisions_and_retains_last_valid_on_error() {
        let _environment = TEST_APP_ENV_LOCK.lock().unwrap();
        let fixture = parity_fixture("runtime-transaction");
        let _env = TestEnvGuard::from_root(&fixture.join("app-home"));
        let project = fixture.join("project");
        let zola_root = project.to_path_buf();
        create_parity_project(&zola_root);

        let app = tauri::test::mock_builder()
            .manage(WriteAuthorityRuntime::default())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let app_handle = app.handle().clone();
        ensure_app_home(&app_handle).unwrap();
        let session_id = "runtime-preview-test/session";
        install_test_project_authority(&app_handle, session_id, &project, &fixture.join("session"))
            .unwrap();
        let project_root = project
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let accepted_disk = AcceptedProjectDiskManifest::new(
            session_id,
            &project_root,
            read_project_disk_manifest(&project).unwrap(),
        )
        .unwrap();
        let mut source_texts = HashMap::from([
            (
                "config.toml".to_string(),
                fs::read_to_string(zola_root.join("config.toml")).unwrap(),
            ),
            (
                "content/_index.md".to_string(),
                fs::read_to_string(zola_root.join("content/_index.md")).unwrap(),
            ),
            (
                "templates/index.html".to_string(),
                fs::read_to_string(zola_root.join("templates/index.html")).unwrap(),
            ),
            (
                "sass/site.scss".to_string(),
                fs::read_to_string(zola_root.join("sass/site.scss")).unwrap(),
            ),
            (
                "static/asset.txt".to_string(),
                fs::read_to_string(zola_root.join("static/asset.txt")).unwrap(),
            ),
        ]);
        let projection =
            |revision: u64,
             source_texts: HashMap<String, String>,
             changed_paths: HashSet<String>| WorkspaceProjectionSnapshot {
                project_root: project_root.clone(),
                runtime_session_id: session_id.to_string(),
                revision,
                workspace_transaction_id: Some(format!("runtime-preview-{revision}")),
                source_texts: source_texts.into(),
                resource_bytes: HashMap::new().into(),
                deleted_sources: HashSet::new(),
                changed_paths,
                accepted_disk: accepted_disk.clone().into(),
            };
        let owner = PersistentPreviewOwner::new(&project_root, session_id);
        let mut engine =
            PersistentZolaPreviewEngine::start(&app_handle, &zola_root, owner).unwrap();

        let first = engine
            .render_candidate_with_project_model_and_pending_authority(
                &app_handle,
                &projection(1, source_texts.clone(), HashSet::new()),
                None,
                None,
                None,
            )
            .unwrap();
        assert_eq!(first.projection_publication.logical_publications, 1);
        assert_eq!(first.projection_publication.durability_operations, 2);
        let first_materialized_entries = first.projection_publication.materialized_entries;
        let projected_template =
            fs::read_to_string(engine.session_root.join("source/templates/index.html")).unwrap();
        assert!(!source_texts["templates/index.html"].contains("data-pana-source-id"));
        assert!(projected_template.contains("data-pana-source-id"));
        let first_revision = first.generation.preview_revision.clone();
        stage_and_confirm(&mut engine, &app_handle, first);
        let first_assets_root = engine
            .active_generation()
            .unwrap()
            .unwrap()
            .assets_root
            .clone();
        let url = engine.url().unwrap();
        let first_document = read_http_document(&format!("{url}/")).unwrap();
        assert!(first_document.contains(&first_revision));
        assert!(!first_document.contains("data-draft=\"two\""));

        let template_path = "templates/index.html".to_string();
        source_texts.insert(
            template_path.clone(),
            source_texts[&template_path].replace("<main>", "<main data-draft=\"two\">"),
        );
        let second_projection = projection(
            2,
            source_texts.clone(),
            HashSet::from([template_path.clone()]),
        );
        let second = engine
            .render_candidate_with_project_model_and_pending_authority(
                &app_handle,
                &second_projection,
                None,
                None,
                None,
            )
            .unwrap();
        assert_ne!(second.generation.assets_root, first_assets_root);
        assert!(second.generation.inherited_assets.is_none());
        assert!(second.projection_publication.reused_entries > 0);
        assert!(
            second.projection_publication.reflinked_files
                + second.projection_publication.copied_fallback_files
                > 0
        );
        assert!(second.projection_publication.materialized_entries < first_materialized_entries);
        eprintln!(
            "[Pană Studio][perf] preview_generation first_materialized_entries={} second_materialized_entries={} second_reused_entries={} second_reused_bytes={} second_reflinked_files={} second_copy_fallback_files={} inherited_artifacts={}",
            first_materialized_entries,
            second.projection_publication.materialized_entries,
            second.projection_publication.reused_entries,
            second.projection_publication.reused_bytes,
            second.projection_publication.reflinked_files,
            second.projection_publication.copied_fallback_files,
            second.generation.inherited_assets.is_some(),
        );
        let second_identity = second.generation.canvas_transaction.identity.clone();
        // Candidate construction is not publication.
        assert!(!read_http_document(&format!("{url}/"))
            .unwrap()
            .contains("data-draft=\"two\""));
        engine.stage_candidate(&app_handle, second).unwrap();

        let second_model = build_project_model_from_workspace_projection(
            Path::new(&second_projection.project_root),
            &second_projection,
        )
        .unwrap();
        let workbench_plan =
            crate::project_model::template_workbench::resolve_template_workbench_plan(
                &second_model,
                &crate::project_model::template_workbench::TemplateWorkbenchPlanInput {
                    template_path: template_path.clone(),
                    preferred_page_path: None,
                    preferred_route: None,
                    preferred_component_name: None,
                },
            )
            .unwrap();
        let workbench = engine
            .publish_template_workbench_view(&second_projection, &second_model, &workbench_plan)
            .unwrap();
        assert!(!workbench.cache_reused);
        assert!(workbench.timings.total_us > 0);
        assert_eq!(workbench.workspace_revision, 2);
        assert_eq!(workbench.preview_revision, second_identity.preview_revision);
        assert_eq!(workbench.canvas_plan.identity, second_identity);
        assert_eq!(
            workbench.canvas_plan.phase,
            crate::preview::canvas::CanvasProjectionPhase::Prepared
        );
        assert!(workbench.route.starts_with("/__pana_workbench/"));
        assert!(read_http_document(&workbench.preview_url)
            .unwrap()
            .contains("data-draft=\"two\""));
        let cached_workbench = engine
            .publish_template_workbench_view(&second_projection, &second_model, &workbench_plan)
            .unwrap();
        assert!(cached_workbench.cache_reused);
        assert_eq!(cached_workbench.route, workbench.route);
        assert_eq!(cached_workbench.preview_url, workbench.preview_url);
        assert_eq!(cached_workbench.reuse_token, workbench.reuse_token);
        assert_eq!(cached_workbench.timings.render_us, 0);
        assert_eq!(cached_workbench.timings.graph_us, 0);
        assert_eq!(cached_workbench.timings.prepare_us, 0);
        let selected_page_path = workbench_plan
            .selected_context
            .as_ref()
            .map(|context| context.page_file.as_str());
        let selected_route = workbench_plan
            .selected_route
            .as_ref()
            .map(|context| context.url.as_str());
        let selected_component_name = workbench_plan.active_component_name.as_deref();
        let confirmed = engine
            .confirm_template_workbench_reuse(TemplateWorkbenchReuseQuery {
                workspace_revision: 2,
                template_path: &template_path,
                preferred_page_path: selected_page_path,
                preferred_route: selected_route,
                preferred_component_name: selected_component_name,
                reuse_token: &workbench.reuse_token,
                expected_preview_revision: &second_identity.preview_revision,
                expected_canvas_transaction_id: &second_identity.transaction_id,
            })
            .unwrap()
            .expect("exact Workbench generation must be reusable");
        assert_eq!(confirmed.route, workbench.route);
        assert_eq!(confirmed.preview_url, workbench.preview_url);
        assert_eq!(confirmed.reuse_token, workbench.reuse_token);
        assert!(engine
            .confirm_template_workbench_reuse(TemplateWorkbenchReuseQuery {
                workspace_revision: 2,
                template_path: &template_path,
                preferred_page_path: Some("content/other.md"),
                preferred_route: selected_route,
                preferred_component_name: selected_component_name,
                reuse_token: &workbench.reuse_token,
                expected_preview_revision: &second_identity.preview_revision,
                expected_canvas_transaction_id: &second_identity.transaction_id,
            })
            .unwrap()
            .is_none());
        assert!(engine
            .confirm_template_workbench_reuse(TemplateWorkbenchReuseQuery {
                workspace_revision: 2,
                template_path: &template_path,
                preferred_page_path: selected_page_path,
                preferred_route: selected_route,
                preferred_component_name: selected_component_name,
                reuse_token: &workbench.reuse_token,
                expected_preview_revision: &second_identity.preview_revision,
                expected_canvas_transaction_id: "canvas-stale",
            })
            .unwrap()
            .is_none());
        assert!(engine
            .confirm_template_workbench_reuse(TemplateWorkbenchReuseQuery {
                workspace_revision: 3,
                template_path: &template_path,
                preferred_page_path: selected_page_path,
                preferred_route: selected_route,
                preferred_component_name: selected_component_name,
                reuse_token: &workbench.reuse_token,
                expected_preview_revision: &second_identity.preview_revision,
                expected_canvas_transaction_id: &second_identity.transaction_id,
            })
            .unwrap()
            .is_none());
        assert!(engine
            .confirm_template_workbench_reuse(TemplateWorkbenchReuseQuery {
                workspace_revision: 2,
                template_path: &template_path,
                preferred_page_path: selected_page_path,
                preferred_route: selected_route,
                preferred_component_name: selected_component_name,
                reuse_token: "sha256:foreign-session",
                expected_preview_revision: &second_identity.preview_revision,
                expected_canvas_transaction_id: &second_identity.transaction_id,
            })
            .unwrap()
            .is_none());
        let second_generation = confirm_staged(&mut engine, &app_handle, second_identity.clone());
        assert_eq!(
            second_generation.canvas_transaction.identity,
            second_identity
        );
        assert!(read_http_document(&format!("{url}/"))
            .unwrap()
            .contains("data-draft=\"two\""));

        let sass_path = "sass/site.scss".to_string();
        source_texts.insert(
            template_path.clone(),
            source_texts[&template_path].replace("data-draft=\"two\"", "data-draft=\"three\""),
        );
        source_texts.insert(
            sass_path.clone(),
            "$accent: #a32952; body { color: $accent; main { display: flex; } }\n".to_string(),
        );
        let third = engine
            .render_candidate_with_project_model_and_pending_authority(
                &app_handle,
                &projection(
                    3,
                    source_texts.clone(),
                    HashSet::from([template_path.clone(), sass_path.clone()]),
                ),
                None,
                None,
                None,
            )
            .unwrap();
        assert!(third.generation.inherited_assets.is_none());
        assert_ne!(third.generation.assets_root, first_assets_root);
        assert_eq!(
            third.projected_paths,
            vec![sass_path.clone(), template_path.clone()]
        );
        let previous_css_hash = second_generation
            .canvas_transaction
            .resources
            .entries
            .iter()
            .find(|entry| entry.url == "/site.css")
            .map(|entry| entry.content_hash.clone())
            .expect("generația template-only trebuie să păstreze manifestul CSS");
        let next_css_hash = third
            .generation
            .canvas_transaction
            .resources
            .entries
            .iter()
            .find(|entry| entry.url == "/site.css")
            .map(|entry| entry.content_hash.clone())
            .expect("generația mixtă trebuie să publice manifestul CSS");
        assert_ne!(next_css_hash, previous_css_hash);
        assert!(read_http_document(&format!("{url}/site.css"))
            .unwrap()
            .contains("#147d6f"));
        stage_and_confirm(&mut engine, &app_handle, third);
        assert!(read_http_document(&format!("{url}/site.css"))
            .unwrap()
            .contains("#a32952"));
        assert!(read_http_document(&format!("{url}/"))
            .unwrap()
            .contains("data-draft=\"three\""));

        source_texts.insert(template_path.clone(), "{% if %}".to_string());
        assert!(engine
            .render_candidate_with_project_model_and_pending_authority(
                &app_handle,
                &projection(4, source_texts, HashSet::from([template_path])),
                None,
                None,
                None,
            )
            .is_err());
        assert!(read_http_document(&format!("{url}/"))
            .unwrap()
            .contains("data-draft=\"three\""));

        engine.stop(&app_handle).unwrap();
        drop(app);
        fs::remove_dir_all(fixture).unwrap();
    }

    fn parity_fixture(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "pana-preview-{label}-{}-{}",
            std::process::id(),
            next_preview_revision(0)
        ))
    }

    fn copy_test_tree(source: &Path, destination: &Path) {
        fs::create_dir_all(destination).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let source_path = entry.path();
            let destination_path = destination.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_test_tree(&source_path, &destination_path);
            } else {
                fs::copy(source_path, destination_path).unwrap();
            }
        }
    }

    fn create_workbench_render_project(root: &Path) {
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates/partials")).unwrap();
        fs::create_dir_all(root.join("templates/components")).unwrap();
        fs::create_dir_all(root.join("static")).unwrap();
        fs::write(
            root.join("zola.toml"),
            r#"base_url = "https://workbench.pana.invalid"
title = "Workbench"
compile_sass = false
build_search_index = false
"#,
        )
        .unwrap();
        fs::write(
            root.join("content/_index.md"),
            r#"+++
title = "Acasă"
template = "index.html"
+++
"#,
        )
        .unwrap();
        fs::write(
            root.join("templates/base.html"),
            r#"<!doctype html><html lang="ro"><head><meta charset="utf-8"><link rel="stylesheet" href="/site.css"><link rel="preload" href="/font.woff2" as="font" type="font/woff2" crossorigin><script src="/site.js"></script></head><body>{% block body %}{% endblock %}</body></html>"#,
        )
        .unwrap();
        fs::write(
            root.join("templates/layout.html"),
            r#"{% extends "base.html" %}{% block body %}<main class="layout">{% block page %}{% endblock %}</main>{% endblock %}"#,
        )
        .unwrap();
        fs::write(
            root.join("templates/index.html"),
            r#"{% extends "layout.html" %}{% block page %}{% include "partials/wrapper.html" %}{% endblock %}"#,
        )
        .unwrap();
        fs::write(
            root.join("templates/partials/wrapper.html"),
            r#"<section class="wrapper">{% include "partials/card.html" %}</section>"#,
        )
        .unwrap();
        fs::write(
            root.join("templates/partials/card.html"),
            r#"<article class="card">{{ section.title }}</article>"#,
        )
        .unwrap();
        fs::write(
            root.join("templates/orphan.html"),
            r#"<aside>{{ page.title }}</aside>"#,
        )
        .unwrap();
        fs::write(
            root.join("templates/components/card.html"),
            r#"{% component card(title: string) %}<strong class="component-card">{{ title }}</strong>{% endcomponent card %}"#,
        )
        .unwrap();
        fs::write(root.join("static/site.css"), ".card { color: red; }\n").unwrap();
        fs::write(root.join("static/site.js"), "window.workbench = true;\n").unwrap();
    }

    fn create_taxonomy_workbench_project(root: &Path) {
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates/tags")).unwrap();
        fs::write(
            root.join("zola.toml"),
            r#"base_url = "https://workbench.pana.invalid"
title = "Taxonomii Workbench"
compile_sass = false
build_search_index = false
taxonomies = [{ name = "tags" }]
"#,
        )
        .unwrap();
        fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = \"Acasă\"\ntemplate = \"index.html\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("content/article.md"),
            "+++\ntitle = \"Articol\"\ntemplate = \"page.html\"\n[taxonomies]\ntags = [\"Rust\"]\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("templates/index.html"),
            "<!doctype html><html><body><main>Acasă</main></body></html>",
        )
        .unwrap();
        fs::write(
            root.join("templates/page.html"),
            "<!doctype html><html><body><article>{{ page.title }}</article></body></html>",
        )
        .unwrap();
        fs::write(
            root.join("templates/tags/list.html"),
            "<!doctype html><html><body><main class=\"taxonomy-list\">{{ taxonomy.name }}</main></body></html>",
        )
        .unwrap();
        fs::write(
            root.join("templates/tags/single.html"),
            "<!doctype html><html><body><main class=\"taxonomy-term\">{{ term.name }}</main></body></html>",
        )
        .unwrap();
    }

    fn create_parity_project(root: &Path) {
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::create_dir_all(root.join("sass")).unwrap();
        fs::create_dir_all(root.join("static")).unwrap();
        fs::write(
            root.join("config.toml"),
            r#"base_url = "https://config.pana.invalid"
title = "Paritate Pană"
compile_sass = true
build_search_index = false
"#,
        )
        .unwrap();
        fs::write(
            root.join("content/_index.md"),
            r#"+++
title = "Acasă"
template = "index.html"
+++
Conținut **Markdown** randat de Zola.
"#,
        )
        .unwrap();
        fs::write(
            root.join("templates/index.html"),
            r#"<!doctype html>
<html lang="ro"><head><meta charset="utf-8"><title>{{ config.title }} · {{ section.title }}</title><link rel="stylesheet" href="{{ get_url(path='site.css') }}"></head><body><main>{{ section.content | safe }}</main><a href="{{ get_url(path='asset.txt') }}">asset</a></body></html>
"#,
        )
        .unwrap();
        fs::write(
            root.join("sass/site.scss"),
            "$accent: #147d6f; body { color: $accent; main { display: grid; } }\n",
        )
        .unwrap();
        fs::write(root.join("static/asset.txt"), "Pană Studio\n").unwrap();
    }

    fn disk_path_for_memory_route(output: &Path, route: &str) -> PathBuf {
        if route.is_empty() {
            output.join("index.html")
        } else if Path::new(route).extension().is_none() {
            output.join(route).join("index.html")
        } else {
            output.join(route)
        }
    }

    fn run_fresh_embedded_disk_build(project: &Path, output: &Path, base_url: &str) {
        with_zola_engine("test disk parity", || {
            let config = zola_config_file(project)?;
            let mut site = Site::new(project, config).map_err(|error| error.to_string())?;
            site.set_base_url(base_url.to_string());
            site.set_output_path(output);
            site.load().map_err(|error| error.to_string())?;
            site.build().map_err(|error| error.to_string())
        })
        .expect("fresh embedded Zola disk build");
    }

    fn assert_rendered_matches_disk(rendered: &HashMap<String, String>, disk_output: &Path) {
        for (route, body) in rendered {
            let disk_path = disk_path_for_memory_route(disk_output, route);
            assert_eq!(
                body.as_bytes(),
                fs::read(&disk_path)
                    .unwrap_or_else(|error| panic!("{}: {error}", disk_path.display())),
                "rendered route differs: {route}"
            );
        }
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
                .collect::<Vec<_>>();
            for (key, path) in bindings {
                env::set_var(key, path);
            }
            Self { previous_values }
        }
    }

    impl Drop for TestEnvGuard {
        fn drop(&mut self) {
            for (key, value) in &self.previous_values {
                if let Some(value) = value {
                    env::set_var(key, value);
                } else {
                    env::remove_var(key);
                }
            }
        }
    }
}
