use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};

use tauri::{AppHandle, Runtime};
use tauri_utils::html::{parse, serialize_node};
use tera::Context;
use zola_site::{sass, BuildMode, Site, SITE_CONTENT};

use crate::{
    kernel::project_workspace::WorkspaceProjectionLease,
    preview::{
        inject::{bind_canvas_identity_to_editor_html, prepare_design_safe_html},
        preprocess::{
            create_persistent_preview_artifact_root, persistent_project_workspace_session_root,
            remove_persistent_preview_artifact_root, remove_persistent_preview_session,
            reset_persistent_preview_editor_cache, seed_persistent_preview_artifacts,
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
    zola_engine::{with_zola_engine, zola_config_file},
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
    pub projected_paths: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct TemplateWorkbenchPublication {
    pub route: String,
    pub preview_url: String,
    pub workspace_revision: u64,
    pub preview_revision: String,
    pub canvas_plan: crate::preview::CanvasProjectionPlan,
}

impl PersistentPreviewCandidate {
    pub fn canvas_plan(&self) -> crate::preview::CanvasProjectionPlan {
        self.generation.canvas_transaction.plan()
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
        // A runtime session is authoritative only inside this process. Remove
        // any editor-preview residue left by an interrupted prior process;
        // sandbox/browser caches live in separate namespaces.
        reset_persistent_preview_editor_cache(app, &zola_root)?;
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

    pub fn canvas_plan_for_identity(
        &self,
        identity: &crate::preview::CanvasProjectionIdentity,
    ) -> Result<Option<crate::preview::CanvasProjectionPlan>, String> {
        self.server
            .as_ref()
            .ok_or_else(|| "Serverul Preview persistent a fost oprit.".to_string())?
            .canvas_plan_for_identity(identity)
    }

    /// Randă template-ul ales în motorul Zola deja încărcat și îl publică în
    /// generația exactă a lease-ului. Generația poate fi încă staged: astfel
    /// Workbench-ul montat poate confirma chiar candidatul canonic, fără să
    /// revină temporar la pagina site-ului sau la generația precedentă.
    pub fn publish_template_workbench_view(
        &mut self,
        lease: &WorkspaceProjectionLease,
        plan: &TemplateWorkbenchPlan,
    ) -> Result<TemplateWorkbenchPublication, String> {
        self.require_lease_owner(lease)?;
        let generation = self
            .server
            .as_ref()
            .ok_or_else(|| "Serverul Preview persistent a fost oprit.".to_string())?
            .generation_for_workspace_revision(
                &self.owner.project_root,
                &self.owner.runtime_session_id,
                lease.revision,
            )?
            .ok_or_else(|| {
                format!(
                    "Context de template nu găsește generația Preview exactă pentru revizia {}.",
                    lease.revision
                )
            })?;
        let site = self.site.as_ref().ok_or_else(|| {
            "Motorul Zola embedded nu are site activ pentru Workbench.".to_string()
        })?;
        let model =
            build_project_model_from_workspace_projection(Path::new(&lease.project_root), lease)?;
        let (rendered, canvas_route) = with_zola_engine("Context de template Preview", || {
            render_template_workbench_document(site, &self.raw_content, &model, plan)
        })?;
        let annotated = CanvasGraph::annotate_rendered_document(&model, &canvas_route, &rendered)?;
        let mut prepared = prepare_design_safe_html(&annotated, &generation.preview_revision)?;
        bind_canvas_identity_to_editor_html(
            &mut prepared,
            &generation.canvas_transaction.identity,
        )?;

        let route = template_workbench_route(&plan.active_template.source_id);
        generation
            .workbench_content
            .write()
            .map_err(|_| "Registrul Context de template este indisponibil.".to_string())?
            .insert(route.clone(), RenderedPreviewContent::Html(prepared));
        let preview_url = format!(
            "{}{}?__pana_preview_revision={}&__pana_canvas_transaction={}",
            self.url()?,
            route,
            generation.preview_revision,
            generation.canvas_transaction.identity.transaction_id
        );
        Ok(TemplateWorkbenchPublication {
            route,
            preview_url,
            workspace_revision: lease.revision,
            preview_revision: generation.preview_revision.clone(),
            canvas_plan: generation.canvas_transaction.plan(),
        })
    }

    pub fn render_candidate<R: Runtime>(
        &mut self,
        app: &AppHandle<R>,
        lease: &WorkspaceProjectionLease,
    ) -> Result<PersistentPreviewCandidate, String> {
        self.require_lease_owner(lease)?;
        self.collect_retired(app);

        let update = match sync_persistent_project_workspace(
            app,
            &self.zola_root,
            &self.session_root,
            self.projection_manifest.as_ref(),
            lease,
        ) {
            Ok(update) => update,
            Err(error) => {
                // A failed delta may have touched the derived source root.
                // Force a complete accepted-baseline rebuild on the next try.
                self.projection_manifest = None;
                self.site = None;
                self.raw_content.clear();
                return Err(error);
            }
        };
        self.projection_manifest = Some(update.manifest.clone());

        let preview_revision = next_preview_revision(lease.revision);
        let artifact_root =
            create_persistent_preview_artifact_root(app, &self.session_root, &preview_revision)?;
        let result =
            self.render_zola_generation(app, &update, &artifact_root, lease, &preview_revision);
        let generation = match result {
            Ok(generation) => generation,
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

        Ok(PersistentPreviewCandidate {
            generation: Arc::new(generation),
            projected_paths: update.projected_paths,
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

    fn render_zola_generation<R: Runtime>(
        &mut self,
        app: &AppHandle<R>,
        update: &PersistentProjectionUpdate,
        artifact_root: &Path,
        lease: &WorkspaceProjectionLease,
        preview_revision: &str,
    ) -> Result<ActivePreviewGeneration, String> {
        let base_url = self.url()?;
        let impact =
            projection_render_impact(update, self.site.is_some(), !self.raw_content.is_empty());
        let previous_assets_root = self
            .active_generation()?
            .map(|generation| generation.assets_root.clone());
        let rendered = with_zola_engine("randare Preview persistentă", || match impact {
            ProjectionRenderImpact::Full => build_new_official_zola_site(
                &update.projection_root,
                artifact_root,
                &base_url,
                lease.revision,
                DraftRenderPolicy::Include,
            )
            .map(|(site, rendered)| {
                self.site = Some(site);
                rendered
            }),
            ProjectionRenderImpact::Templates => self
                .site
                .as_mut()
                .ok_or_else(|| "Motorul Zola persistent nu are site activ.".to_string())
                .and_then(|site| {
                    if let Some(previous_assets_root) = previous_assets_root.as_deref() {
                        seed_persistent_preview_artifacts(
                            app,
                            &self.session_root,
                            previous_assets_root,
                            artifact_root,
                        )?;
                    }
                    site.set_base_url(base_url.clone());
                    site.set_output_path(artifact_root);
                    clear_site_content()?;
                    site.reload_templates().map_err(|error| {
                        format!(
                            "Zola 0.22.1 nu a putut reîncărca template-urile reviziei {}: {error}",
                            lease.revision
                        )
                    })?;
                    capture_site_content()
                }),
            ProjectionRenderImpact::AssetsOnly => self
                .site
                .as_mut()
                .ok_or_else(|| "Motorul Zola persistent nu are site activ.".to_string())
                .and_then(|site| {
                    site.set_base_url(base_url.clone());
                    site.set_output_path(artifact_root);
                    materialize_official_zola_assets(site, lease.revision)?;
                    Ok(self.raw_content.clone())
                }),
        });
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
        self.raw_content = rendered.clone();
        let model =
            build_project_model_from_workspace_projection(Path::new(&lease.project_root), lease)?;
        let mut content = HashMap::with_capacity(rendered.len());
        for (path, body) in rendered {
            let extension = Path::new(&path)
                .extension()
                .and_then(|value| value.to_str());
            let prepared_body = if matches!(extension, Some("xml" | "json" | "txt")) {
                body
            } else {
                CanvasGraph::annotate_rendered_document(
                    &model,
                    &canvas_route_for_content_key(&path),
                    &body,
                )?
            };
            content.insert(
                path.clone(),
                prepare_rendered_content(extension, &prepared_body, preview_revision)?,
            );
        }

        let rendered_documents = content
            .iter()
            .filter_map(|(content_key, rendered)| match rendered {
                RenderedPreviewContent::Html(html) => Some((
                    canvas_route_for_content_key(content_key),
                    html.editor.as_str(),
                )),
                RenderedPreviewContent::Text { .. } => None,
            })
            .collect::<Vec<_>>();
        let graph = CanvasGraph::from_rendered_documents(
            &model,
            lease.revision,
            preview_revision,
            rendered_documents
                .iter()
                .map(|(route, html)| (route.as_str(), *html)),
        )?;
        let resources =
            CanvasResourceManifest::from_artifact_root(preview_revision, artifact_root)?;
        let canvas_transaction = CanvasProjectionTransaction::prepared(
            &self.owner.project_root,
            &self.owner.runtime_session_id,
            lease.revision,
            preview_revision,
            lease.workspace_transaction_id.clone(),
            PreviewImpact::from_projected_paths(&update.projected_paths, update.baseline_rebuilt),
            graph,
            resources,
        )?;
        for rendered in content.values_mut() {
            if let RenderedPreviewContent::Html(html) = rendered {
                bind_canvas_identity_to_editor_html(html, &canvas_transaction.identity)?;
            }
        }

        Ok(ActivePreviewGeneration {
            project_root: self.owner.project_root.clone(),
            runtime_session_id: self.owner.runtime_session_id.clone(),
            workspace_revision: lease.revision,
            preview_revision: preview_revision.to_string(),
            canvas_transaction,
            content,
            workbench_content: Arc::new(RwLock::new(HashMap::new())),
            assets_root: artifact_root.to_path_buf(),
        })
    }

    fn collect_retired<R: Runtime>(&mut self, app: &AppHandle<R>) {
        let mut retained = Vec::new();
        for generation in self.retired.drain(..) {
            if Arc::strong_count(&generation) == 1 {
                let root = generation.assets_root.clone();
                drop(generation);
                if remove_persistent_preview_artifact_root(app, &self.session_root, &root).is_err()
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

    fn require_lease_owner(&self, lease: &WorkspaceProjectionLease) -> Result<(), String> {
        if lease.project_root != self.owner.project_root
            || lease.runtime_session_id != self.owner.runtime_session_id
        {
            return Err(format!(
                "Motorul Preview refuză lease-ul altei sesiuni: primit {}/{}, activ {}/{}.",
                lease.project_root,
                lease.runtime_session_id,
                self.owner.project_root,
                self.owner.runtime_session_id
            ));
        }
        Ok(())
    }
}

fn render_template_workbench_document(
    site: &Site,
    canonical_content: &HashMap<String, String>,
    model: &ProjectModel,
    plan: &TemplateWorkbenchPlan,
) -> Result<(String, String), String> {
    let (mut context, context_route) = template_workbench_context(site, plan)?;
    if !plan.render_context.canonical_truth {
        install_controlled_workbench_fixture(&mut context);
    }
    let active_template_name = engine_template_name(
        &plan.active_template.name,
        plan.active_template.theme_name.as_deref(),
    );

    let rendered = match plan.render_mode {
        TemplateWorkbenchRenderMode::MacroScenario => {
            render_macro_scenario(&site.tera, &active_template_name, context)?
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
    if is_complete_html_document(&rendered)
        && !matches!(
            plan.render_mode,
            TemplateWorkbenchRenderMode::IncludedTemplate
                | TemplateWorkbenchRenderMode::MacroScenario
        )
    {
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
    let result = if theme_template {
        site.tera.render(template_name, &context)
    } else {
        zola_utils::templates::render_template(
            template_name,
            &site.tera,
            context,
            &site.config.theme,
        )
        .map_err(|error| tera::Error::msg(error.to_string()))
    };
    result
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

fn render_macro_scenario(
    source_tera: &tera::Tera,
    template_name: &str,
    mut context: Context,
) -> Result<String, String> {
    let definition = source_tera.get_template(template_name).map_err(|error| {
        format!("Macro scenario nu a găsit template-ul «{template_name}»: {error}")
    })?;
    let mut macro_names = definition.macros.keys().cloned().collect::<Vec<_>>();
    macro_names.sort();
    let macro_name = macro_names.first().ok_or_else(|| {
        format!("Template-ul «{template_name}» nu definește niciun macro apelabil.")
    })?;
    let macro_definition = definition
        .macros
        .get(macro_name)
        .expect("macro name was collected from the same template");
    let mut argument_names = macro_definition.args.keys().cloned().collect::<Vec<_>>();
    argument_names.sort();
    let mut calls = Vec::new();
    for argument_name in argument_names {
        if macro_definition
            .args
            .get(&argument_name)
            .is_some_and(Option::is_some)
        {
            continue;
        }
        let variable_name = format!("__pana_macro_arg_{}", safe_tera_identifier(&argument_name));
        context.insert(&variable_name, &controlled_macro_argument(&argument_name));
        calls.push(format!("{argument_name}={variable_name}"));
    }
    let harness = format!(
        "{{% import \"{template_name}\" as pana_workbench_macro %}}\n{{{{ pana_workbench_macro::{macro_name}({}) }}}}",
        calls.join(", ")
    );
    let mut tera = source_tera.clone();
    let harness_name = "__pana_template_workbench_macro_scenario.html";
    tera.add_raw_template(harness_name, &harness)
        .map_err(|error| format!("Scenariul macro nu a putut fi compilat: {error}"))?;
    tera.render(harness_name, &context)
        .map_err(|error| format!("Scenariul macro controlat a eșuat: {error}"))
}

fn safe_tera_identifier(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn controlled_macro_argument(name: &str) -> serde_json::Value {
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
    let library = site.library.read().map_err(|_| {
        "Biblioteca Zola este indisponibilă pentru Context de template.".to_string()
    })?;

    if let Some(selected_file) = selected_file.as_deref() {
        if let Some(page) = library
            .pages
            .values()
            .find(|page| normalized_content_file(&page.file.relative) == selected_file)
        {
            let mut context = Context::new();
            context.insert("config", &site.config.serialize(&page.lang));
            context.insert("current_url", &page.permalink);
            context.insert("current_path", &page.path);
            context.insert("zola_version", "0.22.1");
            context.insert("page", &page.serialize(&library));
            context.insert("lang", &page.lang);
            return Ok((context, Some(page.path.clone())));
        }
        if let Some(section) = library
            .sections
            .values()
            .find(|section| normalized_content_file(&section.file.relative) == selected_file)
        {
            let mut context = Context::new();
            context.insert("config", &site.config.serialize(&section.lang));
            context.insert("current_url", &section.permalink);
            context.insert("current_path", &section.path);
            context.insert("zola_version", "0.22.1");
            context.insert("section", &section.serialize(&library));
            context.insert("lang", &section.lang);
            return Ok((context, Some(section.path.clone())));
        }
        return Err(format!(
            "Contextul consumator «{selected_file}» nu există în biblioteca motorului Zola pentru această revizie."
        ));
    }

    let lang = site.config.default_language.clone();
    let mut context = Context::new();
    context.insert("config", &site.config.serialize(&lang));
    context.insert("current_url", &site.config.base_url);
    context.insert("current_path", "/");
    context.insert("zola_version", "0.22.1");
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
    let route = route
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(route)
        .trim();
    let key = if route == "/" {
        String::new()
    } else {
        route.trim_start_matches('/').to_string()
    };
    content.get(&key).map(String::as_str)
}

fn is_complete_html_document(html: &str) -> bool {
    let normalized = html.trim_start().to_ascii_lowercase();
    normalized.starts_with("<!doctype html") || normalized.starts_with("<html")
}

fn mount_workbench_fragment(
    canonical: Option<&str>,
    fragment: &str,
    source_id: &str,
    source_file: &str,
) -> Result<String, String> {
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

    let fragment_document = parse(format!(
        "<!doctype html><html><body><div data-pana-workbench-mount>{fragment}</div></body></html>"
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
    Templates,
    AssetsOnly,
}

fn projection_render_impact(
    update: &PersistentProjectionUpdate,
    has_site: bool,
    has_rendered_content: bool,
) -> ProjectionRenderImpact {
    if update.baseline_rebuilt || !has_site || !has_rendered_content {
        return ProjectionRenderImpact::Full;
    }
    let mut templates = false;
    for project_relative in &update.projected_paths {
        let relative = project_relative.as_str();
        if relative == "config.toml"
            || relative == "zola.toml"
            || relative.starts_with("content/")
            || relative.starts_with("themes/")
            || relative.starts_with("templates/shortcodes/")
            || !(relative.starts_with("templates/")
                || relative.starts_with("sass/")
                || relative.starts_with("static/"))
        {
            return ProjectionRenderImpact::Full;
        }
        if relative.starts_with("templates/") {
            templates = true;
        }
    }
    if templates {
        ProjectionRenderImpact::Templates
    } else {
        ProjectionRenderImpact::AssetsOnly
    }
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
        format!("Zola 0.22.1 nu a putut încărca proiecția reviziei {workspace_revision}: {error}")
    })?;
    site.enable_serve_mode(BuildMode::Memory);
    if draft_policy == DraftRenderPolicy::Include {
        // Editare sigură projects the full authoring workspace. Draft visibility
        // is an editor concern and must not change the production-like Source
        // Browser generation rendered from accepted disk state.
        site.include_drafts();
    }
    site.set_base_url(base_url.to_string());
    site.set_output_path(artifact_root);
    site.load().map_err(|error| {
        format!("Zola 0.22.1 nu a putut încărca conținutul reviziei {workspace_revision}: {error}")
    })?;
    site.build().map_err(|error| {
        format!("Zola 0.22.1 nu a putut randă revizia {workspace_revision}: {error}")
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

fn materialize_official_zola_assets(site: &Site, workspace_revision: u64) -> Result<(), String> {
    if let Some(theme) = &site.config.theme {
        let theme_root = site.base_path.join("themes").join(theme);
        if theme_root.join("sass").is_dir() {
            sass::compile_sass(&theme_root, &site.output_path).map_err(|error| {
                format!(
                    "Zola 0.22.1 nu a putut compila Sass-ul temei pentru revizia {workspace_revision}: {error}"
                )
            })?;
        }
    }
    if site.config.compile_sass {
        sass::compile_sass(&site.base_path, &site.output_path).map_err(|error| {
            format!(
                "Zola 0.22.1 nu a putut compila Sass pentru revizia {workspace_revision}: {error}"
            )
        })?;
    }
    site.render_themes_css().map_err(|error| {
        format!(
            "Zola 0.22.1 nu a putut genera temele CSS pentru revizia {workspace_revision}: {error}"
        )
    })?;
    site.process_images().map_err(|error| {
        format!(
            "Zola 0.22.1 nu a putut procesa imaginile pentru revizia {workspace_revision}: {error}"
        )
    })?;
    site.copy_static_directories().map_err(|error| {
        format!(
            "Zola 0.22.1 nu a putut materializa asset-urile reviziei {workspace_revision}: {error}"
        )
    })
}

fn prepare_rendered_content(
    extension: Option<&str>,
    body: &str,
    preview_revision: &str,
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
        None => Ok(RenderedPreviewContent::Html(prepare_design_safe_html(
            body,
            preview_revision,
        )?)),
    }
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
        zola_engine::acquire_zola_engine_for_test,
    };

    use super::*;

    #[test]
    fn macro_scenario_calls_real_macro_with_controlled_required_arguments() {
        let mut tera = tera::Tera::default();
        tera.add_raw_template(
            "macros/card.html",
            concat!(
                "{% macro card(title, visible=true) %}",
                "{% if visible %}<article class=\"card\">{{ title }}</article>{% endif %}",
                "{% endmacro %}",
            ),
        )
        .unwrap();

        let rendered = render_macro_scenario(&tera, "macros/card.html", Context::new()).unwrap();

        assert!(rendered.contains("<article class=\"card\">Exemplu</article>"));
    }

    #[test]
    fn workbench_renders_page_partial_orphan_and_macro_with_declared_contexts() {
        let fixture = parity_fixture("template-workbench-render-matrix");
        let project = fixture.join("project");
        let zola_root = project.to_path_buf();
        let artifacts = fixture.join("artifacts");
        create_workbench_render_project(&zola_root);
        let model = crate::project_model::build_project_model(&project, &HashMap::new()).unwrap();
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
            },
        )
        .unwrap();
        let (index_html, index_route) =
            render_template_workbench_document(&site, &canonical, &model, &index_plan).unwrap();
        assert_eq!(index_route, "/");
        assert!(index_plan.render_context.canonical_truth);
        assert!(index_html.contains("<main class=\"layout\">"));
        assert!(index_html.contains("<article class=\"card\">Acasă</article>"));

        let partial_plan =
            crate::project_model::template_workbench::resolve_template_workbench_plan(
                &model,
                &crate::project_model::template_workbench::TemplateWorkbenchPlanInput {
                    template_path: "templates/partials/wrapper.html".to_string(),
                    preferred_page_path: None,
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
            prepare_design_safe_html(&partial_html, "workbench-partial").unwrap();
        assert!(prepared_partial.editor.contains("/site.css"));
        assert!(!prepared_partial.editor.contains("/site.js"));
        assert!(prepared_partial.interactive.contains("/site.js"));

        let orphan_plan =
            crate::project_model::template_workbench::resolve_template_workbench_plan(
                &model,
                &crate::project_model::template_workbench::TemplateWorkbenchPlanInput {
                    template_path: "templates/orphan.html".to_string(),
                    preferred_page_path: None,
                },
            )
            .unwrap();
        let (orphan_html, _) =
            render_template_workbench_document(&site, &canonical, &model, &orphan_plan).unwrap();
        assert!(!orphan_plan.render_context.canonical_truth);
        assert!(orphan_html.contains("<aside>Pagină demonstrativă</aside>"));

        let macro_plan = crate::project_model::template_workbench::resolve_template_workbench_plan(
            &model,
            &crate::project_model::template_workbench::TemplateWorkbenchPlanInput {
                template_path: "templates/macros/card.html".to_string(),
                preferred_page_path: None,
            },
        )
        .unwrap();
        let (macro_html, _) =
            render_template_workbench_document(&site, &canonical, &model, &macro_plan).unwrap();
        assert!(!macro_plan.render_context.canonical_truth);
        assert!(macro_html.contains("<strong class=\"macro-card\">Exemplu</strong>"));

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
        let model = crate::project_model::build_project_model(&project, &HashMap::new()).unwrap();
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
        assert!(matches!(
            prepare_rendered_content(Some("xml"), "<xml/>", "r1").unwrap(),
            RenderedPreviewContent::Text { content_type, .. } if content_type.starts_with("text/xml")
        ));
        assert!(matches!(
            prepare_rendered_content(None, "<!doctype html><html><body></body></html>", "r1")
                .unwrap(),
            RenderedPreviewContent::Html(_)
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
            workspace_rendered.contains_key("despre/"),
            "generația workspace nu conține ruta draft: {:?}",
            workspace_rendered.keys().collect::<Vec<_>>()
        );
        assert!(
            !disk_rendered.contains_key("despre/"),
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
        };
        assert_eq!(
            projection_render_impact(&update(&["templates/index.html"], false), true, true),
            ProjectionRenderImpact::Templates
        );
        assert_eq!(
            projection_render_impact(&update(&["sass/pages/index.scss"], false), true, true),
            ProjectionRenderImpact::AssetsOnly
        );
        assert_eq!(
            projection_render_impact(&update(&["content/about.md"], false), true, true),
            ProjectionRenderImpact::Full
        );
        assert_eq!(
            projection_render_impact(&update(&[], true), true, true),
            ProjectionRenderImpact::Full
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
        site.set_output_path(&template_output);
        clear_site_content().unwrap();
        site.reload_templates().unwrap();
        let template_rendered = capture_site_content().unwrap();

        fs::write(
            project.join("sass/site.scss"),
            "$accent: #a32952; body { color: $accent; main { display: flex; } }\n",
        )
        .unwrap();
        site.set_output_path(&sass_output);
        materialize_official_zola_assets(&site, 3).unwrap();
        let sass_rendered = template_rendered.clone();
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
        let lease = |revision: u64,
                     source_texts: HashMap<String, String>,
                     changed_paths: HashSet<String>| WorkspaceProjectionLease {
            project_root: project_root.clone(),
            runtime_session_id: session_id.to_string(),
            revision,
            workspace_transaction_id: Some(format!("runtime-preview-{revision}")),
            source_texts,
            resource_bytes: HashMap::new(),
            deleted_sources: HashSet::new(),
            changed_paths,
            accepted_disk: accepted_disk.clone(),
        };
        let owner = PersistentPreviewOwner::new(&project_root, session_id);
        let mut engine =
            PersistentZolaPreviewEngine::start(&app_handle, &zola_root, owner).unwrap();

        let first = engine
            .render_candidate(&app_handle, &lease(1, source_texts.clone(), HashSet::new()))
            .unwrap();
        let first_revision = first.generation.preview_revision.clone();
        stage_and_confirm(&mut engine, &app_handle, first);
        let url = engine.url().unwrap();
        let first_document = read_http_document(&format!("{url}/")).unwrap();
        assert!(first_document.contains(&first_revision));
        assert!(!first_document.contains("data-draft=\"two\""));

        let template_path = "templates/index.html".to_string();
        source_texts.insert(
            template_path.clone(),
            source_texts[&template_path].replace("<main>", "<main data-draft=\"two\">"),
        );
        let second_lease = lease(
            2,
            source_texts.clone(),
            HashSet::from([template_path.clone()]),
        );
        let second = engine.render_candidate(&app_handle, &second_lease).unwrap();
        let second_identity = second.generation.canvas_transaction.identity.clone();
        // Candidate construction is not publication.
        assert!(!read_http_document(&format!("{url}/"))
            .unwrap()
            .contains("data-draft=\"two\""));
        engine.stage_candidate(&app_handle, second).unwrap();

        let second_model = build_project_model_from_workspace_projection(
            Path::new(&second_lease.project_root),
            &second_lease,
        )
        .unwrap();
        let workbench_plan =
            crate::project_model::template_workbench::resolve_template_workbench_plan(
                &second_model,
                &crate::project_model::template_workbench::TemplateWorkbenchPlanInput {
                    template_path: template_path.clone(),
                    preferred_page_path: None,
                },
            )
            .unwrap();
        let workbench = engine
            .publish_template_workbench_view(&second_lease, &workbench_plan)
            .unwrap();
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
            sass_path.clone(),
            "$accent: #a32952; body { color: $accent; main { display: flex; } }\n".to_string(),
        );
        let third = engine
            .render_candidate(
                &app_handle,
                &lease(
                    3,
                    source_texts.clone(),
                    // A real lease still reports every path dirty against
                    // Save; the projection result must expose only this
                    // revision-to-revision Sass delta.
                    HashSet::from([template_path.clone(), sass_path.clone()]),
                ),
            )
            .unwrap();
        assert_eq!(third.projected_paths, vec![sass_path]);
        assert!(read_http_document(&format!("{url}/site.css"))
            .unwrap()
            .contains("#147d6f"));
        stage_and_confirm(&mut engine, &app_handle, third);
        assert!(read_http_document(&format!("{url}/site.css"))
            .unwrap()
            .contains("#a32952"));

        source_texts.insert(template_path.clone(), "{% if %}".to_string());
        assert!(engine
            .render_candidate(
                &app_handle,
                &lease(4, source_texts, HashSet::from([template_path])),
            )
            .is_err());
        assert!(read_http_document(&format!("{url}/"))
            .unwrap()
            .contains("data-draft=\"two\""));

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

    fn create_workbench_render_project(root: &Path) {
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates/partials")).unwrap();
        fs::create_dir_all(root.join("templates/macros")).unwrap();
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
            r#"<!doctype html><html lang="ro"><head><meta charset="utf-8"><link rel="stylesheet" href="/site.css"><script src="/site.js"></script></head><body>{% block body %}{% endblock %}</body></html>"#,
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
            root.join("templates/macros/card.html"),
            r#"{% macro card(title) %}<strong class="macro-card">{{ title }}</strong>{% endmacro %}"#,
        )
        .unwrap();
        fs::write(root.join("static/site.css"), ".card { color: red; }\n").unwrap();
        fs::write(root.join("static/site.js"), "window.workbench = true;\n").unwrap();
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
        } else if route.ends_with('/') {
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
