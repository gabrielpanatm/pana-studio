use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    ops::Range,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};

use crate::{
    deploy::run_zola_editor_check,
    kernel::{
        project_path::normalize_project_relative_path,
        project_workspace::{
            ProjectWorkspace, ProjectWorkspaceIdentity, ProjectWorkspaceMutationReceipt,
            WorkspaceMutationMetadata, WorkspaceResourceDelete, WorkspaceResourceMutation,
        },
        source_graph_rewrite::{
            plan_template_reference_workspace_mutation_from_graph, SourceGraphRewriteOperation,
        },
        write_authority::ComponentValidationSandboxLease,
    },
    localization::LocalizedDiagnostic,
    source_graph::{
        build_source_graph_from_workspace_projection,
        model::{
            ComponentDefinition, ComponentDefinitionKind, ComponentDependency,
            ComponentDependencyKind, ComponentOrigin, SourceDiagnosticSeverity, SourceGraph,
        },
    },
    zola_theme::{
        conventional_script_files_for_template, conventional_style_files_for_template,
        ZolaTemplateOrigin,
    },
};

pub const COMPONENT_MUTATION_SCHEMA_VERSION: u32 = 3;
static COMPONENT_VALIDATION_GENERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ComponentMutationOperation {
    Create,
    Update,
    Duplicate,
    Move,
    Rename,
    Extract,
    Delete,
    OverrideTheme,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ComponentDraftKind {
    Partial,
    TeraComponent,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ComponentCompanionKind {
    Style,
    Script,
    Data,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComponentCompanionDraft {
    pub kind: ComponentCompanionKind,
    pub relative_path: String,
    pub contents: String,
    #[serde(default)]
    pub create_only: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComponentExtractionRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentMutationInput {
    pub operation: ComponentMutationOperation,
    pub definition_id: Option<String>,
    pub kind: Option<ComponentDraftKind>,
    pub name: Option<String>,
    #[serde(default)]
    pub symbol_name: Option<String>,
    pub destination_name: Option<String>,
    pub contents: Option<String>,
    #[serde(default)]
    pub source_file: Option<String>,
    #[serde(default)]
    pub source_range: Option<ComponentExtractionRange>,
    #[serde(default)]
    pub companions: Vec<ComponentCompanionDraft>,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComponentPlannedWrite {
    pub relative_path: String,
    pub contents: String,
    pub create_only: bool,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComponentMutationDiagnostic {
    pub diagnostic: LocalizedDiagnostic,
    pub relative_path: Option<String>,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComponentMutationPlan {
    pub schema_version: u32,
    pub operation: ComponentMutationOperation,
    pub definition_id: Option<String>,
    pub source_relative_path: Option<String>,
    pub destination_relative_path: Option<String>,
    pub source_symbol: Option<String>,
    pub destination_symbol: Option<String>,
    pub writes: Vec<ComponentPlannedWrite>,
    pub deletes: Vec<String>,
    pub touched_files: Vec<String>,
    pub diagnostics: Vec<ComponentMutationDiagnostic>,
}

impl ComponentMutationPlan {
    fn workspace_writes(&self) -> Vec<WorkspaceResourceMutation> {
        self.writes
            .iter()
            .map(|write| WorkspaceResourceMutation {
                relative_path: write.relative_path.clone(),
                contents: write.contents.clone(),
                create_only: write.create_only,
            })
            .collect()
    }

    fn workspace_deletes(&self) -> Vec<WorkspaceResourceDelete> {
        self.deletes
            .iter()
            .map(|relative_path| WorkspaceResourceDelete {
                relative_path: relative_path.clone(),
            })
            .collect()
    }
}

pub fn plan_component_mutation(
    project_root: &Path,
    workspace: &ProjectWorkspace,
    input: ComponentMutationInput,
) -> Result<ComponentMutationPlan, String> {
    require_no_duplicate_companions(&input.companions)?;
    let companion_drafts = input.companions.clone();
    let projection = workspace.capture_projection_snapshot()?;
    let graph = build_source_graph_from_workspace_projection(project_root, &projection)?;
    require_graph_without_errors(&graph, "starea curentă")?;

    let definition = input
        .definition_id
        .as_deref()
        .map(|definition_id| require_component_definition(&graph, definition_id))
        .transpose()?;
    let mut writes = BTreeMap::<String, ComponentPlannedWrite>::new();
    let mut deletes = BTreeSet::<String>::new();
    let mut diagnostics = Vec::new();
    let mut source_relative_path = None;
    let mut destination_relative_path = None;
    let mut source_symbol = None;
    let mut destination_symbol = None;

    match input.operation {
        ComponentMutationOperation::Create => {
            if let Some(definition) = definition {
                return Err(format!(
                    "Crearea unei componente locale nu acceptă o definiție-sursă externă ({}).",
                    definition.display_name
                ));
            }
            let kind = input.kind.unwrap_or(ComponentDraftKind::Partial);
            let expected_symbol = if kind == ComponentDraftKind::TeraComponent {
                Some(validate_component_symbol_name(
                    input.symbol_name.as_deref().ok_or_else(|| {
                        "Crearea unei componente Tera 2 cere numele simbolului.".to_string()
                    })?,
                )?)
            } else {
                None
            };
            let destination = component_path(
                kind,
                input
                    .name
                    .as_deref()
                    .ok_or_else(|| "Crearea componentei cere un nume.".to_string())?,
            )?;
            require_destination_available(workspace, &destination)?;
            insert_write(
                &mut writes,
                ComponentPlannedWrite {
                    relative_path: destination.clone(),
                    contents: input
                        .contents
                        .unwrap_or_else(|| component_draft(kind, &destination)),
                    create_only: true,
                },
            )?;
            destination_relative_path = Some(destination);
            destination_symbol = expected_symbol;
        }
        ComponentMutationOperation::Update => {
            let definition = require_file_component(definition, input.operation)?;
            require_project_definition(definition, input.operation)?;
            require_capability(definition.capabilities.can_edit, definition, "editată")?;
            let source = component_source_path(definition)?;
            if definition.kind == ComponentDefinitionKind::TeraComponent
                && input.destination_name.is_some()
            {
                return Err(
                    "Redenumirea unui simbol Tera 2 folosește operația semantică Rename."
                        .to_string(),
                );
            }
            let contents = input
                .contents
                .ok_or_else(|| "Editarea componentei cere sursa completă.".to_string())?;
            let destination = if let Some(destination_name) = input.destination_name.as_deref() {
                component_path(draft_kind_for_definition(definition)?, destination_name)?
            } else {
                source.clone()
            };
            if destination == source {
                insert_write(
                    &mut writes,
                    ComponentPlannedWrite {
                        relative_path: source.clone(),
                        contents,
                        create_only: false,
                    },
                )?;
            } else {
                require_destination_available(workspace, &destination)?;
                insert_write(
                    &mut writes,
                    ComponentPlannedWrite {
                        relative_path: destination.clone(),
                        contents,
                        create_only: true,
                    },
                )?;
                plan_component_reference_rewrites(
                    project_root,
                    workspace,
                    &graph,
                    &source,
                    &destination,
                    &mut writes,
                )?;
                plan_component_companion_bundle(
                    workspace,
                    &graph,
                    definition,
                    &destination,
                    ComponentCompanionTransfer::Relocate,
                    &mut writes,
                    &mut deletes,
                    &mut diagnostics,
                )?;
                deletes.insert(source.clone());
            }
            source_relative_path = Some(source.clone());
            destination_relative_path = Some(destination);
            source_symbol = definition.symbol.clone();
            destination_symbol = definition.symbol.clone();
        }
        ComponentMutationOperation::Duplicate => {
            let definition = require_file_component(definition, input.operation)?;
            require_capability(
                definition.capabilities.can_duplicate,
                definition,
                "duplicată",
            )?;
            let source = component_source_path(definition)?;
            let kind = draft_kind_for_definition(definition)?;
            let destination = component_path(
                kind,
                input
                    .destination_name
                    .as_deref()
                    .ok_or_else(|| "Duplicarea componentei cere un nume nou.".to_string())?,
            )?;
            require_destination_available(workspace, &destination)?;
            insert_write(
                &mut writes,
                ComponentPlannedWrite {
                    relative_path: destination.clone(),
                    contents: require_workspace_text(workspace, &source)?,
                    create_only: true,
                },
            )?;
            plan_component_companion_bundle(
                workspace,
                &graph,
                definition,
                &destination,
                ComponentCompanionTransfer::Copy,
                &mut writes,
                &mut deletes,
                &mut diagnostics,
            )?;
            source_relative_path = Some(source);
            destination_relative_path = Some(destination);
        }
        ComponentMutationOperation::Rename
            if definition.is_some_and(|definition| {
                definition.kind == ComponentDefinitionKind::TeraComponent
            }) =>
        {
            let definition = require_file_component(definition, input.operation)?;
            require_project_definition(definition, input.operation)?;
            require_capability(definition.capabilities.can_rename, definition, "redenumită")?;
            let source = component_source_path(definition)?;
            let old_symbol = definition.symbol.as_deref().ok_or_else(|| {
                "Definiția Tera 2 nu expune simbolul semantic curent.".to_string()
            })?;
            let new_symbol = validate_component_symbol_name(
                input
                    .destination_name
                    .as_deref()
                    .ok_or_else(|| "Redenumirea cere noul nume al simbolului.".to_string())?,
            )?;
            if old_symbol == new_symbol {
                return Err("Operația nu schimbă numele simbolului.".to_string());
            }
            require_component_symbol_available(&graph, &new_symbol, &definition.id)?;
            plan_tera_component_symbol_rename(
                workspace,
                &graph,
                definition,
                &new_symbol,
                &mut writes,
            )?;
            source_relative_path = Some(source.clone());
            destination_relative_path = Some(source);
            source_symbol = Some(old_symbol.to_string());
            destination_symbol = Some(new_symbol);
        }
        ComponentMutationOperation::Move | ComponentMutationOperation::Rename => {
            let definition = require_file_component(definition, input.operation)?;
            require_project_definition(definition, input.operation)?;
            let allowed = if input.operation == ComponentMutationOperation::Move {
                definition.capabilities.can_move
            } else {
                definition.capabilities.can_rename
            };
            require_capability(allowed, definition, "mutată sau redenumită")?;
            let source = component_source_path(definition)?;
            let kind = draft_kind_for_definition(definition)?;
            let destination = component_path(
                kind,
                input
                    .destination_name
                    .as_deref()
                    .ok_or_else(|| "Mutarea componentei cere destinația nouă.".to_string())?,
            )?;
            if source == destination {
                return Err("Operația nu schimbă path-ul componentei.".to_string());
            }
            require_destination_available(workspace, &destination)?;
            insert_write(
                &mut writes,
                ComponentPlannedWrite {
                    relative_path: destination.clone(),
                    contents: require_workspace_text(workspace, &source)?,
                    create_only: true,
                },
            )?;
            plan_component_reference_rewrites(
                project_root,
                workspace,
                &graph,
                &source,
                &destination,
                &mut writes,
            )?;
            plan_component_companion_bundle(
                workspace,
                &graph,
                definition,
                &destination,
                ComponentCompanionTransfer::Relocate,
                &mut writes,
                &mut deletes,
                &mut diagnostics,
            )?;
            deletes.insert(source.clone());
            source_relative_path = Some(source);
            destination_relative_path = Some(destination);
        }
        ComponentMutationOperation::Extract => {
            if definition.is_some() {
                return Err(
                    "Extragerea cere un nod sursă exact, nu definitionId existent.".to_string(),
                );
            }
            let kind = input.kind.unwrap_or(ComponentDraftKind::Partial);
            if kind != ComponentDraftKind::Partial {
                return Err("Extragerea structurală produce exclusiv o parțială Tera.".to_string());
            }
            if input.contents.is_some() {
                return Err(
                    "Extragerea nu acceptă sursă aproximată; conținutul este citit lossless din range-ul SourceGraph."
                        .to_string(),
                );
            }
            let source = normalize_project_relative_path(
                input
                    .source_file
                    .as_deref()
                    .ok_or_else(|| "Extragerea cere fișierul sursă.".to_string())?,
            )?;
            let extraction_range = input.source_range.as_ref().ok_or_else(|| {
                "Extragerea cere range-ul exact al nodului SourceGraph.".to_string()
            })?;
            let source_node = graph
                .nodes
                .iter()
                .find(|node| {
                    node.file == source
                        && node.range.as_ref().is_some_and(|range| {
                            range.start == extraction_range.start
                                && range.end == extraction_range.end
                        })
                })
                .ok_or_else(|| {
                    "Range-ul de extragere nu corespunde exact niciunui nod SourceGraph."
                        .to_string()
                })?;
            if !source_node.capabilities.can_extract_partial {
                return Err(source_node
                    .capabilities
                    .technical_reason()
                    .map(str::to_string)
                    .unwrap_or_else(|| {
                        format!("Nodul {} nu poate fi extras lossless.", source_node.label)
                    }));
            }
            let source_text = require_workspace_text(workspace, &source)?;
            if extraction_range.start >= extraction_range.end
                || extraction_range.end > source_text.len()
                || !source_text.is_char_boundary(extraction_range.start)
                || !source_text.is_char_boundary(extraction_range.end)
            {
                return Err(
                    "Range-ul de extragere nu este o limită UTF-8 validă în sursa curentă."
                        .to_string(),
                );
            }
            let destination = component_path(
                kind,
                input
                    .name
                    .as_deref()
                    .ok_or_else(|| "Extragerea cere numele noii parțiale.".to_string())?,
            )?;
            require_destination_available(workspace, &destination)?;
            let template_reference = destination
                .strip_prefix("templates/")
                .ok_or_else(|| "Destinația parțialei nu are referință Tera validă.".to_string())?;
            let extracted = source_text[extraction_range.start..extraction_range.end].to_string();
            let mut rewritten = source_text;
            rewritten.replace_range(
                extraction_range.start..extraction_range.end,
                &format!("{{% include \"{template_reference}\" %}}"),
            );
            insert_write(
                &mut writes,
                ComponentPlannedWrite {
                    relative_path: source.clone(),
                    contents: rewritten,
                    create_only: false,
                },
            )?;
            insert_write(
                &mut writes,
                ComponentPlannedWrite {
                    relative_path: destination.clone(),
                    contents: extracted,
                    create_only: true,
                },
            )?;
            source_relative_path = Some(source);
            destination_relative_path = Some(destination);
        }
        ComponentMutationOperation::Delete => {
            let definition = require_file_component(definition, input.operation)?;
            require_project_definition(definition, input.operation)?;
            require_capability(definition.capabilities.can_delete, definition, "ștearsă")?;
            if !definition.consumer_invocation_ids.is_empty() {
                return Err(format!(
                    "Componenta {} este folosită de {} invocări și nu poate fi ștearsă fără o strategie explicită de înlocuire.",
                    definition.display_name,
                    definition.consumer_invocation_ids.len()
                ));
            }
            let source = component_source_path(definition)?;
            let source_text = require_workspace_text(workspace, &source)?;
            if definition.kind == ComponentDefinitionKind::TeraComponent {
                let range = require_component_source_range(definition, &source_text)?;
                let mut remaining = source_text;
                remaining.replace_range(range, "");
                if remaining.trim().is_empty() {
                    deletes.insert(source.clone());
                    plan_component_companion_deletes(
                        workspace,
                        &graph,
                        definition,
                        &mut deletes,
                        &mut diagnostics,
                    )?;
                } else {
                    insert_write(
                        &mut writes,
                        ComponentPlannedWrite {
                            relative_path: source.clone(),
                            contents: remaining,
                            create_only: false,
                        },
                    )?;
                }
                source_symbol = definition.symbol.clone();
            } else {
                deletes.insert(source.clone());
                plan_component_companion_deletes(
                    workspace,
                    &graph,
                    definition,
                    &mut deletes,
                    &mut diagnostics,
                )?;
            }
            source_relative_path = Some(source);
        }
        ComponentMutationOperation::OverrideTheme => {
            let definition = require_file_component(definition, input.operation)?;
            if definition.origin != ComponentOrigin::Theme {
                return Err(
                    "Override-ul local cere o definiție provenită din tema activă.".to_string(),
                );
            }
            let source = component_source_path(definition)?;
            let template_name = definition.template_name.as_deref().ok_or_else(|| {
                "Definiția temei nu expune numele logic al template-ului.".to_string()
            })?;
            let destination =
                normalize_project_relative_path(&format!("templates/{template_name}"))?;
            require_destination_available(workspace, &destination)?;
            insert_write(
                &mut writes,
                ComponentPlannedWrite {
                    relative_path: destination.clone(),
                    contents: require_workspace_text(workspace, &source)?,
                    create_only: true,
                },
            )?;
            plan_component_companion_bundle(
                workspace,
                &graph,
                definition,
                &destination,
                ComponentCompanionTransfer::Copy,
                &mut writes,
                &mut deletes,
                &mut diagnostics,
            )?;
            source_relative_path = Some(source);
            destination_relative_path = Some(destination);
            source_symbol = definition.symbol.clone();
            destination_symbol = definition.symbol.clone();
        }
    }

    if !companion_drafts.is_empty()
        && !matches!(
            input.operation,
            ComponentMutationOperation::Create
                | ComponentMutationOperation::Update
                | ComponentMutationOperation::Extract
        )
    {
        return Err(
            "Resursele companion pot fi modificate atomic numai la creare sau editare.".to_string(),
        );
    }
    require_no_duplicate_companions(&companion_drafts)?;
    for companion in companion_drafts {
        let relative_path = normalize_companion_path(companion.kind, &companion.relative_path)?;
        if companion.create_only {
            require_destination_available(workspace, &relative_path)?;
        } else if workspace.documents.text_for(&relative_path).is_none() {
            return Err(format!(
                "Resursa companion {relative_path} nu există; marcheaz-o createOnly pentru creare."
            ));
        }
        insert_write(
            &mut writes,
            ComponentPlannedWrite {
                relative_path,
                contents: companion.contents,
                create_only: companion.create_only,
            },
        )?;
    }

    let writes = writes.into_values().collect::<Vec<_>>();
    let deletes = deletes.into_iter().collect::<Vec<_>>();
    let mut touched_files = writes
        .iter()
        .map(|write| write.relative_path.clone())
        .chain(deletes.iter().cloned())
        .collect::<Vec<_>>();
    touched_files.sort();
    touched_files.dedup();
    diagnostics.push(ComponentMutationDiagnostic {
        diagnostic: LocalizedDiagnostic::new("component-mutation-semantic-preflight-passed"),
        relative_path: destination_relative_path
            .clone()
            .or_else(|| source_relative_path.clone()),
    });

    Ok(ComponentMutationPlan {
        schema_version: COMPONENT_MUTATION_SCHEMA_VERSION,
        operation: input.operation,
        definition_id: input.definition_id,
        source_relative_path,
        destination_relative_path,
        source_symbol,
        destination_symbol,
        writes,
        deletes,
        touched_files,
        diagnostics,
    })
}

pub fn stage_validated_component_mutation(
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    input: ComponentMutationInput,
    now_ms: u128,
) -> Result<(ComponentMutationPlan, ProjectWorkspaceMutationReceipt), String> {
    let plan = plan_component_mutation(project_root, workspace, input)?;
    let mut candidate = workspace.fork_candidate();
    let candidate_identity = current_identity(&candidate);
    candidate.stage_composite_changes(
        &candidate_identity,
        mutation_metadata(&plan),
        plan.workspace_writes(),
        plan.workspace_deletes(),
        None,
        now_ms,
    )?;
    validate_component_candidate(project_root, &candidate, &plan)?;

    let identity = current_identity(workspace);
    let receipt = workspace.stage_composite_changes(
        &identity,
        mutation_metadata(&plan),
        plan.workspace_writes(),
        plan.workspace_deletes(),
        None,
        now_ms,
    )?;
    Ok((plan, receipt))
}

fn validate_component_candidate(
    project_root: &Path,
    candidate: &ProjectWorkspace,
    plan: &ComponentMutationPlan,
) -> Result<(), String> {
    let graph = validate_component_workspace_candidate(project_root, candidate)?;

    match plan.operation {
        ComponentMutationOperation::Delete => {
            if let (Some(source), Some(symbol)) = (
                plan.source_relative_path.as_deref(),
                plan.source_symbol.as_deref(),
            ) {
                if component_symbol_exists_at(&graph, source, symbol) {
                    return Err(format!(
                        "Validarea semantică a refuzat ștergerea: simbolul {symbol} din {source} există încă."
                    ));
                }
            } else if let Some(source) = plan.source_relative_path.as_deref() {
                if graph
                    .component_graph
                    .definitions
                    .iter()
                    .any(|definition| definition.file.as_deref() == Some(source))
                {
                    return Err(format!(
                        "Validarea semantică a refuzat ștergerea: definiția din {source} există încă."
                    ));
                }
            }
        }
        ComponentMutationOperation::Update => {
            let destination = plan.destination_relative_path.as_deref().ok_or_else(|| {
                "Planul de editare nu păstrează path-ul final al definiției.".to_string()
            })?;
            require_active_project_definition_at(
                &graph,
                destination,
                plan.destination_symbol.as_deref(),
            )?;
        }
        _ => {
            if let Some(destination) = plan.destination_relative_path.as_deref() {
                require_active_project_definition_at(
                    &graph,
                    destination,
                    plan.destination_symbol.as_deref(),
                )?;
            }
        }
    }
    Ok(())
}

pub(super) fn validate_component_workspace_candidate(
    project_root: &Path,
    candidate: &ProjectWorkspace,
) -> Result<SourceGraph, String> {
    validate_semantic_workspace_candidate(project_root, candidate, "Mutația componentei")
}

pub(crate) fn validate_semantic_workspace_candidate(
    project_root: &Path,
    candidate: &ProjectWorkspace,
    mutation_label: &str,
) -> Result<SourceGraph, String> {
    let projection = candidate.capture_projection_snapshot()?;
    let graph = build_source_graph_from_workspace_projection(project_root, &projection)?;
    require_graph_without_errors_for(&graph, "candidatul mutației", mutation_label)?;
    validate_candidate_with_embedded_zola(project_root, candidate, &projection, mutation_label)?;
    Ok(graph)
}

fn validate_candidate_with_embedded_zola(
    project_root: &Path,
    candidate: &ProjectWorkspace,
    projection: &crate::kernel::project_workspace::WorkspaceProjectionSnapshot,
    mutation_label: &str,
) -> Result<(), String> {
    candidate.accepted_disk.require_live_complete(
        &candidate.runtime_session_id(),
        &candidate.session.project_root,
        project_root,
    )?;
    let generation = COMPONENT_VALIDATION_GENERATION.fetch_add(1, Ordering::Relaxed);
    let validation_root = std::env::temp_dir().join(format!(
        "pana-component-zola-validation-{}-{generation}",
        std::process::id()
    ));
    let sandbox = ComponentValidationSandboxLease::capture(&validation_root).map_err(|error| {
        format!("Nu am putut captura sandbox-ul privat pentru validarea Zola embedded: {error}")
    })?;
    let stable_validation_root = sandbox.current_dir_path();

    let validation = (|| {
        materialize_component_validation_projection(project_root, &sandbox, projection)?;
        let zola_relative = Path::new(&candidate.session.zola_root)
            .strip_prefix(project_root)
            .map_err(|_| {
                "Zola root nu aparține rădăcinii proiectului validat de ProjectWorkspace."
                    .to_string()
            })?;
        let validation_zola_root = stable_validation_root.join(zola_relative);
        run_zola_editor_check(&stable_validation_root, &validation_zola_root)
            .map(|_| ())
            .map_err(|error| {
                format!(
                    "{mutation_label} a fost respinsă de Zola embedded pe candidatul complet: {error}"
                )
        })
    })();

    let cleanup = sandbox.discard().map_err(|error| {
        format!(
            "Sandbox-ul validării Zola embedded {} nu a putut fi eliminat: {error}",
            validation_root.display()
        )
    });
    match (validation, cleanup) {
        (Err(error), Err(cleanup_error)) => Err(format!("{error} {cleanup_error}")),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn materialize_component_validation_projection(
    project_root: &Path,
    sandbox: &ComponentValidationSandboxLease,
    projection: &crate::kernel::project_workspace::WorkspaceProjectionSnapshot,
) -> Result<(), String> {
    for entry in &projection.accepted_disk.manifest.files {
        let relative_path = normalize_project_relative_path(&entry.relative_path)?;
        if projection.deleted_sources.contains(&relative_path)
            || projection.source_texts.contains_key(&relative_path)
            || projection.resource_bytes.contains_key(&relative_path)
        {
            continue;
        }
        let source = project_root.join(&relative_path);
        let metadata = fs::symlink_metadata(&source).map_err(|error| {
            format!(
                "Validarea Zola embedded nu poate inspecta sursa acceptată {relative_path}: {error}"
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(format!(
                "Validarea Zola embedded a refuzat sursa acceptată care nu mai este fișier regulat: {relative_path}."
            ));
        }
        sandbox
            .copy_regular_file(&source, Path::new(&relative_path))
            .map_err(|error| {
                format!("Validarea Zola embedded nu a putut proiecta {relative_path}: {error}")
            })?;
    }

    let mut text_paths = projection.source_texts.keys().collect::<Vec<_>>();
    text_paths.sort();
    for relative_path in text_paths {
        let normalized = normalize_project_relative_path(relative_path)?;
        if projection.deleted_sources.contains(&normalized) {
            continue;
        }
        sandbox
            .write_bytes(
                Path::new(&normalized),
                projection
                    .source_texts
                    .get(relative_path)
                    .expect("path capturat din aceeași hartă")
                    .as_bytes(),
            )
            .map_err(|error| {
                format!(
                    "Validarea Zola embedded nu a putut materializa textul {normalized}: {error}"
                )
            })?;
    }

    let mut resource_paths = projection.resource_bytes.keys().collect::<Vec<_>>();
    resource_paths.sort();
    for relative_path in resource_paths {
        let normalized = normalize_project_relative_path(relative_path)?;
        if projection.deleted_sources.contains(&normalized) {
            continue;
        }
        sandbox
            .write_bytes(
                Path::new(&normalized),
                projection
                    .resource_bytes
                    .get(relative_path)
                    .expect("path capturat din aceeași hartă"),
            )
            .map_err(|error| {
                format!(
                    "Validarea Zola embedded nu a putut materializa resursa {normalized}: {error}"
                )
            })?;
    }
    Ok(())
}

fn require_graph_without_errors(graph: &SourceGraph, stage: &str) -> Result<(), String> {
    require_graph_without_errors_for(graph, stage, "Mutația componentei")
}

fn require_graph_without_errors_for(
    graph: &SourceGraph,
    _stage: &str,
    _mutation_label: &str,
) -> Result<(), String> {
    if let Some(diagnostic) = graph
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.severity == SourceDiagnosticSeverity::Error)
    {
        return Err(localized_diagnostic_error(&diagnostic.diagnostic));
    }
    if let Some(diagnostic) = graph
        .component_graph
        .diagnostics
        .iter()
        .chain(
            graph
                .component_graph
                .definitions
                .iter()
                .flat_map(|definition| definition.diagnostics.iter()),
        )
        .chain(
            graph
                .component_graph
                .invocations
                .iter()
                .flat_map(|invocation| invocation.diagnostics.iter()),
        )
        .find(|diagnostic| diagnostic.severity == SourceDiagnosticSeverity::Error)
    {
        return Err(localized_diagnostic_error(&diagnostic.diagnostic));
    }
    Ok(())
}

fn require_active_project_definition_at(
    graph: &SourceGraph,
    path: &str,
    symbol: Option<&str>,
) -> Result<(), String> {
    if graph.component_graph.definitions.iter().any(|definition| {
        definition.file.as_deref() == Some(path)
            && definition.active
            && definition.origin == ComponentOrigin::Project
            && matches!(
                definition.kind,
                ComponentDefinitionKind::Partial
                    | ComponentDefinitionKind::TemplateFile
                    | ComponentDefinitionKind::TeraComponent
            )
            && symbol.is_none_or(|symbol| definition.symbol.as_deref() == Some(symbol))
    }) {
        Ok(())
    } else {
        Err(format!(
            "Validarea semantică nu găsește o definiție locală activă pentru {path}."
        ))
    }
}

fn require_component_definition<'a>(
    graph: &'a SourceGraph,
    definition_id: &str,
) -> Result<&'a ComponentDefinition, String> {
    graph
        .component_graph
        .definitions
        .iter()
        .find(|definition| definition.id == definition_id)
        .ok_or_else(|| {
            format!("ComponentGraph nu mai conține definiția {definition_id} în revizia curentă.")
        })
}

fn require_file_component(
    definition: Option<&ComponentDefinition>,
    operation: ComponentMutationOperation,
) -> Result<&ComponentDefinition, String> {
    let definition = definition.ok_or_else(|| {
        format!("Operația {operation:?} cere identificatorul unei definiții ComponentGraph.")
    })?;
    if !matches!(
        definition.kind,
        ComponentDefinitionKind::Partial
            | ComponentDefinitionKind::TemplateFile
            | ComponentDefinitionKind::TeraComponent
    ) || definition.file.is_none()
        || definition.template_name.is_none()
    {
        return Err(format!(
            "Definiția {} este un simbol intern sau provider, nu o componentă mutabilă ca fișier.",
            definition.display_name
        ));
    }
    Ok(definition)
}

fn require_project_definition(
    definition: &ComponentDefinition,
    operation: ComponentMutationOperation,
) -> Result<(), String> {
    if definition.origin == ComponentOrigin::Project {
        Ok(())
    } else {
        Err(format!(
            "Operația {operation:?} nu poate modifica direct o definiție din temă sau bibliotecă."
        ))
    }
}

fn require_capability(
    allowed: bool,
    definition: &ComponentDefinition,
    action: &str,
) -> Result<(), String> {
    if allowed {
        Ok(())
    } else {
        Err(definition
            .capabilities
            .reason_diagnostic
            .as_ref()
            .map(localized_diagnostic_error)
            .unwrap_or_else(|| {
                format!(
                    "Componenta {} nu poate fi {action}.",
                    definition.display_name
                )
            }))
    }
}

fn localized_diagnostic_error(diagnostic: &crate::localization::LocalizedDiagnostic) -> String {
    serde_json::to_string(diagnostic).unwrap_or_else(|_| diagnostic.code.clone())
}

fn component_source_path(definition: &ComponentDefinition) -> Result<String, String> {
    definition
        .file
        .as_deref()
        .ok_or_else(|| "Definiția nu are fișier sursă.".to_string())
        .and_then(normalize_project_relative_path)
}

fn validate_component_symbol_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    let valid = !name.is_empty()
        && name.split('.').all(|segment| {
            let mut characters = segment.chars();
            characters
                .next()
                .is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
                && characters.all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
                })
        });
    if valid {
        Ok(name.to_string())
    } else {
        Err(
            "Numele componentei Tera 2 trebuie să conțină segmente identificator separate prin punct."
                .to_string(),
        )
    }
}

fn require_component_symbol_available(
    graph: &SourceGraph,
    symbol: &str,
    current_definition_id: &str,
) -> Result<(), String> {
    if graph.component_graph.definitions.iter().any(|definition| {
        definition.id != current_definition_id
            && definition.active
            && definition.kind == ComponentDefinitionKind::TeraComponent
            && definition.symbol.as_deref() == Some(symbol)
    }) {
        Err(format!(
            "Simbolul componentei Tera 2 {symbol} există deja în proiectul efectiv."
        ))
    } else {
        Ok(())
    }
}

fn component_symbol_exists_at(graph: &SourceGraph, path: &str, symbol: &str) -> bool {
    graph.component_graph.definitions.iter().any(|definition| {
        definition.file.as_deref() == Some(path)
            && definition.active
            && definition.kind == ComponentDefinitionKind::TeraComponent
            && definition.symbol.as_deref() == Some(symbol)
    })
}

fn require_component_source_range(
    definition: &ComponentDefinition,
    source: &str,
) -> Result<Range<usize>, String> {
    let range = definition
        .range
        .as_ref()
        .ok_or_else(|| "Definiția componentei nu are range semantic exact.".to_string())?;
    if range.start >= range.end
        || range.end > source.len()
        || !source.is_char_boundary(range.start)
        || !source.is_char_boundary(range.end)
    {
        return Err("Range-ul componentei nu mai este valid în sursa curentă.".to_string());
    }
    Ok(range.start..range.end)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ComponentSymbolEdit {
    start: usize,
    end: usize,
    replacement: String,
}

fn plan_tera_component_symbol_rename(
    workspace: &ProjectWorkspace,
    graph: &SourceGraph,
    definition: &ComponentDefinition,
    new_symbol: &str,
    writes: &mut BTreeMap<String, ComponentPlannedWrite>,
) -> Result<(), String> {
    let old_symbol = definition
        .symbol
        .as_deref()
        .ok_or_else(|| "Definiția Tera 2 nu are simbol semantic.".to_string())?;
    let definition_file = component_source_path(definition)?;
    let mut edits_by_file = BTreeMap::<String, Vec<ComponentSymbolEdit>>::new();
    let definition_source = require_workspace_text(workspace, &definition_file)?;
    let definition_range = require_component_source_range(definition, &definition_source)?;
    let opening_end = definition
        .body_range
        .as_ref()
        .map_or(definition_range.end, |range| range.start);
    let opening = find_keyword_symbol_span(
        &definition_source,
        definition_range.start..opening_end,
        "component",
        old_symbol,
    )?;
    edits_by_file
        .entry(definition_file.clone())
        .or_default()
        .push(ComponentSymbolEdit {
            start: opening.start,
            end: opening.end,
            replacement: new_symbol.to_string(),
        });
    if let Some(body_range) = definition.body_range.as_ref() {
        let closing = find_keyword_symbol_span(
            &definition_source,
            body_range.end..definition_range.end,
            "endcomponent",
            old_symbol,
        )?;
        edits_by_file
            .entry(definition_file.clone())
            .or_default()
            .push(ComponentSymbolEdit {
                start: closing.start,
                end: closing.end,
                replacement: new_symbol.to_string(),
            });
    }

    for invocation in graph
        .component_graph
        .invocations
        .iter()
        .filter(|invocation| {
            invocation.kind == crate::source_graph::model::ComponentInvocationKind::TeraComponent
                && invocation
                    .resolved_definition_ids
                    .iter()
                    .any(|id| id == &definition.id)
        })
    {
        let source = require_workspace_text(workspace, &invocation.file)?;
        let call_range = invocation
            .call_range
            .as_ref()
            .ok_or_else(|| format!("Invocarea din {} nu are callRange exact.", invocation.file))?;
        let opening =
            find_call_symbol_span(&source, call_range.start..call_range.end, old_symbol, false)?;
        edits_by_file
            .entry(invocation.file.clone())
            .or_default()
            .push(ComponentSymbolEdit {
                start: opening.start,
                end: opening.end,
                replacement: new_symbol.to_string(),
            });
        if let (Some(body_range), Some(range)) =
            (invocation.body_range.as_ref(), invocation.range.as_ref())
        {
            let closing =
                find_call_symbol_span(&source, body_range.end..range.end, old_symbol, true)?;
            edits_by_file
                .entry(invocation.file.clone())
                .or_default()
                .push(ComponentSymbolEdit {
                    start: closing.start,
                    end: closing.end,
                    replacement: new_symbol.to_string(),
                });
        }
    }

    for (file, mut edits) in edits_by_file {
        let mut source = require_workspace_text(workspace, &file)?;
        edits.sort_by_key(|edit| (edit.start, edit.end));
        edits.dedup_by(|left, right| left.start == right.start && left.end == right.end);
        for pair in edits.windows(2) {
            if pair[0].end > pair[1].start {
                return Err(format!(
                    "Redenumirea componentei produce editări suprapuse în {file}."
                ));
            }
        }
        for edit in edits.into_iter().rev() {
            if edit.start >= edit.end
                || edit.end > source.len()
                || !source.is_char_boundary(edit.start)
                || !source.is_char_boundary(edit.end)
            {
                return Err(format!(
                    "Range-ul de redenumire nu mai este valid în {file}."
                ));
            }
            source.replace_range(edit.start..edit.end, &edit.replacement);
        }
        insert_write(
            writes,
            ComponentPlannedWrite {
                relative_path: file,
                contents: source,
                create_only: false,
            },
        )?;
    }
    Ok(())
}

fn find_keyword_symbol_span(
    source: &str,
    search: Range<usize>,
    keyword: &str,
    expected: &str,
) -> Result<Range<usize>, String> {
    let slice = source.get(search.clone()).ok_or_else(|| {
        "Range-ul semantic al componentei nu mai aparține sursei curente.".to_string()
    })?;
    let keyword_start = slice
        .find(keyword)
        .ok_or_else(|| format!("Tag-ul {keyword} nu mai există în range-ul semantic."))?;
    let mut relative_start = keyword_start + keyword.len();
    while slice[relative_start..]
        .chars()
        .next()
        .is_some_and(char::is_whitespace)
    {
        relative_start += slice[relative_start..]
            .chars()
            .next()
            .expect("caracter verificat")
            .len_utf8();
    }
    let relative_end = relative_start
        + slice[relative_start..]
            .find(|character: char| {
                character == '(' || character.is_whitespace() || character == '%'
            })
            .unwrap_or(slice.len() - relative_start);
    let actual = &slice[relative_start..relative_end];
    if actual != expected {
        return Err(format!(
            "Simbolul semantic așteptat {expected} nu mai corespunde sursei ({actual})."
        ));
    }
    Ok((search.start + relative_start)..(search.start + relative_end))
}

fn find_call_symbol_span(
    source: &str,
    search: Range<usize>,
    expected: &str,
    closing: bool,
) -> Result<Range<usize>, String> {
    let slice = source.get(search.clone()).ok_or_else(|| {
        "Range-ul semantic al apelului nu mai aparține sursei curente.".to_string()
    })?;
    let marker = if closing { "</" } else { "<" };
    let marker_start = slice
        .find(marker)
        .ok_or_else(|| "Tag-ul apelului componentei nu mai există în range.".to_string())?;
    let mut relative_start = marker_start + marker.len();
    while slice[relative_start..]
        .chars()
        .next()
        .is_some_and(char::is_whitespace)
    {
        relative_start += slice[relative_start..]
            .chars()
            .next()
            .expect("caracter verificat")
            .len_utf8();
    }
    let relative_end = relative_start
        + slice[relative_start..]
            .find(|character: char| {
                character.is_whitespace() || matches!(character, '/' | '>' | '%' | '}')
            })
            .unwrap_or(slice.len() - relative_start);
    let actual = &slice[relative_start..relative_end];
    if actual != expected {
        return Err(format!(
            "Apelul semantic așteptat {expected} nu mai corespunde sursei ({actual})."
        ));
    }
    Ok((search.start + relative_start)..(search.start + relative_end))
}

fn draft_kind_for_definition(
    definition: &ComponentDefinition,
) -> Result<ComponentDraftKind, String> {
    match definition.kind {
        ComponentDefinitionKind::TeraComponent => Ok(ComponentDraftKind::TeraComponent),
        ComponentDefinitionKind::Partial | ComponentDefinitionKind::TemplateFile => Ok(
            if definition
                .file
                .as_deref()
                .is_some_and(|path| path.starts_with("templates/components/"))
            {
                ComponentDraftKind::TeraComponent
            } else {
                ComponentDraftKind::Partial
            },
        ),
        _ => Err("Tipul definiției nu are un draft de componentă mutabil.".to_string()),
    }
}

fn component_path(kind: ComponentDraftKind, name: &str) -> Result<String, String> {
    let mut logical = name.trim().replace('\\', "/");
    for prefix in [
        "templates/partials/",
        "templates/components/",
        "partials/",
        "components/",
    ] {
        if let Some(stripped) = logical.strip_prefix(prefix) {
            logical = stripped.to_string();
            break;
        }
    }
    logical = logical
        .trim_end_matches(".html")
        .trim_end_matches(".md")
        .to_string();
    if logical.is_empty() {
        return Err("Numele componentei este obligatoriu.".to_string());
    }
    let (directory, extension) = match kind {
        ComponentDraftKind::Partial => ("templates/partials", "html"),
        ComponentDraftKind::TeraComponent => ("templates/components", "html"),
    };
    let path = normalize_project_relative_path(&format!("{directory}/{logical}.{extension}"))?;
    if !path.starts_with(&format!("{directory}/")) || !path.ends_with(&format!(".{extension}")) {
        return Err(
            "Path-ul componentei nu respectă spațiul semantic al tipului ales.".to_string(),
        );
    }
    Ok(path)
}

fn component_draft(kind: ComponentDraftKind, path: &str) -> String {
    let stem = Path::new(path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("componenta");
    match kind {
        ComponentDraftKind::Partial => {
            format!("<section class=\"{stem}\">\n  Componentă nouă\n</section>\n")
        }
        ComponentDraftKind::TeraComponent => format!(
            "{{% component {stem}(text: string) %}}\n  <span>{{{{ text }}}}</span>\n{{% endcomponent {stem} %}}\n"
        ),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ComponentCompanionTransfer {
    Copy,
    Relocate,
}

// Keep causal source, destination and the three independent mutation accumulators explicit.
#[allow(clippy::too_many_arguments)]
fn plan_component_companion_bundle(
    workspace: &ProjectWorkspace,
    graph: &SourceGraph,
    definition: &ComponentDefinition,
    destination_component_path: &str,
    transfer: ComponentCompanionTransfer,
    writes: &mut BTreeMap<String, ComponentPlannedWrite>,
    deletes: &mut BTreeSet<String>,
    diagnostics: &mut Vec<ComponentMutationDiagnostic>,
) -> Result<(), String> {
    for dependency in component_file_companions(definition) {
        let source = normalize_project_relative_path(&dependency.reference)?;
        let source_contents = require_workspace_text(workspace, &source)?;
        let Some(destination) =
            component_companion_destination(destination_component_path, dependency, &source)?
        else {
            continue;
        };
        if source == destination {
            continue;
        }
        plan_compatible_companion_write(workspace, writes, &destination, &source_contents)?;

        let shared = component_companion_has_other_consumers(graph, definition, dependency);
        if transfer == ComponentCompanionTransfer::Relocate && !shared {
            deletes.insert(source.clone());
        }
        diagnostics.push(ComponentMutationDiagnostic {
            diagnostic: if transfer == ComponentCompanionTransfer::Relocate && !shared {
                LocalizedDiagnostic::new("component-mutation-companion-relocated")
                    .with_argument("source", source.clone())
                    .with_argument("destination", destination.clone())
            } else if shared {
                LocalizedDiagnostic::new("component-mutation-companion-shared-copied")
                    .with_argument("source", source.clone())
                    .with_argument("destination", destination.clone())
            } else {
                LocalizedDiagnostic::new("component-mutation-companion-copied")
                    .with_argument("source", source.clone())
                    .with_argument("destination", destination.clone())
            },
            relative_path: Some(destination),
        });
    }
    Ok(())
}

fn plan_component_companion_deletes(
    workspace: &ProjectWorkspace,
    graph: &SourceGraph,
    definition: &ComponentDefinition,
    deletes: &mut BTreeSet<String>,
    diagnostics: &mut Vec<ComponentMutationDiagnostic>,
) -> Result<(), String> {
    for dependency in component_file_companions(definition) {
        let source = normalize_project_relative_path(&dependency.reference)?;
        require_workspace_text(workspace, &source)?;
        if component_companion_has_other_consumers(graph, definition, dependency) {
            diagnostics.push(ComponentMutationDiagnostic {
                diagnostic: LocalizedDiagnostic::new("component-mutation-companion-retained")
                    .with_argument("source", source.clone()),
                relative_path: Some(source),
            });
            continue;
        }
        deletes.insert(source.clone());
        diagnostics.push(ComponentMutationDiagnostic {
            diagnostic: LocalizedDiagnostic::new("component-mutation-companion-deleted")
                .with_argument("source", source.clone()),
            relative_path: Some(source),
        });
    }
    Ok(())
}

fn component_file_companions(
    definition: &ComponentDefinition,
) -> impl Iterator<Item = &ComponentDependency> {
    definition.dependencies.iter().filter(|dependency| {
        dependency.resolved
            && matches!(
                dependency.kind,
                ComponentDependencyKind::Style | ComponentDependencyKind::Script
            )
            && !dependency.reference.contains("://")
    })
}

fn component_companion_has_other_consumers(
    graph: &SourceGraph,
    owner: &ComponentDefinition,
    dependency: &ComponentDependency,
) -> bool {
    let used_by_other_definition = graph.component_graph.definitions.iter().any(|candidate| {
        candidate.id != owner.id
            && candidate.active
            && candidate.owner_definition_id.is_none()
            && candidate.dependencies.iter().any(|candidate_dependency| {
                candidate_dependency.kind == dependency.kind
                    && match (
                        candidate_dependency.target_node_id.as_deref(),
                        dependency.target_node_id.as_deref(),
                    ) {
                        (Some(candidate_target), Some(target)) => candidate_target == target,
                        _ => candidate_dependency.reference == dependency.reference,
                    }
            })
    });
    let used_by_other_source_relation =
        dependency.target_node_id.as_deref().is_some_and(|target| {
            graph.relations.iter().any(|relation| {
                relation.to == target
                    && owner
                        .source_node_id
                        .as_deref()
                        .is_none_or(|owner_node| relation.from != owner_node)
            })
        });
    used_by_other_definition || used_by_other_source_relation
}

fn component_companion_destination(
    destination_component_path: &str,
    dependency: &ComponentDependency,
    source_companion_path: &str,
) -> Result<Option<String>, String> {
    let template_name = destination_component_path
        .strip_prefix("templates/")
        .ok_or_else(|| {
            format!(
                "Destinația {destination_component_path} nu este un template local pentru resurse companion."
            )
        })?;
    let origin = ZolaTemplateOrigin::Local;
    let destination = match dependency.kind {
        ComponentDependencyKind::Style => {
            let candidates = conventional_style_files_for_template(template_name, &origin, true);
            let source_uses_partial_prefix = Path::new(source_companion_path)
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with('_'));
            candidates.into_iter().find(|candidate| {
                Path::new(candidate)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with('_') == source_uses_partial_prefix)
            })
        }
        ComponentDependencyKind::Script => {
            conventional_script_files_for_template(template_name, &origin, true)
                .into_iter()
                .next()
        }
        _ => None,
    };
    destination
        .map(|path| normalize_project_relative_path(&path))
        .transpose()
}

fn plan_compatible_companion_write(
    workspace: &ProjectWorkspace,
    writes: &mut BTreeMap<String, ComponentPlannedWrite>,
    destination: &str,
    contents: &str,
) -> Result<(), String> {
    if let Some(existing) = workspace.documents.text_for(destination) {
        if existing == contents {
            return Ok(());
        }
        return Err(format!(
            "Destinația companion {destination} există deja cu alt conținut."
        ));
    }
    insert_write(
        writes,
        ComponentPlannedWrite {
            relative_path: destination.to_string(),
            contents: contents.to_string(),
            create_only: true,
        },
    )
}

fn normalize_companion_path(kind: ComponentCompanionKind, path: &str) -> Result<String, String> {
    let path = normalize_project_relative_path(path)?;
    let valid = match kind {
        ComponentCompanionKind::Style => {
            (path.starts_with("sass/")
                && matches!(
                    Path::new(&path)
                        .extension()
                        .and_then(|value| value.to_str()),
                    Some("scss" | "sass")
                ))
                || (path.starts_with("static/") && path.ends_with(".css"))
        }
        ComponentCompanionKind::Script => path.starts_with("static/") && path.ends_with(".js"),
        ComponentCompanionKind::Data => path.starts_with("date/") && path.ends_with(".toml"),
    };
    if valid {
        Ok(path)
    } else {
        Err(format!(
            "Resursa companion {path} nu respectă rădăcina și extensia tipului {kind:?}."
        ))
    }
}

fn require_workspace_text(workspace: &ProjectWorkspace, path: &str) -> Result<String, String> {
    workspace
        .documents
        .text_for(path)
        .ok_or_else(|| format!("ProjectWorkspace nu urmărește sursa text {path}."))
}

fn require_destination_available(workspace: &ProjectWorkspace, path: &str) -> Result<(), String> {
    if workspace.documents.files.contains_key(path) {
        Err(format!(
            "Destinația {path} există deja în ProjectWorkspace."
        ))
    } else {
        Ok(())
    }
}

fn require_no_duplicate_companions(companions: &[ComponentCompanionDraft]) -> Result<(), String> {
    let mut paths = BTreeSet::new();
    for companion in companions {
        let path = companion.relative_path.trim().replace('\\', "/");
        if !paths.insert(path.clone()) {
            return Err(format!(
                "Planul componentei conține de două ori resursa companion {path}."
            ));
        }
    }
    Ok(())
}

fn insert_write(
    writes: &mut BTreeMap<String, ComponentPlannedWrite>,
    write: ComponentPlannedWrite,
) -> Result<(), String> {
    if writes.contains_key(&write.relative_path) {
        return Err(format!(
            "Planul componentei produce două variante pentru {}.",
            write.relative_path
        ));
    }
    writes.insert(write.relative_path.clone(), write);
    Ok(())
}

fn plan_component_reference_rewrites(
    project_root: &Path,
    workspace: &ProjectWorkspace,
    graph: &SourceGraph,
    source: &str,
    destination: &str,
    writes: &mut BTreeMap<String, ComponentPlannedWrite>,
) -> Result<(), String> {
    let rewrite = plan_template_reference_workspace_mutation_from_graph(
        project_root,
        &workspace.documents,
        graph,
        SourceGraphRewriteOperation::Rename,
        source,
        destination,
    )?;
    if let Some(reference_mutation) = rewrite.workspace_mutation {
        for change in reference_mutation.changes {
            if change.relative_path != source {
                insert_write(
                    writes,
                    ComponentPlannedWrite {
                        relative_path: change.relative_path,
                        contents: change.new_text,
                        create_only: false,
                    },
                )?;
            }
        }
    }
    Ok(())
}

fn current_identity(workspace: &ProjectWorkspace) -> ProjectWorkspaceIdentity {
    ProjectWorkspaceIdentity {
        expected_project_root: workspace.session.project_root.clone(),
        expected_session_id: workspace.runtime_session_id(),
        expected_revision: workspace.revision,
    }
}

fn mutation_metadata(plan: &ComponentMutationPlan) -> WorkspaceMutationMetadata {
    WorkspaceMutationMetadata {
        label: format!("Componentă {:?}", plan.operation),
        source: "components.semantic_mutation".to_string(),
        coalesce_key: None,
        transaction_id: None,
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, fs, path::PathBuf};

    use crate::{
        js::PageJsDraftStore,
        kernel::{
            file_buffer_store::{
                hash_text, FileBufferBaseline, FileBufferEntry, FileBufferStore,
                FileBufferStoreLimits, TextBufferLanguage, TextBufferRole,
            },
            observability::now_ms,
            project_session::{
                ProjectRootFingerprint, ProjectSessionScanSummary, ProjectSessionSnapshot,
            },
            project_workspace::WorkspaceHistoryDirection,
        },
        project::{read_project_disk_manifest, AcceptedProjectDiskManifest},
    };

    use super::*;

    #[test]
    fn create_component_and_toml_companion_is_one_validated_history_entry() {
        let root = test_root("create-bundle");
        let mut workspace = test_workspace(
            &root,
            HashMap::from([(
                "zola.toml".to_string(),
                "base_url = \"https://example.test\"\n".to_string(),
            )]),
        );
        let input = ComponentMutationInput {
            operation: ComponentMutationOperation::Create,
            definition_id: None,
            kind: Some(ComponentDraftKind::Partial),
            name: Some("catalog/card".to_string()),
            symbol_name: None,
            destination_name: None,
            contents: Some("<article>{{ item.title }}</article>\n".to_string()),
            source_file: None,
            source_range: None,
            companions: vec![ComponentCompanionDraft {
                kind: ComponentCompanionKind::Data,
                relative_path: "date/catalog/cards.toml".to_string(),
                contents: "[[cards]]\ntitle = \"Prima\"\n".to_string(),
                create_only: true,
            }],
        };

        let (plan, receipt) =
            stage_validated_component_mutation(&root, &mut workspace, input, 2).unwrap();
        assert_eq!(receipt.history.undo_count, 1);
        assert_eq!(receipt.history.redo_count, 0);
        assert_eq!(
            workspace
                .documents
                .text_for("templates/partials/catalog/card.html")
                .as_deref(),
            Some("<article>{{ item.title }}</article>\n")
        );
        assert!(workspace
            .documents
            .text_for("date/catalog/cards.toml")
            .unwrap()
            .contains("[[cards]]"));
        assert_eq!(plan.touched_files.len(), 2);

        let undo = workspace.undo(&current_identity(&workspace), 3).unwrap();
        assert!(matches!(undo.direction, WorkspaceHistoryDirection::Undo));
        assert!(workspace
            .documents
            .text_for("templates/partials/catalog/card.html")
            .is_none());
        assert!(workspace
            .documents
            .text_for("date/catalog/cards.toml")
            .is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn delete_is_blocked_while_the_component_has_consumers() {
        let root = test_root("delete-consumer");
        let workspace = test_workspace(
            &root,
            HashMap::from([
                (
                    "zola.toml".to_string(),
                    "base_url = \"https://example.test\"\n".to_string(),
                ),
                (
                    "templates/index.html".to_string(),
                    "{% include \"partials/card.html\" %}\n".to_string(),
                ),
                (
                    "templates/partials/card.html".to_string(),
                    "<article>Card</article>\n".to_string(),
                ),
                (
                    "sass/partials/_card.scss".to_string(),
                    ".card { display: grid; }\n".to_string(),
                ),
                (
                    "static/js/card.js".to_string(),
                    "document.querySelector('.card');\n".to_string(),
                ),
            ]),
        );
        let definition_id =
            definition_id_for_path(&root, &workspace, "templates/partials/card.html");
        let error = plan_component_mutation(
            &root,
            &workspace,
            ComponentMutationInput {
                operation: ComponentMutationOperation::Delete,
                definition_id: Some(definition_id),
                kind: None,
                name: None,
                symbol_name: None,
                destination_name: None,
                contents: None,
                source_file: None,
                source_range: None,
                companions: Vec::new(),
            },
        )
        .unwrap_err();
        assert!(error.contains("este folosită de 1 invocări"), "{error}");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn update_can_rename_source_and_rewrite_consumers_in_one_history_entry() {
        let root = test_root("update-and-rename");
        let mut workspace = test_workspace(
            &root,
            HashMap::from([
                (
                    "zola.toml".to_string(),
                    "base_url = \"https://example.test\"\n".to_string(),
                ),
                (
                    "templates/index.html".to_string(),
                    "{% include \"partials/card.html\" %}\n".to_string(),
                ),
                (
                    "templates/partials/card.html".to_string(),
                    "<article>Card</article>\n".to_string(),
                ),
                (
                    "sass/partials/_card.scss".to_string(),
                    ".card { display: grid; }\n".to_string(),
                ),
                (
                    "static/js/card.js".to_string(),
                    "document.querySelector('.card');\n".to_string(),
                ),
            ]),
        );
        let definition_id =
            definition_id_for_path(&root, &workspace, "templates/partials/card.html");
        let (_plan, receipt) = stage_validated_component_mutation(
            &root,
            &mut workspace,
            ComponentMutationInput {
                operation: ComponentMutationOperation::Update,
                definition_id: Some(definition_id),
                kind: None,
                name: None,
                symbol_name: None,
                destination_name: Some("product/card".to_string()),
                contents: Some("<article>{{ product.title }}</article>\n".to_string()),
                source_file: None,
                source_range: None,
                companions: Vec::new(),
            },
            2,
        )
        .unwrap();

        assert_eq!(receipt.history.undo_count, 1);
        assert!(workspace
            .documents
            .text_for("templates/partials/card.html")
            .is_none());
        assert_eq!(
            workspace
                .documents
                .text_for("templates/partials/product/card.html")
                .as_deref(),
            Some("<article>{{ product.title }}</article>\n")
        );
        assert_eq!(
            workspace
                .documents
                .text_for("templates/index.html")
                .as_deref(),
            Some("{% include \"partials/product/card.html\" %}\n")
        );
        assert!(workspace
            .documents
            .text_for("sass/partials/_card.scss")
            .is_none());
        assert_eq!(
            workspace
                .documents
                .text_for("sass/partials/product/_card.scss")
                .as_deref(),
            Some(".card { display: grid; }\n")
        );
        assert!(workspace.documents.text_for("static/js/card.js").is_none());
        assert_eq!(
            workspace
                .documents
                .text_for("static/js/product/card.js")
                .as_deref(),
            Some("document.querySelector('.card');\n")
        );

        workspace.undo(&current_identity(&workspace), 3).unwrap();
        assert!(workspace
            .documents
            .text_for("templates/partials/product/card.html")
            .is_none());
        assert_eq!(
            workspace
                .documents
                .text_for("templates/partials/card.html")
                .as_deref(),
            Some("<article>Card</article>\n")
        );
        assert_eq!(
            workspace
                .documents
                .text_for("templates/index.html")
                .as_deref(),
            Some("{% include \"partials/card.html\" %}\n")
        );
        assert_eq!(
            workspace
                .documents
                .text_for("sass/partials/_card.scss")
                .as_deref(),
            Some(".card { display: grid; }\n")
        );
        assert!(workspace
            .documents
            .text_for("sass/partials/product/_card.scss")
            .is_none());
        assert_eq!(
            workspace.documents.text_for("static/js/card.js").as_deref(),
            Some("document.querySelector('.card');\n")
        );
        assert!(workspace
            .documents
            .text_for("static/js/product/card.js")
            .is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn duplicate_copies_the_component_companion_bundle_in_one_history_entry() {
        let root = test_root("duplicate-bundle");
        let mut workspace = test_workspace(
            &root,
            HashMap::from([
                (
                    "zola.toml".to_string(),
                    "base_url = \"https://example.test\"\n".to_string(),
                ),
                (
                    "templates/partials/card.html".to_string(),
                    "<article>Card</article>\n".to_string(),
                ),
                (
                    "sass/partials/_card.scss".to_string(),
                    ".card { display: grid; }\n".to_string(),
                ),
                (
                    "static/js/card.js".to_string(),
                    "document.querySelector('.card');\n".to_string(),
                ),
            ]),
        );
        let definition_id =
            definition_id_for_path(&root, &workspace, "templates/partials/card.html");

        let (plan, receipt) = stage_validated_component_mutation(
            &root,
            &mut workspace,
            ComponentMutationInput {
                operation: ComponentMutationOperation::Duplicate,
                definition_id: Some(definition_id),
                kind: None,
                name: None,
                symbol_name: None,
                destination_name: Some("catalog/card".to_string()),
                contents: None,
                source_file: None,
                source_range: None,
                companions: Vec::new(),
            },
            2,
        )
        .unwrap();

        assert_eq!(receipt.history.undo_count, 1);
        assert_eq!(plan.touched_files.len(), 3);
        assert_eq!(
            workspace
                .documents
                .text_for("templates/partials/catalog/card.html")
                .as_deref(),
            Some("<article>Card</article>\n")
        );
        assert_eq!(
            workspace
                .documents
                .text_for("sass/partials/catalog/_card.scss")
                .as_deref(),
            Some(".card { display: grid; }\n")
        );
        assert_eq!(
            workspace
                .documents
                .text_for("static/js/catalog/card.js")
                .as_deref(),
            Some("document.querySelector('.card');\n")
        );
        assert!(workspace
            .documents
            .text_for("sass/partials/_card.scss")
            .is_some());
        assert!(workspace.documents.text_for("static/js/card.js").is_some());

        workspace.undo(&current_identity(&workspace), 3).unwrap();
        assert!(workspace
            .documents
            .text_for("templates/partials/catalog/card.html")
            .is_none());
        assert!(workspace
            .documents
            .text_for("sass/partials/catalog/_card.scss")
            .is_none());
        assert!(workspace
            .documents
            .text_for("static/js/catalog/card.js")
            .is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn delete_removes_exclusive_style_but_retains_a_script_used_elsewhere() {
        let root = test_root("delete-bundle");
        let mut workspace = test_workspace(
            &root,
            HashMap::from([
                (
                    "zola.toml".to_string(),
                    "base_url = \"https://example.test\"\n".to_string(),
                ),
                (
                    "templates/index.html".to_string(),
                    "<script src=\"{{ get_url(path='js/card.js') }}\"></script>\n".to_string(),
                ),
                (
                    "templates/partials/card.html".to_string(),
                    "<article>Card</article>\n".to_string(),
                ),
                (
                    "sass/partials/_card.scss".to_string(),
                    ".card { display: grid; }\n".to_string(),
                ),
                (
                    "static/js/card.js".to_string(),
                    "document.querySelector('.card');\n".to_string(),
                ),
            ]),
        );
        let definition_id =
            definition_id_for_path(&root, &workspace, "templates/partials/card.html");

        let (plan, receipt) = stage_validated_component_mutation(
            &root,
            &mut workspace,
            ComponentMutationInput {
                operation: ComponentMutationOperation::Delete,
                definition_id: Some(definition_id),
                kind: None,
                name: None,
                symbol_name: None,
                destination_name: None,
                contents: None,
                source_file: None,
                source_range: None,
                companions: Vec::new(),
            },
            2,
        )
        .unwrap();

        assert_eq!(receipt.history.undo_count, 1);
        assert!(workspace
            .documents
            .text_for("templates/partials/card.html")
            .is_none());
        assert!(workspace
            .documents
            .text_for("sass/partials/_card.scss")
            .is_none());
        assert!(workspace.documents.text_for("static/js/card.js").is_some());
        assert!(plan.diagnostics.iter().any(
            |diagnostic| diagnostic.diagnostic.code == "component-mutation-companion-retained"
        ));

        workspace.undo(&current_identity(&workspace), 3).unwrap();
        assert!(workspace
            .documents
            .text_for("templates/partials/card.html")
            .is_some());
        assert!(workspace
            .documents
            .text_for("sass/partials/_card.scss")
            .is_some());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn extract_uses_an_exact_source_graph_node_and_is_one_undo_entry() {
        let root = test_root("extract-partial");
        let source =
            "<main>\n  <section class=\"hero\"><h1>{{ page.title }}</h1></section>\n</main>\n";
        let mut workspace = test_workspace(
            &root,
            HashMap::from([
                (
                    "zola.toml".to_string(),
                    "base_url = \"https://example.test\"\n".to_string(),
                ),
                ("templates/index.html".to_string(), source.to_string()),
                (
                    "content/_index.md".to_string(),
                    "+++\ntitle = \"Acasă\"\n+++\n".to_string(),
                ),
            ]),
        );
        let expected = "<section class=\"hero\"><h1>{{ page.title }}</h1></section>";
        let start = source.find(expected).unwrap();
        let end = start + expected.len();
        let graph = build_source_graph_from_workspace_projection(
            &root,
            &workspace.capture_projection_snapshot().unwrap(),
        )
        .unwrap();
        assert!(graph.nodes.iter().any(|node| {
            node.file == "templates/index.html"
                && node.capabilities.can_extract_partial
                && node
                    .range
                    .as_ref()
                    .is_some_and(|range| range.start == start && range.end == end)
        }));

        let (_plan, receipt) = stage_validated_component_mutation(
            &root,
            &mut workspace,
            ComponentMutationInput {
                operation: ComponentMutationOperation::Extract,
                definition_id: None,
                kind: Some(ComponentDraftKind::Partial),
                name: Some("hero".to_string()),
                symbol_name: None,
                destination_name: None,
                contents: None,
                source_file: Some("templates/index.html".to_string()),
                source_range: Some(ComponentExtractionRange { start, end }),
                companions: Vec::new(),
            },
            2,
        )
        .unwrap();

        assert_eq!(receipt.history.undo_count, 1);
        assert_eq!(
            workspace
                .documents
                .text_for("templates/partials/hero.html")
                .as_deref(),
            Some(expected)
        );
        assert_eq!(
            workspace
                .documents
                .text_for("templates/index.html")
                .as_deref(),
            Some("<main>\n  {% include \"partials/hero.html\" %}\n</main>\n")
        );

        workspace.undo(&current_identity(&workspace), 3).unwrap();
        assert!(workspace
            .documents
            .text_for("templates/partials/hero.html")
            .is_none());
        assert_eq!(
            workspace
                .documents
                .text_for("templates/index.html")
                .as_deref(),
            Some(source)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_tera_update_is_rejected_without_mutating_workspace() {
        let root = test_root("invalid-update");
        let mut workspace = test_workspace(
            &root,
            HashMap::from([
                (
                    "zola.toml".to_string(),
                    "base_url = \"https://example.test\"\n".to_string(),
                ),
                (
                    "templates/partials/card.html".to_string(),
                    "<article>Card</article>\n".to_string(),
                ),
            ]),
        );
        let definition_id =
            definition_id_for_path(&root, &workspace, "templates/partials/card.html");
        let revision_before = workspace.revision;
        let error = stage_validated_component_mutation(
            &root,
            &mut workspace,
            ComponentMutationInput {
                operation: ComponentMutationOperation::Update,
                definition_id: Some(definition_id),
                kind: None,
                name: None,
                symbol_name: None,
                destination_name: None,
                contents: Some("{% if visible %}<article>{% endfor %}\n".to_string()),
                source_file: None,
                source_range: None,
                companions: Vec::new(),
            },
            2,
        )
        .unwrap_err();
        assert!(
            error.contains("source-graph-tera-syntax-invalid"),
            "{error}"
        );
        assert!(error.contains("EndFor") && error.contains("If"), "{error}");
        assert_eq!(workspace.revision, revision_before);
        assert_eq!(
            workspace
                .documents
                .text_for("templates/partials/card.html")
                .as_deref(),
            Some("<article>Card</article>\n")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn create_typed_namespaced_tera_component_is_validated_by_zola() {
        let root = test_root("create-tera-component");
        let mut workspace = test_workspace(
            &root,
            HashMap::from([(
                "zola.toml".to_string(),
                "base_url = \"https://example.test\"\n".to_string(),
            )]),
        );
        let source = concat!(
            "{% component ui.card(title: string, tone: string = \"neutral\", ...attributes) %}\n",
            "<article class=\"card card--{{ tone }}\">{{ title }}</article>\n",
            "{% endcomponent ui.card %}\n",
        );
        let (plan, receipt) = stage_validated_component_mutation(
            &root,
            &mut workspace,
            ComponentMutationInput {
                operation: ComponentMutationOperation::Create,
                definition_id: None,
                kind: Some(ComponentDraftKind::TeraComponent),
                name: Some("ui/card".to_string()),
                symbol_name: Some("ui.card".to_string()),
                destination_name: None,
                contents: Some(source.to_string()),
                source_file: None,
                source_range: None,
                companions: Vec::new(),
            },
            2,
        )
        .unwrap();

        assert_eq!(plan.schema_version, 3);
        assert_eq!(plan.destination_symbol.as_deref(), Some("ui.card"));
        assert_eq!(receipt.history.undo_count, 1);
        assert_eq!(
            workspace
                .documents
                .text_for("templates/components/ui/card.html")
                .as_deref(),
            Some(source)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rename_tera_component_rewrites_definition_and_all_resolved_calls() {
        let root = test_root("rename-tera-component");
        let mut workspace = test_workspace(
            &root,
            HashMap::from([
                (
                    "zola.toml".to_string(),
                    "base_url = \"https://example.test\"\n".to_string(),
                ),
                (
                    "templates/components.html".to_string(),
                    concat!(
                        "{% component ui.card(title: string) %}<article>{{ title }}{{ body | safe }}</article>{% endcomponent ui.card %}\n",
                        "{{<ui.card title=\"Unu\" />}}\n",
                        "{% <ui.card title=\"Doi\"> %}Corp{% </ui.card> %}\n",
                    )
                    .to_string(),
                ),
            ]),
        );
        let definition_id = definition_id_for_symbol(&root, &workspace, "ui.card");
        let (plan, receipt) = stage_validated_component_mutation(
            &root,
            &mut workspace,
            ComponentMutationInput {
                operation: ComponentMutationOperation::Rename,
                definition_id: Some(definition_id),
                kind: None,
                name: None,
                symbol_name: Some("ui.card".to_string()),
                destination_name: Some("ui.tile".to_string()),
                contents: None,
                source_file: None,
                source_range: None,
                companions: Vec::new(),
            },
            2,
        )
        .unwrap();

        let rewritten = workspace
            .documents
            .text_for("templates/components.html")
            .unwrap();
        assert!(!rewritten.contains("ui.card"), "{rewritten}");
        assert_eq!(rewritten.matches("ui.tile").count(), 5, "{rewritten}");
        assert_eq!(plan.source_symbol.as_deref(), Some("ui.card"));
        assert_eq!(plan.destination_symbol.as_deref(), Some("ui.tile"));
        assert_eq!(receipt.history.undo_count, 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn delete_tera_component_removes_only_the_selected_symbol() {
        let root = test_root("delete-tera-component-symbol");
        let mut workspace = test_workspace(
            &root,
            HashMap::from([
                (
                    "zola.toml".to_string(),
                    "base_url = \"https://example.test\"\n".to_string(),
                ),
                (
                    "templates/components.html".to_string(),
                    concat!(
                        "{% component ui.card() %}<article>Card</article>{% endcomponent ui.card %}\n",
                        "{% component ui.badge() %}<span>Badge</span>{% endcomponent ui.badge %}\n",
                    )
                    .to_string(),
                ),
            ]),
        );
        let definition_id = definition_id_for_symbol(&root, &workspace, "ui.card");
        let (plan, receipt) = stage_validated_component_mutation(
            &root,
            &mut workspace,
            ComponentMutationInput {
                operation: ComponentMutationOperation::Delete,
                definition_id: Some(definition_id),
                kind: None,
                name: None,
                symbol_name: Some("ui.card".to_string()),
                destination_name: None,
                contents: None,
                source_file: None,
                source_range: None,
                companions: Vec::new(),
            },
            2,
        )
        .unwrap();

        let remaining = workspace
            .documents
            .text_for("templates/components.html")
            .unwrap();
        assert!(!remaining.contains("ui.card"), "{remaining}");
        assert!(remaining.contains("ui.badge"), "{remaining}");
        assert!(plan.deletes.is_empty());
        assert_eq!(receipt.history.undo_count, 1);
        fs::remove_dir_all(root).unwrap();
    }

    fn definition_id_for_path(
        root: &Path,
        workspace: &ProjectWorkspace,
        relative_path: &str,
    ) -> String {
        let projection = workspace.capture_projection_snapshot().unwrap();
        build_source_graph_from_workspace_projection(root, &projection)
            .unwrap()
            .component_graph
            .definitions
            .into_iter()
            .find(|definition| {
                definition.file.as_deref() == Some(relative_path) && definition.active
            })
            .map(|definition| definition.id)
            .unwrap()
    }

    fn definition_id_for_symbol(root: &Path, workspace: &ProjectWorkspace, symbol: &str) -> String {
        let projection = workspace.capture_projection_snapshot().unwrap();
        build_source_graph_from_workspace_projection(root, &projection)
            .unwrap()
            .component_graph
            .definitions
            .into_iter()
            .find(|definition| {
                definition.kind == ComponentDefinitionKind::TeraComponent
                    && definition.symbol.as_deref() == Some(symbol)
                    && definition.active
            })
            .map(|definition| definition.id)
            .unwrap()
    }

    fn test_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "pana-component-mutation-{label}-{}-{}",
            std::process::id(),
            now_ms()
        ));
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("date")).unwrap();
        root
    }

    fn test_workspace(root: &Path, sources: HashMap<String, String>) -> ProjectWorkspace {
        for (path, source) in &sources {
            let absolute = root.join(path);
            if let Some(parent) = absolute.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(absolute, source).unwrap();
        }
        let canonical = root.canonicalize().unwrap().to_string_lossy().to_string();
        let session = ProjectSessionSnapshot {
            schema_version: 1,
            id: "component-mutation-test".to_string(),
            project_root: canonical.clone(),
            zola_root: canonical.clone(),
            session_dir: root.join("session").to_string_lossy().to_string(),
            manifest_path: root.join("session.json").to_string_lossy().to_string(),
            opened_at_ms: 1,
            last_seen_at_ms: 1,
            root_fingerprint: ProjectRootFingerprint {
                canonical_path: canonical.clone(),
                modified_ms: 1,
                size: 0,
                readonly: false,
                unix_device: None,
                unix_inode: None,
            },
            scan_summary: ProjectSessionScanSummary {
                active_theme: None,
                file_count: sources.len(),
                directory_count: 3,
            },
        };
        let mut documents = FileBufferStore::for_project_session(
            &session,
            1,
            FileBufferStoreLimits {
                max_files: 128,
                max_file_bytes: 1024 * 1024,
                max_total_bytes: 8 * 1024 * 1024,
            },
        );
        let mut sorted_sources = sources.into_iter().collect::<Vec<_>>();
        sorted_sources.sort_by(|left, right| left.0.cmp(&right.0));
        for (relative_path, source) in sorted_sources {
            let (language, role) = language_and_role(&relative_path);
            documents.insert_loaded_file(FileBufferEntry {
                relative_path: relative_path.clone(),
                absolute_path: root.join(&relative_path).to_string_lossy().to_string(),
                language,
                role,
                baseline: FileBufferBaseline {
                    hash: hash_text(&source),
                    modified_ms: 1,
                    size: source.len() as u64,
                    readonly: false,
                },
                baseline_text: source.clone().into(),
                draft: None,
                revision: 1,
            });
        }
        let manifest = read_project_disk_manifest(root).unwrap();
        let accepted = AcceptedProjectDiskManifest::new(
            session.runtime_instance_id(),
            session.project_root.clone(),
            manifest,
        )
        .unwrap();
        let page_js = PageJsDraftStore::new(&session);
        ProjectWorkspace::new(session, accepted, documents, page_js).unwrap()
    }

    fn language_and_role(path: &str) -> (TextBufferLanguage, TextBufferRole) {
        if path.ends_with(".html") {
            (TextBufferLanguage::Html, TextBufferRole::Template)
        } else if path.starts_with("content/") && path.ends_with(".md") {
            (TextBufferLanguage::Markdown, TextBufferRole::Page)
        } else if path.ends_with(".toml") {
            let role = if path == "zola.toml" {
                TextBufferRole::Config
            } else {
                TextBufferRole::Data
            };
            (TextBufferLanguage::Toml, role)
        } else {
            (TextBufferLanguage::Plain, TextBufferRole::Other)
        }
    }
}
